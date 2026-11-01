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
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(config_path(), s);
        }
    }
}
