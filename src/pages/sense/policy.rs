//! 锁屏策略：场景预设、离开判定、锁定动作、解锁策略、安全兜底、授权人脸。

use super::{fmt_minutes, mem_switch, setting_row, two_col, SenseCtx, AWAY_STEPS};
use crate::config::LockAction;
use crate::pages::PageAction;
use crate::theme;
use crate::ui_kit;
use eframe::egui;
use egui::{Align, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub fn ui(ctx: &mut SenseCtx, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui_kit::page_title(ui, "锁屏策略", "离开多久锁 · 谁能解锁");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui_kit::status_pill(
                ui,
                if ctx.cfg.enable {
                    "策略已启用"
                } else {
                    "已停用"
                },
                if ctx.cfg.enable {
                    theme::accent()
                } else {
                    theme::faint()
                },
            );
        });
    });
    ui.add_space(10.0);

    presets(ctx, ui);
    ui.add_space(12.0);

    two_col(ui, ctx.compact, 0.5, 12.0, |ui, right| {
        if !right {
            away_card(ctx, ui);
            ui.add_space(12.0);
            actions_card(ctx, ui);
        } else {
            unlock_card(ctx, ui);
            ui.add_space(12.0);
            fallback_card(ctx, ui);
        }
    });
    ui.add_space(12.0);

    two_col(ui, ctx.compact, 0.5, 12.0, |ui, right| {
        if !right {
            faces_card(ctx, ui);
        } else {
            recent_card(ctx, ui);
        }
    });
}

// ---- 场景预设 ---------------------------------------------------------------

struct Preset {
    name: &'static str,
    desc: &'static str,
    away: u32,
    pin: bool,
    stranger: bool,
    auto_unlock: bool,
}

const PRESETS: [Preset; 3] = [
    Preset {
        name: "办公室",
        desc: "30s · 自动唤醒提示 · 陌生人策略待接入",
        away: 30,
        pin: false,
        stranger: true,
        auto_unlock: true,
    },
    Preset {
        name: "家庭",
        desc: "60s · 仅本人解锁",
        away: 60,
        pin: false,
        stranger: false,
        auto_unlock: true,
    },
    Preset {
        name: "公共空间",
        desc: "10s · 需 PIN 提示 · 快照待接入",
        away: 10,
        pin: true,
        stranger: true,
        auto_unlock: false,
    },
];

fn selected_preset(cfg: &crate::config::SenseCfg) -> Option<usize> {
    // 设计稿只按「离开秒数 + 是否要 PIN」区分预设：这三组在二维上已互不相同，
    // 再把陌生人策略算进来会让用户手动改过它之后整组预设都显示未选中。
    PRESETS
        .iter()
        .position(|p| p.away == cfg.away_secs && p.pin == cfg.require_pin)
}

fn presets(ctx: &mut SenseCtx, ui: &mut Ui) {
    let selected = selected_preset(ctx.cfg);
    let cols = if ctx.compact || ui.available_width() < 620.0 {
        1
    } else {
        3
    };
    let gap = 12.0;
    let w = ui.available_width();
    let card_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;

    let mut clicked: Option<usize> = None;
    for row in 0..(PRESETS.len() + cols - 1) / cols {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for c in 0..cols {
                let i = row * cols + c;
                if i >= PRESETS.len() {
                    continue;
                }
                let p = &PRESETS[i];
                let on = selected == Some(i);
                let resp = egui::Frame::none()
                    .fill(if on {
                        theme::accent_soft()
                    } else {
                        theme::card()
                    })
                    .stroke(Stroke::new(
                        if on { 1.5_f32 } else { 1.0_f32 },
                        if on { theme::accent() } else { theme::border() },
                    ))
                    .rounding(Rounding::same(ui_kit::CARD_R))
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                    .show(ui, |ui| {
                        // `Frame::show` 继承父 Ui 的排版方向；网格在
                        // `horizontal_top` 里，必须显式切成竖向，否则卡片内容横排。
                        ui.vertical(|ui| {
                            ui.set_width(card_w - 28.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(p.name)
                                        .size(12.5)
                                        .strong()
                                        .color(theme::text()),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if on {
                                        ui_kit::badge(ui, "当前", theme::accent(), theme::card());
                                    }
                                });
                            });
                            ui.add_space(2.0);
                            ui.label(RichText::new(p.desc).size(9.5).color(if on {
                                theme::accent_deep()
                            } else {
                                theme::faint()
                            }));
                        });
                    })
                    .response;
                let resp = resp
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
    }

    if let Some(i) = clicked {
        let p = &PRESETS[i];
        ctx.cfg.away_secs = p.away;
        ctx.cfg.require_pin = p.pin;
        ctx.cfg.stranger_lock = p.stranger;
        ctx.cfg.auto_unlock = p.auto_unlock;
        ctx.actions.push(PageAction::Save);
    }
}

