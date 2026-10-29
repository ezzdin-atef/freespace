use crate::apps::{self, AppsMsg, InstalledApp, Leftover, Outcome, Source, UninstallProgress};
use crate::cleaner::{self, CleanProgress, CleanReport, Maintenance, Policy};
use crate::rules::{Category, DeleteMode, ItemKind, Safety};
use crate::scanner::{self, DirNode, Finding, Progress, ScanResult};
use crate::ui_kit::{self as k, col, Check, Col, Look, RowState};
use crate::util::{fmt_count, fmt_size};
use crate::{safety, win};
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, Key, Layout, Margin, Rect, RichText, Sense, Stroke, Ui};
use egui_phosphor::bold as ph;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What "Full clean" selects: everything safe plus the system leftovers that only matter for rollback/debugging.
fn full_clean_pick(f: &Finding) -> bool {
    match f.safety {
        Safety::Safe => f.category != Category::LargeFiles,
        Safety::Caution => matches!(f.category, Category::WindowsLogs | Category::OldWindows | Category::WindowsUpdate),
        Safety::Protected => false,
    }
}

fn cat_icon(c: Category) -> &'static str {
    use Category::*;
    match c {
        RecycleBin => ph::TRASH,
        UserTemp | SystemTemp => ph::CLOCK_COUNTER_CLOCKWISE,
        WindowsUpdate => ph::DOWNLOAD_SIMPLE,
        CrashDumps => ph::BUG,
        WindowsLogs => ph::WINDOWS_LOGO,
        OldWindows => ph::ARCHIVE,
        BrowserCache => ph::GLOBE,
        AppCache => ph::APP_WINDOW,
        ThumbnailCache => ph::IMAGE,
        GpuCache => ph::CPU,
        DevCache => ph::PACKAGE,
        NodeModules => ph::CUBE,
        BuildArtifacts => ph::HAMMER,
        PythonEnv => ph::CODE,
        LogFiles => ph::FILE_TEXT,
        LargeFiles => ph::FILE,
        Custom => ph::FOLDER,
        AppLeftovers => ph::FOLDER_DASHED,
    }
}

fn restore_hint(c: Category) -> &'static str {
    use Category::*;
    match c {
        NodeModules => "Run npm install, pnpm install or yarn in the project.",
        BuildArtifacts => "Rebuilt by your next build (cargo build, dotnet build, next build...).",
        DevCache => "Downloaded again the next time a project needs it.",
        PythonEnv => "python -m venv .venv, then pip install -r requirements.txt.",
        BrowserCache | AppCache | GpuCache | ThumbnailCache => "Nothing to do - recreated automatically.",
        WindowsUpdate => "Windows downloads it again if it still needs it.",
        RecycleBin | UserTemp | SystemTemp | CrashDumps | LogFiles | WindowsLogs => "Not needed - nothing to restore.",
        OldWindows => "Cannot be restored - rolling back to the previous Windows is no longer possible.",
        LargeFiles | Custom | AppLeftovers => "Only from your own backup (or the Recycle Bin option).",
    }
}

fn fmt_compact(n: u64) -> String {
    match n {
        0..=9_999 => fmt_count(n),
        10_000..=999_999 => format!("{:.1}K", n as f64 / 1e3),
        _ => format!("{:.2}M", n as f64 / 1e6),
    }
}

fn display_path(p: &Path, kind: ItemKind) -> String {
    match kind {
        ItemKind::RecycleBin => format!("Recycle Bin on {}", p.display()),
        _ => p.display().to_string(),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

fn node_at<'a>(root: &'a DirNode, path: &[PathBuf]) -> &'a DirNode {
    let mut n = root;
    for p in path {
        match n.children.iter().find(|c| &c.path == p) {
            Some(c) => n = c,
            None => break,
        }
    }
    n
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Home,
    Cleanable(Option<Category>),
    Usage,
    Large,
    Full,
    Report,
    Apps,
}

#[derive(Clone, Copy, PartialEq)]
enum Show {
    All,
    Safe,
    Review,
    Protected,
}

#[derive(Clone, Copy, PartialEq)]
enum AppShow {
    All,
    Desktop,
    Store,
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Size,
    Name,
    Group,
}

/// UI events, applied after drawing so views only need `&self`.
enum Act {
    ToggleFinding(usize),
    FocusFinding(usize),
    SelectSafeIn(Option<Category>),
    SelectNone,
    View(View),
    Show(Show),
    AppShow(AppShow),
    Sort(Sort),
    FocusUsage(PathBuf),
    ToggleTree(PathBuf, u64),
    UsagePush(PathBuf),
    UsageTruncate(usize),
    Clean,
    Scan,
    ScanRoot(String),
    StopScan,
    StopClean,
    Reveal(PathBuf),
    Admin,
    ResetPc,
    FullClean { run: bool },
    ToggleTrash,
    ToggleDism,
    ToggleHibernate,
    ConfirmGo,
    ConfirmCancel,
    FocusApp(usize),
    ToggleLeftover(usize, usize),
    RescanLeftovers(usize),
    AskUninstall(usize),
    AskRemoveEntry(usize),
    StopUninstall,
    DeleteLeftovers(usize),
    FilterFocused,
    DismissToast,
}

struct Group {
    cat: Category,
    items: Vec<usize>,
    total: u64,
}

struct Plan {
    items: Vec<Finding>,
    maint: Maintenance,
    policy: Policy,
    bytes: u64,
    caution: Vec<String>,
    blocked: Vec<(PathBuf, &'static str)>,
    warn: Option<String>,
    app: Option<usize>,
}

enum Confirm {
    Clean(Plan),
    Uninstall(usize),
    RemoveEntry(usize),
}

enum LeftState {
    Loading,
    Ready(Vec<Leftover>),
}

pub struct App {
    drives: Vec<win::DriveInfo>,
    root_input: String,
    is_admin: bool,
    message: Option<(String, Color32)>,

    scan_rx: Option<Receiver<ScanResult>>,
    scan_prog: Option<Arc<Progress>>,
    scan_started: Instant,
    scan_expected: Option<u64>,
    scan_root: String,

    result: Option<ScanResult>,
    groups: Vec<Group>,
    view: View,
    show: Show,
    app_show: AppShow,
    sort: Sort,
    filter: String,
    focus_filter: bool,
    focus_finding: Option<usize>,
    focus_usage: Option<PathBuf>,
    usage_path: Vec<PathBuf>,
    tree_selected: HashMap<PathBuf, u64>,
    scroll_to: Cell<Option<usize>>,
    table_view: Cell<(f32, f32)>,

    use_trash: bool,
    maint: Maintenance,
    confirm: Option<Confirm>,
    clean_rx: Option<Receiver<CleanReport>>,
    clean_prog: Option<Arc<CleanProgress>>,
    clean_app: Option<usize>,
    report: Option<CleanReport>,

    apps: Vec<InstalledApp>,
    apps_rx: Option<Receiver<AppsMsg>>,
    focus_app: Option<usize>,
    leftovers: HashMap<usize, LeftState>,
    left_rx: Vec<(usize, Receiver<Vec<Leftover>>)>,
    uninstall: Option<(usize, Arc<UninstallProgress>, Receiver<Outcome>)>,
    removed: HashSet<usize>,
    toast_text: Option<String>,
    toast_since: Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        k::setup(&cc.egui_ctx);
        let drives = win::drives();
        let arg = std::env::args().nth(1).filter(|a| !a.trim().is_empty());
        let root_input = arg.clone().unwrap_or_else(|| {
            drives
                .iter()
                .find(|d| d.root.eq_ignore_ascii_case("C:\\"))
                .or(drives.first())
                .map(|d| d.root.clone())
                .unwrap_or_else(|| "C:\\".into())
        });
        let mut app = Self {
            drives,
            root_input,
            is_admin: win::is_admin(),
            message: None,
            scan_rx: None,
            scan_prog: None,
            scan_started: Instant::now(),
            scan_expected: None,
            scan_root: String::new(),
            result: None,
            groups: Vec::new(),
            view: View::Home,
            show: Show::All,
            app_show: AppShow::All,
            sort: Sort::Size,
            filter: String::new(),
            focus_filter: false,
            focus_finding: None,
            focus_usage: None,
            usage_path: Vec::new(),
            tree_selected: HashMap::new(),
            scroll_to: Cell::new(None),
            table_view: Cell::new((0.0, 400.0)),
            use_trash: false,
            maint: Maintenance::default(),
            confirm: None,
            clean_rx: None,
            clean_prog: None,
            clean_app: None,
            report: None,
            apps: Vec::new(),
            apps_rx: Some(apps::load()),
            focus_app: None,
            leftovers: HashMap::new(),
            left_rx: Vec::new(),
            uninstall: None,
            removed: HashSet::new(),
            toast_text: None,
            toast_since: Instant::now(),
        };
        // `logcleaner.exe D:\projects` scans right away; `logcleaner.exe --apps` opens the app list.
        match arg.as_deref() {
            Some("--apps") => {
                app.root_input = "C:\\".into();
                app.view = View::Apps;
            }
            Some(_) => app.start_scan(),
            None => {}
        }
        app
    }

    // ------------------------------------------------------------ state

    fn busy(&self) -> bool {
        self.scan_rx.is_some() || self.clean_rx.is_some()
    }

    fn modal(&self) -> bool {
        self.confirm.is_some() || self.clean_prog.is_some() || self.uninstall.is_some()
    }

    fn drive_for(&self, root: &str) -> Option<&win::DriveInfo> {
        let r = root.trim();
        self.drives.iter().find(|d| r.len() >= 2 && d.root[..2].eq_ignore_ascii_case(&r[..2]))
    }

    fn start_scan(&mut self) {
        let mut root = self.root_input.trim().trim_matches('"').replace('/', "\\");
        if root.len() == 2 && root.ends_with(':') {
            root.push('\\');
        }
        let path = PathBuf::from(&root);
        if !path.is_dir() {
            self.message = Some((format!("'{root}' is not a folder or drive."), k::DANGER));
            return;
        }
        self.message = None;
        self.scan_expected = self
            .drives
            .iter()
            .find(|d| d.root.eq_ignore_ascii_case(&root))
            .map(|d| d.total.saturating_sub(d.free));
        let prog = Arc::new(Progress::default());
        self.scan_rx = Some(scanner::start(path, prog.clone()));
        self.scan_prog = Some(prog);
        self.scan_started = Instant::now();
        self.scan_root = root;
        self.result = None;
        self.groups.clear();
        self.tree_selected.clear();
        self.usage_path.clear();
        self.focus_finding = None;
        self.focus_usage = None;
        if self.view != View::Apps {
            self.view = View::Home;
        }
    }

