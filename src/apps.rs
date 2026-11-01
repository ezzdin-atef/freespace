//! Installed apps: listing (registry + Store), leftover discovery and uninstalling.

use crate::rules::Safety;
use crate::{safety, win};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
use winreg::RegKey;

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hive {
    Machine,
    Machine32,
    User,
}

#[derive(Clone, Debug)]
pub enum Source {
    /// Classic desktop app with an entry under ...\CurrentVersion\Uninstall.
    Registry { hive: Hive, key: String, uninstall: Option<String>, msi: Option<String> },
    /// Microsoft Store / MSIX package.
    Store { full_name: String, family: String },
}

#[derive(Clone, Debug)]
pub struct InstalledApp {
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub installed: String,
    pub location: Option<PathBuf>,
    pub size: u64,
    /// The registry's DisplayIcon: an .exe/.dll/.ico path with an optional ",index". Store apps use their manifest.
    pub icon: Option<String>,
    pub source: Source,
}

impl InstalledApp {
    pub fn is_store(&self) -> bool {
        matches!(self.source, Source::Store { .. })
    }

    pub fn has_uninstaller(&self) -> bool {
        match &self.source {
            Source::Registry { uninstall, msi, .. } => uninstall.is_some() || msi.is_some(),
            Source::Store { .. } => true,
        }
    }
}

fn hive_key(h: Hive) -> (RegKey, u32) {
    match h {
        Hive::Machine => (RegKey::predef(HKEY_LOCAL_MACHINE), KEY_WOW64_64KEY),
        Hive::Machine32 => (RegKey::predef(HKEY_LOCAL_MACHINE), KEY_WOW64_32KEY),
        Hive::User => (RegKey::predef(HKEY_CURRENT_USER), 0),
    }
}

fn clean_path(s: &str) -> Option<PathBuf> {
    let s = s.trim().trim_matches('"').trim_end_matches('\\');
    (!s.is_empty()).then(|| PathBuf::from(s))
}

fn is_hotfix(name: &str) -> bool {
    let n = name.to_lowercase();
    (n.starts_with("kb") && n[2..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        || n.contains("security update for")
        || n.contains("update for microsoft")
        || n.starts_with("hotfix for")
}

pub fn list_registry() -> Vec<InstalledApp> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for hive in [Hive::Machine, Hive::Machine32, Hive::User] {
        let (root, view) = hive_key(hive);
        let Ok(base) = root.open_subkey_with_flags(UNINSTALL, KEY_READ | view) else { continue };
        for key_name in base.enum_keys().flatten() {
            let Ok(k) = base.open_subkey_with_flags(&key_name, KEY_READ | view) else { continue };
            let get = |v: &str| k.get_value::<String, _>(v).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            let dword = |v: &str| k.get_value::<u32, _>(v).ok();
            let Some(name) = get("DisplayName") else { continue };
            if dword("SystemComponent") == Some(1) || get("ParentKeyName").is_some() || is_hotfix(&name) {
                continue;
            }
            if get("ReleaseType").is_some_and(|r| {
                let r = r.to_lowercase();
                r.contains("update") || r.contains("hotfix")
            }) {
                continue;
            }
            let version = get("DisplayVersion").unwrap_or_default();
            if !seen.insert((name.to_lowercase(), version.clone())) {
                continue;
            }
            let msi = (dword("WindowsInstaller") == Some(1) && key_name.starts_with('{')).then(|| key_name.clone());
            let location = get("InstallLocation").and_then(|s| clean_path(&s)).filter(|p| p.is_dir()).or_else(|| {
                // Fall back to the folder of the app's icon/exe, unless that is a shared system folder.
                let icon = get("DisplayIcon")?;
                let exe = clean_path(icon.split(',').next()?)?;
                let dir = exe.parent()?.to_path_buf();
                (dir.is_dir() && safety::uninstall_blocked(&dir).is_none()).then_some(dir)
            });
            // Some apps register their bin\ folder (Android Studio, JetBrains tools); the app is its parent.
            let location = location.map(|l| match l.file_name() {
                Some(n) if n.eq_ignore_ascii_case("bin") => l.parent().map_or(l.clone(), Path::to_path_buf),
                _ => l,
            });
            let installed = get("InstallDate")
                .filter(|d| d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()))
                .map(|d| format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]))
                .unwrap_or_default();
            out.push(InstalledApp {
                name,
                publisher: get("Publisher").unwrap_or_default(),
                version,
                installed,
                location,
                size: dword("EstimatedSize").map_or(0, |kb| kb as u64 * 1024),
                icon: get("DisplayIcon"),
                source: Source::Registry {
                    hive,
                    key: format!("{UNINSTALL}\\{key_name}"),
                    uninstall: get("UninstallString"),
                    msi,
                },
            });
        }
    }
    out
}

