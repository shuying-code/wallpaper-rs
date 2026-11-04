//! Persistent settings and cross-platform config directory.

use crate::wallpaper::Fit; // 引入依赖
use serde::{Deserialize, Serialize}; // 引入依赖
use std::collections::BTreeSet; // 引入依赖
use std::env; // 引入依赖
use std::path::PathBuf; // 引入依赖

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)] // 语句
pub enum Mode { // 枚举 Mode
    Fixed, // 语句
    Slideshow, // 语句
} // 块结束
impl Default for Mode { // 实现
    fn default() -> Self { // 函数 default
        Mode::Fixed // 语句
    } // 块结束
} // 块结束

#[derive(Serialize, Deserialize, Clone, Debug)] // 语句
pub struct Config { // 结构体 Config
    pub folder: Option<PathBuf>, // 字段 folder
    pub recursive: bool, // 字段 recursive
    pub mode: Mode, // 字段 mode
    pub interval_secs: u64, // 字段 interval_secs
    pub shuffle: bool, // 字段 shuffle
    pub fit: Fit, // 字段 fit
    /// 0 = light, 1 = dark
    pub theme: usize, // 字段 theme
    pub last_path: Option<PathBuf>, // 字段 last_path
    pub running: bool, // 字段 running
    pub search: String, // 字段 search
    /// Launch the app automatically at login (Windows Run key / LaunchAgent / autostart .desktop).
    #[serde(default)] // 语句
    pub autostart: bool, // 字段 autostart
    /// Hide to the system tray on close (continues running) instead of exiting.
    #[serde(default = "default_true")] // 语句
    pub close_to_background: bool, // 字段 close_to_background
    /// Album keys the user has collapsed in the library view.
    #[serde(default)] // 语句
    pub collapsed_folders: BTreeSet<String>, // 字段 collapsed_folders
    /// Whether the left navigation sidebar is hidden.
    #[serde(default)] // 语句
    pub sidebar_hidden: bool, // 字段 sidebar_hidden
    /// Online daily wallpaper (Bing) auto-fetch + set on launch/day.
    #[serde(default)] // 语句
    pub daily_online: bool, // 字段 daily_online
    /// When fetching an online wallpaper, also apply it to the desktop.
    #[serde(default = "default_true")] // 语句
    pub online_auto_set: bool, // 字段 online_auto_set
    /// Local "了解有关此图片信息" feedback counters.
    #[serde(default)] // 语句
    pub online_likes: i64, // 字段 online_likes
    #[serde(default)] // 语句
    pub online_dislikes: i64, // 字段 online_dislikes
    /// Last date (YYYYMMDD) an online daily wallpaper was applied (for daily rotation).
    #[serde(default)] // 语句
    pub last_online_date: String, // 字段 last_online_date
    /// Only load wallpaper-grade images (≥1280×720, landscape) into the library.
    #[serde(default = "default_true")] // 语句
    pub wallpaper_only: bool, // 字段 wallpaper_only
    /// Explicit slideshow playlist (absolute paths, from the library and/or the
    /// online daily wallpaper). Empty means "cycle through the whole library".
    #[serde(default)] // 语句
    pub slideshow: Vec<PathBuf>, // 字段 slideshow
} // 块结束

fn default_true() -> bool { // 函数 default_true
    true // 语句
} // 块结束

impl Default for Config { // 实现
    fn default() -> Self { // 函数 default
        Self { // 语句
            folder: None, // 字段 folder
            recursive: true, // 字段 recursive
            mode: Mode::Fixed, // 字段 mode
            interval_secs: 60, // 字段 interval_secs
            shuffle: false, // 字段 shuffle
            fit: Fit::Fill, // 字段 fit
            theme: 0, // 字段 theme
            last_path: None, // 字段 last_path
            running: false, // 字段 running
            search: String::new(), // 字段 search
            autostart: false, // 字段 autostart
            close_to_background: true, // 字段 close_to_background
            collapsed_folders: BTreeSet::new(), // 字段 collapsed_folders
            sidebar_hidden: false, // 字段 sidebar_hidden
            daily_online: false, // 字段 daily_online
            online_auto_set: true, // 字段 online_auto_set
            online_likes: 0, // 字段 online_likes
            online_dislikes: 0, // 字段 online_dislikes
            last_online_date: String::new(), // 字段 last_online_date
            wallpaper_only: true, // 字段 wallpaper_only
            slideshow: Vec::new(), // 字段 slideshow
        } // 块结束
    } // 块结束
} // 块结束

pub fn home() -> PathBuf { // 函数 home
    env::var_os("HOME") // 调用 var_os
        .or_else(|| env::var_os("USERPROFILE")) // 语句
        .map(PathBuf::from) // 语句
        .unwrap_or_else(|| PathBuf::from(".")) // 语句
} // 块结束

pub fn config_dir() -> PathBuf { // 函数 config_dir
    #[cfg(windows)] // 语句
    { // 语句
        if let Some(a) = env::var_os("APPDATA") { // 条件分支
            return PathBuf::from(a).join("wallpaper-rs"); // 返回
        } // 块结束
        home().join("wallpaper-rs") // 调用 home
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        home().join("Library").join("Application Support").join("wallpaper-rs") // 调用 home
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        env::var_os("XDG_CONFIG_HOME") // 调用 var_os
            .map(PathBuf::from) // 语句
            .unwrap_or_else(|| home().join(".config")) // 语句
            .join("wallpaper-rs") // 语句
    } // 块结束
} // 块结束