    fn regroup(&mut self) {
        let Some(res) = &self.result else { return };
        let mut map: BTreeMap<Category, Vec<usize>> = BTreeMap::new();
        for (i, f) in res.findings.iter().enumerate() {
            map.entry(f.category).or_default().push(i);
        }
        let mut groups: Vec<Group> = map
            .into_iter()
            .map(|(cat, items)| {
                let total = items.iter().map(|&i| res.findings[i].size).sum();
                Group { cat, items, total }
            })
            .collect();
        groups.sort_by(|a, b| b.total.cmp(&a.total));
        self.groups = groups;
        if self.focus_finding.is_some_and(|i| i >= res.findings.len()) {
            self.focus_finding = None;
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let mut wake = false;

        let mut scan_done = None;
        if let Some(rx) = &self.scan_rx {
            match rx.try_recv() {
                Ok(r) => scan_done = Some(Some(r)),
                Err(TryRecvError::Disconnected) => scan_done = Some(None),
                Err(TryRecvError::Empty) => wake = true,
            }
        }
        if let Some(r) = scan_done {
            self.scan_rx = None;
            self.scan_prog = None;
            match r {
                Some(r) => {
                    if r.cancelled {
                        self.message = Some(("Scan stopped - results are partial.".into(), k::REVIEW));
                    }
                    self.result = Some(r);
                    self.regroup();
                    if self.view != View::Apps {
                        self.view = View::Cleanable(None);
                        self.dev_view();
                    }
                }
                None => self.message = Some(("The scan stopped unexpectedly.".into(), k::DANGER)),
            }
        }

        let mut clean_done = None;
        if let Some(rx) = &self.clean_rx {
            match rx.try_recv() {
                Ok(r) => clean_done = Some(Some(r)),
                Err(TryRecvError::Disconnected) => clean_done = Some(None),
                Err(TryRecvError::Empty) => wake = true,
            }
        }
        if let Some(r) = clean_done {
            self.clean_rx = None;
            self.clean_prog = None;
            let app = self.clean_app.take();
            match r {
                Some(r) => {
                    let done: HashSet<&PathBuf> = r.processed.iter().collect();
                    if let Some(res) = &mut self.result {
                        res.findings.retain(|f| !done.contains(&f.path));
                    }
                    self.tree_selected.retain(|p, _| !done.contains(p));
                    let freed = fmt_size(r.freed);
                    let failed = r.failed;
                    self.report = Some(r);
                    self.regroup();
                    self.drives = win::drives();
                    match app {
                        Some(i) => {
                            let tail = if failed > 0 { format!(", {failed} files could not be deleted") } else { String::new() };
                            self.message = Some((format!("Deleted {freed} of leftovers{tail}."), k::SAFE));
                            self.request_leftovers(i);
                        }
                        None => self.view = View::Report,
                    }
                }
                None => self.message = Some(("Cleaning stopped unexpectedly.".into(), k::DANGER)),
            }
        }

        if let Some(rx) = &self.apps_rx {
            loop {
                match rx.try_recv() {
                    Ok(AppsMsg::List(list)) => self.apps = list,
                    Ok(AppsMsg::Sizes(sizes)) => {
                        for (i, s) in sizes {
                            if let Some(a) = self.apps.get_mut(i) {
                                a.size = s;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        wake = true;
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.apps_rx = None;
                        break;
                    }
                }
            }
        }

        let mut ready = Vec::new();
        self.left_rx.retain(|(i, rx)| match rx.try_recv() {
            Ok(list) => {
                ready.push((*i, list));
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => false,
        });
        for (i, list) in ready {
            self.leftovers.insert(i, LeftState::Ready(list));
        }
        wake |= !self.left_rx.is_empty();

        let mut outcome = None;
        if let Some((i, _, rx)) = &self.uninstall {
            match rx.try_recv() {
                Ok(o) => outcome = Some((*i, o)),
                Err(TryRecvError::Disconnected) => outcome = Some((*i, Outcome { removed: false, message: "The uninstall stopped unexpectedly.".into() })),
                Err(TryRecvError::Empty) => wake = true,
            }
        }
        if let Some((i, o)) = outcome {
            self.uninstall = None;
            if o.removed {
                self.removed.insert(i);
                self.message = Some((format!("{} Review what it left behind on the right.", o.message), k::SAFE));
            } else {
                self.message = Some((o.message, k::REVIEW));
            }
            self.request_leftovers(i);
        }

        if wake {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    /// Dev aid for screenshot checks: LOGCLEANER_VIEW=row|usage|usage-row|large|full picks the page shown after a scan.
    fn dev_view(&mut self) {
        let Ok(v) = std::env::var("LOGCLEANER_VIEW") else { return };
        match v.as_str() {
            "row" => self.focus_finding = self.visible_findings().first().copied(),
            "usage" | "usage-row" => {
                self.view = View::Usage;
                if v == "usage-row" {
                    self.focus_usage = self.usage_node().and_then(|n| n.children.first()).map(|c| c.path.clone());
                }
            }
            "large" => self.view = View::Large,
            "full" => self.view = View::Full,
            _ => {}
        }
    }

    fn request_leftovers(&mut self, i: usize) {
        let Some(app) = self.apps.get(i) else { return };
        self.leftovers.insert(i, LeftState::Loading);
        self.left_rx.retain(|(j, _)| *j != i);
        self.left_rx.push((i, apps::start_leftovers(app.clone(), self.apps.clone())));
    }

    fn reclaimable(&self) -> (u64, u64) {
        let Some(res) = &self.result else { return (0, 0) };
        let (mut safe, mut review) = (0, 0);
        for f in res.findings.iter().filter(|f| f.category != Category::LargeFiles) {
            match f.safety {
                Safety::Safe => safe += f.size,
                Safety::Caution => review += f.size,
                Safety::Protected => {}
            }
        }
        (safe, review)
    }

    fn selection(&self) -> (usize, u64, usize) {
        let (mut n, mut bytes, mut caution) = (0, 0, 0);
        if let Some(res) = &self.result {
            for f in res.findings.iter().filter(|f| f.selected) {
                n += 1;
                bytes += f.size;
                caution += (f.safety != Safety::Safe) as usize;
            }
        }
        for size in self.tree_selected.values() {
            n += 1;
            bytes += size;
            caution += 1;
        }
        (n, bytes, caution)
    }

    fn filter_lower(&self) -> String {
        self.filter.trim().to_lowercase()
    }

    fn show_ok(&self, s: Safety) -> bool {
        match self.show {
            Show::All => true,
            Show::Safe => s == Safety::Safe,
            Show::Review => s == Safety::Caution,
            Show::Protected => s == Safety::Protected,
        }
    }

    /// Indices into `findings` for the current table view, filtered and sorted.
    fn visible_findings(&self) -> Vec<usize> {
        let Some(res) = &self.result else { return Vec::new() };
        let q = self.filter_lower();
        let mut v: Vec<usize> = res
            .findings
            .iter()
            .enumerate()
            .filter(|(_, f)| match self.view {
                View::Cleanable(None) => f.category != Category::LargeFiles,
                View::Cleanable(Some(c)) => f.category == c,
                View::Large => f.category == Category::LargeFiles,
                _ => false,
            })
            .filter(|(_, f)| self.show_ok(f.safety))
            .filter(|(_, f)| q.is_empty() || f.path.to_string_lossy().to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        let fs = &res.findings;
        match self.sort {
            Sort::Size => v.sort_by(|&a, &b| fs[b].size.cmp(&fs[a].size)),
            Sort::Name => v.sort_by_cached_key(|&i| fs[i].path.to_string_lossy().to_lowercase()),
            Sort::Group => v.sort_by(|&a, &b| fs[a].category.cmp(&fs[b].category).then(fs[b].size.cmp(&fs[a].size))),
        }
        v
    }

    fn usage_node(&self) -> Option<&DirNode> {
        self.result.as_ref().map(|r| node_at(&r.tree, &self.usage_path))
    }

    fn visible_usage(&self) -> Vec<usize> {
        let Some(node) = self.usage_node() else { return Vec::new() };
        let q = self.filter_lower();
        let mut v: Vec<usize> = node
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| q.is_empty() || c.name.to_lowercase().contains(&q))
            .filter(|(_, c)| self.show == Show::All || self.show_ok(safety::assess(&c.path).0))
            .map(|(i, _)| i)
            .collect();
        let ch = &node.children;
        match self.sort {
            Sort::Size | Sort::Group => v.sort_by(|&a, &b| ch[b].size.cmp(&ch[a].size)),
            Sort::Name => v.sort_by_cached_key(|&i| ch[i].name.to_lowercase()),
        }
        v
    }

    fn visible_apps(&self) -> Vec<usize> {
        let q = self.filter_lower();
        let mut v: Vec<usize> = self
            .apps
            .iter()
            .enumerate()
            .filter(|(_, a)| match self.app_show {
                AppShow::All => true,
                AppShow::Desktop => !a.is_store(),
                AppShow::Store => a.is_store(),
            })
            .filter(|(_, a)| q.is_empty() || a.name.to_lowercase().contains(&q) || a.publisher.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        let a = &self.apps;
        match self.sort {
            Sort::Size => v.sort_by(|&x, &y| a[y].size.cmp(&a[x].size)),
            Sort::Name => v.sort_by_cached_key(|&i| a[i].name.to_lowercase()),
            Sort::Group => v.sort_by_cached_key(|&i| (a[i].publisher.to_lowercase(), a[i].name.to_lowercase())),
        }
        v
    }

    fn build_plan(&self, maint: Maintenance) -> Plan {
        let mut items: Vec<Finding> = self
            .result
            .iter()
            .flat_map(|r| r.findings.iter().filter(|f| f.selected).cloned())
            .collect();
        for (p, size) in &self.tree_selected {
            let (s, note) = safety::assess(p);
            items.push(Finding {
                category: Category::Custom,
                path: p.clone(),
                size: *size,
                files: 0,
                safety: s,
                note: note.to_string(),
                mode: DeleteMode::Whole,
                kind: ItemKind::Dir,
                selected: true,
            });
        }

        let mut blocked = Vec::new();
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
                let mut nested = false;
                while let Some(i) = anc.rfind('\\') {
                    anc = &anc[..i];
                    if kept.contains(anc) || kept.contains(&format!("{anc}\\")) {
                        nested = true;
                        break;
                    }
                }
                if nested {
                    continue;
                }
                kept.insert(key);
            }
            out.push(f);
        }
        out.sort_by(|a, b| a.category.cmp(&b.category).then(b.size.cmp(&a.size)));
        finish_plan(out, maint, Policy::Junk, blocked, None, None)
    }

    fn leftover_plan(&self, i: usize) -> Option<Plan> {
        let app = self.apps.get(i)?;
        let Some(LeftState::Ready(list)) = self.leftovers.get(&i) else { return None };
        let mut blocked = Vec::new();
        let items: Vec<Finding> = list
            .iter()
            .filter(|l| l.selected)
            .filter(|l| match safety::uninstall_blocked(&l.path) {
                Some(r) => {
                    blocked.push((l.path.clone(), r));
                    false
                }
                None => true,
            })
            .map(|l| Finding {
                category: Category::AppLeftovers,
                path: l.path.clone(),
                size: l.size,
                files: l.files,
                safety: l.safety,
                note: format!("{} - {}", l.what, l.note),
                mode: DeleteMode::Whole,
                kind: if l.path.is_file() { ItemKind::File } else { ItemKind::Dir },
                selected: true,
            })
            .collect();
        let warn = (!self.removed.contains(&i) && app.has_uninstaller()).then(|| {
            format!("{} is still installed. Deleting its files will break it - uninstall it first.", app.name)
        });
        Some(finish_plan(items, Maintenance::default(), Policy::Uninstall, blocked, warn, Some(i)))
    }

    fn apply(&mut self, act: Act) {
        match act {
            Act::ToggleFinding(i) => {
                if let Some(f) = self.result.as_mut().and_then(|r| r.findings.get_mut(i)) {
                    if f.safety != Safety::Protected {
                        f.selected = !f.selected;
                    }
                }
                self.focus_finding = Some(i);
            }
            Act::FocusFinding(i) => self.focus_finding = Some(i),
            Act::SelectSafeIn(cat) => {
                if let Some(res) = &mut self.result {
                    for f in &mut res.findings {
                        let in_view = match cat {
                            Some(c) => f.category == c,
                            None => f.category != Category::LargeFiles,
                        };
                        if in_view && f.safety == Safety::Safe {
                            f.selected = true;
                        }
                    }
                }
            }
            Act::SelectNone => {
                if let Some(res) = &mut self.result {
                    res.findings.iter_mut().for_each(|f| f.selected = false);
                }
                self.tree_selected.clear();
            }
            Act::View(v) => {
                if v != self.view {
                    self.view = v;
                    self.show = Show::All;
                    self.scroll_to.set(Some(0));
                    if v == View::Usage {
                        self.focus_usage = None;
                    }
                }
            }
            Act::Show(s) => self.show = s,
            Act::AppShow(s) => self.app_show = s,
            Act::Sort(s) => self.sort = s,
            Act::FocusUsage(p) => self.focus_usage = Some(p),
            Act::ToggleTree(p, size) => {
                self.focus_usage = Some(p.clone());
                if self.tree_selected.remove(&p).is_none() {
                    self.tree_selected.insert(p, size);
                }
            }
            Act::UsagePush(p) => {
                self.usage_path.push(p);
                self.focus_usage = None;
                self.scroll_to.set(Some(0));
            }
            Act::UsageTruncate(n) => {
                let back = self.usage_path.get(n).cloned();
                self.usage_path.truncate(n);
                self.focus_usage = back;
            }
            Act::Clean => {
                if !self.busy() && self.selection().0 > 0 {
                    self.confirm = Some(Confirm::Clean(self.build_plan(Maintenance::default())));
                }
            }
            Act::Scan => {
                if !self.busy() {
                    self.start_scan();
                }
            }
            Act::ScanRoot(r) => {
                self.root_input = r;
                if !self.busy() {
                    self.start_scan();
                }
            }
            Act::StopScan => {
                if let Some(p) = &self.scan_prog {
                    p.cancel.store(true, Relaxed);
                }
            }
            Act::StopClean => {
                if let Some(p) = &self.clean_prog {
                    p.cancel.store(true, Relaxed);
                }
            }
            Act::Reveal(p) => win::reveal(&p),
            Act::Admin => {
                if win::relaunch_as_admin() {
                    std::process::exit(0);
                }
            }
            Act::ResetPc => win::open_reset_this_pc(),
            Act::FullClean { run } => {
                if let Some(res) = &mut self.result {
                    for f in &mut res.findings {
                        f.selected = full_clean_pick(f);
                    }
                }
                self.tree_selected.clear();
                if run {
                    self.confirm = Some(Confirm::Clean(self.build_plan(self.maint)));
                } else {
                    self.view = View::Cleanable(None);
                }
            }
            Act::ToggleTrash => self.use_trash = !self.use_trash,
            Act::ToggleDism => self.maint.dism = !self.maint.dism,
            Act::ToggleHibernate => self.maint.hibernate_off = !self.maint.hibernate_off,
            Act::ConfirmGo => match self.confirm.take() {
                Some(Confirm::Clean(plan)) => {
                    let prog = Arc::new(CleanProgress::default());
                    self.clean_app = plan.app;
                    self.clean_rx = Some(cleaner::start(plan.items, self.use_trash, plan.maint, plan.policy, prog.clone()));
                    self.clean_prog = Some(prog);
                }
                Some(Confirm::Uninstall(i)) => {
                    if let Some(app) = self.apps.get(i) {
                        let prog = Arc::new(UninstallProgress::default());
                        let rx = apps::start_uninstall(app.clone(), prog.clone());
                        self.uninstall = Some((i, prog, rx));
                    }
                }
                Some(Confirm::RemoveEntry(i)) => {
                    if let Some(app) = self.apps.get(i) {
                        match apps::remove_entry(app) {
                            Ok(()) => {
                                self.removed.insert(i);
                                self.message = Some((format!("Removed {} from the installed list.", app.name), k::SAFE));
                            }
                            Err(e) => self.message = Some((e, k::DANGER)),
                        }
                    }
                }
                None => {}
            },
            Act::ConfirmCancel => self.confirm = None,
            Act::FocusApp(i) => {
                self.focus_app = Some(i);
                if !self.leftovers.contains_key(&i) {
                    self.request_leftovers(i);
                }
            }
            Act::ToggleLeftover(i, j) => {
                if let Some(LeftState::Ready(list)) = self.leftovers.get_mut(&i) {
                    if let Some(l) = list.get_mut(j) {
                        if l.safety != Safety::Protected {
                            l.selected = !l.selected;
                        }
                    }
                }
            }
            Act::RescanLeftovers(i) => self.request_leftovers(i),
            Act::AskUninstall(i) => self.confirm = Some(Confirm::Uninstall(i)),
            Act::AskRemoveEntry(i) => self.confirm = Some(Confirm::RemoveEntry(i)),
            Act::StopUninstall => {
                if let Some((_, p, _)) = &self.uninstall {
                    p.cancel.store(true, Relaxed);
                }
            }
            Act::DeleteLeftovers(i) => {
                if !self.busy() {
                    if let Some(plan) = self.leftover_plan(i) {
                        if !plan.items.is_empty() {
                            self.confirm = Some(Confirm::Clean(plan));
                        }
                    }
                }
            }
            Act::FilterFocused => self.focus_filter = false,
            Act::DismissToast => self.message = None,
        }
    }

    /// Arrow keys, Space, Delete, Enter, Backspace, Ctrl+F, Esc.
    fn keyboard(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let (up, down, space, del, enter, back, find, esc) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Backspace),
                i.modifiers.command && i.key_pressed(Key::F),
                i.key_pressed(Key::Escape),
            )
        });
        if esc {
            if self.confirm.is_some() {
                acts.push(Act::ConfirmCancel);
            } else {
                self.filter.clear();
            }
            return;
        }
        if find {
            self.focus_filter = true;
        }
        // Arrow keys, Space and Del belong to a focused button or input, not the list.
        if self.modal() || ctx.egui_wants_keyboard_input() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let step: i64 = if down { 1 } else if up { -1 } else { 0 };
        match self.view {
            View::Cleanable(_) | View::Large => {
                let list = self.visible_findings();
                if step != 0 && !list.is_empty() {
                    let pos = self.focus_finding.and_then(|f| list.iter().position(|&i| i == f));
                    let next = pos.map_or(0, |p| (p as i64 + step).clamp(0, list.len() as i64 - 1) as usize);
                    self.focus_finding = Some(list[next]);
                    self.scroll_to.set(Some(next));
                }
                if space {
                    if let Some(i) = self.focus_finding {
                        acts.push(Act::ToggleFinding(i));
                    }
                }
            }
            View::Usage => {
                let list = self.visible_usage();
                let Some(node) = self.usage_node() else { return };
                let pos = self.focus_usage.as_ref().and_then(|f| list.iter().position(|&i| &node.children[i].path == f));
                if step != 0 && !list.is_empty() {
                    let next = pos.map_or(0, |p| (p as i64 + step).clamp(0, list.len() as i64 - 1) as usize);
                    acts.push(Act::FocusUsage(node.children[list[next]].path.clone()));
                    self.scroll_to.set(Some(next));
                }
                if let Some(c) = pos.map(|p| &node.children[list[p]]) {
                    if space && safety::assess(&c.path).0 != Safety::Protected {
                        acts.push(Act::ToggleTree(c.path.clone(), c.size));
                    }
                    if enter && !c.children.is_empty() {
                        acts.push(Act::UsagePush(c.path.clone()));
                    }
                }
                if back && !self.usage_path.is_empty() {
                    acts.push(Act::UsageTruncate(self.usage_path.len() - 1));
                }
            }
            View::Apps => {
                let list = self.visible_apps();
                if step != 0 && !list.is_empty() {
                    let pos = self.focus_app.and_then(|f| list.iter().position(|&i| i == f));
                    let next = pos.map_or(0, |p| (p as i64 + step).clamp(0, list.len() as i64 - 1) as usize);
                    acts.push(Act::FocusApp(list[next]));
                    self.scroll_to.set(Some(next));
                }
            }
            _ => {}
        }
        if del && self.view != View::Apps {
            acts.push(Act::Clean);
        }
    }

    // ------------------------------------------------------------ top bar

    fn top_bar(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let busy = self.busy();
        let scanning = self.scan_prog.is_some();
        let (target_icon, target) = match self.drive_for(&self.root_input) {
            Some(d) if self.root_input.trim().len() <= 3 => (ph::HARD_DRIVE, format!("{}  {} disk", d.root.trim_end_matches('\\'), d.kind)),
            _ => (ph::FOLDER, short(&self.root_input, 34)),
        };
        let stats = self.result.as_ref().map(|r| {
            let s = format!("{} files scanned in {:.1}s", fmt_compact(r.files), r.elapsed.as_secs_f32());
            let mut tip = format!("{} files in {} folders", fmt_count(r.files), fmt_count(r.dirs));
            if r.errors > 0 {
                tip += &format!("\n{} folders could not be read (run as administrator to see them)", fmt_count(r.errors));
            }
            (s, tip)
        });
        let is_admin = self.is_admin;

        ui.horizontal(|ui| {
            ui.set_height(k::CTL_H);
            k::icon_tile(ui, ph::BROOM, 30.0, Color32::WHITE, k::ACC);
            ui.label(RichText::new("LogCleaner").font(k::bold(16.5)).color(k::FG));
            ui.add_space(12.0);
            let (sep, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
            ui.painter().rect_filled(sep, 0.0, k::LINE);
            ui.add_space(12.0);

            let resp = k::select(ui, target_icon, &target, Some(260.0), !busy).on_hover_text("Choose the drive or folder to scan");
            egui::Popup::from_toggle_button_response(&resp)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .width(360.0)
                .gap(6.0)
                .show(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    k::option_heading(ui, "Drives");
                    let current = self.root_input.trim().to_lowercase();
                    for d in &self.drives {
                        let used = d.total.saturating_sub(d.free);
                        let w = ui.available_width();
                        let (r, row) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
                        let on = current == d.root.to_lowercase();
                        let p = ui.painter();
                        if on {
                            p.rect_filled(r, k::R_SM, k::ACC_BG);
                        } else if row.hovered() {
                            p.rect_filled(r, k::R_SM, k::PANE);
                        }
                        p.text(pos2(r.left() + 10.0, r.top() + 17.0), Align2::LEFT_CENTER, ph::HARD_DRIVE, k::body(15.0), if on { k::ACC } else { k::MUT });
                        p.text(pos2(r.left() + 36.0, r.top() + 17.0), Align2::LEFT_CENTER, format!("{}  {} disk", d.root.trim_end_matches('\\'), d.kind), k::semi(14.0), if on { k::ACC } else { k::FG });
                        p.text(pos2(r.right() - 10.0, r.top() + 17.0), Align2::RIGHT_CENTER, format!("{} free of {}", fmt_size(d.free), fmt_size(d.total)), k::body(13.0), k::MUT);
                        k::bar(p, Rect::from_min_size(pos2(r.left() + 36.0, r.top() + 32.0), vec2(w - 46.0, 4.0)), used as f32 / d.total.max(1) as f32, k::ACC);
                        if row.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            acts.push(Act::ScanRoot(d.root.clone()));
                            egui::Popup::close_all(ui.ctx());
                        }
                    }
                    ui.add_space(6.0);
                    k::option_heading(ui, "Or a folder");
                    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), k::CTL_H), Sense::hover());
                    if self.folder_row(ui, row, "folder") {
                        acts.push(Act::Scan);
                        egui::Popup::close_all(ui.ctx());
                    }
                });