/// "Microsoft.WindowsCalculator" -> "Windows Calculator"
pub fn pretty_store_name(name: &str) -> String {
    let last = name.rsplit('.').next().unwrap_or(name);
    let mut out = String::new();
    let chars: Vec<char> = last.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1);
        let boundary = c.is_uppercase()
            && prev.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit()
                || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase())));
        if boundary {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn cn(publisher: &str) -> String {
    publisher
        .split(',')
        .find_map(|p| p.trim().strip_prefix("CN="))
        .unwrap_or(publisher)
        .trim_matches('"')
        .to_string()
}

pub fn list_store() -> Vec<InstalledApp> {
    let script = "Get-AppxPackage | Where-Object { -not $_.IsFramework -and -not $_.NonRemovable -and -not $_.IsResourcePackage -and $_.SignatureKind -ne 'System' } | Select-Object Name,PackageFullName,PackageFamilyName,Publisher,Version,InstallLocation | ConvertTo-Json -Compress";
    let Ok(out) = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else { return Vec::new() };
    let items = match v {
        serde_json::Value::Array(a) => a,
        o @ serde_json::Value::Object(_) => vec![o],
        _ => Vec::new(),
    };
    let s = |o: &serde_json::Value, k: &str| o.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    items
        .iter()
        .filter_map(|o| {
            let full_name = s(o, "PackageFullName");
            (!full_name.is_empty()).then(|| InstalledApp {
                name: pretty_store_name(&s(o, "Name")),
                publisher: cn(&s(o, "Publisher")),
                version: s(o, "Version"),
                installed: String::new(),
                location: clean_path(&s(o, "InstallLocation")),
                size: 0,
                icon: None,
                source: Source::Store { full_name, family: s(o, "PackageFamilyName") },
            })
        })
        .collect()
}

pub enum AppsMsg {
    List(Vec<InstalledApp>),
    /// (index into the list, bytes) for apps whose size was not in the registry.
    Sizes(Vec<(usize, u64)>),
}

pub fn load() -> Receiver<AppsMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut apps = list_registry();
        apps.extend(list_store());
        apps.sort_by_key(|a| a.name.to_lowercase());
        let todo: Vec<(usize, PathBuf)> = apps
            .iter()
            .enumerate()
            .filter(|(_, a)| a.size == 0)
            .filter_map(|(i, a)| a.location.clone().map(|l| (i, l)))
            .collect();
        if tx.send(AppsMsg::List(apps)).is_err() {
            return;
        }
        let sizes = todo.into_par_iter().map(|(i, l)| (i, dir_size(&l).0)).collect();
        let _ = tx.send(AppsMsg::Sizes(sizes));
    });
    rx
}

/// (bytes, files) of a file or folder tree; links are not followed.
pub fn dir_size(p: &Path) -> (u64, u64) {
    let Ok(md) = fs::symlink_metadata(p) else { return (0, 0) };
    if md.file_type().is_symlink() {
        return (0, 0);
    }
    if !md.is_dir() {
        return (md.len(), 1);
    }
    let Ok(rd) = fs::read_dir(p) else { return (0, 0) };
    let entries: Vec<_> = rd.flatten().collect();
    entries
        .par_iter()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_symlink() => (0, 0),
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            _ => (e.metadata().map_or(0, |m| m.len()), 1),
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}

