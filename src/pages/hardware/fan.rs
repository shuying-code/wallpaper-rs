//! Fan Xpert：曲线编辑 + 通道只读读数。
//!
//! 诚实降级：Asusgio3 只允许 SYSTEM / Administrators，普通会话返回
//! E_ACCESSDENIED，所以曲线只写本地配置、不下发主板；转速与温度显示 n/a。

use super::HwCtx;
use crate::config::{CurvePoint, FansCfg};
use crate::pages::PageAction;
use crate::{theme, ui_kit};
use eframe::egui;
use egui::{Align, Layout, RichText, Vec2};

const PRESET_LABELS: [&str; 4] = ["静音", "标准", "性能", "自定义"];
const PRESET_KEYS: [&str; 4] = ["silent", "standard", "turbo", "custom"];

pub fn ui(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    ui_kit::page_title(ui, "散热 Fan Xpert", "曲线编辑 · 实时转速需管理员");
    ui.add_space(12.0);

    ui_kit::notice_card(
        ui,
        theme::amber(),
        "只能只读",
        r"\\.\Asusgio3 的 SDDL 只允许 SYSTEM/Administrators；AsusFanControlService 对普通会话返回 E_ACCESSDENIED。曲线编辑只写入本地配置，不会下发到主板。",
        None,
    );
    ui.add_space(12.0);

    preset_row(ctx, ui);
    ui.add_space(12.0);

    let custom = ctx.cfg.fans.preset == "custom";
    if ctx.compact {
        curve_card(ctx, ui, custom);
        ui.add_space(12.0);
        channels_card(ctx, ui);
    } else {
        let total = ui.available_width();
        let gap = 12.0;
        let lw = ((total - gap) * 0.66).max(320.0);
        let rw = (total - gap - lw).max(240.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(Vec2::new(lw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(lw);
                curve_card(ctx, ui, custom);
            });
            ui.add_space(gap);
            ui.allocate_ui_with_layout(Vec2::new(rw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(rw);
                channels_card(ctx, ui);
            });
        });
    }

    ui.add_space(12.0);
    bottom_card(ctx, ui);
}

// ---- preset -----------------------------------------------------------------

fn preset_row(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let current = PRESET_KEYS
        .iter()
        .position(|k| *k == ctx.cfg.fans.preset)
        .unwrap_or(1);
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "预设");
            ui.add_space(10.0);
            if let Some(i) = ui_kit::segmented(ui, &PRESET_LABELS, current) {
                let key = PRESET_KEYS.get(i).copied().unwrap_or("standard");
                if ctx.cfg.fans.preset != key {
                    ctx.cfg.fans.preset = key.to_string();
                    ctx.actions.push(PageAction::Save);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("预设只切换策略；曲线编辑需选「自定义」")
                        .size(9.5)
                        .color(theme::faint()),
                );
            });
        });
    });
}

// ---- curve ------------------------------------------------------------------

fn curve_card(ctx: &mut HwCtx, ui: &mut egui::Ui, enabled: bool) {
    let compact = ctx.compact;
    let hysteresis = ctx.cfg.fans.hysteresis_c;
    let mut points: Vec<(u8, u8)> = ctx
        .cfg
        .fans
        .curve
        .iter()
        .map(|p| (p.temp.min(100), p.duty.min(100)))
        .collect();
    if points.len() < 2 {
        points = default_points();
    }
    let mut edited = false;

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "风扇曲线");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    if enabled { "可编辑" } else { "只读" },
                    if enabled {
                        theme::accent_soft()
                    } else {
                        theme::extreme()
                    },
                    if enabled {
                        theme::accent()
                    } else {
                        theme::subtext()
                    },
                );
            });
        });
        ui.add_space(3.0);
        ui.label(
            RichText::new(format!(
                "拐点 {} 个 · 滞回 {} °C · 拖动节点编辑（横轴温度 / 纵轴占空比）",
                points.len(),
                hysteresis
            ))
            .size(9.5)
            .color(theme::faint()),
        );
        ui.add_space(8.0);

        let changed = ui
            .add_enabled_ui(enabled, |ui| {
                ui_kit::curve_editor(ui, 190.0, &mut points, theme::accent())
            })
            .inner;

        if changed && enabled {
            ctx.cfg.fans.curve = points
                .iter()
                .map(|(t, d)| CurvePoint { temp: *t, duty: *d })
                .collect();
            edited = true;
        }

        ui.add_space(8.0);
        let gap = 8.0;
        let total = ui.available_width();
        let cols = if compact { 2 } else { points.len().clamp(2, 7) };
        let w = ((total - gap * (cols as f32 - 1.0)) / cols as f32).max(52.0);
        let rows = (points.len() + cols - 1) / cols;
        for r in 0..rows {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for j in 0..cols {
                    let idx = r * cols + j;
                    let Some((t, d)) = points.get(idx).copied() else {
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
                                ui.label(
                                    RichText::new(format!("{} °C", t))
                                        .size(9.0)
                                        .color(theme::faint()),
                                );
                                ui_kit::mono(ui, &format!("{}% PWM", d), 11.5, theme::text());
                            });
                        },
                    );
                }
            });
            ui.add_space(gap);
        }

        if !enabled {
            ui.add_space(2.0);
            ui.label(
                RichText::new("切换预设到「自定义」后才能编辑曲线。")
                    .size(9.5)
                    .color(theme::amber()),
            );
        }
    });
    maybe_save(ui, ctx, edited);
}