            if scanning {
                if k::button(ui, &format!("{}  Stop scan", ph::STOP), Look::Secondary, true).clicked() {
                    acts.push(Act::StopScan);
                }
            } else if k::button(ui, &format!("{}  Scan", ph::PLAY), Look::Primary, !busy)
                .on_hover_text("Scan the selected drive or folder")
                .clicked()
            {
                acts.push(Act::Scan);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if is_admin {
                    ui.label(RichText::new(format!("{}  Administrator", ph::SHIELD_CHECK)).font(k::semi(14.0)).color(k::SAFE));
                } else if k::button(ui, &format!("{}  Run as admin", ph::SHIELD), Look::Secondary, true)
                    .on_hover_text("Needed to clean Windows temp, update cache, Windows.old and other users' files, and to uninstall most apps.")
                    .clicked()
                {
                    acts.push(Act::Admin);
                }
                if let Some((s, tip)) = &stats {
                    ui.add_space(8.0);
                    ui.label(RichText::new(s).font(k::body(14.0)).color(k::MUT)).on_hover_text(tip);
                }
            });
        });
    }

    // ------------------------------------------------------------ sidebar

    fn sidebar(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let has = self.result.is_some();
        let (safe, review) = self.reclaimable();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if k::nav_item(ui, ph::HOUSE, "Home", "", self.view == View::Home, true) {
                acts.push(Act::View(View::Home));
            }
            // Scan-dependent pages stay hidden until there is a scan, rather than showing as greyed-out rows.
            if has {
                k::nav_section(ui, "Clean up");
                if k::nav_item(ui, ph::BROOM, "All cleanable", &fmt_size(safe + review), self.view == View::Cleanable(None), true) {
                    acts.push(Act::View(View::Cleanable(None)));
                }
                for g in self.groups.iter().filter(|g| g.cat != Category::LargeFiles) {
                    let v = View::Cleanable(Some(g.cat));
                    if k::nav_item(ui, cat_icon(g.cat), g.cat.label(), &fmt_size(g.total), self.view == v, true) {
                        acts.push(Act::View(v));
                    }
                }
            }
            let n_large = self.groups.iter().find(|g| g.cat == Category::LargeFiles).map_or(0, |g| g.items.len());
            let explore: Vec<_> = [
                (View::Usage, ph::SQUARES_FOUR, "Space usage", String::new()),
                (View::Large, ph::FILE, "Large files", n_large.to_string()),
                (View::Full, ph::SPARKLE, "Full clean", String::new()),
                (View::Report, ph::CLIPBOARD_TEXT, "Last report", String::new()),
            ]
            .into_iter()
            .filter(|(v, ..)| has || *v == View::Report && self.report.is_some())
            .collect();
            if !explore.is_empty() {
                k::nav_section(ui, "Explore");
            }
            for (v, icon, label, right) in explore {
                if k::nav_item(ui, icon, label, &right, self.view == v, true) {
                    acts.push(Act::View(v));
                }
            }
            k::nav_section(ui, "Programs");
            let count = if self.apps_rx.is_some() && self.apps.is_empty() { "...".to_string() } else { self.apps.len().to_string() };
            if k::nav_item(ui, ph::APP_WINDOW, "Installed apps", &count, self.view == View::Apps, true) {
                acts.push(Act::View(View::Apps));
            }
        });
    }

    // ------------------------------------------------------------ pages

    fn page(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        if self.scan_prog.is_some() && self.view != View::Apps {
            self.scanning_page(ui, acts);
            return;
        }
        match self.view {
            View::Home => self.home_page(ui, acts),
            View::Cleanable(_) | View::Large => self.findings_page(ui, acts),
            View::Usage => self.usage_page(ui, acts),
            View::Full => self.full_page(ui, acts),
            View::Report => self.report_page(ui, acts),
            View::Apps => self.apps_page(ui, acts),
        }
    }

    /// Search box, safety/type filter and sort menu above a table.
    fn list_toolbar(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let apps_view = self.view == View::Apps;
        let hint = match self.view {
            View::Apps => "Search apps",
            View::Usage => "Filter folders",
            _ => "Filter by path",
        };
        let seg = |l: &str, c: String| (l.to_string(), c);
        let (labels, selected): (Vec<(String, String)>, usize) = if apps_view {
            let desktop = self.apps.iter().filter(|a| !a.is_store()).count();
            (
                vec![seg("All", self.apps.len().to_string()), seg("Desktop", desktop.to_string()), seg("Store", (self.apps.len() - desktop).to_string())],
                match self.app_show {
                    AppShow::All => 0,
                    AppShow::Desktop => 1,
                    AppShow::Store => 2,
                },
            )
        } else {
            let (safe, review) = self.reclaimable();
            (
                vec![seg("All", String::new()), seg("Safe", fmt_size(safe)), seg("Review", fmt_size(review)), seg("Protected", String::new())],
                match self.show {
                    Show::All => 0,
                    Show::Safe => 1,
                    Show::Review => 2,
                    Show::Protected => 3,
                },
            )
        };
        let group = if apps_view { "Publisher" } else { "Category" };
        let sort_label = match self.sort {
            Sort::Size => "Size",
            Sort::Name => "Name",
            Sort::Group => group,
        };
        let focus_now = self.focus_filter;
        let gap = 10.0;
        let w = ui.available_width();
        let seg_w = k::segmented_width(ui, &labels);
        let sort_w = 150.0;
        // One row when the search box still gets 220px, otherwise the tabs move to a second row.
        let one_row = w - seg_w - sort_w - 2.0 * gap >= 220.0;
        let search_w = if one_row { (w - seg_w - sort_w - 2.0 * gap).min(360.0) } else { w - sort_w - gap };
        let mut seg_clicked = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            k::input(ui, "filter", &mut self.filter, &format!("{hint}  (Ctrl+F)"), ph::MAGNIFYING_GLASS, search_w, focus_now)
                .on_hover_text("Ctrl+F jumps here, Esc clears it");
            if one_row {
                seg_clicked = k::segmented(ui, &labels, selected);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let resp = k::select(ui, ph::SORT_DESCENDING, sort_label, Some(sort_w), true).on_hover_text("Sort the list");
                egui::Popup::from_toggle_button_response(&resp)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                    .align(egui::RectAlign::BOTTOM_END)
                    .width(220.0)
                    .gap(6.0)
                    .show(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        k::option_heading(ui, "Sort by");
                        for (s, icon, label, detail) in [
                            (Sort::Size, ph::SORT_DESCENDING, "Size", "largest first"),
                            (Sort::Name, ph::TEXT_AA, "Name", "A to Z"),
                            (Sort::Group, ph::STACK, group, ""),
                        ] {
                            if k::option(ui, icon, label, detail, self.sort == s).clicked() {
                                acts.push(Act::Sort(s));
                            }
                        }
                    });
            });
        });
        if !one_row {
            ui.add_space(gap);
            seg_clicked = k::segmented(ui, &labels, selected);
        }
        if focus_now {
            acts.push(Act::FilterFocused);
        }
        if let Some(i) = seg_clicked {
            acts.push(if apps_view {
                Act::AppShow([AppShow::All, AppShow::Desktop, AppShow::Store][i])
            } else {
                Act::Show([Show::All, Show::Safe, Show::Review, Show::Protected][i])
            });
        }
    }

    /// Reserves room for the table card and, when something is ticked, the bulk action bar.
    fn table_card(&self, ui: &mut Ui, with_bar: bool, acts: &mut Vec<Act>, f: impl FnOnce(&mut Ui)) {
        let selected = self.selection().0 > 0;
        let footer = if !with_bar { 0.0 } else if selected { 76.0 } else { 34.0 };
        let h = (ui.available_height() - footer).max(120.0);
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| {
            k::card_frame().show(ui, |ui| {
                ui.set_width(w - 2.0);
                ui.set_height(h - 2.0);
                f(ui);
            });
        });
        if with_bar {
            if selected {
                ui.add_space(10.0);
                self.bulk_bar(ui, acts);
            } else {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let tip = |ui: &mut Ui, s: &str| ui.label(RichText::new(s).font(k::body(13.0)).color(k::MUT));
                    tip(ui, &format!("{}  Tick rows to clean them", ph::INFO));
                    ui.add_space(6.0);
                    k::kbd(ui, "Space");
                    tip(ui, "tick");
                    ui.add_space(6.0);
                    k::kbd(ui, "Del");
                    tip(ui, "clean");
                    ui.add_space(6.0);
                    tip(ui, "Double-click to open");
                });
            }
        }
    }

    fn findings_page(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let list = self.visible_findings();
        let Some(res) = &self.result else { return };
        let cat = match self.view {
            View::Cleanable(c) => c,
            View::Large => Some(Category::LargeFiles),
            _ => None,
        };
        let total: u64 = list.iter().map(|&i| res.findings[i].size).sum();
        let title = cat.map_or("All cleanable", |c| c.label());
        let desc = cat.map_or("Everything LogCleaner can clean on this drive, biggest first. Green is safe to delete, amber needs a quick look, red is never deleted.", |c| c.description());
        let any_selected = self.selection().0 > 0;
        page_header(ui, cat.map_or(ph::BROOM, cat_icon), title, &format!("{}  \u{00B7}  {} items", fmt_size(total), fmt_count(list.len() as u64)), desc, |ui| {
            if any_selected && k::button(ui, "Clear selection", Look::Ghost, true).clicked() {
                acts.push(Act::SelectNone);
            }
            if cat != Some(Category::LargeFiles) && k::button(ui, &format!("{}  Select all safe", ph::CHECK_SQUARE), Look::Secondary, true).clicked() {
                acts.push(Act::SelectSafeIn(cat));
            }
        });
        self.list_toolbar(ui, acts);
        ui.add_space(12.0);

        let res = self.result.as_ref().expect("checked above");
        let fs = &res.findings;
        let rel = |f: &Finding| match (f.kind, f.path.strip_prefix(&res.root)) {
            (ItemKind::RecycleBin, _) => display_path(&f.path, f.kind),
            (_, Ok(r)) if !r.as_os_str().is_empty() => r.display().to_string(),
            _ => f.path.display().to_string(),
        };
        let sort = match self.sort {
            Sort::Size => 4,
            Sort::Name => 1,
            Sort::Group => 2,
        };
        let mut out = k::TableOut::default();
        self.table_card(ui, true, acts, |ui| {
            if list.is_empty() {
                empty_state(ui, ph::CHECK_CIRCLE, if self.filter.is_empty() { "Nothing to clean here" } else { "No items match your filter" }, "Try another category or clear the filter.");
                return;
            }
            const COLS: [Col; 5] = [col("", 48.0, false), col("Path", 0.0, false), col("Category", 200.0, false), col("Safety", 110.0, false), col("Size", 100.0, true)];
            out = k::table(
                ui,
                "findings",
                &COLS,
                true,
                list.len(),
                Some(sort),
                self.scroll_to.take(),
                &self.table_view,
                &|r| {
                    let f = &fs[list[r]];
                    RowState {
                        check: Some((if f.selected { Check::On } else { Check::Off }, f.safety != Safety::Protected)),
                        focus: self.focus_finding == Some(list[r]),
                    }
                },
                &mut |ui, p, r, c, rect| {
                    let f = &fs[list[r]];
                    match c {
                        1 => k::cell_path(ui, p, rect, &rel(f), k::body(14.0), k::FG),
                        2 => k::cell_text(ui, p, rect, format!("{}  {}", cat_icon(f.category), f.category.label()), k::body(14.0), k::MUT, false),
                        3 => k::safety_pill(ui, p, rect, f.safety),
                        4 => k::cell_text(ui, p, rect, fmt_size(f.size), k::semi(14.0), k::FG, true),
                        _ => {}
                    }
                },
            );
        });
        header_sort(out.header, &[(1, Sort::Name), (2, Sort::Group), (4, Sort::Size)], acts);
        if let Some(r) = out.check {
            acts.push(Act::ToggleFinding(list[r]));
        }
        if let Some(r) = out.clicked {
            acts.push(Act::FocusFinding(list[r]));
        }
        if let Some(r) = out.double {
            acts.push(Act::Reveal(fs[list[r]].path.clone()));
        }
    }

    fn usage_page(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let list = self.visible_usage();
        let Some(res) = &self.result else { return };
        let node = node_at(&res.tree, &self.usage_path);
        page_header(ui, ph::SQUARES_FOUR, "Space usage", &format!("{}  \u{00B7}  {} files", fmt_size(node.size), fmt_compact(node.files)), "Biggest folders first. Double-click or Enter opens a folder, Backspace goes back up. Tick any folder to delete it; protected folders can't be ticked.", |_| {});
        ui.horizontal_wrapped(|ui| {
            if k::button_small(ui, &format!("{}  {}", ph::HARD_DRIVE, res.root.display()), if self.usage_path.is_empty() { Look::Secondary } else { Look::Ghost }, true).clicked() {
                acts.push(Act::UsageTruncate(0));
            }
            for (i, p) in self.usage_path.iter().enumerate() {
                ui.label(RichText::new(ph::CARET_RIGHT).color(k::MUT2));
                let last = i + 1 == self.usage_path.len();
                if k::button_small(ui, &file_name(p), if last { Look::Secondary } else { Look::Ghost }, true).clicked() {
                    acts.push(Act::UsageTruncate(i + 1));
                }
            }
        });
        ui.add_space(8.0);
        self.list_toolbar(ui, acts);
        ui.add_space(12.0);

        let res = self.result.as_ref().expect("checked above");
        let node = node_at(&res.tree, &self.usage_path);
        let ch = &node.children;
        let sort = if self.sort == Sort::Name { 1 } else { 4 };
        let mut out = k::TableOut::default();
        self.table_card(ui, true, acts, |ui| {
            if list.is_empty() {
                empty_state(ui, ph::FOLDER_OPEN, "No sub-folders over 1 MB here", "Go back up, or clear the filter.");
                return;
            }
            const COLS: [Col; 5] = [col("", 48.0, false), col("Folder", 0.0, false), col("Share", 160.0, false), col("Safety", 118.0, false), col("Size", 100.0, true)];
            out = k::table(
                ui,
                "usage",
                &COLS,
                true,
                list.len(),
                Some(sort),
                self.scroll_to.take(),
                &self.table_view,
                &|r| {
                    let c = &ch[list[r]];
                    let s = safety::assess(&c.path).0;
                    RowState {
                        check: Some((if self.tree_selected.contains_key(&c.path) { Check::On } else { Check::Off }, s != Safety::Protected)),
                        focus: self.focus_usage.as_ref() == Some(&c.path),
                    }
                },
                &mut |ui, p, r, ci, rect| {
                    let c = &ch[list[r]];
                    match ci {
                        1 => {
                            let icon = if c.children.is_empty() { ph::FOLDER_SIMPLE } else { ph::FOLDER };
                            let g = k::galley(ui, format!("{icon}   {}", c.name), k::body(14.0), k::FG, rect.width());
                            let w = g.size().x;
                            p.galley(pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, k::FG);
                            if let Some(t) = c.tag {
                                if rect.width() - w > 90.0 {
                                    k::pill(ui, p, pos2(rect.left() + w + 10.0, rect.center().y), t.label(), k::ACC, k::ACC_BG);
                                }
                            }
                        }
                        2 => {
                            let frac = c.size as f32 / node.size.max(1) as f32;
                            k::bar(p, Rect::from_min_size(pos2(rect.left(), rect.center().y - 3.0), vec2(90.0, 6.0)), frac, k::ACC);
                            k::cell_text(ui, p, Rect::from_min_max(pos2(rect.left() + 100.0, rect.top()), rect.max), format!("{:.0}%", frac * 100.0), k::body(13.0), k::MUT, false);
                        }
                        3 => k::safety_pill(ui, p, rect, safety::assess(&c.path).0),
                        4 => k::cell_text(ui, p, rect, fmt_size(c.size), k::semi(14.0), k::FG, true),
                        _ => {}
                    }
                },
            );
        });
        header_sort(out.header, &[(1, Sort::Name), (4, Sort::Size)], acts);
        if let Some(r) = out.check {
            let c = &ch[list[r]];
            acts.push(Act::ToggleTree(c.path.clone(), c.size));
        }
        if let Some(r) = out.clicked {
            acts.push(Act::FocusUsage(ch[list[r]].path.clone()));
        }
        if let Some(r) = out.double {
            let c = &ch[list[r]];
            acts.push(if c.children.is_empty() { Act::Reveal(c.path.clone()) } else { Act::UsagePush(c.path.clone()) });
        }
    }

    fn apps_page(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let list = self.visible_apps();
        let total: u64 = list.iter().map(|&i| self.apps[i].size).sum();
        let measuring = if self.apps_rx.is_some() { "  \u{00B7}  measuring sizes..." } else { "" };
        page_header(ui, ph::APP_WINDOW, "Installed apps", &format!("{} apps  \u{00B7}  {}{measuring}", list.len(), fmt_size(total)), "Pick an app to see its folders. Uninstall runs the app's own uninstaller; afterwards LogCleaner finds the files it left behind so you can delete them too.", |_| {});
        self.list_toolbar(ui, acts);
        ui.add_space(12.0);
        if self.apps.is_empty() {
            k::card(ui, 24.0, |ui| {
                ui.set_width(ui.available_width());
                k::spinner_text(ui, "Reading the installed programs list...");
            });
            return;
        }
        let sort = match self.sort {
            Sort::Name => 0,
            Sort::Group => 1,
            Sort::Size => 4,
        };
        let apps = &self.apps;
        let mut out = k::TableOut::default();
        self.table_card(ui, false, acts, |ui| {
            if list.is_empty() {
                empty_state(ui, ph::MAGNIFYING_GLASS, "No apps match your search", "Check the spelling or switch the filter to All.");
                return;
            }
            const COLS: [Col; 5] = [col("App", 0.0, false), col("Publisher", 220.0, false), col("Version", 130.0, false), col("Installed", 110.0, false), col("Size", 96.0, true)];
            out = k::table(
                ui,
                "apps",
                &COLS,
                false,
                list.len(),
                Some(sort),
                self.scroll_to.take(),
                &self.table_view,
                &|r| RowState { check: None, focus: self.focus_app == Some(list[r]) },
                &mut |ui, p, r, c, rect| {
                    let i = list[r];
                    let a = &apps[i];
                    match c {
                        0 => {
                            let gone = self.removed.contains(&i);
                            let icon = if a.is_store() { ph::STOREFRONT } else { ph::APP_WINDOW };
                            let ir = Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(26.0, 26.0));
                            p.rect_filled(ir, 7.0, if a.is_store() { k::ACC_BG } else { k::PANE });
                            p.text(ir.center(), Align2::CENTER_CENTER, icon, k::body(15.0), if a.is_store() { k::ACC } else { k::FG2 });
                            let tr = Rect::from_min_max(pos2(rect.left() + 36.0, rect.top()), rect.max);
                            let g = k::galley(ui, a.name.clone(), k::semi(14.0), if gone { k::MUT } else { k::FG }, tr.width() - if gone { 100.0 } else { 0.0 });
                            let w = g.size().x;
                            p.galley(pos2(tr.left(), tr.center().y - g.size().y / 2.0), g, k::FG);
                            if gone {
                                k::pill(ui, p, pos2(tr.left() + w + 10.0, tr.center().y), "Uninstalled", k::SAFE, k::SAFE_BG);
                            }
                        }
                        1 => k::cell_text(ui, p, rect, &a.publisher, k::body(14.0), k::MUT, false),
                        2 => k::cell_text(ui, p, rect, &a.version, k::body(13.0), k::MUT, false),
                        3 => k::cell_text(ui, p, rect, &a.installed, k::body(13.0), k::MUT, false),
                        _ => k::cell_text(ui, p, rect, if a.size > 0 { fmt_size(a.size) } else { "-".into() }, k::semi(14.0), k::FG, true),
                    }
                },
            );
        });
        header_sort(out.header, &[(0, Sort::Name), (1, Sort::Group), (4, Sort::Size)], acts);
        if let Some(r) = out.clicked.or(out.double) {
            acts.push(Act::FocusApp(list[r]));
        }
    }

    /// Folder path input with a Scan button at its right, inside `row`; true when the user submits.
    fn folder_row(&mut self, ui: &mut Ui, row: Rect, id: &str) -> bool {
        let bw = 84.0;
        let field = Rect::from_min_max(row.min, pos2(row.right() - bw - 8.0, row.bottom()));
        let btn = Rect::from_min_max(pos2(row.right() - bw, row.top()), row.max);
        let busy = self.busy();
        // Detached children: `row` is already allocated, and placing these must not move the cursor.
        let edit = k::input(&mut ui.new_child(egui::UiBuilder::new().max_rect(field)), id, &mut self.root_input, "Paste a folder path, e.g. D:\\projects", ph::FOLDER, field.width(), false);
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let go = k::button_ex(&mut ui.new_child(egui::UiBuilder::new().max_rect(btn)), "Scan", Look::Primary, !busy, k::CTL_H, Some(bw));
        !busy && (go.clicked() || enter)
    }

    fn home_page(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let busy = self.busy();
        let sys = self.drives.iter().position(|d| d.root.eq_ignore_ascii_case("C:\\")).or((!self.drives.is_empty()).then_some(0));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // A centred column, capped so lines stay readable on wide windows; 14px is left for the scrollbar.
            let avail = ui.available_width() - 14.0;
            let w = avail.min(1040.0);
            ui.horizontal_top(|ui| {
                ui.add_space(((avail - w) / 2.0).max(0.0));
                ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(w);
                // Page header straight on the background, like a web page; the drives card is the main action.
                ui.add_space(8.0);
                ui.label(RichText::new("Free up space on your PC").font(k::bold(26.0)).color(k::FG));
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new("Pick a drive to scan, review what LogCleaner finds, then clean it. Nothing is deleted until you tick it and confirm.").font(k::body(15.0)).color(k::MUT)).wrap());

                ui.add_space(24.0);
                k::card(ui, 0.0, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for i in 0..self.drives.len() {
                        if i > 0 {
                            k::hairline(ui);
                        }
                        if self.drive_row(ui, i, sys == Some(i), busy) {
                            acts.push(Act::ScanRoot(self.drives[i].root.clone()));
                        }
                    }
                    k::hairline(ui);
                    egui::Frame::NONE.fill(k::ROW_HOVER).inner_margin(Margin::symmetric(20, 16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new("Scan one folder").font(k::semi(14.0)).color(k::FG));
                        ui.label(RichText::new("Faster when you only want to clean a projects or downloads folder.").font(k::body(13.0)).color(k::MUT));
                        ui.add_space(10.0);
                        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), k::CTL_H), Sense::hover());
                        if self.folder_row(ui, row, "home_folder") {
                            acts.push(Act::Scan);
                        }
                    });
                });

                ui.add_space(32.0);
                k::section_title(ui, "Good to know", "");
                k::card_grid(ui, 2, 320.0, 2, 20.0, |ui, i| match i {
                    0 => Self::help_safety(ui),
                    _ => Self::help_keys(ui),
                });
                ui.add_space(12.0);
                });
            });
        });
    }

    /// One drive in the home page's drives card; true when its Scan button is clicked.
    /// The system drive gets the primary button, so the page has one obvious action.
    fn drive_row(&self, ui: &mut Ui, i: usize, primary: bool, busy: bool) -> bool {
        let d = &self.drives[i];
        let used = d.total.saturating_sub(d.free);
        let frac = used as f32 / d.total.max(1) as f32;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
        let p = ui.painter().clone();
        let tile = Rect::from_center_size(pos2(r.left() + 40.0, r.center().y), vec2(36.0, 36.0));
        p.rect_filled(tile, k::R_CTL, k::ACC_BG);
        p.text(tile.center(), Align2::CENTER_CENTER, ph::HARD_DRIVE, k::body(18.0), k::ACC);
        // Name column, usage bar in the middle, Scan button on the right.
        let btn_w = 92.0;
        let right = r.right() - 20.0 - btn_w - 24.0;
        let name_x = tile.right() + 14.0;
        let bar_x = (name_x + 190.0).min(right - 120.0);
        let name = Rect::from_min_max(pos2(name_x, r.top() + 16.0), pos2(bar_x - 16.0, r.center().y));
        k::cell_text(ui, &p, name, format!("{} disk ({})", d.kind, d.root.trim_end_matches('\\')), k::semi(15.0), k::FG, false);
        let sub = Rect::from_min_max(pos2(name_x, r.center().y), pos2(bar_x - 16.0, r.bottom() - 16.0));
        k::cell_text(ui, &p, sub, format!("{} free of {}", fmt_size(d.free), fmt_size(d.total)), k::body(13.0), k::MUT, false);
        let label = Rect::from_min_max(pos2(bar_x, r.top() + 16.0), pos2(right, r.center().y - 2.0));
        k::cell_text(ui, &p, label, format!("{:.0}% used", frac * 100.0), k::body(13.0), k::MUT, false);
        k::cell_text(ui, &p, label, fmt_size(used), k::body(13.0), k::MUT, true);
        let bar = Rect::from_min_max(pos2(bar_x, r.center().y + 4.0), pos2(right, r.center().y + 10.0));
        k::bar(&p, bar, frac, if frac > 0.9 { k::DANGER } else { k::ACC });
        let btn = Rect::from_center_size(pos2(r.right() - 20.0 - btn_w / 2.0, r.center().y), vec2(btn_w, k::CTL_H));
        let label = format!("{}  Scan", ph::PLAY);
        // Detached child, so the button doesn't pull the cursor back up into this row.
        k::button_ex(&mut ui.new_child(egui::UiBuilder::new().max_rect(btn)), &label, if primary { Look::Primary } else { Look::Secondary }, !busy, k::CTL_H, Some(btn_w)).clicked()
    }

    fn scanning_page(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let Some(p) = &self.scan_prog else { return };
        let secs = self.scan_started.elapsed().as_secs_f32().max(0.001);
        let files = p.files.load(Relaxed);
        let bytes = p.bytes.load(Relaxed);
        page_header(ui, ph::MAGNIFYING_GLASS, &format!("Scanning {}", self.scan_root), "", "Reading every folder in parallel. You can keep using your PC; results appear when the scan finishes.", |ui| {
            if k::button(ui, &format!("{}  Stop scan", ph::STOP), Look::Secondary, true).clicked() {
                acts.push(Act::StopScan);
            }
        });
        let frac = self.scan_expected.map(|e| (bytes as f32 / e.max(1) as f32).min(0.999));
        k::card(ui, 22.0, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(frac.map_or("Working...".to_string(), |f| format!("{:.0}%", f * 100.0))).font(k::bold(26.0)).color(k::FG));
                ui.label(RichText::new(format!("{} read", fmt_size(bytes))).font(k::body(15.0)).color(k::MUT));
            });
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
            match frac {
                Some(f) => k::progress(ui.painter(), r, f),
                None => {
                    let t = (ui.input(|i| i.time) as f32 * 0.5).fract();
                    ui.painter().rect_filled(r, 5.0, k::PANE);
                    let seg = Rect::from_min_size(pos2(r.left() + (r.width() - r.width() * 0.25) * t, r.top()), vec2(r.width() * 0.25, r.height()));
                    ui.painter().rect_filled(seg, 5.0, k::ACC);
                }
            }
            ui.add_space(6.0);
            let cur = p.current.lock().map(|c| c.clone()).unwrap_or_default();
            let g = k::galley(ui, cur, k::mono(13.0), k::MUT, ui.available_width());
            ui.label(g);
        });
        ui.add_space(k::GAP);
        k::stats(ui, &[
            ("Files", fmt_count(files), k::FG),
            ("Folders", fmt_count(p.dirs.load(Relaxed)), k::FG),
            ("Speed", format!("{} files/s", fmt_compact((files as f32 / secs) as u64)), k::FG),
            ("Elapsed", format!("{secs:.1}s"), k::FG),
        ]);
        ui.ctx().request_repaint();
    }

    fn full_picks(&self) -> Vec<(Category, usize, u64)> {
        let mut per: BTreeMap<Category, (usize, u64)> = BTreeMap::new();
        for f in self.result.iter().flat_map(|r| r.findings.iter()).filter(|f| full_clean_pick(f)) {
            let e = per.entry(f.category).or_default();
            e.0 += 1;
            e.1 += f.size;
        }
        let mut v: Vec<(Category, usize, u64)> = per.into_iter().map(|(c, (n, b))| (c, n, b)).collect();
        v.sort_by(|a, b| b.2.cmp(&a.2));
        v
    }

    fn full_page(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let picks = self.full_picks();
        let total: u64 = picks.iter().map(|p| p.2).sum();
        page_header(ui, ph::SPARKLE, "Full clean", "", "Everything that is safe to remove, plus Windows leftovers only needed for rollback or troubleshooting. Your documents, photos, downloads, installed programs, large files, Python venvs and JS dist folders are not touched.", |_| {});
        k::stats(ui, &[
            ("Will be freed", fmt_size(total), k::SAFE),
            ("Categories", picks.len().to_string(), k::FG),
            ("Items", fmt_count(picks.iter().map(|p| p.1 as u64).sum()), k::FG),
        ]);
        ui.add_space(k::GAP);
        // A short list gets a card sized to its rows instead of an empty box down to the bottom.
        let h = (38.0 + picks.len().max(1) as f32 * k::ROW_H + 4.0).min(ui.available_height());
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| self.table_card(ui, false, acts, |ui| {
            const COLS: [Col; 3] = [col("Category", 0.0, false), col("Items", 110.0, true), col("Size", 120.0, true)];
            k::table(ui, "full", &COLS, false, picks.len(), None, None, &self.table_view, &|_| RowState { check: None, focus: false }, &mut |ui, p, r, c, rect| {
                let (cat, n, b) = picks[r];
                match c {
                    0 => k::cell_text(ui, p, rect, format!("{}   {}", cat_icon(cat), cat.label()), k::body(14.0), k::FG, false),
                    1 => k::cell_text(ui, p, rect, fmt_count(n as u64), k::body(14.0), k::MUT, true),
                    _ => k::cell_text(ui, p, rect, fmt_size(b), k::semi(14.0), k::FG, true),
                }
            });
        }));
    }

    fn report_page(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let Some(r) = &self.report else {
            empty_state(ui, ph::CLIPBOARD_TEXT, "Nothing cleaned yet", "After you clean, the results appear here.");
            return;
        };
        let busy = self.busy();
        page_header(ui, ph::CLIPBOARD_TEXT, "Cleaning report", &format!("{:.1}s{}", r.elapsed.as_secs_f32(), if r.cancelled { "  \u{00B7}  stopped early" } else { "" }), if r.used_trash { "Items were moved to the Recycle Bin - the space is freed when you empty it." } else { "Rescan to refresh the sizes." }, |ui| {
            if k::button(ui, &format!("{}  Rescan", ph::ARROW_CLOCKWISE), Look::Primary, !busy).clicked() {
                acts.push(Act::Scan);
            }
        });
        k::stats(ui, &[
            (if r.used_trash { "Moved to bin" } else { "Freed" }, fmt_size(r.freed), k::SAFE),
            ("Files removed", fmt_count(r.files), k::FG),
            ("Couldn't delete", fmt_count(r.failed), if r.failed > 0 { k::REVIEW } else { k::FG }),
            ("Protected, skipped", r.skipped.len().to_string(), if r.skipped.is_empty() { k::FG } else { k::DANGER }),
        ]);
        ui.add_space(k::GAP);
        k::card(ui, 18.0, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height() - 4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for m in &r.maintenance {
                    ui.label(RichText::new(m).color(k::FG2));
                }
                if r.skipped.is_empty() && r.failed == 0 && r.maintenance.is_empty() {
                    ui.label(RichText::new(format!("{}  Everything selected was removed.", ph::CHECK_CIRCLE)).font(k::semi(15.0)).color(k::SAFE));
                }
                if !r.skipped.is_empty() {
                    ui.label(RichText::new("Protected items that were skipped").font(k::semi(15.0)).color(k::DANGER));
                    for (p, why) in r.skipped.iter().take(200) {
                        ui.label(RichText::new(format!("{p}  -  {why}")).font(k::mono(13.0)).color(k::MUT));
                    }
                    ui.add_space(10.0);
                }
                if r.failed > 0 {
                    ui.label(RichText::new("Files that could not be deleted").font(k::semi(15.0)).color(k::REVIEW));
                    ui.label(RichText::new("Usually open in a running app or needing administrator. Normal for Temp folders.").color(k::MUT));
                    for (p, why) in &r.failures {
                        ui.label(RichText::new(format!("{p}  -  {why}")).font(k::mono(13.0)).color(k::MUT));
                    }
                }
            });
        });
    }

    // ------------------------------------------------------------ detail panel

    fn detail(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match self.view {
                View::Cleanable(_) | View::Large => self.detail_finding(ui, acts),
                View::Usage => self.detail_usage(ui, acts),
                View::Full => self.detail_full(ui, acts),
                View::Apps => self.detail_app(ui, acts),
                View::Home | View::Report => {}
            }
        });
    }

    /// Safety levels explained; one card on the home page.
    fn help_safety(ui: &mut Ui) {
        ui.label(RichText::new("Safety labels").font(k::semi(15.0)).color(k::FG));
        ui.add_space(6.0);
        for (s, d) in [
            (Safety::Safe, "Rebuilt or downloaded again automatically. Fine to delete."),
            (Safety::Caution, "Probably fine, but check it is not something you need."),
            (Safety::Protected, "Windows, installed programs, your profile folders, .git. Never deleted."),
        ] {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(104.0, 22.0), Layout::left_to_right(Align::Center), |ui| k::badge(ui, s));
                ui.add(egui::Label::new(RichText::new(d).font(k::body(14.0)).color(k::FG2)).wrap());
            });
            ui.add_space(4.0);
        }
    }

    /// Keyboard shortcuts in two aligned columns; one card on the home page.
    fn help_keys(ui: &mut Ui) {
        ui.label(RichText::new("Keyboard shortcuts").font(k::semi(15.0)).color(k::FG));
        ui.add_space(6.0);
        for (keys, what) in [
            ("Up / Down", "Move through a list"),
            ("Space", "Tick the highlighted row"),
            ("Del", "Clean what is ticked"),
            ("Enter / Backspace", "Open a folder / go back up"),
            ("Ctrl F", "Search"),
            ("Esc", "Clear search, close a dialog"),
        ] {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(130.0, 22.0), Layout::left_to_right(Align::Center), |ui| k::kbd(ui, keys));
                ui.label(RichText::new(what).font(k::body(14.0)).color(k::FG2));
            });
        }
    }

    fn detail_finding(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let Some(res) = &self.result else { return };
        let Some(f) = self.focus_finding.and_then(|i| res.findings.get(i)) else {
            let cat = match self.view {
                View::Cleanable(c) => c,
                View::Large => Some(Category::LargeFiles),
                _ => None,
            };
            empty_detail(ui, cat.map_or(ph::BROOM, cat_icon), "Select a row", "Click any row to see what it is, why it is safe or not, and how to get it back.");
            if let Some(c) = cat {
                ui.add_space(10.0);
                k::callout(ui, k::Tone::Info, "How to get it back", |ui| {
                    ui.label(RichText::new(restore_hint(c)).color(k::FG2));
                });
            }
            return;
        };
        k::badge(ui, f.safety);
        ui.add_space(2.0);
        ui.label(RichText::new(file_name(&f.path)).font(k::bold(18.0)).color(k::FG));
        ui.label(RichText::new(display_path(&f.path, f.kind)).font(k::mono(13.0)).color(k::MUT));
        ui.add_space(8.0);
        let mode = match (f.kind, f.mode) {
            (ItemKind::RecycleBin, _) => "Empty the Recycle Bin",
            (_, DeleteMode::Contents) => "Empty the folder, keep it",
            (ItemKind::File, _) => "Delete the file",
            _ => "Delete the folder",
        };
        k::kv(ui, &[
            ("Size", fmt_size(f.size)),
            ("Files", fmt_count(f.files)),
            ("Category", f.category.label().to_string()),
            ("What happens", mode.to_string()),
        ]);
        ui.add_space(10.0);
        let (tone, title) = match f.safety {
            Safety::Safe => (k::Tone::Safe, "Why it's safe"),
            Safety::Caution => (k::Tone::Review, "Check before deleting"),
            Safety::Protected => (k::Tone::Danger, "Why it's protected"),
        };
        k::callout(ui, tone, title, |ui| {
            ui.label(RichText::new(&f.note).color(k::FG));
            ui.label(RichText::new(f.category.description()).font(k::body(13.0)).color(k::FG2));
        });
        ui.add_space(8.0);
        k::callout(ui, k::Tone::Info, "How to get it back", |ui| {
            ui.label(RichText::new(restore_hint(f.category)).color(k::FG2));
        });
        ui.add_space(14.0);
        let i = self.focus_finding.unwrap_or_default();
        let (label, look) = if f.selected { (format!("{}  Untick", ph::X), Look::Secondary) } else { (format!("{}  Tick for cleaning", ph::CHECK), Look::Primary) };
        if k::button_wide(ui, &label, look, f.safety != Safety::Protected).clicked() {
            acts.push(Act::ToggleFinding(i));
        }
        if f.kind != ItemKind::RecycleBin && k::button_wide(ui, &format!("{}  Show in Explorer", ph::FOLDER_OPEN), Look::Secondary, true).clicked() {
            acts.push(Act::Reveal(f.path.clone()));
        }
    }

    fn detail_usage(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let Some(node) = self.usage_node() else { return };
        let focused = self.focus_usage.as_ref().and_then(|p| node.children.iter().find(|c| &c.path == p));
        let n = focused.unwrap_or(node);
        let (s, note) = if focused.is_some() { safety::assess(&n.path) } else { (Safety::Protected, "The folder you are browsing.") };
        if focused.is_some() {
            k::badge(ui, s);
            ui.add_space(2.0);
        }
        ui.label(RichText::new(&n.name).font(k::bold(18.0)).color(k::FG));
        ui.label(RichText::new(n.path.display().to_string()).font(k::mono(13.0)).color(k::MUT));
        ui.add_space(8.0);
        let mut rows = vec![("Size", fmt_size(n.size)), ("Files", fmt_count(n.files))];
        if focused.is_some() {
            rows.push(("Share of parent", format!("{:.1}%", n.size as f64 / node.size.max(1) as f64 * 100.0)));
        }
        k::kv(ui, &rows);
        ui.add_space(10.0);
        match n.tag {
            Some(cat) => k::callout(ui, k::Tone::Safe, &format!("Cleanable: {}", cat.label()), |ui| {
                ui.label(RichText::new(restore_hint(cat)).color(k::FG2));
            }),
            None if focused.is_some() => {
                let tone = match s {
                    Safety::Safe => k::Tone::Safe,
                    Safety::Caution => k::Tone::Review,
                    Safety::Protected => k::Tone::Danger,
                };
                k::callout(ui, tone, s.label(), |ui| {
                    ui.label(RichText::new(note).color(k::FG2));
                });
            }
            None => {}
        }
        if !n.children.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("What's inside").font(k::semi(15.0)).color(k::FG));
            let items: Vec<(String, u64)> = n.children.iter().take(40).map(|c| (c.name.clone(), c.size)).collect();
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 190.0), Sense::hover());
            if let Some(j) = k::treemap(ui, r, &items) {
                if focused.is_some() {
                    acts.push(Act::UsagePush(n.path.clone()));
                }
                acts.push(Act::FocusUsage(n.children[j].path.clone()));
            }
        }
        ui.add_space(12.0);
        if focused.is_some() && !n.children.is_empty() && k::button_wide(ui, &format!("Open this folder  {}", ph::ARROW_RIGHT), Look::Primary, true).clicked() {
            acts.push(Act::UsagePush(n.path.clone()));
        }
        if k::button_wide(ui, &format!("{}  Show in Explorer", ph::FOLDER_OPEN), Look::Secondary, true).clicked() {
            acts.push(Act::Reveal(n.path.clone()));
        }
    }

    fn detail_full(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let total: u64 = self.full_picks().iter().map(|p| p.2).sum();
        let hiberfil = self
            .result
            .iter()
            .flat_map(|r| r.findings.iter())
            .find(|f| f.path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("hiberfil.sys")))
            .map(|f| f.size);
        ui.label(RichText::new("Extra Windows maintenance").font(k::bold(16.0)).color(k::FG));
        if !self.is_admin {
            k::callout(ui, k::Tone::Review, "Needs administrator", |ui| {
                if ui.link("Restart LogCleaner as administrator").clicked() {
                    acts.push(Act::Admin);
                }
            });
        }
        ui.add_space(6.0);
        if option_row(ui, self.maint.dism, self.is_admin, "Clean up the component store", "DISM /StartComponentCleanup removes superseded update files from WinSxS. Takes 5-15 minutes.") {
            acts.push(Act::ToggleDism);
        }
        let hib = hiberfil.map_or("Turn off hibernation".to_string(), |s| format!("Turn off hibernation (frees {})", fmt_size(s)));
        if option_row(ui, self.maint.hibernate_off, self.is_admin, &hib, "Deletes hiberfil.sys and disables Fast Startup. Undo with powercfg /hibernate on.") {
            acts.push(Act::ToggleHibernate);
        }
        ui.add_space(10.0);
        let can = !self.busy() && (total > 0 || self.maint.dism || self.maint.hibernate_off);
        if k::button_wide(ui, &format!("{}  Run full clean ({})", ph::BROOM, fmt_size(total)), Look::Danger, can).clicked() {
            acts.push(Act::FullClean { run: true });
        }
        if k::button_wide(ui, "Just select these items", Look::Secondary, true).clicked() {
            acts.push(Act::FullClean { run: false });
        }
        ui.add_space(16.0);
        k::callout(ui, k::Tone::Info, "Want a truly fresh Windows?", |ui| {
            ui.label(RichText::new("Cleaning can't remove installed programs or registry clutter. Windows' own Reset this PC reinstalls Windows and removes all apps (you can keep your files). Back up first.").color(k::FG2));
            ui.add_space(4.0);
            if k::button(ui, &format!("{}  Open Reset this PC", ph::ARROW_COUNTER_CLOCKWISE), Look::Secondary, true).clicked() {
                acts.push(Act::ResetPc);
            }
        });
    }

    fn detail_app(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let Some((i, a)) = self.focus_app.and_then(|i| self.apps.get(i).map(|a| (i, a))) else {
            empty_detail(ui, ph::APP_WINDOW, "Select an app", "See where it is installed, uninstall it, and remove the folders it leaves behind.");
            return;
        };
        let gone = self.removed.contains(&i);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
            ui.painter().rect_filled(r, 11.0, if a.is_store() { k::ACC_BG } else { k::PANE });
            ui.painter().text(r.center(), Align2::CENTER_CENTER, if a.is_store() { ph::STOREFRONT } else { ph::APP_WINDOW }, k::body(22.0), if a.is_store() { k::ACC } else { k::FG2 });
            ui.vertical(|ui| {
                ui.label(RichText::new(&a.name).font(k::bold(17.0)).color(k::FG));
                ui.label(RichText::new(if a.publisher.is_empty() { "Unknown publisher" } else { &a.publisher }).font(k::body(14.0)).color(k::MUT));
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (txt, fg, bg) = if a.is_store() { ("Store app", k::ACC, k::ACC_BG) } else { ("Desktop app", k::FG2, k::PANE) };
            let (r, _) = ui.allocate_exact_size(vec2(0.0, 22.0), Sense::hover());
            let w = k::pill(ui, ui.painter(), r.left_center(), txt, fg, bg);
            ui.add_space(w);
            if gone {
                let (r, _) = ui.allocate_exact_size(vec2(0.0, 22.0), Sense::hover());
                let w = k::pill(ui, ui.painter(), r.left_center(), "Uninstalled", k::SAFE, k::SAFE_BG);
                ui.add_space(w);
            }
        });
        ui.add_space(6.0);
        let mut rows = vec![
            ("Version", if a.version.is_empty() { "-".into() } else { a.version.clone() }),
            ("Size", if a.size > 0 { fmt_size(a.size) } else { "Unknown".into() }),
        ];
        if !a.installed.is_empty() {
            rows.push(("Installed", a.installed.clone()));
        }
        k::kv(ui, &rows);
        if let Some(loc) = &a.location {
            ui.add_space(4.0);
            ui.label(RichText::new(loc.display().to_string()).font(k::mono(13.0)).color(k::MUT));
        }
        ui.add_space(12.0);
        let running = self.uninstall.is_some();
        if a.has_uninstaller() {
            if k::button_wide(ui, &format!("{}  Uninstall", ph::TRASH), Look::Danger, !gone && !running).clicked() {
                acts.push(Act::AskUninstall(i));
            }
        } else if matches!(a.source, Source::Registry { .. })
            && k::button_wide(ui, "Remove from installed list", Look::Secondary, !gone)
                .on_hover_text("Its uninstaller is missing. Removes the app from Windows' installed list.")
                .clicked()
        {
            acts.push(Act::AskRemoveEntry(i));
        }
        if let Some(loc) = a.location.as_ref().filter(|l| l.is_dir()) {
            if k::button_wide(ui, &format!("{}  Open install folder", ph::FOLDER_OPEN), Look::Secondary, true).clicked() {
                acts.push(Act::Reveal(loc.clone()));
            }
        }

        ui.add_space(14.0);
        k::divider(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Files and folders").font(k::semi(15.0)).color(k::FG));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if k::button_small(ui, &format!("{}  Look again", ph::ARROW_CLOCKWISE), Look::Ghost, true).clicked() {
                    acts.push(Act::RescanLeftovers(i));
                }
            });
        });
        match self.leftovers.get(&i) {
            None | Some(LeftState::Loading) => k::spinner_text(ui, "Looking for the app's folders..."),
            Some(LeftState::Ready(list)) if list.is_empty() => {
                ui.label(RichText::new(if gone { "Nothing left behind." } else { "No folders found with this app's name." }).color(k::MUT));
            }
            Some(LeftState::Ready(list)) => {
                if !gone && a.has_uninstaller() {
                    k::callout(ui, k::Tone::Review, "Uninstall first", |ui| {
                        ui.label(RichText::new("Deleting these while the app is still installed will break it.").color(k::FG2));
                    });
                    ui.add_space(6.0);
                }
                for (j, l) in list.iter().enumerate() {
                    if leftover_row(ui, l) {
                        acts.push(Act::ToggleLeftover(i, j));
                    }
                }
                let (n, bytes) = list.iter().filter(|l| l.selected).fold((0, 0), |(n, b), l| (n + 1, b + l.size));
                ui.add_space(8.0);
                let label = format!("{}  Delete {n} selected ({})", ph::TRASH, fmt_size(bytes));
                if k::button_wide(ui, &label, Look::Danger, n > 0 && !self.busy()).clicked() {
                    acts.push(Act::DeleteLeftovers(i));
                }
            }
        }
    }

    // ------------------------------------------------------------ bulk bar, toast, modals

    fn bulk_bar(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let (n, bytes, caution) = self.selection();
        egui::Frame::NONE
            .fill(k::DARK)
            .corner_radius(k::R_CARD as u8)
            .inner_margin(Margin::symmetric(18, 10))
            .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(40) })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.set_height(36.0);
                    ui.label(RichText::new(format!("{n} selected")).font(k::semi(15.0)).color(Color32::WHITE));
                    ui.label(RichText::new(fmt_size(bytes)).font(k::bold(16.0)).color(Color32::WHITE));
                    if caution > 0 {
                        ui.label(RichText::new(format!("{}  {caution} to review", ph::WARNING)).font(k::body(14.0)).color(Color32::from_rgb(252, 211, 77)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if k::button(ui, &format!("{}  Delete {}", ph::TRASH, fmt_size(bytes)), Look::Danger, !self.busy()).clicked() {
                            acts.push(Act::Clean);
                        }
                        let bin = if self.use_trash { ph::CHECK_SQUARE } else { ph::SQUARE };
                        if k::button(ui, &format!("{bin}  Recycle Bin"), Look::OnDark, true)
                            .on_hover_text("Move to the Recycle Bin instead of deleting. Recoverable, but slower - the space is only freed after you empty the bin.")
                            .clicked()
                        {
                            acts.push(Act::ToggleTrash);
                        }
                        if k::button(ui, "Clear", Look::OnDark, true).clicked() {
                            acts.push(Act::SelectNone);
                        }
                    });
                });
            });
    }

    fn toast(&self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let Some((m, c)) = &self.message else { return };
        if self.toast_since.elapsed() > Duration::from_secs(8) {
            return;
        }
        let (icon, tint) = if *c == k::SAFE {
            (ph::CHECK_CIRCLE, Color32::from_rgb(74, 222, 128))
        } else if *c == k::DANGER {
            (ph::X_CIRCLE, Color32::from_rgb(248, 113, 113))
        } else {
            (ph::WARNING, Color32::from_rgb(252, 211, 77))
        };
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-24.0, -24.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(k::DARK)
                    .corner_radius(k::R_CARD as u8)
                    .inner_margin(Margin::symmetric(16, 12))
                    .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(50) })
                    .show(ui, |ui| {
                        ui.set_max_width(440.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).font(k::body(18.0)).color(tint));
                            ui.add(egui::Label::new(RichText::new(m).font(k::body(14.0)).color(Color32::WHITE)).wrap());
                            if k::button_small(ui, ph::X, Look::OnDark, true).clicked() {
                                acts.push(Act::DismissToast);
                            }
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn modals(&self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        if !self.modal() {
            return;
        }
        let frame = egui::Frame::window(&ctx.global_style()).fill(k::SURFACE).corner_radius(10).inner_margin(24.0);
        let window = |id: &str| {
            egui::Window::new(id)
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .frame(frame)
        };
        let head = |ui: &mut Ui, icon: &str, tint: Color32, bg: Color32, title: &str| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().rect_filled(r, k::R_CARD, bg);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, k::body(20.0), tint);
                ui.label(RichText::new(title).font(k::bold(19.0)).color(k::FG));
            });
            ui.add_space(4.0);
        };
        let buttons = |ui: &mut Ui, acts: &mut Vec<Act>, go: &str| {
            ui.add_space(16.0);
            k::divider(ui);
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 36.0), Layout::right_to_left(Align::Center), |ui| {
                if k::button(ui, go, Look::Danger, true).clicked() {
                    acts.push(Act::ConfirmGo);
                }
                if k::button(ui, "Cancel", Look::Secondary, true).clicked() {
                    acts.push(Act::ConfirmCancel);
                }
            });
        };

        match &self.confirm {
            Some(Confirm::Clean(plan)) => {
                window("confirm").show(ctx, |ui| {
                    ui.set_width(540.0);
                    head(ui, ph::TRASH, k::DANGER, k::DANGER_BG, &format!("Delete {}?", fmt_size(plan.bytes)));
                    let action = if self.use_trash { "moved to the Recycle Bin" } else { "permanently deleted" };
                    let n = plan.items.len();
                    ui.label(RichText::new(format!("{} {} will be {action}. Files that are in use are skipped.", fmt_count(n as u64), if n == 1 { "item" } else { "items" })).color(k::FG2));
                    if let Some(w) = &plan.warn {
                        ui.add_space(8.0);
                        k::callout(ui, k::Tone::Danger, "The app is still installed", |ui| {
                            ui.label(RichText::new(w).color(k::FG2));
                        });
                    }
                    if !plan.caution.is_empty() {
                        ui.add_space(8.0);
                        k::callout(ui, k::Tone::Review, &format!("{} items need a look", plan.caution.len()), |ui| {
                            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                                for c in &plan.caution {
                                    let g = k::galley(ui, c, k::mono(12.0), k::FG2, ui.available_width());
                                    ui.label(g);
                                }
                            });
                        });
                    }
                    if !plan.blocked.is_empty() {
                        ui.add_space(8.0);
                        k::callout(ui, k::Tone::Danger, &format!("{} protected items will be skipped", plan.blocked.len()), |ui| {
                            for (p, why) in plan.blocked.iter().take(6) {
                                ui.label(RichText::new(format!("{}  -  {why}", p.display())).font(k::mono(12.0)).color(k::FG2));
                            }
                        });
                    }
                    if plan.maint.dism || plan.maint.hibernate_off {
                        ui.add_space(8.0);
                        if plan.maint.dism {
                            ui.label(RichText::new("Then: Windows component store cleanup (several minutes).").color(k::FG2));
                        }
                        if plan.maint.hibernate_off {
                            ui.label(RichText::new("Then: turn off hibernation.").color(k::FG2));
                        }
                    }
                    buttons(ui, acts, &format!("{}  Delete", ph::TRASH));
                });
            }
            Some(Confirm::Uninstall(i)) => {
                let name = self.apps.get(*i).map_or("this app", |a| a.name.as_str());
                window("uninstall").show(ctx, |ui| {
                    ui.set_width(480.0);
                    head(ui, ph::TRASH, k::DANGER, k::DANGER_BG, &format!("Uninstall {name}?"));
                    ui.label(RichText::new("Windows runs the app's own uninstaller. Follow its window. When it finishes, LogCleaner lists the folders it left behind so you can delete them too.").color(k::FG2));
                    buttons(ui, acts, &format!("{}  Uninstall", ph::TRASH));
                });
            }
            Some(Confirm::RemoveEntry(i)) => {
                let name = self.apps.get(*i).map_or("this app", |a| a.name.as_str());
                window("entry").show(ctx, |ui| {
                    ui.set_width(480.0);
                    head(ui, ph::LIST_DASHES, k::REVIEW, k::REVIEW_BG, &format!("Remove {name} from the list?"));
                    ui.label(RichText::new("Its uninstaller is missing, so this only deletes its entry in Windows' installed programs list. Delete its folders afterwards from the panel on the right.").color(k::FG2));
                    buttons(ui, acts, "Remove entry");
                });
            }
            None => {}
        }

        if let Some(p) = &self.clean_prog {
            window("cleaning").show(ctx, |ui| {
                ui.set_width(460.0);
                let done = p.done.load(Relaxed);
                let total = p.total.load(Relaxed).max(1);
                head(ui, ph::BROOM, k::ACC, k::ACC_BG, &format!("Cleaning... {} freed", fmt_size(p.freed.load(Relaxed))));
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
                k::progress(ui.painter(), r, done as f32 / total as f32);
                ui.label(RichText::new(format!("{done} of {total} items  \u{00B7}  {} files  \u{00B7}  {} failed", fmt_count(p.files.load(Relaxed)), fmt_count(p.failed.load(Relaxed)))).color(k::FG2));
                let cur = p.current.lock().map(|c| c.clone()).unwrap_or_default();
                let g = k::galley(ui, cur, k::mono(12.0), k::MUT, 460.0);
                ui.label(g);
                ui.add_space(8.0);
                if k::button(ui, &format!("{}  Stop", ph::STOP), Look::Secondary, true).clicked() {
                    acts.push(Act::StopClean);
                }
            });
        }

        if let Some((i, p, _)) = &self.uninstall {
            let name = self.apps.get(*i).map_or("app", |a| a.name.as_str());
            window("uninstalling").show(ctx, |ui| {
                ui.set_width(460.0);
                head(ui, ph::APP_WINDOW, k::ACC, k::ACC_BG, &format!("Uninstalling {name}"));
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
                let t = (ui.input(|i| i.time) as f32 * 0.5).fract();
                ui.painter().rect_filled(r, 5.0, k::PANE);
                ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + r.width() * 0.75 * t, r.top()), vec2(r.width() * 0.25, r.height())), 5.0, k::ACC);
                let stage = p.stage.lock().map(|s| s.clone()).unwrap_or_default();
                ui.label(RichText::new(stage).color(k::FG2));
                ui.add_space(8.0);
                if k::button(ui, "Stop waiting", Look::Secondary, true).clicked() {
                    acts.push(Act::StopUninstall);
                }
            });
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
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
    Plan { items, maint, policy, bytes, caution, blocked, warn, app }
}