// ------------------------------------------------------------------ leftovers

#[derive(Clone, Debug)]
pub struct Leftover {
    pub path: PathBuf,
    pub what: &'static str,
    pub size: u64,
    pub files: u64,
    pub safety: Safety,
    pub note: &'static str,
    pub selected: bool,
}

/// Words too common to identify one app by themselves.
const GENERIC: &[&str] = &[
    "microsoft", "windows", "google", "update", "updater", "runtime", "redistributable", "visual", "driver",
    "drivers", "tools", "toolkit", "software", "service", "services", "system", "common", "files", "program",
    "programs", "application", "studio", "desktop", "client", "helper", "launcher", "installer", "setup",
    "development", "framework", "edition", "professional", "community", "standard", "enterprise", "plugin",
    "extension", "package", "manager", "player", "viewer", "editor", "assistant", "support", "utility",
    "version", "english", "language", "bundle", "engine", "center", "centre", "control", "panel", "library",
    "shared", "cloud", "online", "graphics", "audio", "sound", "video", "media", "security", "network",
    "wireless", "bluetooth", "device", "devices", "local", "x64", "x86", "64-bit", "32-bit", "preview", "insiders",
];

const PUBLISHER_SUFFIXES: &[&str] = &[
    "inc", "llc", "ltd", "corporation", "corp", "gmbh", "co", "limited", "software", "technologies", "sro",
    "ab", "ag", "sa", "bv", "plc", "foundation", "project",
];

