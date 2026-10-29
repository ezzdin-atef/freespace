#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};

#[derive(Serialize)]
struct Drive {
    label: String,
    sub: String,
    used_pct: String,
    used: String,
    frac: f32,
    primary: bool,
}

fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Same logic as logcleaner's win::drives, shaped for the page.
#[tauri::command]
fn drives() -> Vec<Drive> {
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + i as u8) as char);
        let w: Vec<u16> = root.encode_utf16().chain([0]).collect();
        let kind = match unsafe { GetDriveTypeW(w.as_ptr()) } {
            2 => "Removable",
            3 => "Local",
            4 => "Network",
            6 => "RAM disk",
            _ => continue,
        };
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) } == 0 {
            continue;
        }
        let used = total.saturating_sub(free);
        let frac = used as f32 / total.max(1) as f32;
        out.push(Drive {
            label: format!("{kind} disk ({})", root.trim_end_matches('\\')),
            sub: format!("{} free of {}", fmt_size(free), fmt_size(total)),
            used_pct: format!("{:.0}% used", frac * 100.0),
            used: fmt_size(used),
            frac,
            primary: root.eq_ignore_ascii_case("C:\\"),
        });
    }
    out
}

#[tauri::command]
fn scan(target: String) {
    println!("scan {target}");
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![drives, scan])
        .run(tauri::generate_context!())
        .expect("failed to start the Tauri prototype");
}
