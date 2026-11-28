//! v2 设计语言的共享控件库。
//!
//! egui 0.29 原生没有 switch / 分段组 / 环形指标 / 折线图（`Modal` 要到 0.31），
//! 这里用 `Painter` 自绘。所有控件只做「画 + 命中测试」，状态由调用方持有，
//! 因此可以在壁纸页、感知页、硬件页之间复用。

use crate::theme;
use eframe::egui;
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};

/// 卡片圆角（设计稿 13px）。
pub const CARD_R: f32 = 13.0;
/// 卡片内 padding。
pub const CARD_PAD: Vec2 = Vec2::new(15.0, 14.0);
/// 控件圆角。
pub const CTL_R: f32 = 9.0;

/// 标准卡片：白底 + 1px 描边 + 极轻投影。返回内层 `Ui` 的可用高度。
pub fn card(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(theme::card())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .rounding(Rounding::same(CARD_R))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 2.0),
            blur: 10.0,
            spread: 0.0,
            color: theme::shadow(),
        })
        .inner_margin(egui::Margin::symmetric(CARD_PAD.x, CARD_PAD.y))
        .show(ui, add_contents);
}

/// 无阴影的浅色区块（用于卡片内部的分组）。
pub fn soft_block(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(theme::extreme())
        .rounding(Rounding::same(CTL_R))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, add_contents);
}

/// 卡片标题：3px 强调色竖条 + 12px 加粗。
pub fn card_title(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), Sense::hover());
        ui.painter()
            .rect_filled(bar, Rounding::same(1.5), theme::accent());
        ui.add_space(5.0);
        ui.label(
            egui::RichText::new(text)
                .size(13.0)
                .strong()
                .color(theme::text()),
        );
    });
}

/// 页面标题（顶栏用）：16px 标题 + 9.5px 说明。
pub fn page_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(title)
                .size(16.5)
                .strong()
                .color(theme::text()),
        );
        if !subtitle.is_empty() {
            ui.label(
                egui::RichText::new(subtitle)
                    .size(11.0)
                    .color(theme::faint()),
            );
        }
    });
}

/// 等宽数字（倒计时 / 置信度 / 温度），避免每帧刷新抖动。
pub fn mono(ui: &mut Ui, text: &str, size: f32, color: Color32) -> Response {
    ui.label(
        egui::RichText::new(text)
            .font(FontId::monospace(size))
            .color(color),
    )
}

/// 小圆角标签。
pub fn badge(ui: &mut Ui, text: &str, fill: Color32, color: Color32) -> Response {
    egui::Frame::none()
        .fill(fill)
        .rounding(Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(10.5).strong().color(color));
        })
        .response
}

/// 状态胶囊（带一个实心圆点，用于「保护中 / 已锁定」）。
pub fn status_pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let fill = with_alpha(color, 26);
    egui::Frame::none()
        .fill(fill)
        .rounding(Rounding::same(20.0))
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.0, color);
                ui.label(egui::RichText::new(text).size(11.0).strong().color(color));
            });
        })
        .response
}

/// 34×19 胶囊开关。返回是否被点击。
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, 19.0), Sense::click());
    let enabled = ui.is_enabled();
    paint_switch(ui, rect, resp.id, *on, enabled);
    if resp.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() && enabled {
        *on = !*on;
    }
    resp
}

/// 在固定矩形里画开关并接管点击，不推进父 `Ui` 的布局游标。
///
/// 自绘列表行需要把开关嵌进行内；若这里再开子 `Ui`，父行的布局游标会被
/// 子控件高度覆盖，导致后续行重叠、副标题被裁掉（实测）。
pub fn switch_at(ui: &mut Ui, rect: Rect, id: Id, on: &mut bool, enabled: bool) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    paint_switch(ui, rect, resp.id, *on, enabled);
    if resp.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() && enabled {
        *on = !*on;
    }
    resp
}

fn paint_switch(ui: &Ui, rect: Rect, anim_id: Id, on: bool, enabled: bool) {
    let t = ui.ctx().animate_bool(anim_id, on);
    let track = if enabled {
        lerp_color(theme::border2(), theme::accent(), t)
    } else {
        theme::extreme()
    };
    ui.painter().rect_filled(rect, Rounding::same(9.5), track);
    let cx = rect.left() + 9.5 + (rect.width() - 19.0) * t;
    ui.painter().circle_filled(
        Pos2::new(cx, rect.center().y),
        7.5,
        if enabled {
            Color32::WHITE
        } else {
            theme::faint()
        },
    );
}