fn compact(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

/// Drops "(x64 en-US)", version numbers and architecture tags from a display name.
fn clean_name(name: &str) -> String {
    let mut s = String::new();
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    s.split_whitespace()
        .filter(|t| {
            let l = t.to_lowercase();
            let versionish = l.starts_with(|c: char| c.is_ascii_digit())
                || (l.starts_with('v') && l[1..].starts_with(|c: char| c.is_ascii_digit()));
            !versionish && !matches!(l.as_str(), "x64" | "x86" | "amd64" | "arm64" | "64-bit" | "32-bit" | "-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

struct Keys {
    strong: HashSet<String>,
    weak: HashSet<String>,
    publisher: Option<String>,
}

fn keys(app: &InstalledApp) -> Keys {
    let cleaned = clean_name(&app.name);
    let pub_words: Vec<String> =
        words(&app.publisher).into_iter().filter(|w| !PUBLISHER_SUFFIXES.contains(&w.as_str())).collect();
    let publisher = Some(pub_words.concat()).filter(|p| p.len() >= 3);
    let mut strong = HashSet::new();
    strong.insert(compact(&cleaned));
    let without_pub: String = words(&cleaned).into_iter().filter(|w| !pub_words.contains(w)).collect();
    strong.insert(without_pub);
    if let Some(loc) = &app.location {
        if let Some(n) = loc.file_name() {
            strong.insert(compact(&n.to_string_lossy()));
        }
    }
    strong.retain(|k| k.len() >= 3 && !GENERIC.contains(&k.as_str()));
    let weak = words(&cleaned)
        .into_iter()
        .filter(|w| w.len() >= 5 && !GENERIC.contains(&w.as_str()) && Some(w) != publisher.as_ref() && !strong.contains(w))
        .collect();
    Keys { strong, weak, publisher }
}

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var(k).ok().map(PathBuf::from)
}

/// Finds folders and shortcuts that belong to `app`. `all` is used to skip folders of other apps.
pub fn find_leftovers(app: &InstalledApp, all: &[InstalledApp]) -> Vec<Leftover> {
    let other_locations: Vec<String> = all
        .iter()
        .filter(|a| !(a.name == app.name && a.version == app.version))
        .filter_map(|a| a.location.as_deref().map(safety::norm))
        .collect();
    let mut cands: Vec<(PathBuf, &'static str, Safety)> = Vec::new();
    // A Store app's install folder lives in WindowsApps and is removed by Windows itself.
    if let (Some(loc), false) = (&app.location, app.is_store()) {
        cands.push((loc.clone(), "Install folder", Safety::Safe));
    }
    let local = env_path("LOCALAPPDATA");
    let profile = env_path("USERPROFILE");
    match &app.source {
        Source::Store { family, .. } => {
            if let Some(l) = &local {
                cands.push((l.join("Packages").join(family), "App data", Safety::Safe));
            }
        }
        Source::Registry { .. } => {
            let k = keys(app);
            let pd = env_path("ProgramData");
            let bases: Vec<(Option<PathBuf>, &'static str)> = vec![
                (env_path("ProgramFiles"), "Program folder"),
                (env_path("ProgramFiles(x86)"), "Program folder"),
                (pd.clone(), "Shared app data"),
                (env_path("APPDATA"), "App data (Roaming)"),
                (local.clone(), "App data (Local)"),
                (local.as_ref().map(|l| l.join("Programs")), "Program folder (user)"),
                (profile.as_ref().map(|p| p.join("AppData\\LocalLow")), "App data (LocalLow)"),
                (env_path("APPDATA").map(|a| a.join("Microsoft\\Windows\\Start Menu\\Programs")), "Start menu"),
                (pd.map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs")), "Start menu (all users)"),
                (profile.as_ref().map(|p| p.join("Desktop")), "Desktop shortcut"),
                (env_path("PUBLIC").map(|p| p.join("Desktop")), "Desktop shortcut"),
            ];
            for (base, what) in bases {
                let Some(base) = base else { continue };
                let Ok(rd) = fs::read_dir(&base) else { continue };
                for e in rd.flatten() {
                    let path = e.path();
                    let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
                    let stem = match is_dir {
                        true => e.file_name().to_string_lossy().into_owned(),
                        false if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) => {
                            path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                        }
                        false => continue,
                    };
                    let c = compact(&clean_name(&stem));
                    if k.strong.contains(&c) {
                        cands.push((path, what, Safety::Safe));
                    } else if k.weak.contains(&c) {
                        cands.push((path, what, Safety::Caution));
                    } else if is_dir && k.publisher.as_deref() == Some(c.as_str()) {
                        // Vendor folder (e.g. Program Files\Mozilla): look one level deeper.
                        for child in fs::read_dir(&path).into_iter().flatten().flatten() {
                            let cc = compact(&clean_name(&child.file_name().to_string_lossy()));
                            if k.strong.contains(&cc) {
                                cands.push((child.path(), what, Safety::Safe));
                            } else if k.weak.contains(&cc) {
                                cands.push((child.path(), what, Safety::Caution));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut seen: Vec<String> = Vec::new();
    let mut picked: Vec<(PathBuf, &'static str, Safety, &'static str)> = Vec::new();
    // Exact matches first, so a loose match on a parent folder (Program Files\Android) never
    // swallows the real install folder inside it (Program Files\Android\Android Studio).
    cands.sort_by_key(|c| (c.2 != Safety::Safe, safety::norm(&c.0).len()));
    for (path, what, s) in cands {
        if !path.exists() {
            continue;
        }
        let key = safety::norm(&path);
        if seen.iter().any(|k| safety::is_within(&key, k) || safety::is_within(k, &key)) {
            continue;
        }
        // Never touch another installed app's folder, or a parent folder that contains one.
        if other_locations.iter().any(|o| safety::is_within(&key, o) || safety::is_within(o, &key)) {
            continue;
        }
        seen.push(key);
        let (s, note) = match safety::uninstall_blocked(&path) {
            Some(reason) => (Safety::Protected, reason),
            None if s == Safety::Caution => (s, "Name only partly matches - check it really belongs to this app"),
            None => (s, "Matches the app's name"),
        };
        picked.push((path, what, s, note));
    }

    let mut out: Vec<Leftover> = picked
        .into_par_iter()
        .map(|(path, what, safety, note)| {
            let (size, files) = dir_size(&path);
            Leftover { path, what, size, files, safety, note, selected: safety == Safety::Safe }
        })
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size));
    out
}

// ------------------------------------------------------------------ reset

/// Leftover kinds that hold an app's data rather than the program itself.
const DATA_KINDS: &[&str] = &["App data", "App data (Roaming)", "App data (Local)", "App data (LocalLow)"];

/// Folders of a Store app's package data that Windows' own "Reset" empties.
const STORE_DATA: &[&str] = &["LocalState", "RoamingState", "LocalCache", "TempState", "Settings"];

pub struct ResetTarget {
    pub path: PathBuf,
    /// Store data folders are emptied (Windows expects them to exist); desktop data folders go entirely.
    pub contents: bool,
    pub size: u64,
    pub files: u64,
    pub what: &'static str,
}

/// True when `dir` directly holds an .exe: then it is (part of) the program, not its data.
fn holds_program(dir: &Path) -> bool {
    fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
        e.file_type().is_ok_and(|t| t.is_file()) && e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"))
    })
}

/// What resetting `app` deletes, picked from its leftover list: only data folders whose name fully matches
/// the app (Safe). The install folder, and any folder holding the program itself, never count as data.
pub fn reset_targets(app: &InstalledApp, leftovers: &[Leftover]) -> Vec<ResetTarget> {
    let data = leftovers.iter().filter(|l| l.safety == Safety::Safe && DATA_KINDS.contains(&l.what) && l.path.is_dir());
    if app.is_store() {
        return data
            .flat_map(|l| STORE_DATA.iter().map(|d| l.path.join(d)))
            .filter(|p| p.is_dir())
            .map(|path| {
                let (size, files) = dir_size(&path);
                ResetTarget { path, contents: true, size, files, what: "App data" }
            })
            .collect();
    }
    let install = app.location.as_deref().map(safety::norm);
    data.filter(|l| {
        let key = safety::norm(&l.path);
        !install.as_deref().is_some_and(|i| safety::is_within(&key, i) || safety::is_within(i, &key))
    })
    .filter(|l| !holds_program(&l.path))
    .map(|l| ResetTarget { path: l.path.clone(), contents: false, size: l.size, files: l.files, what: l.what })
    .collect()
}

pub fn start_leftovers(app: InstalledApp, all: Vec<InstalledApp>) -> Receiver<Vec<Leftover>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(find_leftovers(&app, &all));
    });
    rx
}

// ------------------------------------------------------------------ uninstall

#[derive(Default)]
pub struct UninstallProgress {
    pub stage: Mutex<String>,
    pub cancel: AtomicBool,
}

pub struct Outcome {
    pub removed: bool,
    pub message: String,
}

/// Splits an UninstallString into program and arguments. MsiExec /I (repair) becomes /X (remove).
pub fn split_command(cmd: &str) -> (String, String) {
    let c = cmd.trim();
    let (exe, mut args) = if let Some(rest) = c.strip_prefix('"') {
        match rest.find('"') {
            Some(i) => (rest[..i].to_string(), rest[i + 1..].trim().to_string()),
            None => (rest.to_string(), String::new()),
        }
    } else if let Some(i) = c.to_ascii_lowercase().find(".exe") {
        (c[..i + 4].to_string(), c[i + 4..].trim().to_string())
    } else {
        match c.split_once(' ') {
            Some((a, b)) => (a.to_string(), b.trim().to_string()),
            None => (c.to_string(), String::new()),
        }
    };
    let lower = exe.to_ascii_lowercase();
    if lower.ends_with("msiexec.exe") || lower == "msiexec" {
        args = args.replacen("/I", "/X", 1).replacen("/i", "/x", 1);
    }
    (exe, args)
}

pub fn key_exists(hive: Hive, key: &str) -> bool {
    let (root, view) = hive_key(hive);
    root.open_subkey_with_flags(key, KEY_READ | view).is_ok()
}

pub fn start_uninstall(app: InstalledApp, prog: Arc<UninstallProgress>) -> Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(uninstall(&app, &prog));
    });
    rx
}

fn uninstall(app: &InstalledApp, prog: &UninstallProgress) -> Outcome {
    let stage = |s: &str| *prog.stage.lock().unwrap() = s.to_string();
    match &app.source {
        Source::Store { full_name, .. } => {
            stage("Removing the Store app...");
            let cmd = format!("Remove-AppxPackage -Package '{}'", full_name.replace('\'', "''"));
            match Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                Ok(o) if o.status.success() => Outcome { removed: true, message: format!("{} was removed.", app.name) },
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("unknown error");
                    Outcome { removed: false, message: format!("Windows refused to remove it: {first}") }
                }
                Err(e) => Outcome { removed: false, message: format!("Could not run PowerShell: {e}") },
            }
        }
        Source::Registry { hive, key, uninstall, msi } => {
            let (exe, args) = match (msi, uninstall) {
                (Some(code), _) => ("msiexec.exe".to_string(), format!("/x {code}")),
                (None, Some(cmd)) => split_command(cmd),
                (None, None) => {
                    return Outcome {
                        removed: false,
                        message: "This app has no uninstaller. Use 'Remove entry' and delete its leftovers.".into(),
                    }
                }
            };
            stage("Follow the uninstaller window. Freespace waits until it is done.");
            let ran = match win::run_and_wait(&exe, &args, &prog.cancel) {
                Ok(d) => d,
                Err(e) => return Outcome { removed: false, message: e },
            };
            // Many uninstallers copy themselves to Temp and exit at once, so watch the
            // registry entry instead: a quick exit means the real work is still running.
            let grace = if ran < Duration::from_secs(5) { Duration::from_secs(30 * 60) } else { Duration::from_secs(15) };
            stage("Waiting for the app to disappear from Windows' installed list...");
            let start = Instant::now();
            while start.elapsed() < grace && !prog.cancel.load(Relaxed) {
                if !key_exists(*hive, key) {
                    return Outcome { removed: true, message: format!("{} was uninstalled.", app.name) };
                }
                std::thread::sleep(Duration::from_millis(800));
            }
            if !key_exists(*hive, key) {
                return Outcome { removed: true, message: format!("{} was uninstalled.", app.name) };
            }
            Outcome { removed: false, message: format!("{} is still installed - the uninstaller may have been cancelled.", app.name) }
        }
    }
}

