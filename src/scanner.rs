//! Parallel disk walk. One pass builds the space-usage tree and collects everything cleanable.

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
use std::time::{Duration, Instant, SystemTime};

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
    pub selected: bool,
}

pub struct DirNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub files: u64,
    /// Set when the whole folder is a cleanable finding.
    pub tag: Option<Category>,
    pub children: Vec<DirNode>,
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
}

struct Scanner<'a> {
    known: HashMap<String, &'static Known>,
    progress: &'a Progress,
    findings: Mutex<Vec<Finding>>,
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

    let sc = Scanner { known, progress, findings: Mutex::new(Vec::new()), now: SystemTime::now() };
    let protected = safety::is_protected_area(&root_norm);
    let ctx = Ctx {
        depth: 0,
        protected,
        dev_ok: !protected && !root_norm.contains("\\appdata"),
        in_finding: false,
        no_logs: false,
        logs_dir: false,
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
                selected: false,
            });
        }
    }
    findings.sort_by(|a, b| a.category.cmp(&b.category).then(b.size.cmp(&a.size)));

    ScanResult {
        root,
        tree,
        findings,
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
        for entry in rd {
            let Ok(e) = entry else { continue };
            let Ok(ft) = e.file_type() else { continue };
            // Never follow symlinks / junctions: avoids loops and double counting.
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
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

        let children: Vec<DirNode> = subdirs.into_par_iter().map(|p| self.visit(p, &ctx, markers)).collect();
        for c in &children {
            node.size += c.size;
            node.files += c.files;
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
                    selected: false,
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
            selected: false,
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
    }
}
