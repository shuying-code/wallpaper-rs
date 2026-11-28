//! 视频分区 › 电视直播：频道筛选与直播会话控制。
//!
//! 页面只画 UI 和产出 [`LiveAction`]；WebView2 / mpv / 子进程都在
//! `crate::live::LiveController` 中。频道目录来自官方公开页面适配，仅展示
//! 无需会员的频道，不提供内容源、不绕过付费或 DRM。

use crate::live::{Channel, ChannelKind, LivePhase, LiveSnapshot};
use crate::theme;
use crate::ui_kit;
use eframe::egui;
use egui::{Color32, RichText, Rounding, Stroke, Vec2};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiveFilter {
    #[default]
    All,
    Cctv,
    Satellite,
}

impl LiveFilter {
    const LABELS: [&'static str; 3] = ["全部", "央视", "卫视"];

    fn index(self) -> usize {
        match self {
            Self::All => 0,
            Self::Cctv => 1,
            Self::Satellite => 2,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Cctv,
            2 => Self::Satellite,
            _ => Self::All,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LiveUiState {
    pub filter: LiveFilter,
    pub query: String,
    pub selected_pid: Option<String>,
}

impl Default for LiveUiState {
    fn default() -> Self {
        Self {
            filter: LiveFilter::All,
            query: String::new(),
            selected_pid: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveAction {
    Play(String),
    Stop,
    Fullscreen,
    Windowed,
    Volume(u32),
    ToggleMute,
}

pub fn ui(
    ui: &mut egui::Ui,
    snapshot: &LiveSnapshot,
    channels: &[Channel],
    state: &mut LiveUiState,
    actions: &mut Vec<LiveAction>,
    compact: bool,
) {
    ui_kit::page_title(
        ui,
        "电视直播",
        "央视 / 卫视 · 官方页面鉴权 · mpv 或官方播放器",
    );
    ui.add_space(12.0);

    if !snapshot.supported {
        ui_kit::notice_card(
            ui,
            theme::amber(),
            "电视直播仅支持 Windows",
            "鉴权宿主依赖系统 WebView2 Runtime；其它平台仍可使用本地视频播放器。",
            None,
        );
        ui.add_space(10.0);
    }
    if let Some(error) = snapshot.catalog_error.as_deref() {
        ui_kit::notice_card(ui, theme::danger(), "内置频道目录不可用", error, None);
        return;
    }

    status_card(ui, snapshot, channels, state, actions, compact);
    ui.add_space(12.0);
    channel_card(ui, snapshot, channels, state, actions, compact);
    ui.add_space(12.0);
    compliance_card(ui);
}

fn status_card(
    ui: &mut egui::Ui,
    snapshot: &LiveSnapshot,
    channels: &[Channel],
    state: &LiveUiState,
    actions: &mut Vec<LiveAction>,
    compact: bool,
) {
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let selected = state
            .selected_pid
            .as_deref()
            .and_then(|pid| channels.iter().find(|channel| channel.pid == pid));
        let selected_name = selected
            .map(|channel| channel.name.as_str())
            .unwrap_or("未选择频道");
        ui.horizontal_wrapped(|ui| {
            let (fill, color) = phase_color(snapshot.phase);
            ui_kit::badge(ui, snapshot.phase.label(), fill, color);
            if let Some(definition) = snapshot.definition.as_deref() {
                ui_kit::badge(ui, definition, theme::extreme(), theme::subtext());
            }
            ui.add_space(2.0);
            let title = snapshot
                .channel
                .as_ref()
                .map(|channel| channel.name.as_str())
                .unwrap_or(selected_name);
            ui.label(
                RichText::new(title)
                    .size(15.0)
                    .strong()
                    .color(theme::text()),
            );
            if let Some(elapsed) = snapshot.elapsed {
                ui.label(
                    RichText::new(format_elapsed(elapsed))
                        .size(10.0)
                        .color(theme::faint()),
                );
            }
        });

        if let Some(note) = snapshot.route_note.as_deref() {
            ui.add_space(5.0);
            ui.label(RichText::new(note).size(10.5).color(theme::subtext()));
        }
        if !snapshot.host_ready && snapshot.phase == LivePhase::Authenticating {
            ui.add_space(4.0);
            ui.label(
                RichText::new("正在启动隐藏 WebView2 宿主；首次可能需要系统组件初始化。")
                    .size(10.0)
                    .color(theme::faint()),
            );
        }
        if let Some(error) = snapshot.error.as_deref() {
            ui.add_space(6.0);
            ui.label(RichText::new(error).size(10.5).color(theme::danger()));
        }

        ui.add_space(10.0);
        if compact {
            ui.horizontal_wrapped(|ui| {
                if ui_kit::primary_button(ui, &format!("▶ 播放「{selected_name}」"), false)
                    .clicked()
                {
                    if let Some(channel) = selected {
                        actions.push(LiveAction::Play(channel.pid.clone()));
                    }
                }
                if snapshot.active() && ui_kit::small_button(ui, "停止").clicked() {
                    actions.push(LiveAction::Stop);
                }
                if snapshot.can_change_window() {
                    if ui_kit::small_button(ui, "全屏").clicked() {
                        actions.push(LiveAction::Fullscreen);
                    }
                    if ui_kit::small_button(ui, "窗口").clicked() {
                        actions.push(LiveAction::Windowed);
                    }
                }
            });
        } else {
            ui.horizontal(|ui| {
                if ui_kit::primary_button(ui, &format!("▶ 播放「{selected_name}」"), false)
                    .clicked()
                {
                    if let Some(channel) = selected {
                        actions.push(LiveAction::Play(channel.pid.clone()));
                    }
                }
                if snapshot.active() && ui_kit::small_button(ui, "停止").clicked() {
                    actions.push(LiveAction::Stop);
                }
                if snapshot.can_change_window() {
                    if ui_kit::small_button(ui, "全屏").clicked() {
                        actions.push(LiveAction::Fullscreen);
                    }
                    if ui_kit::small_button(ui, "窗口").clicked() {
                        actions.push(LiveAction::Windowed);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", snapshot.volume))
                            .size(10.0)
                            .color(theme::faint()),
                    );
                    let mut volume = snapshot.volume as f32;
                    if ui
                        .add_sized(
                            egui::vec2(150.0, 18.0),
                            egui::Slider::new(&mut volume, 0.0..=100.0)
                                .show_value(false)
                                .trailing_fill(true),
                        )
                        .changed()
                    {
                        actions.push(LiveAction::Volume(volume.round() as u32));
                    }
                    if ui_kit::small_button(
                        ui,
                        if snapshot.muted {
                            "取消静音"
                        } else {
                            "静音"
                        },
                    )
                    .clicked()
                    {
                        actions.push(LiveAction::ToggleMute);
                    }
                });
            });
        }

        if compact {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                if ui_kit::small_button(
                    ui,
                    if snapshot.muted {
                        "取消静音"
                    } else {
                        "静音"
                    },
                )
                .clicked()
                {
                    actions.push(LiveAction::ToggleMute);
                }
                let mut volume = snapshot.volume as f32;
                let width = (ui.available_width() - 48.0).max(80.0);
                if ui
                    .add_sized(
                        egui::vec2(width, 18.0),
                        egui::Slider::new(&mut volume, 0.0..=100.0)
                            .show_value(false)
                            .trailing_fill(true),
                    )
                    .changed()
                {
                    actions.push(LiveAction::Volume(volume.round() as u32));
                }
                ui.label(
                    RichText::new(format!("{:.0}%", snapshot.volume))
                        .size(10.0)
                        .color(theme::faint()),
                );
            });
        }

        ui.add_space(7.0);
        ui.label(
            RichText::new(
                "清晰流在独立 mpv 窗口播放；加密频道自动使用官方播放器。\
                 关闭播放器窗口即结束本次直播。",
            )
            .size(10.0)
            .color(theme::faint()),
        );
    });
}

fn channel_card(
    ui: &mut egui::Ui,
    snapshot: &LiveSnapshot,
    channels: &[Channel],
    state: &mut LiveUiState,
    actions: &mut Vec<LiveAction>,
    compact: bool,
) {
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui_kit::card_title(ui, "频道目录");
            ui_kit::badge(ui, "仅免费频道", theme::mint_soft(), theme::mint());
            if let Some(index) = ui_kit::segmented(ui, &LiveFilter::LABELS, state.filter.index()) {
                state.filter = LiveFilter::from_index(index);
            }
        });
        ui.add_space(7.0);
        ui.horizontal_wrapped(|ui| {
            search_box(ui, &mut state.query, if compact { 150.0 } else { 220.0 });
            ui.label(
                RichText::new("可按频道名或“CCTV / 卫视”搜索")
                    .size(10.0)
                    .color(theme::faint()),
            );
        });
        ui.add_space(8.0);

