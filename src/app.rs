//! The egui application: library grid, slideshow controls, settings, about.

use crate::config::{config_dir, Config, Mode};
use crate::theme;
use crate::thumbs::Thumbnailer;
use crate::wallpaper::{self, Fit};
use eframe::egui;
use egui::{Color32, ColorImage, RichText, Rounding, Stroke, TextureHandle, Vec2};
use rand::Rng;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const CELL_W: f32 = 176.0;
const GAP: f32 = 10.0;
const CARD_PAD: f32 = 8.0;
/// Below this content width (or on a phone/tablet) the layout switches to the
/// compact, touch-friendly single-pane mode with a bottom navigation bar.
const COMPACT_BREAK: f32 = 820.0;
/// How many images each album shows before a "load more" button appears.
/// Keeps very large folders from building thousands of card widgets per frame.
const PAGE_IMAGES: usize = 48;
/// Upper bound on decoded thumbnail textures kept in RAM/VRAM. Cards that
/// scroll far out of view are evicted (and re-requested when visible again) so
/// a 3000-image full-disk scan can't pin ~1.4 GB of textures.
const TEX_CAP: usize = 300;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Library,
    Slideshow,
    Online,
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
    HideToTray,
    ToggleSidebar,
    ToggleTheme,
    ExpandAllAlbums,
    CollapseAllAlbums,
    RetryPermissions,
    OpenPermissionPage,
    DismissPermissions,
    Quit,
    ReapplyCurrent,
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
    close_bg: bool,
    collapsed_folders: BTreeSet<String>,
    sidebar_hidden: bool,
    daily_online: bool,
    online_auto_set: bool,
    online_likes: i64,
    online_dislikes: i64,
    last_online_date: String,
    wallpaper_only: bool,
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
            close_bg: c.close_to_background,
            collapsed_folders: c.collapsed_folders.clone(),
            sidebar_hidden: c.sidebar_hidden,
            daily_online: c.daily_online,
            online_auto_set: c.online_auto_set,
            online_likes: c.online_likes,
            online_dislikes: c.online_dislikes,
            last_online_date: c.last_online_date.clone(),
            wallpaper_only: c.wallpaper_only,
        }
    }
}

