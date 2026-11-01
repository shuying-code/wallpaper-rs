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
use std::sync::mpsc::Receiver;
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
    ScanAllDisks,
    ShiftOnce,
    Minimize,
}

/// Snapshot of the persisted config, used to detect changes and autosave.
#[derive(PartialEq)]
struct Snapshot {
    mode: Mode,
    interval: u64,
    shuffle: bool,
    fit: Fit,
    theme: usize,
    recursive: bool,
    running: bool,
    search: String,
    folder: Option<PathBuf>,
    last_path: Option<PathBuf>,
    oled: bool,
    shift_iv: u64,
    shift_max: u32,
    close_bg: bool,
}

impl Snapshot {
    fn of(c: &Config) -> Self {
        Self {
            mode: c.mode,
            interval: c.interval_secs,
            shuffle: c.shuffle,
            fit: c.fit,
            theme: c.theme,
            recursive: c.recursive,
            running: c.running,
            search: c.search.clone(),
            folder: c.folder.clone(),
            last_path: c.last_path.clone(),
            oled: c.oled_guard,
            shift_iv: c.shift_interval_secs,
            shift_max: c.shift_max_px,
            close_bg: c.close_to_background,
        }
    }
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
    hotkey: crate::hotkey::Hotkey,
    scan_rx: Option<Receiver<Vec<PathBuf>>>,
    scanning: bool,
    shift_rx: Option<Receiver<Result<(), String>>>,
    shifting: bool,
    last_shift: Instant,
    window_hidden: bool,
    monitors: usize,
    primary_res: (i32, i32),
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
            hotkey: crate::hotkey::start(cc.egui_ctx.clone()),
            scan_rx: None,
            scanning: false,
            shift_rx: None,
            shifting: false,
            last_shift: Instant::now(),
            window_hidden: false,
            monitors: wallpaper::monitor_count(),
            primary_res: wallpaper::primary_resolution(),
        };
        // Keep timers (slideshow / OLED shift) ticking even while minimized.
        {
            let c = cc.egui_ctx.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                c.request_repaint();
            });
        }
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
                Action::ScanAllDisks => self.start_full_scan(),
                Action::ShiftOnce => self.spawn_shift(),
                Action::Minimize => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    self.window_hidden = true;
                    self.status = "已最小化到后台（用全局快捷键唤醒）".to_string();
                }
            }
        }
    }

    fn start_full_scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.scan_rx = Some(crate::diskscan::start());
        self.status = "正在全盘扫描图片…（后台进行，完成后自动载入）".to_string();
    }

    fn spawn_shift(&mut self) {
        if self.shifting {
            return;
        }
        let path = match self.current_path() {
            Some(p) => p,
            None => {
                self.status = "请先选择一张壁纸再执行位移".to_string();
                return;
            }
        };
        self.shifting = true;
        self.last_shift = Instant::now();
        let (tx, rx) = std::sync::mpsc::channel();
        self.shift_rx = Some(rx);
        let maxp = self.cfg.shift_max_px.max(1) as i64;
        let fit = self.cfg.fit;
        let out_dir = config_dir();
        std::thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let dx = rng.gen_range(-maxp..=maxp);
            let dy = rng.gen_range(-maxp..=maxp);
            let r = crate::burnin::apply_shift(&path, fit, dx, dy, &out_dir);
            let _ = tx.send(r);
        });
    }

    // ---- UI sections ------------------------------------------------------
    fn draw_header(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("hdr")
            .frame(
                egui::Frame::none()
                    .fill(theme::surface())
                    .inner_margin(egui::Margin::symmetric(20.0, 13.0)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.horizontal(|ui| {
                    // Brand mark
                    let (logo, _) =
                        ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                    ui.painter().rect_filled(logo, Rounding::same(12.0), theme::accent());
                    ui.painter().text(
                        logo.center(),
                        egui::Align2::CENTER_CENTER,
                        "🖼",
                        egui::FontId::proportional(20.0),
                        Color32::WHITE,
                    );
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Wallpaper Studio")
                                .size(18.0)
                                .strong()
                                .color(theme::text()),
                        );
                        ui.label(
                            RichText::new("桌面壁纸 · 固定与轮播")
                                .size(11.0)
                                .color(theme::subtext()),
                        );
                    });
                    ui.add_space(10.0);
                    let mode = if self.cfg.mode == Mode::Slideshow { "轮播" } else { "固定" };
                    chip(ui, mode, theme::accent_soft(), theme::accent());
                    if self.cfg.mode == Mode::Slideshow {
                        if self.cfg.running {
                            chip(ui, "● 运行中", theme::accent_soft(), theme::ok());
                        } else {
                            chip(ui, "已暂停", theme::extreme(), theme::subtext());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(p) = self.current_path() {
                            let name = p
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            egui::Frame::none()
                                .fill(theme::card())
                                .stroke(Stroke::new(1.0_f32, theme::border()))
                                .rounding(Rounding::same(20.0))
                                .inner_margin(egui::Margin::symmetric(12.0, 5.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("🖼  {}", ellipsis(&name, 34)))
                                            .size(11.5)
                                            .color(theme::subtext()),
                                    );
                                });
                        }
                    });
                });
                // hairline under the header
                ui.painter().line_segment(
                    [
                        egui::pos2(full.left(), full.bottom()),
                        egui::pos2(full.right(), full.bottom()),
                    ],
                    Stroke::new(1.0_f32, theme::border()),
                );
            });
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(214.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::surface())
                    .inner_margin(egui::Margin::symmetric(12.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.add_space(2.0);
                ui.label(RichText::new("工作区").size(10.5).color(theme::subtext()));
                ui.add_space(6.0);
                if nav_button(ui, "🖼", "壁纸库", self.tab == Tab::Library) {
                    self.tab = Tab::Library;
                }
                ui.add_space(3.0);
                if nav_button(ui, "🎞", "轮播", self.tab == Tab::Slideshow) {
                    self.tab = Tab::Slideshow;
                }
                ui.add_space(3.0);
                if nav_button(ui, "⚙", "设置", self.tab == Tab::Settings) {
                    self.tab = Tab::Settings;
                }
                ui.add_space(3.0);
                if nav_button(ui, "ℹ", "关于", self.tab == Tab::About) {
                    self.tab = Tab::About;
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("v{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS))
                            .size(10.0)
                            .color(theme::subtext()),
                    );
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);
                    let dark = self.cfg.theme == 1;
                    let label = if dark { "☀  浅色主题" } else { "🌙  深色主题" };
                    if theme_toggle(ui, label) {
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
            .show(ctx, |ui| {
                theme::paint_bg_gradient(ui, ui.max_rect());
                match self.tab {
                    Tab::Library => self.ui_library(ui, actions),
                    Tab::Slideshow => self.ui_slideshow(ui, actions),
                    Tab::Settings => {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.ui_settings(ui, actions));
                    }
                    Tab::About => {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.ui_about(ui));
                    }
                }
            });
    }

    fn ui_library(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let folder = self
            .cfg
            .folder
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（未选择文件夹）".to_string());
        let total = self.images.len();
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 2.0),
                blur: 10.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(16.0, 13.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📁").size(15.0).color(theme::accent()));
                    egui::Frame::none()
                        .fill(theme::extreme())
                        .rounding(Rounding::same(8.0))
                        .inner_margin(egui::Margin::symmetric(11.0, 5.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(ellipsis(&folder, 54))
                                    .size(12.5)
                                    .color(theme::text()),
                            );
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "选择文件夹…").clicked() {
                            actions.push(Action::PickFolder);
                        }
                        if ghost_button(ui, "全盘扫描").clicked() {
                            actions.push(Action::ScanAllDisks);
                        }
                        if ghost_button(ui, "重新扫描").clicked() {
                            actions.push(Action::Rescan);
                        }
                        if ghost_button(ui, "打开目录").clicked() {
                            actions.push(Action::OpenFolder);
                        }
                    });
                });
                ui.add_space(11.0);
                ui.horizontal(|ui| {
                    let mut rc = self.cfg.recursive;
                    if ui.checkbox(&mut rc, "包含子文件夹").changed() {
                        self.cfg.recursive = rc;
                        actions.push(Action::Rescan);
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new("🔍").size(13.0).color(theme::subtext()));
                    egui::Frame::none()
                        .fill(theme::extreme())
                        .stroke(Stroke::new(1.0_f32, theme::border()))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(egui::Margin::symmetric(9.0, 4.0))
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.cfg.search)
                                    .hint_text("按文件名过滤…")
                                    .desired_width(210.0)
                                    .frame(false),
                            );
                        });
                    ui.add_space(10.0);
                    chip(
                        ui,
                        &format!("共 {total} 张"),
                        theme::accent_soft(),
                        theme::accent(),
                    );
                    if self.scanning {
                        ui.add_space(6.0);
                        chip(ui, "扫描中…", theme::accent_soft(), theme::accent());
                    }
                });
            });
        ui.add_space(12.0);

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
            ui.add_space(70.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(104.0, 104.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 52.0, theme::accent_soft());
            ui.painter().circle_stroke(r.center(), 51.0, Stroke::new(1.0_f32, theme::accent()));
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "🖼",
                egui::FontId::proportional(42.0),
                theme::accent(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new("还没有壁纸")
                    .size(20.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("选择一个包含图片的文件夹，即可开始使用固定壁纸或轮播")
                    .size(13.0)
                    .color(theme::subtext()),
            );
            ui.add_space(18.0);
            if primary_button(ui, "选择壁纸文件夹…").clicked() {
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
        let base_stroke = if is_current {
            Stroke::new(2.0_f32, theme::accent())
        } else {
            Stroke::new(1.0_f32, theme::border())
        };
        let inner = egui::Frame::none()
            .fill(theme::card())
            .stroke(base_stroke)
            .rounding(Rounding::same(14.0))
            .inner_margin(egui::Margin::same(CARD_PAD + 2.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(CELL_W);
                    ui.set_max_width(CELL_W);
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(CELL_W, IMG_H), egui::Sense::click());
                    {
                        let painter = ui.painter();
                        painter.rect_filled(rect, Rounding::same(9.0), theme::extreme());
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
                        // Hover overlay with a call-to-action pill.
                        if resp.hovered() {
                            painter.rect_filled(
                                rect,
                                Rounding::same(9.0),
                                Color32::from_black_alpha(70),
                            );
                            let pill =
                                egui::Rect::from_center_size(rect.center(), Vec2::new(96.0, 30.0));
                            painter.rect_filled(pill, Rounding::same(15.0), theme::accent());
                            painter.text(
                                pill.center(),
                                egui::Align2::CENTER_CENTER,
                                "设为壁纸",
                                egui::FontId::proportional(12.5),
                                Color32::WHITE,
                            );
                        }
                        // Current badge, top-right of the thumbnail.
                        if is_current {
                            let badge = egui::Rect::from_min_size(
                                egui::pos2(rect.right() - 52.0, rect.top() + 8.0),
                                Vec2::new(44.0, 20.0),
                            );
                            painter.rect_filled(badge, Rounding::same(10.0), theme::ok());
                            painter.text(
                                badge.center(),
                                egui::Align2::CENTER_CENTER,
                                "当前",
                                egui::FontId::proportional(11.0),
                                Color32::WHITE,
                            );
                        }
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        actions.push(Action::SetPath(path.to_path_buf()));
                    }
                    ui.add_space(9.0);
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    ui.add(egui::Label::new(
                        RichText::new(ellipsis(&name, 20))
                            .size(12.5)
                            .strong()
                            .color(theme::text()),
                    ));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let ext = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_ascii_uppercase())
                            .unwrap_or_default();
                        egui::Frame::none()
                            .fill(theme::extreme())
                            .rounding(Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(ext).size(9.5).color(theme::subtext()));
                            });
                        if is_current {
                            ui.label(RichText::new("● 当前").size(11.0).color(theme::ok()));
                        } else if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("设为壁纸").size(11.0).color(theme::accent()),
                                )
                                .fill(theme::accent_soft())
                                .rounding(Rounding::same(7.0)),
                            )
                            .clicked()
                        {
                            actions.push(Action::SetPath(path.to_path_buf()));
                        }
                    });
                });
            });
        // Accent outline on hover, drawn over the card.
        if inner.response.hovered() && !is_current {
            ui.painter().rect_stroke(
                inner.response.rect,
                Rounding::same(14.0),
                Stroke::new(1.5_f32, theme::accent()),
            );
        }
    }

    fn ui_slideshow(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let cur = self.current_path();
        let w = ui.available_width();
        let h = (ui.available_height() * 0.56).clamp(200.0, 460.0);

        // Preview card ------------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(16.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 4.0),
                blur: 16.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .show(ui, |ui| {
                let (rect, _resp) =
                    ui.allocate_exact_size(Vec2::new(w - 2.0, h), egui::Sense::hover());
                {
                    let painter = ui.painter();
                    painter.rect_filled(rect, Rounding::same(16.0), theme::extreme());
                    let inner = rect.shrink(12.0);
                    let mut painted = false;
                    if let (Some((p, tex)), Some(c)) = (&self.preview, &cur) {
                        if p == c {
                            let ts = tex.size();
                            let tw = ts[0] as f32;
                            let th = ts[1] as f32;
                            if tw > 0.0 && th > 0.0 {
                                let s = (inner.width() / tw).min(inner.height() / th);
                                let size = Vec2::new(tw * s, th * s);
                                let ir = egui::Rect::from_center_size(inner.center(), size);
                                painter.image(
                                    tex.id(),
                                    ir,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    Color32::WHITE,
                                );
                                painted = true;
                            }
                        }
                    }
                    if !painted {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "🖼",
                            egui::FontId::proportional(46.0),
                            theme::subtext(),
                        );
                    }
                    // status chip (top-left)
                    let (chip_txt, chip_fill, chip_col) = if self.cfg.mode == Mode::Slideshow {
                        if self.cfg.running {
                            ("● 正在轮播", theme::accent2(), Color32::WHITE)
                        } else {
                            ("已暂停", theme::extreme(), theme::subtext())
                        }
                    } else {
                        ("● 固定壁纸", theme::accent_soft(), theme::accent())
                    };
                    let g = painter.layout_no_wrap(
                        chip_txt.to_string(),
                        egui::FontId::proportional(11.5),
                        chip_col,
                    );
                    let cw = g.size().x + 22.0;
                    let pill = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 14.0, rect.top() + 14.0),
                        egui::vec2(cw, 26.0),
                    );
                    painter.rect_filled(pill, Rounding::same(13.0), chip_fill);
                    painter.galley(
                        egui::pos2(pill.left() + 11.0, pill.center().y - g.size().y * 0.5),
                        g,
                        chip_col,
                    );
                    // filename pill (bottom-left)
                    if let Some(pth) = &cur {
                        let name = pth
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let g2 = painter.layout_no_wrap(
                            ellipsis(&name, 46),
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                        let pw = g2.size().x + 22.0;
                        let p2 = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 14.0, rect.bottom() - 40.0),
                            egui::vec2(pw, 26.0),
                        );
                        painter.rect_filled(p2, Rounding::same(13.0), Color32::from_black_alpha(120));
                        painter.galley(
                            egui::pos2(p2.left() + 11.0, p2.center().y - g2.size().y * 0.5),
                            g2,
                            Color32::WHITE,
                        );
                    }
                }
            });
        ui.add_space(12.0);

        // Controls card -----------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(16.0, 14.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("播放模式").size(12.0).color(theme::subtext()));
                    ui.add_space(4.0);
                    ui.selectable_value(&mut self.cfg.mode, Mode::Fixed, " 固定壁纸 ");
                    ui.selectable_value(&mut self.cfg.mode, Mode::Slideshow, " 轮播 ");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.cfg.mode == Mode::Slideshow {
                            let mut sh = self.cfg.shuffle;
                            if ui.checkbox(&mut sh, "随机顺序").changed() {
                                self.cfg.shuffle = sh;
                            }
                        }
                    });
                });
                ui.add_space(10.0);
                if self.cfg.mode == Mode::Slideshow {
                    ui.horizontal(|ui| {
                        let start = if self.cfg.running { "暂停轮播" } else { "▶  开始轮播" };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(start).size(13.0).color(Color32::WHITE).strong(),
                                )
                                .fill(theme::accent())
                                .rounding(Rounding::same(10.0))
                                .min_size(Vec2::new(124.0, 38.0)),
                            )
                            .clicked()
                        {
                            self.cfg.running = !self.cfg.running;
                            if self.cfg.running {
                                self.last_switch = Instant::now();
                            }
                        }
                        ui.add_space(4.0);
                        if ghost_button(ui, "⏮ 上一张").clicked() {
                            actions.push(Action::Prev);
                        }
                        if ghost_button(ui, "⏭ 下一张").clicked() {
                            actions.push(Action::Next);
                        }
                        if ghost_button(ui, "🎲 随机").clicked() {
                            actions.push(Action::Random);
                        }
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("间隔").size(12.5).color(theme::subtext()));
                        ui.add(
                            egui::Slider::new(&mut self.cfg.interval_secs, 5..=86400)
                                .logarithmic(true)
                                .show_value(false)
                                .text(""),
                        );
                        chip(
                            ui,
                            &fmt_duration(self.cfg.interval_secs),
                            theme::accent_soft(),
                            theme::accent(),
                        );
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        for (label, secs) in PRESETS {
                            let selected = self.cfg.interval_secs == *secs;
                            let fill = if selected { theme::accent() } else { theme::extreme() };
                            let color = if selected { Color32::WHITE } else { theme::text() };
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(*label).size(12.0).color(color))
                                        .fill(fill)
                                        .rounding(Rounding::same(16.0))
                                        .min_size(Vec2::new(0.0, 28.0)),
                                )
                                .clicked()
                            {
                                self.cfg.interval_secs = *secs;
                            }
                        }
                    });
                    if self.cfg.running {
                        ui.add_space(12.0);
                        let iv = self.cfg.interval_secs.max(1);
                        let el = self.last_switch.elapsed().as_secs();
                        let frac = (el as f32 / iv as f32).clamp(0.0, 1.0);
                        let remain = iv.saturating_sub(el);
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .desired_width(ui.available_width())
                                .fill(theme::accent())
                                .text(format!("距离下次更换：{}", fmt_duration(remain))),
                        );
                    }
                } else {
                    ui.horizontal(|ui| {
                        if ghost_button(ui, "换下一张").clicked() {
                            actions.push(Action::Next);
                        }
                        if ghost_button(ui, "🎲 随机一张").clicked() {
                            actions.push(Action::Random);
                        }
                        ui.label(
                            RichText::new("固定模式：在「壁纸库」点击任意图片即可设为桌面壁纸。")
                                .size(12.5)
                                .color(theme::subtext()),
                        );
                    });
                }
            });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        page_title(ui, "设置", "个性化壁纸的显示与启动方式");
        ui.add_space(14.0);

        // Appearance & display ---------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "外观与显示");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "显示方式", "填充 / 适应 / 拉伸 / 居中 / 平铺");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("fit_combo")
                            .selected_text(self.cfg.fit.label())
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for f in Fit::all() {
                                    ui.selectable_value(&mut self.cfg.fit, f, f.label());
                                }
                            });
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "主题", "浅色 / 深色，即时生效");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.selectable_value(&mut self.cfg.theme, 1, "深色");
                        ui.selectable_value(&mut self.cfg.theme, 0, "浅色");
                    });
                });
            });
        ui.add_space(12.0);

        // Behaviour ---------------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "行为");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "扫描包含子文件夹", "载入壁纸库时递归子目录");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut rc = self.cfg.recursive;
                        if ui.checkbox(&mut rc, "").changed() {
                            self.cfg.recursive = rc;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "开机自动启动", "登录系统后自动运行本程序");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut auto = self.cfg.autostart;
                        if ui.checkbox(&mut auto, "").changed() {
                            match crate::autostart::set_enabled(auto) {
                                Ok(_) => {
                                    self.cfg.autostart = auto;
                                    self.status = if auto {
                                        "已开启开机自启".to_string()
                                    } else {
                                        "已关闭开机自启".to_string()
                                    };
                                }
                                Err(e) => {
                                    self.cfg.autostart = !auto;
                                    self.status = format!("开机自启设置失败：{e}");
                                }
                            }
                        }
                    });
                });
            });
        ui.add_space(12.0);

        // OLED burn-in guard ------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "OLED 防烧屏");
                ui.add_space(4.0);
                ui.label(
                    RichText::new("长时间显示同一张壁纸，OLED 屏幕上的固定元素（任务栏、时钟、图标）可能留下残影甚至烧屏。开启守护后会定期把壁纸微移几个像素，让像素负载均摊。")
                        .size(11.5)
                        .color(theme::subtext()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "防烧屏守护", "定期微移壁纸，缓解残影 / 烧屏");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut g = self.cfg.oled_guard;
                        if ui.checkbox(&mut g, "").changed() {
                            self.cfg.oled_guard = g;
                            if g {
                                self.last_shift = Instant::now();
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "位移间隔", "每隔多久微移一次（秒）");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled(
                            self.cfg.oled_guard,
                            egui::DragValue::new(&mut self.cfg.shift_interval_secs)
                                .range(10..=3600)
                                .suffix(" 秒"),
                        );
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "最大位移", "单次最大偏移像素（1–30）");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled(
                            self.cfg.oled_guard,
                            egui::DragValue::new(&mut self.cfg.shift_max_px)
                                .range(1..=30)
                                .suffix(" px"),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "立即位移一次").clicked() {
                        actions.push(Action::ShiftOnce);
                    }
                    ui.label(
                        RichText::new("建议同时开启轮播并缩短间隔，防烧屏效果更好。")
                            .size(11.0)
                            .color(theme::subtext()),
                    );
                });
            });
        ui.add_space(12.0);

        // Resident & hotkey -------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "常驻后台与快捷键");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "关闭时最小化到后台", "关闭窗口不退出，轮播 / 防烧屏继续运行");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut cb = self.cfg.close_to_background;
                        if ui.checkbox(&mut cb, "").changed() {
                            self.cfg.close_to_background = cb;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                let reg = self.hotkey.is_registered();
                ui.horizontal(|ui| {
                    let desc = if reg {
                        format!("{}：最小化 / 唤回窗口", self.hotkey.label())
                    } else {
                        "注册失败：快捷键可能被其它软件占用".to_string()
                    };
                    setting_label(ui, "全局快捷键", &desc);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if reg {
                            let txt = format!("已注册 · {}", self.hotkey.label());
                            chip(ui, &txt, theme::accent_soft(), theme::ok());
                        } else {
                            let txt = format!("注册失败 (code {})", self.hotkey.error_code());
                            chip(ui, &txt, theme::extreme(), theme::subtext());
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "立即最小化", "收进任务栏，后台继续运行");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ghost_button(ui, "最小化到后台").clicked() {
                            actions.push(Action::Minimize);
                        }
                    });
                });
            });
        ui.add_space(12.0);

        // Display & compatibility -------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "显示与兼容");
                ui.add_space(10.0);
                let (pw, ph) = self.primary_res;
                ui.label(
                    RichText::new(format!(
                        "检测到 {} 台显示器 · 主屏 {}×{} · 当前主题 {}",
                        self.monitors,
                        pw,
                        ph,
                        if self.cfg.theme == 1 { "深色" } else { "浅色" }
                    ))
                    .size(12.0)
                    .color(theme::text()),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("壁纸将应用到所有显示器（Windows 通过系统 API）。高 DPI 缩放由系统自动适配。")
                        .size(11.0)
                        .color(theme::subtext()),
                );
            });
        ui.add_space(12.0);

        // Config file -------------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(18.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "配置文件");
                ui.add_space(10.0);
                egui::Frame::none()
                    .fill(theme::extreme())
                    .rounding(Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(11.0, 7.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(crate::config::config_path().display().to_string())
                                .size(12.0)
                                .color(theme::subtext()),
                        );
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ghost_button(ui, "打开配置目录").clicked() {
                        actions.push(Action::OpenConfig);
                    }
                    if primary_button(ui, "立即保存").clicked() {
                        actions.push(Action::Save);
                    }
                });
            });
        ui.add_space(12.0);
        ui.label(
            RichText::new("提示：Windows 使用系统 API 设置壁纸并写入显示方式；Linux 使用 GNOME(gsettings，含 picture-options) 或 feh；macOS 通过 System Events。开机自启在 Windows 写入 Run 注册表、macOS 写入 LaunchAgent、Linux 写入 autostart .desktop。")
                .size(11.5)
                .color(theme::subtext()),
        );
    }

    fn ui_about(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(84.0, 84.0), egui::Sense::hover());
            ui.painter().rect_filled(r, Rounding::same(22.0), theme::accent());
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "🖼",
                egui::FontId::proportional(40.0),
                Color32::WHITE,
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Wallpaper Studio")
                    .size(24.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(6.0);
            let vtxt = format!("v{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS);
            let g = ui
                .painter()
                .layout_no_wrap(vtxt, egui::FontId::proportional(11.5), theme::accent());
            let pw = g.size().x + 24.0;
            let (vr, _) = ui.allocate_exact_size(Vec2::new(pw, 24.0), egui::Sense::hover());
            ui.painter().rect_filled(vr, Rounding::same(12.0), theme::accent_soft());
            ui.painter().galley(
                egui::pos2(vr.left() + 12.0, vr.center().y - g.size().y * 0.5),
                g,
                theme::accent(),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("跨平台桌面壁纸工具（Windows / macOS / Linux），支持固定壁纸与轮播。")
                    .size(13.0)
                    .color(theme::subtext()),
            );
        });
        ui.add_space(18.0);
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .shadow(egui::epaint::Shadow {
                offset: egui::vec2(0.0, 3.0),
                blur: 12.0,
                spread: 0.0,
                color: theme::shadow(),
            })
            .inner_margin(egui::Margin::symmetric(20.0, 16.0))
            .show(ui, |ui| {
                section_header(ui, "功能一览");
                ui.add_space(10.0);
                let features = [
                    ("📁", "批量载入", "从本地文件夹载入图片，可递归子目录"),
                    ("📌", "固定壁纸", "点击任意图片即设为桌面壁纸"),
                    ("🎞", "自动轮播", "按间隔更换，支持顺序 / 随机"),
                    ("🖥", "显示方式", "填充 / 适应 / 拉伸 / 居中 / 平铺"),
                    ("🎨", "主题切换", "浅色 / 深色主题，即时生效"),
                    ("💾", "配置保存", "设置自动持久化，开机自启可选"),
                ];
                ui.columns(2, |cols| {
                    for (i, (icon, title, desc)) in features.iter().enumerate() {
                        let col = &mut cols[i % 2];
                        col.horizontal(|ui| {
                            let (ir, _) = ui
                                .allocate_exact_size(Vec2::new(30.0, 30.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(ir, Rounding::same(9.0), theme::accent_soft());
                            ui.painter().text(
                                ir.center(),
                                egui::Align2::CENTER_CENTER,
                                *icon,
                                egui::FontId::proportional(15.0),
                                theme::accent(),
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(*title)
                                        .size(12.5)
                                        .strong()
                                        .color(theme::text()),
                                );
                                ui.label(RichText::new(*desc).size(11.0).color(theme::subtext()));
                            });
                        });
                        col.add_space(8.0);
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

        // Global hotkey: toggle minimize / restore.
        if self.hotkey.take_toggle() {
            if self.window_hidden {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.window_hidden = false;
                self.status = "已唤回窗口".to_string();
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                self.window_hidden = true;
                self.status = "已最小化到后台（用全局快捷键唤醒）".to_string();
            }
        }

        // Close-to-background: keep running (wallpaper rotation / OLED shift) when
        // the window is closed, provided the global hotkey is available to restore.
        if ctx.input(|i| i.viewport().close_requested())
            && self.cfg.close_to_background
            && self.hotkey.is_registered()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            self.window_hidden = true;
            self.status = "已最小化到后台继续运行（用全局快捷键唤醒）".to_string();
        }

        // Poll background full-disk scan.
        let scan_result = self.scan_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(found) = scan_result {
            self.scan_rx = None;
            self.scanning = false;
            self.thumb.reset();
            self.tex_cache.clear();
            self.preview = None;
            self.images = found;
            self.current = if self.images.is_empty() { None } else { Some(0) };
            self.status = if self.images.is_empty() {
                "全盘扫描完成，但未找到图片".to_string()
            } else {
                format!("全盘扫描完成：共 {} 张图片", self.images.len())
            };
            self.last_switch = Instant::now();
        }

        // Poll background OLED pixel-shift.
        let shift_result = self.shift_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(res) = shift_result {
            self.shift_rx = None;
            self.shifting = false;
            self.status = match res {
                Ok(_) => "已执行防烧屏像素位移".to_string(),
                Err(e) => format!("像素位移失败：{e}"),
            };
        }

        // OLED burn-in guard scheduling: micro-shift on an interval.
        if self.cfg.oled_guard
            && !self.shifting
            && !self.images.is_empty()
            && self.last_shift.elapsed().as_secs() >= self.cfg.shift_interval_secs.max(10)
        {
            self.spawn_shift();
        }

        let snap = Snapshot::of(&self.cfg);

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

        let snap2 = Snapshot::of(&self.cfg);
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let hint = if self.hotkey.is_registered() {
                            format!("{} 后台 / 唤回", self.hotkey.label())
                        } else {
                            "全局快捷键不可用".to_string()
                        };
                        ui.label(RichText::new(hint).size(11.0).color(theme::subtext()));
                    });
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

fn page_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(Vec2::new(4.0, 34.0), egui::Sense::hover());
        ui.painter().rect_filled(bar, Rounding::same(2.0), theme::accent());
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(19.0).strong().color(theme::text()));
            ui.label(RichText::new(subtitle).size(11.5).color(theme::subtext()));
        });
    });
}

fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(bar, Rounding::same(1.5), theme::accent());
        ui.add_space(4.0);
        ui.label(RichText::new(text).size(14.0).strong().color(theme::text()));
    });
}

fn setting_label(ui: &mut egui::Ui, title: &str, desc: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).size(13.0).strong().color(theme::text()));
        ui.label(RichText::new(desc).size(11.0).color(theme::subtext()));
    });
}

fn nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> bool {
    let height = 42.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let hovered = resp.hovered();
    let t = ui.ctx().animate_bool(resp.id, hovered);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, Rounding::same(11.0), theme::accent_soft());
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + 9.0),
            egui::vec2(3.0, height - 18.0),
        );
        painter.rect_filled(bar, Rounding::same(2.0), theme::accent());
    } else if t > 0.01 {
        let base = theme::accent_soft();
        let c = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), (60.0 * t) as u8);
        painter.rect_filled(rect, Rounding::same(11.0), c);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        egui::vec2(26.0, 26.0),
    );
    let icon_bg = if selected { theme::accent() } else { theme::extreme() };
    painter.rect_filled(icon_rect, Rounding::same(8.0), icon_bg);
    let icon_color = if selected { Color32::WHITE } else { theme::subtext() };
    painter.text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        icon_color,
    );
    let txt_color = if selected {
        theme::accent()
    } else if hovered {
        theme::text()
    } else {
        theme::subtext()
    };
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(14.0),
        txt_color,
    );
    painter.galley(
        egui::pos2(rect.left() + 50.0, rect.center().y - galley.size().y * 0.5),
        galley,
        txt_color,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(Color32::WHITE).strong())
            .fill(theme::accent())
            .rounding(Rounding::same(9.0))
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(theme::text()))
            .fill(theme::extreme())
            .rounding(Rounding::same(9.0))
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

fn theme_toggle(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    let hovered = resp.hovered();
    let fill = if hovered { theme::accent_soft() } else { theme::extreme() };
    ui.painter().rect_filled(rect, Rounding::same(9.0), fill);
    let color = if hovered { theme::accent() } else { theme::text() };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), color);
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    if hovered {
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
