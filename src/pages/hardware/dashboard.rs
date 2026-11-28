//! 控制面板：整机状态 + 一键切换场景。
//!
//! 诚实降级：本机没有可信的温度序列（MSAcpi 类为空），所以温度历史只显示
//! 说明卡；双环是由配置推导的「示意」值，不冒充硬件读数。

use super::HwCtx;
use crate::pages::PageAction;
use crate::{theme, ui_kit};
use eframe::egui;
use egui::{Align, Color32, Layout, RichText, Vec2};

pub fn ui(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let compact = ctx.compact;

    ui.add_space(2.0);
    ui_kit::page_title(ui, "控制面板", "整机状态 · 一键切换场景");
    ui.add_space(12.0);

    metric_row(ctx, ui);
    ui.add_space(12.0);

    split_cols(ui, compact, 0.62, ctx, perf_card, scene_card);
    ui.add_space(12.0);
    split_cols(ui, compact, 0.62, ctx, history_card, services_card);
}

// ---- metrics ---------------------------------------------------------------

fn metric_row(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let view = ctx.view;
    let compact = ctx.compact;
    let gap = 12.0;

    let cpu_known = view.system.cpu_load >= 0.0;
    let cpu_load = if cpu_known {
        format!("{:.0}", view.system.cpu_load.clamp(0.0, 100.0))
    } else {
        "—".to_string()
    };
    let cpu_unit = if cpu_known { "%" } else { "" };
    let cpu_name = if view.system.cpu_name.is_empty() {
        "CPU 型号未读取".to_string()
    } else {
        truncate(&view.system.cpu_name, if compact { 16 } else { 30 })
    };
    let mem_total = view.system.mem_total_gb;
    let mem_used = view.system.mem_used_gb;
    let mem_known = mem_total > 0.0;
    let mem_frac = if mem_total > 0.0 {
        (mem_used / mem_total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gpu_max = if compact { 12 } else { 13 };
    let gpu = if view.system.gpu_name.is_empty() {
        "未识别".to_string()
    } else {
        truncate(&view.system.gpu_name, gpu_max)
    };
    let gpu_full = if view.system.gpu_name.is_empty() {
        "GPU 型号未读取".to_string()
    } else {
        truncate(&view.system.gpu_name, if compact { 14 } else { 38 })
    };
    let aura_ready = view.aura_ready();
    let mode = view.aura.active_mode.clone().unwrap_or_else(|| {
        if aura_ready {
            "读取中".to_string()
        } else {
            "不可用".to_string()
        }
    });
    let lamp_color = if aura_ready {
        theme::accent()
    } else {
        theme::faint()
    };

    if compact {
        let total = ui.available_width();
        let w = ((total - gap) / 2.0).max(110.0);
        let cw = (w - 30.0).max(60.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "CPU 占用", &cpu_load, cpu_unit, theme::accent());
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(RichText::new(&cpu_name).size(9.5).color(theme::faint()))
                        .truncate(),
                );
            });
            ui.add_space(gap);
            metric_cell(ui, w, cw, |ui| {
                if mem_known {
                    ui_kit::metric(
                        ui,
                        "内存",
                        &format!("{mem_used:.1}"),
                        &format!("/ {mem_total:.1} GB"),
                        theme::text(),
                    );
                } else {
                    ui_kit::metric(ui, "内存", "n/a", "", theme::subtext());
                }
                ui.add_space(6.0);
                ui_kit::bar(ui, mem_frac, theme::accent2(), 5.0);
            });
        });
        ui.add_space(gap);
        let total = ui.available_width();
        let w = ((total - gap) / 2.0).max(110.0);
        let cw = (w - 30.0).max(60.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "GPU 型号", &gpu, "", theme::text());
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(RichText::new(&gpu_full).size(9.5).color(theme::faint()))
                        .truncate(),
                );
            });
            ui.add_space(gap);
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "灯效", &truncate(&mode, 10), "", lamp_color);
                ui.add_space(3.0);
                ui.label(
                    RichText::new(format!(
                        "{} 灯珠 · {} 台设备",
                        view.aura.led_count,
                        view.devices().len()
                    ))
                    .size(9.5)
                    .color(theme::faint()),
                );
            });
        });
    } else {
        let w = ((ui.available_width() - gap * 3.0) / 4.0).max(150.0);
        let cw = (w - 30.0).max(60.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "CPU 占用", &cpu_load, cpu_unit, theme::accent());
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(RichText::new(&cpu_name).size(9.5).color(theme::faint()))
                        .truncate(),
                );
            });
            ui.add_space(gap);
            metric_cell(ui, w, cw, |ui| {
                if mem_known {
                    ui_kit::metric(
                        ui,
                        "内存",
                        &format!("{mem_used:.1}"),
                        &format!("/ {mem_total:.1} GB"),
                        theme::text(),
                    );
                } else {
                    ui_kit::metric(ui, "内存", "n/a", "", theme::subtext());
                }
                ui.add_space(6.0);
                ui_kit::bar(ui, mem_frac, theme::accent2(), 5.0);
            });
            ui.add_space(gap);
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "GPU 型号", &gpu, "", theme::text());
                ui.add_space(3.0);
                ui.add(
                    egui::Label::new(RichText::new(&gpu_full).size(9.5).color(theme::faint()))
                        .truncate(),
                );
            });
            ui.add_space(gap);
            metric_cell(ui, w, cw, |ui| {
                ui_kit::metric(ui, "灯效", &truncate(&mode, 10), "", lamp_color);
                ui.add_space(3.0);
                ui.label(
                    RichText::new(format!(
                        "{} 灯珠 · {} 台设备",
                        view.aura.led_count,
                        view.devices().len()
                    ))
                    .size(9.5)
                    .color(theme::faint()),
                );
            });
        });
    }
}

