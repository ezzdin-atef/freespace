//! Groups a scan's dev findings by the project they belong to, and checks each one before it is offered:
//! can it really be restored (lockfile, requirements) and is it committed to git?
//!
//! Git is read from the files in `.git`, never by running `git`: a folder found while scanning a drive is not
//! trusted, and its `.git/config` can make `git status` run any program (core.fsmonitor, clean filters).

use crate::rules::{self, Category, Safety};
use crate::safety;
use crate::scanner::Finding;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// No edit, commit or checkout for this long and a project counts as dormant.
pub const DORMANT: Duration = Duration::from_secs(90 * 24 * 3600);

const NPM_LOCKS: &[(&str, &str)] = &[
    ("pnpm-lock.yaml", "pnpm install"),
    ("yarn.lock", "yarn install"),
    ("bun.lock", "bun install"),
    ("bun.lockb", "bun install"),
    ("package-lock.json", "npm ci"),
    ("npm-shrinkwrap.json", "npm ci"),
];

const PY_LOCKS: &[(&str, &str)] = &[
    ("uv.lock", "uv sync"),
    ("poetry.lock", "poetry install"),
    ("Pipfile.lock", "pipenv install"),
    ("requirements.txt", "python -m venv .venv, then pip install -r requirements.txt"),
    ("pyproject.toml", "python -m venv .venv, then pip install ."),
];

/// A project root the scanner came across, before its findings are attached.
pub struct Seed {
    pub root: PathBuf,
    /// Includes projects nested inside this one.
    pub size: u64,
    pub files: u64,
    pub markers: u32,
    pub newest: SystemTime,
    pub has_git: bool,
}

#[derive(Debug)]
pub struct Artifact {
    /// Index into the scan's findings.
    pub finding: usize,
    pub restore: String,
}

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub kinds: Vec<&'static str>,
    /// Without the projects nested inside this one, so totals add up.
    pub size: u64,
    pub files: u64,
    /// Seconds since 1970 of the last edit, commit or checkout.
    pub last_active: Option<u64>,
    pub dormant: bool,
    /// None when the project is not a git repository.
    pub branch: Option<String>,
    pub artifacts: Vec<Artifact>,
}

fn is_artifact(c: Category) -> bool {
    matches!(c, Category::NodeModules | Category::BuildArtifacts | Category::PythonEnv | Category::LogFiles)
}

/// The nearest project at or above `path`, given projects keyed by normalized root.
fn owner(index: &HashMap<String, usize>, path: &str) -> Option<usize> {
    let mut cur = path;
    loop {
        if let Some(&i) = index.get(cur) {
            return Some(i);
        }
        cur = &cur[..cur.rfind('\\')?];
    }
}

/// The first of `names` found in `from` or a folder above it, up to `root`, with its restore command.
fn find_up(from: &Path, root: &Path, names: &[(&'static str, &'static str)]) -> Option<(&'static str, &'static str)> {
    let mut dir = from;
    loop {
        if let Some(&hit) = names.iter().find(|(n, _)| dir.join(n).is_file()) {
            return Some(hit);
        }
        if dir == root {
            return None;
        }
        dir = dir.parent()?;
    }
}