pub fn config_path() -> PathBuf { // 函数 config_path
    config_dir().join("config.json") // 调用 config_dir
} // 块结束

impl Config { // 实现
    pub fn load() -> Self { // 函数 load
        // Prefer the live file; fall back to the last-known-good backup if the
        // live file is missing or was left corrupt by an interrupted write.
        parse_file(&config_path()) // 调用 parse_file
            .or_else(|| parse_file(&config_path().with_extension("json.bak"))) // 语句
            .unwrap_or_default() // 语句
    } // 块结束

    pub fn save(&self) { // 函数 save
        let _ = std::fs::create_dir_all(config_dir()); // 变量 _
        let Ok(s) = serde_json::to_string_pretty(self) else { // 变量 Ok
            return; // 返回
        }; // 块结束
        let path = config_path(); // 变量 path
        // Roll the current good file to `.bak` before overwriting.
        if path.exists() { // 条件分支
            let _ = std::fs::copy(&path, path.with_extension("json.bak")); // 变量 _
        } // 块结束
        // Atomic write: temp file in the same dir, then rename over the target.
        let tmp = path.with_extension("json.tmp"); // 变量 tmp
        if std::fs::write(&tmp, s.as_bytes()).is_ok() { // 条件分支
            if std::fs::rename(&tmp, &path).is_err() { // 条件分支
                let _ = std::fs::remove_file(&tmp); // 变量 _
            } // 块结束
        } // 块结束
    } // 块结束
} // 块结束

fn parse_file(p: &std::path::Path) -> Option<Config> { // 函数 parse_file
    std::fs::read_to_string(p) // 调用 read_to_string
        .ok() // 语句
        .and_then(|s| serde_json::from_str(&s).ok()) // 语句
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    /// A config written by an older version must still load, keeping the user's
    /// data and filling new fields with defaults. Legacy `oled_*` / `shift_*`
    /// keys from removed features are simply ignored (serde skips unknown keys).
    #[test] // 语句
    fn legacy_config_loads_with_defaults() { // 函数 legacy_config_loads_with_defaults
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
        }"#; // 语句
        let c: Config = serde_json::from_str(legacy).expect("legacy config should parse"); // 变量 c
        assert_eq!(c.interval_secs, 30); // 语句
        assert!(c.shuffle); // 语句
        assert_eq!(c.theme, 1); // 语句
        assert_eq!(c.folder.as_deref(), Some(std::path::Path::new("C:\\pics"))); // 语句
        // Newer fields fall back to defaults.
        assert!(c.close_to_background); // 语句
        assert!(!c.autostart); // 语句
    } // 块结束

    #[test] // 语句
    fn unknown_fields_are_ignored() { // 函数 unknown_fields_are_ignored
        let with_extra = r#"{"folder":null,"recursive":true,"mode":"Fixed",
            "interval_secs":60,"shuffle":false,"fit":"Fill","theme":0,
            "last_path":null,"running":false,"search":"","future_field":123}"#; // 语句
        let c: Config = serde_json::from_str(with_extra).expect("unknown fields tolerated"); // 变量 c
        assert_eq!(c.mode, Mode::Fixed); // 语句
    } // 块结束

    /// Atomic save writes config.json; reloading it preserves values; a second
    /// save rolls the previous file to `.bak`. Uses a temp APPDATA so the real
    /// user config is never touched.
    #[test] // 语句
    fn save_writes_atomically_and_keeps_backup() { // 函数 save_writes_atomically_and_keeps_backup
        let dir = std::env::temp_dir().join(format!("wp_cfg_test_{}", std::process::id())); // 变量 dir
        std::fs::create_dir_all(&dir).unwrap(); // 调用 create_dir_all
        let old = std::env::var_os("APPDATA"); // 变量 old
        std::env::set_var("APPDATA", &dir); // 调用 set_var

        let mut c = Config::default(); // 变量 c
        c.interval_secs = 123; // 语句
        c.close_to_background = false; // 语句
        c.folder = Some(std::path::PathBuf::from("X:/pics")); // 语句
        c.save(); // 语句

        let p = config_path(); // 变量 p
        assert!(p.exists(), "config.json should be written"); // 语句
        assert!(!p.with_extension("json.tmp").exists(), "temp file cleaned up"); // 语句
        let loaded = Config::load(); // 变量 loaded
        assert_eq!(loaded.interval_secs, 123); // 语句
        assert!(!loaded.close_to_background); // 语句
        assert_eq!(loaded.folder.as_deref(), Some(std::path::Path::new("X:/pics"))); // 语句

        // Second save rolls the first good file into .bak.
        c.interval_secs = 456; // 语句
        c.save(); // 语句
        assert!(p.with_extension("json.bak").exists(), "backup should exist"); // 语句
        assert_eq!(Config::load().interval_secs, 456); // 语句

        match old { // 分支匹配
            Some(v) => std::env::set_var("APPDATA", v), // 匹配分支
            None => std::env::remove_var("APPDATA"), // 匹配分支
        } // 块结束
        let _ = std::fs::remove_dir_all(&dir); // 变量 _
    } // 块结束
} // 块结束