// ---- 离开判定 ---------------------------------------------------------------

fn away_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "离开判定");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("三步协同 · 避免误锁")
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);

        let idx = super::away_index(ctx.cfg.away_secs);
        let labels: Vec<String> = AWAY_STEPS.iter().map(|s| format!("{}s", s)).collect();
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let picked = {
            let mut picked = None;
            setting_row(
                ui,
                if ctx.view.camera_live {
                    "人脸消失时长"
                } else {
                    "键鼠空闲时长"
                },
                if ctx.view.camera_live {
                    "连续检测不到人脸达到阈值后开始锁定倒计时"
                } else {
                    "摄像头未接入：连续无键鼠输入达到阈值后开始锁定倒计时"
                },
                super::segments_width(ui, &refs),
                |ui| {
                    picked = super::segmented_right(ui, &refs, idx);
                },
            );
            picked
        };
        if let Some(i) = picked {
            ctx.cfg.away_secs = AWAY_STEPS[i.min(AWAY_STEPS.len() - 1)];
            ctx.actions.push(PageAction::Save);
        }

        setting_row(
            ui,
            "二次确认",
            "倒计时结束前再做一次宽松检测，防止转头误判",
            40.0,
            |ui| {
                ui_kit::switch(ui, &mut ctx.cfg.confirm_second_pass);
            },
        );
        setting_row(
            ui,
            "全屏暂停",
            "检测到视频 / 游戏 / 演示窗口时不判定离开",
            40.0,
            |ui| {
                ui_kit::switch(ui, &mut ctx.cfg.exempt_fullscreen);
            },
        );
        setting_row(
            ui,
            "输入活动续期",
            "键鼠有输入时重置计时（最长豁免 3 分钟）",
            40.0,
            |ui| {
                let mut on = ctx.cfg.input_reset_secs > 0;
                ui_kit::switch(ui, &mut on);
                if !on && ctx.cfg.input_reset_secs > 0 {
                    ctx.cfg.input_reset_secs = 0;
                    ctx.actions.push(PageAction::Save);
                } else if on && ctx.cfg.input_reset_secs == 0 {
                    ctx.cfg.input_reset_secs = 180;
                    ctx.actions.push(PageAction::Save);
                }
            },
        );
        if ctx.cfg.input_reset_secs > 0 {
            let mut v = ctx.cfg.input_reset_secs as f32;
            ui.add_space(2.0);
            if ui_kit::coarse_slider(ui, &mut v, 0.0, 300.0, theme::accent()) {
                ctx.cfg.input_reset_secs = v.round() as u32;
                ctx.actions.push(PageAction::Save);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("0").size(8.5).color(theme::faint()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("上限 5 分").size(8.5).color(theme::faint()));
                    ui.label(
                        RichText::new(format!(
                            "当前 {} 分",
                            fmt_minutes(std::time::Duration::from_secs(
                                ctx.cfg.input_reset_secs as u64
                            ))
                        ))
                        .size(8.5)
                        .strong()
                        .color(theme::accent_deep()),
                    );
                });
            });
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("检测灵敏度")
                    .size(11.5)
                    .strong()
                    .color(theme::text()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui_kit::badge(
                    ui,
                    &format!("{:.0}%", ctx.cfg.threshold * 100.0),
                    theme::accent_soft(),
                    theme::accent_deep(),
                );
            });
        });
        ui.label(
            RichText::new("越高越早判定离开，误锁率也越高")
                .size(9.5)
                .color(theme::faint()),
        );
        let mut t = ctx.cfg.threshold;
        if ui_kit::coarse_slider(ui, &mut t, 0.30, 0.90, theme::accent()) {
            ctx.cfg.threshold = t.clamp(0.30, 0.90);
            ctx.actions.push(PageAction::Save);
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("宽松 · 少误锁")
                    .size(8.5)
                    .color(theme::faint()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("严格 · 更安全")
                        .size(8.5)
                        .color(theme::faint()),
                );
            });
        });
    });
}