/// Turns seeds into projects, and updates `findings` where a check made a Safe item less safe. Findings keep their
/// order, so their ids stay valid.
pub fn analyze(seeds: Vec<Seed>, findings: &mut [Finding], now: SystemTime) -> Vec<Project> {
    let index: HashMap<String, usize> = seeds.iter().enumerate().map(|(i, s)| (safety::norm(&s.root), i)).collect();

    let mut own_size: Vec<u64> = seeds.iter().map(|s| s.size).collect();
    let mut own_files: Vec<u64> = seeds.iter().map(|s| s.files).collect();
    for s in &seeds {
        let key = safety::norm(&s.root);
        let parent = key.rfind('\\').and_then(|i| owner(&index, &key[..i]));
        if let Some(p) = parent {
            own_size[p] = own_size[p].saturating_sub(s.size);
            own_files[p] = own_files[p].saturating_sub(s.files);
        }
    }

    let mut owned: Vec<Vec<usize>> = vec![Vec::new(); seeds.len()];
    for (id, f) in findings.iter().enumerate() {
        if is_artifact(f.category) {
            if let Some(p) = owner(&index, &safety::norm(&f.path)) {
                owned[p].push(id);
            }
        }
    }

    // Reading git indexes is the slow part: one file per repository, and some are large.
    let tracked: Vec<HashSet<usize>> = seeds
        .par_iter()
        .zip(owned.par_iter())
        .map(|(s, ids)| if s.has_git && !ids.is_empty() { committed(&s.root, ids, findings) } else { HashSet::new() })
        .collect();

    let mut projects = Vec::with_capacity(seeds.len());
    for (i, s) in seeds.into_iter().enumerate() {
        let artifacts = owned[i]
            .iter()
            .map(|&id| {
                let is_tracked = tracked[i].contains(&id);
                Artifact { finding: id, restore: review_artifact(&mut findings[id], &s.root, is_tracked) }
            })
            .collect();
        let newest = if s.has_git { s.newest.max(git_activity(&s.root)) } else { s.newest };
        let last_active = newest.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs()).filter(|&t| t > 0);
        let dormant = last_active.is_some() && now.duration_since(newest).is_ok_and(|age| age > DORMANT);
        projects.push(Project {
            name: s.root.file_name().map_or_else(|| s.root.display().to_string(), |n| n.to_string_lossy().into_owned()),
            kinds: rules::project_kinds(s.markers),
            size: own_size[i],
            files: own_files[i],
            last_active,
            dormant,
            branch: s.has_git.then(|| git_branch(&s.root)),
            artifacts,
            root: s.root,
        });
    }
    projects
}

/// Lowers `f`'s safety when deleting it would cost more than a rebuild; returns how to get it back.
fn review_artifact(f: &mut Finding, root: &Path, is_tracked: bool) -> String {
    let parent = f.path.parent().unwrap_or(root).to_path_buf();
    let restore = match f.category {
        Category::NodeModules => match find_up(&parent, root, NPM_LOCKS) {
            Some((lock, cmd)) => {
                f.note = format!("Restore with {cmd} - exact versions are pinned in {lock}");
                cmd.to_string()
            }
            None => {
                f.safety = f.safety.max(Safety::Caution);
                f.note = "No lockfile - a reinstall may pick different package versions".into();
                "npm install".to_string()
            }
        },
        Category::PythonEnv => match find_up(&parent, root, PY_LOCKS) {
            Some((_, cmd)) => cmd.to_string(),
            None => {
                f.note = "No requirements file - you would have to work out which packages to reinstall".into();
                "Recreate it and reinstall the packages by hand".to_string()
            }
        },
        c => c.restore_hint().to_string(),
    };
    if is_tracked {
        f.safety = f.safety.max(Safety::Caution);
        f.note = "Committed to git - it will show up as deleted files until you restore or commit it".into();
        let rel = f.path.strip_prefix(root).map_or_else(|_| f.path.display().to_string(), |p| p.display().to_string());
        return format!("git restore {}", rel.replace('\\', "/"));
    }
    restore
}

// ------------------------------------------------------------------ git, read from .git

fn modified(p: &Path) -> SystemTime {
    fs::metadata(p).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH)
}

/// When the repository last moved: `logs/HEAD` on a commit, checkout or pull, `index` on a stage or status.
fn git_activity(root: &Path) -> SystemTime {
    let git = root.join(".git");
    modified(&git.join("logs").join("HEAD")).max(modified(&git.join("index")))
}

/// The checked-out branch, or the short commit id when HEAD is detached.
fn git_branch(root: &Path) -> String {
    let head = fs::read_to_string(root.join(".git").join("HEAD")).unwrap_or_default();
    let head = head.trim();
    match head.strip_prefix("ref: ") {
        Some(r) => r.strip_prefix("refs/heads/").unwrap_or(r).to_string(),
        None => head.chars().take(8).collect(),
    }
}

/// Which of the findings `ids` hold files committed to the repository at `root`.
fn committed(root: &Path, ids: &[usize], findings: &[Finding]) -> HashSet<usize> {
    let git = root.join(".git");
    let Ok(data) = fs::read(git.join("index")) else { return HashSet::new() };
    let config = fs::read_to_string(git.join("config")).unwrap_or_default().to_lowercase();
    let hash = if config.contains("sha256") { 32 } else { 20 };

    let wanted: HashMap<String, usize> = ids
        .iter()
        .filter_map(|&id| {
            let rel = findings[id].path.strip_prefix(root).ok()?;
            Some((rel.to_string_lossy().replace('\\', "/").to_lowercase(), id))
        })
        .collect();
    let mut hits = HashSet::new();
    // An index this code cannot read proves nothing, so nothing is reported as committed.
    let _ = index_paths(&data, hash, |path| {
        let path = String::from_utf8_lossy(path).to_lowercase();
        let mut end = 0;
        while let Some(i) = path[end..].find('/') {
            end += i;
            if let Some(&id) = wanted.get(&path[..end]) {
                hits.insert(id);
            }
            end += 1;
        }
        if let Some(&id) = wanted.get(path.as_str()) {
            hits.insert(id);
        }
    });
    hits
}

