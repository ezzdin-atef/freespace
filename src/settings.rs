//! User preferences, kept in a small JSON file. A missing or damaged file just means the defaults.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Rescanning a whole drive is real disk work, so the automatic rescan never runs more often than this.
const MIN_RESCAN_MINUTES: u32 = 15;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Launch into the tray instead of opening the window.
    pub start_in_tray: bool,
    /// 0 turns the automatic rescan off.
    pub rescan_minutes: u32,
    /// What the last scan looked at; the automatic rescan repeats it.
    pub last_root: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { start_in_tray: true, rescan_minutes: 60, last_root: None }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut s: Self = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        s.rescan_minutes = clamp_minutes(s.rescan_minutes);
        s
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)
    }
}

pub fn clamp_minutes(minutes: u32) -> u32 {
    if minutes == 0 {
        0
    } else {
        minutes.max(MIN_RESCAN_MINUTES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip_and_bad_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("settings-fixture");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default(), "no file yet");

        let s = Settings { start_in_tray: false, rescan_minutes: 360, last_root: Some("D:\\code".into()) };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);

        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default(), "damaged file");
        fs::write(&path, r#"{"rescan_minutes": 1}"#).unwrap();
        let partial = Settings::load(&path);
        assert_eq!((partial.rescan_minutes, partial.start_in_tray), (MIN_RESCAN_MINUTES, true), "missing keys get defaults");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rescan_interval_has_a_floor_but_can_be_off() {
        assert_eq!(clamp_minutes(0), 0);
        assert_eq!(clamp_minutes(5), MIN_RESCAN_MINUTES);
        assert_eq!(clamp_minutes(60), 60);
    }
}
