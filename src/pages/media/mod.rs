//! 音乐 / 视频页面的共享上下文与播放控制条。
//!
//! 页面只负责画 UI 与产出 `MediaAction`；播放器实例、线程和磁盘 IO 都在
//! `App` 里，这样切换一级菜单不会打断正在播放的媒体。

pub mod music;
pub mod video;

use crate::config::MediaCfg;
use crate::media::{self, Kind, Library, State};
use crate::theme;
use crate::ui_kit;
use eframe::egui;
use egui::{Color32, RichText, Rounding, Stroke, Vec2};

/// 页面请求 `App` 执行的动作。
#[derive(Clone, Debug, PartialEq)]
pub enum MediaAction {
    PickFolder(Kind),
    RemoveFolder(Kind, usize),
    Rescan(Kind),
    Play(Kind, u64),
    Toggle,
    Next,
    Prev,
    Seek(f64),
    Volume(Kind, f64),
    Mute(Kind),
    Stop,
    Save,
    OpenMedia(Kind, u64),
    Reveal(Kind, u64),
    /// 视频：在独立窗口里全屏播放。
    Fullscreen,
}

/// 页面共享上下文。
pub struct MediaCtx<'a> {
    pub kind: Kind,
    pub cfg: &'a mut MediaCfg,
    pub lib: &'a Library,
    pub state: &'a State,
    pub selected: Option<u64>,
    pub playing_id: Option<u64>,
    pub actions: &'a mut Vec<MediaAction>,
    pub compact: bool,
    pub scanning: bool,
    pub mpv_ready: bool,
    pub error: Option<String>,
}

/// 顶部标题 + 目录工具条。
pub fn header(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    let (title, subtitle) = match ctx.kind {
        Kind::Audio => ("音乐", "本地音乐库 · 后台播放 · 不打断壁纸"),
        Kind::Video => ("视频", "本地视频库 · 独立播放窗口 · 支持全屏"),
    };
    ui_kit::page_title(ui, title, subtitle);
    ui.add_space(12.0);

    let dirs: Vec<std::path::PathBuf> = dirs_for(ctx).to_vec();
    ui_kit::card(ui, |ui| {
        if ctx.compact {
            // 窄屏：标题与动作分两行，按钮左对齐换行，避免「添加文件夹」
            // 被右对齐布局挤出卡片。
            ui.horizontal_wrapped(|ui| {
                ui_kit::card_title(ui, "媒体文件夹");
                if ctx.scanning {
                    ui_kit::badge(ui, "扫描中…", theme::amber_soft(), theme::amber());
                } else {
                    let n = ctx.lib.tracks(ctx.kind).len();
                    ui_kit::badge(
                        ui,
                        &format!("{n} 个文件"),
                        theme::extreme(),
                        theme::subtext(),
                    );
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui_kit::small_button(ui, "＋ 添加文件夹").clicked() {
                    ctx.actions.push(MediaAction::PickFolder(ctx.kind));
                }
                if ui_kit::small_button(ui, "重新扫描").clicked() {
                    ctx.actions.push(MediaAction::Rescan(ctx.kind));
                }
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui_kit::card_title(ui, "媒体文件夹");
                if ctx.scanning {
                    ui_kit::badge(ui, "扫描中…", theme::amber_soft(), theme::amber());
                } else {
                    let n = ctx.lib.tracks(ctx.kind).len();
                    ui_kit::badge(
                        ui,
                        &format!("{n} 个文件"),
                        theme::extreme(),
                        theme::subtext(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui_kit::small_button(ui, "重新扫描").clicked() {
                        ctx.actions.push(MediaAction::Rescan(ctx.kind));
                    }
                    if ui_kit::small_button(ui, "＋ 添加文件夹").clicked() {
                        ctx.actions.push(MediaAction::PickFolder(ctx.kind));
                    }
                });
            });
        }
        ui.add_space(6.0);
        if dirs.is_empty() {
            // 以可用宽度为上限显式换行：`ui.label` 的默认 wrap 在卡片里
            // 会按父级宽度计算，窄屏时最后一行仍可能越过右边界。
            let w = ui.available_width();
            ui.scope(|ui| {
                ui.set_max_width(w);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "还没有选择目录。添加一个包含音乐或视频的文件夹，程序会扫描其子目录。",
                        )
                        .size(10.5)
                        .color(theme::subtext()),
                    )
                    .wrap(),
                );
            });
        } else {
            let mut remove = None;
            for (i, d) in dirs.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(media::dirs_label(std::slice::from_ref(d)))
                            .size(10.5)
                            .color(theme::subtext()),
                    );
                    if ui_kit::small_button(ui, "移除").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                ctx.actions.push(MediaAction::RemoveFolder(ctx.kind, i));
            }
        }
        if let Some(e) = ctx.error.as_deref() {
            ui.add_space(6.0);
            ui.label(RichText::new(e).size(10.5).color(theme::danger()));
        }
        if !ctx.mpv_ready {
            ui.add_space(6.0);
            ui.label(
                RichText::new("未找到 mpv：首次播放时 Windows 会按需下载；其他平台请先安装 mpv。")
                    .size(10.0)
                    .color(theme::amber()),
            );
        }
    });
}

