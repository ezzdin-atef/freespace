//! Decides what may never be deleted. Every delete goes through `deletion_blocked`
//! right before it happens, so a bad rule or a bad selection cannot touch these paths.

use crate::rules::Safety;
use std::path::Path;
use std::sync::OnceLock;

pub struct Sys {
    pub windir: String,
    pub pf: String,
    pub pf86: String,
    pub programdata: String,
    pub users: String,
}

pub fn sys() -> &'static Sys {
    static SYS: OnceLock<Sys> = OnceLock::new();
    SYS.get_or_init(|| {
        let env = |k: &str, d: &str| norm(Path::new(&std::env::var(k).unwrap_or_else(|_| d.into())));
        let profile = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Default".into());
        let users = Path::new(&profile).parent().map(norm).unwrap_or_else(|| r"c:\users".into());
        Sys {
            windir: env("WINDIR", r"C:\Windows"),
            pf: env("ProgramFiles", r"C:\Program Files"),
            pf86: env("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            programdata: env("ProgramData", r"C:\ProgramData"),
            users,
        }
    })
}

/// Lowercase, backslashes, no `\\?\` prefix, no trailing slash (except `c:\`).
pub fn norm(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('/', "\\");
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    let mut s = s.to_lowercase();
    if s.len() == 2 && s.ends_with(':') {
        s.push('\\');
    }
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

pub fn is_within(p: &str, base: &str) -> bool {
    if base.ends_with('\\') {
        return p.starts_with(base);
    }
    p == base || (p.len() > base.len() && p.starts_with(base) && p.as_bytes()[base.len()] == b'\\')
}

fn strictly_within(p: &str, base: &str) -> bool {
    p.len() > base.len() && is_within(p, base)
}

/// First path segment after the drive root, e.g. `windows` for `c:\windows\temp`.
fn root_segment(p: &str) -> Option<&str> {
    let b = p.as_bytes();
    if b.len() > 3 && b[1] == b':' && b[2] == b'\\' {
        p[3..].split('\\').next()
    } else {
        None
    }
}

const ROOT_SYSTEM_ITEMS: &[&str] = &[
    "windows", "program files", "program files (x86)", "programdata", "users",
    "system volume information", "$recycle.bin", "recovery", "boot", "efi", "pagefile.sys",
    "hiberfil.sys", "swapfile.sys", "bootmgr", "bootnxt", "dumpstack.log", "dumpstack.log.tmp",
    "config.msi", "documents and settings", "$winreagent", "$sysreset",
];

/// Windows subfolders whose *contents* may be cleaned.
const WINDIR_ALLOWED: &[&str] = &[
    "temp",
    "softwaredistribution\\download",
    "logs",
    "minidump",
    "livekernelreports",
    "serviceprofiles\\networkservice\\appdata\\local\\microsoft\\windows\\deliveryoptimization\\cache",
];

const PROGRAMDATA_ALLOWED: &[&str] = &["microsoft\\windows\\wer", "nvidia corporation\\downloader"];

const PROFILE_FOLDERS: &[&str] = &[
    "appdata", "documents", "desktop", "downloads", "pictures", "videos", "music", "onedrive",
    "favorites", "contacts", "saved games", "searches", "links", "3d objects", ".ssh",
];

/// Returns the reason a path must not be deleted, or None if deleting it is allowed.
pub fn deletion_blocked(path: &Path) -> Option<&'static str> {
    blocked_norm(&norm(path))
}

pub fn blocked_norm(p: &str) -> Option<&'static str> {
    let s = sys();
    if p.len() <= 3 {
        return Some("Drive root");
    }
    if p.contains("\\.git\\") || p.ends_with("\\.git") {
        return Some("Git repository data - deleting it loses history");
    }
    if let Some(seg) = root_segment(p) {
        if seg == "system volume information" || seg == "$recycle.bin" {
            return Some("Managed by Windows (use the Recycle Bin item instead)");
        }
        let at_root = !p[3..].contains('\\');
        if at_root && ROOT_SYSTEM_ITEMS.contains(&seg) {
            return Some(match seg {
                "hiberfil.sys" => "Hibernation file - use 'Turn off hibernation' in Full clean",
                "pagefile.sys" | "swapfile.sys" => "Windows virtual memory - managed by Windows",
                _ => "Windows system item",
            });
        }
    }
    if is_within(p, &s.windir) {
        let allowed = WINDIR_ALLOWED
            .iter()
            .any(|a| strictly_within(p, &format!("{}\\{a}", s.windir)));
        return if allowed { None } else { Some("Windows system folder") };
    }
    if is_within(p, &s.pf) || is_within(p, &s.pf86) {
        return Some("Installed program - uninstall it from Settings > Apps instead");
    }
    if is_within(p, &s.programdata) {
        let allowed = PROGRAMDATA_ALLOWED
            .iter()
            .any(|a| strictly_within(p, &format!("{}\\{a}", s.programdata)));
        return if allowed { None } else { Some("Shared program data - apps and drivers depend on it") };
    }
    if is_within(p, &s.users) {
        if p == s.users {
            return Some("All user profiles");
        }
        let parts: Vec<&str> = p[s.users.len() + 1..].split('\\').collect();
        match parts.as_slice() {
            [_] => return Some("A whole user profile"),
            [_, f] if PROFILE_FOLDERS.contains(f) => {
                return Some("Profile folder - delete individual items inside it instead")
            }
            [_, "appdata", _] => return Some("Profile folder - delete individual items inside it instead"),
            [_, "appdata", "local", "temp"] => return Some("Temp folder itself - only its contents can be cleaned"),
            _ => {}
        }
    }
    None
}

