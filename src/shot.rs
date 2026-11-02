//! Built-in "screen-size adaptation screenshot" feature.
//!
//! Launch with `--shots <dir>` and the app will, for a set of device sizes ×
//! tabs, resize its own window, render the real UI, grab the framebuffer via
//! `egui::ViewportCommand::Screenshot`, save a PNG, then exit. This makes it
//! easy to review how the responsive layout looks on phone / tablet / desktop
//! without manually resizing the window.

use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Frames to let the window finish resizing (and thumbnails/textures settle)
/// before requesting the screenshot.
pub const SETTLE_FRAMES: u32 = 14;

pub struct ShotItem {
    pub label: String,
    /// Tab index: 0 Library, 1 Slideshow, 2 Online, 3 Settings, 4 About.
    pub tab: usize,
    pub w: f32,
    pub h: f32,
}

pub struct Shooter {
    pub dir: PathBuf,
    pub items: Vec<ShotItem>,
    pub idx: usize,
    /// Frames elapsed since the current item's size was applied.
    pub settle: u32,
    /// Whether a screenshot request is in flight (waiting for the event).
    pub pending: bool,
    pub saved: Vec<PathBuf>,
}

impl Shooter {
    pub fn new(dir: PathBuf) -> Self {
        let sizes: [(&str, f32, f32); 3] =
            [("phone", 390.0, 844.0), ("tablet", 834.0, 1112.0), ("desktop", 1440.0, 900.0)];
        let tabs: [(&str, usize); 4] =
            [("library", 0), ("slideshow", 1), ("settings", 3), ("about", 4)];
        let mut items = Vec::new();
        for (sname, w, h) in sizes {
            for (tname, tab) in tabs.iter().copied() {
                items.push(ShotItem {
                    label: format!("{sname}-{tname}"),
                    tab,
                    w,
                    h,
                });
            }
        }
        Self {
            dir,
            items,
            idx: 0,
            settle: 0,
            pending: false,
            saved: Vec::new(),
        }
    }

    /// Built only when the `WP_SHOT_DIR` env var is set (see `main.rs`).
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os("WP_SHOT_DIR")?;
        Some(Self::new(PathBuf::from(dir)))
    }

    pub fn current(&self) -> Option<&ShotItem> {
        self.items.get(self.idx)
    }

    pub fn finished(&self) -> bool {
        self.idx >= self.items.len()
    }

    /// Save the captured frame as `<label>.png`, advancing to the next item.
    pub fn store(&mut self, image: &egui::ColorImage) -> Option<PathBuf> {
        let item = self.items.get(self.idx)?;
        let path = self.dir.join(format!("{}.png", item.label));
        let [w, h] = image.size;
        let mut bytes = Vec::with_capacity(w * h * 4);
        for px in &image.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b(), px.a()]);
        }
        let ok = image::save_buffer(
            &path,
            &bytes,
            w as u32,
            h as u32,
            image::ColorType::Rgba8,
        )
        .is_ok();
        self.idx += 1;
        self.settle = 0;
        self.pending = false;
        if ok {
            self.saved.push(path.clone());
            Some(path)
        } else {
            None
        }
    }

    /// Write a small text index of everything that was captured.
    pub fn write_manifest(&self) {
        let mut lines = vec!["wallpaper-rs adaptation shots".to_string()];
        for it in &self.items {
            lines.push(format!("{}  {}x{}", it.label, it.w, it.h));
        }
        let _ = std::fs::write(self.dir.join("shots.txt"), lines.join("\n"));
    }
}

/// Find a screenshot image among this frame's input events.
pub fn take_screenshot(ctx: &egui::Context) -> Option<Arc<egui::ColorImage>> {
    ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    })
}

pub fn ensure_dir(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
}
