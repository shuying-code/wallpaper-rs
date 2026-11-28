//! 设备页：本机灯效设备清单 + 选中设备详情 + 风扇通道速览。
//!
//! 诚实降级：温度 / 转速需要 Asusgio3 驱动通道（只有 SYSTEM/Administrators
//! 能打开），本版未接入，所以详情里明说「未接入」而不是填 0。

use super::HwCtx;
use crate::{armoury::AuraDevice, theme, ui_kit};
use eframe::egui;
use egui::{Align, Layout, RichText, Sense, Vec2};

pub fn ui(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let devices = ctx.view.devices();
    let aura_n = devices.len();
    let leds = ctx.view.aura.led_count;
    ui_kit::page_title(
        ui,
        "设备",
        &format!("本机灯效设备 · 风扇通道 · 接口（{aura_n} 台 / {leds} 灯珠）"),
    );
    ui.add_space(12.0);

    if ctx.compact {
        list_card(ctx, ui);
        ui.add_space(12.0);
        detail_card(ctx, ui);
    } else {
        let total = ui.available_width();
        let gap = 12.0;
        let lw = ((total - gap) * 0.62).max(300.0);
        let rw = (total - gap - lw).max(260.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(Vec2::new(lw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(lw);
                list_card(ctx, ui);
            });
            ui.add_space(gap);
            ui.allocate_ui_with_layout(Vec2::new(rw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(rw);
                detail_card(ctx, ui);
            });
        });
    }

    ui.add_space(12.0);
    channels_card(ctx, ui);
}

// ---- selected index ---------------------------------------------------------

fn selected(ctx: &HwCtx, ui: &egui::Ui) -> usize {
    let n = ctx.view.devices().len();
    let mut sel =
        ui.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("hw_dev_sel"), || 0usize));
    if n == 0 {
        sel = 0;
    } else if sel >= n {
        sel = n - 1;
    }
    sel
}

// ---- device list ------------------------------------------------------------

fn list_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let devices = ctx.view.devices();
    let mut sel = selected(ctx, ui);
    let mut clicked: Option<usize> = None;

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "已连接设备");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    &format!("{} 台", devices.len()),
                    theme::extreme(),
                    theme::subtext(),
                );
            });
        });
        ui.add_space(6.0);
        if devices.is_empty() {
            ui_kit::notice_card(
                ui,
                theme::amber(),
                "未发现灯效设备",
                ctx.view
                    .aura_note()
                    .unwrap_or_else(|| "Aura 服务尚未返回设备清单。".to_string())
                    .as_str(),
                None,
            );
            return;
        }
        for (i, d) in devices.iter().enumerate() {
            if device_row(ui, d, i == sel) {
                clicked = Some(i);
            }
            ui.add_space(6.0);
        }
    });

    if let Some(i) = clicked {
        sel = i;
    }
    ui.data_mut(|d| {
        *d.get_temp_mut_or_insert_with(egui::Id::new("hw_dev_sel"), || 0usize) = sel;
    });
}

fn device_row(ui: &mut egui::Ui, d: &AuraDevice, is_sel: bool) -> bool {
    let h = 50.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let fill = if is_sel {
        theme::accent_soft()
    } else if resp.hovered() {
        theme::extreme()
    } else {
        theme::card()
    };
    let stroke = if is_sel {
        egui::Stroke::new(1.5_f32, theme::accent())
    } else {
        egui::Stroke::new(1.0_f32, theme::border())
    };
    let p = ui.painter();
    p.rect(rect, egui::Rounding::same(10.0), fill, stroke);

    let icon_rect =
        egui::Rect::from_min_size(rect.left_top() + Vec2::new(9.0, 10.0), Vec2::splat(30.0));
    p.rect_filled(icon_rect, egui::Rounding::same(8.0), theme::accent_soft());
    p.text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        d.icon(),
        egui::FontId::proportional(17.0),
        theme::accent(),
    );

    let name = d.display_name();
    p.text(
        egui::Pos2::new(icon_rect.right() + 9.0, rect.top() + 13.0),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::proportional(12.5),
        theme::text(),
    );
    let sub = if d.manufacture.is_empty() {
        format!("{} · 序号 {}", d.kind, d.index)
    } else {
        format!("{} · {}", d.kind, d.manufacture)
    };
    p.text(
        egui::Pos2::new(icon_rect.right() + 9.0, rect.top() + 30.0),
        egui::Align2::LEFT_TOP,
        sub,
        egui::FontId::proportional(9.5),
        theme::faint(),
    );

    let badge_text = format!("{} 灯珠", d.leds);
    let galley = p.layout_no_wrap(
        badge_text.clone(),
        egui::FontId::proportional(10.5),
        theme::accent(),
    );
    let bw = galley.size().x + 14.0;
    let brect = egui::Rect::from_min_size(
        egui::Pos2::new(rect.right() - bw - 9.0, rect.top() + 15.0),
        Vec2::new(bw, 20.0),
    );
    p.rect_filled(brect, egui::Rounding::same(6.0), theme::accent_soft());
    p.text(
        brect.center(),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::FontId::proportional(10.5),
        theme::accent(),
    );

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

