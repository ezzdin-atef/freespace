//! Parallel disk walk. One pass builds the space-usage tree and collects everything cleanable.

use crate::projects::{self, Project, Seed};
use crate::rules::{self, Category, DeleteMode, ItemKind, Known, Safety};
use crate::{safety, win};
use rayon::prelude::*;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const LARGE_FILE: u64 = 512 * 1024 * 1024;
const TREE_MIN: u64 = 1024 * 1024;
const TREE_MAX_CHILDREN: usize = 80;
const TREE_MAX_DEPTH: u32 = 16;
const OLD_LOG: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Clone, Debug)]
pub struct Finding {
    pub category: Category,
    pub path: PathBuf,
    pub size: u64,
    pub files: u64,
    pub safety: Safety,
    pub note: String,
    pub mode: DeleteMode,
    pub kind: ItemKind,
}

pub struct DirNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub files: u64,
    /// Set when the whole folder is a cleanable finding.
    pub tag: Option<Category>,
    pub children: Vec<DirNode>,
    /// Last edit in here, not counting cleanable folders, `.git` or nested projects.
    pub newest: SystemTime,
    /// Project marker files in here, with the same exclusions.
    pub markers: u32,
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub current: Mutex<String>,
    pub cancel: AtomicBool,
}

pub struct ScanResult {
    pub root: PathBuf,
    pub tree: DirNode,
    pub findings: Vec<Finding>,
    pub projects: Vec<Project>,
    pub elapsed: Duration,
    pub files: u64,
    pub dirs: u64,
    pub errors: u64,
    pub cancelled: bool,
}

pub fn start(root: PathBuf, progress: Arc<Progress>) -> Receiver<ScanResult> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(root, &progress));
    });
    rx
}

#[derive(Clone, Copy)]
struct Ctx {
    depth: u32,
    /// Inside Windows / Program Files / ProgramData: no dev or log matching.
    protected: bool,
    /// May match build-artifact patterns (false in AppData and dot-folders).
    dev_ok: bool,
    /// Inside a folder that is already a finding: just count bytes.
    in_finding: bool,
    /// Inside a database folder where `.log` files are journals.
    no_logs: bool,
    /// Inside a folder named log/logs.
    logs_dir: bool,
    /// Inside a project: only a real git repository starts another one.
    in_project: bool,
    /// Inside a package store, where nothing starts a project.
    no_project: bool,
}

struct Scanner<'a> {
    known: HashMap<String, &'static Known>,
    progress: &'a Progress,
    findings: Mutex<Vec<Finding>>,
    seeds: Mutex<Vec<Seed>>,
    now: SystemTime,
}

fn run(root: PathBuf, progress: &Progress) -> ScanResult {
    let started = Instant::now();
    let root_norm = safety::norm(&root);
    let known = rules::expand_known()
        .into_iter()
        .map(|(p, k)| (safety::norm(&p), k))
        .filter(|(p, _)| *p != root_norm && safety::is_within(p, &root_norm))
        .collect();

    let sc = Scanner { known, progress, findings: Mutex::new(Vec::new()), seeds: Mutex::new(Vec::new()), now: SystemTime::now() };
    let protected = safety::is_protected_area(&root_norm);
    let ctx = Ctx {
        depth: 0,
        protected,
        dev_ok: !protected && !root_norm.contains("\\appdata"),
        in_finding: false,
        no_logs: false,
        logs_dir: false,
        in_project: false,
        no_project: false,
    };

    // Disk walking is I/O bound: oversubscribe the cores so the SSD queue stays full.
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads((cores * 2).clamp(4, 64))
        .stack_size(8 << 20)
        .build()
        .expect("thread pool");
    let mut tree = pool.install(|| sc.walk(&root, ctx));
    tree.name = root.display().to_string();

    let mut findings = sc.findings.into_inner().unwrap_or_default();
    if root_norm.as_bytes().get(1) == Some(&b':') {
        let drive = format!("{}\\", &root_norm[..2].to_uppercase());
        let (bytes, items) = win::recycle_bin_info(&drive);
        if bytes > 0 {
            findings.push(Finding {
                category: Category::RecycleBin,
                path: PathBuf::from(&drive),
                size: bytes,
                files: items,
                safety: Safety::Safe,
                note: format!("{items} deleted items on {drive}"),
                mode: DeleteMode::Whole,
                kind: ItemKind::RecycleBin,
            });
        }
    }
    findings.sort_by(|a, b| a.category.cmp(&b.category).then(b.size.cmp(&a.size)));
    // After the sort: projects refer to findings by index.
    let projects = pool.install(|| projects::analyze(sc.seeds.into_inner().unwrap_or_default(), &mut findings, sc.now));

    ScanResult {
        root,
        tree,
        findings,
        projects,
        elapsed: started.elapsed(),
        files: progress.files.load(Relaxed),
        dirs: progress.dirs.load(Relaxed),
        errors: progress.errors.load(Relaxed),
        cancelled: progress.cancel.load(Relaxed),
    }
}