        let shown = visible_channels(channels, state.filter, &state.query);
        if state.selected_pid.is_none() {
            state.selected_pid = shown.first().map(|channel| channel.pid.clone());
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "显示 {} 路 · 免费频道 {} 路",
                    shown.len(),
                    channels
                        .iter()
                        .filter(|channel| !channel.requires_membership())
                        .count()
                ))
                .size(10.0)
                .color(theme::faint()),
            );
        });
        ui.add_space(7.0);

        if shown.is_empty() {
            ui.label(
                RichText::new("没有匹配的免费频道，请更换筛选或搜索词。")
                    .size(10.5)
                    .color(theme::subtext()),
            );
            return;
        }

        let cols: usize = if compact { 1 } else { 2 };
        let gap = 8.0;
        let tile_width = ((ui.available_width() - gap * (cols.saturating_sub(1) as f32))
            / cols as f32)
            .max(150.0);
        egui::Grid::new("wallpaper_live_channel_grid")
            .num_columns(cols)
            .spacing(egui::vec2(gap, gap))
            .show(ui, |ui| {
                for (index, channel) in shown.iter().enumerate() {
                    let playing = snapshot.channel.as_ref().is_some_and(|current| {
                        current.pid == channel.pid
                            && matches!(
                                snapshot.phase,
                                LivePhase::Authenticating
                                    | LivePhase::StartingPlayer
                                    | LivePhase::Playing
                                    | LivePhase::Official
                            )
                    });
                    let selected = state.selected_pid.as_deref() == Some(channel.pid.as_str());
                    let response = channel_tile(ui, channel, selected, playing, tile_width);
                    if response.clicked() {
                        state.selected_pid = Some(channel.pid.clone());
                    }
                    if response.double_clicked() {
                        state.selected_pid = Some(channel.pid.clone());
                        actions.push(LiveAction::Play(channel.pid.clone()));
                    }
                    if (index + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

fn compliance_card(ui: &mut egui::Ui) {
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "来源与使用说明");
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "频道目录、官方页面鉴权脚本与播放规则适配自 MIT 许可的 ysp-live 项目。\
                 画面来自央视频官方网站；本应用不提供内容源、不代理视频、不绕过会员或 DRM。\
                 频道可用性、清晰度与登录要求由官方决定，需保持联网。\n\
                 Windows 需要 Microsoft Edge WebView2 Runtime（Windows 11 通常已内置）。",
            )
            .size(10.2)
            .color(theme::subtext()),
        );
    });
}