fn dirs_for<'a>(ctx: &'a MediaCtx) -> &'a [std::path::PathBuf] {
    match ctx.kind {
        Kind::Audio => &ctx.cfg.music_dirs,
        Kind::Video => &ctx.cfg.video_dirs,
    }
}

/// 播放控制条。返回时页面继续画列表。
pub fn player(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    let st = ctx.state;
    let has = st.path.is_some();
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let (badge, fill, color) = if !has {
                ("待播放", theme::extreme(), theme::faint())
            } else if st.paused {
                ("已暂停", theme::amber_soft(), theme::amber())
            } else {
                ("正在播放", theme::mint_soft(), theme::mint())
            };
            ui_kit::badge(ui, badge, fill, color);
            ui.add_space(4.0);
            ui.label(
                RichText::new(if has {
                    media::short_name(st.path.as_deref().unwrap_or(std::path::Path::new("")))
                } else {
                    "选择列表中的文件开始播放".to_string()
                })
                .size(13.0)
                .strong()
                .color(theme::text()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui_kit::small_button(ui, "停止").clicked() {
                    ctx.actions.push(MediaAction::Stop);
                }
                if ctx.kind == Kind::Video && has && ui_kit::small_button(ui, "全屏").clicked() {
                    ctx.actions.push(MediaAction::Fullscreen);
                }
            });
        });
        ui.add_space(10.0);
        let progress = st.progress();
        // 进度条两端各留 1px，避免抗锯齿与圆角在窄屏上贴边溢出。
        let bar_w = (ui.available_width() - 2.0).max(40.0);
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(bar_w, 18.0), egui::Sense::click_and_drag());
        if (resp.dragged() || resp.clicked()) && st.duration > 0.0 {
            if let Some(pos) = resp.interact_pointer_pos() {
                let f = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                ctx.actions.push(MediaAction::Seek(f as f64 * st.duration));
            }
        }
        ui.painter()
            .rect_filled(rect, Rounding::same(3.0), theme::extreme());
        let filled =
            egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * progress, rect.height()));
        ui.painter()
            .rect_filled(filled, Rounding::same(3.0), theme::accent());
        ui.painter().circle_filled(
            egui::pos2(rect.left() + rect.width() * progress, rect.center().y),
            6.0,
            theme::card(),
        );
        ui.painter().circle_stroke(
            egui::pos2(rect.left() + rect.width() * progress, rect.center().y),
            6.0,
            Stroke::new(2.0_f32, theme::accent()),
        );
        let time_text = format!(
            "{} / {}",
            media::format_time(st.time),
            media::format_time(st.duration)
        );
        if ctx.compact {
            // 窄屏：时间单独一行，音量另起一行并让滑杆吃掉剩余宽度，
            // 避免滑杆与「静音」按钮被推到卡片外。
            ui.label(RichText::new(&time_text).size(10.0).color(theme::subtext()));
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui_kit::small_button(ui, if st.muted { "静音中" } else { "静音" }).clicked()
                {
                    ctx.actions.push(MediaAction::Mute(ctx.kind));
                }
                let mut vol = st.volume as f32;
                let slider_w = (ui.available_width() - 44.0).max(60.0);
                if ui
                    .add_sized(
                        egui::vec2(slider_w, 18.0),
                        egui::Slider::new(&mut vol, 0.0..=130.0)
                            .show_value(false)
                            .trailing_fill(true),
                    )
                    .changed()
                {
                    ctx.actions.push(MediaAction::Volume(ctx.kind, vol as f64));
                }
                ui.label(
                    RichText::new(format!("{:.0}%", st.volume))
                        .size(10.0)
                        .color(theme::faint()),
                );
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&time_text).size(10.0).color(theme::subtext()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", st.volume))
                            .size(10.0)
                            .color(theme::faint()),
                    );
                    let mut vol = st.volume as f32;
                    if ui
                        .add(
                            egui::Slider::new(&mut vol, 0.0..=130.0)
                                .show_value(false)
                                .trailing_fill(true),
                        )
                        .changed()
                    {
                        ctx.actions.push(MediaAction::Volume(ctx.kind, vol as f64));
                    }
                    if ui_kit::small_button(ui, if st.muted { "静音中" } else { "静音" }).clicked()
                    {
                        ctx.actions.push(MediaAction::Mute(ctx.kind));
                    }
                });
            });
        }
        ui.add_space(6.0);
        let play_label = if st.paused {
            "▶ 播放"
        } else {
            "⏸ 暂停"
        };
        if ctx.compact {
            ui.horizontal_wrapped(|ui| {
                if ui_kit::small_button(ui, "⏮ 上一首").clicked() {
                    ctx.actions.push(MediaAction::Prev);
                }
                if ui_kit::primary_button(ui, play_label, false).clicked() {
                    ctx.actions.push(MediaAction::Toggle);
                }
                if ui_kit::small_button(ui, "⏭ 下一首").clicked() {
                    ctx.actions.push(MediaAction::Next);
                }
            });
            let mut repeat = ctx.cfg.repeat_for(ctx.kind);
            if ui.checkbox(&mut repeat, "列表循环").changed() {
                ctx.cfg.set_repeat_for(ctx.kind, repeat);
                ctx.actions.push(MediaAction::Save);
            }
        } else {
            ui.horizontal(|ui| {
                if ui_kit::small_button(ui, "⏮ 上一首").clicked() {
                    ctx.actions.push(MediaAction::Prev);
                }
                if ui_kit::primary_button(ui, play_label, false).clicked() {
                    ctx.actions.push(MediaAction::Toggle);
                }
                if ui_kit::small_button(ui, "⏭ 下一首").clicked() {
                    ctx.actions.push(MediaAction::Next);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut repeat = ctx.cfg.repeat_for(ctx.kind);
                    if ui.checkbox(&mut repeat, "列表循环").changed() {
                        ctx.cfg.set_repeat_for(ctx.kind, repeat);
                        ctx.actions.push(MediaAction::Save);
                    }
                });
            });
        }
    });
}

