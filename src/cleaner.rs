//! Deletes the selected findings in parallel. Every path is re-checked against
//! `safety::deletion_blocked` right before it is removed.

use crate::rules::{DeleteMode, ItemKind};
use crate::scanner::Finding;
use crate::{safety, win};
use rayon::prelude::*;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_FAILURES: usize = 500;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Default)]
pub struct Maintenance {
    /// DISM /StartComponentCleanup - shrinks WinSxS.
    pub dism: bool,
    /// powercfg /hibernate off - removes hiberfil.sys.
    pub hibernate_off: bool,
}

/// Which safety rules gate each delete.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Policy {
    /// Junk cleaning: Program Files, ProgramData and profile roots are off-limits.
    #[default]
    Junk,
    /// Leftovers of an app being uninstalled: its own Program Files / AppData folders may go.
    Uninstall,
}

#[derive(Default)]
pub struct CleanProgress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub freed: AtomicU64,
    pub files: AtomicU64,
    pub failed: AtomicU64,
    pub current: Mutex<String>,
    pub cancel: AtomicBool,
}

pub struct CleanReport {
    pub freed: u64,
    pub files: u64,
    pub failed: u64,
    pub failures: Vec<(String, String)>,
    pub skipped: Vec<(String, String)>,
    pub maintenance: Vec<String>,
    pub processed: Vec<PathBuf>,
    pub elapsed: Duration,
    pub used_trash: bool,
    pub cancelled: bool,
}

pub fn start(
    items: Vec<Finding>,
    use_trash: bool,
    maint: Maintenance,
    policy: Policy,
    prog: Arc<CleanProgress>,
) -> Receiver<CleanReport> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(items, use_trash, maint, policy, &prog));
    });
    rx
}

struct Deleter<'a> {
    prog: &'a CleanProgress,
    failures: Mutex<Vec<(String, String)>>,
}

impl Deleter<'_> {
    fn fail(&self, p: &Path, msg: String) {
        self.prog.failed.fetch_add(1, Relaxed);
        let mut f = self.failures.lock().unwrap();
        if f.len() < MAX_FAILURES {
            f.push((p.display().to_string(), msg));
        }
    }

    /// Removes a file or folder tree. Links are removed, never followed.
    fn remove_tree(&self, p: &Path) -> bool {
        if self.prog.cancel.load(Relaxed) {
            return false;
        }
        let md = match fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => return true,
            Err(e) => {
                self.fail(p, e.to_string());
                return false;
            }
        };
        let ft = md.file_type();
        if ft.is_symlink() {
            return match fs::remove_dir(p).or_else(|_| fs::remove_file(p)) {
                Ok(()) => true,
                Err(e) => {
                    self.fail(p, e.to_string());
                    false
                }
            };
        }
        if !ft.is_dir() {
            return self.remove_file(p, md.len());
        }
        let children: Vec<PathBuf> = match fs::read_dir(p) {
            Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
            Err(e) => {
                self.fail(p, e.to_string());
                return false;
            }
        };
        let failed = children.par_iter().map(|c| self.remove_tree(c)).filter(|ok| !ok).count();
        if failed > 0 {
            return false;
        }
        retry_readonly(p, |p| fs::remove_dir(p)).map_err(|e| self.fail(p, e.to_string())).is_ok()
    }

    fn remove_file(&self, p: &Path, len: u64) -> bool {
        match retry_readonly(p, |p| fs::remove_file(p)) {
            Ok(()) => {
                self.prog.freed.fetch_add(len, Relaxed);
                self.prog.files.fetch_add(1, Relaxed);
                true
            }
            Err(e) => {
                self.fail(p, e.to_string());
                false
            }
        }
    }
}

/// Retries once after clearing the read-only attribute (common in node_modules and .git).
fn retry_readonly(p: &Path, op: impl Fn(&Path) -> io::Result<()>) -> io::Result<()> {
    match op(p) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            if let Ok(md) = fs::symlink_metadata(p) {
                let mut perm = md.permissions();
                if perm.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perm.set_readonly(false);
                    let _ = fs::set_permissions(p, perm);
                    return op(p);
                }
            }
            Err(e)
        }
        r => r,
    }
}

