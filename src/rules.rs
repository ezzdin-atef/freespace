//! What counts as junk: known cache locations and build-artifact patterns.

use crate::safety;
use crate::util::{ends_with_ci, wild_match};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Category {
    RecycleBin,
    UserTemp,
    SystemTemp,
    WindowsUpdate,
    CrashDumps,
    WindowsLogs,
    OldWindows,
    BrowserCache,
    AppCache,
    ThumbnailCache,
    GpuCache,
    DevCache,
    NodeModules,
    BuildArtifacts,
    PythonEnv,
    LogFiles,
    LargeFiles,
    Custom,
    AppLeftovers,
}

impl Category {
    pub fn label(self) -> &'static str {
        use Category::*;
        match self {
            RecycleBin => "Recycle Bin",
            UserTemp => "User temp files",
            SystemTemp => "Windows temp files",
            WindowsUpdate => "Windows Update cache",
            CrashDumps => "Crash dumps & error reports",
            WindowsLogs => "Windows logs",
            OldWindows => "Previous Windows installation",
            BrowserCache => "Browser caches",
            AppCache => "App caches",
            ThumbnailCache => "Thumbnail & icon cache",
            GpuCache => "GPU shader caches",
            DevCache => "Package manager caches",
            NodeModules => "node_modules",
            BuildArtifacts => "Build output & dev caches",
            PythonEnv => "Python virtual environments",
            LogFiles => "Log files",
            LargeFiles => "Large files",
            Custom => "Folders you picked",
            AppLeftovers => "App leftovers",
        }
    }

    pub fn description(self) -> &'static str {
        use Category::*;
        match self {
            RecycleBin => "Files you already deleted. Emptying the bin frees the space for good.",
            UserTemp => "Temporary files left by apps and installers. Files still in use are skipped.",
            SystemTemp => "Temporary files written by Windows and services. Needs administrator.",
            WindowsUpdate => "Downloaded update packages and Delivery Optimization cache. Windows re-downloads what it still needs.",
            CrashDumps => "Memory dumps and error reports from crashed apps. Only useful for debugging.",
            WindowsLogs => "Setup, servicing and diagnostic logs. Only useful when troubleshooting Windows.",
            OldWindows => "Windows.old and upgrade leftovers. Deleting them removes the option to roll back to the previous Windows version.",
            BrowserCache => "Chrome / Edge / Brave / Firefox caches. Close the browser first. Pages load a bit slower once while the cache rebuilds. Passwords, history and cookies are NOT touched.",
            AppCache => "Caches of VS Code, Cursor, Discord, Slack, Teams, Spotify... Rebuilt automatically.",
            ThumbnailCache => "Explorer thumbnail and icon databases. Rebuilt when you browse folders.",
            GpuCache => "DirectX / NVIDIA / AMD shader caches. Games may stutter briefly while they rebuild.",
            DevCache => "npm, yarn, pnpm, pip, uv, cargo, NuGet, Gradle, Maven, Go... Packages are downloaded again when a project needs them.",
            NodeModules => "JavaScript dependencies of your projects. Restore with npm / yarn / pnpm install.",
            BuildArtifacts => "Rust target, .NET bin/obj, Gradle/CMake/Flutter build, Next.js .next, Python caches... Rebuilt on the next build.",
            PythonEnv => "Project virtual environments. Recreate with python -m venv and reinstall requirements.",
            LogFiles => "*.log files outside system folders. Database journals (which look like logs) are excluded.",
            LargeFiles => "Files of 512 MB or more. These are usually your own data - review each one.",
            Custom => "Folders you selected in Space usage.",
            AppLeftovers => "Folders and shortcuts an app left behind after uninstalling.",
        }
    }

    /// How to get the space's contents back after cleaning, shown under "How to get it back".
    pub fn restore_hint(self) -> &'static str {
        use Category::*;
        match self {
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum Safety {
    Safe,
    Caution,
    Protected,
}

impl Safety {
    // Only the egui UI reads this; goes away with it in phase 7.
    #[cfg_attr(not(feature = "egui"), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self {
            Safety::Safe => "Safe",
            Safety::Caution => "Review",
            Safety::Protected => "Do not delete",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeleteMode {
    /// Empty the folder but keep it (Temp, caches).
    Contents,
    /// Remove the folder or file itself.
    Whole,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Dir,
    File,
    RecycleBin,
}

pub struct Known {
    pub pattern: &'static str,
    pub cat: Category,
    pub safety: Safety,
    pub mode: DeleteMode,
    pub note: &'static str,
}

const fn k(pattern: &'static str, cat: Category, safety: Safety, note: &'static str) -> Known {
    Known { pattern, cat, safety, mode: DeleteMode::Contents, note }
}

const fn whole(pattern: &'static str, cat: Category, safety: Safety, note: &'static str) -> Known {
    Known { pattern, cat, safety, mode: DeleteMode::Whole, note }
}

use Category::*;
use Safety::*;

/// `~` = every user profile, `%WINDIR%` / `%PROGRAMDATA%` / `%SYSTEMDRIVE%` expand, `*` matches one folder.
pub const KNOWN: &[Known] = &[
    // Windows
    k("~\\AppData\\Local\\Temp", UserTemp, Safe, "Temp folder (contents only)"),
    k("%WINDIR%\\Temp", SystemTemp, Safe, "Windows temp folder (contents only)"),
    k("%WINDIR%\\SoftwareDistribution\\Download", WindowsUpdate, Safe, "Downloaded Windows updates"),
    k("%WINDIR%\\ServiceProfiles\\NetworkService\\AppData\\Local\\Microsoft\\Windows\\DeliveryOptimization\\Cache", WindowsUpdate, Safe, "Delivery Optimization cache"),
    k("%WINDIR%\\Minidump", CrashDumps, Safe, "Blue-screen mini dumps"),
    k("%WINDIR%\\LiveKernelReports", CrashDumps, Safe, "Kernel diagnostic dumps"),
    k("%PROGRAMDATA%\\Microsoft\\Windows\\WER", CrashDumps, Safe, "Windows Error Reporting (all users)"),
    k("~\\AppData\\Local\\CrashDumps", CrashDumps, Safe, "App crash dumps"),
    k("~\\AppData\\Local\\Microsoft\\Windows\\WER", CrashDumps, Safe, "Windows Error Reporting"),
    k("%WINDIR%\\Logs", WindowsLogs, Caution, "Windows setup / servicing logs"),
    whole("%SYSTEMDRIVE%\\Windows.old", OldWindows, Caution, "Previous Windows - removes rollback. If deletion fails, use Settings > System > Storage > Temporary files."),
    whole("%SYSTEMDRIVE%\\$WINDOWS.~BT", OldWindows, Caution, "Windows upgrade leftovers"),
    whole("%SYSTEMDRIVE%\\$Windows.~WS", OldWindows, Caution, "Windows upgrade leftovers"),
    k("~\\AppData\\Local\\Microsoft\\Windows\\INetCache", BrowserCache, Safe, "Internet Explorer / WebView cache"),
    k("~\\AppData\\Local\\Microsoft\\Windows\\Explorer", ThumbnailCache, Safe, "Thumbnail and icon cache"),
    // Browsers
    k("~\\AppData\\Local\\Google\\Chrome\\User Data\\*\\Cache", BrowserCache, Safe, "Chrome cache"),
    k("~\\AppData\\Local\\Google\\Chrome\\User Data\\*\\Code Cache", BrowserCache, Safe, "Chrome script cache"),
    k("~\\AppData\\Local\\Google\\Chrome\\User Data\\*\\GPUCache", BrowserCache, Safe, "Chrome GPU cache"),
    k("~\\AppData\\Local\\Google\\Chrome\\User Data\\*\\Service Worker\\CacheStorage", BrowserCache, Safe, "Chrome offline site cache"),
    k("~\\AppData\\Local\\Microsoft\\Edge\\User Data\\*\\Cache", BrowserCache, Safe, "Edge cache"),
    k("~\\AppData\\Local\\Microsoft\\Edge\\User Data\\*\\Code Cache", BrowserCache, Safe, "Edge script cache"),
    k("~\\AppData\\Local\\Microsoft\\Edge\\User Data\\*\\GPUCache", BrowserCache, Safe, "Edge GPU cache"),
    k("~\\AppData\\Local\\Microsoft\\Edge\\User Data\\*\\Service Worker\\CacheStorage", BrowserCache, Safe, "Edge offline site cache"),
    k("~\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\*\\Cache", BrowserCache, Safe, "Brave cache"),
    k("~\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\*\\Code Cache", BrowserCache, Safe, "Brave script cache"),
    k("~\\AppData\\Local\\Mozilla\\Firefox\\Profiles\\*\\cache2", BrowserCache, Safe, "Firefox cache"),
    k("~\\AppData\\Local\\Opera Software\\Opera Stable\\Cache", BrowserCache, Safe, "Opera cache"),
    // Apps
    k("~\\AppData\\Roaming\\Code\\Cache", AppCache, Safe, "VS Code cache"),
    k("~\\AppData\\Roaming\\Code\\CachedData", AppCache, Safe, "VS Code compiled cache"),
    k("~\\AppData\\Roaming\\Code\\Code Cache", AppCache, Safe, "VS Code script cache"),
    k("~\\AppData\\Roaming\\Code\\GPUCache", AppCache, Safe, "VS Code GPU cache"),
    k("~\\AppData\\Roaming\\Code\\CachedExtensionVSIXs", AppCache, Safe, "VS Code extension downloads"),
    k("~\\AppData\\Roaming\\Code\\logs", LogFiles, Safe, "VS Code logs"),
    k("~\\AppData\\Roaming\\Cursor\\Cache", AppCache, Safe, "Cursor cache"),
    k("~\\AppData\\Roaming\\Cursor\\CachedData", AppCache, Safe, "Cursor compiled cache"),
    k("~\\AppData\\Roaming\\Cursor\\logs", LogFiles, Safe, "Cursor logs"),
    k("~\\AppData\\Roaming\\discord\\Cache", AppCache, Safe, "Discord cache"),
    k("~\\AppData\\Roaming\\discord\\Code Cache", AppCache, Safe, "Discord script cache"),
    k("~\\AppData\\Roaming\\Slack\\Cache", AppCache, Safe, "Slack cache"),
    k("~\\AppData\\Roaming\\Slack\\Service Worker\\CacheStorage", AppCache, Safe, "Slack offline cache"),
    k("~\\AppData\\Roaming\\Microsoft\\Teams\\Cache", AppCache, Safe, "Teams (classic) cache"),
    k("~\\AppData\\Local\\Spotify\\Data", AppCache, Safe, "Spotify streaming cache"),
    k("~\\AppData\\Local\\JetBrains\\*\\caches", AppCache, Safe, "JetBrains IDE caches (re-indexes on next start)"),
    // GPU
    k("~\\AppData\\Local\\D3DSCache", GpuCache, Safe, "DirectX shader cache"),
    k("~\\AppData\\Local\\NVIDIA\\DXCache", GpuCache, Safe, "NVIDIA DirectX shader cache"),
    k("~\\AppData\\Local\\NVIDIA\\GLCache", GpuCache, Safe, "NVIDIA OpenGL shader cache"),
    k("~\\AppData\\LocalLow\\NVIDIA\\PerDriverVersion\\DXCache", GpuCache, Safe, "NVIDIA shader cache"),
    k("~\\AppData\\Local\\AMD\\DxCache", GpuCache, Safe, "AMD shader cache"),
    k("~\\AppData\\Local\\AMD\\DxcCache", GpuCache, Safe, "AMD shader cache"),
    // Package managers
    k("~\\AppData\\Local\\npm-cache", DevCache, Safe, "npm cache"),
    k("~\\AppData\\Roaming\\npm-cache", DevCache, Safe, "npm cache (old location)"),
    k("~\\AppData\\Local\\Yarn\\Cache", DevCache, Safe, "Yarn cache"),
    k("~\\AppData\\Local\\pnpm\\store", DevCache, Caution, "pnpm store - run pnpm install in your projects afterwards"),
    k("~\\.bun\\install\\cache", DevCache, Safe, "Bun cache"),
    k("~\\AppData\\Local\\node-gyp\\Cache", DevCache, Safe, "node-gyp headers"),
    k("~\\AppData\\Local\\electron\\Cache", DevCache, Safe, "Electron downloads"),
    k("~\\AppData\\Local\\electron-builder\\Cache", DevCache, Safe, "electron-builder downloads"),
    k("~\\AppData\\Local\\ms-playwright", DevCache, Caution, "Playwright browsers - reinstall with npx playwright install"),
    k("~\\AppData\\Local\\pip\\cache", DevCache, Safe, "pip cache"),
    k("~\\AppData\\Local\\uv\\cache", DevCache, Safe, "uv cache"),
    k("~\\.cargo\\registry\\cache", DevCache, Safe, "Cargo crate downloads"),
    k("~\\.cargo\\registry\\src", DevCache, Safe, "Cargo extracted crate sources"),
    k("~\\.cargo\\git\\checkouts", DevCache, Safe, "Cargo git checkouts"),
    k("~\\AppData\\Local\\go-build", DevCache, Safe, "Go build cache"),
    k("~\\AppData\\Local\\NuGet\\v3-cache", DevCache, Safe, "NuGet HTTP cache"),
    k("~\\AppData\\Local\\NuGet\\plugins-cache", DevCache, Safe, "NuGet plugin cache"),
    k("~\\.nuget\\packages", DevCache, Caution, "NuGet packages - restored by dotnet restore"),
    k("~\\.gradle\\caches", DevCache, Caution, "Gradle caches - next build downloads again"),
    k("~\\.gradle\\wrapper\\dists", DevCache, Caution, "Gradle distributions"),
    k("~\\.m2\\repository", DevCache, Caution, "Maven repository - next build downloads again"),
    k("~\\.android\\cache", DevCache, Safe, "Android SDK cache"),
];

pub fn user_profiles() -> Vec<PathBuf> {
    let base = PathBuf::from(&safety::sys().users);
    let Ok(rd) = std::fs::read_dir(&base) else {
        return std::env::var("USERPROFILE").map(|p| vec![PathBuf::from(p)]).unwrap_or_default();
    };
    rd.flatten()
        .filter(|e| e.file_type().map_or(false, |t| t.is_dir() && !t.is_symlink()))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            !matches!(n.as_str(), "public" | "default" | "default user" | "all users")
        })
        .map(|e| e.path())
        .collect()
}