pub struct App {
    cfg: Config,
    images: Vec<PathBuf>,
    /// Images grouped by their containing folder (folders = albums).
    albums: Vec<crate::albums::Album>,
    /// Folders the last scan could not read (used to prompt for permission).
    permission_issues: Vec<PathBuf>,
    current: Option<usize>,
    last_switch: Instant,
    status: String,
    tab: Tab,
    thumb: Thumbnailer,
    tex_cache: HashMap<PathBuf, Option<TextureHandle>>,
    /// Insertion order of `tex_cache` keys, used for bounded eviction.
    tex_order: Vec<PathBuf>,
    preview: Option<(PathBuf, TextureHandle)>,
    applied_theme: usize,
    hotkey: crate::hotkey::Hotkey,
    tray: Option<crate::tray::Tray>,
    scan_rx: Option<Receiver<Vec<PathBuf>>>,
    scanning: bool,
    window_hidden: bool,
    force_quit: bool,
    #[cfg(windows)]
    window_hwnd: isize,
    #[cfg(windows)]
    hidden_placement: Option<crate::winhide::Placement>,
    monitors: usize,
    primary_res: (i32, i32),
    // Online daily wallpaper (Bing) state.
    online_rx: Option<Receiver<Result<crate::online::OnlineImage, String>>>,
    online_loading: bool,
    online_img: Option<crate::online::OnlineImage>,
    online_tex: Option<TextureHandle>,
    online_idx: u32,
    online_boot_checked: bool,
    last_online_auto: Instant,
    wallpaper_managers: Vec<String>,
    /// Compact (phone/tablet or narrow window) layout with a bottom nav bar.
    compact: bool,
    /// Per-album display limit (pages of `PAGE_IMAGES`), grows via "load more".
    album_limits: HashMap<String, usize>,
    /// Active screen-size adaptation screenshot run (from `--shots <dir>`).
    shot: Option<crate::shot::Shooter>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = Config::load();
        theme::apply(&cc.egui_ctx, cfg.theme == 1);
        #[cfg(windows)]
        let window_hwnd = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            cc.window_handle()
                .ok()
                .and_then(|h| match h.as_raw() {
                    RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
                    _ => None,
                })
                .unwrap_or(0)
        };
        let mut app = Self {
            applied_theme: cfg.theme,
            cfg,
            images: Vec::new(),
            albums: Vec::new(),
            permission_issues: Vec::new(),
            current: None,
            last_switch: Instant::now(),
            status: String::new(),
            tab: Tab::Library,
            thumb: Thumbnailer::new(),
            tex_cache: HashMap::new(),
            tex_order: Vec::new(),
            preview: None,
            hotkey: crate::hotkey::start(cc.egui_ctx.clone()),
            tray: None,
            scan_rx: None,
            scanning: false,
            window_hidden: false,
            force_quit: false,
            #[cfg(windows)]
            window_hwnd,
            #[cfg(windows)]
            hidden_placement: None,
            monitors: wallpaper::monitor_count(),
            primary_res: wallpaper::primary_resolution(),
            online_rx: None,
            online_loading: false,
            online_img: None,
            online_tex: None,
            online_idx: 0,
            online_boot_checked: false,
            last_online_auto: Instant::now(),
            wallpaper_managers: wallpaper::conflicting_wallpaper_managers(),
            compact: false,
            album_limits: HashMap::new(),
            shot: crate::shot::Shooter::from_env(),
        };
        // Optional deep-link to a tab, e.g. `wallpaper-rs --tab settings`.
        if let Some(pos) = std::env::args().position(|a| a == "--tab") {
            if let Some(name) = std::env::args().nth(pos + 1) {
                app.tab = match name.to_ascii_lowercase().as_str() {
                    "slideshow" => Tab::Slideshow,
                    "online" => Tab::Online,
                    "settings" => Tab::Settings,
                    "about" => Tab::About,
                    _ => Tab::Library,
                };
            }
        }
        if let Some(sh) = app.shot.as_ref() {
            crate::shot::ensure_dir(&sh.dir);
        }
        // Install the system-tray icon (needed to restore the window after the
        // user hides it by clicking "X"). `None` means the platform has no tray;
        // in that case we fall back to the global-hotkey restore path.
        app.tray = crate::tray::Tray::install(cc.egui_ctx.clone());
        // Keep timers (slideshow / daily wallpaper) ticking even while hidden.
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
        self.albums.clear();
        self.permission_issues.clear();
        self.thumb.reset();
        self.tex_cache.clear();
        self.tex_order.clear();
        self.preview = None;
        self.current = None;
        if let Some(folder) = self.cfg.folder.clone() {
            if folder.exists() {
                let mut files = Vec::new();
                let mut denied = Vec::new();
                if crate::permissions::probe_dir(&folder)
                    == crate::permissions::PermissionState::Denied
                {
                    denied.push(folder.clone());
                }
                collect_images(&folder, self.cfg.recursive, &mut files, &mut denied);
                denied.sort();
                denied.dedup();
                files.sort();
                if self.cfg.wallpaper_only {
                    files.retain(|p| crate::thumbs::is_wallpaper_file(p));
                }
                self.images = files;
                self.albums = crate::albums::build_albums(&self.images, Some(&folder));
                self.permission_issues = denied;
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
        if !self.permission_issues.is_empty() {
            self.status = format!(
                "已载入 {} 张图片，但有 {} 个目录无访问权限",
                self.images.len(),
                self.permission_issues.len()
            );
        }
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
                Action::HideToTray => self.hide_to_tray(ctx),
                Action::ToggleSidebar => {
                    self.cfg.sidebar_hidden = !self.cfg.sidebar_hidden;
                }
                Action::ToggleTheme => {
                    self.cfg.theme = if self.cfg.theme == 1 { 0 } else { 1 };
                }
                Action::ExpandAllAlbums => self.cfg.collapsed_folders.clear(),
                Action::CollapseAllAlbums => {
                    self.cfg.collapsed_folders =
                        self.albums.iter().map(|a| a.key.clone()).collect();
                }
                Action::RetryPermissions => {
                    self.scan();
                    if self.permission_issues.is_empty() {
                        self.status = "权限已获取，图库已重新载入".to_string();
                    }
                }
                Action::OpenPermissionPage => {
                    // Reveal the denied folder itself: Explorer shows its
                    // "get access to this folder" confirmation dialog, which is
                    // the OS-native way to let the operator grant permission.
                    // On macOS this instead opens the Full Disk Access page.
                    let target = self.permission_issues.first().cloned();
                    crate::permissions::open_authorization(target.as_deref());
                }
                Action::DismissPermissions => self.permission_issues.clear(),
                Action::Quit => {
                    // Explicit quit always exits, even if hide-to-tray is on.
                    self.force_quit = true;
                    self.cfg.save();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::ReapplyCurrent => {
                    if let Some(i) = self.current {
                        // Re-apply the current wallpaper so a display-mode change
                        // takes effect immediately, without waiting for the next switch.
                        self.set_current(ctx, i);
                    }
                }
            }
        }
    }

    fn start_full_scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.scan_rx = Some(crate::diskscan::start(self.cfg.wallpaper_only));
        self.status = if self.cfg.wallpaper_only {
            "正在全盘扫描壁纸（仅载入 ≥1280×720 横屏）…".to_string()
        } else {
            "正在全盘扫描图片…（后台进行，完成后自动载入）".to_string()
        };
    }

    /// Hide the main window into the system tray while keeping the app running
    /// (slideshow / daily wallpaper continue in the background).
    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        // On Windows we keep the HWND "visible" to the OS but move it off-screen
        // and out of the taskbar. That preserves the egui event loop (a truly
        // hidden window stops receiving repaints, freezing rotation/timers).
        #[cfg(windows)]
        {
            if !self.window_hidden {
                if let Some(p) = unsafe { crate::winhide::hide(self.window_hwnd) } {
                    self.hidden_placement = Some(p);
                }
            }
        }
        #[cfg(not(windows))]
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        let _ = ctx;
        self.window_hidden = true;
        self.status = "已隐藏到系统托盘，点击托盘图标可唤回".to_string();
    }

    /// Restore and focus the main window (from the tray icon or global hotkey).
    fn show_window(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if let Some(p) = self.hidden_placement.take() {
                unsafe { crate::winhide::show(self.window_hwnd, &p) };
            }
        }
        #[cfg(not(windows))]
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        let _ = ctx;
        self.window_hidden = false;
    }

    /// Keep the thumbnail texture cache bounded so scanning thousands of images
    /// cannot exhaust VRAM/RAM. Evicted paths are forgotten by the thumbnailer
    /// so they are re-decoded if they scroll back into view.
    /// Handle an incoming screenshot result and advance the shot plan. Returns
    /// `true` when the whole run is finished (the app then closes).
    fn shot_pre(&mut self, ctx: &egui::Context) -> bool {
        if self.shot.is_none() {
            return false;
        }
        // Consume a screenshot from the previous frame's request.
        let img = if self.shot.as_ref().map(|s| s.pending).unwrap_or(false) {
            crate::shot::take_screenshot(ctx)
        } else {
            None
        };
        if let Some(img) = img {
            if let Some(sh) = self.shot.as_mut() {
                sh.store(&img);
            }
        }
        // Finished?
        let finished = self.shot.as_ref().map(|s| s.finished()).unwrap_or(false);
        if finished {
            let (n, dir) = self
                .shot
                .as_ref()
                .map(|s| (s.saved.len(), s.dir.display().to_string()))
                .unwrap_or((0, String::new()));
            if let Some(sh) = self.shot.as_ref() {
                sh.write_manifest();
            }
            self.status = format!("适配截图完成：{n} 张，已保存到 {dir}");
            self.force_quit = true;
            self.cfg.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return true;
        }
        // Apply the current item's tab and (once) its window size.
        let info = self.shot.as_ref().and_then(|s| {
            s.current().map(|it| (it.tab, it.w, it.h, s.settle, s.pending))
        });
        if let Some((tab, w, h, settle, pending)) = info {
            self.tab = tab_from_index(tab);
            if settle == 0 && !pending {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            }
        }
        false
    }

    /// Advance the settle timer and request the screenshot once the window has
    /// had time to resize and render.
    fn shot_post(&mut self, ctx: &egui::Context) {
        let Some(sh) = self.shot.as_mut() else {
            return;
        };
        if sh.pending || sh.finished() {
            return;
        }
        sh.settle += 1;
        if sh.settle >= crate::shot::SETTLE_FRAMES {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            sh.pending = true;
        }
    }

    fn enforce_tex_cap(&mut self) {
        let plan = evict_plan(&self.tex_order, TEX_CAP);
        if plan.is_empty() {
            return;
        }
        for k in &plan {
            self.tex_cache.remove(k);
            self.thumb.forget(k);
        }
        self.tex_order.retain(|k| self.tex_cache.contains_key(k));
    }

    // ---- Online daily wallpaper (Bing) ------------------------------------
    fn start_online_fetch(&mut self, idx: u32) {
        if self.online_loading {
            return;
        }
        self.online_loading = true;
        self.online_idx = idx;
        self.status = format!("正在获取在线壁纸（第 {} 张）…", idx + 1);
        let (tx, rx) = std::sync::mpsc::channel();
        self.online_rx = Some(rx);
        let cache = config_dir().join("online");
        std::thread::spawn(move || {
            let r = crate::online::fetch(idx, &cache);
            let _ = tx.send(r);
        });
    }

    fn apply_online_wallpaper(&mut self) {
        let (path, title) = match &self.online_img {
            Some(img) => (img.path.clone(), img.title.clone()),
            None => {
                self.status = "请先获取在线壁纸".to_string();
                return;
            }
        };
        match wallpaper::set_wallpaper(&path, self.cfg.fit) {
            Ok(_) => self.status = format!("已设为桌面壁纸：{title}"),
            Err(e) => self.status = format!("设置失败：{e}"),
        }
    }

    fn set_lock_from_current(&mut self) {
        let Some(path) = self.current_path() else {
            self.status = "请先选择一张壁纸".to_string();
            return;
        };
        match wallpaper::set_lock_screen(&path) {
            Ok(_) => self.status = "已设置锁屏壁纸".to_string(),
            Err(e) => self.status = format!("锁屏设置失败：{e}"),
        }
    }

    fn set_lock_from_online(&mut self) {
        let Some(img) = &self.online_img else {
            self.status = "请先获取在线壁纸".to_string();
            return;
        };
        let path = img.path.clone();
        match wallpaper::set_lock_screen(&path) {
            Ok(_) => self.status = "已设置锁屏壁纸".to_string(),
            Err(e) => self.status = format!("锁屏设置失败：{e}"),
        }
    }

    fn ui_online(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("每日壁纸 · 在线")
                    .size(20.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(8.0);
            chip(ui, "Bing · cn.bing.com", theme::accent_soft(), theme::accent());
        });
        ui.add_space(2.0);
        ui.label(
            RichText::new(
                "联网获取 Bing 每日壁纸（cn.bing.com，国内可直接访问）。可设为桌面 / 锁屏、赞或踩、切换上一张/下一张，并支持每日自动更换。",
            )
            .size(11.5)
            .color(theme::subtext()),
        );
        ui.add_space(12.0);

        if !self.wallpaper_managers.is_empty() {
            egui::Frame::none()
                .fill(theme::accent_soft())
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "⚠ 检测到可能在接管桌面壁纸的程序：{}。若点击「设为桌面壁纸」后桌面没有变化，请退出该程序（例如华硕 Armoury Crate 的 Aura 壁纸功能）后重试。",
                            self.wallpaper_managers.join("、")
                        ))
                        .size(11.5)
                        .color(theme::accent()),
                    );
                });
            ui.add_space(10.0);
        }

        // Toolbar ----------------------------------------------------------
        egui::Frame::none()
            .fill(theme::card())
            .stroke(Stroke::new(1.0_f32, theme::border()))
            .rounding(Rounding::same(14.0))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if primary_button(ui, "获取今日壁纸").clicked() {
                        self.start_online_fetch(0);
                    }
                    if ghost_button(ui, "‹ 上一张").clicked() {
                        let i = self.online_idx.saturating_sub(1);
                        self.start_online_fetch(i);
                    }
                    if ghost_button(ui, "下一张 ›").clicked() {
                        let i = (self.online_idx + 1).min(7);
                        self.start_online_fetch(i);
                    }
                    if self.online_loading {
                        chip(ui, "获取中…", theme::accent_soft(), theme::accent());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut d = self.cfg.daily_online;
                        if ui.checkbox(&mut d, "自动每日换壁纸").changed() {
                            self.cfg.daily_online = d;
                            if d {
                                self.online_boot_checked = true;
                                if self.online_img.is_none() {
                                    self.start_online_fetch(0);
                                }
                            }
                        }
                        let mut a = self.cfg.online_auto_set;
                        if ui.checkbox(&mut a, "获取后自动设为桌面").changed() {
                            self.cfg.online_auto_set = a;
                        }
                    });
                });
            });
        ui.add_space(12.0);

        // Preview + info card ---------------------------------------------
        let tex_id = self.online_tex.as_ref().map(|t| t.id());
        let tex_size = self.online_tex.as_ref().map(|t| t.size());
        let info = self.online_img.as_ref().map(|i| {
            (
                i.title.clone(),
                i.copyright.clone(),
                i.date.clone(),
                i.url.clone(),
            )
        });

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
            .inner_margin(egui::Margin::symmetric(16.0, 16.0))
            .show(ui, |ui| {
                let w = ui.available_width();
                let h = (ui.available_height() * 0.5).clamp(180.0, 420.0);
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
                {
                    let p = ui.painter();
                    p.rect_filled(rect, Rounding::same(10.0), theme::extreme());
                    match (tex_id, tex_size) {
                        (Some(id), Some(ts)) => {
                            let tw = ts[0] as f32;
                            let th = ts[1] as f32;
                            if tw > 0.0 && th > 0.0 {
                                let s = (rect.width() / tw).min(rect.height() / th);
                                let size = Vec2::new(tw * s, th * s);
                                let ir = egui::Rect::from_center_size(rect.center(), size);
                                p.image(
                                    id,
                                    ir,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    Color32::WHITE,
                                );
                            }
                        }
                        _ => {
                            let txt = if self.online_loading {
                                "正在联网获取…"
                            } else {
                                "点击「获取今日壁纸」开始"
                            };
                            p.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                txt,
                                egui::FontId::proportional(14.0),
                                theme::subtext(),
                            );
                        }
                    }
                }
                ui.add_space(12.0);

                // "了解有关此图片信息" info panel.
                egui::Frame::none()
                    .fill(theme::extreme())
                    .rounding(Rounding::same(10.0))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            chip(
                                ui,
                                "🔎 了解有关此图片信息",
                                theme::accent_soft(),
                                theme::accent(),
                            );
                            if let Some((title, _c, date, _u)) = &info {
                                ui.label(
                                    RichText::new(title).size(13.5).strong().color(theme::text()),
                                );
                                if !date.is_empty() {
                                    ui.label(
                                        RichText::new(date).size(11.0).color(theme::subtext()),
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new("暂无图片信息")
                                        .size(12.0)
                                        .color(theme::subtext()),
                                );
                            }
                        });
                        if let Some((_t, copyright, _d, _u)) = &info {
                            if !copyright.is_empty() {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(copyright)
                                        .size(11.0)
                                        .color(theme::subtext()),
                                );
                            }
                        }
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "设为桌面壁纸").clicked() {
                        self.apply_online_wallpaper();
                    }
                    if ghost_button(ui, "设为锁屏").clicked() {
                        self.set_lock_from_online();
                    }
                    if let Some((_t, _c, _d, url)) = &info {
                        if ghost_button(ui, "打开图片页面").clicked() {
                            open_url(url);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ghost_button(ui, &format!("👎 {}", self.cfg.online_dislikes)).clicked() {
                            self.cfg.online_dislikes += 1;
                            self.status = "已记录：不喜欢这张".to_string();
                        }
                        if ghost_button(ui, &format!("👍 {}", self.cfg.online_likes)).clicked() {
                            self.cfg.online_likes += 1;
                            self.status = "已记录：喜欢这张".to_string();
                        }
                    });
                });
            });
    }

    // ---- UI sections ------------------------------------------------------
    fn draw_header(&self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let (mx, my) = if self.compact { (12.0, 9.0) } else { (20.0, 13.0) };
        egui::TopBottomPanel::top("hdr")
            .frame(
                egui::Frame::none()
                    .fill(theme::surface())
                    .inner_margin(egui::Margin::symmetric(mx, my)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.horizontal(|ui| {
                    if !self.compact {
                        if ghost_button(ui, "☰").on_hover_text("显示 / 隐藏侧栏").clicked() {
                            actions.push(Action::ToggleSidebar);
                        }
                        ui.add_space(2.0);
                    }
                    // Brand mark
                    let logo_size = if self.compact { 34.0 } else { 40.0 };
                    let (logo, _) =
                        ui.allocate_exact_size(egui::vec2(logo_size, logo_size), egui::Sense::hover());
                    ui.painter().rect_filled(logo, Rounding::same(12.0), theme::accent());
                    ui.painter().text(
                        logo.center(),
                        egui::Align2::CENTER_CENTER,
                        "🖼",
                        egui::FontId::proportional(if self.compact { 16.0 } else { 20.0 }),
                        Color32::WHITE,
                    );
                    ui.add_space(if self.compact { 8.0 } else { 2.0 });
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Wallpaper Studio")
                                .size(if self.compact { 14.5 } else { 18.0 })
                                .strong()
                                .color(theme::text()),
                        );
                        if !self.compact {
                            ui.label(
                                RichText::new("桌面壁纸 · 固定与轮播")
                                    .size(11.0)
                                    .color(theme::subtext()),
                            );
                        }
                    });
                    ui.add_space(if self.compact { 6.0 } else { 10.0 });
                    let mode = if self.cfg.mode == Mode::Slideshow { "轮播" } else { "固定" };
                    chip(ui, mode, theme::accent_soft(), theme::accent());
                    if !self.compact && self.cfg.mode == Mode::Slideshow {
                        if self.cfg.running {
                            chip(ui, "● 运行中", theme::accent_soft(), theme::ok());
                        } else {
                            chip(ui, "已暂停", theme::extreme(), theme::subtext());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.compact {
                            // Theme toggle lives in the header on mobile (the
                            // sidebar, where it normally sits, is hidden).
                            let dark = self.cfg.theme == 1;
                            let lbl = if dark { "☀" } else { "🌙" };
                            let btn = ui.add(
                                egui::Button::new(RichText::new(lbl).size(16.0))
                                    .frame(false)
                                    .min_size(egui::vec2(38.0, 34.0)),
                            );
                            if btn.on_hover_text("切换浅色 / 深色主题").clicked() {
                                actions.push(Action::ToggleTheme);
                            }
                        } else if let Some(p) = self.current_path() {
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
        if self.cfg.sidebar_hidden {
            return;
        }
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
                if nav_button(ui, "🌐", "每日壁纸", self.tab == Tab::Online) {
                    self.tab = Tab::Online;
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

    /// Compact bottom navigation bar (phones/tablets, narrow windows).
    fn draw_bottom_nav(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("mobilenav")
            .exact_height(66.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::surface())
                    .inner_margin(egui::Margin::symmetric(6.0, 6.0)),
            )
            .show(ctx, |ui| {
                // hairline on top edge
                let r = ui.max_rect();
                ui.painter().line_segment(
                    [r.left_top(), r.right_top()],
                    Stroke::new(1.0_f32, theme::border()),
                );
                let items: [(Tab, &str, &str); 5] = [
                    (Tab::Library, "🖼", "壁纸库"),
                    (Tab::Slideshow, "🎞", "轮播"),
                    (Tab::Online, "🌐", "每日"),
                    (Tab::Settings, "⚙", "设置"),
                    (Tab::About, "ℹ", "关于"),
                ];
                let w = ui.available_width() / items.len() as f32;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (tab, icon, label) in items {
                        if bottom_nav_button(ui, icon, label, self.tab == tab, w) {
                            self.tab = tab;
                        }
                    }
                });
            });
    }

    fn draw_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::card())
                    .inner_margin(egui::Margin::symmetric(18.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(12.0).color(theme::subtext()));
                    if !self.compact {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let hint = if self.hotkey.is_registered() {
                                format!("{} 后台 / 唤回", self.hotkey.label())
                            } else {
                                "全局快捷键不可用".to_string()
                            };
                            ui.label(RichText::new(hint).size(11.0).color(theme::subtext()));
                        });
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
                    Tab::Online => {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.ui_online(ui));
                    }
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
                let folder_hint = if self.compact { 30 } else { 54 };
                // Folder path chip (own row on compact, inline on desktop).
                if self.compact {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("📁").size(15.0).color(theme::accent()));
                        folder_chip(ui, &ellipsis(&folder, folder_hint));
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
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
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📁").size(15.0).color(theme::accent()));
                        folder_chip(ui, &ellipsis(&folder, folder_hint));
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
                }
                ui.add_space(11.0);
                // Filters + search. On compact screens the checkboxes, the
                // search field and the count chip each get their own row so
                // nothing overflows the narrow viewport.
                let only_label = if self.compact { "仅壁纸尺寸" } else { "仅壁纸尺寸(≥1280×720)" };
                let search_w = if self.compact { ui.available_width() - 24.0 } else { 210.0 };
                if self.compact {
                    ui.horizontal_wrapped(|ui| {
                        let mut rc = self.cfg.recursive;
                        if ui.checkbox(&mut rc, "包含子文件夹").changed() {
                            self.cfg.recursive = rc;
                            actions.push(Action::Rescan);
                        }
                        ui.add_space(8.0);
                        let mut wo = self.cfg.wallpaper_only;
                        if ui.checkbox(&mut wo, only_label).changed() {
                            self.cfg.wallpaper_only = wo;
                            actions.push(Action::Rescan);
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍").size(13.0).color(theme::subtext()));
                        search_field(ui, &mut self.cfg.search, search_w.max(120.0));
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        chip(
                            ui,
                            &format!("共 {total} 张"),
                            theme::accent_soft(),
                            theme::accent(),
                        );
                        if self.scanning {
                            chip(ui, "扫描中…", theme::accent_soft(), theme::accent());
                        }
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        let mut rc = self.cfg.recursive;
                        if ui.checkbox(&mut rc, "包含子文件夹").changed() {
                            self.cfg.recursive = rc;
                            actions.push(Action::Rescan);
                        }
                        ui.add_space(8.0);
                        let mut wo = self.cfg.wallpaper_only;
                        if ui
                            .checkbox(&mut wo, only_label)
                            .on_hover_text("过滤掉图标、小图、竖图等不适合做壁纸的图片")
                            .changed()
                        {
                            self.cfg.wallpaper_only = wo;
                            actions.push(Action::Rescan);
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("🔍").size(13.0).color(theme::subtext()));
                        search_field(ui, &mut self.cfg.search, search_w);
                        ui.add_space(8.0);
                        chip(
                            ui,
                            &format!("共 {total} 张"),
                            theme::accent_soft(),
                            theme::accent(),
                        );
                        if self.scanning {
                            chip(ui, "扫描中…", theme::accent_soft(), theme::accent());
                        }
                    });
                }
            });
        ui.add_space(12.0);

        // Permission banner: folders the last scan couldn't read.
        if !self.permission_issues.is_empty() {
            self.ui_permission_banner(ui, actions);
            ui.add_space(12.0);
        }

        if self.images.is_empty() {
            self.empty_state(ui, actions);
            return;
        }

        let q = self.cfg.search.trim().to_lowercase();
        let matches = |p: &PathBuf| {
            q.is_empty()
                || p.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().contains(&q))
                    .unwrap_or(false)
        };

        // Toolbar: album count + expand/collapse all.
        let album_label =
            format!("{} 个相册 · 按文件夹分类", self.albums.len());
        if self.compact {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(album_label).size(12.0).color(theme::subtext()));
                if ghost_button(ui, "全部折叠").clicked() {
                    actions.push(Action::CollapseAllAlbums);
                }
                if ghost_button(ui, "全部展开").clicked() {
                    actions.push(Action::ExpandAllAlbums);
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(album_label).size(12.0).color(theme::subtext()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ghost_button(ui, "全部折叠").clicked() {
                        actions.push(Action::CollapseAllAlbums);
                    }
                    if ghost_button(ui, "全部展开").clicked() {
                        actions.push(Action::ExpandAllAlbums);
                    }
                });
            });
        }
        ui.add_space(8.0);

        // Snapshot the albums to a local list so we can call `draw_card(&mut self)`
        // while iterating without holding a borrow on `self.albums`.
        // Each entry: (key, name, images, collapsed, display_limit).
        let mut sections: Vec<(String, String, Vec<PathBuf>, bool, usize)> = Vec::new();
        for a in &self.albums {
            let imgs: Vec<PathBuf> = a.images.iter().filter(|p| matches(p)).cloned().collect();
            if imgs.is_empty() && !q.is_empty() {
                continue;
            }
            let collapsed = self.cfg.collapsed_folders.contains(&a.key);
            let limit = *self.album_limits.get(&a.key).unwrap_or(&PAGE_IMAGES);
            sections.push((a.key.clone(), a.name.clone(), imgs, collapsed, limit));
        }

        let current_path = self.current_path();
        let avail = ui.available_width();
        // Fluid grid: 2–3 columns on compact screens, fixed cells on desktop.
        let (cols, cell_w) = if self.compact {
            let n = if avail < 470.0 { 2 } else { 3 };
            let cw = ((avail - GAP * (n as f32 - 1.0)) / n as f32 - CARD_PAD * 2.0).max(120.0);
            (n, cw)
        } else {
            let item_w = CELL_W + CARD_PAD * 2.0;
            let n = (((avail + GAP) / (item_w + GAP)).floor() as usize).max(1);
            (n, CELL_W)
        };
        let mut toggles: Vec<String> = Vec::new();
        let mut load_more: Vec<String> = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if sections.is_empty() {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(if q.is_empty() {
                            "暂无相册"
                        } else {
                            "没有匹配的图片"
                        })
                        .size(13.0)
                        .color(theme::subtext()),
                    );
                    return;
                }
                for (key, name, imgs, collapsed, limit) in &sections {
                    let arrow = if *collapsed { "▶" } else { "▼" };
                    ui.horizontal(|ui| {
                        let label = RichText::new(format!("{arrow}  📁  {name}"))
                            .size(13.5)
                            .color(theme::text());
                        let resp = ui.add(
                            egui::Button::new(label)
                                .frame(false)
                                .min_size(egui::vec2(0.0, 26.0)),
                        );
                        if resp.clicked() {
                            toggles.push(key.clone());
                        }
                        chip(
                            ui,
                            &format!("{} 张", imgs.len()),
                            theme::accent_soft(),
                            theme::accent(),
                        );
                    });
                    if !*collapsed {
                        ui.add_space(4.0);
                        // Only build up to `limit` cards; large folders page in.
                        let shown = (*limit).min(imgs.len());
                        let mut idx = 0usize;
                        while idx < shown {
                            ui.horizontal(|ui| {
                                for _ in 0..cols {
                                    if idx >= shown {
                                        break;
                                    }
                                    let path = imgs[idx].clone();
                                    self.draw_card(ui, &path, current_path.as_deref(), actions, cell_w);
                                    idx += 1;
                                }
                            });
                            ui.add_space(GAP);
                        }
                        if imgs.len() > shown {
                            let remain = imgs.len() - shown;
                            ui.add_space(2.0);
                            if ghost_button(ui, &format!("加载更多（还有 {remain} 张）")).clicked() {
                                load_more.push(key.clone());
                            }
                        }
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                }
            });

        // Apply collapse toggles requested while iterating.
        for k in toggles {
            if !self.cfg.collapsed_folders.remove(&k) {
                self.cfg.collapsed_folders.insert(k);
            }
        }
        // Grow the page for albums whose "load more" button was clicked.
        for k in load_more {
            let e = self.album_limits.entry(k).or_insert(PAGE_IMAGES);
            *e += PAGE_IMAGES;
        }
    }

    /// Banner shown when a scan hits folders it cannot read. Offers to retry
    /// (triggers the OS prompt where applicable) or jump to the authorization
    /// page / reveal the folder (platform-specific).
    fn ui_permission_banner(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let g = crate::permissions::guidance();
        let count = self.permission_issues.len();
        egui::Frame::none()
            .fill(theme::accent_soft())
            .stroke(Stroke::new(1.0_f32, theme::accent()))
            .rounding(Rounding::same(12.0))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔒").size(16.0));
                    ui.label(
                        RichText::new(format!("{count} 个目录无访问权限 · {}", g.title))
                            .size(13.5)
                            .strong()
                            .color(theme::text()),
                    );
                });
                ui.add_space(4.0);
                for p in self.permission_issues.iter().take(3) {
                    ui.label(
                        RichText::new(format!("• {}", ellipsis(&p.display().to_string(), 80)))
                            .size(11.0)
                            .color(theme::subtext()),
                    );
                }
                if count > 3 {
                    ui.label(
                        RichText::new(format!("… 其余 {} 个目录", count - 3))
                            .size(11.0)
                            .color(theme::subtext()),
                    );
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(g.detail)
                        .size(11.5)
                        .color(theme::subtext()),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "请求权限 / 重试").clicked() {
                        actions.push(Action::RetryPermissions);
                    }
                    if ghost_button(ui, g.action).clicked() {
                        actions.push(Action::OpenPermissionPage);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ghost_button(ui, "忽略").clicked() {
                            actions.push(Action::DismissPermissions);
                        }
                    });
                });
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
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        current: Option<&Path>,
        actions: &mut Vec<Action>,
        cell_w: f32,
    ) {
        let img_h = (cell_w * 0.58).round().max(70.0);
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
                    ui.set_min_width(cell_w);
                    ui.set_max_width(cell_w);
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(cell_w, img_h), egui::Sense::click());
                    // Lazy-load: only decode thumbnails for cards actually on
                    // screen (or about to be), not all 3000 scanned files.
                    if ui.is_rect_visible(rect)
                        && !self.tex_cache.contains_key(path)
                        && !self.thumb.is_requested(path)
                    {
                        self.thumb.request(path);
                    }
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
                    if segment(ui, self.cfg.mode == Mode::Fixed, "固定壁纸") {
                        self.cfg.mode = Mode::Fixed;
                    }
                    if segment(ui, self.cfg.mode == Mode::Slideshow, "轮播") {
                        self.cfg.mode = Mode::Slideshow;
                    }
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
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("固定模式：在「壁纸库」点击任意图片即可设为桌面壁纸。")
                            .size(12.0)
                            .color(theme::subtext()),
                    );
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
                        let before_fit = self.cfg.fit;
                        let fit_text = if self.compact {
                            self.cfg.fit.short()
                        } else {
                            self.cfg.fit.label()
                        };
                        egui::ComboBox::from_id_salt("fit_combo")
                            .selected_text(fit_text)
                            .width(if self.compact { 116.0 } else { 200.0 })
                            .show_ui(ui, |ui| {
                                for f in Fit::all() {
                                    let t = if self.compact { f.short() } else { f.label() };
                                    ui.selectable_value(&mut self.cfg.fit, f, t);
                                }
                            });
                        if self.cfg.fit != before_fit {
                            // Apply immediately to the current wallpaper.
                            actions.push(Action::ReapplyCurrent);
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "主题", "浅色 / 深色，即时生效");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if segment(ui, self.cfg.theme == 1, "深色") {
                            self.cfg.theme = 1;
                        }
                        if segment(ui, self.cfg.theme == 0, "浅色") {
                            self.cfg.theme = 0;
                        }
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
                // Autostart is desktop-only (login items / Run key / .desktop).
                if !crate::platform::is_mobile() {
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
                }
            });
        ui.add_space(12.0);

        // Resident & hotkey -------------------------------------------------
        if !crate::platform::is_mobile() {
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
                section_header(ui, "托盘与快捷键");
                ui.add_space(4.0);
                ui.label(
                    RichText::new("关闭窗口（右上角 ×）后会隐藏到系统托盘：Windows 右下角通知区域 / macOS 菜单栏 / Linux 状态区，程序继续在后台运行。单击托盘图标可重新打开窗口，右键菜单可退出。")
                        .size(11.5)
                        .color(theme::subtext()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let desc = if self.tray.is_some() {
                        "托盘图标已就绪"
                    } else {
                        "当前环境无系统托盘，将改用全局快捷键隐藏 / 唤回"
                    };
                    setting_label(ui, "系统托盘", desc);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.tray.is_some() {
                            chip(ui, "可用", theme::accent_soft(), theme::ok());
                        } else {
                            chip(ui, "不可用", theme::extreme(), theme::subtext());
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    setting_label(ui, "关闭时隐藏到托盘", "关闭窗口后隐藏到通知区域，轮播 / 每日壁纸继续运行");
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
                        format!("{}：显示 / 隐藏窗口", self.hotkey.label())
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
                    setting_label(ui, "隐藏 / 退出", "隐藏到托盘继续运行，或彻底退出应用");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ghost_button(ui, "退出应用").clicked() {
                            actions.push(Action::Quit);
                        }
                        if ghost_button(ui, "隐藏到托盘").clicked() {
                            actions.push(Action::HideToTray);
                        }
                    });
                });
            });
        ui.add_space(12.0);
        }

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
                let res_txt = if pw > 0 && ph > 0 {
                    format!("主屏 {}×{}", pw, ph)
                } else {
                    "主屏分辨率未知".to_string()
                };
                ui.label(
                    RichText::new(format!(
                        "检测到 {} 台显示器 · {} · 当前主题 {}",
                        self.monitors,
                        res_txt,
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

        // Lock screen -------------------------------------------------------
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
                section_header(ui, "锁屏壁纸");
                ui.add_space(4.0);
                ui.label(
                    RichText::new("把当前壁纸同时设为系统锁屏。Windows 通过 PersonalizationCSP（需以管理员身份运行）；Ubuntu / GNOME 通过 gsettings 设置锁屏 picture-uri；macOS 锁屏跟随系统壁纸。")
                        .size(11.5)
                        .color(theme::subtext()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "将当前壁纸设为锁屏").clicked() {
                        self.set_lock_from_current();
                    }
                    ui.label(
                        RichText::new("当前壁纸会同步应用到锁屏")
                            .size(11.0)
                            .color(theme::subtext()),
                    );
                });
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
                RichText::new("跨平台壁纸工具：Windows / macOS / Linux 桌面 + iOS / Android 自适应界面，支持固定壁纸与轮播。")
                    .size(13.0)
                    .color(theme::subtext()),
            );
            ui.add_space(6.0);
            let cap = crate::platform::wallpaper_support();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("当前平台：{}", crate::platform::name()))
                        .size(11.5)
                        .color(theme::subtext()),
                );
                let (fill, col, txt) = if cap.can_set {
                    (theme::accent_soft(), theme::ok(), format!("{} · 可设置", cap.label))
                } else {
                    (theme::extreme(), theme::subtext(), format!("{} · 仅浏览", cap.label))
                };
                chip(ui, &txt, fill, col);
            });
            ui.label(RichText::new(cap.note).size(11.0).color(theme::subtext()));
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
                section_header(ui, "下载（各平台）");
                ui.add_space(10.0);
                ui.label(
                    RichText::new("下载最新版安装包（点击「下载」在浏览器打开）：")
                        .size(12.0)
                        .color(theme::text()),
                );
                ui.add_space(6.0);
                let links = [
                    (
                        "Windows (x64) · .zip",
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-windows-x64.zip",
                    ),
                    (
                        "macOS (Universal) · .tar.gz",
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-macos-universal.tar.gz",
                    ),
                    (
                        "Linux / Ubuntu (x64) · .tar.gz",
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-linux-x64.tar.gz",
                    ),
                    (
                        "全部版本 · Release 页面",
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest",
                    ),
                ];
                for (label, url) in links {
                    ui.horizontal(|ui| {
                        if ghost_button(ui, "⬇ 下载").clicked() {
                            open_url(url);
                        }
                        ui.label(RichText::new(label).size(12.0).color(theme::accent()));
                        ui.label(RichText::new(url).size(10.0).color(theme::subtext()));
                    });
                    ui.add_space(4.0);
                }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_theme != self.cfg.theme {
            theme::apply(ctx, self.cfg.theme == 1);
            self.applied_theme = self.cfg.theme;
        }

        // Screen-size adaptation screenshot mode (`--shots <dir>`): drive the
        // resize / capture state machine every frame and keep repainting.
        if self.shot.is_some() {
            ctx.request_repaint();
            if self.shot_pre(ctx) {
                return;
            }
        }

        // System tray: react to menu / click actions queued by the tray handlers.
        let tray_actions = self.tray.as_ref().map(|t| t.drain()).unwrap_or_default();
        for action in tray_actions {
            match action {
                crate::tray::TrayAction::Show => self.show_window(ctx),
                crate::tray::TrayAction::Hide => self.hide_to_tray(ctx),
                crate::tray::TrayAction::Quit => {
                    self.force_quit = true;
                    self.cfg.save();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // Global hotkey: show / hide the window (bonus shortcut, tray optional).
        if self.hotkey.take_toggle() {
            if self.window_hidden {
                self.show_window(ctx);
                self.status = "已唤回窗口".to_string();
            } else {
                self.hide_to_tray(ctx);
            }
        }

        // "X" close -> hide to tray instead of quitting (unless the user chose
        // Quit). We can swallow the close as long as there is a way to bring the
        // window back: the system tray, or the global hotkey as a fallback.
        let can_restore = self.tray.is_some() || self.hotkey.is_registered();
        if should_background_on_close(
            ctx.input(|i| i.viewport().close_requested()),
            self.cfg.close_to_background,
            can_restore,
            self.force_quit,
        ) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_to_tray(ctx);
        }

        // Poll background full-disk scan.
        let scan_result = self.scan_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(found) = scan_result {
            self.scan_rx = None;
            self.scanning = false;
            self.thumb.reset();
            self.tex_cache.clear();
            self.tex_order.clear();
            self.preview = None;
            self.images = found;
            self.albums = crate::albums::build_albums(&self.images, None);
            self.permission_issues.clear();
            self.current = if self.images.is_empty() { None } else { Some(0) };
            self.status = if self.images.is_empty() {
                "全盘扫描完成，但未找到符合条件的壁纸".to_string()
            } else {
                format!(
                    "全盘扫描完成：符合壁纸要求 {} 张{}",
                    self.images.len(),
                    if self.cfg.wallpaper_only {
                        "（已过滤图标/小图/竖图等）"
                    } else {
                        ""
                    }
                )
            };
            self.last_switch = Instant::now();
        }

        // Poll background online-wallpaper fetch.
        let online_result = self.online_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(res) = online_result {
            self.online_rx = None;
            self.online_loading = false;
            match res {
                Ok(img) => {
                    if let Some(t) = crate::thumbs::load_image(&img.path, 1400) {
                        let ci = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba);
                        let tex = ctx.load_texture(
                            format!("online:{}", img.path.display()),
                            ci,
                            egui::TextureOptions::LINEAR,
                        );
                        self.online_tex = Some(tex);
                    } else {
                        self.online_tex = None;
                    }
                    self.cfg.last_online_date = img.date.clone();
                    self.status = format!("已获取在线壁纸：{}", img.title);
                    self.online_img = Some(img);
                    if self.cfg.online_auto_set {
                        self.apply_online_wallpaper();
                    }
                }
                Err(e) => {
                    self.status = format!("在线壁纸获取失败：{e}");
                }
            }
        }

        // Daily online wallpaper: bootstrap on launch, then refresh every 24h.
        if self.cfg.daily_online && !self.online_loading {
            if !self.online_boot_checked {
                self.online_boot_checked = true;
                self.start_online_fetch(0);
            } else if self.last_online_auto.elapsed() >= Duration::from_secs(86_400) {
                self.last_online_auto = Instant::now();
                self.start_online_fetch(0);
            }
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
                    if !self.tex_cache.contains_key(&p) {
                        self.tex_order.push(p.clone());
                    }
                    self.tex_cache.insert(p, Some(tex));
                }
                None => {
                    if !self.tex_cache.contains_key(&p) {
                        self.tex_order.push(p.clone());
                    }
                    self.tex_cache.insert(p, None);
                }
            }
        }
        self.enforce_tex_cap();

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

        // Responsive: use the compact, touch-friendly layout on phones/tablets
        // (or a narrow desktop window), and the sidebar layout otherwise.
        self.compact =
            crate::platform::is_mobile() || ctx.screen_rect().width() < COMPACT_BREAK;
        let (touch_h, pad) = if self.compact {
            (46.0, egui::vec2(16.0, 10.0))
        } else {
            (34.0, egui::vec2(15.0, 8.0))
        };
        ctx.style_mut(|s| {
            s.spacing.interact_size.y = touch_h;
            s.spacing.button_padding = pad;
        });

        let mut actions: Vec<Action> = Vec::new();
        self.draw_header(ctx, &mut actions);
        if self.compact {
            self.draw_bottom_nav(ctx);
        } else {
            self.draw_sidebar(ctx);
        }
        self.draw_status_bar(ctx);
        self.draw_center(ctx, &mut actions);
        self.apply(ctx, actions);

        let snap2 = Snapshot::of(&self.cfg);
        if snap != snap2 {
            self.cfg.save();
        }

        self.shot_post(ctx);
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