/// 分段组：返回被点中的下标（未点击返回 `None`）。
pub fn segmented(ui: &mut Ui, options: &[&str], selected: usize) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::none()
        .fill(theme::extreme())
        .rounding(Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(2.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, text) in options.iter().enumerate() {
                    let galley = ui.painter().layout_no_wrap(
                        (*text).to_string(),
                        FontId::proportional(11.5),
                        theme::text(),
                    );
                    let w = (galley.size().x + 20.0).max(38.0);
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
                    if i == selected {
                        ui.painter()
                            .rect_filled(r, Rounding::same(6.5), theme::card());
                    } else if resp.hovered() {
                        ui.painter().rect_filled(
                            r,
                            Rounding::same(6.5),
                            with_alpha(theme::card(), 120),
                        );
                    }
                    let color = if i == selected {
                        theme::accent()
                    } else {
                        theme::subtext()
                    };
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        *text,
                        FontId::proportional(11.5),
                        color,
                    );
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
        });
    clicked
}

/// 大数字指标（23px）+ 标题（9.5px）+ 可选后缀。
pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str, color: Color32) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(label).size(9.8).color(theme::faint()));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.label(egui::RichText::new(value).size(22.0).strong().color(color));
            if !unit.is_empty() {
                ui.label(egui::RichText::new(unit).size(10.0).color(theme::faint()));
            }
        });
    });
}

/// 横向细进度条。
pub fn bar(ui: &mut Ui, frac: f32, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(height / 2.0), theme::extreme());
    let w = rect.width() * frac.clamp(0.0, 1.0);
    if w > 1.0 {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(w, height)),
            Rounding::same(height / 2.0),
            color,
        );
    }
}

/// 分段刻度条（阈值滑杆的视觉：一格 1 分，过线部分实心）。
pub fn tick_bar(ui: &mut Ui, frac: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
    let n = 34usize;
    let gap = 2.0;
    let cell = (rect.width() - gap * (n as f32 - 1.0)) / n as f32;
    let limit = (frac.clamp(0.0, 1.0) * n as f32).round() as usize;
    for i in 0..n {
        let x = rect.left() + i as f32 * (cell + gap);
        let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(cell, rect.height()));
        let c = if i < limit { color } else { theme::border() };
        ui.painter().rect_filled(r, Rounding::same(1.5), c);
    }
}

/// 环形指标。`center_text` 画在圆心。
pub fn ring(ui: &mut Ui, size: f32, frac: f32, color: Color32, center_text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let center = rect.center();
    let radius = size * 0.42;
    let p = ui.painter();
    p.circle_stroke(center, radius, Stroke::new(9.0_f32, theme::extreme()));
    let steps = 128;
    let total = (frac.clamp(0.0, 1.0) * steps as f32) as usize;
    for i in 0..total {
        let a0 = -std::f32::consts::FRAC_PI_2 + (i as f32 / steps as f32) * std::f32::consts::TAU;
        let a1 =
            -std::f32::consts::FRAC_PI_2 + ((i + 1) as f32 / steps as f32) * std::f32::consts::TAU;
        p.line_segment(
            [
                Pos2::new(center.x + radius * a0.cos(), center.y + radius * a0.sin()),
                Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin()),
            ],
            Stroke::new(9.0_f32, color),
        );
    }
    if !center_text.is_empty() {
        p.text(
            center,
            Align2::CENTER_CENTER,
            center_text,
            FontId::proportional(size * 0.22),
            theme::text(),
        );
    }
}

/// 面积折线（26px 高）。`values` 为空时画一条基线。
pub fn sparkline(ui: &mut Ui, values: &[f32], color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let p = ui.painter();
    if values.len() < 2 {
        p.line_segment(
            [rect.left_center(), rect.right_center()],
            Stroke::new(1.0_f32, theme::border()),
        );
        return;
    }
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let span = (max - min).max(1e-3);
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Pos2::new(
                rect.left() + rect.width() * i as f32 / (values.len() - 1) as f32,
                rect.bottom() - (v - min) / span * (rect.height() - 3.0) - 1.5,
            )
        })
        .collect();
    let mut area = pts.clone();
    area.push(Pos2::new(rect.right(), rect.bottom()));
    area.push(Pos2::new(rect.left(), rect.bottom()));
    p.add(egui::Shape::convex_polygon(
        area,
        with_alpha(color, 26),
        Stroke::NONE,
    ));
    p.add(egui::Shape::line(pts, Stroke::new(1.6_f32, color)));
}

