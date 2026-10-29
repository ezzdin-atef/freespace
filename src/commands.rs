//! Tauri commands: the only surface the web UI can call. Keep them thin wrappers over the backend modules.
//!
//! Long jobs (scan, clean, apps list, leftovers, uninstall) run on the existing `start(...) -> Receiver`
//! threads. A pump thread waits for each result, emitting a progress event every 100 ms meanwhile, then
//! stores the result in the `Session` and emits a `...:done` / `...:changed` event. The UI reads state with
//! the query commands, so it never depends on catching a particular event.

use crate::apps::{self, AppsMsg};
use crate::cleaner::{self, CleanProgress, CleanReport, Maintenance};
use crate::rules::{Category, DeleteMode, ItemKind, Safety};
use crate::scanner::{self, Finding, Progress};
use crate::state::{self, CleanJob, LeftoverSet, Plan, ScanJob, ScanState, Session, BUSY};
use crate::{safety, win};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

pub type Shared = Mutex<Session>;
type Res<T> = Result<T, String>;

const TICK: Duration = Duration::from_millis(100);

fn lock(m: &Shared) -> MutexGuard<'_, Session> {
    // A panic in one command must not wedge the whole UI.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn lock_app(app: &AppHandle) -> MutexGuard<'_, Session> {
    lock(app.state::<Shared>().inner())
}

/// Waits for a job's result on a new thread, emitting `progress()` as `event` every tick until it arrives.
fn pump<T, P>(
    app: AppHandle,
    rx: Receiver<T>,
    event: &'static str,
    progress: impl Fn() -> P + Send + 'static,
    done: impl FnOnce(&AppHandle, Option<T>) + Send + 'static,
) where
    T: Send + 'static,
    P: Serialize + Clone,
{
    std::thread::spawn(move || loop {
        match rx.recv_timeout(TICK) {
            Ok(v) => return done(&app, Some(v)),
            Err(RecvTimeoutError::Timeout) => {
                let _ = app.emit(event, progress());
            }
            Err(RecvTimeoutError::Disconnected) => return done(&app, None),
        }
    });
}

fn ms(d: Duration) -> u64 {
    d.as_millis() as u64
}

// ------------------------------------------------------------------ shared DTOs

fn category_key(c: Category) -> String {
    format!("{c:?}")
}

fn safety_key(s: Safety) -> &'static str {
    match s {
        Safety::Safe => "safe",
        Safety::Caution => "review",
        Safety::Protected => "protected",
    }
}

fn kind_key(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Dir => "dir",
        ItemKind::File => "file",
        ItemKind::RecycleBin => "recycle_bin",
    }
}

fn path_str(p: &Path) -> String {
    p.display().to_string()
}

#[derive(Serialize, Clone)]
pub struct Drive {
    pub root: String,
    pub kind: &'static str,
    pub total: u64,
    pub free: u64,
}

fn drives() -> Vec<Drive> {
    win::drives()
        .into_iter()
        .map(|d| Drive { root: d.root, kind: d.kind, total: d.total, free: d.free })
        .collect()
}

#[derive(Serialize, Clone)]
pub struct PathReason {
    pub path: String,
    pub reason: String,
}

// ------------------------------------------------------------------ startup and status

#[derive(Serialize)]
pub struct Init {
    pub drives: Vec<Drive>,
    pub is_admin: bool,
    /// `logcleaner.exe D:\projects` scans that folder right away.
    pub scan_on_start: Option<String>,
    /// `logcleaner.exe --apps` opens the installed apps page.
    pub open_apps: bool,
}

/// Everything the UI needs on first paint.
#[tauri::command]
pub fn init() -> Init {
    let arg = std::env::args().nth(1).filter(|a| !a.trim().is_empty());
    let open_apps = arg.as_deref() == Some("--apps");
    Init {
        drives: drives(),
        is_admin: win::is_admin(),
        scan_on_start: arg.filter(|_| !open_apps),
        open_apps,
    }
}

#[derive(Serialize)]
pub struct Status {
    pub scan: Option<ScanSummary>,
    pub scanning: Option<ScanProgress>,
    pub cleaning: Option<CleanProgressDto>,
    pub uninstalling: Option<UninstallProgressDto>,
    pub has_report: bool,
}

