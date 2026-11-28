//! 音乐页：目录、播放控制、曲目列表。

use super::{header, list, player, MediaCtx};
use crate::theme;
use crate::ui_kit;
use eframe::egui;

pub fn ui(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    header(ctx, ui);
    ui.add_space(12.0);
    player(ctx, ui);
    ui.add_space(12.0);

    if !ctx.compact {
        let total = ui.available_width();
        let gap = 12.0;
        let lw = (total * 0.32).max(250.0);
        let rw = (total - gap - lw).max(260.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                egui::vec2(lw, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(lw);
                    now_playing(ctx, ui);
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                egui::vec2(rw, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(rw);
                    list(ctx, ui);
                },
            );
        });
    } else {
        now_playing(ctx, ui);
        ui.add_space(12.0);
        list(ctx, ui);
    }
}

fn now_playing(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    let st = ctx.state;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 150.0),
            egui::Sense::hover(),
        );
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), theme::video1());
        mesh.colored_vertex(rect.right_top(), theme::video1());
        mesh.colored_vertex(rect.left_bottom(), theme::video2());
        mesh.colored_vertex(rect.right_bottom(), theme::video2());
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        ui.painter().add(egui::Shape::mesh(mesh));
        ui.painter().circle_stroke(
            rect.center(),
            40.0,
            egui::Stroke::new(
                1.2_f32,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
            ),
        );
        ui.painter().circle_stroke(
            rect.center(),
            28.0,
            egui::Stroke::new(
                1.2_f32,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
            ),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "♪",
            egui::FontId::proportional(42.0),
            theme::accent(),
        );
        let label = if st.title.is_empty() {
            "等待选择曲目"
        } else {
            &st.title
        };
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 16.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200),
        );
    });
    ui.add_space(12.0);
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "播放提示");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "音乐在后台通过 mpv 播放，不会更改桌面壁纸；切到其他页面仍会继续。\
                 单击曲目开始播放，右键在资源管理器中定位。",
            )
            .size(10.5)
            .color(theme::subtext()),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui_kit::badge(
                ui,
                if crate::media::available() {
                    "mpv 已就绪"
                } else {
                    "mpv 待准备"
                },
                if crate::media::available() {
                    theme::mint_soft()
                } else {
                    theme::amber_soft()
                },
                if crate::media::available() {
                    theme::mint()
                } else {
                    theme::amber()
                },
            );
            if let Some(p) = st.path.as_deref() {
                ui.label(
                    egui::RichText::new(crate::media::short_name(p))
                        .size(10.0)
                        .color(theme::faint()),
                );
            }
        });
    });
}