fn expand_env(s: &str) -> String {
    let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    s.replace("%WINDIR%", &get("WINDIR", "C:\\Windows"))
        .replace("%PROGRAMDATA%", &get("ProgramData", "C:\\ProgramData"))
        .replace("%SYSTEMDRIVE%", &get("SystemDrive", "C:"))
}

fn glob(pattern: &str) -> Vec<PathBuf> {
    if !pattern.contains('*') {
        return vec![PathBuf::from(pattern)];
    }
    let mut parts = pattern.split('\\');
    let first = parts.next().unwrap_or_default();
    let mut cur = vec![PathBuf::from(format!("{first}\\"))];
    for part in parts {
        let mut next = Vec::new();
        for c in &cur {
            if !part.contains('*') {
                next.push(c.join(part));
                continue;
            }
            let Ok(rd) = std::fs::read_dir(c) else { continue };
            for e in rd.flatten() {
                if e.file_type().map_or(false, |t| t.is_dir()) && wild_match(part, &e.file_name().to_string_lossy()) {
                    next.push(e.path());
                }
            }
        }
        cur = next;
    }
    cur
}

/// Resolves every known location to the concrete folders that exist on this PC.
pub fn expand_known() -> Vec<(PathBuf, &'static Known)> {
    let profiles = user_profiles();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for k in KNOWN {
        let bases: Vec<String> = match k.pattern.strip_prefix("~\\") {
            Some(rest) => profiles.iter().map(|p| format!("{}\\{rest}", p.display())).collect(),
            None => vec![expand_env(k.pattern)],
        };
        for b in bases {
            for p in glob(&b) {
                if p.is_dir() && seen.insert(safety::norm(&p)) {
                    out.push((p, k));
                }
            }
        }
    }
    out
}