/// Snapshot of everything running or finished, for (re)building the UI from scratch.
#[tauri::command]
pub fn status(state: State<'_, Shared>) -> Status {
    let s = lock(&state);
    Status {
        scan: s.scan.as_ref().map(scan_summary),
        scanning: s.scan_job.as_ref().map(|j| scan_progress(&j.progress, j.id, &j.root, j.started, j.expected)),
        cleaning: s.clean_job.as_ref().map(|j| clean_progress(&j.progress, j.started, j.app)),
        uninstalling: s.uninstall_job.as_ref().map(|(i, p)| uninstall_progress(*i, p)),
        has_report: s.report.is_some(),
    }
}

// ------------------------------------------------------------------ scan

#[derive(Serialize, Clone)]
pub struct ScanProgress {
    pub scan_id: u64,
    pub root: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub current: String,
    pub elapsed_ms: u64,
    /// Used bytes of the drive being scanned, when the root is a whole drive.
    pub expected: Option<u64>,
}

fn scan_progress(p: &Progress, scan_id: u64, root: &str, started: Instant, expected: Option<u64>) -> ScanProgress {
    ScanProgress {
        scan_id,
        root: root.to_string(),
        files: p.files.load(Relaxed),
        dirs: p.dirs.load(Relaxed),
        bytes: p.bytes.load(Relaxed),
        errors: p.errors.load(Relaxed),
        current: p.current.lock().map(|c| c.clone()).unwrap_or_default(),
        elapsed_ms: ms(started.elapsed()),
        expected,
    }
}

#[derive(Serialize, Clone)]
pub struct ScanSummary {
    pub scan_id: u64,
    pub root: String,
    pub files: u64,
    pub dirs: u64,
    pub errors: u64,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

fn scan_summary(s: &ScanState) -> ScanSummary {
    let r = &s.result;
    ScanSummary {
        scan_id: s.id,
        root: path_str(&r.root),
        files: r.files,
        dirs: r.dirs,
        errors: r.errors,
        elapsed_ms: ms(r.elapsed),
        cancelled: r.cancelled,
    }
}

#[derive(Serialize, Clone)]
pub struct ScanDone {
    pub scan_id: u64,
    /// None when the scan thread died.
    pub summary: Option<ScanSummary>,
}

/// Accepts "C:", "C:\", quoted paths and forward slashes.
fn normalize_root(input: &str) -> Res<PathBuf> {
    let mut root = input.trim().trim_matches('"').replace('/', "\\");
    if root.len() == 2 && root.ends_with(':') {
        root.push('\\');
    }
    let path = PathBuf::from(&root);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!("'{root}' is not a folder or drive."))
    }
}

/// Starts scanning `root`; returns the new scan id. Events: `scan:progress`, `scan:done`.
#[tauri::command]
pub fn start_scan(app: AppHandle, state: State<'_, Shared>, root: String) -> Res<u64> {
    let path = normalize_root(&root)?;
    let mut s = lock(&state);
    if s.busy() {
        return Err(BUSY.into());
    }
    let root = path_str(&path);
    let expected = win::drives()
        .into_iter()
        .find(|d| d.root.eq_ignore_ascii_case(&root))
        .map(|d| d.total.saturating_sub(d.free));
    let id = s.next_id();
    let progress = Arc::new(Progress::default());
    let started = Instant::now();
    let rx = scanner::start(path, progress.clone());
    s.scan = None;
    s.scan_job = Some(ScanJob { id, root: root.clone(), progress: progress.clone(), started, expected });
    drop(s);

    pump(app, rx, "scan:progress", move || scan_progress(&progress, id, &root, started, expected), move |app, result| {
        let mut s = lock_app(app);
        if s.scan_job.as_ref().is_some_and(|j| j.id == id) {
            s.scan_job = None;
        }
        let summary = result.map(|r| {
            let st = ScanState::new(id, r);
            let sum = scan_summary(&st);
            s.scan = Some(st);
            sum
        });
        drop(s);
        let _ = app.emit("scan:done", ScanDone { scan_id: id, summary });
    });
    Ok(id)
}

#[tauri::command]
pub fn stop_scan(state: State<'_, Shared>) {
    if let Some(j) = &lock(&state).scan_job {
        j.progress.cancel.store(true, Relaxed);
    }
}

// ------------------------------------------------------------------ results

#[derive(Serialize)]
pub struct Group {
    pub category: String,
    pub label: &'static str,
    pub description: &'static str,
    pub restore: &'static str,
    pub total: u64,
    pub count: usize,
}