/// Calls `visit` with every path in a git index file (format versions 2 to 4). None when it is not a valid index.
fn index_paths(data: &[u8], hash: usize, mut visit: impl FnMut(&[u8])) -> Option<()> {
    let u32_at = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    if data.get(..4)? != b"DIRC" {
        return None;
    }
    let version = u32_at(4)?;
    if !(2..=4).contains(&version) {
        return None;
    }
    // ctime, mtime, dev, ino, mode, uid, gid and size come first, then the object id and the flags.
    let flags_at = 40 + hash;
    let mut prev: Vec<u8> = Vec::new();
    let mut at = 12;
    for _ in 0..u32_at(8)? {
        let flags = u16::from_be_bytes(data.get(at + flags_at..at + flags_at + 2)?.try_into().ok()?);
        let extended = version >= 3 && flags & 0x4000 != 0;
        let mut p = at + flags_at + if extended { 4 } else { 2 };
        if version == 4 {
            // How many bytes to drop from the previous path, then the new end of the path.
            let mut byte = *data.get(p)?;
            p += 1;
            let mut strip = (byte & 0x7f) as usize;
            while byte & 0x80 != 0 {
                byte = *data.get(p)?;
                p += 1;
                strip = ((strip + 1) << 7) + (byte & 0x7f) as usize;
            }
            let len = data.get(p..)?.iter().position(|&b| b == 0)?;
            prev.truncate(prev.len().checked_sub(strip)?);
            prev.extend_from_slice(&data[p..p + len]);
            visit(&prev);
            at = p + len + 1;
        } else {
            let len = data.get(p..)?.iter().position(|&b| b == 0)?;
            visit(&data[p..p + len]);
            // Entries are padded with NULs to a multiple of 8 bytes.
            let size = p - at + len;
            at += (size + 8) & !7;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{DeleteMode, ItemKind};
    use std::process::Command;

    fn finding(path: &Path, category: Category, safety: Safety) -> Finding {
        Finding {
            category,
            path: path.to_path_buf(),
            size: 100,
            files: 1,
            safety,
            note: String::new(),
            mode: DeleteMode::Whole,
            kind: ItemKind::Dir,
        }
    }

    fn seed(root: &Path, size: u64, newest: SystemTime) -> Seed {
        Seed { root: root.to_path_buf(), size, files: 1, markers: rules::PKG_JSON, newest, has_git: false }
    }

    fn fixture(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nested_projects_are_not_counted_twice() {
        let now = SystemTime::now();
        let seeds = vec![seed(Path::new("D:\\mono"), 1000, now), seed(Path::new("D:\\mono\\libs\\a"), 300, now)];
        let p = analyze(seeds, &mut [], now);
        assert_eq!((p[0].size, p[1].size), (700, 300));
    }

    #[test]
    fn findings_go_to_their_nearest_project() {
        let now = SystemTime::now();
        let mut findings = vec![
            finding(Path::new("D:\\mono\\target"), Category::BuildArtifacts, Safety::Safe),
            finding(Path::new("D:\\mono\\libs\\a\\target"), Category::BuildArtifacts, Safety::Safe),
            finding(Path::new("D:\\monolith\\target"), Category::BuildArtifacts, Safety::Safe),
            finding(Path::new("D:\\mono\\big.iso"), Category::LargeFiles, Safety::Caution),
        ];
        let seeds = vec![seed(Path::new("D:\\mono"), 1000, now), seed(Path::new("D:\\mono\\libs\\a"), 300, now)];
        let p = analyze(seeds, &mut findings, now);
        let ids = |i: usize| p[i].artifacts.iter().map(|a| a.finding).collect::<Vec<_>>();
        assert_eq!(ids(0), [0], "not the look-alike folder, not large files");
        assert_eq!(ids(1), [1]);
    }

    #[test]
    fn dormant_means_no_activity_for_a_while() {
        let now = SystemTime::now();
        let old = now - DORMANT - Duration::from_secs(60);
        let seeds = vec![
            seed(Path::new("D:\\old"), 1, old),
            seed(Path::new("D:\\new"), 1, now),
            seed(Path::new("D:\\empty"), 1, UNIX_EPOCH),
        ];
        let p = analyze(seeds, &mut [], now);
        assert!(p[0].dormant);
        assert!(!p[1].dormant);
        assert!(!p[2].dormant && p[2].last_active.is_none(), "no files: unknown, not dormant");
    }

    #[test]
    fn node_modules_without_a_lockfile_needs_review() {
        let root = fixture("projects-locks");
        let (a, b) = (root.join("a"), root.join("b"));
        fs::create_dir_all(a.join("packages/ui/node_modules")).unwrap();
        fs::create_dir_all(b.join("node_modules")).unwrap();
        // A monorepo keeps its lockfile at the top, above the package.
        fs::write(a.join("pnpm-lock.yaml"), "").unwrap();

        let mut findings = vec![
            finding(&a.join("packages/ui/node_modules"), Category::NodeModules, Safety::Safe),
            finding(&b.join("node_modules"), Category::NodeModules, Safety::Safe),
        ];
        let now = SystemTime::now();
        let p = analyze(vec![seed(&a, 1, now), seed(&b, 1, now)], &mut findings, now);
        assert_eq!(findings[0].safety, Safety::Safe);
        assert_eq!(p[0].artifacts[0].restore, "pnpm install");
        assert_eq!(findings[1].safety, Safety::Caution);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn index_paths_reads_a_handmade_v2_index() {
        let mut data = b"DIRC".to_vec();
        data.extend(2u32.to_be_bytes());
        data.extend(2u32.to_be_bytes());
        for path in ["a.txt", "dist/app.js"] {
            let start = data.len();
            data.extend([0u8; 60]);
            data.extend((path.len() as u16).to_be_bytes());
            data.extend(path.as_bytes());
            data.resize(start + ((62 + path.len() + 8) & !7), 0);
        }
        let mut seen = Vec::new();
        assert!(index_paths(&data, 20, |p| seen.push(String::from_utf8_lossy(p).into_owned())).is_some());
        assert_eq!(seen, ["a.txt", "dist/app.js"]);

        assert!(index_paths(b"nope", 20, |_| {}).is_none());
        data.truncate(40);
        assert!(index_paths(&data, 20, |_| {}).is_none(), "cut short: refused, no panic");
    }

    /// Against what git really writes, in every index format. Only this test runs git, on its own fixture.
    #[test]
    fn committed_artifacts_need_review() {
        let git = |dir: &Path, args: &[&str]| {
            Command::new("git").arg("-C").arg(dir).args(args).output().is_ok_and(|o| o.status.success())
        };
        for version in ["2", "3", "4"] {
            let root = fixture(&format!("projects-git-{version}"));
            if !git(&root, &["init", "-q"]) {
                eprintln!("git is not installed; skipping");
                return;
            }
            fs::create_dir_all(root.join("dist/assets")).unwrap();
            fs::create_dir_all(root.join("build")).unwrap();
            fs::create_dir_all(root.join("distant")).unwrap();
            fs::write(root.join("dist/assets/app.js"), "x").unwrap();
            fs::write(root.join("build/out.js"), "x").unwrap();
            fs::write(root.join("distant/keep.js"), "x").unwrap();
            fs::write(root.join(".gitignore"), "build/\n").unwrap();
            assert!(git(&root, &["add", "-A"]));
            assert!(git(&root, &["update-index", "--index-version", version]));

            let mut findings = vec![
                finding(&root.join("dist"), Category::BuildArtifacts, Safety::Safe),
                finding(&root.join("build"), Category::BuildArtifacts, Safety::Safe),
            ];
            let now = SystemTime::now();
            let s = Seed { has_git: true, ..seed(&root, 1, now) };
            let p = analyze(vec![s], &mut findings, now);
            assert_eq!(findings[0].safety, Safety::Caution, "index version {version}");
            assert_eq!(p[0].artifacts[0].restore, "git restore dist");
            assert_eq!(findings[1].safety, Safety::Safe, "ignored output stays safe (index version {version})");
            assert!(p[0].branch.is_some());
            fs::remove_dir_all(&root).unwrap();
        }
    }
}