/// 折线图骨架：网格 + 轴标签 + 单序列。`y_max` 为 0 时按数据自适应。
pub fn line_chart(ui: &mut Ui, height: f32, values: &[f32], y_max: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    for i in 0..=3 {
        let y = rect.top() + rect.height() * i as f32 / 3.0;
        p.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0_f32, theme::border()),
        );
    }
    if values.len() < 2 {
        return;
    }
    let max = if y_max > 0.0 {
        y_max
    } else {
        values.iter().cloned().fold(0.0_f32, f32::max).max(1.0)
    };
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Pos2::new(
                rect.left() + rect.width() * i as f32 / (values.len() - 1) as f32,
                rect.bottom() - (v / max).clamp(0.0, 1.0) * (rect.height() - 4.0) - 2.0,
            )
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.7_f32, color)));
}

/// 摄像头画面占位：暗场渐变 + 四角取景框 + 可选检测框。
///
/// 真实采集尚未接入（里程碑 M2），这里先把版式与 HUD 位置固定下来，
/// 后续只需把 `texture` 换成 `TextureHandle` 即可。
pub struct CamFrame {
    pub label: String,
    pub present: bool,
    pub masked: bool,
    /// 采集后端是否真的在出帧。为 `false` 时不画检测框 / 不报「已识别」，
    /// 因为那会让人以为人脸识别已经在跑。
    pub live: bool,
}

pub fn camera_frame(ui: &mut Ui, height: f32, frame: &CamFrame) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    // 暗场渐变底
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), theme::video1());
    mesh.colored_vertex(rect.right_top(), theme::video1());
    mesh.colored_vertex(rect.left_bottom(), theme::video2());
    mesh.colored_vertex(rect.right_bottom(), theme::video2());
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));

    let p = ui.painter();
    // 人形剪影：告诉用户「这里会是画面」。未接入时调暗，避免被误认为已出画面。
    let ghost = if frame.live { 26 } else { 12 };
    let ghost_body = if frame.live { 18 } else { 8 };
    let head = Pos2::new(rect.center().x, rect.center().y - height * 0.1);
    p.circle_filled(head, height * 0.13, with_alpha(Color32::WHITE, ghost));
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(head.x - height * 0.17, rect.center().y + height * 0.05),
            Pos2::new(head.x + height * 0.17, rect.center().y + height * 0.05),
            Pos2::new(head.x + height * 0.22, rect.bottom() - height * 0.1),
            Pos2::new(head.x - height * 0.22, rect.bottom() - height * 0.1),
        ],
        with_alpha(Color32::WHITE, ghost_body),
        Stroke::NONE,
    ));

    let accent = if !frame.live {
        with_alpha(Color32::WHITE, 90)
    } else if frame.present {
        theme::mint()
    } else {
        theme::border2()
    };
    let det = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.center().y + height * 0.02),
        Vec2::new(rect.width() * 0.30, height * 0.62),
    );
    if frame.live {
        p.rect_stroke(det, Rounding::same(4.0), Stroke::new(1.4_f32, accent));
    }
    // 四角取景框
    let corner = 12.0;
    for (c, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        let c = c + Vec2::new(dx * 8.0, dy * 8.0);
        p.line_segment(
            [c, c + Vec2::new(dx * corner, 0.0)],
            Stroke::new(1.6_f32, with_alpha(Color32::WHITE, 110)),
        );
        p.line_segment(
            [c, c + Vec2::new(0.0, dy * corner)],
            Stroke::new(1.6_f32, with_alpha(Color32::WHITE, 110)),
        );
    }
    // HUD
    p.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 12.0),
        Align2::LEFT_TOP,
        &frame.label,
        FontId::monospace(9.0),
        with_alpha(Color32::WHITE, 210),
    );
    let state = if !frame.live {
        "未接入"
    } else if frame.present {
        "已识别"
    } else {
        "无检测"
    };
    p.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 12.0),
        Align2::RIGHT_TOP,
        format!("● {state}"),
        FontId::proportional(9.5),
        accent,
    );
    if frame.masked && frame.live {
        let mask = Rect::from_center_size(
            det.center(),
            Vec2::new(det.width() * 0.72, det.height() * 0.55),
        );
        p.rect_filled(mask, Rounding::same(3.0), with_alpha(theme::video2(), 210));
        p.text(
            mask.center(),
            Align2::CENTER_CENTER,
            "隐私打码",
            FontId::proportional(10.0),
            with_alpha(Color32::WHITE, 160),
        );
    }
    p.text(
        Pos2::new(rect.left() + 12.0, rect.bottom() - 10.0),
        Align2::LEFT_BOTTOM,
        "画面采集未接入 · 里程碑 M2",
        FontId::proportional(9.0),
        with_alpha(Color32::WHITE, 120),
    );
}

