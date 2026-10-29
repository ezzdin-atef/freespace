//! Tauri commands: the only surface the web UI can call. Keep them thin wrappers over the backend modules.

use crate::win;
use serde::Serialize;

#[derive(Serialize)]
pub struct Drive {
    pub root: String,
    pub kind: &'static str,
    pub total: u64,
    pub free: u64,
}

#[derive(Serialize)]
pub struct Init {
    pub drives: Vec<Drive>,
    pub is_admin: bool,
}

/// Everything the UI needs on first paint.
#[tauri::command]
pub fn init() -> Init {
    let drives = win::drives()
        .into_iter()
        .map(|d| Drive { root: d.root, kind: d.kind, total: d.total, free: d.free })
        .collect();
    Init { drives, is_admin: win::is_admin() }
}
