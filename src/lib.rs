//! wallpaper-rs library root.
//!
//! Shared by the desktop binary (`src/main.rs`) and the Android `cdylib`
//! (`src/android.rs`). Keeping the UI/logic in a library crate is what lets the
//! same code be packaged as a desktop app and as an Android APK via `cargo-apk`.

pub mod albums;
pub mod app;
pub mod autostart;
pub mod config;
pub mod diskscan;
pub mod elevate;
pub mod hotkey;
pub mod instances;
pub mod online;
pub mod permissions;
pub mod platform;
pub mod shot;
pub mod theme;
pub mod thumbs;
pub mod tray;
pub mod wallpaper;

#[cfg(windows)]
pub mod winhide;

#[cfg(target_os = "android")]
mod android;

pub use eframe::egui;

/// Platform-aware native window options (desktop window vs. full-screen mobile).
pub fn native_options() -> eframe::NativeOptions {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_title("Wallpaper Studio"),
            ..Default::default()
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1080.0, 720.0])
                .with_min_inner_size([360.0, 560.0])
                .with_title("Wallpaper Studio · 桌面壁纸"),
            ..Default::default()
        }
    }
}

/// Run the desktop app (used by the `wallpaper-rs` binary).
pub fn run_desktop() -> eframe::Result<()> {
    let native = native_options();
    eframe::run_native(
        "wallpaper-rs",
        native,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
