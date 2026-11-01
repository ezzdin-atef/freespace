//! Session state behind the Tauri commands, and the delete-plan logic.
//!
//! Trust boundary: the web UI never names paths to delete. It sends finding ids (stable indices into the
//! current scan) tagged with the scan id they came from, plus folders it saw in the usage tree, which are
//! looked up in that tree again. Plans are rebuilt here from Rust-side data and re-checked against the
//! safety rules every time.

use crate::apps::{InstalledApp, Leftover, UninstallProgress};
use crate::cleaner::{CleanProgress, CleanReport, Maintenance, Policy};
use crate::rules::{Category, DeleteMode, ItemKind, Safety};
use crate::scanner::{DirNode, Finding, Progress, ScanResult};
use crate::settings::Settings;
use crate::util::fmt_size;
use crate::{apps, safety};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const STALE_SCAN: &str = "The scan results changed since this list was shown. Check your selection and try again.";
pub const STALE_LEFTOVERS: &str = "The leftover list changed since it was shown. Check your selection and try again.";
pub const BUSY: &str = "Another scan, clean or uninstall is still running.";

/// A finished scan. Cleaned findings are marked, never removed, so ids stay valid for the scan's lifetime.
pub struct ScanState {
    pub id: u64,
    pub result: ScanResult,
    pub cleaned: Vec<bool>,
}

impl ScanState {
    pub fn new(id: u64, result: ScanResult) -> Self {
        let cleaned = vec![false; result.findings.len()];
        Self { id, result, cleaned }
    }

    /// (id, finding) for everything not cleaned yet.
    pub fn live(&self) -> impl Iterator<Item = (usize, &Finding)> {
        self.result.findings.iter().enumerate().filter(|(i, _)| !self.cleaned[*i])
    }

    pub fn mark_cleaned(&mut self, processed: &[PathBuf]) {
        let done: HashSet<&PathBuf> = processed.iter().collect();
        for (i, f) in self.result.findings.iter().enumerate() {
            if done.contains(&f.path) {
                self.cleaned[i] = true;
            }
        }
    }
}

pub struct ScanJob {
    pub id: u64,
    pub root: String,
    pub progress: Arc<Progress>,
    pub started: Instant,
    /// Used bytes of the drive being scanned, for a progress estimate.
    pub expected: Option<u64>,
}

pub struct CleanJob {
    pub progress: Arc<CleanProgress>,
    pub started: Instant,
    /// Set when the job deletes an app's leftovers rather than scan findings.
    pub app: Option<usize>,
}

/// Leftovers found for one app; `gen` changes on every rescan so stale ids are refused.
pub struct LeftoverSet {
    pub gen: u64,
    pub items: Vec<Leftover>,
}

#[derive(Default)]
pub struct Session {
    next_id: u64,
    pub scan: Option<ScanState>,
    pub scan_job: Option<ScanJob>,
    pub clean_job: Option<CleanJob>,
    pub report: Option<CleanReport>,

    /// Loaded once at startup, so an app id (index) stays valid for the life of the process.
    pub apps: Vec<InstalledApp>,
    pub apps_loading: bool,
    /// App id -> logo as a PNG data URL, filled in the background after the list loads.
    pub app_logos: HashMap<usize, String>,
    pub leftovers: HashMap<usize, LeftoverSet>,
    pub leftovers_loading: HashSet<usize>,
    pub uninstall_job: Option<(usize, Arc<UninstallProgress>)>,
    /// Apps uninstalled (or whose entry was removed) during this session.
    pub removed: HashSet<usize>,

    pub settings: Settings,
    /// None in tests, which keep settings in memory.
    pub settings_path: Option<PathBuf>,
    pub has_tray: bool,
    pub scan_finished: Option<Instant>,
}

impl Session {
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn busy(&self) -> bool {
        self.scan_job.is_some() || self.clean_job.is_some() || self.uninstall_job.is_some()
    }

    /// The current scan, if `scan_id` still names it.
    pub fn scan_for(&self, scan_id: u64) -> Result<&ScanState, String> {
        self.scan.as_ref().filter(|s| s.id == scan_id).ok_or_else(|| STALE_SCAN.to_string())
    }

    /// The folder the automatic rescan should scan at `now`, if it is time. The UI still decides whether to go ahead,
    /// and is asked again a minute later if it does not.
    pub fn rescan_due(&self, now: Instant) -> Option<&str> {
        let every = Duration::from_secs(u64::from(self.settings.rescan_minutes) * 60);
        let waited = self.scan_finished.is_none_or(|t| now.saturating_duration_since(t) >= every);
        (!every.is_zero() && waited && !self.busy()).then_some(self.settings.last_root.as_deref()).flatten()
    }

