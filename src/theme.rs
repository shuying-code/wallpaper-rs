//! 全局视觉令牌：调色板、egui 样式、渐变与系统中文字体加载。
//!
//! v2 视觉语言（冷调中性 + 靛蓝强调）：
//! - 壁纸是第一公民，所以界面底色用低饱和的 porcelain / 近黑，不带暖色调，
//!   不与壁纸抢视觉；彩色只留给「状态」（在场 / 警告 / 危险）与灯效预览。
//! - 三种语义色固定不变：薄荷绿 = 在场 / 成功，琥珀 = 陌生人 / 警告，
//!   朱红 = 危险 / 失败。其余全部是中性灰阶。
//! - 所有取色函数保持原有签名，壁纸页无需改动即可继承新配色。

use eframe::egui;
use egui::{Color32, Rounding, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(false);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

// ---- Palette ---------------------------------------------------------------

/// 内容区底色（porcelain）。
pub fn bg() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x0B, 0x0D, 0x11)
    } else {
        Color32::from_rgb(0xF2, 0xF3, 0xF6)
    }
}

/// 内容区顶部渐变的浅端（几乎与 `bg()` 同色，只做极轻的层次）。
pub fn bg_top() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x10, 0x13, 0x19)
    } else {
        Color32::from_rgb(0xF8, 0xF9, 0xFB)
    }
}

/// 卡片面。
pub fn card() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x12, 0x16, 0x1C)
    } else {
        Color32::from_rgb(0xFF, 0xFF, 0xFF)
    }
}

/// 顶栏 / 图标栏 / 底部导航面。
pub fn surface() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x0E, 0x11, 0x16)
    } else {
        Color32::from_rgb(0xFF, 0xFF, 0xFF)
    }
}

/// 1px 描边 / 分隔线。
pub fn border() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x23, 0x2A, 0x33)
    } else {
        Color32::from_rgb(0xE3, 0xE6, 0xEC)
    }
}

/// 控件描边 / 关闭态开关描边（比 `border()` 深一档）。
pub fn border2() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x2E, 0x37, 0x43)
    } else {
        Color32::from_rgb(0xD3, 0xD8, 0xE1)
    }
}

pub fn text() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xEE, 0xF1, 0xF5)
    } else {
        Color32::from_rgb(0x10, 0x14, 0x19)
    }
}

/// 次文字 / 侧栏图标。
pub fn subtext() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x98, 0xA2, 0xB0)
    } else {
        Color32::from_rgb(0x6B, 0x74, 0x80)
    }
}

/// 说明文字 / 坐标轴标签。
pub fn faint() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x6C, 0x76, 0x83)
    } else {
        Color32::from_rgb(0x9A, 0xA3, 0xAF)
    }
}

/// 输入框底 / 分段控件底 / 悬停底。
pub fn extreme() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x1A, 0x20, 0x28)
    } else {
        Color32::from_rgb(0xED, 0xEF, 0xF3)
    }
}

/// 主强调色（靛蓝）。
pub fn accent() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x6E, 0x8C, 0xFF)
    } else {
        Color32::from_rgb(0x2F, 0x4B, 0xFF)
    }
}

/// 主强调色的浅底（选中态 / 轻色区块）。
pub fn accent_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x1A, 0x23, 0x40)
    } else {
        Color32::from_rgb(0xE8, 0xED, 0xFF)
    }
}

/// 主强调色的深端（渐变末端 / 暗色高亮文字）。
pub fn accent_deep() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xA9, 0xBC, 0xFF)
    } else {
        Color32::from_rgb(0x1E, 0x33, 0xC4)
    }
}

/// 次要强调色：薄荷绿，与 `ok()` 同源，用于壁纸页渐变末端。
pub fn accent2() -> Color32 {
    mint()
}

/// 语义色 · 在场 / 成功 / 自动解锁。
pub fn mint() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x3E, 0xD9, 0xA4)
    } else {
        Color32::from_rgb(0x0E, 0x9F, 0x6E)
    }
}

