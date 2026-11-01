//! Persistent settings and cross-platform config directory.

use crate::wallpaper::Fit;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Fixed,
    Slideshow,
}
impl Default for Mode {
    fn default() -> Self {
        Mode::Fixed
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub folder: Option<PathBuf>,
    pub recursive: bool,
    pub mode: Mode,
    pub interval_secs: u64,
    pub shuffle: bool,
    pub fit: Fit,
    /// 0 = light, 1 = dark
    pub theme: usize,
    pub last_path: Option<PathBuf>,
    pub running: bool,
    pub search: String,
    /// Launch the app automatically at login (Windows Run key / LaunchAgent / autostart .desktop).
    #[serde(default)]
    pub autostart: bool,
    /// OLED burn-in guard: periodically micro-shift the wallpaper.
    #[serde(default)]
    pub oled_guard: bool,
    /// How often (seconds) to micro-shift while the OLED guard is on.
    #[serde(default = "default_shift_interval")]
    pub shift_interval_secs: u64,
    /// Maximum micro-shift in pixels.
    #[serde(default = "default_shift_max")]
    pub shift_max_px: u32,
    /// Minimize to background on close (continues running) instead of exiting.
    #[serde(default = "default_true")]
    pub close_to_background: bool,
    /// Online daily wallpaper (Bing) auto-fetch + set on launch/day.
    #[serde(default)]
    pub daily_online: bool,
    /// When fetching an online wallpaper, also apply it to the desktop.
    #[serde(default = "default_true")]
    pub online_auto_set: bool,
    /// Local "了解有关此图片信息" feedback counters.
    #[serde(default)]
    pub online_likes: i64,
    #[serde(default)]
    pub online_dislikes: i64,
    /// Last date (YYYYMMDD) an online daily wallpaper was applied (for daily rotation).
    #[serde(default)]
    pub last_online_date: String,
}

fn default_shift_interval() -> u64 {
    60
}
fn default_shift_max() -> u32 {
    8
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folder: None,
            recursive: true,
            mode: Mode::Fixed,
            interval_secs: 60,
            shuffle: false,
            fit: Fit::Fill,
            theme: 0,
            last_path: None,
            running: false,
            search: String::new(),
            autostart: false,
            oled_guard: false,
            shift_interval_secs: 60,
            shift_max_px: 8,
            close_to_background: true,
            daily_online: false,
            online_auto_set: true,
            online_likes: 0,
            online_dislikes: 0,
            last_online_date: String::new(),
        }
    }
}

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(a) = env::var_os("APPDATA") {
            return PathBuf::from(a).join("wallpaper-rs");
        }
        home().join("wallpaper-rs")
    }
    #[cfg(target_os = "macos")]
    {
        home().join("Library").join("Application Support").join("wallpaper-rs")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("wallpaper-rs")
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

impl Config {
    pub fn load() -> Self {
        // Prefer the live file; fall back to the last-known-good backup if the
        // live file is missing or was left corrupt by an interrupted write.
        parse_file(&config_path())
            .or_else(|| parse_file(&config_path().with_extension("json.bak")))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        let Ok(s) = serde_json::to_string_pretty(self) else {
            return;
        };
        let path = config_path();
        // Roll the current good file to `.bak` before overwriting.
        if path.exists() {
            let _ = std::fs::copy(&path, path.with_extension("json.bak"));
        }
        // Atomic write: temp file in the same dir, then rename over the target.
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, s.as_bytes()).is_ok() {
            if std::fs::rename(&tmp, &path).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }
}

fn parse_file(p: &std::path::Path) -> Option<Config> {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config written by an older version (no OLED / background fields) must
    /// still load, keeping the user's data and filling new fields with defaults.
    #[test]
    fn legacy_config_loads_with_defaults() {
        let legacy = r#"{
            "folder": "C:\\pics",
            "recursive": true,
            "mode": "Slideshow",
            "interval_secs": 30,
            "shuffle": true,
            "fit": "Fill",
            "theme": 1,
            "last_path": "C:\\pics\\a.png",
            "running": false,
            "search": "cat"
        }"#;
        let c: Config = serde_json::from_str(legacy).expect("legacy config should parse");
        assert_eq!(c.interval_secs, 30);
        assert!(c.shuffle);
        assert_eq!(c.theme, 1);
        assert_eq!(c.folder.as_deref(), Some(std::path::Path::new("C:\\pics")));
        // New-in-0.4 fields fall back to defaults.
        assert!(!c.oled_guard);
        assert_eq!(c.shift_interval_secs, 60);
        assert_eq!(c.shift_max_px, 8);
        assert!(c.close_to_background);
        assert!(!c.autostart);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let with_extra = r#"{"folder":null,"recursive":true,"mode":"Fixed",
            "interval_secs":60,"shuffle":false,"fit":"Fill","theme":0,
            "last_path":null,"running":false,"search":"","future_field":123}"#;
        let c: Config = serde_json::from_str(with_extra).expect("unknown fields tolerated");
        assert_eq!(c.mode, Mode::Fixed);
    }

    /// Atomic save writes config.json; reloading it preserves values; a second
    /// save rolls the previous file to `.bak`. Uses a temp APPDATA so the real
    /// user config is never touched.
    #[test]
    fn save_writes_atomically_and_keeps_backup() {
        let dir = std::env::temp_dir().join(format!("wp_cfg_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = std::env::var_os("APPDATA");
        std::env::set_var("APPDATA", &dir);

        let mut c = Config::default();
        c.interval_secs = 123;
        c.close_to_background = false;
        c.folder = Some(std::path::PathBuf::from("X:/pics"));
        c.save();

        let p = config_path();
        assert!(p.exists(), "config.json should be written");
        assert!(!p.with_extension("json.tmp").exists(), "temp file cleaned up");
        let loaded = Config::load();
        assert_eq!(loaded.interval_secs, 123);
        assert!(!loaded.close_to_background);
        assert_eq!(loaded.folder.as_deref(), Some(std::path::Path::new("X:/pics")));

        // Second save rolls the first good file into .bak.
        c.interval_secs = 456;
        c.save();
        assert!(p.with_extension("json.bak").exists(), "backup should exist");
        assert_eq!(Config::load().interval_secs, 456);

        match old {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