    /// Saves the settings. A failure only means they are back to the old values next time.
    pub fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            let _ = self.settings.save(path);
        }
    }

    pub fn app(&self, app_id: usize) -> Result<&InstalledApp, String> {
        self.apps.get(app_id).ok_or_else(|| "That app is no longer in the list.".to_string())
    }
}

// ------------------------------------------------------------------ plans

pub struct Plan {
    pub items: Vec<Finding>,
    pub maint: Maintenance,
    pub policy: Policy,
    pub bytes: u64,
    /// One line per item that is not marked Safe, shown in the confirm dialog.
    pub caution: Vec<String>,
    /// Items dropped from the plan, with the reason.
    pub blocked: Vec<(PathBuf, &'static str)>,
    pub warn: Option<String>,
    pub app: Option<usize>,
    /// Apps whose leftover lists are rescanned once the clean is done.
    pub refresh: Vec<usize>,
}

/// What "Full clean" selects: everything safe plus the system leftovers that only matter for rollback/debugging.
pub fn full_clean_pick(f: &Finding) -> bool {
    match f.safety {
        Safety::Safe => f.category != Category::LargeFiles,
        Safety::Caution => matches!(f.category, Category::WindowsLogs | Category::OldWindows | Category::WindowsUpdate),
        Safety::Protected => false,
    }
}

/// The node at exactly `path` in the scan tree, descending only through ancestors of it.
pub fn find_node<'a>(node: &'a DirNode, path: &Path) -> Option<&'a DirNode> {
    if node.path == path {
        return Some(node);
    }
    node.children.iter().filter(|c| path.starts_with(&c.path)).find_map(|c| find_node(c, path))
}

/// Delete plan for findings `ids` plus usage-tree `folders` of `scan`.
pub fn clean_plan(scan: &ScanState, ids: &[usize], folders: &[PathBuf], maint: Maintenance) -> Result<Plan, String> {
    let mut items = Vec::new();
    let mut blocked = Vec::new();
    let mut seen = HashSet::new();
    for &id in ids {
        if !seen.insert(id) {
            continue;
        }
        let f = scan.result.findings.get(id).filter(|_| !scan.cleaned[id]).ok_or_else(|| STALE_SCAN.to_string())?;
        if f.safety == Safety::Protected {
            blocked.push((f.path.clone(), "Marked Do not delete"));
            continue;
        }
        items.push(f.clone());
    }
    for p in folders {
        let node = find_node(&scan.result.tree, p).ok_or_else(|| STALE_SCAN.to_string())?;
        let (s, note) = safety::assess(&node.path);
        items.push(Finding {
            category: Category::Custom,
            path: node.path.clone(),
            size: node.size,
            files: node.files,
            safety: s,
            note: note.to_string(),
            mode: DeleteMode::Whole,
            kind: ItemKind::Dir,
        });
    }

    items.retain(|f| {
        if f.kind == ItemKind::RecycleBin || f.mode == DeleteMode::Contents {
            return true;
        }
        match safety::deletion_blocked(&f.path) {
            Some(r) => {
                blocked.push((f.path.clone(), r));
                false
            }
            None => true,
        }
    });

    // Drop items nested inside another selected item so nothing is counted twice.
    let mut keyed: Vec<(String, Finding)> = items.into_iter().map(|f| (safety::norm(&f.path), f)).collect();
    keyed.sort_by_key(|(k, _)| k.len());
    let mut kept: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (key, f) in keyed {
        if f.kind != ItemKind::RecycleBin {
            let mut anc = key.as_str();
            let mut nested = kept.contains(anc);
            while !nested {
                let Some(i) = anc.rfind('\\') else { break };
                anc = &anc[..i];
                nested = kept.contains(anc) || kept.contains(&format!("{anc}\\"));
            }
            if nested {
                continue;
            }
            kept.insert(key);
        }
        out.push(f);
    }
    out.sort_by(|a, b| a.category.cmp(&b.category).then(b.size.cmp(&a.size)));
    Ok(finish_plan(out, maint, Policy::Junk, blocked, None, None))
}

