//! 灯效 & Aura Sync：效果模式、颜色、亮度 / 速度、异步下发。
//!
//! 预览条是自绘示意（按设备分段画圆点），不代表真实灯珠排布；
//! 下发失败只显示服务返回的原因，不做乐观假设。

use super::HwCtx;
use crate::pages::PageAction;
use crate::{theme, ui_kit};
use eframe::egui;
use egui::{Align, Color32, Layout, Pos2, RichText, Rounding, Sense, Stroke, Vec2};

const PRESETS: [(&str, &str); 6] = [
    ("#2F4BFF", "靛蓝"),
    ("#7A5CFF", "紫"),
    ("#0E9F6E", "薄荷"),
    ("#E5A54B", "琥珀"),
    ("#F2557A", "玫红"),
    ("#3ED9A4", "青绿"),
];

pub fn ui(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let ready = ctx.view.aura_ready();
    let device_n = ctx.view.devices().len();
    let leds = ctx.view.aura.led_count;

    ui.add_space(2.0);
    ui_kit::page_title(
        ui,
        "灯效 & Aura Sync",
        &format!("官方 Aura COM · {device_n} 台设备 / {leds} 灯珠"),
    );
    ui.add_space(12.0);

    let mut retry = false;
    if !ready {
        let note = ctx
            .view
            .aura_note()
            .unwrap_or_else(|| "Aura 服务不可用：尚未探测到 AsRogAuraService。".to_string());
        if ui_kit::notice_card(ui, theme::amber(), "Aura 服务不可用", &note, Some("重试")) {
            retry = true;
        }
        ui.add_space(12.0);
    }

    ui.add_enabled_ui(ready, |ui| {
        preview_card(ctx, ui);
        ui.add_space(12.0);
        devices_card(ctx, ui);
        ui.add_space(12.0);

        if ctx.compact {
            modes_card(ctx, ui);
            ui.add_space(12.0);
            params_card(ctx, ui);
        } else {
            let total = ui.available_width();
            let gap = 12.0;
            let lw = ((total - gap) * 0.58).max(280.0);
            let rw = (total - gap - lw).max(260.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.allocate_ui_with_layout(
                    Vec2::new(lw, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(lw);
                        modes_card(ctx, ui);
                    },
                );
                ui.add_space(gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(rw, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(rw);
                        params_card(ctx, ui);
                    },
                );
            });
        }

        ui.add_space(12.0);
        apply_row(ctx, ui);
    });

    if retry {
        ctx.actions.push(PageAction::RefreshHardware);
    }

    ui.add_space(10.0);
    ui.label(
        RichText::new(
            "回写 XML 的 funcid 必须为 1；写前解除独占、写完还 1，否则 Armoury Crate 自己的灯效页会失联。首次下发前已自动备份状态。",
        )
        .size(9.5)
        .color(theme::faint()),
    );
}

// ---- per-device switches ----------------------------------------------------

fn devices_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let devices = ctx.view.devices().to_vec();
    let mut toggle: Option<(String, bool)> = None;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "逐设备灯光");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let off = devices
                    .iter()
                    .filter(|d| {
                        ctx.view.aura.switchable_devices.contains(&d.key())
                            && !ctx.cfg.lighting.device_enabled(&d.key())
                    })
                    .count();
                let badge_text = if off == 0 {
                    "全部开启".to_string()
                } else {
                    format!("{off} 台熄灭")
                };
                ui_kit::badge(
                    ui,
                    &badge_text,
                    if off == 0 {
                        theme::mint_soft()
                    } else {
                        theme::amber_soft()
                    },
                    if off == 0 {
                        theme::mint()
                    } else {
                        theme::amber()
                    },
                );
            });
        });
        ui.add_space(6.0);
        if devices.is_empty() {
            ui_kit::notice_card(
                ui,
                theme::subtext(),
                "没有可控制的设备",
                "Aura 服务尚未返回设备清单。",
                None,
            );
            return;
        }
        for d in devices.iter() {
            let key = d.key();
            let switchable = ctx.view.aura.switchable_devices.contains(&key);
            let mut on = ctx.cfg.lighting.device_enabled(&key);
            ui.horizontal(|ui| {
                ui.label(RichText::new(d.display_name()).size(11.0).color(if on {
                    theme::text()
                } else {
                    theme::faint()
                }));
                ui_kit::badge(
                    ui,
                    &format!("{} 灯珠", d.leds),
                    theme::extreme(),
                    theme::subtext(),
                );
                if !switchable {
                    ui_kit::badge(ui, "不可独立控制", theme::amber_soft(), theme::amber());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(switchable, |ui| ui_kit::switch(ui, &mut on))
                        .inner
                        .changed()
                    {
                        toggle = Some((key.clone(), on));
                    }
                });
            });
            ui.add_space(4.0);
        }
        ui.label(
            RichText::new(
                "开关按设备映射到 Aura Group S0 的 LED 节点；关闭即软件熄灭该设备，其他设备保持当前效果。\
                 没有 LED 节点的设备会标为「不可独立控制」，不会假装已断电。",
            )
            .size(9.3)
            .color(theme::faint()),
        );
    });
    if let Some((key, on)) = toggle {
        ctx.cfg.lighting.set_device_enabled(&key, on);
        ctx.actions.push(PageAction::Save);
        ctx.actions.push(PageAction::ApplyAura);
    }
}