impl Scanner<'_> {
    fn walk(&self, dir: &Path, ctx: Ctx) -> DirNode {
        let mut node = DirNode {
            name: dir.file_name().map_or_else(|| dir.display().to_string(), |n| n.to_string_lossy().into_owned()),
            path: dir.to_path_buf(),
            size: 0,
            files: 0,
            tag: None,
            children: Vec::new(),
            newest: UNIX_EPOCH,
            markers: 0,
        };
        if self.progress.cancel.load(Relaxed) {
            return node;
        }
        let rd = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => {
                self.progress.errors.fetch_add(1, Relaxed);
                return node;
            }
        };

        let mut subdirs = Vec::new();
        let mut markers = 0u32;
        let mut local = Vec::new();
        let mut has_git = false;
        for entry in rd {
            let Ok(e) = entry else { continue };
            let Ok(ft) = e.file_type() else { continue };
            // Never follow symlinks / junctions: avoids loops and double counting.
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                // Without HEAD it is an empty or broken folder, not a repository.
                has_git |= e.file_name().eq_ignore_ascii_case(".git") && e.path().join("HEAD").is_file();
                subdirs.push(e.path());
                continue;
            }
            // On Windows this comes from the directory listing itself - no extra syscall.
            let Ok(md) = e.metadata() else { continue };
            let len = md.len();
            node.size += len;
            node.files += 1;
            if !ctx.in_finding {
                let name = e.file_name();
                markers |= rules::marker_of(&name);
                node.newest = node.newest.max(md.modified().unwrap_or(UNIX_EPOCH));
                if let Some(f) = self.check_file(&e, &name, len, &md, &ctx) {
                    local.push(f);
                }
            }
        }
        if !local.is_empty() {
            self.findings.lock().unwrap().extend(local);
        }

        self.progress.files.fetch_add(node.files, Relaxed);
        self.progress.bytes.fetch_add(node.size, Relaxed);
        if self.progress.dirs.fetch_add(1, Relaxed) % 97 == 0 {
            if let Ok(mut cur) = self.progress.current.try_lock() {
                *cur = dir.display().to_string();
            }
        }

        let is_project = !ctx.in_finding
            && ctx.dev_ok
            && !ctx.no_project
            && (has_git || (markers != 0 && !ctx.in_project))
            && safety::deletion_blocked(dir).is_none();
        let inner = Ctx { in_project: ctx.in_project || is_project, ..ctx };
        let children: Vec<DirNode> = subdirs.into_par_iter().map(|p| self.visit(p, &inner, markers)).collect();
        node.markers = markers;
        for c in &children {
            node.size += c.size;
            node.files += c.files;
            if c.tag.is_none() && !c.name.eq_ignore_ascii_case(".git") {
                node.newest = node.newest.max(c.newest);
                node.markers |= c.markers;
            }
        }
        if is_project {
            self.seeds.lock().unwrap().push(Seed {
                root: dir.to_path_buf(),
                size: node.size,
                files: node.files,
                markers: node.markers,
                newest: node.newest,
                has_git,
            });
            // A nested project's activity and type are its own, not its parent's.
            node.newest = UNIX_EPOCH;
            node.markers = 0;
        }
        if !ctx.in_finding && ctx.depth < TREE_MAX_DEPTH {
            let mut children = children;
            children.retain(|c| c.size >= TREE_MIN);
            children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
            children.truncate(TREE_MAX_CHILDREN);
            node.children = children;
        }
        node
    }

    fn visit(&self, path: PathBuf, ctx: &Ctx, markers: u32) -> DirNode {
        let mut cctx = Ctx { depth: ctx.depth + 1, ..*ctx };
        let mut finding: Option<(Category, Safety, String, DeleteMode)> = None;

        if !ctx.in_finding {
            let key = safety::norm(&path);
            let name = key.rsplit('\\').next().unwrap_or_default();
            if let Some(k) = self.known.get(&key) {
                finding = Some((k.cat, k.safety, k.note.to_string(), k.mode));
            }
            if !cctx.protected && safety::is_protected_area(&key) {
                cctx.protected = true;
            }
            if finding.is_none() && ctx.dev_ok && !cctx.protected {
                if let Some((c, s, n)) = rules::match_dev(name, markers, &path) {
                    finding = Some((c, s, n.to_string(), DeleteMode::Whole));
                }
            }
            // App installs and tool folders (AppData, .vscode, .cargo, .git...) contain
            // node_modules / target folders that the apps need: never match inside them.
            if cctx.protected || name == "appdata" || name.starts_with('.') {
                cctx.dev_ok = false;
            }
            // Inside a project, worktrees are that project's own checkouts, unlike the rest of a dot-folder.
            if ctx.in_project && !cctx.protected && rules::is_worktrees_dir(&key) {
                cctx.dev_ok = true;
            }
            if rules::is_package_store(name, &key) {
                cctx.no_project = true;
            }
            if rules::is_db_dir(name) {
                cctx.no_logs = true;
            }
            if name == "logs" || name == "log" {
                cctx.logs_dir = true;
            }
            cctx.in_finding = finding.is_some();
        }

        let mut node = self.walk(&path, cctx);
        if let Some((category, safety, note, mode)) = finding {
            node.tag = Some(category);
            if node.size > 0 || node.files > 0 {
                self.findings.lock().unwrap().push(Finding {
                    category,
                    path,
                    size: node.size,
                    files: node.files,
                    safety,
                    note,
                    mode,
                    kind: ItemKind::Dir,
                });
            }
        }
        node
    }

    fn check_file(&self, e: &DirEntry, name: &OsStr, len: u64, md: &Metadata, ctx: &Ctx) -> Option<Finding> {
        let as_path = Path::new(name);
        let ext_is = |x: &str| as_path.extension().is_some_and(|e| e.eq_ignore_ascii_case(x));
        let file = |category, safety, note: String| Finding {
            category,
            path: e.path(),
            size: len,
            files: 1,
            safety,
            note,
            mode: DeleteMode::Whole,
            kind: ItemKind::File,
        };

        if len >= LARGE_FILE {
            let (safety, note) = rules::large_file_note(&e.path());
            return Some(file(Category::LargeFiles, safety, note));
        }
        if ctx.protected {
            return None;
        }
        if ext_is("dmp") {
            return Some(file(Category::CrashDumps, Safety::Safe, "Crash dump".into()));
        }
        if ext_is("log") && !ctx.no_logs {
            let stem = as_path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if rules::is_journal_log(&stem) {
                return None;
            }
            let old = md
                .modified()
                .ok()
                .and_then(|m| self.now.duration_since(m).ok())
                .is_some_and(|age| age > OLD_LOG);
            return Some(if ctx.logs_dir || old {
                file(Category::LogFiles, Safety::Safe, "Old log or inside a logs folder".into())
            } else {
                file(Category::LogFiles, Safety::Caution, "Written in the last 7 days - the app may still use it".into())
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner;

    fn write(p: &Path, bytes: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn finds_and_cleans_dev_junk() {
        // Not %TEMP%: that is under AppData, where dev-artifact matching is intentionally off.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test-fixture");
        let _ = fs::remove_dir_all(&root);
        write(&root.join("web/package.json"), 10);
        write(&root.join("web/node_modules/a/index.js"), 1000);
        write(&root.join("web/src/main.js"), 50);
        write(&root.join("rs/Cargo.toml"), 10);
        write(&root.join("rs/target/debug/app.exe"), 2000);
        write(&root.join("rs/src/target/keep.txt"), 5); // no Cargo.toml next to it
        write(&root.join("logs/app.log"), 300);
        write(&root.join("db/leveldb/000003.log"), 300); // journal, must be ignored
        write(&root.join("repo/.git/logs/x.log"), 300); // inside .git, must be ignored

        let prog = Arc::new(Progress::default());
        let r = start(root.clone(), prog).recv().unwrap();
        let found: Vec<(Category, String)> = r
            .findings
            .iter()
            .map(|f| (f.category, f.path.strip_prefix(&root).unwrap().display().to_string()))
            .collect();
        assert!(found.contains(&(Category::NodeModules, r"web\node_modules".into())), "{found:?}");
        assert!(found.contains(&(Category::BuildArtifacts, r"rs\target".into())), "{found:?}");
        assert!(found.contains(&(Category::LogFiles, r"logs\app.log".into())), "{found:?}");
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(r.tree.size, 10 + 1000 + 50 + 10 + 2000 + 5 + 300 * 3);

        let items: Vec<Finding> = r.findings.clone();
        let cp = Arc::new(cleaner::CleanProgress::default());
        let rep = cleaner::start(items, false, Default::default(), Default::default(), cp).recv().unwrap();
        assert_eq!(rep.freed, 1000 + 2000 + 300);
        assert!(!root.join("web/node_modules").exists());
        assert!(!root.join("rs/target").exists());
        assert!(root.join("web/src/main.js").exists());
        assert!(root.join("rs/src/target/keep.txt").exists());
        assert!(root.join("db/leveldb/000003.log").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn finds_projects_and_their_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("projects-fixture");
        let _ = fs::remove_dir_all(&root);
        // A monorepo: one project however many packages it has.
        write(&root.join("mono/.git/HEAD"), 5);
        write(&root.join("mono/package.json"), 10);
        write(&root.join("mono/package-lock.json"), 10);
        write(&root.join("mono/packages/ui/package.json"), 10);
        write(&root.join("mono/packages/ui/node_modules/a/index.js"), 1000);
        write(&root.join("mono/services/api/Cargo.toml"), 10);
        write(&root.join("mono/services/api/target/app.exe"), 2000);
        // A repository cloned inside another project is its own project.
        write(&root.join("mono/third_party/lib/.git/HEAD"), 5);
        write(&root.join("mono/third_party/lib/go.mod"), 10);
        // A broken `.git` folder does not make a project, and does not swallow the ones inside it.
        write(&root.join("broken/.git/config"), 5);
        write(&root.join("broken/inner/package.json"), 10);
        // An agent's worktree belongs to the project it sits in; other dot-folders stay off-limits.
        write(&root.join("mono/.claude/worktrees/agent-1/.git"), 5);
        write(&root.join("mono/.claude/worktrees/agent-1/package.json"), 10);
        write(&root.join("mono/.claude/worktrees/agent-1/node_modules/a.js"), 500);
        write(&root.join("mono/.claude/skills/tool/package.json"), 10);
        write(&root.join("mono/.claude/skills/tool/node_modules/b.js"), 500);
        // No git, just a marker file.
        write(&root.join("script/requirements.txt"), 10);
        // Not projects: packages in a store, tool folders, folders without a marker.
        write(&root.join("tools/node_modules/npm/package.json"), 10);
        write(&root.join(".nvm/versions/package.json"), 10);
        write(&root.join("photos/a.jpg"), 10);

        let r = start(root.clone(), Arc::new(Progress::default())).recv().unwrap();
        let name = |n: &str| r.projects.iter().find(|p| p.name == n).unwrap_or_else(|| panic!("{n}: {:?}", r.projects));
        assert_eq!(r.projects.len(), 4, "{:?}", r.projects);
        assert!(name("inner").branch.is_none());

        let mono = name("mono");
        assert_eq!(mono.kinds, ["Node.js", "Rust"], "types come from the whole project, not just the top folder");
        assert_eq!(mono.artifacts.len(), 3, "{:?}", mono.artifacts);
        assert!(mono.size >= 4000 && mono.size < 4100, "{} excludes the nested repository", mono.size);
        assert!(r.findings.iter().any(|f| f.path.ends_with(r"agent-1\node_modules")));
        assert!(!r.findings.iter().any(|f| f.path.ends_with(r"tool\node_modules")), "a skill needs its dependencies");
        assert!(mono.branch.is_some());
        assert_eq!(name("lib").kinds, ["Go"]);
        assert!(name("script").branch.is_none());
        assert!(name("script").last_active.is_some());
        fs::remove_dir_all(&root).unwrap();
    }

    /// cargo test --release -- --ignored --nocapture scan_speed
    #[test]
    #[ignore]
    fn scan_speed() {
        let root = PathBuf::from(std::env::var("SCAN_ROOT").unwrap_or_else(|_| "C:\\".into()));
        let r = start(root, Arc::new(Progress::default())).recv().unwrap();
        let mut per: std::collections::BTreeMap<Category, (usize, u64)> = Default::default();
        for f in &r.findings {
            let e = per.entry(f.category).or_default();
            e.0 += 1;
            e.1 += f.size;
        }
        println!(
            "{} files, {} dirs, {} in {:.2}s, {} unreadable",
            r.files,
            r.dirs,
            crate::util::fmt_size(r.tree.size),
            r.elapsed.as_secs_f32(),
            r.errors
        );
        for (c, (n, b)) in per {
            println!("{:32} {:>6} items {:>10}", c.label(), n, crate::util::fmt_size(b));
            for f in r.findings.iter().filter(|f| f.category == c).take(4) {
                println!("      {:>10} {:?} {}", crate::util::fmt_size(f.size), f.safety, f.path.display());
            }
        }
        println!("{} projects", r.projects.len());
        for p in &r.projects {
            let cleanable: u64 = p.artifacts.iter().map(|a| r.findings[a.finding].size).sum();
            println!(
                "{:>10} {:>10} dormant={:<5} {:?} {:?} {}",
                crate::util::fmt_size(p.size),
                crate::util::fmt_size(cleanable),
                p.dormant,
                p.kinds,
                p.branch,
                p.root.display()
            );
        }
    }
}