/// Per-category totals of what is left to clean, largest first.
#[tauri::command]
pub fn groups(state: State<'_, Shared>, scan_id: u64) -> Res<Vec<Group>> {
    let s = lock(&state);
    let scan = s.scan_for(scan_id)?;
    let mut map: std::collections::BTreeMap<Category, (u64, usize)> = Default::default();
    for (_, f) in scan.live() {
        let e = map.entry(f.category).or_default();
        e.0 += f.size;
        e.1 += 1;
    }
    let mut out: Vec<Group> = map
        .into_iter()
        .map(|(c, (total, count))| Group {
            category: category_key(c),
            label: c.label(),
            description: c.description(),
            restore: c.restore_hint(),
            total,
            count,
        })
        .collect();
    out.sort_by(|a, b| b.total.cmp(&a.total));
    Ok(out)
}

#[derive(Serialize)]
pub struct FindingRow {
    pub id: usize,
    pub category: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub safety: &'static str,
    pub note: String,
    pub kind: &'static str,
    /// "contents" empties the folder and keeps it; "whole" deletes the item.
    pub mode: &'static str,
}

fn finding_row(id: usize, f: &Finding) -> FindingRow {
    let path = match f.kind {
        ItemKind::RecycleBin => format!("Recycle Bin on {}", f.path.display()),
        _ => path_str(&f.path),
    };
    let name = f.path.file_name().map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    FindingRow {
        id,
        category: category_key(f.category),
        path,
        name,
        size: f.size,
        files: f.files,
        safety: safety_key(f.safety),
        note: f.note.clone(),
        kind: kind_key(f.kind),
        mode: match f.mode {
            DeleteMode::Contents => "contents",
            DeleteMode::Whole => "whole",
        },
    }
}

/// Every finding not cleaned yet. Filtering and sorting happen in the UI.
#[tauri::command]
pub fn findings(state: State<'_, Shared>, scan_id: u64) -> Res<Vec<FindingRow>> {
    let s = lock(&state);
    Ok(s.scan_for(scan_id)?.live().map(|(i, f)| finding_row(i, f)).collect())
}

/// Ids "Full clean" ticks: everything safe plus Windows rollback/debug leftovers.
#[tauri::command]
pub fn full_clean_ids(state: State<'_, Shared>, scan_id: u64) -> Res<Vec<usize>> {
    let s = lock(&state);
    Ok(s.scan_for(scan_id)?.live().filter(|(_, f)| state::full_clean_pick(f)).map(|(i, _)| i).collect())
}

