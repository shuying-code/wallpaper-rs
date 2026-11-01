//! The egui application: library grid, slideshow controls, settings, about.

use crate::config::{config_dir, Config, Mode};
use crate::theme;
use crate::thumbs::Thumbnailer;
use crate::wallpaper::{self, Fit};
use eframe::egui;
use egui::{Color32, ColorImage, RichText, Rounding, Stroke, TextureHandle, Vec2};
use rand::Rng;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CELL_W: f32 = 176.0;
const IMG_H: f32 = 100.0;
const GAP: f32 = 10.0;
const CARD_PAD: f32 = 8.0;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Library,
    Slideshow,
    Settings,
    About,
}

enum Action {
    SetPath(PathBuf),
    Next,
    Prev,
    Random,
    Rescan,
    PickFolder,
    OpenFolder,
    OpenConfig,
    Save,
}

pub struct App {
    cfg: Config,
    images: Vec<PathBuf>,
    current: Option<usize>,
    last_switch: Instant,
    status: String,
    tab: Tab,
    thumb: Thumbnailer,
    tex_cache: HashMap<PathBuf, Option<TextureHandle>>,
    preview: Option<(PathBuf, TextureHandle)>,
    applied_theme: usize,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = Config::load();
        theme::apply(&cc.egui_ctx, cfg.theme == 1);
        let mut app = Self {
            applied_theme: cfg.theme,
            cfg,
            images: Vec::new(),
            current: None,
            last_switch: Instant::now(),
            status: String::new(),
            tab: Tab::Library,
            thumb: Thumbnailer::new(),
            tex_cache: HashMap::new(),
            preview: None,
        };
        app.scan();
        app
    }

    fn scan(&mut self) {
        self.images.clear();
        self.thumb.reset();
        self.tex_cache.clear();
        self.preview = None;
        self.current = None;
        if let Some(folder) = self.cfg.folder.clone() {
            if folder.exists() {
                let mut files = Vec::new();
                collect_images(&folder, self.cfg.recursive, &mut files);
                files.sort();
                self.images = files;
            }
        }
        if let Some(lp) = self.cfg.last_path.clone() {
            if let Some(pos) = self.images.iter().position(|p| p == &lp) {
                self.current = Some(pos);
            }
        }
        if self.current.is_none() && !self.images.is_empty() {
            self.current = Some(0);
        }
        self.status = if self.images.is_empty() {
            "尚未载入图片，请选择壁纸文件夹".to_string()
        } else {
            format!("已载入 {} 张图片", self.images.len())
        };
        self.last_switch = Instant::now();
    }

    fn load_preview(&mut self, ctx: &egui::Context, path: &Path) {
        if let Some((p, _)) = &self.preview {
            if p == path {
                return;
            }
        }
        match crate::thumbs::load_image(path, 1400) {
            Some(t) => {
                let img = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba);
                let tex = ctx.load_texture(
                    format!("preview:{}", path.display()),
                    img,
                    egui::TextureOptions::LINEAR,
                );
                self.preview = Some((path.to_path_buf(), tex));
            }
            None => self.preview = None,
        }
    }

    fn set_current(&mut self, ctx: &egui::Context, i: usize) {
        if self.images.is_empty() {
            return;
        }
        let i = i % self.images.len();
        let path = self.images[i].clone();
        self.current = Some(i);
        self.cfg.last_path = Some(path.clone());
        self.load_preview(ctx, &path);
        match wallpaper::set_wallpaper(&path, self.cfg.fit) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.status = format!("已设置壁纸：{}", name);
            }
            Err(e) => self.status = format!("设置失败：{e}"),
        }
        self.last_switch = Instant::now();
    }

    fn advance(&mut self, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }
        let n = self.images.len();
        let next = if self.cfg.shuffle {
            let mut k = rand::thread_rng().gen_range(0..n);
            if n > 1 {
                while Some(k) == self.current {
                    k = rand::thread_rng().gen_range(0..n);
                }
            }
            k
        } else {
            match self.current {
                Some(c) => (c + 1) % n,
                None => 0,
            }
        };
        self.set_current(ctx, next);
    }

    fn step_back(&mut self, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }
        let n = self.images.len();
        let prev = match self.current {
            Some(c) => (c + n - 1) % n,
            None => 0,
        };
        self.set_current(ctx, prev);
    }

    fn random_pick(&mut self, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }
        let n = self.images.len();
        let mut k = rand::thread_rng().gen_range(0..n);
        if n > 1 {
            while Some(k) == self.current {
                k = rand::thread_rng().gen_range(0..n);
            }
        }
        self.set_current(ctx, k);
    }

    fn current_path(&self) -> Option<PathBuf> {
        self.current.and_then(|i| self.images.get(i)).cloned()
    }

    fn apply(&mut self, ctx: &egui::Context, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::SetPath(p) => {
                    if let Some(i) = self.images.iter().position(|x| x == &p) {
                        self.set_current(ctx, i);
                    }
                }
                Action::Next => self.advance(ctx),
                Action::Prev => self.step_back(ctx),
                Action::Random => self.random_pick(ctx),
                Action::Rescan => self.scan(),
                Action::PickFolder => {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.cfg.folder = Some(dir);
                        self.scan();
                    }
                }
                Action::OpenFolder => {
                    if let Some(f) = &self.cfg.folder {
                        open_path(f);
                    }
                }
                Action::OpenConfig => {
                    let _ = std::fs::create_dir_all(config_dir());
                    open_path(&config_dir());
                }
                Action::Save => {
                    self.cfg.save();
                    self.status = "配置已保存".to_string();
                }
            }
        }
    }

    // ---- UI sections ------------------------------------------------------
    fn draw_header(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("hdr")
            .frame(
                egui::Frame::none()
                    .fill(theme::card())
                    .inner_margin(egui::Margin::symmetric(18.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🖼 Wallpaper Studio")
                            .size(17.0)
                            .strong()
                            .color(theme::accent()),
                    );
                    ui.label(RichText::new("桌面壁纸").size(12.5).color(theme::subtext()));
                    ui.add_space(10.0);
                    let mode = if self.cfg.mode == Mode::Slideshow { "轮播" } else { "固定" };
                    chip(ui, mode, theme::accent_soft(), theme::accent());
                    if self.cfg.mode == Mode::Slideshow {
                        if self.cfg.running {
                            chip(ui, "● 运行中", theme::accent_soft(), theme::ok());
                        } else {
                            chip(ui, "❚❚ 已暂停", theme::extreme(), theme::subtext());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(p) = self.current_path() {
                            let name = p
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            ui.label(RichText::new(ellipsis(&name, 40)).size(12.0).color(theme::subtext()));
                        }
                    });
                });
            });
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(208.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::card())
                    .inner_margin(egui::Margin::symmetric(10.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("导航").size(11.0).color(theme::subtext()));
                ui.add_space(4.0);
                if nav_button(ui, "🖼", "壁纸库", self.tab == Tab::Library) {
                    self.tab = Tab::Library;
                }
                if nav_button(ui, "🎞", "轮播", self.tab == Tab::Slideshow) {
                    self.tab = Tab::Slideshow;
                }
                if nav_button(ui, "⚙", "设置", self.tab == Tab::Settings) {
                    self.tab = Tab::Settings;
                }
                if nav_button(ui, "ℹ", "关于", self.tab == Tab::About) {
                    self.tab = Tab::About;
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("v{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS))
                            .size(10.5)
                            .color(theme::subtext()),
                    );
                    ui.separator();
                    let dark = self.cfg.theme == 1;
                    let label = if dark { "☀ 浅色主题" } else { "🌙 深色主题" };
                    if ui.add(egui::Button::new(RichText::new(label).size(12.0)).fill(theme::extreme()).rounding(Rounding::same(8.0))).clicked() {
                        self.cfg.theme = if dark { 0 } else { 1 };
                    }
                });
            });
    }

    fn draw_center(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::bg())
                    .inner_margin(egui::Margin::symmetric(18.0, 14.0)),
            )
            .show(ctx, |ui| match self.tab {
                Tab::Library => self.ui_library(ui, actions),
                Tab::Slideshow => self.ui_slideshow(ui, actions),
                Tab::Settings => self.ui_settings(ui, actions),
                Tab::About => self.ui_about(ui),
            });
    }

    fn ui_library(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let folder = self
                .cfg
                .folder
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "（未选择）".to_string());
            ui.label(RichText::new("壁纸文件夹：").size(13.0).color(theme::subtext()));
            ui.label(RichText::new(ellipsis(&folder, 48)).size(13.0).color(theme::text()));
            if ui.button("选择文件夹…").clicked() {
                actions.push(Action::PickFolder);
            }
            if ui.button("重新扫描").clicked() {
                actions.push(Action::Rescan);
            }
            if ui.button("打开目录").clicked() {
                actions.push(Action::OpenFolder);
            }
        });
        ui.horizontal(|ui| {
            let mut rc = self.cfg.recursive;
            if ui.checkbox(&mut rc, "包含子文件夹").changed() {
                self.cfg.recursive = rc;
                actions.push(Action::Rescan);
            }
            ui.add_space(12.0);
            ui.label(RichText::new("搜索：").size(13.0).color(theme::subtext()));
            ui.add(
                egui::TextEdit::singleline(&mut self.cfg.search)
                    .hint_text("按文件名过滤")
                    .desired_width(200.0),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("共 {} 张", self.images.len()))
                    .size(12.0)
                    .color(theme::subtext()),
            );
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        if self.images.is_empty() {
            self.empty_state(ui, actions);
            return;
        }

        let q = self.cfg.search.trim().to_lowercase();
        let paths: Vec<PathBuf> = self
            .images
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.file_name()
                        .map(|n| n.to_string_lossy().to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();

        for p in &paths {
            self.thumb.request(p);
        }

        let current_path = self.current_path();
        let avail = ui.available_width();
        let item_w = CELL_W + CARD_PAD * 2.0;
        let cols = (((avail + GAP) / (item_w + GAP)).floor() as usize).max(1);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut idx = 0usize;
                while idx < paths.len() {
                    ui.horizontal(|ui| {
                        for _ in 0..cols {
                            if idx >= paths.len() {
                                break;
                            }
                            self.draw_card(ui, &paths[idx], current_path.as_deref(), actions);
                            idx += 1;
                        }
                    });
                    ui.add_space(GAP);
                }
            });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(RichText::new("🖼").size(48.0).color(theme::accent()));
            ui.add_space(8.0);
            ui.label(
                RichText::new("还没有壁纸")
                    .size(18.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("选择一个包含图片的文件夹，即可开始使用固定壁纸或轮播。")
                    .size(13.0)
                    .color(theme::subtext()),
            );
            ui.add_space(14.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("选择壁纸文件夹…").color(Color32::WHITE).strong())
                        .fill(theme::accent())
                        .rounding(Rounding::same(8.0))
                        .min_size(Vec2::new(180.0, 34.0)),
                )
                .clicked()
            {
                actions.push(Action::PickFolder);
            }
        });
    }

    fn draw_card(
        &self,
        ui: &mut egui::Ui,
        path: &Path,
        current: Option<&Path>,
        actions: &mut Vec<Action>,
    ) {
        let is_current = current == Some(path);
        let stroke = if is_current {
            Stroke::new(2.0_f32, theme::accent())
        } else {
            Stroke::new(1.0_f32, theme::border())
        };
        egui::Frame::none()
            .fill(theme::card())
            .stroke(stroke)
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::same(CARD_PAD))
            .show(ui, |ui| {
                // Frame inherits the parent (horizontal) layout; force vertical
                // so the card stacks image / name / button instead of stretching wide.
                ui.vertical(|ui| {
                ui.set_min_width(CELL_W);
                ui.set_max_width(CELL_W);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(CELL_W, IMG_H), egui::Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, Rounding::same(6.0), theme::extreme());
                match self.tex_cache.get(path) {
                    Some(Some(tex)) => {
                        let ts = tex.size();
                        let tw = ts[0] as f32;
                        let th = ts[1] as f32;
                        if tw > 0.0 && th > 0.0 {
                            let s = (rect.width() / tw).min(rect.height() / th);
                            let size = Vec2::new(tw * s, th * s);
                            let ir = egui::Rect::from_center_size(rect.center(), size);
                            painter.image(
                                tex.id(),
                                ir,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                    Some(None) => {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "无法预览",
                            egui::FontId::proportional(12.0),
                            theme::subtext(),
                        );
                    }
                    None => {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "载入中…",
                            egui::FontId::proportional(12.0),
                            theme::subtext(),
                        );
                    }
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    actions.push(Action::SetPath(path.to_path_buf()));
                }
                ui.add_space(6.0);
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                ui.add(egui::Label::new(
                    RichText::new(ellipsis(&name, 20)).size(12.0).color(theme::text()),
                ));
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("设为壁纸").size(12.0).color(Color32::WHITE),
                            )
                            .fill(theme::accent())
                            .rounding(Rounding::same(6.0)),
                        )
                        .clicked()
                    {
                        actions.push(Action::SetPath(path.to_path_buf()));
                    }
                    if is_current {
                        ui.label(RichText::new("● 当前").size(11.0).color(theme::ok()));
                    }
                });
                });
            });
    }

    fn ui_slideshow(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // preview
        let cur = self.current_path();
        let w = ui.available_width();
        let h = (ui.available_height() * 0.5).max(170.0);
        let (rect, _resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
        ui.painter().rect_filled(rect, Rounding::same(12.0), theme::extreme());
        let mut painted = false;
        if let (Some((p, tex)), Some(c)) = (&self.preview, &cur) {
            if p == c {
                let ts = tex.size();
                let tw = ts[0] as f32;
                let th = ts[1] as f32;
                if tw > 0.0 && th > 0.0 {
                    let s = (rect.width() / tw).min(rect.height() / th);
                    let size = Vec2::new(tw * s, th * s);
                    let ir = egui::Rect::from_center_size(rect.center(), size);
                    ui.painter().image(
                        tex.id(),
                        ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    painted = true;
                }
            }
        }
        if !painted {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "无预览",
                egui::FontId::proportional(14.0),
                theme::subtext(),
            );
        }
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("模式：").size(13.0).color(theme::subtext()));
            ui.selectable_value(&mut self.cfg.mode, Mode::Fixed, "固定壁纸");
            ui.selectable_value(&mut self.cfg.mode, Mode::Slideshow, "轮播");
        });
        ui.add_space(6.0);

        if self.cfg.mode == Mode::Slideshow {
            ui.horizontal(|ui| {
                let start = if self.cfg.running { "⏸ 暂停" } else { "▶ 开始" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(start).color(Color32::WHITE).strong())
                            .fill(theme::accent())
                            .rounding(Rounding::same(8.0))
                            .min_size(Vec2::new(96.0, 32.0)),
                    )
                    .clicked()
                {
                    self.cfg.running = !self.cfg.running;
                    if self.cfg.running {
                        self.last_switch = Instant::now();
                    }
                }
                if ui.button("⏮ 上一张").clicked() {
                    actions.push(Action::Prev);
                }
                if ui.button("⏭ 下一张").clicked() {
                    actions.push(Action::Next);
                }
                if ui.button("🎲 随机").clicked() {
                    actions.push(Action::Random);
                }
                let mut sh = self.cfg.shuffle;
                if ui.checkbox(&mut sh, "随机顺序").changed() {
                    self.cfg.shuffle = sh;
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("间隔：").size(13.0).color(theme::subtext()));
                ui.add(
                    egui::Slider::new(&mut self.cfg.interval_secs, 5..=86400)
                        .logarithmic(true)
                        .text(""),
                );
                ui.label(
                    RichText::new(fmt_duration(self.cfg.interval_secs))
                        .size(12.0)
                        .color(theme::subtext()),
                );
            });
            ui.horizontal_wrapped(|ui| {
                for (label, secs) in PRESETS {
                    let selected = self.cfg.interval_secs == *secs;
                    let fill = if selected { theme::accent() } else { theme::extreme() };
                    let color = if selected { Color32::WHITE } else { theme::text() };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(*label).size(12.0).color(color))
                                .fill(fill)
                                .rounding(Rounding::same(6.0)),
                        )
                        .clicked()
                    {
                        self.cfg.interval_secs = *secs;
                    }
                }
            });
            if self.cfg.running {
                let iv = self.cfg.interval_secs.max(1);
                let el = self.last_switch.elapsed().as_secs();
                let frac = (el as f32 / iv as f32).clamp(0.0, 1.0);
                let remain = iv.saturating_sub(el);
                ui.add_space(4.0);
                ui.add(
                    egui::ProgressBar::new(frac)
                        .desired_width(ui.available_width())
                        .text(format!("距离下次更换：{}", fmt_duration(remain))),
                );
            }
        } else {
            ui.horizontal(|ui| {
                if ui.button("换下一张").clicked() {
                    actions.push(Action::Next);
                }
                if ui.button("随机一张").clicked() {
                    actions.push(Action::Random);
                }
                ui.label(
                    RichText::new("当前为「固定壁纸」模式：在“壁纸库”点击图片即可固定使用。")
                        .size(12.0)
                        .color(theme::subtext()),
                );
            });
        }
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.heading(RichText::new("设置").size(18.0).color(theme::text()));
        ui.add_space(8.0);

        egui::Frame::none()
            .fill(theme::card())
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(RichText::new("显示方式（Windows 生效）").size(13.0).strong().color(theme::text()));
                egui::ComboBox::from_id_salt("fit_combo")
                    .selected_text(self.cfg.fit.label())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for f in Fit::all() {
                            ui.selectable_value(&mut self.cfg.fit, f, f.label());
                        }
                    });
                ui.add_space(10.0);
                ui.label(RichText::new("主题").size(13.0).strong().color(theme::text()));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.cfg.theme, 0, "浅色");
                    ui.selectable_value(&mut self.cfg.theme, 1, "深色");
                });
                ui.add_space(10.0);
                let mut rc = self.cfg.recursive;
                if ui.checkbox(&mut rc, "扫描时包含子文件夹").changed() {
                    self.cfg.recursive = rc;
                }
            });
        ui.add_space(10.0);

        egui::Frame::none()
            .fill(theme::card())
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(RichText::new("配置文件").size(13.0).strong().color(theme::text()));
                ui.label(
                    RichText::new(crate::config::config_path().display().to_string())
                        .size(12.0)
                        .color(theme::subtext()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("打开配置目录").clicked() {
                        actions.push(Action::OpenConfig);
                    }
                    if ui.button("立即保存").clicked() {
                        actions.push(Action::Save);
                    }
                });
            });
        ui.add_space(10.0);
        ui.label(
            RichText::new("提示：Windows 使用系统 API 设置壁纸并写入显示方式；macOS 通过 System Events；Linux 使用 GNOME(gsettings) 或 feh。")
                .size(12.0)
                .color(theme::subtext()),
        );
    }

    fn ui_about(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("🖼").size(54.0).color(theme::accent()));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Wallpaper Studio")
                    .size(22.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.label(
                RichText::new(format!("版本 {}", env!("CARGO_PKG_VERSION")))
                    .size(12.5)
                    .color(theme::subtext()),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("跨平台桌面壁纸工具（Windows / macOS / Linux），支持固定壁纸与轮播。")
                    .size(13.0)
                    .color(theme::text()),
            );
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            ui.vertical(|ui| {
                for b in [
                    "• 从本地文件夹批量载入图片（可含子文件夹）",
                    "• 固定壁纸：点击即设为桌面壁纸",
                    "• 轮播：按间隔自动更换，支持顺序 / 随机",
                    "• 显示方式：填充 / 适应 / 拉伸 / 居中 / 平铺",
                    "• 浅色 / 深色主题，配置自动保存",
                ] {
                    ui.label(RichText::new(b).size(13.0).color(theme::subtext()));
                    ui.add_space(2.0);
                }
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_theme != self.cfg.theme {
            theme::apply(ctx, self.cfg.theme == 1);
            self.applied_theme = self.cfg.theme;
        }

        let snap = (
            self.cfg.mode,
            self.cfg.interval_secs,
            self.cfg.shuffle,
            self.cfg.fit,
            self.cfg.theme,
            self.cfg.recursive,
            self.cfg.running,
            self.cfg.search.clone(),
            self.cfg.folder.clone(),
            self.cfg.last_path.clone(),
        );

        // drain finished thumbnails
        for (p, t) in self.thumb.poll() {
            match t {
                Some(t) => {
                    let img = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba);
                    let tex = ctx.load_texture(
                        format!("thumb:{}", p.display()),
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.tex_cache.insert(p, Some(tex));
                }
                None => {
                    self.tex_cache.insert(p, None);
                }
            }
        }

        // keyboard shortcuts
        if self.images.len() > 1 {
            let (right, left) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::ArrowLeft),
                )
            });
            if right {
                self.advance(ctx);
            } else if left {
                self.step_back(ctx);
            }
        }

        // ensure preview for current
        if let Some(p) = self.current_path() {
            if self.preview.as_ref().map(|(q, _)| q != &p).unwrap_or(true) {
                self.load_preview(ctx, &p);
            }
        }

        // slideshow ticking
        if self.cfg.mode == Mode::Slideshow && self.cfg.running && !self.images.is_empty() {
            let iv = Duration::from_secs(self.cfg.interval_secs.max(1));
            if self.last_switch.elapsed() >= iv {
                self.advance(ctx);
            }
            let remain = iv.saturating_sub(self.last_switch.elapsed());
            ctx.request_repaint_after(remain.min(Duration::from_millis(400)));
        } else if !self.images.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(350));
        }

        let mut actions: Vec<Action> = Vec::new();
        self.draw_header(ctx);
        self.draw_sidebar(ctx);
        self.draw_center(ctx, &mut actions);
        self.apply(ctx, actions);

        let snap2 = (
            self.cfg.mode,
            self.cfg.interval_secs,
            self.cfg.shuffle,
            self.cfg.fit,
            self.cfg.theme,
            self.cfg.recursive,
            self.cfg.running,
            self.cfg.search.clone(),
            self.cfg.folder.clone(),
            self.cfg.last_path.clone(),
        );
        if snap != snap2 {
            self.cfg.save();
        }

        // status bar
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::card())
                    .inner_margin(egui::Margin::symmetric(18.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(12.0).color(theme::subtext()));
                });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cfg.save();
    }
}

