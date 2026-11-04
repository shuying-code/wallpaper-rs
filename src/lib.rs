//! wallpaper-rs library root.
//!
//! Shared by the desktop binary (`src/main.rs`) and the Android `cdylib`
//! (`src/android.rs`). Keeping the UI/logic in a library crate is what lets the
//! same code be packaged as a desktop app and as an Android APK via `cargo-apk`.

pub mod albums; // 模块 albums
pub mod app; // 模块 app
pub mod autostart; // 模块 autostart
pub mod config; // 模块 config
pub mod diskscan; // 模块 diskscan
pub mod elevate; // 模块 elevate
pub mod hotkey; // 模块 hotkey
pub mod instances; // 模块 instances
pub mod online; // 模块 online
pub mod permissions; // 模块 permissions
pub mod platform; // 模块 platform
pub mod shot; // 模块 shot
pub mod theme; // 模块 theme
pub mod thumbs; // 模块 thumbs
pub mod tray; // 模块 tray
pub mod wallpaper; // 模块 wallpaper

#[cfg(windows)] // 语句
pub mod winhide; // 模块 winhide

#[cfg(target_os = "android")] // 语句
mod android; // 模块 android

pub use eframe::egui; // 语句

/// Platform-aware native window options (desktop window vs. full-screen mobile).
pub fn native_options() -> eframe::NativeOptions { // 函数 native_options
    #[cfg(any(target_os = "android", target_os = "ios"))] // 语句
    { // 语句
        eframe::NativeOptions { // 语句
            viewport: egui::ViewportBuilder::default().with_title("Wallpaper Studio"), // 字段 viewport
            ..Default::default() // 语句
        } // 块结束
    } // 块结束
    #[cfg(not(any(target_os = "android", target_os = "ios")))] // 语句
    { // 语句
        eframe::NativeOptions { // 语句
            viewport: egui::ViewportBuilder::default() // 语句
                .with_inner_size([1080.0, 720.0]) // 语句
                .with_min_inner_size([360.0, 560.0]) // 语句
                .with_title("Wallpaper Studio · 桌面壁纸"), // 语句
            ..Default::default() // 语句
        } // 块结束
    } // 块结束
} // 块结束

/// Run the desktop app (used by the `wallpaper-rs` binary).
pub fn run_desktop() -> eframe::Result<()> { // 函数 run_desktop
    let native = native_options(); // 变量 native
    eframe::run_native( // 调用 run_native
        "wallpaper-rs", // 语句
        native, // 语句
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))), // 字段 Box
    ) // 块结束
} // 块结束
