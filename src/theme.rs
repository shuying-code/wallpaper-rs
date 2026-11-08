//! Global look & feel: palette, egui style, gradients, and system CJK font loading.

use eframe::egui; // 引入依赖
use egui::{Color32, Rounding, Stroke}; // 引入依赖
use std::sync::atomic::{AtomicBool, Ordering}; // 引入依赖

static DARK: AtomicBool = AtomicBool::new(false); // 常量 DARK

pub fn is_dark() -> bool { // 函数 is_dark
    DARK.load(Ordering::Relaxed) // 语句
} // 块结束

// ---- Palette ---------------------------------------------------------------
// A warm "editorial" palette: paper-cream surfaces, terracotta accent, deep
// teal secondary. Distinct, calm, and easy on the eyes for a wallpaper viewer.
pub fn bg() -> Color32 { // 函数 bg
    if is_dark() { Color32::from_rgb(0x16, 0x12, 0x10) } else { Color32::from_rgb(0xF6, 0xF1, 0xE9) } // 条件分支
} // 块结束
/// Slightly lighter/darker surface used for the top gradient of the content area.
pub fn bg_top() -> Color32 { // 函数 bg_top
    if is_dark() { Color32::from_rgb(0x1E, 0x18, 0x15) } else { Color32::from_rgb(0xFC, 0xF9, 0xF3) } // 条件分支
} // 块结束
pub fn card() -> Color32 { // 函数 card
    if is_dark() { Color32::from_rgb(0x23, 0x1C, 0x18) } else { Color32::from_rgb(0xFF, 0xFF, 0xFF) } // 条件分支
} // 块结束
/// Sidebar / header surface — a hair off `card()` for gentle separation.
pub fn surface() -> Color32 { // 函数 surface
    if is_dark() { Color32::from_rgb(0x1B, 0x15, 0x12) } else { Color32::from_rgb(0xFB, 0xF7, 0xF0) } // 条件分支
} // 块结束
pub fn border() -> Color32 { // 函数 border
    if is_dark() { Color32::from_rgb(0x39, 0x2F, 0x28) } else { Color32::from_rgb(0xE8, 0xDF, 0xD1) } // 条件分支
} // 块结束
pub fn text() -> Color32 { // 函数 text
    if is_dark() { Color32::from_rgb(0xF4, 0xEE, 0xE5) } else { Color32::from_rgb(0x21, 0x1B, 0x15) } // 条件分支
} // 块结束
pub fn subtext() -> Color32 { // 函数 subtext
    if is_dark() { Color32::from_rgb(0xAB, 0x9E, 0x8F) } else { Color32::from_rgb(0x7C, 0x71, 0x64) } // 条件分支
} // 块结束
pub fn extreme() -> Color32 { // 函数 extreme
    if is_dark() { Color32::from_rgb(0x11, 0x0D, 0x0B) } else { Color32::from_rgb(0xF0, 0xE9, 0xDD) } // 条件分支
} // 块结束
pub fn accent() -> Color32 { // 函数 accent
    if is_dark() { Color32::from_rgb(0xFF, 0x7E, 0x54) } else { Color32::from_rgb(0xD4, 0x4A, 0x27) } // 条件分支
} // 块结束
pub fn accent_soft() -> Color32 { // 函数 accent_soft
    if is_dark() { Color32::from_rgb(0x3E, 0x24, 0x1B) } else { Color32::from_rgb(0xFB, 0xE6, 0xDA) } // 条件分支
} // 块结束
/// Secondary accent (teal) for highlights / gradient end points.
pub fn accent2() -> Color32 { // 函数 accent2
    if is_dark() { Color32::from_rgb(0x3D, 0xC9, 0xAC) } else { Color32::from_rgb(0x0E, 0x7C, 0x6B) } // 条件分支
} // 块结束
pub fn ok() -> Color32 { // 函数 ok
    if is_dark() { Color32::from_rgb(0x43, 0xD1, 0x8B) } else { Color32::from_rgb(0x12, 0x93, 0x6A) } // 条件分支
} // 块结束
pub fn shadow() -> Color32 { // 函数 shadow
    if is_dark() { Color32::from_rgba_unmultiplied(0, 0, 0, 160) } else { Color32::from_rgba_unmultiplied(0x3A, 0x24, 0x14, 26) } // 条件分支
} // 块结束

