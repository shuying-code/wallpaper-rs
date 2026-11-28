//! 智能感知 / 锁屏策略两页的共享上下文与小部件。

pub mod monitor;
pub mod policy;

use crate::theme;
use crate::ui_kit;
use eframe::egui;
use egui::{Align, Color32, Id, Layout, Response, RichText, Stroke, Ui, Vec2};

/// 两页共用的上下文（只读快照 + 可写配置 + 动作队列）。
pub struct SenseCtx<'a> {
    pub view: &'a crate::pages::SenseView,
    pub cfg: &'a mut crate::config::SenseCfg,
    pub actions: &'a mut Vec<crate::pages::PageAction>,
    pub compact: bool,
}

/// 「离开判定」分段组的候选秒数，顺序与 UI 一致。
pub const AWAY_STEPS: [u32; 4] = [10, 30, 60, 180];

/// 双栏骨架：宽屏左右分栏，窄屏单列堆叠。
///
/// `body` 会被调用两次（先左栏，后右栏）；`right` 为真时画右栏。
/// 之所以传一个闭包而不是两个，是为了让调用方只捕获一次 `&mut SenseCtx`。
pub fn two_col(
    ui: &mut Ui,
    compact: bool,
    ratio: f32,
    gap: f32,
    mut body: impl FnMut(&mut Ui, bool),
) -> bool {
    if !compact && ui.available_width() > 980.0 {
        let w = ui.available_width();
        let left_w = ((w - gap) * ratio).max(260.0);
        let right_w = (w - gap - left_w).max(260.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            // `set_width` 是必须的：top_down 子 Ui 否则会收缩到内容宽度，
            // 后续兄弟项就按内容宽排版，整页会塌成一条窄柱。
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(left_w);
                    body(ui, false);
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(right_w, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(right_w);
                    body(ui, true);
                },
            );
        });
        true
    } else {
        body(ui, false);
        ui.add_space(12.0);
        body(ui, true);
        false
    }
}

/// 页头：左标题，右状态胶囊。
pub fn header(ui: &mut Ui, title: &str, subtitle: &str, pill: &str, pill_color: Color32) {
    ui.horizontal(|ui| {
        ui_kit::page_title(ui, title, subtitle);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui_kit::status_pill(ui, pill, pill_color);
        });
    });
}

/// 「左说明 + 右控件」的设置行；控件区宽度随窗口收敛。
pub fn setting_row(
    ui: &mut Ui,
    title: &str,
    desc: &str,
    control_w: f32,
    control: impl FnOnce(&mut Ui),
) {
    let total = ui.available_width();
    let cw = control_w.min(total * 0.62).max(48.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.allocate_ui_with_layout(
            Vec2::new((total - cw - 10.0).max(80.0), 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width((total - cw - 10.0).max(80.0));
                ui.label(
                    RichText::new(title)
                        .size(11.5)
                        .strong()
                        .color(theme::text()),
                );
                if !desc.is_empty() {
                    ui.label(RichText::new(desc).size(9.5).color(theme::faint()));
                }
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(cw, 0.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_width(cw);
                control(ui);
            },
        );
    });
}

/// 前置开关的一行 KV（用于「隐私与冲突」）。
pub fn switch_row(ui: &mut Ui, on: &mut bool, key: &str, value: &str, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui_kit::switch(ui, on);
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(w, 20.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_width(w);
                ui_kit::kv_row(ui, key, value, value_color);
            },
        );
    });
}

/// 居中的小标签（`vertical_centered` 里的 `Frame` 会撑满整行）。
pub fn badge_centered(ui: &mut Ui, text: &str, fill: Color32, color: Color32) {
    let w = ui
        .painter()
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5), color)
        .size()
        .x
        + 14.0;
    ui.horizontal(|ui| {
        let pad = ((ui.available_width() - w) / 2.0).max(0.0);
        ui.add_space(pad);
        ui_kit::badge(ui, text, fill, color);
    });
}

/// 只读勾选项。
pub fn check_item(ui: &mut Ui, done: bool, text: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        // 字体里没有稳定的 ✔ / ○ 字形（缺字形会画成方框），直接自绘。
        let (r, _) = ui.allocate_exact_size(Vec2::splat(11.0), egui::Sense::hover());
        let c = r.center();
        if done {
            let p = ui.painter();
            p.line_segment(
                [
                    egui::pos2(c.x - 3.6, c.y + 0.2),
                    egui::pos2(c.x - 1.0, c.y + 2.7),
                ],
                Stroke::new(1.5_f32, color),
            );
            p.line_segment(
                [
                    egui::pos2(c.x - 1.0, c.y + 2.7),
                    egui::pos2(c.x + 3.8, c.y - 3.0),
                ],
                Stroke::new(1.5_f32, color),
            );
        } else {
            ui.painter()
                .circle_stroke(c, 3.4, Stroke::new(1.1_f32, color));
        }
        ui.label(RichText::new(text).size(10.0).color(if done {
            theme::subtext()
        } else {
            theme::faint()
        }));
    });
}

/// 尚未接入 / 无法判定的前置条件：画「—」而不是空心圆，避免被读成「未通过」。
pub fn pending_item(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        ui.label(RichText::new("—").size(10.0).color(theme::faint()));
        ui.label(RichText::new(text).size(10.0).color(theme::faint()));
    });
}