/// Delete plan for leftovers `ids` (from generation `gen`) of app `app_id`.
pub fn leftover_plan(session: &Session, app_id: usize, gen: u64, ids: &[usize]) -> Result<Plan, String> {
    let app = session.app(app_id)?;
    let set = session.leftovers.get(&app_id).filter(|s| s.gen == gen).ok_or_else(|| STALE_LEFTOVERS.to_string())?;
    let mut blocked = Vec::new();
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for &id in ids {
        if !seen.insert(id) {
            continue;
        }
        let l = set.items.get(id).ok_or_else(|| STALE_LEFTOVERS.to_string())?;
        if l.safety == Safety::Protected {
            blocked.push((l.path.clone(), "Marked Do not delete"));
            continue;
        }
        if let Some(r) = safety::uninstall_blocked(&l.path) {
            blocked.push((l.path.clone(), r));
            continue;
        }
        items.push(Finding {
            category: Category::AppLeftovers,
            path: l.path.clone(),
            size: l.size,
            files: l.files,
            safety: l.safety,
            note: format!("{} - {}", l.what, l.note),
            mode: DeleteMode::Whole,
            kind: if l.path.is_file() { ItemKind::File } else { ItemKind::Dir },
        });
    }
    let warn = (!session.removed.contains(&app_id) && app.has_uninstaller())
        .then(|| format!("{} is still installed. Deleting its files will break it - uninstall it first.", app.name));
    let mut plan = finish_plan(items, Maintenance::default(), Policy::Uninstall, blocked, warn, Some(app_id));
    plan.refresh = vec![app_id];
    Ok(plan)
}

/// Delete plan that resets apps to a fresh state: each `(app id, leftover generation)` loses its data folders.
pub fn reset_plan(session: &Session, picks: &[(usize, u64)]) -> Result<Plan, String> {
    // Freespace never resets itself while it runs.
    let me = std::env::current_exe().ok().map(|p| safety::norm(&p));
    let mut items = Vec::new();
    let mut blocked = Vec::new();
    let mut names = Vec::new();
    let mut refresh = Vec::new();
    for &(app_id, gen) in picks {
        if refresh.contains(&app_id) || session.removed.contains(&app_id) {
            continue;
        }
        let app = session.app(app_id)?;
        if app.location.as_deref().zip(me.as_deref()).is_some_and(|(l, me)| safety::is_within(me, &safety::norm(l))) {
            continue;
        }
        let set = session.leftovers.get(&app_id).filter(|s| s.gen == gen).ok_or_else(|| STALE_LEFTOVERS.to_string())?;
        for t in apps::reset_targets(app, &set.items) {
            if let Some(r) = safety::uninstall_blocked(&t.path) {
                blocked.push((t.path, r));
                continue;
            }
            items.push(Finding {
                category: Category::AppData,
                path: t.path,
                size: t.size,
                files: t.files,
                safety: Safety::Safe,
                note: format!("{} - {}", app.name, t.what),
                mode: if t.contents { DeleteMode::Contents } else { DeleteMode::Whole },
                kind: ItemKind::Dir,
            });
        }
        names.push(app.name.as_str());
        refresh.push(app_id);
    }
    let who = match names.as_slice() {
        [one] => one.to_string(),
        _ => format!("these {} apps", names.len()),
    };
    let warn = (!items.is_empty()).then(|| {
        format!("Close {who} first. Their settings, sign-ins and local data are deleted, and they start like a fresh install.")
    });
    let app = (refresh.len() == 1).then(|| refresh[0]);
    let mut plan = finish_plan(items, Maintenance::default(), Policy::Uninstall, blocked, warn, app);
    plan.refresh = refresh;
    Ok(plan)
}

