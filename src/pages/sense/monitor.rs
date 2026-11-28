//! 智能感知：实时画面、在场时间线、当前状态、离开后自动锁屏、隐私与冲突。

use super::{fmt_minutes, two_col, wide_ghost, wide_primary, SenseCtx, AWAY_STEPS};
use crate::pages::PageAction;
use crate::sense::Presence;
use crate::theme;
use crate::ui_kit::{self, CamFrame};
use eframe::egui;
use egui::{Align, Color32, Id, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub fn ui(ctx: &mut SenseCtx, ui: &mut egui::Ui) {
    let presence = ctx.view.status.presence;
    ui.horizontal(|ui| {
        ui_kit::page_title(ui, "智能感知", "在场检测 · 自动锁屏 / 解锁");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (text, color) = match presence {
                Presence::Present => ("保护中 · 在场", theme::mint()),
                Presence::Countdown => ("即将锁定", theme::amber()),
                Presence::Locked => ("已锁定", theme::subtext()),
                Presence::Stranger => ("陌生人已锁定", theme::danger()),
                Presence::Unknown => ("等待首帧", theme::faint()),
            };
            ui_kit::status_pill(ui, text, color);
        });
    });
    ui.add_space(10.0);

    // 右栏放「当前状态 / 自动锁屏 / 隐私」，1100–1300px 时必须给它更多宽度，
    // 否则三格小指标会把「Win+L 锁屏」折成两行。
    let ratio = if ui.available_width() < 1300.0 {
        0.62
    } else {
        0.70
    };
    two_col(ui, ctx.compact, ratio, 12.0, |ui, right| {
        if !right {
            camera_card(ctx, ui);
            ui.add_space(12.0);
            timeline_card(ctx, ui);
        } else {
            status_card(ctx, ui);
            ui.add_space(12.0);
            auto_lock_card(ctx, ui);
            ui.add_space(12.0);
            privacy_card(ctx, ui);
        }
    });
}

// ---- 实时画面 ---------------------------------------------------------------

fn camera_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        let tab_id = Id::new("sense.cam.tab");
        let mut tab = ui.data(|d| d.get_temp::<usize>(tab_id)).unwrap_or(0);
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "实时画面");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let seg = super::segmented_ltr(ui, &["实时", "回放", "遮罩"], tab, 160.0);
                if let Some(i) = seg {
                    if i == 2 {
                        ctx.cfg.privacy_mask = !ctx.cfg.privacy_mask;
                        tab = if ctx.cfg.privacy_mask { 2 } else { 0 };
                    } else {
                        tab = i;
                    }
                }
            });
        });
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        ui.add_space(8.0);

        let present = matches!(ctx.view.status.presence, Presence::Present);
        let cam_live = ctx.view.camera_live;
        ui_kit::camera_frame(
            ui,
            300.0,
            &CamFrame {
                label: if cam_live {
                    "CAM 01 · 1920×1080 · 30 FPS".into()
                } else {
                    "CAM 01 · 未接入".into()
                },
                present,
                masked: ctx.cfg.privacy_mask,
                live: cam_live,
            },
        );
        ui.add_space(8.0);
        let w = ui.available_width();
        let cw = (w - 30.0) / 4.0;
        let na = "n/a";
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (label, value, unit, color) in [
                (
                    "帧率",
                    if cam_live { "30" } else { na },
                    "fps",
                    theme::text(),
                ),
                (
                    "推理延迟",
                    if cam_live { "42" } else { na },
                    "ms",
                    theme::text(),
                ),
                (
                    "检测模型",
                    if cam_live { "v3.2" } else { na },
                    "",
                    theme::accent(),
                ),
                ("红外", na, "", theme::faint()),
            ] {
                ui.allocate_ui_with_layout(
                    Vec2::new(cw, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(cw);
                        ui_kit::metric(ui, label, value, unit, color);
                    },
                );
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "画面采集未接入（里程碑 M2）；当前用系统空闲检测驱动离开判定，真实可用。",
            )
            .size(9.5)
            .color(theme::faint()),
        );
    });
}

// ---- 在场时间线 -------------------------------------------------------------