/// 一片指标位：先钉死栏宽，再画卡片，卡片内宽 = 栏宽 - 两侧 padding。
fn metric_cell(ui: &mut egui::Ui, w: f32, cw: f32, contents: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_max_width(w);
        ui.set_min_width(w);
        ui_kit::card(ui, |ui| {
            ui.set_min_width(cw);
            ui.set_max_width(cw);
            ui.set_min_height(66.0);
            contents(ui);
        });
    });
}

// ---- rings + scenes ---------------------------------------------------------

fn ring_block(ui: &mut egui::Ui, frac: f32, color: Color32, label: &str) {
    // 必须固定宽度：`vertical_centered` 会吃掉全部剩余宽度，把同一行的
    // 键值列挤成 0 宽、文字溢出到隔壁卡片上（实测缺陷）。
    ui.allocate_ui_with_layout(
        Vec2::new(104.0, 118.0),
        Layout::top_down(Align::Center),
        |ui| {
            ui.set_width(104.0);
            ui_kit::ring(ui, 88.0, frac, color, &format!("{:.0}%", frac * 100.0));
            ui.add_space(4.0);
            ui.label(RichText::new(label).size(9.8).color(theme::faint()));
        },
    );
}

fn perf_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let preset = preset_index(&ctx.cfg.fans.preset);
    let hysteresis = ctx.cfg.fans.hysteresis_c;
    let takeover = ctx.cfg.fans.takeover;
    let poll_ms = ctx.cfg.poll_ms;
    let base: f32 = match preset {
        0 => 0.34,
        2 => 0.88,
        3 => 0.62,
        _ => 0.56,
    };
    let perf = (base + if takeover { 0.04_f32 } else { 0.0 }).clamp(0.05, 1.0);
    let noise: f32 = (0.86_f32 - hysteresis as f32 * 0.09).clamp(0.12, 0.95);

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "性能与散热");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(ui, "示意", theme::amber_soft(), theme::amber());
            });
        });
        ui.add_space(2.0);
        kv_strip(ui, preset, hysteresis, takeover, poll_ms, perf, noise);
        ui.add_space(6.0);
        ui.label(
            RichText::new("双环由当前预设与滞回配置推导，仅表达策略倾向，不是实时传感器读数。")
                .size(9.5)
                .color(theme::faint()),
        );
    });
}