/// First-level folders of Program Files / ProgramData / AppData that Windows or many apps share.
const SHARED_APP_DIRS: &[&str] = &[
    "common files", "windowsapps", "modifiablewindowsapps", "internet explorer", "microsoft.net",
    "reference assemblies", "msbuild", "dotnet", "uninstall information", "package cache", "packages",
    "programs", "ssh", "usoshared", "usoprivate", "temp", "comms", "connecteddevicesplatform",
    "application data", "start menu", "templates", "desktop", "documents", "desktop.ini", "crashdumps",
];

fn shared_app_dir(name: &str) -> bool {
    SHARED_APP_DIRS.contains(&name) || name.starts_with("windows") || name.starts_with("microsoft") || name.starts_with("regid.")
}

/// Like `deletion_blocked`, but for leftovers of an app being uninstalled: an app's own folder
/// in Program Files, ProgramData or AppData may go, folders shared with Windows or other apps may not.
pub fn uninstall_blocked(path: &Path) -> Option<&'static str> {
    let p = norm(path);
    let s = sys();
    if p.len() <= 3 {
        return Some("Drive root");
    }
    if p.contains("\\.git\\") || p.ends_with("\\.git") {
        return Some("Git repository data");
    }
    if is_within(&p, &s.windir) {
        return Some("Windows system folder");
    }
    if let Some(seg) = root_segment(&p) {
        if !p[3..].contains('\\') && ROOT_SYSTEM_ITEMS.contains(&seg) {
            return Some("Windows system item");
        }
        if seg == "system volume information" || seg == "$recycle.bin" {
            return Some("Managed by Windows");
        }
    }
    let start_menu = format!("{}\\microsoft\\windows\\start menu\\programs", s.programdata);
    let wer = format!("{}\\microsoft\\windows\\wer", s.programdata);
    if strictly_within(&p, &start_menu) || strictly_within(&p, &wer) {
        return None;
    }
    for base in [&s.pf, &s.pf86, &s.programdata] {
        if is_within(&p, base) {
            if p == *base {
                return Some("Shared program folder");
            }
            let first = p[base.len() + 1..].split('\\').next().unwrap_or_default();
            return if shared_app_dir(first) { Some("Shared by Windows or many apps") } else { None };
        }
    }
    if is_within(&p, &s.users) {
        if p == s.users {
            return Some("All user profiles");
        }
        let parts: Vec<&str> = p[s.users.len() + 1..].split('\\').collect();
        return match parts.as_slice() {
            [_] => Some("A whole user profile"),
            [_, f] if PROFILE_FOLDERS.contains(f) => Some("Profile folder"),
            [_, "appdata", _] => Some("Profile folder"),
            [_, "appdata", "roaming", "microsoft", "windows", "start menu", "programs", _, ..] => None,
            [_, "appdata", "local", "packages" | "programs", _, ..] => None,
            [_, "appdata", _, first, ..] if shared_app_dir(first) => Some("Shared by Windows or many apps"),
            _ => None,
        };
    }
    None
}

/// System areas where we never look for dev artifacts or loose log files.
pub fn is_protected_area(p: &str) -> bool {
    let s = sys();
    is_within(p, &s.windir)
        || is_within(p, &s.pf)
        || is_within(p, &s.pf86)
        || is_within(p, &s.programdata)
        || matches!(
            root_segment(p),
            Some("system volume information" | "$recycle.bin" | "recovery" | "config.msi" | "boot" | "efi")
        )
}