// ---- 锁定动作 ---------------------------------------------------------------

const ACTIONS: [LockAction; 4] = [
    LockAction::Lock,
    LockAction::MonitorOff,
    LockAction::Mute,
    LockAction::Blur,
];

fn action_icon(a: LockAction) -> &'static str {
    match a {
        LockAction::Lock => "🔒",
        LockAction::MonitorOff => "⏻",
        LockAction::Mute => "🔇",
        LockAction::Blur => "▣",
    }
}

fn action_desc(a: LockAction) -> &'static str {
    match a {
        LockAction::Lock => "系统锁定",
        LockAction::MonitorOff => "关闭显示器",
        LockAction::Mute => "暂停媒体",
        LockAction::Blur => "隐藏壁纸",
    }
}

fn actions_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "锁定动作");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("可叠加 · 按顺序执行")
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);

        let cols = if ui.available_width() < 420.0 { 2 } else { 4 };
        let gap = 10.0;
        let w = ui.available_width();
        let tile_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;

        let mut toggled: Option<LockAction> = None;
        for row in 0..(ACTIONS.len() + cols - 1) / cols {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= ACTIONS.len() {
                        continue;
                    }
                    let a = ACTIONS[i];
                    let on = ctx.cfg.lock_actions.contains(&a);
                    let resp = egui::Frame::none()
                        .fill(if on {
                            theme::accent_soft()
                        } else {
                            theme::card()
                        })
                        .stroke(Stroke::new(
                            if on { 1.2_f32 } else { 1.0_f32 },
                            if on { theme::accent() } else { theme::border() },
                        ))
                        .rounding(Rounding::same(ui_kit::CTL_R))
                        .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(tile_w - 24.0);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    ui.label(RichText::new(action_icon(a)).size(11.5).color(
                                        if on {
                                            theme::accent()
                                        } else {
                                            theme::subtext()
                                        },
                                    ));
                                    ui.label(
                                        RichText::new(a.label())
                                            .size(11.0)
                                            .strong()
                                            .color(theme::text()),
                                    );
                                });
                                ui.label(
                                    RichText::new(action_desc(a))
                                        .size(9.0)
                                        .color(theme::faint()),
                                );
                            });
                        })
                        .response;
                    let resp = resp
                        .interact(Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        toggled = Some(a);
                    }
                }
            });
        }

        let mut warn = false;
        if let Some(a) = toggled {
            let mut set: Vec<LockAction> = ctx.cfg.lock_actions.clone();
            if let Some(pos) = set.iter().position(|x| *x == a) {
                set.remove(pos);
                if a == LockAction::Lock {
                    warn = true;
                }
            } else {
                set.push(a);
            }
            // 固定顺序：Blur → MonitorOff → Mute → Lock（先软动作，后硬动作）。
            let order = [
                LockAction::Blur,
                LockAction::MonitorOff,
                LockAction::Mute,
                LockAction::Lock,
            ];
            let mut rebuilt: Vec<LockAction> = Vec::new();
            for o in order {
                if set.contains(&o) {
                    rebuilt.push(o);
                }
            }
            if !rebuilt.contains(&LockAction::Lock) {
                rebuilt.push(LockAction::Lock);
                warn = true;
            }
            if rebuilt != ctx.cfg.lock_actions {
                ctx.cfg.lock_actions = rebuilt;
                ctx.actions.push(PageAction::Save);
            }
            if warn {
                super::flash(ui, "policy.need_lock", 2.5);
            }
        }

        if super::flashing(ui, "policy.need_lock") {
            ui.label(
                RichText::new("必须保留锁屏：已自动补回「Win+L 锁屏」")
                    .size(9.5)
                    .strong()
                    .color(theme::danger()),
            );
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "先关屏再锁系统，减少锁屏瞬间的画面暴露；息屏唤醒需人站在原 1.5s 以上，避免「路过瞥一眼」被当成返回。",
            )
            .size(9.0)
            .color(theme::faint()),
        );
    });
}

// ---- 解锁策略 ---------------------------------------------------------------