fn short(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let tail: String = s.chars().rev().take(max - 3).collect::<Vec<_>>().into_iter().rev().collect();
        format!("...{tail}")
    }
}

/// Page title row: icon, title, meta text, description, and right-aligned actions.
fn page_header(ui: &mut Ui, icon: &str, title: &str, meta: &str, desc: &str, actions: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    // Actions are laid out first from the right, so the title gets only the room that is left.
    ui.allocate_ui_with_layout(vec2(w, 48.0), Layout::right_to_left(Align::Center), |ui| {
        ui.set_height(48.0);
        actions(ui);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            k::icon_tile(ui, icon, 44.0, k::ACC, k::ACC_BG);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let tw = ui.available_width();
                ui.label(k::galley(ui, title, k::bold(21.0), k::FG, tw));
                if !meta.is_empty() {
                    ui.label(k::galley(ui, meta, k::body(14.0), k::MUT, tw));
                }
            });
        });
    });
    if !desc.is_empty() {
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(vec2(w.min(820.0), 0.0), Layout::top_down(Align::Min), |ui| {
            ui.add(egui::Label::new(RichText::new(desc).font(k::body(14.0)).color(k::MUT)).wrap());
        });
    }
    ui.add_space(18.0);
}

fn empty_state(ui: &mut Ui, icon: &str, title: &str, text: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
        ui.painter().rect_filled(r, k::R_CARD, k::PANE);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, k::body(26.0), k::MUT);
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(k::semi(16.0)).color(k::FG));
        ui.label(RichText::new(text).color(k::MUT));
    });
}