// ---- preview ----------------------------------------------------------------

fn preview_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let mode = ctx
        .view
        .aura
        .active_mode
        .clone()
        .unwrap_or_else(|| "未知模式".to_string());
    let read_hex = ctx.view.aura.active_color.clone();
    let read_color = read_hex.as_deref().map(ui_kit::parse_hex);
    // 灯效关闭时服务返回的 HSL 全 0（实测本机 `isenabled=0`），不能把这当成
    // 「当前是纯黑」——否则预览条整块发黑，看起来像渲染坏了。
    let dark_read = read_color
        .map(|c| c.r() < 6 && c.g() < 6 && c.b() < 6)
        .unwrap_or(true);
    let hex = if dark_read {
        ctx.cfg.lighting.color.clone()
    } else {
        read_hex.clone().unwrap_or_default()
    };
    let color = ui_kit::parse_hex(&hex);
    let bright = ctx.view.aura.brightness.max(1);
    let leds = ctx.view.aura.led_count;

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "灯效预览");
            ui.label(
                RichText::new(format!("{mode} · {bright}% 亮度"))
                    .size(9.5)
                    .color(theme::faint()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::mono(ui, &ui_kit::to_hex(color), 10.0, theme::subtext());
            });
        });
        if dark_read {
            ui.add_space(4.0);
            ui.label(
                RichText::new("灯效当前为关闭态（服务报 HSL 全 0）· 预览显示的是待下发配置")
                    .size(9.5)
                    .color(theme::amber()),
            );
        }
        ui.add_space(8.0);
        preview_band(ui, ctx, color);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("预览为示意，真实灯珠数 {leds}"))
                    .size(9.5)
                    .color(theme::faint()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    if ctx.view.aura.exclusive {
                        "当前独占"
                    } else {
                        "未独占"
                    },
                    if ctx.view.aura.exclusive {
                        theme::amber_soft()
                    } else {
                        theme::extreme()
                    },
                    if ctx.view.aura.exclusive {
                        theme::amber()
                    } else {
                        theme::subtext()
                    },
                );
            });
        });
    });
}

fn preview_band(ui: &mut egui::Ui, ctx: &HwCtx, color: Color32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 96.0), Sense::hover());
    let p = ui.painter();

    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), theme::video1());
    mesh.colored_vertex(rect.right_top(), theme::video2());
    mesh.colored_vertex(rect.left_bottom(), theme::video2());
    mesh.colored_vertex(rect.right_bottom(), theme::video1());
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(egui::Shape::mesh(mesh));
    p.rect_stroke(
        rect,
        Rounding::same(10.0),
        Stroke::new(1.0_f32, theme::border2()),
    );

    let devices = ctx.view.devices();
    let mut groups: Vec<usize> = Vec::new();
    for d in devices.iter().take(6) {
        groups.push((d.leds as usize).clamp(1, 24));
    }
    if groups.is_empty() {
        groups.push(16);
    }
    let dots: usize = groups.iter().map(|n| *n).sum::<usize>().max(1);
    let group_gap = 16.0;
    let usable =
        (rect.width() - 28.0 - group_gap * groups.len().saturating_sub(1) as f32).max(60.0);
    let step = (usable / dots as f32).clamp(5.5, 13.0);
    let dot = step * 0.55;
    let total = step * dots as f32 + group_gap * groups.len().saturating_sub(1) as f32;
    let mut x = (rect.center().x - total * 0.5).max(rect.left() + 12.0);
    let cy = rect.center().y;

    for (gi, n) in groups.iter().enumerate() {
        for i in 0..*n {
            let t = if *n > 1 {
                i as f32 / (*n as f32 - 1.0)
            } else {
                0.5
            };
            let c = ui_kit::lerp_color(ui_kit::with_alpha(color, 110), color, 0.35 + t * 0.65);
            let center = Pos2::new(x + dot * 0.5, cy);
            p.circle_filled(center, dot * 0.5 + 2.0, ui_kit::with_alpha(c, 30));
            p.circle_filled(center, dot * 0.5, c);
            x += step;
        }
        if gi + 1 < groups.len() {
            x += group_gap;
        }
    }
    p.text(
        Pos2::new(rect.left() + 12.0, rect.bottom() - 9.0),
        egui::Align2::LEFT_BOTTOM,
        "HUD · AURA COM PREVIEW",
        egui::FontId::monospace(8.5),
        ui_kit::with_alpha(Color32::WHITE, 110),
    );
}