/// Safety label + explanation for arbitrary folders/files the user picks by hand.
pub fn assess(path: &Path) -> (Safety, &'static str) {
    let p = norm(path);
    if let Some(reason) = blocked_norm(&p) {
        return (Safety::Protected, reason);
    }
    let s = sys();
    if is_within(&p, &s.users) {
        if p.contains("\\appdata\\") {
            return (Safety::Caution, "App data - the app may lose settings or data");
        }
        return (Safety::Caution, "Your files - review before deleting");
    }
    (Safety::Caution, "Not a known cache - review before deleting")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(p: &str) -> bool {
        deletion_blocked(Path::new(p)).is_some()
    }

    #[test]
    fn system_paths_are_blocked() {
        let w = &sys().windir;
        assert!(blocked("C:\\"));
        assert!(blocked("C:"));
        assert!(blocked(w));
        assert!(blocked(&format!("{w}\\System32")));
        assert!(blocked(&format!("{w}\\System32\\drivers\\etc\\hosts")));
        assert!(blocked(&format!("{w}\\Temp")));
        assert!(!blocked(&format!("{w}\\Temp\\foo.tmp")));
        assert!(!blocked(&format!("{w}\\SoftwareDistribution\\Download\\abc")));
        assert!(blocked(&format!("{w}\\SoftwareDistribution\\DataStore")));
        assert!(blocked(&sys().pf));
        assert!(blocked(&format!("{}\\Git\\bin", sys().pf)));
        let pd = &sys().programdata;
        assert!(!blocked(&format!("{pd}\\NVIDIA Corporation\\Downloader\\abc123")));
        assert!(blocked(&format!("{pd}\\NVIDIA Corporation\\Downloader")));
        assert!(blocked(&format!("{pd}\\NVIDIA Corporation\\Drs")));
        assert!(blocked("C:\\pagefile.sys"));
        assert!(blocked("D:\\System Volume Information\\x"));
        assert!(blocked("C:\\$Recycle.Bin\\S-1-5"));
    }

    #[test]
    fn profile_roots_are_blocked() {
        let u = &sys().users;
        assert!(blocked(u));
        assert!(blocked(&format!("{u}\\bob")));
        assert!(blocked(&format!("{u}\\bob\\Documents")));
        assert!(blocked(&format!("{u}\\bob\\AppData\\Local")));
        assert!(blocked(&format!("{u}\\bob\\AppData\\Local\\Temp")));
        assert!(!blocked(&format!("{u}\\bob\\AppData\\Local\\Temp\\x")));
        assert!(!blocked(&format!("{u}\\bob\\Documents\\proj\\node_modules")));
        assert!(blocked(&format!("{u}\\bob\\proj\\.git")));
        assert!(blocked(&format!("{u}\\bob\\proj\\.git\\objects")));
    }

    #[test]
    fn normal_paths_are_allowed() {
        assert!(!blocked("D:\\projects\\app\\target"));
        assert!(!blocked("C:\\Windows.old"));
        assert!(!blocked("C:\\big.iso"));
    }

    #[test]
    fn uninstall_rules() {
        let ub = |p: String| uninstall_blocked(Path::new(&p)).is_some();
        let s = sys();
        let u = &s.users;
        assert!(!ub(format!("{}\\Mozilla Firefox", s.pf)));
        assert!(!ub(format!("{}\\Mozilla\\Firefox", s.pf86)));
        assert!(ub(s.pf.clone()));
        assert!(ub(format!("{}\\Common Files", s.pf)));
        assert!(ub(format!("{}\\Common Files\\Oracle", s.pf)));
        assert!(ub(format!("{}\\WindowsApps", s.pf)));
        assert!(ub(format!("{}\\Microsoft Office", s.pf)));
        assert!(ub(format!("{}\\Microsoft", s.programdata)));
        assert!(!ub(format!("{}\\Microsoft\\Windows\\Start Menu\\Programs\\Zoom", s.programdata)));
        assert!(!ub(format!("{}\\Zoom", s.programdata)));
        assert!(!ub(format!("{u}\\bob\\AppData\\Roaming\\Zoom")));
        assert!(!ub(format!("{u}\\bob\\AppData\\Local\\Programs\\cursor")));
        assert!(ub(format!("{u}\\bob\\AppData\\Local\\Programs")));
        assert!(!ub(format!("{u}\\bob\\AppData\\Local\\Packages\\SpotifyAB.SpotifyMusic_zpdnekdrzrea0")));
        assert!(ub(format!("{u}\\bob\\AppData\\Local\\Packages")));
        assert!(ub(format!("{u}\\bob\\AppData\\Roaming\\Microsoft")));
        assert!(!ub(format!("{u}\\bob\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Zoom")));
        assert!(ub(format!("{u}\\bob\\AppData\\Local")));
        assert!(ub(format!("{u}\\bob\\Desktop")));
        assert!(!ub(format!("{u}\\bob\\Desktop\\Zoom.lnk")));
        assert!(ub(format!("{}\\System32", s.windir)));
        assert!(ub("C:\\".into()));
    }

    #[test]
    fn norm_works() {
        assert_eq!(norm(Path::new("C:/Foo/Bar/")), "c:\\foo\\bar");
        assert_eq!(norm(Path::new("\\\\?\\C:\\X")), "c:\\x");
        assert_eq!(norm(Path::new("D:")), "d:\\");
        assert!(is_within("c:\\a\\b", "c:\\a"));
        assert!(!is_within("c:\\ab", "c:\\a"));
        assert!(is_within("c:\\a", "c:\\"));
    }
}