fn channel_tile(
    ui: &mut egui::Ui,
    channel: &Channel,
    selected: bool,
    playing: bool,
    width: f32,
) -> egui::Response {
    let height = 46.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    let fill = if selected {
        theme::accent_soft()
    } else if response.hovered() {
        theme::extreme()
    } else {
        ui_kit::with_alpha(theme::extreme(), 135)
    };
    ui.painter().rect_filled(rect, Rounding::same(9.0), fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            Rounding::same(9.0),
            Stroke::new(1.2_f32, theme::accent()),
        );
    }

    let name_color = if selected {
        theme::accent()
    } else {
        theme::text()
    };
    ui.painter().text(
        egui::pos2(rect.left() + 11.0, rect.center().y - 7.0),
        egui::Align2::LEFT_CENTER,
        &channel.name,
        egui::FontId::proportional(12.0),
        name_color,
    );
    let subtitle = format!(
        "{} · {}{}",
        channel.kind.label(),
        if channel.is_4k { "4K · " } else { "" },
        "免费"
    );
    ui.painter().text(
        egui::pos2(rect.left() + 11.0, rect.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        subtitle,
        egui::FontId::proportional(9.0),
        theme::faint(),
    );

    if playing {
        let label = "LIVE";
        let font = egui::FontId::proportional(8.5);
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_string(), font, Color32::WHITE);
        let size = Vec2::new(galley.size().x + 12.0, 17.0);
        let live_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - size.x - 9.0, rect.center().y - 8.5),
            size,
        );
        ui.painter()
            .rect_filled(live_rect, Rounding::same(8.0), theme::danger());
        ui.painter().galley(
            egui::pos2(
                live_rect.center().x - galley.size().x * 0.5,
                live_rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            Color32::WHITE,
        );
    } else if channel.kind == ChannelKind::Cctv {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "CCTV",
            egui::FontId::proportional(8.5),
            ui_kit::with_alpha(theme::accent(), 150),
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn visible_channels<'a>(
    channels: &'a [Channel],
    filter: LiveFilter,
    query: &str,
) -> Vec<&'a Channel> {
    let query = query.trim().to_lowercase();
    channels
        .iter()
        .filter(|channel| !channel.requires_membership())
        .filter(|channel| match filter {
            LiveFilter::All => true,
            LiveFilter::Cctv => channel.kind == ChannelKind::Cctv,
            LiveFilter::Satellite => channel.kind == ChannelKind::Satellite,
        })
        .filter(|channel| {
            query.is_empty()
                || channel.name.to_lowercase().contains(&query)
                || channel.kind.label().contains(&query)
        })
        .collect()
}

