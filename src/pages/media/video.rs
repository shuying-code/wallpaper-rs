//! 视频页：视频库 + 投放到 mpv 独立窗口。

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
        let lw = (total - gap) * 0.72;
        let rw = (total - gap - lw).max(230.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                egui::vec2(lw, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(lw);
                    list(ctx, ui);
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                egui::vec2(rw, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(rw);
                    video_card(ctx, ui);
                },
            );
        });
    } else {
        list(ctx, ui);
        ui.add_space(12.0);
        video_card(ctx, ui);
    }
}

fn video_card(ctx: &mut MediaCtx, ui: &mut egui::Ui) {
    let st = ctx.state;
    ui_kit::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui_kit::card_title(ui, "播放窗口");
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 130.0),
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
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "▶",
            egui::FontId::proportional(38.0),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180),
        );
        if let Some(p) = st.path.as_deref() {
            ui.painter().text(
                egui::pos2(rect.left() + 10.0, rect.bottom() - 12.0),
                egui::Align2::LEFT_BOTTOM,
                crate::media::short_name(p),
                egui::FontId::proportional(9.5),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 170),
            );
        }
        ui.add_space(8.0);
        if ui_kit::primary_button(ui, "在独立窗口播放", false).clicked() {
            if let Some(id) = ctx.selected {
                ctx.actions.push(super::MediaAction::Play(ctx.kind, id));
            } else if let Some(t) = ctx.lib.tracks(ctx.kind).first() {
                ctx.actions.push(super::MediaAction::Play(ctx.kind, t.id));
            }
        }
        ui.add_space(4.0);
        if ui_kit::small_button(ui, "全屏播放").clicked() {
            ctx.actions.push(super::MediaAction::Fullscreen);
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "视频由 mpv 弹出独立窗口播放，可拖动 / 全屏；关闭播放器窗口即可退出。\
                 动态壁纸与视频播放互不影响。",
            )
            .size(10.0)
            .color(theme::subtext()),
        );
    });
}