#[derive(Serialize)]
pub struct UsageRow {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub has_children: bool,
    /// Category label when the whole folder is a cleanable finding.
    pub tag: Option<&'static str>,
    pub safety: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
pub struct UsageLevel {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub children: Vec<UsageRow>,
}

/// One level of the space-usage tree (the scan root when `path` is None). Never sends the whole tree.
#[tauri::command]
pub fn usage(state: State<'_, Shared>, scan_id: u64, path: Option<String>) -> Res<UsageLevel> {
    let s = lock(&state);
    let tree = &s.scan_for(scan_id)?.result.tree;
    let node = match &path {
        Some(p) => state::find_node(tree, Path::new(p)).ok_or_else(|| state::STALE_SCAN.to_string())?,
        None => tree,
    };
    let children = node
        .children
        .iter()
        .map(|c| {
            let (safety, note) = safety::assess(&c.path);
            UsageRow {
                name: c.name.clone(),
                path: path_str(&c.path),
                size: c.size,
                files: c.files,
                has_children: !c.children.is_empty(),
                tag: c.tag.map(Category::label),
                safety: safety_key(safety),
                note,
            }
        })
        .collect();
    Ok(UsageLevel { name: node.name.clone(), path: path_str(&node.path), size: node.size, files: node.files, children })
}

// ------------------------------------------------------------------ clean

#[derive(Deserialize)]
pub struct CleanRequest {
    pub scan_id: u64,
    #[serde(default)]
    pub ids: Vec<usize>,
    /// Folders ticked in Space usage; each must exist in the scan tree.
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub dism: bool,
    #[serde(default)]
    pub hibernate_off: bool,
    #[serde(default)]
    pub use_trash: bool,
}

#[derive(Serialize)]
pub struct PlanDto {
    pub count: usize,
    pub bytes: u64,
    pub caution: Vec<String>,
    pub blocked: Vec<PathReason>,
    pub warn: Option<String>,
    pub dism: bool,
    pub hibernate_off: bool,
}

fn plan_dto(p: &Plan) -> PlanDto {
    PlanDto {
        count: p.items.len(),
        bytes: p.bytes,
        caution: p.caution.clone(),
        blocked: p.blocked.iter().map(|(path, r)| PathReason { path: path_str(path), reason: r.to_string() }).collect(),
        warn: p.warn.clone(),
        dism: p.maint.dism,
        hibernate_off: p.maint.hibernate_off,
    }
}

fn request_plan(s: &Session, req: &CleanRequest) -> Res<Plan> {
    let scan = s.scan_for(req.scan_id)?;
    let folders: Vec<PathBuf> = req.folders.iter().map(PathBuf::from).collect();
    let maint = Maintenance { dism: req.dism, hibernate_off: req.hibernate_off };
    state::clean_plan(scan, &req.ids, &folders, maint)
}

/// What `clean` would do with this request, for the confirm dialog.
#[tauri::command]
pub fn plan_clean(state: State<'_, Shared>, req: CleanRequest) -> Res<PlanDto> {
    Ok(plan_dto(&request_plan(&lock(&state), &req)?))
}

#[derive(Serialize, Clone)]
pub struct CleanProgressDto {
    pub done: u64,
    pub total: u64,
    pub freed: u64,
    pub files: u64,
    pub failed: u64,
    pub current: String,
    pub elapsed_ms: u64,
    pub app_id: Option<usize>,
}

fn clean_progress(p: &CleanProgress, started: Instant, app_id: Option<usize>) -> CleanProgressDto {
    CleanProgressDto {
        done: p.done.load(Relaxed),
        total: p.total.load(Relaxed),
        freed: p.freed.load(Relaxed),
        files: p.files.load(Relaxed),
        failed: p.failed.load(Relaxed),
        current: p.current.lock().map(|c| c.clone()).unwrap_or_default(),
        elapsed_ms: ms(started.elapsed()),
        app_id,
    }
}

#[derive(Serialize, Clone)]
pub struct Report {
    pub freed: u64,
    pub files: u64,
    pub failed: u64,
    pub failures: Vec<PathReason>,
    pub skipped: Vec<PathReason>,
    pub maintenance: Vec<String>,
    pub processed: usize,
    pub elapsed_ms: u64,
    pub used_trash: bool,
    pub cancelled: bool,
}

fn report_dto(r: &CleanReport) -> Report {
    let pairs = |v: &[(String, String)]| v.iter().map(|(p, r)| PathReason { path: p.clone(), reason: r.clone() }).collect();
    Report {
        freed: r.freed,
        files: r.files,
        failed: r.failed,
        failures: pairs(&r.failures),
        skipped: pairs(&r.skipped),
        maintenance: r.maintenance.clone(),
        processed: r.processed.len(),
        elapsed_ms: ms(r.elapsed),
        used_trash: r.used_trash,
        cancelled: r.cancelled,
    }
}

#[derive(Serialize, Clone)]
pub struct CleanDone {
    /// None when the clean thread died.
    pub report: Option<Report>,
    pub app_id: Option<usize>,
    pub drives: Vec<Drive>,
}

fn start_clean(app: AppHandle, s: &mut Session, plan: Plan, use_trash: bool) -> Res<()> {
    if s.busy() {
        return Err(BUSY.into());
    }
    if plan.items.is_empty() && !plan.maint.dism && !plan.maint.hibernate_off {
        return Err("Nothing to clean in this selection.".into());
    }
    let progress = Arc::new(CleanProgress::default());
    let started = Instant::now();
    let app_id = plan.app;
    let scan_id = s.scan.as_ref().map(|sc| sc.id);
    let rx = cleaner::start(plan.items, use_trash, plan.maint, plan.policy, progress.clone());
    s.clean_job = Some(CleanJob { progress: progress.clone(), started, app: app_id });

    pump(app, rx, "clean:progress", move || clean_progress(&progress, started, app_id), move |app, result| {
        let mut s = lock_app(app);
        s.clean_job = None;
        if let Some(r) = &result {
            if let Some(scan) = s.scan.as_mut().filter(|sc| Some(sc.id) == scan_id) {
                scan.mark_cleaned(&r.processed);
            }
        }
        let report = result.map(|r| {
            let dto = report_dto(&r);
            s.report = Some(r);
            dto
        });
        if let Some(i) = app_id {
            request_leftovers(app, &mut s, i);
        }
        drop(s);
        let _ = app.emit("clean:done", CleanDone { report, app_id, drives: drives() });
    });
    Ok(())
}

/// Starts cleaning. Events: `clean:progress`, `clean:done`.
#[tauri::command]
pub fn clean(app: AppHandle, state: State<'_, Shared>, req: CleanRequest) -> Res<()> {
    let mut s = lock(&state);
    let plan = request_plan(&s, &req)?;
    start_clean(app, &mut s, plan, req.use_trash)
}

#[tauri::command]
pub fn stop_clean(state: State<'_, Shared>) {
    if let Some(j) = &lock(&state).clean_job {
        j.progress.cancel.store(true, Relaxed);
    }
}

/// The last clean's report.
#[tauri::command]
pub fn report(state: State<'_, Shared>) -> Option<Report> {
    lock(&state).report.as_ref().map(report_dto)
}

// ------------------------------------------------------------------ installed apps

/// Loads the installed apps list in the background, once, at startup. Event: `apps:changed`.
pub fn start_apps_load(app: &AppHandle) {
    lock_app(app).apps_loading = true;
    let rx = apps::load();
    let app = app.clone();
    std::thread::spawn(move || {
        for msg in rx {
            let mut s = lock_app(&app);
            match msg {
                AppsMsg::List(list) => s.apps = list,
                AppsMsg::Sizes(sizes) => {
                    for (i, size) in sizes {
                        if let Some(a) = s.apps.get_mut(i) {
                            a.size = size;
                        }
                    }
                }
            }
            drop(s);
            let _ = app.emit("apps:changed", ());
        }
        lock_app(&app).apps_loading = false;
        let _ = app.emit("apps:changed", ());
    });
}

#[derive(Serialize)]
pub struct AppRow {
    pub id: usize,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub installed: String,
    pub location: Option<String>,
    pub size: u64,
    pub store: bool,
    pub has_uninstaller: bool,
    pub removed: bool,
}

#[derive(Serialize)]
pub struct AppsList {
    pub loading: bool,
    pub items: Vec<AppRow>,
}

#[tauri::command]
pub fn apps(state: State<'_, Shared>) -> AppsList {
    let s = lock(&state);
    let items = s
        .apps
        .iter()
        .enumerate()
        .map(|(id, a)| AppRow {
            id,
            name: a.name.clone(),
            publisher: a.publisher.clone(),
            version: a.version.clone(),
            installed: a.installed.clone(),
            location: a.location.as_deref().map(path_str),
            size: a.size,
            store: a.is_store(),
            has_uninstaller: a.has_uninstaller(),
            removed: s.removed.contains(&id),
        })
        .collect();
    AppsList { loading: s.apps_loading, items }
}

#[derive(Serialize, Clone)]
pub struct AppEvent {
    pub app_id: usize,
}

/// (Re)scans one app's leftovers in the background. Event: `leftovers:changed`.
fn request_leftovers(app: &AppHandle, s: &mut Session, app_id: usize) {
    let Some(target) = s.apps.get(app_id).cloned() else { return };
    s.leftovers_loading.insert(app_id);
    let rx = apps::start_leftovers(target, s.apps.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        let items = rx.recv().ok();
        let mut s = lock_app(&app);
        s.leftovers_loading.remove(&app_id);
        if let Some(items) = items {
            let gen = s.next_id();
            s.leftovers.insert(app_id, LeftoverSet { gen, items });
        }
        drop(s);
        let _ = app.emit("leftovers:changed", AppEvent { app_id });
    });
}