// ---- detail -----------------------------------------------------------------

fn detail_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let devices = ctx.view.devices();
    let sel = selected(ctx, ui);
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "设备详情");
        ui.add_space(6.0);
        let Some(d) = devices.get(sel) else {
            ui_kit::notice_card(
                ui,
                theme::subtext(),
                "没有可展示的设备",
                "Aura 服务在线但没有返回任何灯效设备。",
                None,
            );
            return;
        };

        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
            ui.painter()
                .rect_filled(r, egui::Rounding::same(9.0), theme::accent_soft());
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                d.icon(),
                egui::FontId::proportional(19.0),
                theme::accent(),
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(d.display_name())
                        .size(13.5)
                        .strong()
                        .color(theme::text()),
                );
                ui.label(
                    RichText::new(if d.manufacture.is_empty() {
                        "厂商未报告".to_string()
                    } else {
                        d.manufacture.clone()
                    })
                    .size(9.5)
                    .color(theme::faint()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::status_pill(ui, "Aura 已同步", theme::mint());
            });
        });
        ui.add_space(8.0);

        ui_kit::soft_block(ui, |ui| {
            ui_kit::kv_row(ui, "类型", &d.kind, theme::text());
            ui_kit::kv_row(
                ui,
                "厂商",
                if d.manufacture.is_empty() {
                    "n/a"
                } else {
                    d.manufacture.as_str()
                },
                theme::subtext(),
            );
            ui_kit::kv_row(
                ui,
                "模型",
                if d.model.is_empty() {
                    "n/a"
                } else {
                    d.model.as_str()
                },
                theme::subtext(),
            );
            ui_kit::kv_row(ui, "灯珠", &format!("{} 颗", d.leds), theme::accent());
            let matrix = if d.width > 0 && d.height > 0 {
                format!("{} × {}", d.width, d.height)
            } else {
                "n/a".to_string()
            };
            ui_kit::kv_row(ui, "矩阵", &matrix, theme::subtext());
            ui_kit::kv_row(ui, "序号", &format!("#{}", d.index), theme::subtext());
        });

        ui.add_space(10.0);
        ui_kit::notice_card(
            ui,
            theme::amber(),
            "温度 / 转速未接入",
            "温度与转速需 Asusgio3 驱动通道，本版未接入；这里只展示 Aura COM 能报告的灯效规格。",
            None,
        );

        ui.add_space(10.0);
        ui.label(
            RichText::new("接口")
                .size(10.5)
                .strong()
                .color(theme::text()),
        );
        ui.add_space(4.0);
        ui_kit::kv_row(ui, "Aura COM", "✓ 可用（免管理员）", theme::mint());
        ui_kit::kv_row(ui, "Asusgio3", "✗ 需管理员", theme::amber());
    });
}

// ---- fan channels -----------------------------------------------------------

fn channels_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let fans = &ctx.view.fans;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "风扇通道");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    if fans.can_write { "可写" } else { "只读" },
                    if fans.can_write {
                        theme::mint_soft()
                    } else {
                        theme::extreme()
                    },
                    if fans.can_write {
                        theme::mint()
                    } else {
                        theme::subtext()
                    },
                );
            });
        });
        ui.add_space(6.0);
        if fans.channels.is_empty() {
            let body = fans
                .error
                .clone()
                .unwrap_or_else(|| "未读到风扇标定表。".to_string());
            ui_kit::notice_card(ui, theme::amber(), "风扇通道不可用", &body, None);
            return;
        }
        let n = fans.channels.len();
        let gap = 10.0;
        let cols = if ctx.compact { 1 } else { n.min(4) };
        let w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).max(140.0);
        for chunk in 0..((n + cols - 1) / cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for j in 0..cols {
                    let idx = chunk * cols + j;
                    let Some(ch) = fans.channels.get(idx) else {
                        break;
                    };
                    if j > 0 {
                        ui.add_space(gap);
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(w, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(w);
                            ui_kit::soft_block(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(
                                    RichText::new(&ch.name)
                                        .size(10.5)
                                        .strong()
                                        .color(theme::text()),
                                );
                                ui.add_space(3.0);
                                ui.horizontal(|ui| {
                                    let rpm = if ch.max_rpm > 0 {
                                        format!("{}", ch.max_rpm)
                                    } else {
                                        "n/a".to_string()
                                    };
                                    ui_kit::mono(ui, &rpm, 15.0, theme::text());
                                    ui.label(
                                        RichText::new("RPM 上限").size(8.5).color(theme::faint()),
                                    );
                                });
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    ui_kit::badge(ui, "只读", theme::extreme(), theme::subtext());
                                    ui.label(
                                        RichText::new(format!("最低 {}%", ch.min_duty))
                                            .size(8.5)
                                            .color(theme::faint()),
                                    );
                                });
                            });
                        },
                    );
                }
            });
            ui.add_space(gap);
        }
    });
}