/// 说明 / 降级卡：图标 + 标题 + 正文 + 可选按钮。用于「摄像头不可用」等诚实提示。
pub fn notice_card(
    ui: &mut Ui,
    tone: Color32,
    title: &str,
    body: &str,
    action: Option<&str>,
) -> bool {
    let mut clicked = false;
    egui::Frame::none()
        .fill(with_alpha(tone, 18))
        .stroke(Stroke::new(1.0_f32, with_alpha(tone, 70)))
        .rounding(Rounding::same(CTL_R))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (sq, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                ui.painter()
                    .rect_stroke(sq, Rounding::same(4.0), Stroke::new(1.4_f32, tone));
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(0.0), Sense::hover());
                ui.painter().circle_filled(
                    dot.center() + Vec2::new(-8.0, 8.0),
                    0.0,
                    Color32::TRANSPARENT,
                );
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).size(11.5).strong().color(tone));
                    ui.label(egui::RichText::new(body).size(10.0).color(theme::subtext()));
                });
                if let Some(a) = action {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if small_button(ui, a).clicked() {
                            clicked = true;
                        }
                    });
                }
            });
        });
    clicked
}

/// 小号次级按钮。
pub fn small_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(10.5).color(theme::text()))
            .fill(theme::extreme())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(7.0))
            .min_size(Vec2::new(0.0, 24.0)),
    )
}

/// 主按钮（靛蓝实心）。
pub fn primary_button(ui: &mut Ui, text: &str, danger: bool) -> Response {
    let fill = if danger {
        theme::danger()
    } else {
        theme::accent()
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(12.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(fill)
        .rounding(Rounding::same(CTL_R))
        .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// 无边框幽灵按钮。
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(11.5).color(theme::subtext()))
            .fill(Color32::TRANSPARENT)
            .rounding(Rounding::same(7.0))
            .min_size(Vec2::new(0.0, 24.0)),
    )
}