fn unlock_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "解锁策略");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if ctx.view.camera_live {
                        "仅授权人脸"
                    } else {
                        "人脸识别未接入"
                    })
                    .size(9.0)
                    .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);

        setting_row(
            ui,
            "回到座位自动解锁",
            if ctx.view.camera_live {
                "检测到已授权人脸 → 唤醒显示器并触发解锁提示"
            } else {
                "摄像头未接入：当前仅保存配置，不会识别人脸"
            },
            40.0,
            |ui| {
                ui_kit::switch(ui, &mut ctx.cfg.auto_unlock);
            },
        );

        let mut pin = ctx.cfg.require_pin;
        setting_row(
            ui,
            "解锁确认强度",
            if pin {
                "人脸 + PIN 双因子"
            } else {
                "单人脸直接解锁"
            },
            128.0,
            |ui| {
                let sel = if pin { 1 } else { 0 };
                if let Some(i) = super::segmented_right(ui, &["直接", "+PIN"], sel) {
                    pin = i == 1;
                }
            },
        );
        if pin != ctx.cfg.require_pin {
            ctx.cfg.require_pin = pin;
            ctx.actions.push(PageAction::Save);
        }

        setting_row(
            ui,
            "多用户切换",
            "识别用户 A 时切到 B，绝不自动登录他人",
            40.0,
            |ui| {
                mem_switch(ui, "policy.multi_user", false);
            },
        );
        ui.label(
            RichText::new("多用户切换需 Credential Provider，本版仅记录偏好")
                .size(9.0)
                .color(theme::faint()),
        );

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "解锁前置条件");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(if ctx.view.camera_live {
                        "全部满足才自动解锁"
                    } else {
                        "接入后判定"
                    })
                    .size(9.0)
                    .color(theme::faint()),
                );
            });
        });
        let items: [(&str, Option<bool>); 6] = [
            ("人脸相似度 ≥ 90%", None),
            ("Windows Hello 可用", None),
            ("无锁定策略冲突", None),
            ("账户在授权名单", None),
            ("非他人会话", None),
            ("离开时长 ≥ 5s", None),
        ];
        // 行优先铺：读起来是「左、右、左、右」，而不是先读完整列。
        let rows = (items.len() + 1) / 2;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let w = (ui.available_width() - 12.0) / 2.0;
            for col in 0..2 {
                ui.allocate_ui_with_layout(Vec2::new(w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(w);
                    for r in 0..rows {
                        let idx = r * 2 + col;
                        let Some((text, ok)) = items.get(idx) else {
                            continue;
                        };
                        match ok {
                            Some(true) => super::check_item(ui, true, text, theme::mint()),
                            Some(false) => super::check_item(ui, false, text, theme::danger()),
                            None => super::pending_item(ui, text),
                        }
                    }
                });
            }
        });
        ui.label(
            RichText::new(
                "摄像头采集与本地特征库尚未接入（里程碑 M2 / M3）；以上条件均标记为「—」，接入后才有真实结果。",
            )
            .size(9.0)
            .color(theme::faint()),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("ⓘ").size(10.5).color(theme::accent()));
            ui.label(
                RichText::new("解锁由 Credential Provider 完成；不注入密码、不改安全策略。")
                    .size(9.0)
                    .color(theme::subtext()),
            );
        });
    });
}

// ---- 安全兜底 ---------------------------------------------------------------

fn fallback_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "安全兜底");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("防止把自己锁在外面")
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);
        let _ = ctx;
        let cols = if ui.available_width() < 380.0 { 1 } else { 2 };
        let gap = 10.0;
        let w = ui.available_width();
        let cell_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let items: [(&str, &str, &str, &str); 4] = [
            (
                "policy.fb_pin",
                "识别失败 3 次",
                "自动退回 PIN / 密码",
                "未接入",
            ),
            (
                "policy.fb_cam",
                "摄像头不可用",
                "保留锁屏与空闲检测",
                "已生效",
            ),
            (
                "policy.fb_stranger",
                "陌生人脸",
                "需人脸识别接入后生效",
                "未接入",
            ),
            (
                "policy.fb_mislock",
                "连续误锁 3 次",
                "自动暂停感知 1 小时",
                "未接入",
            ),
        ];
        for row in 0..(items.len() + cols - 1) / cols {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= items.len() {
                        continue;
                    }
                    let (key, title, desc, state) = items[i];
                    ui.allocate_ui_with_layout(
                        Vec2::new(cell_w, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui_kit::soft_block(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(title)
                                            .size(11.0)
                                            .strong()
                                            .color(theme::text()),
                                    );
                                    let _ = key;
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui_kit::badge(
                                            ui,
                                            state,
                                            if state == "已生效" {
                                                theme::mint_soft()
                                            } else {
                                                theme::extreme()
                                            },
                                            if state == "已生效" {
                                                theme::mint()
                                            } else {
                                                theme::faint()
                                            },
                                        );
                                    });
                                });
                                ui.label(RichText::new(desc).size(9.3).color(theme::faint()));
                            });
                        },
                    );
                }
            });
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "摄像头不可用时，程序继续用键鼠空闲检测锁屏；陌生人相关规则需人脸识别接入后生效。",
            )
            .size(9.0)
            .color(theme::faint()),
        );
    });
}

