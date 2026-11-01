//! Global look & feel: palette, egui style, gradients, and system CJK font loading.

use eframe::egui;
use egui::{Color32, Rounding, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(false);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

// ---- Palette ---------------------------------------------------------------
pub fn bg() -> Color32 {
    if is_dark() { Color32::from_rgb(0x11, 0x15, 0x1E) } else { Color32::from_rgb(0xF5, 0xF7, 0xFB) }
}
/// Slightly lighter/darker surface used for the top gradient of the content area.
pub fn bg_top() -> Color32 {
    if is_dark() { Color32::from_rgb(0x18, 0x1E, 0x2B) } else { Color32::from_rgb(0xFB, 0xFC, 0xFF) }
}
pub fn card() -> Color32 {
    if is_dark() { Color32::from_rgb(0x1B, 0x22, 0x30) } else { Color32::from_rgb(0xFF, 0xFF, 0xFF) }
}
/// Sidebar / header surface — a hair off `card()` for gentle separation.
pub fn surface() -> Color32 {
    if is_dark() { Color32::from_rgb(0x16, 0x1C, 0x28) } else { Color32::from_rgb(0xFA, 0xFB, 0xFE) }
}
pub fn border() -> Color32 {
    if is_dark() { Color32::from_rgb(0x2A, 0x33, 0x45) } else { Color32::from_rgb(0xE5, 0xEA, 0xF2) }
}
pub fn text() -> Color32 {
    if is_dark() { Color32::from_rgb(0xEA, 0xEF, 0xF7) } else { Color32::from_rgb(0x17, 0x1B, 0x26) }
}
pub fn subtext() -> Color32 {
    if is_dark() { Color32::from_rgb(0x96, 0xA1, 0xB7) } else { Color32::from_rgb(0x67, 0x6E, 0x7D) }
}
pub fn extreme() -> Color32 {
    if is_dark() { Color32::from_rgb(0x0E, 0x12, 0x1A) } else { Color32::from_rgb(0xEC, 0xF0, 0xF7) }
}
pub fn accent() -> Color32 {
    if is_dark() { Color32::from_rgb(0x6B, 0x92, 0xFF) } else { Color32::from_rgb(0x2E, 0x6B, 0xF0) }
}
pub fn accent_soft() -> Color32 {
    if is_dark() { Color32::from_rgb(0x22, 0x30, 0x4E) } else { Color32::from_rgb(0xE3, 0xEC, 0xFF) }
}
/// Secondary accent (teal) for highlights / gradient end points.
pub fn accent2() -> Color32 {
    if is_dark() { Color32::from_rgb(0x2B, 0xD4, 0xA0) } else { Color32::from_rgb(0x0E, 0xB5, 0x8B) }
}
pub fn ok() -> Color32 {
    if is_dark() { Color32::from_rgb(0x35, 0xD0, 0x7F) } else { Color32::from_rgb(0x12, 0xB7, 0x6A) }
}
pub fn shadow() -> Color32 {
    if is_dark() { Color32::from_rgba_unmultiplied(0, 0, 0, 150) } else { Color32::from_rgba_unmultiplied(0x1B, 0x26, 0x44, 22) }
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
    v.hyperlink_color = accent();
    v.striped = false;

    v.widgets.noninteractive.bg_fill = card();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border());
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text());
    v.widgets.noninteractive.rounding = Rounding::same(8.0);

    v.widgets.inactive.bg_fill = extreme();
    v.widgets.inactive.weak_bg_fill = extreme();
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border());
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text());
    v.widgets.inactive.rounding = Rounding::same(9.0);

    v.widgets.hovered.bg_fill = accent_soft();
    v.widgets.hovered.weak_bg_fill = accent_soft();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, text());
    v.widgets.hovered.rounding = Rounding::same(9.0);
    v.widgets.hovered.expansion = 1.0;

    v.widgets.active.bg_fill = accent_soft();
    v.widgets.active.weak_bg_fill = accent_soft();
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent());
    v.widgets.active.fg_stroke = Stroke::new(1.2_f32, text());
    v.widgets.active.rounding = Rounding::same(9.0);

    v.selection.bg_fill = accent_soft();
    v.selection.stroke = Stroke::new(1.0_f32, accent());

    v.window_rounding = Rounding::same(14.0);
    v.menu_rounding = Rounding::same(12.0);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 28.0,
        spread: 0.0,
        color: shadow(),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: shadow(),
    };

    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(9.0, 9.0);
    style.spacing.button_padding = egui::vec2(13.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.window_margin = egui::Margin::same(12.0);
    ctx.set_style(style);
}

/// Paint a very subtle vertical gradient behind the central content.
pub fn paint_bg_gradient(ui: &egui::Ui, rect: egui::Rect) {
    let top = bg_top();
    let bottom = bg();
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
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