// ---- modes ------------------------------------------------------------------

fn modes_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let modes = ctx.view.modes();
    let current = ctx.cfg.lighting.mode;
    let mut pick: Option<u32> = None;

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "效果模式");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} 种", modes.len()))
                        .size(9.5)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(6.0);
        if modes.is_empty() {
            ui_kit::notice_card(
                ui,
                theme::subtext(),
                "没有可用模式",
                "Aura 服务没有返回 get_QueryAllDeviceCap 的模式表。",
                None,
            );
            return;
        }
        let gap = 8.0;
        let cols = 2usize;
        let w = ((ui.available_width() - gap) / cols as f32).max(110.0);
        let rows = (modes.len() + cols - 1) / cols;
        for r in 0..rows {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for j in 0..cols {
                    let idx = r * cols + j;
                    let Some(m) = modes.get(idx) else { break };
                    if j > 0 {
                        ui.add_space(gap);
                    }
                    let sel = m.key == current;
                    ui.allocate_ui_with_layout(
                        Vec2::new(w, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(w);
                            let text = RichText::new(&m.name).size(11.5).color(if sel {
                                Color32::WHITE
                            } else {
                                theme::text()
                            });
                            let fill = if sel {
                                theme::accent()
                            } else {
                                theme::extreme()
                            };
                            let resp = ui.add(
                                egui::Button::new(text)
                                    .fill(fill)
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if sel {
                                            theme::accent()
                                        } else {
                                            theme::border()
                                        },
                                    ))
                                    .rounding(Rounding::same(8.0))
                                    .min_size(Vec2::new(w, 32.0)),
                            );
                            if resp.clicked() {
                                pick = Some(m.key);
                            }
                            if !m.can_color {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new("不支持调色").size(9.0).color(theme::faint()),
                                );
                            }
                            ui.add_space(2.0);
                        },
                    );
                }
            });
            ui.add_space(8.0);
        }
    });

    if let Some(key) = pick {
        if ctx.cfg.lighting.mode != key {
            ctx.cfg.lighting.mode = key;
            save_edit(ui, ctx);
        }
    }
}

// ---- color / brightness / speed ---------------------------------------------