fn finish_plan(
    items: Vec<Finding>,
    maint: Maintenance,
    policy: Policy,
    blocked: Vec<(PathBuf, &'static str)>,
    warn: Option<String>,
    app: Option<usize>,
) -> Plan {
    let bytes = items.iter().map(|f| f.size).sum();
    let caution = items
        .iter()
        .filter(|f| f.safety != Safety::Safe)
        .map(|f| format!("{}  ({})  {}", f.path.display(), fmt_size(f.size), f.note))
        .collect();
    Plan { items, maint, policy, bytes, caution, blocked, warn, app, refresh: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::Source;
    use std::time::Duration;

    fn finding(path: &str, cat: Category, safety: Safety, size: u64) -> Finding {
        Finding {
            category: cat,
            path: PathBuf::from(path),
            size,
            files: 1,
            safety,
            note: String::new(),
            mode: DeleteMode::Whole,
            kind: ItemKind::Dir,
        }
    }

    fn node(path: &str, size: u64, children: Vec<DirNode>) -> DirNode {
        let p = PathBuf::from(path);
        let name = p.file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        DirNode { name, path: p, size, files: 1, tag: None, children, newest: std::time::UNIX_EPOCH, markers: 0 }
    }

    fn scan(findings: Vec<Finding>) -> ScanState {
        let tree = node(
            "D:\\",
            1000,
            vec![
                node("D:\\proj", 600, vec![node("D:\\proj\\web", 400, vec![node("D:\\proj\\web\\node_modules", 300, vec![])])]),
                node("D:\\Windows", 100, vec![]),
            ],
        );
        ScanState::new(
            7,
            ScanResult {
                root: PathBuf::from("D:\\"),
                tree,
                findings,
                projects: Vec::new(),
                elapsed: Duration::ZERO,
                files: 0,
                dirs: 0,
                errors: 0,
                cancelled: false,
            },
        )
    }

    fn paths(plan: &Plan) -> Vec<String> {
        plan.items.iter().map(|f| f.path.display().to_string()).collect()
    }

    #[test]
    fn rescan_waits_for_its_interval() {
        let mut s = Session::default();
        let now = Instant::now();
        s.settings.rescan_minutes = 60;
        assert_eq!(s.rescan_due(now), None, "nothing has been scanned, so there is nothing to repeat");

        s.settings.last_root = Some("D:\\code".into());
        assert_eq!(s.rescan_due(now), Some("D:\\code"), "the first one after a restart runs right away");

        s.scan_finished = Some(now);
        assert_eq!(s.rescan_due(now + Duration::from_secs(59 * 60)), None);
        assert_eq!(s.rescan_due(now + Duration::from_secs(60 * 60)), Some("D:\\code"));

        s.settings.rescan_minutes = 0;
        assert_eq!(s.rescan_due(now + Duration::from_secs(99 * 60)), None, "turned off");
    }

    #[test]
    fn rescan_never_interrupts_a_job() {
        let mut s = Session::default();
        s.settings.last_root = Some("D:\\code".into());
        s.clean_job = Some(CleanJob { progress: Default::default(), started: Instant::now(), app: None });
        assert_eq!(s.rescan_due(Instant::now()), None);
    }

    #[test]
    fn stale_scan_id_is_refused() {
        let mut s = Session::default();
        s.scan = Some(scan(vec![]));
        assert!(s.scan_for(7).is_ok());
        assert_eq!(s.scan_for(6).err().as_deref(), Some(STALE_SCAN));
        s.scan = None;
        assert!(s.scan_for(7).is_err());
    }

    #[test]
    fn unknown_or_cleaned_ids_are_refused() {
        let mut st = scan(vec![finding("D:\\proj\\web\\node_modules", Category::NodeModules, Safety::Safe, 300)]);
        assert!(clean_plan(&st, &[0], &[], Maintenance::default()).is_ok());
        assert!(clean_plan(&st, &[1], &[], Maintenance::default()).is_err());
        st.mark_cleaned(&[PathBuf::from("D:\\proj\\web\\node_modules")]);
        assert_eq!(st.live().count(), 0);
        assert!(clean_plan(&st, &[0], &[], Maintenance::default()).is_err());
    }

    #[test]
    fn protected_findings_never_enter_a_plan() {
        let st = scan(vec![finding("D:\\proj\\keep", Category::LargeFiles, Safety::Protected, 50)]);
        let plan = clean_plan(&st, &[0], &[], Maintenance::default()).unwrap();
        assert!(plan.items.is_empty());
        assert_eq!(plan.blocked.len(), 1);
    }

    #[test]
    fn nested_items_are_counted_once() {
        let st = scan(vec![
            finding("D:\\proj\\web\\node_modules", Category::NodeModules, Safety::Safe, 300),
            finding("D:\\proj\\web\\node_modules\\.cache", Category::BuildArtifacts, Safety::Safe, 20),
        ]);
        let plan = clean_plan(&st, &[1, 0, 0], &[], Maintenance::default()).unwrap();
        assert_eq!(paths(&plan), ["D:\\proj\\web\\node_modules"]);
        assert_eq!(plan.bytes, 300);
    }

    #[test]
    fn folders_must_come_from_the_scan_tree() {
        let st = scan(vec![finding("D:\\proj\\web\\node_modules", Category::NodeModules, Safety::Safe, 300)]);
        let plan = clean_plan(&st, &[0], &[PathBuf::from("D:\\proj\\web")], Maintenance::default()).unwrap();
        // The picked folder contains the finding, so only the folder is kept, with the tree's size.
        assert_eq!(paths(&plan), ["D:\\proj\\web"]);
        assert_eq!(plan.bytes, 400);
        assert_eq!(plan.items[0].category, Category::Custom);
        assert!(clean_plan(&st, &[], &[PathBuf::from("D:\\elsewhere")], Maintenance::default()).is_err());
    }

    #[test]
    fn system_folders_are_blocked() {
        let st = scan(vec![]);
        let windir = PathBuf::from(&safety::sys().windir);
        let mut st = st;
        st.result.tree.children.push(node(&windir.display().to_string(), 10, vec![]));
        let plan = clean_plan(&st, &[], &[windir], Maintenance::default()).unwrap();
        assert!(plan.items.is_empty());
        assert_eq!(plan.blocked.len(), 1);
    }

    #[test]
    fn full_clean_skips_large_and_protected() {
        assert!(full_clean_pick(&finding("D:\\a", Category::UserTemp, Safety::Safe, 1)));
        assert!(full_clean_pick(&finding("D:\\a", Category::OldWindows, Safety::Caution, 1)));
        assert!(!full_clean_pick(&finding("D:\\a", Category::LargeFiles, Safety::Safe, 1)));
        assert!(!full_clean_pick(&finding("D:\\a", Category::LogFiles, Safety::Caution, 1)));
        assert!(!full_clean_pick(&finding("D:\\a", Category::UserTemp, Safety::Protected, 1)));
    }

    fn app(uninstaller: bool) -> InstalledApp {
        InstalledApp {
            name: "Demo".into(),
            publisher: String::new(),
            version: String::new(),
            installed: String::new(),
            location: None,
            size: 0,
            icon: None,
            source: Source::Registry {
                hive: crate::apps::Hive::User,
                key: "Demo".into(),
                uninstall: uninstaller.then(|| "demo.exe /uninstall".into()),
                msi: None,
            },
        }
    }

    fn leftover(path: &str, safety: Safety) -> Leftover {
        Leftover { path: PathBuf::from(path), what: "Folder", size: 10, files: 1, safety, note: "", selected: true }
    }

    #[test]
    fn leftover_plans_check_generation_and_safety() {
        let mut s = Session::default();
        s.apps = vec![app(true)];
        s.leftovers.insert(
            0,
            LeftoverSet {
                gen: 3,
                items: vec![
                    leftover("D:\\DemoData", Safety::Safe),
                    leftover("D:\\DemoKeep", Safety::Protected),
                    leftover(&safety::sys().windir, Safety::Caution),
                ],
            },
        );
        assert_eq!(leftover_plan(&s, 0, 2, &[0]).err().as_deref(), Some(STALE_LEFTOVERS));
        assert!(leftover_plan(&s, 0, 3, &[9]).is_err());
        assert!(leftover_plan(&s, 1, 3, &[0]).is_err());

        let plan = leftover_plan(&s, 0, 3, &[0, 1, 2]).unwrap();
        assert_eq!(paths(&plan), ["D:\\DemoData"]);
        assert_eq!(plan.blocked.len(), 2);
        assert!(plan.warn.is_some(), "still installed, so the plan warns");

        s.removed.insert(0);
        assert!(leftover_plan(&s, 0, 3, &[0]).unwrap().warn.is_none());
    }

    #[test]
    fn reset_plans_take_only_app_data() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("reset-plan-fixture");
        let _ = std::fs::remove_dir_all(&dir);
        let data = dir.join("Roaming").join("Demo");
        std::fs::create_dir_all(&data).unwrap();
        let mut s = Session::default();
        s.apps = vec![app(true), app(true)];
        let lo = |what: &'static str| Leftover { what, ..leftover(&data.display().to_string(), Safety::Safe) };
        s.leftovers.insert(0, LeftoverSet { gen: 4, items: vec![lo("App data (Roaming)"), lo("Start menu")] });
        s.leftovers.insert(1, LeftoverSet { gen: 5, items: vec![] });

        assert_eq!(reset_plan(&s, &[(0, 3)]).err().as_deref(), Some(STALE_LEFTOVERS));
        let plan = reset_plan(&s, &[(0, 4), (0, 4), (1, 5)]).unwrap();
        assert_eq!(paths(&plan), [data.display().to_string()]);
        assert_eq!(plan.items[0].category, Category::AppData);
        assert_eq!(plan.refresh, [0, 1]);
        assert_eq!(plan.app, None, "several apps: no single app to report on");
        assert!(plan.warn.as_deref().is_some_and(|w| w.contains("these 2 apps")));

        s.removed.insert(0);
        let plan = reset_plan(&s, &[(0, 4)]).unwrap();
        assert!(plan.items.is_empty() && plan.warn.is_none(), "uninstalled apps are not reset");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