/// 拖动中节流保存，松手时补一次，避免每帧写盘。
fn maybe_save(ui: &egui::Ui, ctx: &mut HwCtx, edited: bool) {
    let now = ui.input(|i| i.time);
    let down = ui.input(|i| i.pointer.any_down());
    let (dirty, last) = ui.data_mut(|d| {
        let dirty = d.get_temp_mut_or_insert_with(egui::Id::new("hw_fan_dirty"), || false);
        if edited {
            *dirty = true;
        }
        let is_dirty = *dirty;
        let last = d.get_temp_mut_or_insert_with(egui::Id::new("hw_fan_save_at"), || 0.0_f64);
        (is_dirty, *last)
    });
    let throttled = edited && now - last > 0.6;
    let finished = dirty && !down;
    if throttled || finished {
        ui.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("hw_fan_dirty"), || false) = false;
            *d.get_temp_mut_or_insert_with(egui::Id::new("hw_fan_save_at"), || 0.0_f64) = now;
        });
        ctx.actions.push(PageAction::Save);
    }
}

// ---- channels ---------------------------------------------------------------

fn channels_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let fans = &ctx.view.fans;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "风扇通道");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    &format!("{} 路", fans.channels.len()),
                    theme::extreme(),
                    theme::subtext(),
                );
            });
        });
        ui.add_space(6.0);
        if fans.channels.is_empty() {
            let body = fans
                .error
                .clone()
                .unwrap_or_else(|| "没有读到 FanCalibrationData.xml。".to_string());
            ui_kit::notice_card(ui, theme::amber(), "风扇通道不可用", &body, None);
            return;
        }
        for ch in &fans.channels {
            ui_kit::soft_block(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&ch.name)
                            .size(10.8)
                            .strong()
                            .color(theme::text()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui_kit::badge(ui, "只读", theme::extreme(), theme::subtext());
                    });
                });
                ui.horizontal(|ui| {
                    let rpm = if ch.max_rpm > 0 {
                        format!("{}", ch.max_rpm)
                    } else {
                        "n/a".to_string()
                    };
                    ui_kit::mono(
                        ui,
                        &rpm,
                        15.0,
                        if ch.max_rpm > 0 {
                            theme::text()
                        } else {
                            theme::faint()
                        },
                    );
                    ui.label(RichText::new("RPM 上限").size(8.5).color(theme::faint()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("最低 {}%", ch.min_duty))
                                .size(8.5)
                                .color(theme::faint()),
                        );
                    });
                });
                ui.add_space(3.0);
                let duties: Vec<f32> = ch.curve.iter().map(|(_, d)| *d as f32).collect();
                ui_kit::sparkline(ui, &duties, theme::accent());
            });
            ui.add_space(8.0);
        }
    });
}

// ---- bottom -----------------------------------------------------------------

fn bottom_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let profiles = &ctx.view.fans.profiles;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            if ui_kit::ghost_button(ui, "恢复 ASUS 预设曲线").clicked() {
                ctx.cfg.fans.preset = "standard".to_string();
                ctx.cfg.fans.curve = FansCfg::default().curve;
                ctx.actions.push(PageAction::Save);
            }
            ui.add_space(6.0);
            ui.add_enabled_ui(false, |ui| {
                let _ = ui_kit::small_button(ui, "交还控制");
            });
            ui.label(
                RichText::new("交还控制需管理员；普通会话没有 Asusgio3 写入权限。")
                    .size(9.0)
                    .color(theme::faint()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if profiles.is_empty() {
                        "未读到 ASUS 预设".to_string()
                    } else {
                        format!("ASUS 预设 {} 套", profiles.len())
                    })
                    .size(9.0)
                    .color(theme::faint()),
                );
            });
        });
    });
}

fn default_points() -> Vec<(u8, u8)> {
    FansCfg::default()
        .curve
        .iter()
        .map(|p| (p.temp, p.duty))
        .collect()
}