fn params_card(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    let current = ctx.cfg.lighting.mode;
    let can_speed = ctx
        .view
        .modes()
        .iter()
        .find(|m| m.key == current)
        .map(|m| m.can_speed)
        .unwrap_or(true);
    let can_color = ctx
        .view
        .modes()
        .iter()
        .find(|m| m.key == current)
        .map(|m| m.can_color)
        .unwrap_or(true);
    let mut switched = false;

    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "效果参数");
        ui.add_space(7.0);

        ui.label(RichText::new("颜色").size(9.8).color(theme::faint()));
        ui.add_space(3.0);
        let mut chosen: Option<Color32> = None;
        let selected = ui_kit::parse_hex(&ctx.cfg.lighting.color);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            for (hex, _) in PRESETS.iter() {
                let c = ui_kit::parse_hex(hex);
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                ui.painter().rect_filled(rect, Rounding::same(7.0), c);
                if ui_kit::to_hex(selected) == *hex {
                    ui.painter().rect_stroke(
                        rect.expand(1.5),
                        Rounding::same(8.5),
                        Stroke::new(1.6_f32, theme::accent()),
                    );
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    chosen = Some(c);
                }
            }
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui_kit::mono(ui, &ui_kit::to_hex(selected), 12.0, theme::text());
                ui.label(RichText::new("当前主色").size(8.5).color(theme::faint()));
            });
        });
        if let Some(c) = chosen {
            ctx.cfg.lighting.color = ui_kit::to_hex(c);
            save_edit(ui, ctx);
        }
        if !can_color {
            ui.add_space(3.0);
            ui.label(
                RichText::new("当前模式不支持调色，写入会被 Aura 忽略。")
                    .size(9.0)
                    .color(theme::amber()),
            );
        }

        ui.add_space(10.0);
        let mut brightness = ctx.cfg.lighting.brightness as f32;
        if slider_row(ui, "亮度", &mut brightness, theme::accent()) {
            ctx.cfg.lighting.brightness = brightness.round().clamp(0.0, 100.0) as u8;
            save_edit(ui, ctx);
        }

        ui.add_space(8.0);
        if can_speed {
            let mut speed = ctx.cfg.lighting.speed as f32;
            if slider_row(ui, "速度", &mut speed, theme::mint()) {
                ctx.cfg.lighting.speed = speed.round().clamp(0.0, 100.0) as u8;
                save_edit(ui, ctx);
            }
        } else {
            ui.label(RichText::new("速度").size(9.8).color(theme::faint()));
            ui.label(
                RichText::new("当前模式不支持调速")
                    .size(9.0)
                    .color(theme::faint()),
            );
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("写入前解除独占")
                        .size(10.5)
                        .color(theme::text()),
                );
                ui.label(
                    RichText::new("关闭时直接覆盖，可能让 Armoury Crate 失联")
                        .size(8.5)
                        .color(theme::faint()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let before = ctx.cfg.lighting.exclusive_on_write;
                if ui_kit::switch(ui, &mut ctx.cfg.lighting.exclusive_on_write).clicked() {
                    switched = switched || before != ctx.cfg.lighting.exclusive_on_write;
                }
            });
        });
    });

    if switched {
        ctx.actions.push(PageAction::Save);
    }
}

fn slider_row(ui: &mut egui::Ui, label: &str, value: &mut f32, color: Color32) -> bool {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(9.8).color(theme::faint()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui_kit::mono(ui, &format!("{:.0}%", *value), 10.5, theme::subtext());
        });
    });
    ui_kit::coarse_slider(ui, value, 0.0, 100.0, color)
}

/// 拖动滑杆时节流保存：最多每 0.4s 落盘一次。
fn save_edit(ui: &egui::Ui, ctx: &mut HwCtx) {
    let now = ui.input(|i| i.time);
    let due = ui.data_mut(|d| {
        let last = d.get_temp_mut_or_insert_with(egui::Id::new("hw_light_save_at"), || 0.0_f64);
        if now - *last > 0.4 {
            *last = now;
            true
        } else {
            false
        }
    });
    if due {
        ctx.actions.push(PageAction::Save);
    }
}

// ---- apply / refresh --------------------------------------------------------

fn apply_row(ctx: &mut HwCtx, ui: &mut egui::Ui) {
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!ctx.applying, |ui| {
                let label = if ctx.applying {
                    "下发中…"
                } else {
                    "应用到全部设备"
                };
                if ui_kit::primary_button(ui, label, false).clicked() && !ctx.applying {
                    ctx.actions.push(PageAction::ApplyAura);
                }
            });
            if ui_kit::ghost_button(ui, "刷新读数").clicked() {
                ctx.actions.push(PageAction::RefreshHardware);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ctx.applying {
                    ui_kit::status_pill(ui, "正在写入…", theme::amber());
                } else {
                    ui_kit::status_pill(ui, "就绪", theme::mint());
                }
            });
        });
        if let Some((ok, msg)) = &ctx.apply_msg {
            ui.add_space(8.0);
            let (tone, title) = if *ok {
                (theme::mint(), "下发成功")
            } else {
                (theme::danger(), "下发失败")
            };
            ui_kit::notice_card(ui, tone, title, msg, None);
        }
        if ctx.view.aura.exclusive {
            ui.add_space(8.0);
            ui_kit::notice_card(
                ui,
                theme::amber(),
                "当前处于独占态",
                "Aura 被独占时 Armoury Crate 自己的灯效页会失联；下发会自动先解除、写完再还 1。",
                None,
            );
        }
    });
}