/// 「标题 …… 右侧值」的一行。
pub fn kv_row(ui: &mut Ui, key: &str, value: &str, value_color: Color32) {
    // 手动定位而不是嵌套 `right_to_left`：卡片内的可用宽度在 `horizontal`
    // 里偶尔会被压成 0，导致右侧值消失（实测于「服务与后台」卡片）。
    let h = 17.0;
    let (mut rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    // `available_width()` 在 Frame 子层里偶尔会算到卡片外框（而非内边距内），
    // 右对齐值会被卡片裁切。以 clip_rect 为准再收 2px，保证值永远在卡内。
    let clip = ui.clip_rect();
    if rect.right() > clip.right() - 2.0 {
        rect.max.x = (clip.right() - 2.0).max(rect.left() + 24.0);
    }
    let p = ui.painter();
    let key_g = p.layout_no_wrap(
        key.to_string(),
        FontId::proportional(10.5),
        theme::subtext(),
    );
    p.galley(
        Pos2::new(rect.left(), rect.center().y - key_g.size().y * 0.5),
        key_g.clone(),
        theme::subtext(),
    );
    // 右侧值：超宽时截断，避免压到左键上。
    let limit = (rect.width() - key_g.size().x - 12.0).max(20.0);
    let mut text = value.to_string();
    let g = p.layout_no_wrap(text.clone(), FontId::proportional(10.5), value_color);
    if g.size().x > limit {
        while text.chars().count() > 1 {
            text.pop();
            let t = format!("{text}…");
            if p.layout_no_wrap(t.clone(), FontId::proportional(10.5), value_color)
                .size()
                .x
                <= limit
            {
                text = t;
                break;
            }
        }
    }
    let g = p.layout_no_wrap(text, FontId::proportional(10.5), value_color);
    p.galley(
        Pos2::new(
            rect.right() - g.size().x,
            rect.center().y - g.size().y * 0.5,
        ),
        g,
        value_color,
    );
}

/// 细进度滑杆：拖动整条即可调值。返回是否变化。
pub fn coarse_slider(ui: &mut Ui, value: &mut f32, min: f32, max: f32, color: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 18.0),
        Sense::click_and_drag(),
    );
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let f = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = min + f * (max - min);
        }
    }
    let f = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let cy = rect.center().y;
    ui.painter().line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(4.0_f32, theme::extreme()),
    );
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), cy),
            Pos2::new(rect.left() + rect.width() * f, cy),
        ],
        Stroke::new(4.0_f32, color),
    );
    let cx = rect.left() + rect.width() * f;
    ui.painter()
        .circle_filled(Pos2::new(cx, cy), 7.0, theme::card());
    ui.painter()
        .circle_stroke(Pos2::new(cx, cy), 7.0, Stroke::new(2.0_f32, color));
    resp.hovered() || resp.dragged()
}

/// 风扇曲线编辑器：节点可拖动。`points` 为 (温度, 占空比)，返回是否被修改。
pub fn curve_editor(ui: &mut Ui, height: f32, points: &mut Vec<(u8, u8)>, color: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(CTL_R), theme::extreme());
    for i in 0..=4 {
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        p.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(1.0_f32, theme::border()),
        );
    }
    for i in 0..=4 {
        let x = rect.left() + rect.width() * i as f32 / 4.0;
        p.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0_f32, theme::border()),
        );
    }
    let to_px = |t: u8, d: u8| {
        Pos2::new(
            rect.left() + rect.width() * (t as f32 / 100.0),
            rect.bottom() - rect.height() * (d as f32 / 100.0),
        )
    };
    let pts: Vec<Pos2> = points.iter().map(|(t, d)| to_px(*t, *d)).collect();
    if pts.len() >= 2 {
        p.add(egui::Shape::line(pts.clone(), Stroke::new(1.9_f32, color)));
    }
    let mut changed = false;
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            // 命中最近的节点
            let mut best = None;
            for (i, q) in pts.iter().enumerate() {
                let d = (q.x - pos.x).abs() + (q.y - pos.y).abs();
                if d < 26.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((i, d));
                }
            }
            if let Some((i, _)) = best {
                let t = (((pos.x - rect.left()) / rect.width()) * 100.0).clamp(1.0, 100.0);
                let d = (((rect.bottom() - pos.y) / rect.height()) * 100.0).clamp(0.0, 100.0);
                let lo = if i == 0 {
                    0.0
                } else {
                    points[i - 1].0 as f32 + 1.0
                };
                let hi = if i + 1 == points.len() {
                    100.0
                } else {
                    points[i + 1].0 as f32 - 1.0
                };
                points[i] = (t.clamp(lo, hi).round() as u8, d.round() as u8);
                changed = true;
            }
        }
    }
    for (i, q) in pts.iter().enumerate() {
        p.circle_filled(*q, 4.5, theme::card());
        p.circle_stroke(*q, 4.5, Stroke::new(1.8_f32, color));
        if resp.hovered() {
            p.text(
                *q + Vec2::new(0.0, -12.0),
                Align2::CENTER_BOTTOM,
                format!("{}%", points[i].1),
                FontId::proportional(8.4),
                theme::faint(),
            );
        }
    }
    changed
}

// ---- helpers ---------------------------------------------------------------

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// `#RRGGBB` → Color32（解析失败回落到强调色）。
pub fn parse_hex(s: &str) -> Color32 {
    let s = s.trim().trim_start_matches('#');
    if s.len() < 6 {
        return theme::accent();
    }
    let v = u32::from_str_radix(&s[..6], 16).unwrap_or(0x2F4BFF);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}