// Project marker files found next to a candidate folder.
pub const PKG_JSON: u32 = 1;
pub const CARGO: u32 = 2;
pub const DOTNET: u32 = 4;
pub const GRADLE: u32 = 8;
pub const PUBSPEC: u32 = 16;
pub const CMAKE: u32 = 32;
pub const POM: u32 = 64;

pub fn marker_of(name: &OsStr) -> u32 {
    let Some(n) = name.to_str() else { return 0 };
    let is = |s: &str| n.eq_ignore_ascii_case(s);
    if is("package.json") {
        PKG_JSON
    } else if is("Cargo.toml") {
        CARGO
    } else if ends_with_ci(n, ".csproj") || ends_with_ci(n, ".fsproj") || ends_with_ci(n, ".vbproj") {
        DOTNET
    } else if is("build.gradle") || is("build.gradle.kts") || is("settings.gradle") || is("settings.gradle.kts") {
        GRADLE
    } else if is("pubspec.yaml") {
        PUBSPEC
    } else if is("CMakeLists.txt") {
        CMAKE
    } else if is("pom.xml") {
        POM
    } else {
        0
    }
}

/// Matches regenerable dev folders. `name` is lowercase, `m` holds the parent's markers.
pub fn match_dev(name: &str, m: u32, path: &Path) -> Option<(Category, Safety, &'static str)> {
    let has = |bit: u32| m & bit != 0;
    let r = match name {
        "node_modules" if has(PKG_JSON) => (NodeModules, Safe, "Restore with npm / yarn / pnpm install"),
        "target" if has(CARGO) => (BuildArtifacts, Safe, "Rust build output - rebuilt by cargo build"),
        "target" if has(POM) => (BuildArtifacts, Safe, "Maven build output - rebuilt by mvn package"),
        "bin" | "obj" if has(DOTNET) => (BuildArtifacts, Safe, ".NET build output - rebuilt by dotnet build"),
        "build" if has(GRADLE | PUBSPEC | CMAKE) => (BuildArtifacts, Safe, "Build output - rebuilt on next build"),
        "build" | "dist" | "out" if has(PKG_JSON) => {
            (BuildArtifacts, Caution, "JS build output - may be a release you want to keep")
        }
        ".gradle" if has(GRADLE) => (BuildArtifacts, Safe, "Gradle project cache"),
        ".dart_tool" if has(PUBSPEC) => (BuildArtifacts, Safe, "Dart / Flutter tool cache"),
        ".next" | ".nuxt" | ".turbo" | ".parcel-cache" | ".svelte-kit" | ".angular" | ".vite" | ".cache"
            if has(PKG_JSON) =>
        {
            (BuildArtifacts, Safe, "JS framework cache - rebuilt on next dev / build")
        }
        "__pycache__" | ".pytest_cache" | ".mypy_cache" | ".ruff_cache" | ".tox" => {
            (BuildArtifacts, Safe, "Python cache - recreated automatically")
        }
        ".venv" | "venv" | "env" if path.join("pyvenv.cfg").is_file() => {
            (PythonEnv, Caution, "Virtual env - recreate it and reinstall packages")
        }
        ".terraform" => (BuildArtifacts, Caution, "Terraform providers - restored by terraform init"),
        _ if name.starts_with("cmake-build-") => (BuildArtifacts, Safe, "CMake / CLion build output"),
        _ => return None,
    };
    Some(r)
}