const PRESETS: &[(&str, u64)] = &[
    ("10秒", 10),
    ("30秒", 30),
    ("1分", 60),
    ("5分", 300),
    ("15分", 900),
    ("30分", 1800),
    ("1时", 3600),
    ("6时", 21600),
    ("1天", 86400),
];

fn chip(ui: &mut egui::Ui, text: &str, fill: Color32, color: Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(Rounding::same(20.0))
        .inner_margin(egui::Margin::symmetric(10.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).color(color).strong());
        });
}

fn nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> bool {
    let fill = if selected { theme::accent_soft() } else { Color32::TRANSPARENT };
    let txt = if selected { theme::accent() } else { theme::subtext() };
    let resp = ui.add_sized(
        [ui.available_width(), 38.0],
        egui::Button::new(RichText::new(format!("{icon}   {label}")).size(14.0).color(txt))
            .fill(fill)
            .rounding(Rounding::same(8.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if recursive {
                collect_images(&p, recursive, out);
            }
        } else if is_image(&p) {
            out.push(p);
        }
    }
}

fn is_image(p: &Path) -> bool {
    match p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()) {
        Some(ext) => matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "tif" | "tiff"
        ),
        None => false,
    }
}

fn fmt_duration(secs: u64) -> String {
    if secs >= 86400 {
        format!("{}天{}时", secs / 86400, (secs % 86400) / 3600)
    } else if secs >= 3600 {
        format!("{}时{}分", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}分{}秒", secs / 60, secs % 60)
    } else {
        format!("{}秒", secs)
    }
}

fn ellipsis(s: &str, max: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() <= max {
        s.to_string()
    } else {
        let mut t: String = cs[..max].iter().collect();
        t.push('…');
        t
    }
}

fn open_path(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}