// ---- Style -----------------------------------------------------------------
pub fn apply(ctx: &egui::Context, dark: bool) { // 函数 apply
    DARK.store(dark, Ordering::Relaxed); // 语句
    install_fonts(ctx); // 调用 install_fonts

    let mut style = (*ctx.style()).clone(); // 变量 style
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() }; // 变量 v
    v.panel_fill = bg(); // 语句
    v.window_fill = card(); // 语句
    v.extreme_bg_color = extreme(); // 语句
    v.faint_bg_color = extreme(); // 语句
    v.override_text_color = Some(text()); // 语句
    v.hyperlink_color = accent(); // 语句
    v.striped = false; // 语句

    v.widgets.noninteractive.bg_fill = card(); // 语句
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border()); // 语句
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text()); // 语句
    v.widgets.noninteractive.rounding = Rounding::same(10.0); // 语句

    v.widgets.inactive.bg_fill = extreme(); // 语句
    v.widgets.inactive.weak_bg_fill = extreme(); // 语句
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border()); // 语句
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text()); // 语句
    v.widgets.inactive.rounding = Rounding::same(12.0); // 语句

    v.widgets.hovered.bg_fill = accent_soft(); // 语句
    v.widgets.hovered.weak_bg_fill = accent_soft(); // 语句
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent()); // 语句
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, text()); // 语句
    v.widgets.hovered.rounding = Rounding::same(12.0); // 语句
    v.widgets.hovered.expansion = 1.0; // 语句

    v.widgets.active.bg_fill = accent_soft(); // 语句
    v.widgets.active.weak_bg_fill = accent_soft(); // 语句
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent()); // 语句
    v.widgets.active.fg_stroke = Stroke::new(1.2_f32, text()); // 语句
    v.widgets.active.rounding = Rounding::same(12.0); // 语句

    v.selection.bg_fill = accent_soft(); // 语句
    v.selection.stroke = Stroke::new(1.0_f32, accent()); // 语句

    v.window_rounding = Rounding::same(18.0); // 语句
    v.menu_rounding = Rounding::same(14.0); // 语句
    v.window_shadow = egui::epaint::Shadow { // 语句
        offset: egui::vec2(0.0, 8.0), // 字段 offset
        blur: 28.0, // 字段 blur
        spread: 0.0, // 字段 spread
        color: shadow(), // 字段 color
    }; // 块结束
    v.popup_shadow = egui::epaint::Shadow { // 语句
        offset: egui::vec2(0.0, 6.0), // 字段 offset
        blur: 18.0, // 字段 blur
        spread: 0.0, // 字段 spread
        color: shadow(), // 字段 color
    }; // 块结束

    style.visuals = v; // 语句
    style.spacing.item_spacing = egui::vec2(10.0, 10.0); // 语句
    style.spacing.button_padding = egui::vec2(15.0, 8.0); // 语句
    style.spacing.interact_size.y = 34.0; // 语句
    style.spacing.window_margin = egui::Margin::same(14.0); // 语句
    ctx.set_style(style); // 语句
} // 块结束

/// Paint a very subtle vertical gradient behind the central content.
pub fn paint_bg_gradient(ui: &egui::Ui, rect: egui::Rect) { // 函数 paint_bg_gradient
    let top = bg_top(); // 变量 top
    let bottom = bg(); // 变量 bottom
    let mut mesh = egui::Mesh::default(); // 变量 mesh
    mesh.colored_vertex(rect.left_top(), top); // 语句
    mesh.colored_vertex(rect.right_top(), top); // 语句
    mesh.colored_vertex(rect.left_bottom(), bottom); // 语句
    mesh.colored_vertex(rect.right_bottom(), bottom); // 语句
    mesh.add_triangle(0, 1, 2); // 语句
    mesh.add_triangle(1, 3, 2); // 语句
    ui.painter().add(egui::Shape::mesh(mesh)); // 语句
} // 块结束

fn install_fonts(ctx: &egui::Context) { // 函数 install_fonts
    let mut fonts = egui::FontDefinitions::default(); // 变量 fonts
    if let Some((name, bytes)) = load_system_cjk() { // 条件分支
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes)); // 语句
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) { // 条件分支
            f.insert(0, name.clone()); // 语句
        } // 块结束
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Monospace) { // 条件分支
            f.push(name); // 语句
        } // 块结束
    } // 块结束
    ctx.set_fonts(fonts); // 语句
} // 块结束

/// Find a usable system CJK font so Chinese text never renders as tofu boxes.
fn load_system_cjk() -> Option<(String, Vec<u8>)> { // 函数 load_system_cjk
    #[cfg(windows)] // 语句
    let candidates = [ // 变量 candidates
        "C:\\Windows\\Fonts\\simhei.ttf", // 语句
        "C:\\Windows\\Fonts\\Deng.ttf", // 语句
        "C:\\Windows\\Fonts\\msyh.ttc", // 语句
        "C:\\Windows\\Fonts\\simsun.ttc", // 语句
    ]; // 语句
    #[cfg(target_os = "macos")] // 语句
    let candidates = [ // 变量 candidates
        "/System/Library/Fonts/PingFang.ttc", // 语句
        "/System/Library/Fonts/STHeiti Medium.ttc", // 语句
        "/Library/Fonts/Arial Unicode.ttf", // 语句
        "/System/Library/Fonts/Hiragino Sans GB.ttc", // 语句
    ]; // 语句
    #[cfg(target_os = "android")] // 语句
    let candidates = [ // 变量 candidates
        "/system/fonts/NotoSansCJK-Regular.ttc", // 语句
        "/system/fonts/NotoSansCJKsc-Regular.otf", // 语句
        "/system/fonts/NotoSansSC-Regular.otf", // 语句
        "/system/fonts/NotoSansSC-VF.ttf", // 语句
        "/system/fonts/NotoSansHans-Regular.otf", // 语句
        "/system/fonts/DroidSansFallback.ttf", // 语句
        "/system/fonts/DroidSansChinese.ttf", // 语句
        "/system/fonts/DroidSansFallbackFull.ttf", // 语句
    ]; // 语句
    #[cfg(target_os = "ios")] // 语句
    let candidates = [ // 变量 candidates
        "/System/Library/Fonts/PingFang.ttc", // 语句
        "/System/Library/Fonts/STHeiti Medium.ttc", // 语句
    ]; // 语句
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))] // 语句
    let candidates = [ // 变量 candidates
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // 语句
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", // 语句
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", // 语句
        "/usr/share/fonts/truetype/arphic/uming.ttc", // 语句
    ]; // 语句

    for (i, p) in candidates.iter().enumerate() { // 循环
        if let Ok(bytes) = std::fs::read(p) { // 条件分支
            return Some((format!("cjk-{i}"), bytes)); // 返回
        } // 块结束
    } // 块结束
    None // 语句
} // 块结束
