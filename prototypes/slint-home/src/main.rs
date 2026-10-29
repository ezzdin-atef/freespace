#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};

slint::include_modules!();

struct DriveInfo {
    root: String,
    kind: &'static str,
    total: u64,
    free: u64,
}

// Same logic as logcleaner's win::drives, copied so the prototype stays standalone.
fn drives() -> Vec<DriveInfo> {
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
        out.push(DriveInfo { root, kind, total, free });
    }
    out
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

fn main() -> Result<(), slint::PlatformError> {
    // Skia gives the best text; run with SLINT_BACKEND=winit-femtovg to compare the default renderer.
    if std::env::var_os("SLINT_BACKEND").is_none() {
        std::env::set_var("SLINT_BACKEND", "winit-skia");
    }
    let renderer = std::env::var("SLINT_BACKEND").unwrap_or_default();

    let ui = AppWindow::new()?;
    ui.set_renderer(SharedString::from(renderer.trim_start_matches("winit-")));

    let list = drives();
    let sys = list.iter().position(|d| d.root.eq_ignore_ascii_case("C:\\")).unwrap_or(0);
    let rows: Vec<Drive> = list
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let used = d.total.saturating_sub(d.free);
            let frac = used as f32 / d.total.max(1) as f32;
            Drive {
                label: format!("{} disk ({})", d.kind, d.root.trim_end_matches('\\')).into(),
                sub: format!("{} free of {}", fmt_size(d.free), fmt_size(d.total)).into(),
                used_pct: format!("{:.0}% used", frac * 100.0).into(),
                used: fmt_size(used).into(),
                frac,
                primary: i == sys,
            }
        })
        .collect();
    ui.set_drives(ModelRc::from(Rc::new(VecModel::from(rows))));

    ui.on_scan(|i| println!("scan drive {i}"));
    ui.on_scan_folder(|path| println!("scan folder {path}"));
    ui.run()
}