fn scene_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let selected = preset_index(&ctx.cfg.fans.preset);
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "场景预设");
        ui.add_space(4.0);
        if let Some(i) = ui_kit::segmented(ui, &["静音", "均衡", "性能", "自定义"], selected)
        {
            let key = preset_key(i);
            if ctx.cfg.fans.preset != key {
                ctx.cfg.fans.preset = key.to_string();
                ctx.actions.push(PageAction::Save);
            }
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(preset_desc(preset_index(&ctx.cfg.fans.preset)))
                .size(10.0)
                .color(theme::subtext()),
        );
        ui.add_space(10.0);
        if ui_kit::primary_button(ui, "一键切换场景", false).clicked() {
            ctx.actions.push(PageAction::RefreshHardware);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("会同步写入风扇预设；灯效按 Aura COM 当前能力下发。")
                .size(9.5)
                .color(theme::faint()),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui_kit::status_pill(ui, "配置即存", theme::subtext());
            ui_kit::status_pill(
                ui,
                if ctx.view.aura_ready() {
                    "Aura 在线"
                } else {
                    "Aura 离线"
                },
                if ctx.view.aura_ready() {
                    theme::mint()
                } else {
                    theme::amber()
                },
            );
        });
    });
}

/// 双环 + 配置键值：宽窗三列并排，窄窗改为双环一行、键值两列。
fn kv_strip(
    ui: &mut egui::Ui,
    preset: usize,
    hysteresis: u8,
    takeover: bool,
    poll_ms: u64,
    perf: f32,
    noise: f32,
) {
    let total = ui.available_width();
    let rows = [
        ("风扇预设", preset_label(preset).to_string(), theme::text()),
        ("拐点滞回", format!("{} °C", hysteresis), theme::text()),
        (
            "控制接管",
            if takeover { "开" } else { "关" }.to_string(),
            if takeover {
                theme::mint()
            } else {
                theme::subtext()
            },
        ),
        ("采样间隔", format!("{} ms", poll_ms), theme::subtext()),
    ];

    if total < 540.0 {
        // 窄窗：两环各占一半，键值折成两列，避免把卡片撑出视口。
        let half = (total * 0.5).clamp(96.0, 220.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                Vec2::new(half, 0.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.set_width(half);
                    ring_block(ui, perf, theme::accent(), "性能预期");
                },
            );
            ui.allocate_ui_with_layout(
                Vec2::new(half, 0.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.set_width(half);
                    ring_block(ui, noise, theme::mint(), "噪音预期");
                },
            );
        });
        ui.add_space(8.0);
        let gap = 10.0;
        let cw = ((total - gap) / 2.0).max(80.0);
        for pair in 0..2 {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for j in 0..2 {
                    let idx = pair * 2 + j;
                    let Some((k, v, c)) = rows.get(idx) else {
                        break;
                    };
                    if j > 0 {
                        ui.add_space(gap);
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(cw, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_max_width(cw);
                            ui.set_min_width(cw);
                            ui_kit::kv_row(ui, k, v, *c);
                        },
                    );
                }
            });
            ui.add_space(4.0);
        }
        return;
    }

    let ring_area = 236.0;
    let gap = 14.0;
    let kv_w = (total - ring_area - gap).max(120.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            Vec2::new(ring_area, 0.0),
            Layout::left_to_right(Align::Min),
            |ui| {
                ui.set_width(ring_area);
                ring_block(ui, perf, theme::accent(), "性能预期");
                ring_block(ui, noise, theme::mint(), "噪音预期");
            },
        );
        ui.add_space(gap);
        ui.allocate_ui_with_layout(Vec2::new(kv_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_max_width(kv_w);
            ui.set_min_width(kv_w);
            for (k, v, c) in rows.iter() {
                ui_kit::kv_row(ui, k, v, *c);
            }
        });
    });
}

// ---- history + services -----------------------------------------------------