// ---- 授权人脸 ---------------------------------------------------------------

fn faces_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "授权人脸");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} 人 · 仅名单占位（特征库未接入）",
                        ctx.cfg.authorized_faces.len()
                    ))
                    .size(9.0)
                    .color(theme::faint()),
                );
            });
        });
        ui.add_space(8.0);

        if ctx.cfg.authorized_faces.is_empty() {
            if ui_kit::notice_card(
                ui,
                theme::faint(),
                "尚无授权人脸",
                "摄像头采集与特征库在里程碑 M2/M3；现在不会读摄像头，也不会存任何生物特征。",
                Some("添加（需先接入摄像头）"),
            ) {}
            ui.add_space(4.0);
            ui.label(
                RichText::new("当前只保存姓名列表，不采集、不存储任何生物特征。")
                    .size(9.0)
                    .color(theme::faint()),
            );
            return;
        }

        let gap = 10.0;
        let w = ui.available_width();
        let cols = if w < 420.0 { 2 } else { 3 };
        let cell_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        for row in 0..(ctx.cfg.authorized_faces.len() + cols - 1) / cols {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in 0..cols {
                    let i = row * cols + c;
                    let Some(name) = ctx.cfg.authorized_faces.get(i).cloned() else {
                        continue;
                    };
                    ui.allocate_ui_with_layout(
                        Vec2::new(cell_w, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui_kit::soft_block(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    let (a, _) =
                                        ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                                    let p = ui.painter();
                                    p.circle_filled(a.center(), 15.0, theme::card());
                                    p.circle_stroke(
                                        a.center(),
                                        15.0,
                                        Stroke::new(1.0_f32, theme::border2()),
                                    );
                                    p.text(
                                        a.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "人",
                                        egui::FontId::proportional(12.0),
                                        theme::subtext(),
                                    );
                                    ui.label(
                                        RichText::new(name)
                                            .size(11.0)
                                            .strong()
                                            .color(theme::text()),
                                    );
                                    ui.label(
                                        RichText::new("名单占位").size(9.0).color(theme::faint()),
                                    );
                                    super::badge_centered(
                                        ui,
                                        "未注册特征",
                                        theme::extreme(),
                                        theme::faint(),
                                    );
                                });
                            });
                        },
                    );
                }
            });
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("这些姓名只是配置占位；特征采集、加密存储与本地识别将在 M3 接入。")
                .size(9.0)
                .color(theme::faint()),
        );
    });
}

// ---- 最近锁定 ---------------------------------------------------------------

fn recent_card(ctx: &mut SenseCtx, ui: &mut Ui) {
    ui_kit::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "最近锁定");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("今日 {} 次", ctx.view.locked_today))
                        .size(9.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(6.0);
        let rows: Vec<&(String, String)> = ctx
            .view
            .events
            .iter()
            .filter(|(_, t)| t.contains("锁定") || t.contains("锁屏") || t.contains("陌生人"))
            .take(4)
            .collect();
        if rows.is_empty() {
            ui.label(
                RichText::new("今日尚无锁定记录。")
                    .size(10.0)
                    .color(theme::faint()),
            );
        }
        for (time, text) in rows {
            let (fill, color) = if text.contains("陌生人") {
                (theme::danger_soft(), theme::danger())
            } else if text.contains("手动") {
                (theme::extreme(), theme::subtext())
            } else {
                (theme::amber_soft(), theme::amber())
            };
            ui.horizontal(|ui| {
                ui_kit::mono(ui, time, 9.0, theme::faint());
                ui.label(RichText::new(text).size(10.5).color(theme::text()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui_kit::badge(
                        ui,
                        if text.contains("陌生人") {
                            "已拦截"
                        } else {
                            "Win+L"
                        },
                        fill,
                        color,
                    );
                });
            });
        }
        ui.add_space(6.0);
        ui_kit::kv_row(ui, "锁定瞬间保存截图", "未接入（—）", theme::faint());
    });
}