/// `.log` files that are really database journals (ESE, LevelDB) - deleting them corrupts data.
pub fn is_journal_log(stem_lower: &str) -> bool {
    stem_lower.is_empty()
        || stem_lower.bytes().all(|b| b.is_ascii_digit())
        || stem_lower.starts_with("edb")
        || stem_lower.starts_with("v01")
        || stem_lower.starts_with("jet")
}

/// Folder names whose `.log` files are database journals, not text logs.
pub fn is_db_dir(name_lower: &str) -> bool {
    matches!(
        name_lower,
        ".git" | "leveldb" | "indexeddb" | "local storage" | "session storage" | "databases" | "webcache"
            | "service worker" | "shared proto db" | "sync data"
    )
}

pub fn large_file_note(path: &Path) -> (Safety, String) {
    let (safety, reason) = safety::assess(path);
    if safety == Protected {
        return (Protected, reason.to_string());
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let note = match ext.as_str() {
        "vhdx" | "vhd" | "vmdk" | "vdi" | "qcow2" => {
            "Virtual disk (WSL / Docker / VM) - deleting it destroys that machine's data"
        }
        "iso" | "img" => "Disk image - safe to delete if you no longer need it",
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" => "Archive - check whether you still need it",
        "exe" | "msi" | "msix" => "Installer - usually not needed after installing",
        "mp4" | "mkv" | "mov" | "avi" | "wmv" => "Video - your personal data",
        "dmp" => return (Safe, "Crash dump - safe to delete".into()),
        "pst" | "ost" => "Outlook mailbox file - only delete if Outlook no longer uses it",
        "bak" | "old" => "Backup file",
        _ => reason,
    };
    (Caution, note.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_patterns_need_markers() {
        let p = Path::new("C:\\x");
        assert!(match_dev("node_modules", PKG_JSON, p).is_some());
        assert!(match_dev("node_modules", 0, p).is_none());
        assert!(match_dev("target", 0, p).is_none());
        assert_eq!(match_dev("target", CARGO, p).unwrap().1, Safe);
        assert!(match_dev("bin", PKG_JSON, p).is_none());
        assert_eq!(match_dev("dist", PKG_JSON, p).unwrap().1, Caution);
        assert!(match_dev("__pycache__", 0, p).is_some());
        assert!(match_dev("src", PKG_JSON | CARGO, p).is_none());
    }

    #[test]
    fn journals_are_excluded() {
        assert!(is_journal_log("000123"));
        assert!(is_journal_log("edb"));
        assert!(is_journal_log("v0100023"));
        assert!(!is_journal_log("install"));
        assert!(!is_journal_log("npm-debug"));
    }

    #[test]
    fn markers() {
        assert_eq!(marker_of(OsStr::new("Package.JSON")), PKG_JSON);
        assert_eq!(marker_of(OsStr::new("App.csproj")), DOTNET);
        assert_eq!(marker_of(OsStr::new("readme.md")), 0);
    }
}