/// Deletes the app's entry from Windows' installed list (for apps whose uninstaller is gone).
pub fn remove_entry(app: &InstalledApp) -> Result<(), String> {
    let Source::Registry { hive, key, .. } = &app.source else {
        return Err("Store apps have no registry entry to remove.".into());
    };
    let (root, view) = hive_key(*hive);
    let (parent, name) = key.rsplit_once('\\').ok_or("Bad registry path")?;
    let parent = root
        .open_subkey_with_flags(parent, KEY_ALL_ACCESS | view)
        .map_err(|e| format!("Could not open the entry (run as administrator?): {e}"))?;
    parent.delete_subkey_all(name).map_err(|e| format!("Could not delete the entry: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_uninstall_strings() {
        assert_eq!(
            split_command("\"C:\\Program Files\\App\\uninst.exe\" /S"),
            ("C:\\Program Files\\App\\uninst.exe".into(), "/S".into())
        );
        assert_eq!(
            split_command("C:\\Program Files\\App\\uninstall.exe --force"),
            ("C:\\Program Files\\App\\uninstall.exe".into(), "--force".into())
        );
        assert_eq!(
            split_command("MsiExec.exe /I{1234-ABCD}"),
            ("MsiExec.exe".into(), "/X{1234-ABCD}".into())
        );
    }

    #[test]
    fn names() {
        assert_eq!(clean_name("Mozilla Firefox (x64 en-US)"), "Mozilla Firefox");
        assert_eq!(clean_name("Node.js 20.11.0 x64"), "Node.js");
        assert_eq!(clean_name("Zoom Workplace v6.1"), "Zoom Workplace");
        assert_eq!(pretty_store_name("Microsoft.WindowsCalculator"), "Windows Calculator");
        assert_eq!(pretty_store_name("SpotifyAB.SpotifyMusic"), "Spotify Music");
        assert_eq!(cn("CN=Microsoft Corporation, O=Microsoft Corporation, C=US"), "Microsoft Corporation");
    }

    #[test]
    fn keys_skip_generic_words() {
        let app = InstalledApp {
            name: "Mozilla Firefox (x64 en-US)".into(),
            publisher: "Mozilla".into(),
            version: "130".into(),
            installed: String::new(),
            location: None,
            size: 0,
            icon: None,
            source: Source::Store { full_name: String::new(), family: String::new() },
        };
        let k = keys(&app);
        assert!(k.strong.contains("mozillafirefox"));
        assert!(k.strong.contains("firefox"));
        assert_eq!(k.publisher.as_deref(), Some("mozilla"));
        let vc = InstalledApp { name: "Microsoft Visual C++ 2015 Redistributable".into(), publisher: "Microsoft Corporation".into(), ..app };
        let k = keys(&vc);
        assert!(!k.weak.contains("visual") && !k.weak.contains("redistributable"));
    }

    #[test]
    fn reset_keeps_the_program() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("reset-fixture");
        let _ = fs::remove_dir_all(&root);
        let mk = |rel: &str| {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, b"x").unwrap();
            p.parent().unwrap().to_path_buf()
        };
        let roaming = mk("Roaming/Demo/settings.json");
        let local_prog = mk("Local/Demo/Update.exe");
        let install = mk("Programs/Demo/demo.exe");
        let partial = mk("Local/DemoTools/cache.bin");
        let shortcut = mk("Start/Demo/x.lnk");
        let lo = |path: &Path, what: &'static str, safety: Safety| Leftover {
            path: path.to_path_buf(), what, size: 1, files: 1, safety, note: "", selected: true,
        };
        let leftovers = vec![
            lo(&roaming, "App data (Roaming)", Safety::Safe),
            lo(&local_prog, "App data (Local)", Safety::Safe),
            lo(&install, "App data (Local)", Safety::Safe),
            lo(&partial, "App data (Local)", Safety::Caution),
            lo(&shortcut, "Start menu", Safety::Safe),
        ];
        let app = InstalledApp {
            name: "Demo".into(),
            publisher: String::new(),
            version: String::new(),
            installed: String::new(),
            location: Some(install.clone()),
            size: 0,
            icon: None,
            source: Source::Registry { hive: Hive::User, key: String::new(), uninstall: None, msi: None },
        };
        let picked: Vec<PathBuf> = reset_targets(&app, &leftovers).into_iter().map(|t| t.path).collect();
        assert_eq!(picked, [roaming], "only the fully matching data folder without an .exe");

        let pkg = root.join("Packages/Demo_123");
        fs::create_dir_all(pkg.join("LocalState")).unwrap();
        fs::create_dir_all(pkg.join("AC")).unwrap();
        let store = InstalledApp {
            location: None,
            source: Source::Store { full_name: String::new(), family: "Demo_123".into() },
            ..app
        };
        let t = reset_targets(&store, &[lo(&pkg, "App data", Safety::Safe)]);
        assert_eq!(t.iter().map(|t| (t.path.clone(), t.contents)).collect::<Vec<_>>(), [(pkg.join("LocalState"), true)]);
        fs::remove_dir_all(&root).unwrap();
    }

    /// cargo test --release -- --ignored --nocapture list_apps
    #[test]
    #[ignore]
    fn list_apps() {
        let t = Instant::now();
        let reg = list_registry();
        let store = list_store();
        println!("{} desktop + {} store apps in {:.2}s", reg.len(), store.len(), t.elapsed().as_secs_f32());
        let all: Vec<InstalledApp> = reg.iter().chain(&store).cloned().collect();
        for a in reg.iter().take(6).chain(store.iter().take(3)) {
            println!("- {} | {} | {} | {:?}", a.name, a.publisher, a.version, a.location);
            for l in find_leftovers(a, &all) {
                println!("    {:>10} {:?} {} {}", crate::util::fmt_size(l.size), l.safety, l.what, l.path.display());
            }
        }
    }
}