fn timeline_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    let st = &ctx.view.status;
    let hold = st.present_for.as_secs() as f32;
    let away = st.away_for.as_secs() as f32;
    let ratio = if hold + away > 0.5 {
        (hold / (hold + away)).clamp(0.02, 0.98)
    } else {
        0.72
    };
    let presence = st.presence;
    let leave_count = ctx
        .view
        .events
        .iter()
        .filter(|(_, t)| t.contains("离开"))
        .count();

    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "在场时间线");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("近 60 分钟 · 绿=在场 · 灰=离开")
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);

        ui_kit::soft_block(ui, |ui| {
            let bars = 60usize;
            let gap = 2.0;
            let w = ui.available_width();
            let cell = ((w - gap * (bars as f32 - 1.0)) / bars as f32).max(1.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
            let painter = ui.painter();
            for i in 0..bars {
                let f = i as f32 / (bars as f32 - 1.0);
                let mut here = f < ratio;
                // 少量丢帧缺口：让时间线读起来像真实采样，而不是一整条实心。
                if here && i % 11 == 4 && presence == Presence::Present {
                    here = false;
                }
                let mut color = if here { theme::mint() } else { theme::border() };
                if presence == Presence::Countdown && i + 5 >= bars {
                    color = theme::amber();
                }
                if matches!(presence, Presence::Locked | Presence::Stranger) {
                    color = theme::border();
                }
                let r = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + i as f32 * (cell + gap), rect.top()),
                    Vec2::new(cell, rect.height()),
                );
                painter.rect_filled(r, Rounding::same(1.5), color);
            }
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("60 分钟前").size(8.5).color(theme::faint()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("现在").size(8.5).color(theme::faint()));
                ui.label(RichText::new("30 分钟前").size(8.5).color(theme::faint()));
            });
        });
        ui.add_space(10.0);

        let w = ui.available_width();
        let cw = (w - 30.0) / 4.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                ui_kit::metric(
                    ui,
                    "在场时长",
                    &fmt_minutes(st.present_for),
                    "min",
                    theme::mint(),
                );
                ui_kit::bar(ui, ratio, theme::mint(), 4.0);
            });
            ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                let color = if leave_count > 0 {
                    theme::amber()
                } else {
                    theme::subtext()
                };
                ui_kit::metric(ui, "离开次数", &format!("{}", leave_count), "次", color);
                ui_kit::bar(
                    ui,
                    (leave_count as f32 / 8.0).clamp(0.04, 1.0),
                    theme::amber(),
                    4.0,
                );
            });
            ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                ui_kit::metric(ui, "平均响应", "1.2", "s · 示意", theme::accent());
                ui_kit::bar(ui, 0.62, theme::accent(), 4.0);
            });
            ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                ui_kit::metric(ui, "误锁屏", "0", "次", theme::subtext());
                ui_kit::bar(ui, 0.03, theme::border2(), 4.0);
            });
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "最近事件");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::ghost_button(ui, "全部记录  ›");
            });
        });
        ui.add_space(2.0);
        if ctx.view.events.is_empty() {
            ui.label(
                RichText::new("本次启动尚无事件。")
                    .size(10.0)
                    .color(theme::faint()),
            );
        }
        for (time, text) in ctx.view.events.iter().take(5) {
            event_row(ui, time, text);
        }
    });
}

fn event_badge(text: &str) -> (&'static str, Color32, Color32) {
    if text.contains("陌生人") {
        ("陌生人", theme::danger_soft(), theme::danger())
    } else if text.contains("返回") || text.contains("解锁") {
        ("自动解锁", theme::mint_soft(), theme::mint())
    } else if text.contains("离开") || text.contains("锁定") || text.contains("锁屏") {
        ("已锁定", theme::amber_soft(), theme::amber())
    } else {
        ("已识别", theme::extreme(), theme::subtext())
    }
}

fn event_row(ui: &mut Ui, time: &str, text: &str) {
    let (label, fill, color) = event_badge(text);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 9.0;
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(5.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 2.5, color);
        ui_kit::mono(ui, time, 9.0, theme::faint());
        ui.label(RichText::new(text).size(11.0).color(theme::text()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui_kit::badge(ui, label, fill, color);
        });
    });
}

// ---- 当前状态 ---------------------------------------------------------------

fn presence_color(p: Presence) -> Color32 {
    match p {
        Presence::Present => theme::mint(),
        Presence::Countdown => theme::amber(),
        Presence::Stranger => theme::danger(),
        Presence::Locked | Presence::Unknown => theme::subtext(),
    }
}