#[tauri::command]
pub fn scan_leftovers(app: AppHandle, state: State<'_, Shared>, app_id: usize) -> Res<()> {
    let mut s = lock(&state);
    s.app(app_id)?;
    request_leftovers(&app, &mut s, app_id);
    Ok(())
}

#[derive(Serialize)]
pub struct LeftoverRow {
    pub id: usize,
    pub path: String,
    pub what: &'static str,
    pub size: u64,
    pub files: u64,
    pub safety: &'static str,
    pub note: &'static str,
    /// Ticked by default.
    pub selected: bool,
}

#[derive(Serialize)]
pub struct Leftovers {
    pub loading: bool,
    /// Generation of `items`; pass it back with ids so a rescan in between is detected.
    pub gen: Option<u64>,
    pub items: Vec<LeftoverRow>,
}

#[tauri::command]
pub fn leftovers(state: State<'_, Shared>, app_id: usize) -> Leftovers {
    let s = lock(&state);
    let set = s.leftovers.get(&app_id);
    Leftovers {
        loading: s.leftovers_loading.contains(&app_id),
        gen: set.map(|l| l.gen),
        items: set
            .map(|l| {
                l.items
                    .iter()
                    .enumerate()
                    .map(|(id, l)| LeftoverRow {
                        id,
                        path: path_str(&l.path),
                        what: l.what,
                        size: l.size,
                        files: l.files,
                        safety: safety_key(l.safety),
                        note: l.note,
                        selected: l.selected,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[tauri::command]
pub fn plan_leftovers(state: State<'_, Shared>, app_id: usize, gen: u64, ids: Vec<usize>) -> Res<PlanDto> {
    Ok(plan_dto(&state::leftover_plan(&lock(&state), app_id, gen, &ids)?))
}

/// Deletes ticked leftovers. Events: `clean:progress`, `clean:done` (with `app_id`), then `leftovers:changed`.
#[tauri::command]
pub fn delete_leftovers(
    app: AppHandle,
    state: State<'_, Shared>,
    app_id: usize,
    gen: u64,
    ids: Vec<usize>,
    use_trash: bool,
) -> Res<()> {
    let mut s = lock(&state);
    let plan = state::leftover_plan(&s, app_id, gen, &ids)?;
    start_clean(app, &mut s, plan, use_trash)
}

#[derive(Serialize, Clone)]
pub struct UninstallProgressDto {
    pub app_id: usize,
    pub stage: String,
}

fn uninstall_progress(app_id: usize, p: &apps::UninstallProgress) -> UninstallProgressDto {
    UninstallProgressDto { app_id, stage: p.stage.lock().map(|s| s.clone()).unwrap_or_default() }
}

#[derive(Serialize, Clone)]
pub struct UninstallDone {
    pub app_id: usize,
    pub removed: bool,
    pub message: String,
}

/// Runs the app's uninstaller. Events: `uninstall:progress`, `uninstall:done`, then `leftovers:changed`.
#[tauri::command]
pub fn uninstall(app: AppHandle, state: State<'_, Shared>, app_id: usize) -> Res<()> {
    let mut s = lock(&state);
    if s.busy() {
        return Err(BUSY.into());
    }
    let target = s.app(app_id)?.clone();
    let progress = Arc::new(apps::UninstallProgress::default());
    let rx = apps::start_uninstall(target, progress.clone());
    s.uninstall_job = Some((app_id, progress.clone()));
    drop(s);

    pump(app, rx, "uninstall:progress", move || uninstall_progress(app_id, &progress), move |app, outcome| {
        let mut s = lock_app(app);
        s.uninstall_job = None;
        let (removed, message) = match outcome {
            Some(o) => (o.removed, o.message),
            None => (false, "The uninstall stopped unexpectedly.".into()),
        };
        if removed {
            s.removed.insert(app_id);
        }
        request_leftovers(app, &mut s, app_id);
        drop(s);
        let _ = app.emit("uninstall:done", UninstallDone { app_id, removed, message });
    });
    Ok(())
}

#[tauri::command]
pub fn stop_uninstall(state: State<'_, Shared>) {
    if let Some((_, p)) = &lock(&state).uninstall_job {
        p.cancel.store(true, Relaxed);
    }
}

/// Removes a broken entry from Windows' installed list; returns the message to show.
#[tauri::command]
pub fn remove_entry(state: State<'_, Shared>, app_id: usize) -> Res<String> {
    let mut s = lock(&state);
    let target = s.app(app_id)?;
    apps::remove_entry(target)?;
    let msg = format!("Removed {} from the installed list.", target.name);
    s.removed.insert(app_id);
    Ok(msg)
}

// ------------------------------------------------------------------ system

/// Shows the item in Explorer. Read-only, so any path is fine.
#[tauri::command]
pub fn reveal(path: String) {
    win::reveal(Path::new(&path));
}

/// Restarts elevated (UAC prompt); this process exits when the elevated one starts.
#[tauri::command]
pub fn relaunch_admin(app: AppHandle) -> bool {
    let ok = win::relaunch_as_admin();
    if ok {
        app.exit(0);
    }
    ok
}

#[tauri::command]
pub fn reset_pc() {
    win::open_reset_this_pc();
}
