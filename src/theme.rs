//! Global look & feel: palette, egui style, and system CJK font loading.

use eframe::egui;
use egui::{Color32, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(false);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

// ---- Palette ---------------------------------------------------------------
pub fn bg() -> Color32 {
    if is_dark() { Color32::from_rgb(0x14, 0x18, 0x21) } else { Color32::from_rgb(0xF4, 0xF6, 0xFA) }
}
pub fn card() -> Color32 {
    if is_dark() { Color32::from_rgb(0x1C, 0x22, 0x30) } else { Color32::from_rgb(0xFF, 0xFF, 0xFF) }
}
pub fn border() -> Color32 {
    if is_dark() { Color32::from_rgb(0x2A, 0x32, 0x44) } else { Color32::from_rgb(0xE3, 0xE8, 0xF0) }
}
pub fn text() -> Color32 {
    if is_dark() { Color32::from_rgb(0xE8, 0xEC, 0xF4) } else { Color32::from_rgb(0x1B, 0x1F, 0x2A) }
}
pub fn subtext() -> Color32 {
    if is_dark() { Color32::from_rgb(0x97, 0xA0, 0xB5) } else { Color32::from_rgb(0x6B, 0x72, 0x80) }
}
pub fn extreme() -> Color32 {
    if is_dark() { Color32::from_rgb(0x0F, 0x13, 0x1B) } else { Color32::from_rgb(0xED, 0xF1, 0xF7) }
}
pub fn accent() -> Color32 {
    if is_dark() { Color32::from_rgb(0x5B, 0x8C, 0xFF) } else { Color32::from_rgb(0x2E, 0x6B, 0xF0) }
}
pub fn accent_soft() -> Color32 {
    if is_dark() { Color32::from_rgb(0x22, 0x30, 0x4E) } else { Color32::from_rgb(0xE4, 0xED, 0xFF) }
}
pub fn ok() -> Color32 {
    if is_dark() { Color32::from_rgb(0x35, 0xD0, 0x7F) } else { Color32::from_rgb(0x12, 0xB7, 0x6A) }
}
// ---- Style -----------------------------------------------------------------
pub fn apply(ctx: &egui::Context, dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    install_fonts(ctx);

    let mut style = (*ctx.style()).clone();
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = bg();
    v.window_fill = card();
    v.extreme_bg_color = extreme();
    v.faint_bg_color = extreme();
    v.override_text_color = Some(text());
    v.widgets.noninteractive.bg_fill = card();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    v.widgets.inactive.bg_fill = extreme();
    v.widgets.inactive.weak_bg_fill = extreme();
    v.widgets.hovered.bg_fill = accent_soft();
    v.widgets.hovered.weak_bg_fill = accent_soft();
    v.widgets.active.bg_fill = accent_soft();
    v.selection.bg_fill = accent_soft();
    v.selection.stroke = Stroke::new(1.0_f32, accent());
    v.hyperlink_color = accent();
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    ctx.set_style(style);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some((name, bytes)) = load_system_cjk() {
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, name.clone());
        }
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            f.push(name);
        }
    }
    ctx.set_fonts(fonts);
}

/// Find a usable system CJK font so Chinese text never renders as tofu boxes.
fn load_system_cjk() -> Option<(String, Vec<u8>)> {
    #[cfg(windows)]
    let candidates = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    #[cfg(target_os = "macos")]
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let candidates = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
    ];

    for (i, p) in candidates.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(p) {
            return Some((format!("cjk-{i}"), bytes));
        }
    }
    None
}