fn history_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let sys = &ctx.view.system;
    let cpu = sys
        .cpu_temp_c
        .map(|t| format!("{t:.0} °C"))
        .unwrap_or_else(|| "n/a".to_string());
    let gpu = sys
        .gpu_temp_c
        .map(|t| format!("{t:.0} °C"))
        .unwrap_or_else(|| "n/a".to_string());

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "温度历史");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("近 60 秒 · 最多保留 5 分钟")
                        .size(9.5)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(6.0);
        ui_kit::notice_card(
            ui,
            theme::amber(),
            "温度读取不可用",
            "本机 MSAcpi_ThermalZoneTemperature 为空，Armoury Crate 自己也是从硬件驱动读；这里不编造曲线。",
            None,
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("本帧读数").size(9.5).color(theme::faint()));
            ui_kit::mono(ui, "CPU", 9.5, theme::faint());
            ui_kit::mono(
                ui,
                &cpu,
                12.0,
                if sys.cpu_temp_c.is_some() {
                    theme::mint()
                } else {
                    theme::subtext()
                },
            );
            ui.add_space(12.0);
            ui_kit::mono(ui, "GPU", 9.5, theme::faint());
            ui_kit::mono(
                ui,
                &gpu,
                12.0,
                if sys.gpu_temp_c.is_some() {
                    theme::mint()
                } else {
                    theme::subtext()
                },
            );
        });
    });
}

fn services_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let view = ctx.view;
    let ready = view.aura_ready();
    let aura_note = view
        .aura_note()
        .map(|s| truncate(&s, 26))
        .unwrap_or_else(|| "尚未采样".to_string());
    let fan_text = match &view.fans.error {
        Some(err) => truncate(err, 24),
        None => format!("{} 路 · 只读", view.fans.channels.len()),
    };

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "服务与后台");
        ui.add_space(6.0);
        ui_kit::kv_row(
            ui,
            "Aura 服务",
            &aura_note,
            if ready { theme::mint() } else { theme::amber() },
        );
        ui_kit::kv_row(
            ui,
            "Aura 灯珠",
            &format!(
                "{} 颗 · {} 台设备",
                view.aura.led_count,
                view.devices().len()
            ),
            theme::subtext(),
        );
        ui_kit::kv_row(
            ui,
            "Fan 通道",
            &fan_text,
            if view.fans.error.is_some() {
                theme::amber()
            } else {
                theme::subtext()
            },
        );
        ui_kit::kv_row(
            ui,
            "采样间隔",
            &format!("{} ms", ctx.cfg.poll_ms),
            theme::subtext(),
        );
        ui_kit::kv_row(
            ui,
            "版本",
            &format!("v{} · Windows", env!("CARGO_PKG_VERSION")),
            theme::faint(),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui_kit::status_pill(
                ui,
                if ready {
                    "Aura 已连接"
                } else {
                    "Aura 未连接"
                },
                if ready { theme::mint() } else { theme::amber() },
            );
            ui_kit::status_pill(ui, "EC 驱动只读", theme::subtext());
        });
    });
}

// ---- helpers ----------------------------------------------------------------

fn split_cols(
    ui: &mut egui::Ui,
    compact: bool,
    frac: f32,
    ctx: &mut HwCtx,
    left: fn(&mut HwCtx, &mut egui::Ui),
    right: fn(&mut HwCtx, &mut egui::Ui),
) {
    if compact {
        left(ctx, ui);
        ui.add_space(12.0);
        right(ctx, ui);
        return;
    }
    let total = ui.available_width();
    let gap = 12.0;
    let lw = ((total - gap) * frac).max(240.0);
    let rw = (total - gap - lw).max(240.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(Vec2::new(lw, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(lw);
            left(ctx, ui);
        });
        ui.add_space(gap);
        ui.allocate_ui_with_layout(Vec2::new(rw, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(rw);
            right(ctx, ui);
        });
    });
}

fn preset_index(p: &str) -> usize {
    match p {
        "silent" => 0,
        "turbo" => 2,
        "custom" => 3,
        _ => 1,
    }
}

fn preset_key(i: usize) -> &'static str {
    match i {
        0 => "silent",
        2 => "turbo",
        3 => "custom",
        _ => "standard",
    }
}

fn preset_label(i: usize) -> &'static str {
    match i {
        0 => "静音",
        2 => "性能",
        3 => "自定义",
        _ => "均衡",
    }
}

fn preset_desc(i: usize) -> &'static str {
    match i {
        0 => "静音：曲线整体下压，优先降低转速与噪音。",
        2 => "性能：曲线提前拉高，优先压制温度。",
        3 => "自定义：使用 Fan Xpert 页里手动编辑的曲线。",
        _ => "均衡：日常使用的散热与噪音折中。",
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