/// 分段组宽度（用于手动右对齐，保持选项从左到右的阅读顺序）。
pub fn segments_width(ui: &Ui, options: &[&str]) -> f32 {
    let cells: f32 = options
        .iter()
        .map(|t| {
            let w = ui
                .painter()
                .layout_no_wrap(
                    (*t).to_string(),
                    egui::FontId::proportional(11.5),
                    theme::text(),
                )
                .size()
                .x
                + 20.0;
            w.max(38.0)
        })
        .sum();
    cells + 4.0 + 2.0 * options.len().saturating_sub(1) as f32
}

/// 右对齐时使用的固定宽度分段组。
///
/// `setting_row` 的控件区是 `right_to_left`，`ui_kit::segmented` 会沿用该方向
/// 从而把选项倒序；这里开一个 `left_to_right` 子 Ui 保持阅读顺序。
pub fn segmented_right(ui: &mut Ui, options: &[&str], selected: usize) -> Option<usize> {
    let w = segments_width(ui, options);
    let avail = ui.available_width();
    if avail > w {
        ui.add_space(avail - w);
    }
    segmented_ltr(ui, options, selected, w)
}

/// 固定宽度的分段组。
///
/// `ui_kit::segmented` 内部沿用父 Ui 的排版方向：在 `right_to_left` 布局里
/// 选项会被倒序并且溢出右边。这里显式开一个从左到右的子 Ui 固定阅读顺序。
pub fn segmented_ltr(ui: &mut Ui, options: &[&str], selected: usize, width: f32) -> Option<usize> {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 26.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_width(width);
            ui_kit::segmented(ui, options, selected)
        },
    )
    .inner
}

/// 右对齐的分段组（父 Ui 必须是 `top_down` 或 `left_to_right`）。
pub fn right_segmented(ui: &mut Ui, options: &[&str], selected: usize) -> Option<usize> {
    let w = segments_width(ui, options);
    ui.add_space((ui.available_width() - w).max(0.0));
    segmented_ltr(ui, options, selected, w)
}

/// 记忆型布尔（本地偏好，写进 egui 的 `IdTypeMap`，不落盘）。
pub fn mem_bool(ui: &mut Ui, key: &str, default: bool) -> bool {
    ui.data(|d| d.get_temp::<bool>(Id::new(("sense.mem", key))))
        .unwrap_or(default)
}

pub fn set_mem_bool(ui: &mut Ui, key: &str, value: bool) {
    ui.data_mut(|d| d.insert_temp(Id::new(("sense.mem", key)), value));
}

/// 绑定到本地偏好的开关。
pub fn mem_switch(ui: &mut Ui, key: &str, default: bool) -> bool {
    let mut on = mem_bool(ui, key, default);
    ui_kit::switch(ui, &mut on);
    set_mem_bool(ui, key, on);
    on
}

/// 闪一下提示（秒），用于「必须保留锁屏」。
pub fn flash(ui: &mut Ui, key: &str, secs: f64) {
    let now = ui.input(|i| i.time);
    ui.data_mut(|d| d.insert_temp(Id::new(("sense.flash", key)), now + secs));
}

pub fn flashing(ui: &mut Ui, key: &str) -> bool {
    let now = ui.input(|i| i.time);
    ui.data(|d| d.get_temp::<f64>(Id::new(("sense.flash", key))))
        .map(|t| now < t)
        .unwrap_or(false)
}

/// 在场时长：`52` / `1.2` 这类数字（单位由调用方给 `min`）。
///
/// 不足一分钟给秒数，其余给分钟（带一位小数，避免「52 min」掩盖 52.5）。
pub fn fmt_minutes(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 60 {
        let m = s as f32 / 60.0;
        if (m - m.round()).abs() < 0.05 {
            format!("{}", m.round() as u64)
        } else {
            format!("{:.1}", m)
        }
    } else {
        format!("{}", s)
    }
}

/// 同上，但带单位（`52m` / `18s`），用于一行式文案。
pub fn fmt_duration(d: std::time::Duration) -> String {
    if d.as_secs() >= 60 {
        format!("{}m", fmt_minutes(d))
    } else {
        format!("{}s", d.as_secs())
    }
}

/// 「离开判定」秒数 → 分段下标。
pub fn away_index(secs: u32) -> usize {
    AWAY_STEPS
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| (**v as i64 - secs as i64).abs())
        .map(|(i, _)| i)
        .unwrap_or(1)
}

/// 撑满宽度的主按钮。
pub fn wide_primary(ui: &mut Ui, text: &str, danger: bool, width: f32) -> Response {
    let fill = if danger {
        theme::danger()
    } else {
        theme::accent()
    };
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .size(11.5)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(fill)
        .rounding(egui::Rounding::same(ui_kit::CTL_R))
        .min_size(Vec2::new(width, 30.0)),
    )
}

/// 撑满宽度的次按钮。
pub fn wide_ghost(ui: &mut Ui, text: &str, width: f32) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(theme::text()))
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border2()))
            .rounding(egui::Rounding::same(ui_kit::CTL_R))
            .min_size(Vec2::new(width, 30.0)),
    )
}