fn search_box(ui: &mut egui::Ui, text: &mut String, width: f32) {
    egui::Frame::none()
        .fill(theme::extreme())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .rounding(Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(9.0, 4.0))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .hint_text("搜索频道…")
                    .desired_width(width)
                    .frame(false),
            );
        });
}

fn phase_color(phase: LivePhase) -> (Color32, Color32) {
    match phase {
        LivePhase::Playing => (theme::mint_soft(), theme::mint()),
        LivePhase::Official => (theme::amber_soft(), theme::amber()),
        LivePhase::Failed => (theme::danger_soft(), theme::danger()),
        LivePhase::Authenticating | LivePhase::StartingPlayer => {
            (theme::accent_soft(), theme::accent())
        }
        LivePhase::Idle | LivePhase::Stopped => (theme::extreme(), theme::subtext()),
    }
}

fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::ChannelCatalog;

    #[test]
    fn only_free_channels_are_listed() {
        let catalog = ChannelCatalog::bundled().unwrap();
        let all = visible_channels(&catalog.channels, LiveFilter::All, "");
        assert_eq!(all.len(), 51);
        assert!(all.iter().all(|channel| !channel.requires_membership()));
    }

    #[test]
    fn kind_and_name_filters_work() {
        let catalog = ChannelCatalog::bundled().unwrap();
        let cctv = visible_channels(&catalog.channels, LiveFilter::Cctv, "cctv");
        assert!(!cctv.is_empty());
        assert!(cctv.iter().all(|channel| channel.kind == ChannelKind::Cctv));

        let satellite = visible_channels(&catalog.channels, LiveFilter::Satellite, "天津");
        assert_eq!(satellite.len(), 1);
        assert_eq!(satellite[0].name, "天津卫视");
    }

    #[test]
    fn missing_query_returns_empty_list() {
        let catalog = ChannelCatalog::bundled().unwrap();
        assert!(visible_channels(&catalog.channels, LiveFilter::All, "不存在的频道").is_empty());
    }
}