/// Rounded chip used for the selected folder path.
fn folder_chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(theme::extreme())
        .rounding(Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(11.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.5).color(theme::text()));
        });
}

/// Rounded search box used on the library toolbar.
fn search_field(ui: &mut egui::Ui, text: &mut String, width: f32) {
    egui::Frame::none()
        .fill(theme::extreme())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .rounding(Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(9.0, 4.0))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .hint_text("按文件名过滤…")
                    .desired_width(width)
                    .frame(false),
            );
        });
}

fn tab_from_index(i: usize) -> Tab {
    match i {
        0 => Tab::Library,
        1 => Tab::Slideshow,
        2 => Tab::Online,
        3 => Tab::Settings,
        _ => Tab::About,
    }
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

/// A single bottom-nav item: stacked icon + label with a soft selected pill.
fn bottom_nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool, width: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 54.0), egui::Sense::click());
    let hovered = resp.hovered();
    if selected {
        ui.painter()
            .rect_filled(rect.shrink2(egui::vec2(6.0, 4.0)), Rounding::same(12.0), theme::accent_soft());
    } else if hovered {
        ui.painter()
            .rect_filled(rect.shrink2(egui::vec2(6.0, 4.0)), Rounding::same(12.0), theme::extreme());
    }
    let color = if selected { theme::accent() } else { theme::subtext() };
    let center = rect.center();
    ui.painter().text(
        egui::pos2(center.x, center.y - 10.0),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(18.0),
        color,
    );
    ui.painter().text(
        egui::pos2(center.x, center.y + 12.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        color,
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

/// A small pill-style segmented option (e.g. 浅色 / 深色), themed to the palette.
fn segment(ui: &mut egui::Ui, selected: bool, text: &str) -> bool {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(12.5), theme::text());
    let textw = (galley.size().x + 26.0).max(40.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(textw, 28.0), egui::Sense::click());
    let fill = if selected {
        theme::accent_soft()
    } else if resp.hovered() {
        theme::extreme()
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, Rounding::same(9.0), fill);
    }
    let color = if selected { theme::accent() } else { theme::subtext() };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.5),
        color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn collect_images(
    dir: &Path,
    recursive: bool,
    out: &mut Vec<PathBuf>,
    denied: &mut Vec<PathBuf>,
) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                denied.push(dir.to_path_buf());
            }
            return;
        }
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if recursive {
                collect_images(&p, recursive, out, denied);
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

/// Open a URL in the system default browser.
fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Decide whether a window-close should be swallowed into the tray.
/// `can_restore` means there is some way to bring the window back (system tray
/// or global hotkey); if not, and the user explicitly quit, we fall through to a
/// real exit so the app can never get stuck un-closable and unrecoverable.
fn should_background_on_close(
    close_requested: bool,
    close_to_background: bool,
    can_restore: bool,
    force_quit: bool,
) -> bool {
    close_requested && close_to_background && can_restore && !force_quit
}

/// Return the oldest keys that must be dropped so `order` fits within `cap`.
fn evict_plan(order: &[PathBuf], cap: usize) -> Vec<PathBuf> {
    if order.len() <= cap {
        return Vec::new();
    }
    order[..order.len() - cap].to_vec()
}

#[cfg(test)]
mod tests {
    use super::{evict_plan, should_background_on_close};
    use std::path::PathBuf;

    #[test]
    fn close_logic_matrix() {
        // Normal close with everything enabled -> background.
        assert!(should_background_on_close(true, true, true, false));
        // Explicit quit always exits.
        assert!(!should_background_on_close(true, true, true, true));
        // No way to restore the window -> must exit.
        assert!(!should_background_on_close(true, true, false, false));
        // Setting off -> exit.
        assert!(!should_background_on_close(true, false, true, false));
        // No close request -> nothing to do.
        assert!(!should_background_on_close(false, true, true, false));
    }

    #[test]
    fn eviction_is_bounded_and_oldest_first() {
        let order: Vec<PathBuf> = (0..10).map(|i| PathBuf::from(format!("{i}.png"))).collect();
        // Under cap -> nothing evicted.
        assert!(evict_plan(&order, 10).is_empty());
        assert!(evict_plan(&order, 20).is_empty());
        // Over cap -> drop the oldest (len - cap) entries.
        let plan = evict_plan(&order, 7);
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0], PathBuf::from("0.png"));
        assert_eq!(plan[2], PathBuf::from("2.png"));
    }
}