/// Paths to delete for an item, minus anything the safety rules forbid.
fn targets(item: &Finding, del: &Deleter, policy: Policy, skipped: &mut Vec<(String, String)>) -> Vec<PathBuf> {
    let candidates = match item.mode {
        DeleteMode::Whole => vec![item.path.clone()],
        DeleteMode::Contents => match fs::read_dir(&item.path) {
            Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
            Err(e) => {
                del.fail(&item.path, e.to_string());
                return Vec::new();
            }
        },
    };
    candidates
        .into_iter()
        .filter(|p| match match policy {
            Policy::Junk => safety::deletion_blocked(p),
            Policy::Uninstall => safety::uninstall_blocked(p),
        } {
            Some(reason) => {
                skipped.push((p.display().to_string(), reason.to_string()));
                false
            }
            None => true,
        })
        .collect()
}

fn run(items: Vec<Finding>, use_trash: bool, maint: Maintenance, policy: Policy, prog: &CleanProgress) -> CleanReport {
    let started = Instant::now();
    prog.total.store(items.len() as u64, Relaxed);
    let del = Deleter { prog, failures: Mutex::new(Vec::new()) };
    let mut skipped = Vec::new();
    let mut processed = Vec::new();
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads((cores * 2).clamp(4, 32))
        .stack_size(8 << 20)
        .build()
        .expect("thread pool");

    for item in &items {
        if prog.cancel.load(Relaxed) {
            break;
        }
        *prog.current.lock().unwrap() = item.path.display().to_string();
        match item.kind {
            ItemKind::RecycleBin => {
                if win::empty_recycle_bin(&item.path) {
                    prog.freed.fetch_add(item.size, Relaxed);
                    prog.files.fetch_add(item.files, Relaxed);
                } else {
                    del.fail(&item.path, "Could not empty the Recycle Bin".into());
                }
            }
            _ if use_trash => {
                let mut ok = true;
                for t in targets(item, &del, policy, &mut skipped) {
                    if prog.cancel.load(Relaxed) {
                        break;
                    }
                    if let Err(e) = trash::delete(&t) {
                        ok = false;
                        del.fail(&t, e.to_string());
                    }
                }
                if ok {
                    prog.files.fetch_add(item.files, Relaxed);
                }
            }
            _ => {
                let ts = targets(item, &del, policy, &mut skipped);
                pool.install(|| ts.par_iter().for_each(|t| {
                    del.remove_tree(t);
                }));
            }
        }
        processed.push(item.path.clone());
        prog.done.fetch_add(1, Relaxed);
    }

    let mut maintenance = Vec::new();
    if maint.dism && !prog.cancel.load(Relaxed) {
        *prog.current.lock().unwrap() = "Windows component store cleanup (DISM) - can take several minutes...".into();
        maintenance.push(run_cmd(
            "Component store cleanup",
            "dism.exe",
            &["/Online", "/Cleanup-Image", "/StartComponentCleanup"],
        ));
    }
    if maint.hibernate_off && !prog.cancel.load(Relaxed) {
        *prog.current.lock().unwrap() = "Turning off hibernation...".into();
        maintenance.push(run_cmd("Turn off hibernation", "powercfg.exe", &["/hibernate", "off"]));
    }

    CleanReport {
        freed: prog.freed.load(Relaxed),
        files: prog.files.load(Relaxed),
        failed: prog.failed.load(Relaxed),
        failures: del.failures.into_inner().unwrap_or_default(),
        skipped,
        maintenance,
        processed,
        elapsed: started.elapsed(),
        used_trash: use_trash,
        cancelled: prog.cancel.load(Relaxed),
    }
}

fn run_cmd(title: &str, exe: &str, args: &[&str]) -> String {
    match Command::new(exe).args(args).creation_flags(CREATE_NO_WINDOW).output() {
        Ok(o) => {
            let raw = if o.stdout.is_empty() { &o.stderr } else { &o.stdout };
            let text = String::from_utf8_lossy(raw);
            let mut tail: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).rev().take(2).collect();
            tail.reverse();
            let status = if o.status.success() { "done" } else { "failed" };
            format!("{title}: {status}. {}", tail.join(" "))
        }
        Err(e) => format!("{title}: could not start ({e})"),
    }
}