pub fn mint_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x12, 0x2B, 0x24)
    } else {
        Color32::from_rgb(0xE6, 0xF6, 0xF0)
    }
}

pub fn ok() -> Color32 {
    mint()
}

/// 语义色 · 陌生人 / 警告 / 仅提醒。
pub fn amber() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xE5, 0xA5, 0x4B)
    } else {
        Color32::from_rgb(0xB8, 0x72, 0x0B)
    }
}

pub fn amber_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x2A, 0x21, 0x17)
    } else {
        Color32::from_rgb(0xFB, 0xF0, 0xDC)
    }
}

/// 语义色 · 失败 / 立即锁定 / 危险操作。
pub fn danger() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0xF2, 0x6D, 0x62)
    } else {
        Color32::from_rgb(0xD3, 0x38, 0x2F)
    }
}

pub fn danger_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x2A, 0x1A, 0x18)
    } else {
        Color32::from_rgb(0xFB, 0xEA, 0xE8)
    }
}

/// 摄像头画面 / 灯效预览用的暗场底（渐变两端）。
pub fn video1() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x1A, 0x22, 0x31)
    } else {
        Color32::from_rgb(0x1D, 0x25, 0x34)
    }
}

pub fn video2() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x08, 0x0A, 0x0F)
    } else {
        Color32::from_rgb(0x0A, 0x0D, 0x13)
    }
}

/// 画面下缘压暗用的半透明遮罩。
pub fn scrim() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(4, 6, 9, 204)
    } else {
        Color32::from_rgba_unmultiplied(8, 10, 14, 184)
    }
}

pub fn shadow() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(0, 0, 0, 170)
    } else {
        Color32::from_rgba_unmultiplied(0x10, 0x18, 0x28, 22)
    }
}

// ---- Style -----------------------------------------------------------------

pub fn apply(ctx: &egui::Context, dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
    install_fonts(ctx);

    let mut style = (*ctx.style()).clone();
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
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
    v.widgets.noninteractive.rounding = Rounding::same(9.0);

    v.widgets.inactive.bg_fill = extreme();
    v.widgets.inactive.weak_bg_fill = extreme();
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border());
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text());
    v.widgets.inactive.rounding = Rounding::same(9.0);

    v.widgets.hovered.bg_fill = accent_soft();
    v.widgets.hovered.weak_bg_fill = accent_soft();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, border2());
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

    v.window_rounding = Rounding::same(16.0);
    v.menu_rounding = Rounding::same(12.0);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 26.0,
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
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.window_margin = egui::Margin::same(14.0);
    ctx.set_style(style);
}

/// 内容区背后一层极轻的垂直渐变。
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
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_owned(bytes));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, name.clone());
        }
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            f.push(name);
        }
    }
    // emoji 字体保留在末尾，图标（◱ ⚙ ✳ 等）即使缺字形也有兜底。
    ctx.set_fonts(fonts);
}

/// 找一个可用的系统中文字体，避免中文渲染成方框。
fn load_system_cjk() -> Option<(String, Vec<u8>)> {
    #[cfg(windows)]
    let candidates = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    #[cfg(target_os = "macos")]
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ];
    #[cfg(target_os = "android")]
    let candidates = [
        "/system/fonts/NotoSansCJK-Regular.ttc",
        "/system/fonts/NotoSansCJKsc-Regular.otf",
        "/system/fonts/NotoSansSC-Regular.otf",
        "/system/fonts/NotoSansSC-VF.ttf",
        "/system/fonts/NotoSansHans-Regular.otf",
        "/system/fonts/DroidSansFallback.ttf",
        "/system/fonts/DroidSansChinese.ttf",
        "/system/fonts/DroidSansFallbackFull.ttf",
    ];
    #[cfg(target_os = "ios")]
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
    ];
    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
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