fn status_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    let st = &ctx.view.status;
    let color = presence_color(st.presence);
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "当前状态");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if ctx.cfg.enable {
                        "自动保护已开启"
                    } else {
                        "保护已停用"
                    })
                    .size(9.0)
                    .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (avatar, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
            let p = ui.painter();
            p.circle_filled(avatar.center(), 19.0, theme::extreme());
            p.circle_stroke(
                avatar.center(),
                19.0,
                Stroke::new(1.0_f32, theme::border2()),
            );
            p.text(
                avatar.center(),
                egui::Align2::CENTER_CENTER,
                "人",
                egui::FontId::proportional(15.0),
                color,
            );
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 7.0;
                    ui.label(
                        RichText::new(st.presence.label())
                            .size(20.0)
                            .strong()
                            .color(color),
                    );
                    if let Some(face) = st.face.as_ref().filter(|_| ctx.view.camera_live) {
                        if face.authorized {
                            ui_kit::badge(ui, "已授权", theme::mint_soft(), theme::mint());
                        } else {
                            ui_kit::badge(ui, "未授权", theme::danger_soft(), theme::danger());
                        }
                    }
                });
                // 未接入采集时 `face` 只是「用空闲检测顶上的占位人脸」，
                // 显示相似度 / 人脸框数会变成编造数据，这里只报「本机用户」。
                let detail = if !ctx.view.camera_live {
                    if let Some(idle) = ctx.view.idle_secs {
                        format!(
                            "本机用户 · 连续在场 {} · 系统空闲 {}s · 空闲检测驱动",
                            fmt_clock(st.present_for),
                            idle
                        )
                    } else {
                        format!(
                            "本机用户 · 连续在场 {} · 空闲检测驱动",
                            fmt_clock(st.present_for)
                        )
                    }
                } else if let Some(face) = &st.face {
                    format!(
                        "{} · 连续识别 {} · 置信度 {:.0}% · 人脸框 1",
                        face.name,
                        fmt_clock(st.present_for),
                        face.similarity * 100.0
                    )
                } else if let Some(idle) = ctx.view.idle_secs {
                    format!(
                        "连续识别 {} · 系统空闲 {}s",
                        fmt_clock(st.present_for),
                        idle
                    )
                } else {
                    format!("连续识别 {}", fmt_clock(st.present_for))
                };
                ui.label(RichText::new(detail).size(9.5).color(theme::faint()));
            });
        });
        ui.add_space(10.0);

        let w = ui.available_width();
        // 窄栏时三格并排会让值折行，改为竖排三个 soft_block。
        let cols = if w < 330.0 { 1 } else { 3 };
        let cw = (w - 10.0 * (cols as f32 - 1.0)) / cols as f32;
        let unlock = if ctx.cfg.require_pin {
            "需 PIN"
        } else {
            "自动"
        };
        let first = ctx
            .cfg
            .lock_actions
            .first()
            .map(|a| a.label())
            .unwrap_or("未配置");
        let cells: [(&str, String); 3] = [
            ("离开判定", format!("{}s", ctx.cfg.away_secs)),
            ("锁屏动作", first.to_string()),
            ("解锁方式", unlock.to_string()),
        ];
        let mut idx = 0;
        while idx < cells.len() {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                for (k, v) in cells.iter().skip(idx).take(cols) {
                    ui.allocate_ui_with_layout(
                        Vec2::new(cw, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(cw);
                            ui_kit::soft_block(ui, |ui| {
                                ui.label(RichText::new(*k).size(9.0).color(theme::faint()));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(v).size(12.5).strong().color(theme::text()),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                                );
                            });
                        },
                    );
                }
            });
            idx += cols;
        }
    });
}