fn empty_detail(ui: &mut Ui, icon: &str, title: &str, text: &str) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
        ui.painter().rect_filled(r, k::R_CARD, k::ACC_BG);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, k::body(24.0), k::ACC);
        ui.add_space(4.0);
        ui.label(RichText::new(title).font(k::semi(16.0)).color(k::FG));
        ui.add(egui::Label::new(RichText::new(text).color(k::MUT)).wrap());
    });
}

fn header_sort(clicked: Option<usize>, map: &[(usize, Sort)], acts: &mut Vec<Act>) {
    if let Some(c) = clicked {
        if let Some((_, s)) = map.iter().find(|(i, _)| *i == c) {
            acts.push(Act::Sort(*s));
        }
    }
}

fn option_row(ui: &mut Ui, on: bool, enabled: bool, title: &str, hint: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        clicked = k::checkbox(ui, if on { Check::On } else { Check::Off }, enabled).clicked();
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(k::semi(14.0)).color(if enabled { k::FG } else { k::MUT }));
            ui.add(egui::Label::new(RichText::new(hint).font(k::body(13.0)).color(k::MUT)).wrap());
        });
    });
    ui.add_space(6.0);
    clicked
}

/// One leftover in the app panel; returns true when its checkbox was clicked.
fn leftover_row(ui: &mut Ui, l: &Leftover) -> bool {
    let w = ui.available_width();
    let h = if l.safety == Safety::Safe { 52.0 } else { 68.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let p = ui.painter().clone();
    p.rect(r.shrink2(vec2(0.0, 2.0)), k::R_CTL, if resp.hovered() { k::ROW_HOVER } else { k::SURFACE }, Stroke::new(1.0_f32, k::LINE), egui::StrokeKind::Inside);
    let enabled = l.safety != Safety::Protected;
    let cb = Rect::from_min_size(pos2(r.left() + 10.0, r.top() + 12.0), vec2(18.0, 18.0));
    let cr = ui.interact(cb.expand(6.0), ui.id().with(("left", &l.path)), if enabled { Sense::click() } else { Sense::hover() });
    k::paint_checkbox(&p, cb, if l.selected { Check::On } else { Check::Off }, enabled, cr.hovered());
    let x = r.left() + 38.0;
    p.text(pos2(x, r.top() + 21.0), Align2::LEFT_CENTER, l.what, k::semi(14.0), k::FG);
    p.text(pos2(r.right() - 10.0, r.top() + 21.0), Align2::RIGHT_CENTER, fmt_size(l.size), k::semi(14.0), k::FG);
    let g = k::galley(ui, l.path.display().to_string(), k::mono(12.0), k::MUT, r.right() - x - 10.0);
    p.galley(pos2(x, r.top() + 31.0), g, k::MUT);
    if l.safety != Safety::Safe {
        k::pill(ui, &p, pos2(x, r.top() + 54.0), l.note, k::safety_color(l.safety), k::safety_bg(l.safety));
    }
    resp.on_hover_text(format!("{}\n{} files\n{}", l.path.display(), fmt_count(l.files), l.note));
    cr.clicked() && enabled
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.poll(ctx);
        let msg = self.message.as_ref().map(|m| m.0.clone());
        if msg != self.toast_text {
            self.toast_text = msg;
            self.toast_since = Instant::now();
        }
        let mut acts = Vec::new();
        self.keyboard(ctx, &mut acts);
        // Opening the app list selects the first app so the detail panel is never empty.
        if self.view == View::Apps && self.focus_app.is_none() {
            if let Some(&first) = self.visible_apps().first() {
                acts.push(Act::FocusApp(first));
            }
        }
        let enabled = !self.modal();

        egui::Panel::top("top")
            .frame(egui::Frame::NONE.fill(k::SURFACE).inner_margin(Margin::symmetric(20, 10)))
            .show(ui, |ui| {
                ui.add_enabled_ui(enabled, |ui| self.top_bar(ui, &mut acts));
            });
        egui::Panel::left("nav")
            .exact_size(248.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(k::SURFACE).inner_margin(Margin { left: 12, right: 12, top: 12, bottom: 12 }))
            .show(ui, |ui| {
                ui.add_enabled_ui(enabled, |ui| self.sidebar(ui, &mut acts));
            });
        // Home and the report use the full width; list pages get a details panel on the right.
        let scanning = self.scan_prog.is_some() && self.view != View::Apps;
        if !scanning && !matches!(self.view, View::Home | View::Report) {
            egui::Panel::right("detail")
                .exact_size((ctx.content_rect().width() * 0.25).clamp(300.0, 380.0))
                .resizable(false)
                .frame(egui::Frame::NONE.fill(k::SURFACE).inner_margin(Margin::same(20)))
                .show(ui, |ui| {
                    ui.add_enabled_ui(enabled, |ui| self.detail(ui, &mut acts));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(k::BG).inner_margin(Margin { left: 28, right: 28, top: 24, bottom: 20 }))
            .show(ui, |ui| {
                ui.add_enabled_ui(enabled, |ui| self.page(ui, &mut acts));
            });
        if self.modal() {
            // Painted last on the root layer, so it dims the panels but stays under the modal window.
            ui.painter().rect_filled(ctx.content_rect(), 0.0, Color32::from_rgba_unmultiplied(15, 23, 42, 110));
        }
        self.modals(ctx, &mut acts);
        self.toast(ctx, &mut acts);
        for a in acts {
            self.apply(a);
        }
    }
}
