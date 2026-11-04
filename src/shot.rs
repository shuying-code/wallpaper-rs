//! Built-in "screen-size adaptation screenshot" feature.
//!
//! Launch with `--shots <dir>` and the app will, for a set of device sizes ×
//! tabs, resize its own window, render the real UI, grab the framebuffer via
//! `egui::ViewportCommand::Screenshot`, save a PNG, then exit. This makes it
//! easy to review how the responsive layout looks on phone / tablet / desktop
//! without manually resizing the window.

use eframe::egui; // 引入依赖
use std::path::{Path, PathBuf}; // 引入依赖
use std::sync::Arc; // 引入依赖

/// Frames to let the window finish resizing (and thumbnails/textures settle)
/// before requesting the screenshot.
pub const SETTLE_FRAMES: u32 = 14; // 常量 SETTLE_FRAMES

pub struct ShotItem { // 结构体 ShotItem
    pub label: String, // 字段 label
    /// Tab index: 0 Library, 1 Slideshow, 2 Online, 3 Settings, 4 About.
    pub tab: usize, // 字段 tab
    pub w: f32, // 字段 w
    pub h: f32, // 字段 h
} // 块结束

pub struct Shooter { // 结构体 Shooter
    pub dir: PathBuf, // 字段 dir
    pub items: Vec<ShotItem>, // 字段 items
    pub idx: usize, // 字段 idx
    /// Frames elapsed since the current item's size was applied.
    pub settle: u32, // 字段 settle
    /// Whether a screenshot request is in flight (waiting for the event).
    pub pending: bool, // 字段 pending
    pub saved: Vec<PathBuf>, // 字段 saved
} // 块结束

impl Shooter { // 实现
    pub fn new(dir: PathBuf) -> Self { // 函数 new
        let sizes: [(&str, f32, f32); 3] = // 变量 sizes
            [("phone", 390.0, 844.0), ("tablet", 834.0, 1112.0), ("desktop", 1440.0, 900.0)]; // 语句
        let tabs: [(&str, usize); 4] = // 变量 tabs
            [("library", 0), ("slideshow", 1), ("settings", 3), ("about", 4)]; // 语句
        let mut items = Vec::new(); // 变量 items
        for (sname, w, h) in sizes { // 循环
            for (tname, tab) in tabs.iter().copied() { // 循环
                items.push(ShotItem { // 语句
                    label: format!("{sname}-{tname}"), // 字段 label
                    tab, // 语句
                    w, // 语句
                    h, // 语句
                }); // 语句
            } // 块结束
        } // 块结束
        Self { // 语句
            dir, // 语句
            items, // 语句
            idx: 0, // 字段 idx
            settle: 0, // 字段 settle
            pending: false, // 字段 pending
            saved: Vec::new(), // 字段 saved
        } // 块结束
    } // 块结束

    /// Built only when the `WP_SHOT_DIR` env var is set (see `main.rs`).
    pub fn from_env() -> Option<Self> { // 函数 from_env
        let dir = std::env::var_os("WP_SHOT_DIR")?; // 变量 dir
        Some(Self::new(PathBuf::from(dir))) // 调用 Some
    } // 块结束

    pub fn current(&self) -> Option<&ShotItem> { // 函数 current
        self.items.get(self.idx) // 语句
    } // 块结束

    pub fn finished(&self) -> bool { // 函数 finished
        self.idx >= self.items.len() // 语句
    } // 块结束

    /// Save the captured frame as `<label>.png`, advancing to the next item.
    pub fn store(&mut self, image: &egui::ColorImage) -> Option<PathBuf> { // 函数 store
        let item = self.items.get(self.idx)?; // 变量 item
        let path = self.dir.join(format!("{}.png", item.label)); // 变量 path
        let [w, h] = image.size; // 语句
        let mut bytes = Vec::with_capacity(w * h * 4); // 变量 bytes
        for px in &image.pixels { // 循环
            bytes.extend_from_slice(&[px.r(), px.g(), px.b(), px.a()]); // 语句
        } // 块结束
        let ok = image::save_buffer( // 变量 ok
            &path, // 语句
            &bytes, // 语句
            w as u32, // 语句
            h as u32, // 语句
            image::ColorType::Rgba8, // 字段 image
        ) // 块结束
        .is_ok(); // 语句
        self.idx += 1; // 语句
        self.settle = 0; // 语句
        self.pending = false; // 语句
        if ok { // 条件分支
            self.saved.push(path.clone()); // 语句
            Some(path) // 调用 Some
        } else { // 语句
            None // 语句
        } // 块结束
    } // 块结束

    /// Write a small text index of everything that was captured.
    pub fn write_manifest(&self) { // 函数 write_manifest
        let mut lines = vec!["wallpaper-rs adaptation shots".to_string()]; // 变量 lines
        for it in &self.items { // 循环
            lines.push(format!("{}  {}x{}", it.label, it.w, it.h)); // 语句
        } // 块结束
        let _ = std::fs::write(self.dir.join("shots.txt"), lines.join("\n")); // 变量 _
    } // 块结束
} // 块结束

/// Find a screenshot image among this frame's input events.
pub fn take_screenshot(ctx: &egui::Context) -> Option<Arc<egui::ColorImage>> { // 函数 take_screenshot
    ctx.input(|i| { // 语句
        i.events.iter().find_map(|e| match e { // 语句
            egui::Event::Screenshot { image, .. } => Some(image.clone()), // 匹配分支
            _ => None, // 匹配分支
        }) // 语句
    }) // 语句
} // 块结束

pub fn ensure_dir(dir: &Path) { // 函数 ensure_dir
    let _ = std::fs::create_dir_all(dir); // 变量 _
} // 块结束