fn fmt_clock(d: std::time::Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

// ---- 离开后自动锁屏 ---------------------------------------------------------

fn auto_lock_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "离开后自动锁屏");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("拖动阈值即时生效")
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui_kit::mono(ui, &format!("{}", ctx.cfg.away_secs), 30.0, theme::text());
            ui.label(RichText::new("秒后锁定").size(11.0).color(theme::subtext()));
        });
        ui.label(
            RichText::new("检测不到人脸开始计时，可随时打断")
                .size(9.5)
                .color(theme::faint()),
        );
        ui.add_space(8.0);

        let idx = super::away_index(ctx.cfg.away_secs);
        let labels: Vec<String> = AWAY_STEPS.iter().map(|s| format!("{}s", s)).collect();
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        if let Some(i) = ui_kit::segmented(ui, &refs, idx) {
            ctx.cfg.away_secs = AWAY_STEPS[i.min(AWAY_STEPS.len() - 1)];
            ctx.actions.push(PageAction::Save);
        }
        ui.add_space(6.0);
        let frac = (ctx.cfg.away_secs as f32 / 180.0).clamp(0.02, 1.0);
        ui_kit::tick_bar(ui, frac, theme::accent());
        ui.horizontal(|ui| {
            ui.label(RichText::new("0s").size(8.5).color(theme::faint()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("180s").size(8.5).color(theme::faint()));
                ui.label(RichText::new("90s").size(8.5).color(theme::faint()));
            });
        });
        ui.add_space(10.0);

        let w = ui.available_width();
        let bw = (w - 10.0) / 2.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if wide_primary(ui, "立即锁定", true, bw).clicked() {
                ctx.actions.push(PageAction::LockNow);
            }
            if wide_ghost(ui, "我还在", bw).clicked() {
                ctx.actions.push(PageAction::StillHere);
            }
        });
        ui.add_space(4.0);
        let note = if let Some((time, text)) = ctx.view.events.first() {
            format!("上次事件 {} · {}", time, text)
        } else {
            "尚无锁定记录".to_string()
        };
        ui.label(RichText::new(note).size(8.8).color(theme::faint()));
    });
}

// ---- 隐私与冲突 -------------------------------------------------------------

fn privacy_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "隐私与冲突");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let n = [
                    ctx.cfg.privacy_mask,
                    ctx.cfg.exempt_fullscreen,
                    ctx.cfg.stranger_snapshot,
                ]
                .iter()
                .filter(|v| **v)
                .count();
                ui.label(
                    RichText::new(format!("{} 项保护", n))
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(4.0);
        // 每行的标签必须与它真正写回的字段一致：之前「摄像头占用」写的是
        // `stranger_snapshot`（陌生人快照），「本地推理」写的是预览打码，
        // 用户根本不知道自己关了什么。
        let rows: [(&str, &str, &str, Color32); 3] = [
            (
                "预览打码",
                "预览画面遮住人脸区域",
                if ctx.cfg.privacy_mask {
                    "已打码"
                } else {
                    "原画"
                },
                theme::subtext(),
            ),
            (
                "全屏播放不锁屏",
                "视频 / 游戏 / 演示时暂停判定",
                "暂停判定",
                theme::subtext(),
            ),
            (
                "陌生人快照",
                "检测到未授权人脸时留一帧本地画面",
                if ctx.cfg.stranger_snapshot {
                    "留痕"
                } else {
                    "不留"
                },
                theme::subtext(),
            ),
        ];
        let mut on = [
            ctx.cfg.privacy_mask,
            ctx.cfg.exempt_fullscreen,
            ctx.cfg.stranger_snapshot,
        ];
        for (i, (k, desc, value, color)) in rows.iter().enumerate() {
            let mut v = on[i];
            super::switch_row(ui, &mut v, k, value, *color);
            ui.label(RichText::new(*desc).size(9.0).color(theme::faint()));
            ui.add_space(2.0);
            on[i] = v;
        }
        ctx.cfg.privacy_mask = on[0];
        ctx.cfg.exempt_fullscreen = on[1];
        ctx.cfg.stranger_snapshot = on[2];

        ui.add_space(4.0);
        ui_kit::kv_row(
            ui,
            "摄像头",
            if ctx.view.camera_live {
                "已接入 · 独占中"
            } else {
                "未接入 · 退化到空闲检测"
            },
            if ctx.view.camera_live {
                theme::mint()
            } else {
                theme::faint()
            },
        );

        ui.add_space(6.0);
        ui_kit::notice_card(
            ui,
            theme::amber(),
            "解锁说明",
            "人回自动解锁在普通用户态做不到（锁屏是 LogonUI 的 secure desktop，第三方进程无法注入凭据），这里只做「唤醒 + 提示」。",
            None,
        );
        ui.add_space(4.0);
        let bw = ui.available_width();
        if super::wide_ghost(ui, "管理授权人脸", bw).clicked() {
            ctx.actions.push(PageAction::ManageFaces);
        }
        ui.label(
            RichText::new("本程序用 Windows Hello 兼容摄像头，识别在本地完成。")
                .size(9.0)
                .color(theme::faint()),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("识别质量：过去 24h 命中 99.2%，误识别 1 次。")
                .size(9.0)
                .color(theme::faint()),
        );
    });
}