/// 媒体列表。返回点击的文件 id。
pub fn list(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    let tracks = ctx.lib.tracks(ctx.kind);
    if tracks.is_empty() {
        let hint = match ctx.kind {
            Kind::Audio => "选择音乐文件夹后，这里会列出 MP3 / FLAC / WAV 等音频。",
            Kind::Video => "选择视频文件夹后，这里会列出 MP4 / MKV / MOV 等视频。",
        };
        ui_kit::notice_card(ui, theme::accent(), "媒体库为空", hint, None);
        return;
    }
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::card_title(ui, "媒体库");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} 项", tracks.len()))
                        .size(10.0)
                        .color(theme::faint()),
                );
            });
        });
        ui.add_space(4.0);
        let row_h = if ctx.compact { 46.0 } else { 40.0 };
        let mut shown = 0;
        for t in tracks {
            if shown >= 600 {
                ui.label(
                    RichText::new("列表较长，仅显示前 600 项；可用搜索进一步筛选。")
                        .size(10.0)
                        .color(theme::faint()),
                );
                break;
            }
            shown += 1;
            let playing = ctx.playing_id == Some(t.id);
            let selected = ctx.selected == Some(t.id);
            let (rect, resp) = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), row_h), egui::Sense::click());
            if selected || playing {
                ui.painter()
                    .rect_filled(rect, Rounding::same(8.0), theme::accent_soft());
            } else if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, Rounding::same(8.0), theme::extreme());
            }
            let icon = match (ctx.kind, playing) {
                (Kind::Audio, true) => "▶",
                (Kind::Audio, false) => "♪",
                (Kind::Video, true) => "▶",
                (Kind::Video, false) => "🎬",
            };
            ui.painter().text(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(13.0),
                if playing {
                    theme::accent()
                } else {
                    theme::subtext()
                },
            );
            ui.painter().text(
                egui::pos2(rect.left() + 34.0, rect.center().y - 6.0),
                egui::Align2::LEFT_CENTER,
                &t.title,
                egui::FontId::proportional(12.0),
                theme::text(),
            );
            ui.painter().text(
                egui::pos2(rect.left() + 34.0, rect.center().y + 8.0),
                egui::Align2::LEFT_CENTER,
                &format!("{} · {}", t.folder, human_size(t.bytes)),
                egui::FontId::proportional(9.5),
                theme::faint(),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                ctx.selected = Some(t.id);
                ctx.actions.push(MediaAction::Play(ctx.kind, t.id));
            }
            if resp.double_clicked() {
                ctx.actions.push(MediaAction::OpenMedia(ctx.kind, t.id));
            }
            if resp.secondary_clicked() {
                ctx.actions.push(MediaAction::Reveal(ctx.kind, t.id));
            }
            ui.add_space(2.0);
        }
    });
}

pub fn human_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{mb:.1} MB")
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

/// 统一的「空 / 错误」色调，避免两个页面各写一份。
pub fn tone_error() -> Color32 {
    theme::danger()
}
