//! Thin wrappers over the Win32 calls the app needs.

use crate::util::wide;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
use windows_sys::Win32::System::Threading::{CreateEventW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE};
use windows_sys::Win32::UI::Shell::{
    IsUserAnAdmin, SHEmptyRecycleBinW, SHQueryRecycleBinW, ShellExecuteExW, ShellExecuteW, SEE_MASK_NOCLOSEPROCESS,
    SHELLEXECUTEINFOW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};

#[derive(Clone, Debug)]
pub struct DriveInfo {
    pub root: String,
    pub kind: &'static str,
    pub total: u64,
    pub free: u64,
}

pub fn drives() -> Vec<DriveInfo> {
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + i as u8) as char);
        let w = wide(&root);
        let kind = match unsafe { GetDriveTypeW(w.as_ptr()) } {
            2 => "Removable",
            3 => "Local",
            4 => "Network",
            6 => "RAM disk",
            _ => continue, // CD-ROM, unknown, no root
        };
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) } == 0 {
            continue;
        }
        out.push(DriveInfo { root, kind, total, free });
    }
    out
}

/// Returns (bytes, items) in the Recycle Bin of the drive that holds `root`.
pub fn recycle_bin_info(root: &str) -> (u64, u64) {
    let w = wide(root);
    let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
    let hr = unsafe { SHQueryRecycleBinW(w.as_ptr(), &mut info) };
    if hr == 0 {
        (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64)
    } else {
        (0, 0)
    }
}

pub fn empty_recycle_bin(root: &Path) -> bool {
    let w = wide(&root.to_string_lossy());
    let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
    let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), w.as_ptr(), flags) };
    // E_UNEXPECTED is returned when the bin is already empty.
    hr == 0 || hr as u32 == 0x8000_FFFF
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Marks a copy started by another copy that is about to exit, so it does not mistake that one for a running app.
pub const RELAUNCHED: &str = "--relaunched";

const SHOW_EVENT: &str = "Local\\Freespace.ShowWindow";

/// Everyone signed in may signal the event, and the medium label lets a normal copy reach an elevated one. Windows
/// blocks window messages across that boundary, which is why this is an event.
const SHOW_EVENT_ACCESS: &str = "D:(A;;0x100002;;;AU)S:(ML;;NW;;;ME)";

/// If another copy is running, asks it to show its window. Returns whether there was one.
pub fn signal_running_copy() -> bool {
    signal_event(SHOW_EVENT)
}

/// Runs `show` on a background thread each time a later copy calls `signal_running_copy`.
pub fn on_show_request(show: impl Fn() + Send + 'static) {
    listen_event(SHOW_EVENT, show);
}

fn signal_event(name: &str) -> bool {
    let name = wide(name);
    unsafe {
        let h = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
        if h.is_null() {
            return false;
        }
        SetEvent(h);
        CloseHandle(h);
    }
    true
}

fn listen_event(name: &str, show: impl Fn() + Send + 'static) {
    let name = wide(name);
    let access = wide(SHOW_EVENT_ACCESS);
    let h = unsafe {
        let mut sd = std::ptr::null_mut();
        let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(access.as_ptr(), 1, &mut sd, std::ptr::null_mut()) != 0;
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let h = CreateEventW(if ok { &attrs } else { std::ptr::null() }, 0, 0, name.as_ptr());
        if ok {
            LocalFree(sd);
        }
        h
    };
    if h.is_null() {
        return;
    }
    // The handle lives as long as the process: it is what tells later copies this one is running.
    let h = h as usize;
    std::thread::spawn(move || loop {
        if unsafe { WaitForSingleObject(h as _, INFINITE) } != 0 {
            return;
        }
        show();
    });
}

/// Starts a new elevated copy of this exe with the same arguments. Returns true if the UAC prompt was accepted.
pub fn relaunch_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    // A trailing backslash would escape the closing quote; `C:` and `D:\x` both still name the same folder.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != RELAUNCHED)
        .map(|a| format!("\"{}\"", a.trim_end_matches('\\')))
        .chain([RELAUNCHED.to_string()])
        .collect();
    let params = wide(&args.join(" "));
    let h = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    h as isize > 32
}

/// Opens Explorer with the item selected.
pub fn reveal(path: &Path) {
    let _ = Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn();
}

/// Runs a program through the shell (so its UAC prompt appears) and waits until it exits
/// or `cancel` is set. Returns how long the process ran.
pub fn run_and_wait(exe: &str, args: &str, cancel: &AtomicBool) -> Result<Duration, String> {
    let verb = wide("open");
    let file = wide(exe);
    let params = wide(args);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = 1; // SW_SHOWNORMAL
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let e = std::io::Error::last_os_error();
        return Err(if e.raw_os_error() == Some(1223) {
            "Cancelled at the Windows permission prompt.".into()
        } else {
            format!("Could not start the uninstaller: {e}")
        });
    }
    let started = Instant::now();
    if info.hProcess.is_null() {
        return Ok(started.elapsed());
    }
    const WAIT_TIMEOUT: u32 = 0x102;
    while unsafe { WaitForSingleObject(info.hProcess, 300) } == WAIT_TIMEOUT && !cancel.load(Relaxed) {}
    unsafe { CloseHandle(info.hProcess) };
    Ok(started.elapsed())
}

pub fn open_reset_this_pc() {
    let _ = Command::new("explorer.exe").arg("ms-settings:recovery").spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_later_copy_reaches_the_running_one() {
        let name = format!("Local\\Freespace.Test.{}", std::process::id());
        assert!(!signal_event(&name), "nothing is running yet");

        let (tx, rx) = mpsc::channel();
        listen_event(&name, move || tx.send(()).unwrap());
        for _ in 0..2 {
            assert!(signal_event(&name));
            rx.recv_timeout(Duration::from_secs(5)).expect("the running copy is told");
        }
    }
}
