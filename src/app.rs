//! The egui application: library grid, slideshow controls, settings, about.

use crate::config::{config_dir, Config, Mode}; // 引入依赖
use crate::theme; // 引入依赖
use crate::thumbs::Thumbnailer; // 引入依赖
use crate::wallpaper::{self, Fit}; // 引入依赖
use eframe::egui; // 引入依赖
use egui::{Color32, ColorImage, RichText, Rounding, Stroke, TextureHandle, Vec2}; // 引入依赖
use rand::Rng; // 引入依赖
use std::collections::{BTreeSet, HashMap}; // 引入依赖
use std::path::{Path, PathBuf}; // 引入依赖
use std::sync::mpsc::Receiver; // 引入依赖
use std::time::{Duration, Instant}; // 引入依赖

const CELL_W: f32 = 176.0; // 常量 CELL_W
const GAP: f32 = 10.0; // 常量 GAP
const CARD_PAD: f32 = 8.0; // 常量 CARD_PAD
/// Below this content width (or on a phone/tablet) the layout switches to the
/// compact, touch-friendly single-pane mode with a bottom navigation bar.
const COMPACT_BREAK: f32 = 820.0; // 常量 COMPACT_BREAK
/// 「窄桌面」断点：常见笔记本（1366×768 / 1280×720，或高 DPI 缩放后的较小逻辑
/// 宽度）低于此宽度时，收紧侧栏宽度与页面留白，把更多空间让给内容区——但仍保留
/// 桌面式侧栏（不进入手机/平板紧凑单栏）。
const NARROW_BREAK: f32 = 1400.0; // 常量 NARROW_BREAK
/// How many images each album shows before a "load more" button appears.
/// Keeps very large folders from building thousands of card widgets per frame.
const PAGE_IMAGES: usize = 48; // 常量 PAGE_IMAGES
/// Upper bound on decoded thumbnail textures kept in RAM/VRAM. Cards that
/// scroll far out of view are evicted (and re-requested when visible again) so
/// a 3000-image full-disk scan can't pin ~1.4 GB of textures.
const TEX_CAP: usize = 300; // 常量 TEX_CAP

#[derive(PartialEq, Eq, Clone, Copy)] // 语句
enum Tab { // 枚举 Tab
    Web4k, // 语句
    Library, // 语句
    Slideshow, // 语句
    Online, // 语句
    Settings, // 语句
    About, // 语句
} // 块结束

/// 网上壁纸库中，原图下载完成后要执行的动作。
#[derive(Clone, Copy, PartialEq, Eq)] // 语句
enum WebApply { // 枚举 WebApply
    Wallpaper, // 语句
    Lock, // 语句
    Save, // 语句
} // 块结束

/// 网上壁纸库中，动态壁纸（原视频 mp4）下载完成后要执行的动作。
#[derive(Clone, Copy, PartialEq, Eq, Default)] // 语句
enum WebVideoAfter { // 枚举 WebVideoAfter
    /// 仅下载保存到「图片/网上壁纸库/<站点>」。
    #[default] // 语句
    Save, // 语句
    /// 下载后立即「设为动态壁纸」。
    Live, // 语句
    /// 下载后「加入轮播清单」：轮播轮到该条目时以原视频播放动态壁纸。
    Playlist, // 语句
    /// 下载后「设为锁屏」：从视频截取一帧作为锁屏海报图。
    Lock, // 语句
} // 块结束

/// mpv 缺失时按需下载完成后要执行的动作（桌面动态壁纸宿主依赖 mpv）。
#[derive(Clone)] // 语句
enum AfterMpv { // 枚举 AfterMpv
    /// 下载完成后把该视频设为动态壁纸。
    LiveWallpaper(PathBuf), // 语句
    /// 下载完成后从该视频截帧设为锁屏。
    LockVideo(PathBuf), // 语句
} // 块结束

enum Action { // 枚举 Action
    SetPath(PathBuf), // 调用 SetPath
    Next, // 语句
    Prev, // 语句
    Random, // 语句
    Rescan, // 语句
    PickFolder, // 语句
    OpenFolder, // 语句
    OpenConfig, // 语句
    Save, // 语句
    ScanAllDisks, // 语句
    HideToTray, // 语句
    ToggleSidebar, // 语句
    ToggleTheme, // 语句
    ExpandAllAlbums, // 语句
    CollapseAllAlbums, // 语句
    RetryPermissions, // 语句
    OpenPermissionPage, // 语句
    DismissPermissions, // 语句
    Quit, // 语句
    ReapplyCurrent, // 语句
    /// 把视频设为锁屏：Windows 锁屏只能设置静态图片，先截取一帧再设置。
    LockVideo(PathBuf), // 语句
} // 块结束

/// Snapshot of the persisted config, used to detect changes and autosave.
#[derive(PartialEq)] // 语句
struct Snapshot { // 结构体 Snapshot
    mode: Mode, // 字段 mode
    interval: u64, // 字段 interval
    shuffle: bool, // 字段 shuffle
    fit: Fit, // 字段 fit
    theme: usize, // 字段 theme
    recursive: bool, // 字段 recursive
    running: bool, // 字段 running
    search: String, // 字段 search
    folder: Option<PathBuf>, // 字段 folder
    last_path: Option<PathBuf>, // 字段 last_path
    close_bg: bool, // 字段 close_bg
    collapsed_folders: BTreeSet<String>, // 字段 collapsed_folders
    sidebar_hidden: bool, // 字段 sidebar_hidden
    daily_online: bool, // 字段 daily_online
    online_auto_set: bool, // 字段 online_auto_set
    online_likes: i64, // 字段 online_likes
    online_dislikes: i64, // 字段 online_dislikes
    last_online_date: String, // 字段 last_online_date
    wallpaper_only: bool, // 字段 wallpaper_only
    slideshow: Vec<PathBuf>, // 字段 slideshow
} // 块结束

impl Snapshot { // 实现
    fn of(c: &Config) -> Self { // 函数 of
        Self { // 语句
            mode: c.mode, // 字段 mode
            interval: c.interval_secs, // 字段 interval
            shuffle: c.shuffle, // 字段 shuffle
            fit: c.fit, // 字段 fit
            theme: c.theme, // 字段 theme
            recursive: c.recursive, // 字段 recursive
            running: c.running, // 字段 running
            search: c.search.clone(), // 字段 search
            folder: c.folder.clone(), // 字段 folder
            last_path: c.last_path.clone(), // 字段 last_path
            close_bg: c.close_to_background, // 字段 close_bg
            collapsed_folders: c.collapsed_folders.clone(), // 字段 collapsed_folders
            sidebar_hidden: c.sidebar_hidden, // 字段 sidebar_hidden
            daily_online: c.daily_online, // 字段 daily_online
            online_auto_set: c.online_auto_set, // 字段 online_auto_set
            online_likes: c.online_likes, // 字段 online_likes
            online_dislikes: c.online_dislikes, // 字段 online_dislikes
            last_online_date: c.last_online_date.clone(), // 字段 last_online_date
            wallpaper_only: c.wallpaper_only, // 字段 wallpaper_only
            slideshow: c.slideshow.clone(), // 字段 slideshow
        } // 块结束
    } // 块结束
} // 块结束

pub struct App { // 结构体 App
    cfg: Config, // 字段 cfg
    images: Vec<PathBuf>, // 字段 images
    /// Images grouped by their containing folder (folders = albums).
    albums: Vec<crate::albums::Album>, // 字段 albums
    /// Folders the last scan could not read (used to prompt for permission).
    permission_issues: Vec<PathBuf>, // 字段 permission_issues
    current: Option<usize>, // 字段 current
    last_switch: Instant, // 字段 last_switch
    posters_warmed: bool, // 动态壁纸首帧海报是否已后台预热
    status: String, // 字段 status
    tab: Tab, // 字段 tab
    thumb: Thumbnailer, // 字段 thumb
    tex_cache: HashMap<PathBuf, Option<TextureHandle>>, // 字段 tex_cache
    /// Insertion order of `tex_cache` keys, used for bounded eviction.
    tex_order: Vec<PathBuf>, // 字段 tex_order
    preview: Option<(PathBuf, TextureHandle)>, // 字段 preview
    applied_theme: usize, // 字段 applied_theme
    hotkey: crate::hotkey::Hotkey, // 字段 hotkey
    tray: Option<crate::tray::Tray>, // 字段 tray
    scan_rx: Option<Receiver<Vec<PathBuf>>>, // 字段 scan_rx
    scanning: bool, // 字段 scanning
    window_hidden: bool, // 字段 window_hidden
    force_quit: bool, // 字段 force_quit
    #[cfg(windows)] // 语句
    window_hwnd: isize, // 字段 window_hwnd
    monitors: usize, // 字段 monitors
    primary_res: (i32, i32), // 字段 primary_res
    // Online daily wallpaper (Bing) state.
    online_rx: Option<Receiver<Result<crate::online::OnlineImage, String>>>, // 字段 online_rx
    online_loading: bool, // 字段 online_loading
    online_img: Option<crate::online::OnlineImage>, // 字段 online_img
    online_tex: Option<TextureHandle>, // 字段 online_tex
    online_idx: u32, // 字段 online_idx
    online_boot_checked: bool, // 字段 online_boot_checked
    last_online_auto: Instant, // 字段 last_online_auto
    // 网上壁纸库（多站点：必应 / 彼岸图网 / MoeWalls / MotionBGs）状态。
    web_source: usize, // 当前站点下标（0 = 必应）
    web_items: Vec<crate::websrc::Item>, // 字段 web_items
    web_page: u32, // 字段 web_page
    web_total_pages: u32, // 字段 web_total_pages
    web_has_more: bool, // 字段 web_has_more
    web_only_fav: bool, // 只看收藏
    web_error: Option<String>, // 上次列表加载错误
    web_category: usize, // 字段 web_category
    web_query: String, // 字段 web_query
    web_search_active: bool, // 字段 web_search_active
    web_search_id: Option<u32>, // 字段 web_search_id
    web_loading: bool, // 字段 web_loading
    web_loaded_once: bool, // 字段 web_loaded_once
    web_list_rx: Option<Receiver<Result<crate::websrc::Page, String>>>, // 字段 web_list_rx
    web_thumb: crate::websrc::ThumbWorker, // 字段 web_thumb
    web_thumbs: HashMap<String, Option<TextureHandle>>, // 字段 web_thumbs
    web_thumb_order: Vec<String>, // 字段 web_thumb_order
    web_full: HashMap<u32, Option<crate::websrc::Full>>, // 字段 web_full
    web_full_rx: Option<Receiver<(u32, Result<crate::websrc::Full, String>)>>, // 字段 web_full_rx
    web_preview_id: Option<u32>, // 字段 web_preview_id
    web_preview_tex: Option<TextureHandle>, // 字段 web_preview_tex
    web_pending: Option<(u32, WebApply)>, // 字段 web_pending
    web_busy: bool, // 字段 web_busy
    web_video_busy: bool, // 动态壁纸（mp4）下载中
    /// 动态壁纸（mp4）下载完成后要执行的动作（保存 / 设为动态壁纸 / 加入轮播）。
    web_video_after: WebVideoAfter, // 字段 web_video_after
    web_video_rx: Option<Receiver<Result<PathBuf, String>>>, // 字段 web_video_rx
    /// mpv 按需下载中（仅 Windows；不随安装包分发，缺失时联网下载）。
    mpv_dl_busy: bool, // 字段 mpv_dl_busy
    /// mpv 下载完成后要执行的动作（设为动态壁纸 / 设为锁屏）。
    mpv_after: Option<AfterMpv>, // 字段 mpv_after
    mpv_dl_rx: Option<Receiver<Result<PathBuf, String>>>, // 字段 mpv_dl_rx
    wallpaper_managers: Vec<String>, // 字段 wallpaper_managers
    /// Compact (phone/tablet or narrow window) layout with a bottom nav bar.
    compact: bool, // 字段 compact
    /// Per-album display limit (pages of `PAGE_IMAGES`), grows via "load more".
    album_limits: HashMap<String, usize>, // 字段 album_limits
    /// Active screen-size adaptation screenshot run (from `--shots <dir>`).
    shot: Option<crate::shot::Shooter>, // 字段 shot
    /// 是否已按显示器可用范围校正过初始窗口尺寸（只做一次）。
    window_fitted: bool, // 字段 window_fitted
} // 块结束

impl App { // 实现
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self { // 函数 new
        let cfg = Config::load(); // 变量 cfg
        theme::apply(&cc.egui_ctx, cfg.theme == 1); // 调用 apply
        // 让滚动条常驻可见（实心、会占位）：右侧内容「可上下滑动」一目了然，
        // 且不会像浮动滚动条那样遮住最右侧的内容。
        cc.egui_ctx.style_mut(|s| { // 调用 style_mut
            s.spacing.scroll = egui::style::ScrollStyle::solid(); // 语句
        }); // 语句
        // 恢复上次「网上壁纸库」的来源 / 分类 / 关键词 / 收藏筛选。
        let web_src = crate::websrc::source_index_by_key(&cfg.web_source_key); // 变量 web_src
        let web_cat = cfg.web_category; // 变量 web_cat
        let web_query = cfg.web_query.clone(); // 变量 web_query
        let web_search_active = cfg.web_search_active; // 变量 web_search_active
        let web_only_fav = cfg.web_only_fav; // 变量 web_only_fav
        #[cfg(windows)] // 语句
        let window_hwnd = { // 变量 window_hwnd
            use raw_window_handle::{HasWindowHandle, RawWindowHandle}; // 引入依赖
            cc.window_handle() // 语句
                .ok() // 语句
                .and_then(|h| match h.as_raw() { // 语句
                    RawWindowHandle::Win32(w) => Some(w.hwnd.get()), // 匹配分支
                    _ => None, // 匹配分支
                }) // 语句
                .unwrap_or(0) // 语句
        }; // 块结束
        let mut app = Self { // 变量 app
            applied_theme: cfg.theme, // 字段 applied_theme
            cfg, // 语句
            images: Vec::new(), // 字段 images
            albums: Vec::new(), // 字段 albums
            permission_issues: Vec::new(), // 字段 permission_issues
            current: None, // 字段 current
            last_switch: Instant::now(), // 字段 last_switch
            posters_warmed: false, // 字段 posters_warmed
            status: String::new(), // 字段 status
            tab: Tab::Web4k, // 字段 tab（默认进入「网上壁纸库」）
            thumb: Thumbnailer::new(), // 字段 thumb
            tex_cache: HashMap::new(), // 字段 tex_cache
            tex_order: Vec::new(), // 字段 tex_order
            preview: None, // 字段 preview
            hotkey: crate::hotkey::start(cc.egui_ctx.clone()), // 字段 hotkey
            tray: None, // 字段 tray
            scan_rx: None, // 字段 scan_rx
            scanning: false, // 字段 scanning
            window_hidden: false, // 字段 window_hidden
            force_quit: false, // 字段 force_quit
            #[cfg(windows)] // 语句
            window_hwnd, // 语句
            monitors: wallpaper::monitor_count(), // 字段 monitors
            primary_res: wallpaper::primary_resolution(), // 字段 primary_res
            online_rx: None, // 字段 online_rx
            online_loading: false, // 字段 online_loading
            online_img: None, // 字段 online_img
            online_tex: None, // 字段 online_tex
            online_idx: 0, // 字段 online_idx
            online_boot_checked: false, // 字段 online_boot_checked
            last_online_auto: Instant::now(), // 字段 last_online_auto
            web_source: web_src, // 默认必应（排第一）
            web_items: Vec::new(), // 字段 web_items
            web_page: 1, // 字段 web_page
            web_total_pages: 1, // 字段 web_total_pages
            web_has_more: false, // 字段 web_has_more
            web_only_fav, // 只看收藏
            web_error: None, // 上次列表加载错误
            web_category: web_cat, // 字段 web_category
            web_query, // 字段 web_query
            web_search_active, // 字段 web_search_active
            web_search_id: None, // 字段 web_search_id
            web_loading: false, // 字段 web_loading
            web_loaded_once: false, // 字段 web_loaded_once
            web_list_rx: None, // 字段 web_list_rx
            web_thumb: crate::websrc::ThumbWorker::new(config_dir().join("webwall")), // 字段 web_thumb
            web_thumbs: HashMap::new(), // 字段 web_thumbs
            web_thumb_order: Vec::new(), // 字段 web_thumb_order
            web_full: HashMap::new(), // 字段 web_full
            web_full_rx: None, // 字段 web_full_rx
            web_preview_id: None, // 字段 web_preview_id
            web_preview_tex: None, // 字段 web_preview_tex
            web_pending: None, // 字段 web_pending
            web_busy: false, // 字段 web_busy
            web_video_busy: false, // 字段 web_video_busy
            web_video_after: WebVideoAfter::default(), // 字段 web_video_after
            web_video_rx: None, // 字段 web_video_rx
            mpv_dl_busy: false, // 字段 mpv_dl_busy
            mpv_after: None, // 字段 mpv_after
            mpv_dl_rx: None, // 字段 mpv_dl_rx
            wallpaper_managers: wallpaper::conflicting_wallpaper_managers(), // 字段 wallpaper_managers
            compact: false, // 字段 compact
            album_limits: HashMap::new(), // 字段 album_limits
            shot: crate::shot::Shooter::from_env(), // 字段 shot
            window_fitted: false, // 字段 window_fitted
        }; // 块结束
        // Optional deep-link to a tab, e.g. `wallpaper-rs --tab settings`.
        if let Some(pos) = std::env::args().position(|a| a == "--tab") { // 条件分支
            if let Some(name) = std::env::args().nth(pos + 1) { // 条件分支
                app.tab = match name.to_ascii_lowercase().as_str() { // 语句
                    "web" | "netbian" | "web4k" => Tab::Web4k, // 匹配分支
                    "slideshow" => Tab::Slideshow, // 匹配分支
                    "online" => Tab::Online, // 匹配分支
                    "settings" => Tab::Settings, // 匹配分支
                    "about" => Tab::About, // 匹配分支
                    _ => Tab::Library, // 匹配分支
                }; // 块结束
            } // 块结束
        } // 块结束
        if let Some(sh) = app.shot.as_ref() { // 条件分支
            crate::shot::ensure_dir(&sh.dir); // 调用 ensure_dir
        } // 块结束
        // Optional: preselect the online source, e.g. `WP_WEB_SOURCE=2` (MoeWalls).
        if let Some(v) = std::env::var("WP_WEB_SOURCE").ok().and_then(|s| s.trim().parse::<usize>().ok()) { // 条件分支
            app.web_source = v.min(crate::websrc::SOURCES.len() - 1); // 语句
        } // 块结束
        // Install the system-tray icon (needed to restore the window after the
        // user hides it by clicking "X"). `None` means the platform has no tray;
        // in that case we fall back to the global-hotkey restore path.
        #[cfg(windows)] // 语句
        let tray_hwnd = window_hwnd; // 变量 tray_hwnd
        #[cfg(not(windows))] // 语句
        let tray_hwnd = 0isize; // 变量 tray_hwnd
        app.tray = crate::tray::Tray::install(cc.egui_ctx.clone(), tray_hwnd); // 语句
        // Keep timers (slideshow / daily wallpaper) ticking even while hidden.
        { // 语句
            let c = cc.egui_ctx.clone(); // 变量 c
            std::thread::spawn(move || loop { // 调用 spawn
                std::thread::sleep(Duration::from_secs(1)); // 调用 sleep
                c.request_repaint(); // 语句
            }); // 语句
        } // 块结束
        app.scan(); // 语句
        // 打开软件时若播放模式已是「轮播」，默认就开始轮播，并**立刻先换一张**，
        // 随后按「间隔」一张一张地换（不必再点「▶ 开始轮播」）。
        if app.cfg.mode == Mode::Slideshow { // 条件分支
            app.cfg.running = true; // 语句
            app.advance(&cc.egui_ctx); // 打开瞬间立刻换下一张
        } // 块结束
        app // 语句
    } // 块结束

    fn scan(&mut self) { // 函数 scan
        self.images.clear(); // 语句
        self.albums.clear(); // 语句
        self.permission_issues.clear(); // 语句
        self.thumb.reset(); // 语句
        self.tex_cache.clear(); // 语句
        self.tex_order.clear(); // 语句
        self.preview = None; // 语句
        self.current = None; // 语句
        if let Some(folder) = self.cfg.folder.clone() { // 条件分支
            if folder.exists() { // 条件分支
                let mut files = Vec::new(); // 变量 files
                let mut denied = Vec::new(); // 变量 denied
                if crate::permissions::probe_dir(&folder) // 条件分支
                    == crate::permissions::PermissionState::Denied // 语句
                { // 语句
                    denied.push(folder.clone()); // 语句
                } // 块结束
                collect_images(&folder, self.cfg.recursive, &mut files, &mut denied); // 调用 collect_images
                denied.sort(); // 语句
                denied.dedup(); // 语句
                files.sort(); // 语句
                if self.cfg.wallpaper_only { // 条件分支
                    // 视频（动态壁纸）无法用图片头判断尺寸，直接保留。
                    files.retain(|p| { // 调用 retain
                        crate::thumbs::is_video_file(p) || crate::thumbs::is_wallpaper_file(p) // 语句
                    }); // 语句
                } // 块结束
                self.images = files; // 语句
                self.albums = crate::albums::build_albums(&self.images, Some(&folder)); // 语句
                self.permission_issues = denied; // 语句
            } // 块结束
        } // 块结束
        if let Some(lp) = self.cfg.last_path.clone() { // 条件分支
            if let Some(pos) = self.images.iter().position(|p| p == &lp) { // 条件分支
                self.current = Some(pos); // 语句
            } // 块结束
        } // 块结束
        if self.current.is_none() && !self.images.is_empty() { // 条件分支
            self.current = Some(0); // 语句
        } // 块结束
        self.status = if self.images.is_empty() { // 语句
            "尚未载入图片，请选择壁纸文件夹".to_string() // 语句
        } else { // 语句
            format!("已载入 {} 张图片", self.images.len()) // 语句
        }; // 块结束
        if !self.permission_issues.is_empty() { // 条件分支
            self.status = format!( // 语句
                "已载入 {} 张图片，但有 {} 个目录无访问权限", // 语句
                self.images.len(), // 语句
                self.permission_issues.len() // 语句
            ); // 块结束
        } // 块结束
        self.last_switch = Instant::now(); // 语句
    } // 块结束

    fn load_preview(&mut self, ctx: &egui::Context, path: &Path) { // 函数 load_preview
        // 视频（动态壁纸）没有静态预览：清空即可，避免每帧白白尝试解码。
        if crate::thumbs::is_video_file(path) { // 条件分支
            self.preview = None; // 语句
            return; // 返回
        } // 块结束
        if let Some((p, _)) = &self.preview { // 条件分支
            if p == path { // 条件分支
                return; // 返回
            } // 块结束
        } // 块结束
        match crate::thumbs::load_image(path, 1400) { // 分支匹配
            Some(t) => { // 匹配分支
                let img = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba); // 变量 img
                let tex = ctx.load_texture( // 变量 tex
                    format!("preview:{}", path.display()), // 语句
                    img, // 语句
                    egui::TextureOptions::LINEAR, // 字段 egui
                ); // 块结束
                self.preview = Some((path.to_path_buf(), tex)); // 语句
            } // 块结束
            None => self.preview = None, // 匹配分支
        } // 块结束
    } // 块结束

    /// The set of images the slideshow cycles through. When the user has curated
    /// a playlist (`cfg.slideshow`, i.e. added / removed specific pictures) use
    /// it; otherwise fall back to every image in the library. Missing files are
    /// skipped so a deleted picture never wedges the rotation.
    fn slide_paths(&self) -> Vec<PathBuf> { // 函数 slide_paths
        let base = if self.cfg.slideshow.is_empty() { // 变量 base
            self.images.clone() // 语句
        } else { // 语句
            self.cfg.slideshow.clone() // 语句
        }; // 块结束
        base.into_iter().filter(|p| p.is_file()).collect() // 语句
    } // 块结束

    /// Whether `path` is part of the slideshow (accounting for the "empty means
    /// all library images" default).
    fn in_playlist(&self, path: &Path) -> bool { // 函数 in_playlist
        if self.cfg.slideshow.is_empty() { // 条件分支
            self.images.iter().any(|p| p == path) // 语句
        } else { // 语句
            self.cfg.slideshow.iter().any(|p| p == path) // 语句
        } // 块结束
    } // 块结束

    /// Add `path` to the slideshow playlist. Adding to an empty (="all") playlist
    /// first materialises the current library so the implicit membership is kept.
    fn add_to_playlist(&mut self, path: &Path) { // 函数 add_to_playlist
        if self.cfg.slideshow.is_empty() { // 条件分支
            self.cfg.slideshow = self.images.clone(); // 语句
        } // 块结束
        if !self.cfg.slideshow.iter().any(|p| p == path) { // 条件分支
            self.cfg.slideshow.push(path.to_path_buf()); // 语句
        } // 块结束
        self.status = "已加入轮播清单".to_string(); // 语句
    } // 块结束

    /// Remove `path` from the slideshow playlist, materialising the "all" default
    /// first so removing one picture keeps the rest.
    fn remove_from_playlist(&mut self, path: &Path) { // 函数 remove_from_playlist
        if self.cfg.slideshow.is_empty() { // 条件分支
            self.cfg.slideshow = self.images.clone(); // 语句
        } // 块结束
        self.cfg.slideshow.retain(|p| p != path); // 语句
        self.status = "已从轮播清单移除".to_string(); // 语句
    } // 块结束

    fn set_current(&mut self, ctx: &egui::Context, i: usize) { // 函数 set_current
        if self.images.is_empty() { // 条件分支
            return; // 返回
        } // 块结束
        let i = i % self.images.len(); // 变量 i
        let path = self.images[i].clone(); // 变量 path
        self.apply_path(ctx, &path); // 语句
    } // 块结束

    /// Apply a specific picture as the current wallpaper (used by both the
    /// library "set wallpaper" action and the slideshow rotation).
    fn apply_path(&mut self, ctx: &egui::Context, path: &Path) { // 函数 apply_path
        self.current = self.images.iter().position(|p| p == path); // 语句
        self.cfg.last_path = Some(path.to_path_buf()); // 语句
        self.load_preview(ctx, path); // 语句
        let name = path // 变量 name
            .file_name() // 语句
            .map(|s| s.to_string_lossy().to_string()) // 语句
            .unwrap_or_default(); // 语句
        if crate::thumbs::is_video_file(path) { // 条件分支
            // 视频 -> 动态壁纸宿主（mpv 循环渲染）；mpv 缺失时（Windows）按需下载。
            self.with_mpv_or_download(AfterMpv::LiveWallpaper(path.to_path_buf())); // 语句
        } else { // 语句
            // 从视频切回静态图：先把新壁纸铺到底层（此刻动态壁纸仍在播放、盖在上面，
            // 看不到切换过程），再停掉动态壁纸宿主——露出桌面时就已经是新壁纸，
            // 不会「闪过」上一张固定壁纸。
            match wallpaper::set_wallpaper(path, self.cfg.fit) { // 分支匹配
                Ok(_) => self.status = format!("已设置壁纸：{}", name), // 匹配分支
                Err(e) => self.status = format!("设置失败：{e}"), // 匹配分支
            } // 块结束
            if crate::livewall::active_video().is_some() { // 条件分支
                let _ = crate::livewall::stop(); // 变量 _
            } // 块结束
        } // 块结束
        self.last_switch = Instant::now(); // 语句
    } // 块结束

    fn advance(&mut self, ctx: &egui::Context) { // 函数 advance
        let list = self.slide_paths(); // 变量 list
        if list.is_empty() { // 条件分支
            return; // 返回
        } // 块结束
        let cur = self.current_path(); // 变量 cur
        let next = if self.cfg.shuffle { // 变量 next
            let mut k = rand::thread_rng().gen_range(0..list.len()); // 变量 k
            if list.len() > 1 { // 条件分支
                while list.get(k) == cur.as_ref() { // 循环
                    k = rand::thread_rng().gen_range(0..list.len()); // 语句
                } // 块结束
            } // 块结束
            k // 语句
        } else { // 语句
            match cur.and_then(|p| list.iter().position(|x| *x == p)) { // 分支匹配
                Some(c) => (c + 1) % list.len(), // 匹配分支
                None => 0, // 匹配分支
            } // 块结束
        }; // 块结束
        let path = list[next].clone(); // 变量 path
        self.apply_path(ctx, &path); // 语句
    } // 块结束

    fn step_back(&mut self, ctx: &egui::Context) { // 函数 step_back
        let list = self.slide_paths(); // 变量 list
        if list.is_empty() { // 条件分支
            return; // 返回
        } // 块结束
        let cur = self.current_path(); // 变量 cur
        let prev = match cur.and_then(|p| list.iter().position(|x| *x == p)) { // 变量 prev
            Some(c) => (c + list.len() - 1) % list.len(), // 匹配分支
            None => 0, // 匹配分支
        }; // 块结束
        let path = list[prev].clone(); // 变量 path
        self.apply_path(ctx, &path); // 语句
    } // 块结束

    fn random_pick(&mut self, ctx: &egui::Context) { // 函数 random_pick
        let list = self.slide_paths(); // 变量 list
        if list.is_empty() { // 条件分支
            return; // 返回
        } // 块结束
        let cur = self.current_path(); // 变量 cur
        let mut k = rand::thread_rng().gen_range(0..list.len()); // 变量 k
        if list.len() > 1 { // 条件分支
            while list.get(k) == cur.as_ref() { // 循环
                k = rand::thread_rng().gen_range(0..list.len()); // 语句
            } // 块结束
        } // 块结束
        let path = list[k].clone(); // 变量 path
        self.apply_path(ctx, &path); // 语句
    } // 块结束

    fn current_path(&self) -> Option<PathBuf> { // 函数 current_path
        self.current // 语句
            .and_then(|i| self.images.get(i)) // 语句
            .cloned() // 语句
            .or_else(|| self.cfg.last_path.clone()) // 语句
    } // 块结束

    /// Cheap count of the pictures the slideshow will cycle (avoids cloning the
    /// whole list every frame; file existence is checked when actually rotating).
    fn slide_count(&self) -> usize { // 函数 slide_count
        if self.cfg.slideshow.is_empty() { // 条件分支
            self.images.len() // 语句
        } else { // 语句
            self.cfg.slideshow.len() // 语句
        } // 块结束
    } // 块结束

    fn apply(&mut self, ctx: &egui::Context, actions: Vec<Action>) { // 函数 apply
        for a in actions { // 循环
            match a { // 分支匹配
                Action::SetPath(p) => { // 匹配分支
                    if let Some(i) = self.images.iter().position(|x| x == &p) { // 条件分支
                        self.set_current(ctx, i); // 语句
                    } // 块结束
                } // 块结束
                Action::Next => self.advance(ctx), // 匹配分支
                Action::Prev => self.step_back(ctx), // 匹配分支
                Action::Random => self.random_pick(ctx), // 匹配分支
                Action::Rescan => self.scan(), // 匹配分支
                Action::PickFolder => { // 匹配分支
                    #[cfg(not(target_os = "android"))] // 语句
                    { // 语句
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() { // 条件分支
                            self.cfg.folder = Some(dir); // 语句
                            self.scan(); // 语句
                        } // 块结束
                    } // 块结束
                    #[cfg(target_os = "android")] // 语句
                    { // 语句
                        self.status = "移动端请在系统中选择图片文件夹".to_string(); // 语句
                    } // 块结束
                } // 块结束
                Action::OpenFolder => { // 匹配分支
                    if let Some(f) = &self.cfg.folder { // 条件分支
                        open_path(f); // 调用 open_path
                    } // 块结束
                } // 块结束
                Action::OpenConfig => { // 匹配分支
                    let _ = std::fs::create_dir_all(config_dir()); // 变量 _
                    open_path(&config_dir()); // 调用 open_path
                } // 块结束
                Action::Save => { // 匹配分支
                    self.cfg.save(); // 语句
                    self.status = "配置已保存".to_string(); // 语句
                } // 块结束
                Action::ScanAllDisks => self.start_full_scan(), // 匹配分支
                Action::HideToTray => self.hide_to_tray(ctx), // 匹配分支
                Action::ToggleSidebar => { // 匹配分支
                    self.cfg.sidebar_hidden = !self.cfg.sidebar_hidden; // 语句
                } // 块结束
                Action::ToggleTheme => { // 匹配分支
                    self.cfg.theme = if self.cfg.theme == 1 { 0 } else { 1 }; // 语句
                } // 块结束
                Action::ExpandAllAlbums => self.cfg.collapsed_folders.clear(), // 匹配分支
                Action::CollapseAllAlbums => { // 匹配分支
                    self.cfg.collapsed_folders = // 语句
                        self.albums.iter().map(|a| a.key.clone()).collect(); // 语句
                } // 块结束
                Action::RetryPermissions => { // 匹配分支
                    self.scan(); // 语句
                    if self.permission_issues.is_empty() { // 条件分支
                        self.status = "权限已获取，图库已重新载入".to_string(); // 语句
                    } // 块结束
                } // 块结束
                Action::OpenPermissionPage => { // 匹配分支
                    // Reveal the denied folder itself: Explorer shows its
                    // "get access to this folder" confirmation dialog, which is
                    // the OS-native way to let the operator grant permission.
                    // On macOS this instead opens the Full Disk Access page.
                    let target = self.permission_issues.first().cloned(); // 变量 target
                    crate::permissions::open_authorization(target.as_deref()); // 调用 open_authorization
                } // 块结束
                Action::DismissPermissions => self.permission_issues.clear(), // 匹配分支
                Action::Quit => { // 匹配分支
                    // Explicit quit always truly ends the process, even if
                    // hide-to-tray is on or the window is currently hidden.
                    self.hard_exit(); // 语句
                } // 块结束
                Action::ReapplyCurrent => { // 匹配分支
                    if let Some(i) = self.current { // 条件分支
                        // Re-apply the current wallpaper so a display-mode change
                        // takes effect immediately, without waiting for the next switch.
                        self.set_current(ctx, i); // 语句
                    } // 块结束
                } // 块结束
                Action::LockVideo(p) => self.with_mpv_or_download(AfterMpv::LockVideo(p)), // 匹配分支
            } // 块结束
        } // 块结束
    } // 块结束

    fn start_full_scan(&mut self) { // 函数 start_full_scan
        if self.scanning { // 条件分支
            return; // 返回
        } // 块结束
        self.scanning = true; // 语句
        self.scan_rx = Some(crate::diskscan::start(self.cfg.wallpaper_only)); // 语句
        self.status = if self.cfg.wallpaper_only { // 语句
            "正在全盘扫描壁纸（仅载入 ≥1280×720 横屏）…".to_string() // 语句
        } else { // 语句
            "正在全盘扫描图片…（后台进行，完成后自动载入）".to_string() // 语句
        }; // 块结束
    } // 块结束

    /// Hide the main window into the system tray while keeping the app running
    /// (slideshow / daily wallpaper continue in the background).
    fn hide_to_tray(&mut self, ctx: &egui::Context) { // 函数 hide_to_tray
        // On Windows we keep the HWND on-screen but fully transparent and
        // click-through (and out of the taskbar). Keeping it on-screen preserves
        // the egui event loop, so the tray menu / hotkey / timers keep working.
        #[cfg(windows)] // 语句
        { // 语句
            unsafe { crate::winhide::hide_to_tray(self.window_hwnd) }; // 语句
        } // 块结束
        #[cfg(not(windows))] // 语句
        { // 语句
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false)); // 语句
        } // 块结束
        let _ = ctx; // 变量 _
        self.window_hidden = true; // 语句
        self.status = "已隐藏到系统托盘，点击托盘图标可唤回".to_string(); // 语句
    } // 块结束

    /// Restore and focus the main window (from the tray icon or global hotkey).
    fn show_window(&mut self, ctx: &egui::Context) { // 函数 show_window
        #[cfg(windows)] // 语句
        { // 语句
            unsafe { crate::winhide::show_from_tray(self.window_hwnd) }; // 语句
        } // 块结束
        #[cfg(not(windows))] // 语句
        { // 语句
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true)); // 语句
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false)); // 语句
            // Centre roughly on the current monitor (macOS / Linux).
            let info = ctx.input(|i| { // 变量 info
                let vp = i.viewport(); // 变量 vp
                (vp.monitor_size, vp.outer_rect.map(|r| r.size())) // 语句
            }); // 语句
            if let (Some(mon), Some(size)) = info { // 条件分支
                let pos = egui::pos2( // 变量 pos
                    ((mon.x - size.x).max(0.0)) * 0.5, // 语句
                    ((mon.y - size.y).max(0.0)) * 0.5, // 语句
                ); // 块结束
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos)); // 语句
            } // 块结束
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus); // 语句
        } // 块结束
        let _ = ctx; // 变量 _
        self.window_hidden = false; // 语句
        // Keep the status line consistent (otherwise it can keep showing
        // "已隐藏到系统托盘…" after the window is back).
        self.status = "已唤回窗口".to_string(); // 语句
    } // 块结束

    /// Really terminate the background process (used by the tray "Quit" item and
    /// the in-app Quit button). Saves config and removes the tray icon first so
    /// no stale notification-area icon is left behind, then exits the process.
    fn hard_exit(&mut self) -> ! { // 函数 hard_exit
        self.force_quit = true; // 语句
        self.cfg.save(); // 语句
        // 停掉动态壁纸宿主：结束 mpv 子进程并销毁桌面层窗口，避免残留。
        let _ = crate::livewall::stop(); // 变量 _
        // Dropping the tray icon sends Shell_NotifyIcon(NIM_DELETE).
        self.tray = None; // 语句
        // "退出" means the whole app: take down every other wallpaper-rs copy
        // (autostart + manual start, double-clicked exe, stale instances, ...)
        // so no background slideshow is left running.
        crate::instances::terminate_other_instances(); // 调用 terminate_other_instances
        std::process::exit(0) // 调用 exit
    } // 块结束

    /// Handle an incoming screenshot result and advance the shot plan. Returns
    /// `true` when the whole run is finished (the app then closes).
    fn shot_pre(&mut self, ctx: &egui::Context) -> bool { // 函数 shot_pre
        if self.shot.is_none() { // 条件分支
            return false; // 返回
        } // 块结束
        // Consume a screenshot from the previous frame's request.
        let img = if self.shot.as_ref().map(|s| s.pending).unwrap_or(false) { // 变量 img
            crate::shot::take_screenshot(ctx) // 调用 take_screenshot
        } else { // 语句
            None // 语句
        }; // 块结束
        if let Some(img) = img { // 条件分支
            if let Some(sh) = self.shot.as_mut() { // 条件分支
                sh.store(&img); // 语句
            } // 块结束
        } // 块结束
        // Finished?
        let finished = self.shot.as_ref().map(|s| s.finished()).unwrap_or(false); // 变量 finished
        if finished { // 条件分支
            let (n, dir) = self // 调用 let
                .shot // 语句
                .as_ref() // 语句
                .map(|s| (s.saved.len(), s.dir.display().to_string())) // 语句
                .unwrap_or((0, String::new())); // 语句
            if let Some(sh) = self.shot.as_ref() { // 条件分支
                sh.write_manifest(); // 语句
            } // 块结束
            self.status = format!("适配截图完成：{n} 张，已保存到 {dir}"); // 语句
            self.force_quit = true; // 语句
            self.cfg.save(); // 语句
            ctx.send_viewport_cmd(egui::ViewportCommand::Close); // 语句
            return true; // 返回
        } // 块结束
        // Apply the current item's tab and (once) its window size.
        let info = self.shot.as_ref().and_then(|s| { // 变量 info
            s.current().map(|it| (it.tab, it.w, it.h, s.settle, s.pending)) // 语句
        }); // 语句
        if let Some((tab, w, h, settle, pending)) = info { // 条件分支
            self.tab = tab_from_index(tab); // 语句
            if settle == 0 && !pending { // 条件分支
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h))); // 语句
            } // 块结束
            // In screenshot mode, open a preview on the web tab so the captured
            // frame shows the "preview + list together" layout we are verifying.
            if self.tab == Tab::Web4k && self.web_preview_id.is_none() { // 条件分支
                if let Some(first) = self.web_items.first().map(|i| i.id) { // 条件分支
                    self.web_open_preview(ctx, first); // 语句
                } // 块结束
            } // 块结束
        } // 块结束
        false // 语句
    } // 块结束

    /// Advance the settle timer and request the screenshot once the window has
    /// had time to resize and render.
    fn shot_post(&mut self, ctx: &egui::Context) { // 函数 shot_post
        let Some(sh) = self.shot.as_mut() else { // 变量 Some
            return; // 返回
        }; // 块结束
        if sh.pending || sh.finished() { // 条件分支
            return; // 返回
        } // 块结束
        sh.settle += 1; // 语句
        if sh.settle >= crate::shot::SETTLE_FRAMES { // 条件分支
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot); // 语句
            sh.pending = true; // 语句
        } // 块结束
    } // 块结束

    /// Keep the thumbnail texture cache bounded so scanning thousands of images
    /// cannot exhaust VRAM/RAM. Evicted paths are forgotten by the thumbnailer
    /// so they are re-decoded if they scroll back into view.
    fn enforce_tex_cap(&mut self) { // 函数 enforce_tex_cap
        let plan = evict_plan(&self.tex_order, TEX_CAP); // 变量 plan
        if plan.is_empty() { // 条件分支
            return; // 返回
        } // 块结束
        for k in &plan { // 循环
            self.tex_cache.remove(k); // 语句
            self.thumb.forget(k); // 语句
        } // 块结束
        self.tex_order.retain(|k| self.tex_cache.contains_key(k)); // 语句
    } // 块结束

    // ---- Online daily wallpaper (Bing) ------------------------------------
    fn start_online_fetch(&mut self, idx: u32) { // 函数 start_online_fetch
        if self.online_loading { // 条件分支
            return; // 返回
        } // 块结束
        self.online_loading = true; // 语句
        self.online_idx = idx; // 语句
        self.status = format!("正在获取在线壁纸（第 {} 张）…", idx + 1); // 语句
        let (tx, rx) = std::sync::mpsc::channel(); // 调用 let
        self.online_rx = Some(rx); // 语句
        let cache = config_dir().join("online"); // 变量 cache
        std::thread::spawn(move || { // 调用 spawn
            let r = crate::online::fetch(idx, &cache); // 变量 r
            let _ = tx.send(r); // 变量 _
        }); // 语句
    } // 块结束

    fn apply_online_wallpaper(&mut self) { // 函数 apply_online_wallpaper
        let (path, title) = match &self.online_img { // 调用 let
            Some(img) => (img.path.clone(), img.title.clone()), // 匹配分支
            None => { // 匹配分支
                self.status = "请先获取在线壁纸".to_string(); // 语句
                return; // 返回
            } // 块结束
        }; // 块结束
        match wallpaper::set_wallpaper(&path, self.cfg.fit) { // 分支匹配
            Ok(_) => self.status = format!("已设为桌面壁纸：{title}"), // 匹配分支
            Err(e) => self.status = format!("设置失败：{e}"), // 匹配分支
        } // 块结束
    } // 块结束

    fn set_lock_from_current(&mut self) { // 函数 set_lock_from_current
        let Some(path) = self.current_path() else { // 变量 Some
            self.status = "请先选择一张壁纸".to_string(); // 语句
            return; // 返回
        }; // 块结束
        // 当前是动态壁纸视频时，先截取一帧再设置（锁屏只支持静态图片）。
        if crate::thumbs::is_video_file(&path) { // 条件分支
            self.with_mpv_or_download(AfterMpv::LockVideo(path)); // 语句
        } else { // 语句
            self.apply_lock_screen(&path); // 语句
        } // 块结束
    } // 块结束

    fn set_lock_from_online(&mut self) { // 函数 set_lock_from_online
        let Some(img) = &self.online_img else { // 变量 Some
            self.status = "请先获取在线壁纸".to_string(); // 语句
            return; // 返回
        }; // 块结束
        let path = img.path.clone(); // 变量 path
        self.apply_lock_screen(&path); // 语句
    } // 块结束

    /// 把视频设为锁屏壁纸。
    ///
    /// Windows 锁屏只接受静态图片、无法直接播放视频，因此先用 mpv 从视频中截取
    /// 一帧作为海报图（缓存于 `<配置目录>/lock_posters/<文件名>.png`），再按普通
    /// 静态壁纸设置锁屏（必要时走 UAC 提权）。
    fn set_lock_from_video(&mut self, video: &Path) { // 函数 set_lock_from_video
        let stem = video // 变量 stem
            .file_stem() // 语句
            .map(|s| s.to_string_lossy().to_string()) // 语句
            .unwrap_or_else(|| "poster".to_string()); // 语句
        let out = config_dir().join("lock_posters").join(format!("{stem}.png")); // 变量 out
        match crate::livewall::poster_frame(video, &out) { // 分支匹配
            Ok(_) => self.apply_lock_screen(&out), // 匹配分支
            Err(e) => self.status = format!("动态壁纸锁屏失败：{e}"), // 匹配分支
        } // 块结束
    } // 块结束

    /// Set the lock-screen image, transparently requesting administrator rights
    /// when needed. On Windows the write goes to `HKLM`, so a non-elevated
    /// attempt is denied (code 5): rather than telling the user to "run as
    /// administrator", we relaunch ourselves through the UAC prompt, perform the
    /// write in that elevated copy, then report the outcome and carry on.
    fn apply_lock_screen(&mut self, path: &Path) { // 函数 apply_lock_screen
        match wallpaper::set_lock_screen(path) { // 分支匹配
            Ok(_) => self.status = "已设置锁屏壁纸".to_string(), // 匹配分支
            Err(e) if e.needs_admin && !crate::elevate::is_elevated() => { // 匹配分支
                self.status = "正在请求管理员权限以设置锁屏…".to_string(); // 语句
                let arg = path.to_string_lossy().to_string(); // 变量 arg
                match crate::elevate::run_elevated(crate::elevate::ACTION_LOCK_SCREEN, &[arg]) { // 分支匹配
                    crate::elevate::ElevationResult::Ok => { // 匹配分支
                        self.status = "已设置锁屏壁纸（已通过管理员授权）".to_string() // 语句
                    } // 块结束
                    crate::elevate::ElevationResult::Cancelled => { // 匹配分支
                        self.status = "已取消管理员授权，未设置锁屏壁纸".to_string() // 语句
                    } // 块结束
                    crate::elevate::ElevationResult::Failed(msg) => { // 匹配分支
                        self.status = format!("锁屏设置失败：{msg}") // 语句
                    } // 块结束
                } // 块结束
            } // 块结束
            Err(e) => self.status = format!("锁屏设置失败：{e}"), // 匹配分支
        } // 块结束
    } // 块结束

    fn ui_online(&mut self, ui: &mut egui::Ui) { // 函数 ui_online
        ui.horizontal(|ui| { // 语句
            ui.label( // 语句
                RichText::new("每日壁纸 · 在线") // 调用 new
                    .size(20.0) // 语句
                    .strong() // 语句
                    .color(theme::text()), // 语句
            ); // 块结束
            ui.add_space(8.0); // 语句
            chip(ui, "Bing · cn.bing.com", theme::accent_soft(), theme::accent()); // 调用 chip
        }); // 语句
        ui.add_space(2.0); // 语句
        ui.label( // 语句
            RichText::new( // 调用 new
                "联网获取 Bing 每日壁纸（cn.bing.com，国内可直接访问）。可设为桌面 / 锁屏、赞或踩、切换上一张/下一张，并支持每日自动更换。", // 语句
            ) // 块结束
            .size(11.5) // 语句
            .color(theme::subtext()), // 语句
        ); // 块结束
        ui.add_space(12.0); // 语句

        if !self.wallpaper_managers.is_empty() { // 条件分支
            egui::Frame::none() // 调用 none
                .fill(theme::accent_soft()) // 语句
                .rounding(Rounding::same(10.0)) // 语句
                .inner_margin(egui::Margin::symmetric(12.0, 8.0)) // 语句
                .show(ui, |ui| { // 语句
                    ui.label( // 语句
                        RichText::new(format!( // 调用 new
                            "⚠ 检测到可能在接管桌面壁纸的程序：{}。若点击「设为桌面壁纸」后桌面没有变化，请退出该程序（例如华硕 Armoury Crate 的 Aura 壁纸功能）后重试。", // 语句
                            self.wallpaper_managers.join("、") // 语句
                        )) // 语句
                        .size(11.5) // 语句
                        .color(theme::accent()), // 语句
                    ); // 块结束
                }); // 语句
            ui.add_space(10.0); // 语句
        } // 块结束

        // Toolbar ----------------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 12.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.horizontal(|ui| { // 语句
                    if primary_button(ui, "获取今日壁纸").clicked() { // 条件分支
                        self.start_online_fetch(0); // 语句
                    } // 块结束
                    if ghost_button(ui, "‹ 上一张").clicked() { // 条件分支
                        let i = self.online_idx.saturating_sub(1); // 变量 i
                        self.start_online_fetch(i); // 语句
                    } // 块结束
                    if ghost_button(ui, "下一张 ›").clicked() { // 条件分支
                        let i = (self.online_idx + 1).min(7); // 变量 i
                        self.start_online_fetch(i); // 语句
                    } // 块结束
                    if self.online_loading { // 条件分支
                        chip(ui, "获取中…", theme::accent_soft(), theme::accent()); // 调用 chip
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        let mut d = self.cfg.daily_online; // 变量 d
                        if ui.checkbox(&mut d, "自动每日换壁纸").changed() { // 条件分支
                            self.cfg.daily_online = d; // 语句
                            if d { // 条件分支
                                self.online_boot_checked = true; // 语句
                                if self.online_img.is_none() { // 条件分支
                                    self.start_online_fetch(0); // 语句
                                } // 块结束
                            } // 块结束
                        } // 块结束
                        let mut a = self.cfg.online_auto_set; // 变量 a
                        if ui.checkbox(&mut a, "获取后自动设为桌面").changed() { // 条件分支
                            self.cfg.online_auto_set = a; // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
        ui.add_space(12.0); // 语句

        // Preview + info card ---------------------------------------------
        let tex_id = self.online_tex.as_ref().map(|t| t.id()); // 变量 tex_id
        let tex_size = self.online_tex.as_ref().map(|t| t.size()); // 变量 tex_size
        let info = self.online_img.as_ref().map(|i| { // 变量 info
            ( // 语句
                i.title.clone(), // 语句
                i.copyright.clone(), // 语句
                i.date.clone(), // 语句
                i.url.clone(), // 语句
            ) // 块结束
        }); // 语句

        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(16.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 4.0), // 字段 offset
                blur: 16.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                let w = ui.available_width(); // 变量 w
                let h = preview_img_height(ui.ctx(), 0.5); // 变量 h
                let (rect, _) = // 调用 let
                    ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover()); // 语句
                { // 语句
                    let p = ui.painter(); // 变量 p
                    p.rect_filled(rect, Rounding::same(10.0), theme::extreme()); // 语句
                    match (tex_id, tex_size) { // 分支匹配
                        (Some(id), Some(ts)) => { // 匹配分支
                            let tw = ts[0] as f32; // 变量 tw
                            let th = ts[1] as f32; // 变量 th
                            if tw > 0.0 && th > 0.0 { // 条件分支
                                let s = (rect.width() / tw).min(rect.height() / th); // 变量 s
                                let size = Vec2::new(tw * s, th * s); // 变量 size
                                let ir = egui::Rect::from_center_size(rect.center(), size); // 变量 ir
                                p.image( // 语句
                                    id, // 语句
                                    ir, // 语句
                                    egui::Rect::from_min_max( // 调用 from_min_max
                                        egui::pos2(0.0, 0.0), // 字段 egui
                                        egui::pos2(1.0, 1.0), // 字段 egui
                                    ), // 语句
                                    Color32::WHITE, // 字段 Color32
                                ); // 块结束
                            } // 块结束
                        } // 块结束
                        _ => { // 匹配分支
                            let txt = if self.online_loading { // 变量 txt
                                "正在联网获取…" // 语句
                            } else { // 语句
                                "点击「获取今日壁纸」开始" // 语句
                            }; // 块结束
                            p.text( // 语句
                                rect.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                txt, // 语句
                                egui::FontId::proportional(14.0), // 字段 egui
                                theme::subtext(), // 字段 theme
                            ); // 块结束
                        } // 块结束
                    } // 块结束
                } // 块结束
                ui.add_space(12.0); // 语句

                // "了解有关此图片信息" info panel.
                egui::Frame::none() // 调用 none
                    .fill(theme::extreme()) // 语句
                    .rounding(Rounding::same(10.0)) // 语句
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)) // 语句
                    .show(ui, |ui| { // 语句
                        ui.horizontal(|ui| { // 语句
                            chip( // 调用 chip
                                ui, // 语句
                                "🔎 了解有关此图片信息", // 语句
                                theme::accent_soft(), // 字段 theme
                                theme::accent(), // 字段 theme
                            ); // 块结束
                            if let Some((title, _c, date, _u)) = &info { // 条件分支
                                ui.label( // 语句
                                    RichText::new(title).size(13.5).strong().color(theme::text()), // 字段 RichText
                                ); // 块结束
                                if !date.is_empty() { // 条件分支
                                    ui.label( // 语句
                                        RichText::new(date).size(11.0).color(theme::subtext()), // 字段 RichText
                                    ); // 块结束
                                } // 块结束
                            } else { // 语句
                                ui.label( // 语句
                                    RichText::new("暂无图片信息") // 调用 new
                                        .size(12.0) // 语句
                                        .color(theme::subtext()), // 语句
                                ); // 块结束
                            } // 块结束
                        }); // 语句
                        if let Some((_t, copyright, _d, _u)) = &info { // 条件分支
                            if !copyright.is_empty() { // 条件分支
                                ui.add_space(4.0); // 语句
                                ui.label( // 语句
                                    RichText::new(copyright) // 调用 new
                                        .size(11.0) // 语句
                                        .color(theme::subtext()), // 语句
                                ); // 块结束
                            } // 块结束
                        } // 块结束
                    }); // 语句
                ui.add_space(10.0); // 语句
                ui.horizontal(|ui| { // 语句
                    if primary_button(ui, "设为桌面壁纸").clicked() { // 条件分支
                        self.apply_online_wallpaper(); // 语句
                    } // 块结束
                    if ghost_button(ui, "设为锁屏").clicked() { // 条件分支
                        self.set_lock_from_online(); // 语句
                    } // 块结束
                    // Add / remove the fetched daily wallpaper from the slideshow.
                    if let Some(opath) = self.online_img.as_ref().map(|i| i.path.clone()) { // 条件分支
                        let in_pl = self.in_playlist(&opath); // 变量 in_pl
                        let lbl = if in_pl { "移出轮播" } else { "+ 加入轮播" }; // 变量 lbl
                        if ghost_button(ui, lbl) // 条件分支
                            .on_hover_text(if in_pl { // 语句
                                "从轮播清单移除这张每日壁纸" // 语句
                            } else { // 语句
                                "把这张每日壁纸加入轮播清单" // 语句
                            }) // 语句
                            .clicked() // 语句
                        { // 语句
                            if in_pl { // 条件分支
                                self.remove_from_playlist(&opath); // 语句
                            } else { // 语句
                                self.add_to_playlist(&opath); // 语句
                            } // 块结束
                        } // 块结束
                    } // 块结束
                    if let Some((_t, _c, _d, url)) = &info { // 条件分支
                        if ghost_button(ui, "打开图片页面").clicked() { // 条件分支
                            open_url(url); // 调用 open_url
                        } // 块结束
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if ghost_button(ui, &format!("👎 {}", self.cfg.online_dislikes)).clicked() { // 条件分支
                            self.cfg.online_dislikes += 1; // 语句
                            self.status = "已记录：不喜欢这张".to_string(); // 语句
                        } // 块结束
                        if ghost_button(ui, &format!("👍 {}", self.cfg.online_likes)).clicked() { // 条件分支
                            self.cfg.online_likes += 1; // 语句
                            self.status = "已记录：喜欢这张".to_string(); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
    } // 块结束

    // ---- UI sections ------------------------------------------------------
    // ---- 网上壁纸库（多站点） --------------------------------------------
    /// 载入当前站点分类 / 搜索结果的第 `page` 页（1 基）。后台线程取数据，
    /// 结果在 `update()` 里轮询。
    fn web_load(&mut self, page: u32) {
        // 允许新请求顶替进行中的请求：新接收端会替换旧的，旧结果自动作废。
        self.web_loading = true;
        if self.web_only_fav {
            // 收藏视图不联网，只展示本地收藏。
            self.web_loading = false;
            self.web_persist();
            return;
        }
        let page = page.max(1);
        let src = self.web_source.min(crate::websrc::SOURCES.len() - 1);
        let cat = self.web_category;
        let keyword = if self.web_search_active {
            self.web_query.trim().to_string()
        } else {
            String::new()
        };
        let sid = self.web_search_id;
        let name = crate::websrc::SOURCES[src].name;
        self.status = format!("正在加载{name}（第 {page} 页）…");
        let (tx, rx) = std::sync::mpsc::channel();
        self.web_list_rx = Some(rx);
        std::thread::spawn(move || {
            let r = crate::websrc::fetch(src, cat, page, &keyword, sid);
            let _ = tx.send(r);
        });
        self.web_persist();
    }

    /// 切换站点：重置分类 / 关键词并载入第一页。
    fn web_switch(&mut self, src: usize) {
        self.web_source = src.min(crate::websrc::SOURCES.len() - 1);
        self.web_category = 0;
        self.web_search_active = false;
        self.web_query.clear();
        self.web_search_id = None;
        self.web_preview_id = None;
        self.web_preview_tex = None;
        self.web_load(1);
        self.web_persist();
    }

    /// 后台取某张壁纸的原图（详情页 → 原图直链 → 下载到缓存）。
    fn web_prepare(&mut self, id: u32) {
        if self.web_busy || matches!(self.web_full.get(&id), Some(Some(_))) {
            return;
        }
        let Some(item) = self.web_view_items().into_iter().find(|i| i.id == id) else {
            return;
        };
        self.web_busy = true;
        self.status = format!("正在解析原图（#{id}）…");
        let cache = self.web_thumb.cache_dir().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        self.web_full_rx = Some(rx);
        std::thread::spawn(move || {
            let r = crate::websrc::full(&item, &cache);
            let _ = tx.send((id, r));
        });
    }

    /// 原图就绪后执行选定的动作。
    fn apply_web_full(&mut self, kind: WebApply, full: &crate::websrc::Full) {
        match kind {
            WebApply::Wallpaper => match wallpaper::set_wallpaper(&full.path, self.cfg.fit) {
                Ok(_) => self.status = format!("已设为桌面壁纸：{}", full.title),
                Err(e) => self.status = format!("设置失败：{e}"),
            },
            WebApply::Lock => self.apply_lock_screen(&full.path),
            WebApply::Save => self.web_save(full),
        }
    }

    /// 把已下载的 mp4 设为动态壁纸（自建桌面宿主 + mpv，Windows/macOS/Linux）。
    fn apply_live_wallpaper(&mut self, path: &Path) {
        // 进入动态壁纸前，先把视频首帧截出来铺成静态壁纸：mpv 初始化（约几百毫秒）
        // 期间桌面显示的就是这一帧，不会「闪过」上一张固定壁纸。
        if let Some(poster) = self.live_poster(path) {
            let _ = wallpaper::set_wallpaper(&poster, self.cfg.fit);
        }
        match crate::livewall::set_live_wallpaper(path) {
            Ok(_) => self.status = format!("已设为动态壁纸：{}", path.display()),
            Err(e) => self.status = format!("动态壁纸设置失败：{e}"),
        }
    }

    /// 动态壁纸「首帧海报」缓存路径：`<配置目录>/live_posters/<文件名>-<路径哈希>.png`。
    /// 加入路径哈希，避免不同目录下的同名视频共用缓存。
    fn live_poster_path(&self, video: &Path) -> PathBuf {
        let stem = video
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "poster".to_string());
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in video.to_string_lossy().as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        config_dir()
            .join("live_posters")
            .join(format!("{stem}-{:08x}.png", h as u32))
    }

    /// 已缓存的视频首帧海报路径；未缓存则返回 `None`。
    ///
    /// **不在这里截取**（mpv 截帧需几百毫秒到一秒，放 UI 线程会卡住轮播）；
    /// 截取统一交给后台预热 [`Self::precache_live_posters`]。
    fn live_poster(&self, video: &Path) -> Option<PathBuf> {
        let out = self.live_poster_path(video);
        out.is_file().then_some(out)
    }

    /// 后台为轮播清单里的视频预截首帧海报（单线程、尽力而为），
    /// 这样轮播轮到动态壁纸时可以「秒切」而不必临时调用 mpv 截帧。
    fn precache_live_posters(&mut self) {
        let jobs: Vec<(PathBuf, PathBuf)> = self
            .slide_paths()
            .into_iter()
            .filter(|p| crate::thumbs::is_video_file(p))
            .map(|p| { // 调用 map
                let out = self.live_poster_path(&p); // 变量 out
                (p, out) // 语句
            })
            .filter(|(_, out)| !out.is_file())
            .collect();
        if jobs.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            for (v, out) in jobs {
                let _ = crate::livewall::poster_frame(&v, &out);
            }
        });
    }

    /// 需要 mpv 的动态壁纸 / 锁屏动作：mpv 已就绪则直接执行；缺失时（Windows）先在
    /// 后台按需下载 mpv，下载完成后再执行 `after`。mpv 不随安装包分发。
    fn with_mpv_or_download(&mut self, after: AfterMpv) {
        if crate::livewall::mpv_available() || !crate::livewall::supported() {
            self.run_after_mpv(after);
            return;
        }
        self.mpv_after = Some(after);
        self.start_mpv_download();
    }

    /// 后台按需下载 mpv 到用户缓存目录（仅本机缺失时，第一次需要）。
    fn start_mpv_download(&mut self) {
        if self.mpv_dl_busy {
            return;
        }
        self.mpv_dl_busy = true;
        self.status = "正在准备动态壁纸所需的 mpv 播放器…（Windows 缺失时会按需下载）"
            .to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.mpv_dl_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(crate::livewall::ensure_mpv());
        });
    }

    /// 执行「下载 mpv 完成后」的待办动作。
    fn run_after_mpv(&mut self, after: AfterMpv) {
        match after {
            AfterMpv::LiveWallpaper(p) => self.apply_live_wallpaper(&p),
            AfterMpv::LockVideo(p) => self.set_lock_from_video(&p),
        }
    }

    /// 当前站点的保存目录：`图片/网上壁纸库/<站点>`。
    fn web_save_dir(&self) -> PathBuf {
        let src = self.web_source.min(crate::websrc::SOURCES.len() - 1);
        crate::config::home()
            .join("Pictures")
            .join("网上壁纸库")
            .join(crate::websrc::SOURCES[src].key)
    }

    /// 把原图另存到「图片/网上壁纸库/<站点>」目录。
    fn web_save(&mut self, full: &crate::websrc::Full) {
        let dir = self.web_save_dir();
        let _ = std::fs::create_dir_all(&dir);
        let name = full
            .path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("web_{}.jpg", full.id));
        let dest = dir.join(name);
        match std::fs::copy(&full.path, &dest) {
            Ok(_) => self.status = format!("已下载原图到 {}", dest.display()),
            Err(e) => self.status = format!("保存失败：{e}"),
        }
    }

    /// 下载动态壁纸（mp4）到「图片/网上壁纸库/<站点>」目录。
    /// `after` 决定下载完成后执行的动作：保存 / 设为动态壁纸 / 加入轮播。
    fn web_video_download(&mut self, url: String, name: String, after: WebVideoAfter) {
        if self.web_video_busy {
            return;
        }
        self.web_video_busy = true;
        self.web_video_after = after;
        let src = self.web_source.min(crate::websrc::SOURCES.len() - 1);
        let referer = crate::websrc::SOURCES[src].referer;
        let dir = self.web_save_dir();
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join(name);
        self.status = format!("正在下载动态壁纸…（{}）", dest.display());
        let (tx, rx) = std::sync::mpsc::channel();
        self.web_video_rx = Some(rx);
        std::thread::spawn(move || {
            let r = crate::websrc::download_to(&url, &dest, referer).map(|_| dest.clone());
            let _ = tx.send(r);
        });
    }

    /// 当前要展示的条目：收藏视图（可按关键词过滤）或当前站点列表。
    fn web_view_items(&self) -> Vec<crate::websrc::Item> {
        if self.web_only_fav {
            let kw = if self.web_search_active {
                self.web_query.trim().to_lowercase()
            } else {
                String::new()
            };
            self.cfg
                .web_favorites
                .iter()
                .filter(|f| kw.is_empty() || f.title.to_lowercase().contains(&kw))
                .map(|f| crate::websrc::Item {
                    id: f.id,
                    title: f.title.clone(),
                    thumb: f.thumb.clone(),
                    page: f.page.clone(),
                    src: crate::websrc::source_index_by_key(&f.key),
                    full_url: f.full_url.clone(),
                    video_url: f.video_url.clone(),
                })
                .collect()
        } else {
            self.web_items.clone()
        }
    }

    /// 该条目是否已收藏。
    fn web_is_fav(&self, it: &crate::websrc::Item) -> bool {
        let key = crate::websrc::SOURCES[it.src.min(crate::websrc::SOURCES.len() - 1)].key;
        self.cfg
            .web_favorites
            .iter()
            .any(|f| f.key == key && f.id == it.id)
    }

    /// 切换收藏并持久化。
    fn web_toggle_fav(&mut self, it: &crate::websrc::Item) {
        let key = crate::websrc::SOURCES[it.src.min(crate::websrc::SOURCES.len() - 1)].key;
        if let Some(pos) = self
            .cfg
            .web_favorites
            .iter()
            .position(|f| f.key == key && f.id == it.id)
        {
            self.cfg.web_favorites.remove(pos);
            self.status = format!("已取消收藏：{}", ellipsis(&it.title, 24));
        } else {
            self.cfg.web_favorites.push(crate::config::FavItem {
                key: key.to_string(),
                id: it.id,
                title: it.title.clone(),
                thumb: it.thumb.clone(),
                page: it.page.clone(),
                full_url: it.full_url.clone(),
                video_url: it.video_url.clone(),
            });
            self.status = format!(
                "已收藏（共 {} 条）：{}",
                self.cfg.web_favorites.len(),
                ellipsis(&it.title, 24)
            );
        }
        self.cfg.save();
    }

    /// 打开预览：优先用已下载原图的纹理，否则请求原图。
    fn web_open_preview(&mut self, ctx: &egui::Context, id: u32) {
        self.web_preview_id = Some(id);
        self.web_preview_tex = None;
        if let Some(Some(f)) = self.web_full.get(&id).cloned() {
            if let Some(t) = crate::thumbs::load_image(&f.path, 1600) {
                let ci = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba);
                self.web_preview_tex = Some(ctx.load_texture(
                    format!("webp:{}", f.path.display()),
                    ci,
                    egui::TextureOptions::LINEAR,
                ));
            }
        } else {
            self.web_prepare(id);
        }
    }

    /// 在当前列表 / 收藏内切换上一张 / 下一张预览。
    fn web_step(&mut self, ctx: &egui::Context, delta: i32) {
        let items = self.web_view_items();
        let Some(id) = self.web_preview_id else {
            return;
        };
        let Some(pos) = items.iter().position(|i| i.id == id) else {
            return;
        };
        let np = pos as i32 + delta;
        if np < 0 || np >= items.len() as i32 {
            return;
        }
        let nid = items[np as usize].id;
        self.web_open_preview(ctx, nid);
    }

    /// 清空缩略图磁盘缓存。
    fn web_clear_cache(&mut self) {
        let dir = self.web_thumb.cache_dir().to_path_buf();
        let mut n = 0usize;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if std::fs::remove_file(e.path()).is_ok() {
                    n += 1;
                }
            }
        }
        self.web_thumb.reset();
        self.web_thumbs.clear();
        self.web_thumb_order.clear();
        self.status = format!("已清理缩略图缓存（{n} 个文件）");
    }

    /// 持久化网上壁纸库的浏览状态。
    fn web_persist(&mut self) {
        let key = crate::websrc::SOURCES[self.web_source.min(crate::websrc::SOURCES.len() - 1)].key;
        self.cfg.web_source_key = key.to_string();
        self.cfg.web_category = self.web_category;
        self.cfg.web_query = self.web_query.clone();
        self.cfg.web_search_active = self.web_search_active;
        self.cfg.web_only_fav = self.web_only_fav;
        self.cfg.save();
    }

    /// 网上壁纸库缩略图纹理上限（滚动出屏的会被逐出并允许重新拉取）。
    fn enforce_web_tex_cap(&mut self) { // 函数 enforce_web_tex_cap
        const WEB_CAP: usize = 160; // 常量 WEB_CAP
        if self.web_thumb_order.len() <= WEB_CAP { // 条件分支
            return; // 返回
        } // 块结束
        let extra = self.web_thumb_order.len() - WEB_CAP; // 变量 extra
        let evicted: Vec<String> = self.web_thumb_order.drain(0..extra).collect(); // 变量 evicted
        for k in evicted { // 循环
            self.web_thumbs.remove(&k); // 语句
            self.web_thumb.forget(&k); // 语句
        } // 块结束
    } // 块结束

    fn ui_web4k(&mut self, ui: &mut egui::Ui) { // 函数 ui_web4k
        // 「预览 + 操作」常驻：点开大图后固定成独立区域（宽屏在右侧、窄屏在底部），
        // 主列的来源选择 / 工具条 / 网格各自滚动——上下滑动列表时预览与操作按钮始终存在，
        // 不会像以前那样随整页滚出屏幕、变成「一滑动就没了」。
        if let Some(id) = self.web_preview_id { // 条件分支
            if self.compact { // 条件分支
                egui::TopBottomPanel::bottom("web_preview") // 调用 bottom
                    .resizable(false) // 语句
                    .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, 6.0))) // 语句
                    .show_inside(ui, |ui| { // 语句
                        egui::ScrollArea::vertical() // 调用 vertical
                            .scroll_bar_visibility(egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible) // 语句
                            .max_height((ui.available_height() * 0.55).max(200.0)) // 语句
                            .show(ui, |ui| self.ui_web_preview(ui, id)); // 语句
                    }); // 语句
            } else { // 语句
                // 笔记本窄屏时预览栏收窄，给左侧列表留出更多列。
                let narrow = is_narrow(ui.ctx()); // 变量 narrow
                let frac = if narrow { 0.30 } else { 0.34 }; // 变量 frac
                let max_w = if narrow { 380.0 } else { 460.0 }; // 变量 max_w
                let w = (ui.available_width() * frac).clamp(280.0, max_w); // 变量 w
                egui::SidePanel::right("web_preview") // 调用 right
                    .resizable(false) // 语句
                    .exact_width(w) // 语句
                    .frame(egui::Frame::none().inner_margin(egui::Margin { left: 12.0, right: 0.0, top: 0.0, bottom: 0.0 })) // 语句
                    .show_inside(ui, |ui| { // 语句
                        vscroll() // 调用 vscroll
                            .show(ui, |ui| self.ui_web_preview(ui, id)); // 语句
                    }); // 语句
            } // 块结束
        } // 块结束

        // 主列单独滚动：来源选择 / 工具条 / 网格自上而下一起滚动（不再嵌套独立滚动区）。
        // 滚动条常驻可见：点开预览 / 操作后主列依然能上下滑动，不会「一操作就没有了」。
        vscroll() // 调用 vscroll
            .show(ui, |ui| { // 语句
                self.ui_web4k_body(ui); // 语句
                ui.add_space(24.0); // 底部留白，最后一行不被状态栏贴边
            }); // 语句
    } // 块结束

    fn ui_web4k_body(&mut self, ui: &mut egui::Ui) { // 函数 ui_web4k_body
        let src = self.web_source.min(crate::websrc::SOURCES.len() - 1);
        let s = &crate::websrc::SOURCES[src];
        ui.horizontal(|ui| { // 语句
            ui.label(RichText::new("网上壁纸库").size(20.0).strong().color(theme::text())); // 语句
            ui.add_space(8.0); // 语句
            chip(ui, &format!("{} · {}", s.name, s.host), theme::accent_soft(), theme::accent()); // 调用 chip
            ui.add_space(6.0); // 语句
            chip(ui, s.tag, theme::accent_soft(), theme::accent()); // 调用 chip
        }); // 语句
        ui.add_space(2.0); // 语句
        let note = if s.video { // 变量 note
            "联网浏览动态壁纸站：支持关键词搜索、按分类浏览、分页翻页；可下载封面设为桌面/锁屏，也可下载动态 MP4，或直接「设为动态壁纸」。" // 语句
        } else { // 语句
            "联网浏览高清壁纸：支持关键词搜索、按分类浏览、分页翻页；可下载原图、设为桌面壁纸或锁屏。" // 语句
        }; // 块结束
        ui.label( // 语句
            RichText::new(note) // 调用 new
                .size(11.5) // 语句
                .color(theme::subtext()), // 语句
        ); // 块结束
        ui.add_space(10.0); // 语句

        // 站点选择器（必应排第一）----------------------------------------
        ui.label(RichText::new("壁纸来源").size(10.5).color(theme::subtext())); // 语句
        ui.add_space(4.0); // 语句
        let mut switch_to: Option<usize> = None; // 变量 switch_to
        ui.horizontal_wrapped(|ui| { // 语句
            for (i, sc) in crate::websrc::SOURCES.iter().enumerate() { // 循环
                if segment(ui, self.web_source == i, sc.name) { // 条件分支
                    switch_to = Some(i); // 语句
                } // 块结束
            } // 块结束
        }); // 语句
        if let Some(i) = switch_to { // 条件分支
            if i != self.web_source { // 条件分支
                self.web_switch(i); // 语句
            } // 块结束
        } // 块结束
        ui.add_space(10.0); // 语句

        // 收藏 & 缓存工具 ------------------------------------------------
        ui.horizontal_wrapped(|ui| { // 语句
            let fav_n = self.cfg.web_favorites.len(); // 变量 fav_n
            let fav_label = if self.web_only_fav { // 变量 fav_label
                format!("★ 收藏中（{fav_n}）") // 语句
            } else { // 语句
                format!("☆ 仅看收藏（{fav_n}）") // 语句
            }; // 块结束
            if segment(ui, self.web_only_fav, &fav_label) { // 条件分支
                self.web_only_fav = !self.web_only_fav; // 语句
                self.web_preview_id = None; // 语句
                self.web_preview_tex = None; // 语句
                self.web_error = None; // 语句
                self.web_load(1); // 语句
                self.web_persist(); // 语句
            } // 块结束
            if ghost_button(ui, "清理缩略图缓存").clicked() { // 条件分支
                self.web_clear_cache(); // 语句
            } // 块结束
        }); // 语句
        ui.add_space(10.0); // 语句

        // 工具条：搜索 / 分类 / 分页 -------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 12.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.horizontal_wrapped(|ui| { // 语句
                    ui.label(RichText::new("🔍").size(13.0).color(theme::subtext())); // 语句
                    let w = if self.compact { ui.available_width() - 160.0 } else { 260.0 }; // 变量 w
                    egui::Frame::none() // 调用 none
                        .fill(theme::extreme()) // 语句
                        .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
                        .rounding(Rounding::same(8.0)) // 语句
                        .inner_margin(egui::Margin::symmetric(9.0, 4.0)) // 语句
                        .show(ui, |ui| { // 语句
                            ui.add( // 语句
                                egui::TextEdit::singleline(&mut self.web_query) // 调用 singleline
                                    .hint_text(if self.web_only_fav { // 调用 hint_text
                                        "在收藏中按标题筛选…" // 语句
                                    } else { // 语句
                                        "搜索关键词，如：风景 / 星空…" // 语句
                                    }) // 块结束
                                    .desired_width(w.max(120.0)) // 语句
                                    .frame(false), // 语句
                            ); // 块结束
                        }); // 语句
                    if primary_button(ui, "搜索").clicked() && !self.web_query.trim().is_empty() { // 条件分支
                        self.web_search_active = true; // 语句
                        self.web_search_id = None; // 语句
                        if self.web_only_fav { // 条件分支
                            self.web_persist(); // 语句
                        } // 块结束
                        self.web_load(1); // 语句
                    } // 块结束
                    if ghost_button(ui, "返回分类").clicked() { // 条件分支
                        self.web_search_active = false; // 语句
                        self.web_search_id = None; // 语句
                        self.web_query.clear(); // 语句
                        self.web_load(1); // 语句
                    } // 块结束
                    if self.web_loading { // 条件分支
                        chip(ui, "加载中…", theme::accent_soft(), theme::accent()); // 调用 chip
                    } // 块结束
                    if let Some(e) = self.web_error.clone() { // 条件分支
                        chip(ui, &format!("失败：{}", ellipsis(&e, 22)), theme::accent_soft(), theme::accent()); // 调用 chip
                        if ghost_button(ui, "重试").clicked() { // 条件分支
                            self.web_load(self.web_page.max(1)); // 语句
                        } // 块结束
                    } // 块结束
                }); // 语句
                ui.add_space(8.0); // 语句
                let cats = s.cats; // 变量 cats
                if cats.len() > 1 && !self.web_only_fav { // 条件分支
                    ui.horizontal_wrapped(|ui| { // 语句
                        for (i, c) in cats.iter().enumerate() { // 循环
                            let sel = self.web_category == i && !self.web_search_active; // 变量 sel
                            if segment(ui, sel, c.name) { // 条件分支
                                self.web_category = i; // 语句
                                self.web_search_active = false; // 语句
                                self.web_search_id = None; // 语句
                                self.web_preview_id = None; // 语句
                                self.web_preview_tex = None; // 语句
                                self.web_load(1); // 语句
                            } // 块结束
                        } // 块结束
                    }); // 语句
                } // 块结束
                ui.add_space(8.0); // 语句
                if self.web_only_fav { // 条件分支
                    ui.horizontal_wrapped(|ui| { // 语句
                        chip(ui, &format!("收藏 {} 条", self.cfg.web_favorites.len()), theme::accent_soft(), theme::accent()); // 调用 chip
                        if self.web_search_active { // 条件分支
                            chip(ui, &format!("筛选：{}", ellipsis(&self.web_query, 16)), theme::extreme(), theme::text()); // 调用 chip
                        } // 块结束
                    }); // 语句
                } else { // 语句
                ui.horizontal_wrapped(|ui| { // 语句
                    if ghost_button(ui, "‹ 上一页").clicked() && self.web_page > 1 { // 条件分支
                        self.web_load(self.web_page - 1); // 语句
                    } // 块结束
                    let page_label = if self.web_total_pages > 0 { // 变量 page_label
                        format!("第 {} / {} 页", self.web_page, self.web_total_pages) // 语句
                    } else { // 语句
                        format!("第 {} 页", self.web_page) // 语句
                    }; // 块结束
                    chip( // 调用 chip
                        ui, // 语句
                        &page_label, // 语句
                        theme::accent_soft(), // 字段 theme
                        theme::accent(), // 字段 theme
                    ); // 块结束
                    if ghost_button(ui, "下一页 ›").clicked() && self.web_has_more { // 条件分支
                        self.web_load(self.web_page + 1); // 语句
                    } // 块结束
                    chip( // 调用 chip
                        ui, // 语句
                        &format!("本页 {} 张", self.web_items.len()), // 语句
                        theme::extreme(), // 字段 theme
                        theme::text(), // 字段 theme
                    ); // 块结束
                    if self.web_search_active { // 条件分支
                        chip( // 调用 chip
                            ui, // 语句
                            &format!("搜索：{}", ellipsis(&self.web_query, 16)), // 语句
                            theme::accent_soft(), // 字段 theme
                            theme::accent(), // 字段 theme
                        ); // 块结束
                    } // 块结束
                    if ghost_button(ui, "刷新").clicked() { // 条件分支
                        self.web_load(self.web_page.max(1)); // 语句
                    } // 块结束
                }); // 语句
                } // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句

        // 预览 + 操作已移到主列之外常驻渲染（见 ui_web4k），这里只渲染网格。
        let items = self.web_view_items(); // 变量 items
        if items.is_empty() { // 条件分支
            ui.add_space(20.0); // 语句
            let msg = if self.web_loading { // 变量 msg
                "正在加载…" // 语句
            } else if self.web_only_fav { // 语句
                if self.web_search_active { // 条件分支
                    "收藏里没有匹配的条目" // 语句
                } else { // 语句
                    "还没有收藏，点卡片上的 ☆ 即可添加" // 语句
                } // 块结束
            } else { // 语句
                "没有结果，换个关键词或分类试试" // 语句
            }; // 块结束
            ui.label(RichText::new(msg).size(13.0).color(theme::subtext())); // 语句
            return; // 返回
        } // 块结束

        // 网格：直接跟随整页滚动（不再嵌套独立滚动区，避免出现「滑动即消失」）。
        let avail = ui.available_width(); // 变量 avail
        let cols = grid_cols(avail, self.compact); // 变量 cols
        let cell_w = grid_cell_w(avail, self.compact); // 变量 cell_w
        let mut idx = 0usize; // 变量 idx
        while idx < items.len() { // 循环
            ui.horizontal(|ui| { // 语句
                for _ in 0..cols { // 循环
                    if idx >= items.len() { // 条件分支
                        break; // 语句
                    } // 块结束
                    let it = items[idx].clone(); // 变量 it
                    self.web_card(ui, &it, cell_w); // 语句
                    idx += 1; // 语句
                } // 块结束
            }); // 语句
            ui.add_space(GAP); // 语句
        } // 块结束
    } // 块结束

    fn ui_web_preview(&mut self, ui: &mut egui::Ui, id: u32) { // 函数 ui_web_preview
        let view = self.web_view_items(); // 变量 view
        let item = view.iter().find(|i| i.id == id).cloned(); // 变量 item
        let fav = item.as_ref().map(|i| self.web_is_fav(i)).unwrap_or(false); // 变量 fav
        let pos = view.iter().position(|i| i.id == id); // 变量 pos
        let has_prev = matches!(pos, Some(p) if p > 0); // 变量 has_prev
        let has_next = matches!(pos, Some(p) if p + 1 < view.len()); // 变量 has_next
        let full = self.web_full.get(&id).cloned().flatten(); // 变量 full
        let tex = self.web_preview_tex.as_ref().map(|t| (t.id(), t.size())); // 变量 tex
        let busy = self.web_busy; // 变量 busy
        let mut pending_apply: Option<WebApply> = None; // 变量 pending_apply
        let mut close = false; // 变量 close
        let mut toggle_fav = false; // 变量 toggle_fav
        let mut toggle_playlist = false; // 变量 toggle_playlist

        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(16.0)) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                let w = ui.available_width(); // 变量 w
                // 侧栏 / 底部常驻面板里给更紧凑的预览高度（窄屏尤其要省空间）。
                let frac = if self.compact { 0.24 } else { 0.42 }; // 变量 frac
                let h = preview_img_height(ui.ctx(), frac); // 变量 h
                let (rect, _) = // 调用 let
                    ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover()); // 语句
                { // 语句
                    let p = ui.painter(); // 变量 p
                    p.rect_filled(rect, Rounding::same(10.0), theme::extreme()); // 语句
                    match tex { // 分支匹配
                        Some((tid, ts)) => { // 匹配分支
                            let tw = ts[0] as f32; // 变量 tw
                            let th = ts[1] as f32; // 变量 th
                            if tw > 0.0 && th > 0.0 { // 条件分支
                                let s = (rect.width() / tw).min(rect.height() / th); // 变量 s
                                let size = Vec2::new(tw * s, th * s); // 变量 size
                                let ir = egui::Rect::from_center_size(rect.center(), size); // 变量 ir
                                p.image( // 语句
                                    tid, // 语句
                                    ir, // 语句
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), // 字段 egui
                                    Color32::WHITE, // 字段 Color32
                                ); // 块结束
                            } // 块结束
                        } // 块结束
                        None => { // 匹配分支
                            p.text( // 语句
                                rect.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                if busy { "正在下载原图…" } else { "正在准备…" }, // 语句
                                egui::FontId::proportional(14.0), // 字段 egui
                                theme::subtext(), // 字段 theme
                            ); // 块结束
                        } // 块结束
                    } // 块结束
                } // 块结束
                ui.add_space(10.0); // 语句
                ui.horizontal_wrapped(|ui| { // 语句
                    let title = item.as_ref().map(|i| i.title.clone()).unwrap_or_default(); // 变量 title
                    ui.label( // 语句
                        RichText::new(ellipsis(&title, 46)).size(13.5).strong().color(theme::text()), // 字段 RichText
                    ); // 块结束
                    if let Some(f) = &full { // 条件分支
                        ui.label( // 语句
                            RichText::new(format!( // 调用 new
                                "#{} · {} KB", // 语句
                                f.id, // 语句
                                std::fs::metadata(&f.path).map(|m| m.len() / 1024).unwrap_or(0) // 语句
                            )) // 语句
                            .size(10.5) // 语句
                            .color(theme::subtext()), // 语句
                        ); // 块结束
                    } // 块结束
                }); // 语句
                ui.add_space(8.0); // 语句
                ui.horizontal_wrapped(|ui| { // 语句
                    if primary_button(ui, "设为桌面壁纸").clicked() { // 条件分支
                        if full.is_some() { // 条件分支
                            pending_apply = Some(WebApply::Wallpaper); // 语句
                        } else { // 语句
                            self.web_pending = Some((id, WebApply::Wallpaper)); // 语句
                            self.web_prepare(id); // 语句
                        } // 块结束
                    } // 块结束
                    if ghost_button(ui, "设为锁屏").clicked() { // 条件分支
                        if full.is_some() { // 条件分支
                            pending_apply = Some(WebApply::Lock); // 语句
                        } else { // 语句
                            self.web_pending = Some((id, WebApply::Lock)); // 语句
                            self.web_prepare(id); // 语句
                        } // 块结束
                    } // 块结束
                    if ghost_button(ui, "下载原图").clicked() { // 条件分支
                        if full.is_some() { // 条件分支
                            pending_apply = Some(WebApply::Save); // 语句
                        } else { // 语句
                            self.web_pending = Some((id, WebApply::Save)); // 语句
                            self.web_prepare(id); // 语句
                        } // 块结束
                    } // 块结束
                    if let Some(f) = &full { // 条件分支
                        if let (Some(url), Some(name)) = (f.video.clone(), f.video_name.clone()) { // 条件分支
                            if primary_button(ui, "下载动态壁纸 MP4").clicked() && !self.web_video_busy { // 条件分支
                                self.web_video_download(url.clone(), name.clone(), WebVideoAfter::Save); // 语句
                            } // 块结束
                            // 设为动态壁纸：已下载过就直接设置，否则先下载再自动设置。
                            if ghost_button(ui, "设为动态壁纸").clicked() && !self.web_video_busy { // 条件分支
                                let dest = self.web_save_dir().join(&name); // 变量 dest
                                if dest.is_file() { // 条件分支
                                    self.with_mpv_or_download(AfterMpv::LiveWallpaper(dest)); // 语句
                                } else { // 语句
                                    self.web_video_download(url.clone(), name.clone(), WebVideoAfter::Live); // 语句
                                } // 块结束
                            } // 块结束
                            // 设为锁屏：Windows 锁屏只能设置静态图片，这里从视频截取一帧作为锁屏海报。
                            if ghost_button(ui, "设为锁屏(动态)").clicked() && !self.web_video_busy { // 条件分支
                                let dest = self.web_save_dir().join(&name); // 变量 dest
                                if dest.is_file() { // 条件分支
                                    self.with_mpv_or_download(AfterMpv::LockVideo(dest)); // 语句
                                } else { // 语句
                                    self.web_video_download(url.clone(), name.clone(), WebVideoAfter::Lock); // 语句
                                } // 块结束
                            } // 块结束
                            // 用「原视频」加入轮播：轮播轮到该条目时以 mpv 播放动态壁纸（Windows）。
                            let dest = self.web_save_dir().join(&name); // 变量 dest
                            let in_pl = self.in_playlist(&dest); // 变量 in_pl
                            let lbl = if in_pl { "移出轮播(原视频)" } else { "＋ 轮播(原视频)" }; // 变量 lbl
                            if ghost_button(ui, lbl).clicked() && !self.web_video_busy { // 条件分支
                                if in_pl { // 条件分支
                                    self.remove_from_playlist(&dest); // 语句
                                } else if dest.is_file() { // 条件分支
                                    self.add_to_playlist(&dest); // 语句
                                    self.status = "原视频已加入轮播清单".to_string(); // 语句
                                } else { // 语句
                                    self.web_video_download(url.clone(), name.clone(), WebVideoAfter::Playlist); // 语句
                                } // 块结束
                            } // 块结束
                        } // 块结束
                    } // 块结束
                    if crate::livewall::active_video().is_some() { // 条件分支
                        if ghost_button(ui, "停止动态壁纸").clicked() { // 条件分支
                            let _ = crate::livewall::stop(); // 变量 _
                            self.status = "已停止动态壁纸".to_string(); // 语句
                        } // 块结束
                    } // 块结束
                    if self.web_video_busy { // 条件分支
                        chip(ui, "动态壁纸下载中…", theme::accent_soft(), theme::accent()); // 调用 chip
                    } // 块结束
                    if self.mpv_dl_busy { // 条件分支
                        chip(ui, "正在下载 mpv…", theme::accent_soft(), theme::accent()); // 调用 chip
                    } // 块结束
                    if let Some(it) = &item { // 条件分支
                        if ghost_button(ui, "打开图片页面").clicked() { // 条件分支
                            open_url(&it.page); // 调用 open_url
                        } // 块结束
                    } // 块结束
                    if star_button(ui, fav) { // 条件分支
                        toggle_fav = true; // 语句
                    } // 块结束
                    if let Some(f) = &full { // 条件分支
                        let in_pl = self.in_playlist(&f.path); // 变量 in_pl
                        // 封面原图加入轮播；动态条目另有「＋ 轮播(原视频)」按钮。
                        let lbl = if in_pl { "移出轮播(原图)" } else { "＋ 轮播(原图)" }; // 变量 lbl
                        if ghost_button(ui, lbl).clicked() { // 条件分支
                            toggle_playlist = true; // 语句
                        } // 块结束
                    } // 块结束
                    if ghost_button(ui, "‹ 上一张").clicked() && has_prev { // 条件分支
                        self.web_step(&ui.ctx().clone(), -1); // 语句
                    } // 块结束
                    if ghost_button(ui, "下一张 ›").clicked() && has_next { // 条件分支
                        self.web_step(&ui.ctx().clone(), 1); // 语句
                    } // 块结束
                    if ghost_button(ui, "关闭预览").clicked() { // 条件分支
                        close = true; // 语句
                    } // 块结束
                    if full.is_none() { // 条件分支
                        ui.label( // 语句
                            RichText::new("原图下载中，完成后自动执行所选操作") // 调用 new
                                .size(10.5) // 语句
                                .color(theme::subtext()), // 语句
                        ); // 块结束
                    } // 块结束
                }); // 语句
            }); // 语句

        if toggle_fav { // 条件分支
            if let Some(it) = &item { // 条件分支
                self.web_toggle_fav(it); // 语句
            } // 块结束
        } // 块结束
        if toggle_playlist { // 条件分支
            if let Some(f) = self.web_full.get(&id).cloned().flatten() { // 条件分支
                if self.in_playlist(&f.path) { // 条件分支
                    self.remove_from_playlist(&f.path); // 语句
                } else { // 语句
                    self.add_to_playlist(&f.path); // 语句
                } // 块结束
            } // 块结束
        } // 块结束
        if close { // 条件分支
            self.web_preview_id = None; // 语句
            self.web_preview_tex = None; // 语句
        } // 块结束
        if let Some(kind) = pending_apply { // 条件分支
            if let Some(Some(f)) = self.web_full.get(&id).cloned() { // 条件分支
                self.apply_web_full(kind, &f); // 语句
            } // 块结束
        } // 块结束
    } // 块结束

    fn web_card(&mut self, ui: &mut egui::Ui, it: &crate::websrc::Item, cell_w: f32) { // 函数 web_card
        let referer = crate::websrc::SOURCES[it.src.min(crate::websrc::SOURCES.len() - 1)].referer; // 变量 referer
        let img_h = (cell_w * 0.6).round().max(72.0); // 变量 img_h
        let selected = self.web_preview_id == Some(it.id); // 变量 selected
        let stroke = if selected { // 变量 stroke
            Stroke::new(2.0_f32, theme::accent()) // 调用 new
        } else { // 语句
            Stroke::new(1.0_f32, theme::border()) // 调用 new
        }; // 块结束
        let inner = egui::Frame::none() // 变量 inner
            .fill(theme::card()) // 语句
            .stroke(stroke) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .inner_margin(egui::Margin::same(CARD_PAD + 2.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.vertical(|ui| { // 语句
                    ui.set_min_width(cell_w); // 语句
                    ui.set_max_width(cell_w); // 语句
                    let (rect, resp) = // 调用 let
                        ui.allocate_exact_size(Vec2::new(cell_w, img_h), egui::Sense::click()); // 语句
                    if ui.is_rect_visible(rect) // 条件分支
                        && !self.web_thumbs.contains_key(&it.thumb) // 语句
                        && !self.web_thumb.is_requested(&it.thumb) // 语句
                    { // 语句
                        self.web_thumb.request(&it.thumb, referer); // 语句
                    } // 块结束
                    { // 语句
                        let painter = ui.painter(); // 变量 painter
                        painter.rect_filled(rect, Rounding::same(9.0), theme::extreme()); // 语句
                        match self.web_thumbs.get(&it.thumb) { // 分支匹配
                            Some(Some(tex)) => { // 匹配分支
                                let ts = tex.size(); // 变量 ts
                                let tw = ts[0] as f32; // 变量 tw
                                let th = ts[1] as f32; // 变量 th
                                if tw > 0.0 && th > 0.0 { // 条件分支
                                    let s = (rect.width() / tw).min(rect.height() / th); // 变量 s
                                    let size = Vec2::new(tw * s, th * s); // 变量 size
                                    let ir = egui::Rect::from_center_size(rect.center(), size); // 变量 ir
                                    painter.image( // 语句
                                        tex.id(), // 语句
                                        ir, // 语句
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), // 字段 egui
                                        Color32::WHITE, // 字段 Color32
                                    ); // 块结束
                                } // 块结束
                            } // 块结束
                            Some(None) => { // 匹配分支
                                painter.text( // 语句
                                    rect.center(), // 语句
                                    egui::Align2::CENTER_CENTER, // 字段 egui
                                    "无法加载", // 语句
                                    egui::FontId::proportional(12.0), // 字段 egui
                                    theme::subtext(), // 字段 theme
                                ); // 块结束
                            } // 块结束
                            None => { // 匹配分支
                                painter.text( // 语句
                                    rect.center(), // 语句
                                    egui::Align2::CENTER_CENTER, // 字段 egui
                                    "载入中…", // 语句
                                    egui::FontId::proportional(12.0), // 字段 egui
                                    theme::subtext(), // 字段 theme
                                ); // 块结束
                            } // 块结束
                        } // 块结束
                        if it.video_url.is_some() { // 条件分支
                            let badge = egui::Rect::from_min_size(rect.left_top() + Vec2::new(6.0, 6.0), Vec2::new(48.0, 18.0)); // 变量 badge
                            painter.rect_filled(badge, Rounding::same(9.0), theme::accent()); // 语句
                            painter.text(badge.center(), egui::Align2::CENTER_CENTER, "▶ MP4", egui::FontId::proportional(10.5), Color32::WHITE); // 语句
                        } // 块结束
                        if resp.hovered() { // 条件分支
                            painter.rect_filled(rect, Rounding::same(9.0), Color32::from_black_alpha(70)); // 语句
                            let pill = egui::Rect::from_center_size(rect.center(), Vec2::new(104.0, 30.0)); // 变量 pill
                            painter.rect_filled(pill, Rounding::same(15.0), theme::accent()); // 语句
                            painter.text( // 语句
                                pill.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "查看 / 操作", // 语句
                                egui::FontId::proportional(12.5), // 字段 egui
                                Color32::WHITE, // 字段 Color32
                            ); // 块结束
                        } // 块结束
                    } // 块结束
                    if resp.hovered() { // 条件分支
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
                    } // 块结束
                    if resp.clicked() { // 条件分支
                        let ctx = ui.ctx().clone(); // 变量 ctx
                        self.web_open_preview(&ctx, it.id); // 语句
                    } // 块结束
                    ui.add_space(8.0); // 语句
                    ui.add(egui::Label::new( // 语句
                        RichText::new(ellipsis(&it.title, 18)) // 调用 new
                            .size(12.0) // 语句
                            .strong() // 语句
                            .color(theme::text()), // 语句
                    )); // 语句
                    ui.add_space(3.0); // 语句
                    ui.horizontal_wrapped(|ui| { // 语句
                        egui::Frame::none() // 调用 none
                            .fill(theme::extreme()) // 语句
                            .rounding(Rounding::same(6.0)) // 语句
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0)) // 语句
                            .show(ui, |ui| { // 语句
                                ui.label(RichText::new(format!("#{}", it.id)).size(9.5).color(theme::subtext())); // 语句
                            }); // 语句
                        let fav = self.web_is_fav(it); // 变量 fav
                        if star_button(ui, fav) { // 条件分支
                            self.web_toggle_fav(it); // 语句
                        } // 块结束
                        if selected { // 条件分支
                            ui.label(RichText::new("● 预览中").size(11.0).color(theme::accent())); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
        if inner.response.hovered() && !selected { // 条件分支
            ui.painter().rect_stroke( // 语句
                inner.response.rect, // 语句
                Rounding::same(14.0), // 字段 Rounding
                Stroke::new(1.5_f32, theme::accent()), // 字段 Stroke
            ); // 块结束
        } // 块结束
    } // 块结束

    fn draw_header(&self, ctx: &egui::Context, actions: &mut Vec<Action>) { // 函数 draw_header
        let (mx, my) = if self.compact { // 调用 let
            (12.0, 9.0) // 语句
        } else if is_narrow(ctx) { // 条件分支
            (14.0, 10.0) // 语句
        } else { // 语句
            (20.0, 13.0) // 语句
        }; // 块结束
        egui::TopBottomPanel::top("hdr") // 调用 top
            .frame( // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::surface()) // 语句
                    .inner_margin(egui::Margin::symmetric(mx, my)), // 语句
            ) // 块结束
            .show(ctx, |ui| { // 语句
                let full = ui.max_rect(); // 变量 full
                ui.horizontal(|ui| { // 语句
                    if !self.compact { // 条件分支
                        if ghost_button(ui, "☰").on_hover_text("显示 / 隐藏侧栏").clicked() { // 条件分支
                            actions.push(Action::ToggleSidebar); // 语句
                        } // 块结束
                        ui.add_space(2.0); // 语句
                    } // 块结束
                    // Brand mark
                    let logo_size = if self.compact { 34.0 } else { 40.0 }; // 变量 logo_size
                    let (logo, _) = // 调用 let
                        ui.allocate_exact_size(egui::vec2(logo_size, logo_size), egui::Sense::hover()); // 语句
                    ui.painter().rect_filled(logo, Rounding::same(12.0), theme::accent()); // 语句
                    ui.painter().text( // 语句
                        logo.center(), // 语句
                        egui::Align2::CENTER_CENTER, // 字段 egui
                        "🖼", // 语句
                        egui::FontId::proportional(if self.compact { 16.0 } else { 20.0 }), // 字段 egui
                        Color32::WHITE, // 字段 Color32
                    ); // 块结束
                    ui.add_space(if self.compact { 8.0 } else { 2.0 }); // 语句
                    ui.vertical(|ui| { // 语句
                        ui.label( // 语句
                            RichText::new("Wallpaper Studio") // 调用 new
                                .size(if self.compact { 14.5 } else { 18.0 }) // 语句
                                .strong() // 语句
                                .color(theme::text()), // 语句
                        ); // 块结束
                        if !self.compact { // 条件分支
                            ui.label( // 语句
                                RichText::new("桌面壁纸 · 固定与轮播") // 调用 new
                                    .size(11.0) // 语句
                                    .color(theme::subtext()), // 语句
                            ); // 块结束
                        } // 块结束
                    }); // 语句
                    ui.add_space(if self.compact { 6.0 } else { 10.0 }); // 语句
                    let mode = if self.cfg.mode == Mode::Slideshow { "轮播" } else { "固定" }; // 变量 mode
                    chip(ui, mode, theme::accent_soft(), theme::accent()); // 调用 chip
                    if !self.compact && self.cfg.mode == Mode::Slideshow { // 条件分支
                        if self.cfg.running { // 条件分支
                            chip(ui, "● 运行中", theme::accent_soft(), theme::ok()); // 调用 chip
                        } else { // 语句
                            chip(ui, "已暂停", theme::extreme(), theme::subtext()); // 调用 chip
                        } // 块结束
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if self.compact { // 条件分支
                            // Theme toggle lives in the header on mobile (the
                            // sidebar, where it normally sits, is hidden).
                            let dark = self.cfg.theme == 1; // 变量 dark
                            let lbl = if dark { "☀" } else { "🌙" }; // 变量 lbl
                            let btn = ui.add( // 变量 btn
                                egui::Button::new(RichText::new(lbl).size(16.0)) // 调用 new
                                    .frame(false) // 语句
                                    .min_size(egui::vec2(38.0, 34.0)), // 语句
                            ); // 块结束
                            if btn.on_hover_text("切换浅色 / 深色主题").clicked() { // 条件分支
                                actions.push(Action::ToggleTheme); // 语句
                            } // 块结束
                        } else if let Some(p) = self.current_path() { // 条件分支
                            let name = p // 变量 name
                                .file_name() // 语句
                                .map(|s| s.to_string_lossy().to_string()) // 语句
                                .unwrap_or_default(); // 语句
                            egui::Frame::none() // 调用 none
                                .fill(theme::card()) // 语句
                                .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
                                .rounding(Rounding::same(20.0)) // 语句
                                .inner_margin(egui::Margin::symmetric(12.0, 5.0)) // 语句
                                .show(ui, |ui| { // 语句
                                    ui.label( // 语句
                                        RichText::new(format!("🖼  {}", ellipsis(&name, 34))) // 调用 new
                                            .size(11.5) // 语句
                                            .color(theme::subtext()), // 语句
                                    ); // 块结束
                                }); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
                // hairline under the header
                ui.painter().line_segment( // 语句
                    [ // 语句
                        egui::pos2(full.left(), full.bottom()), // 字段 egui
                        egui::pos2(full.right(), full.bottom()), // 字段 egui
                    ], // 语句
                    Stroke::new(1.0_f32, theme::border()), // 字段 Stroke
                ); // 块结束
            }); // 语句
    } // 块结束

    fn draw_sidebar(&mut self, ctx: &egui::Context) { // 函数 draw_sidebar
        if self.cfg.sidebar_hidden { // 条件分支
            return; // 返回
        } // 块结束
        egui::SidePanel::left("nav") // 调用 left
            .resizable(false) // 语句
            .exact_width(if is_narrow(ctx) { 188.0 } else { 214.0 }) // 语句
            .frame( // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::surface()) // 语句
                    .inner_margin(egui::Margin::symmetric(12.0, 14.0)), // 语句
            ) // 块结束
            .show(ctx, |ui| { // 语句
                ui.add_space(2.0); // 语句
                ui.label(RichText::new("工作区").size(10.5).color(theme::subtext())); // 语句
                ui.add_space(6.0); // 语句
                if nav_button(ui, "🌉", "网上壁纸库", self.tab == Tab::Web4k) { // 条件分支
                    self.tab = Tab::Web4k; // 语句
                } // 块结束
                ui.add_space(3.0); // 语句
                if nav_button(ui, "🖼", "本地壁纸库", self.tab == Tab::Library) { // 条件分支
                    self.tab = Tab::Library; // 语句
                } // 块结束
                ui.add_space(3.0); // 语句
                if nav_button(ui, "🎞", "轮播", self.tab == Tab::Slideshow) { // 条件分支
                    self.tab = Tab::Slideshow; // 语句
                } // 块结束
                ui.add_space(3.0); // 语句
                if nav_button(ui, "🌐", "每日壁纸", self.tab == Tab::Online) { // 条件分支
                    self.tab = Tab::Online; // 语句
                } // 块结束
                ui.add_space(3.0); // 语句
                if nav_button(ui, "⚙", "设置", self.tab == Tab::Settings) { // 条件分支
                    self.tab = Tab::Settings; // 语句
                } // 块结束
                ui.add_space(3.0); // 语句
                if nav_button(ui, "ℹ", "关于", self.tab == Tab::About) { // 条件分支
                    self.tab = Tab::About; // 语句
                } // 块结束
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| { // 语句
                    ui.add_space(4.0); // 语句
                    ui.label( // 语句
                        RichText::new(format!("v{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS)) // 调用 new
                            .size(10.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                    ui.add_space(4.0); // 语句
                    ui.separator(); // 语句
                    ui.add_space(6.0); // 语句
                    let dark = self.cfg.theme == 1; // 变量 dark
                    let label = if dark { "☀  浅色主题" } else { "🌙  深色主题" }; // 变量 label
                    if theme_toggle(ui, label) { // 条件分支
                        self.cfg.theme = if dark { 0 } else { 1 }; // 语句
                    } // 块结束
                }); // 语句
            }); // 语句
    } // 块结束

    /// Compact bottom navigation bar (phones/tablets, narrow windows).
    fn draw_bottom_nav(&mut self, ctx: &egui::Context) { // 函数 draw_bottom_nav
        egui::TopBottomPanel::bottom("mobilenav") // 调用 bottom
            .exact_height(66.0) // 语句
            .frame( // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::surface()) // 语句
                    .inner_margin(egui::Margin::symmetric(6.0, 6.0)), // 语句
            ) // 块结束
            .show(ctx, |ui| { // 语句
                // hairline on top edge
                let r = ui.max_rect(); // 变量 r
                ui.painter().line_segment( // 语句
                    [r.left_top(), r.right_top()], // 语句
                    Stroke::new(1.0_f32, theme::border()), // 字段 Stroke
                ); // 块结束
                let items: [(Tab, &str, &str); 6] = [ // 变量 items
                    (Tab::Web4k, "🌉", "网上"), // 语句
                    (Tab::Library, "🖼", "本地"), // 语句
                    (Tab::Slideshow, "🎞", "轮播"), // 语句
                    (Tab::Online, "🌐", "每日"), // 语句
                    (Tab::Settings, "⚙", "设置"), // 语句
                    (Tab::About, "ℹ", "关于"), // 语句
                ]; // 语句
                let w = ui.available_width() / items.len() as f32; // 变量 w
                ui.horizontal(|ui| { // 语句
                    ui.spacing_mut().item_spacing.x = 0.0; // 语句
                    for (tab, icon, label) in items { // 循环
                        if bottom_nav_button(ui, icon, label, self.tab == tab, w) { // 条件分支
                            self.tab = tab; // 语句
                        } // 块结束
                    } // 块结束
                }); // 语句
            }); // 语句
    } // 块结束

    fn draw_status_bar(&self, ctx: &egui::Context) { // 函数 draw_status_bar
        egui::TopBottomPanel::bottom("status") // 调用 bottom
            .frame( // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::card()) // 语句
                    .inner_margin(egui::Margin::symmetric(18.0, 6.0)), // 语句
            ) // 块结束
            .show(ctx, |ui| { // 语句
                ui.horizontal(|ui| { // 语句
                    ui.label(RichText::new(&self.status).size(12.0).color(theme::subtext())); // 语句
                    if !self.compact { // 条件分支
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                            let hint = if self.hotkey.is_registered() { // 变量 hint
                                format!("{} 后台 / 唤回", self.hotkey.label()) // 语句
                            } else { // 语句
                                "全局快捷键不可用".to_string() // 语句
                            }; // 块结束
                            ui.label(RichText::new(hint).size(11.0).color(theme::subtext())); // 语句
                        }); // 语句
                    } // 块结束
                }); // 语句
            }); // 语句
    } // 块结束

    fn draw_center(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) { // 函数 draw_center
        let cx = if is_narrow(ctx) { 12.0 } else { 18.0 }; // 变量 cx
        egui::CentralPanel::default() // 调用 default
            .frame( // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::bg()) // 语句
                    .inner_margin(egui::Margin::symmetric(cx, 14.0)), // 语句
            ) // 块结束
            .show(ctx, |ui| { // 语句
                theme::paint_bg_gradient(ui, ui.max_rect()); // 调用 paint_bg_gradient
                // 左侧菜单选中的任意页面，右侧内容都可在垂直方向滑动浏览。
                match self.tab { // 分支匹配
                    // 「网上壁纸库」页面自带滚动区域，单独渲染，避免嵌套滚动。
                    Tab::Web4k => self.ui_web4k(ui), // 匹配分支
                    // 其余页面统一包一层可垂直滚动区域，并在底部补留白。
                    _ => { // 语句
                        vscroll() // 调用 vscroll
                            .show(ui, |ui| { // 语句
                                match self.tab { // 分支匹配
                                    Tab::Library => self.ui_library(ui, actions), // 匹配分支
                                    Tab::Slideshow => self.ui_slideshow(ui, actions), // 匹配分支
                                    Tab::Online => self.ui_online(ui), // 匹配分支
                                    Tab::Settings => self.ui_settings(ui, actions), // 匹配分支
                                    Tab::About => self.ui_about(ui), // 匹配分支
                                    Tab::Web4k => {} // 已在上面单独渲染
                                } // 块结束
                                ui.add_space(24.0); // 底部留白，最后一项不被状态栏贴边
                            }); // 语句
                    } // 块结束
                } // 块结束
            }); // 语句
    } // 块结束

    fn ui_library(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 ui_library
        let folder = self // 变量 folder
            .cfg // 语句
            .folder // 语句
            .as_ref() // 语句
            .map(|p| p.display().to_string()) // 语句
            .unwrap_or_else(|| "（未选择文件夹）".to_string()); // 语句
        let total = self.images.len(); // 变量 total
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 2.0), // 字段 offset
                blur: 10.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 13.0)) // 语句
            .show(ui, |ui| { // 语句
                let folder_hint = if self.compact { 30 } else { 54 }; // 变量 folder_hint
                // Folder path chip (own row on compact, inline on desktop).
                if self.compact { // 条件分支
                    ui.horizontal_wrapped(|ui| { // 语句
                        ui.label(RichText::new("📁").size(15.0).color(theme::accent())); // 语句
                        folder_chip(ui, &ellipsis(&folder, folder_hint)); // 调用 folder_chip
                    }); // 语句
                    ui.add_space(8.0); // 语句
                    ui.horizontal_wrapped(|ui| { // 语句
                        if primary_button(ui, "选择文件夹…").clicked() { // 条件分支
                            actions.push(Action::PickFolder); // 语句
                        } // 块结束
                        if ghost_button(ui, "全盘扫描").clicked() { // 条件分支
                            actions.push(Action::ScanAllDisks); // 语句
                        } // 块结束
                        if ghost_button(ui, "重新扫描").clicked() { // 条件分支
                            actions.push(Action::Rescan); // 语句
                        } // 块结束
                        if ghost_button(ui, "打开目录").clicked() { // 条件分支
                            actions.push(Action::OpenFolder); // 语句
                        } // 块结束
                    }); // 语句
                } else { // 语句
                    ui.horizontal(|ui| { // 语句
                        ui.label(RichText::new("📁").size(15.0).color(theme::accent())); // 语句
                        folder_chip(ui, &ellipsis(&folder, folder_hint)); // 调用 folder_chip
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                            if primary_button(ui, "选择文件夹…").clicked() { // 条件分支
                                actions.push(Action::PickFolder); // 语句
                            } // 块结束
                            if ghost_button(ui, "全盘扫描").clicked() { // 条件分支
                                actions.push(Action::ScanAllDisks); // 语句
                            } // 块结束
                            if ghost_button(ui, "重新扫描").clicked() { // 条件分支
                                actions.push(Action::Rescan); // 语句
                            } // 块结束
                            if ghost_button(ui, "打开目录").clicked() { // 条件分支
                                actions.push(Action::OpenFolder); // 语句
                            } // 块结束
                        }); // 语句
                    }); // 语句
                } // 块结束
                ui.add_space(11.0); // 语句
                // Filters + search. On compact screens the checkboxes, the
                // search field and the count chip each get their own row so
                // nothing overflows the narrow viewport.
                let only_label = if self.compact { "仅壁纸尺寸" } else { "仅壁纸尺寸(≥1280×720)" }; // 变量 only_label
                let search_w = if self.compact { ui.available_width() - 24.0 } else { 210.0 }; // 变量 search_w
                if self.compact { // 条件分支
                    ui.horizontal_wrapped(|ui| { // 语句
                        let mut rc = self.cfg.recursive; // 变量 rc
                        if ui.checkbox(&mut rc, "包含子文件夹").changed() { // 条件分支
                            self.cfg.recursive = rc; // 语句
                            actions.push(Action::Rescan); // 语句
                        } // 块结束
                        ui.add_space(8.0); // 语句
                        let mut wo = self.cfg.wallpaper_only; // 变量 wo
                        if ui.checkbox(&mut wo, only_label).changed() { // 条件分支
                            self.cfg.wallpaper_only = wo; // 语句
                            actions.push(Action::Rescan); // 语句
                        } // 块结束
                    }); // 语句
                    ui.add_space(6.0); // 语句
                    ui.horizontal(|ui| { // 语句
                        ui.label(RichText::new("🔍").size(13.0).color(theme::subtext())); // 语句
                        search_field(ui, &mut self.cfg.search, search_w.max(120.0)); // 调用 search_field
                    }); // 语句
                    ui.add_space(6.0); // 语句
                    ui.horizontal(|ui| { // 语句
                        chip( // 调用 chip
                            ui, // 语句
                            &format!("共 {total} 张"), // 语句
                            theme::accent_soft(), // 字段 theme
                            theme::accent(), // 字段 theme
                        ); // 块结束
                        if self.scanning { // 条件分支
                            chip(ui, "扫描中…", theme::accent_soft(), theme::accent()); // 调用 chip
                        } // 块结束
                    }); // 语句
                } else { // 语句
                    ui.horizontal_wrapped(|ui| { // 语句
                        let mut rc = self.cfg.recursive; // 变量 rc
                        if ui.checkbox(&mut rc, "包含子文件夹").changed() { // 条件分支
                            self.cfg.recursive = rc; // 语句
                            actions.push(Action::Rescan); // 语句
                        } // 块结束
                        ui.add_space(8.0); // 语句
                        let mut wo = self.cfg.wallpaper_only; // 变量 wo
                        if ui // 条件分支
                            .checkbox(&mut wo, only_label) // 语句
                            .on_hover_text("过滤掉图标、小图、竖图等不适合做壁纸的图片") // 语句
                            .changed() // 语句
                        { // 语句
                            self.cfg.wallpaper_only = wo; // 语句
                            actions.push(Action::Rescan); // 语句
                        } // 块结束
                        ui.add_space(8.0); // 语句
                        ui.label(RichText::new("🔍").size(13.0).color(theme::subtext())); // 语句
                        search_field(ui, &mut self.cfg.search, search_w); // 调用 search_field
                        ui.add_space(8.0); // 语句
                        chip( // 调用 chip
                            ui, // 语句
                            &format!("共 {total} 张"), // 语句
                            theme::accent_soft(), // 字段 theme
                            theme::accent(), // 字段 theme
                        ); // 块结束
                        if self.scanning { // 条件分支
                            chip(ui, "扫描中…", theme::accent_soft(), theme::accent()); // 调用 chip
                        } // 块结束
                    }); // 语句
                } // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句

        // Permission banner: folders the last scan couldn't read.
        if !self.permission_issues.is_empty() { // 条件分支
            self.ui_permission_banner(ui, actions); // 语句
            ui.add_space(12.0); // 语句
        } // 块结束

        if self.images.is_empty() { // 条件分支
            self.empty_state(ui, actions); // 语句
            return; // 返回
        } // 块结束

        let q = self.cfg.search.trim().to_lowercase(); // 变量 q
        let matches = |p: &PathBuf| { // 变量 matches
            q.is_empty() // 语句
                || p.file_name() // 语句
                    .map(|n| n.to_string_lossy().to_lowercase().contains(&q)) // 语句
                    .unwrap_or(false) // 语句
        }; // 块结束

        // Toolbar: album count + expand/collapse all.
        let album_label = // 变量 album_label
            format!("{} 个相册 · 按文件夹分类", self.albums.len()); // 语句
        if self.compact { // 条件分支
            ui.horizontal_wrapped(|ui| { // 语句
                ui.label(RichText::new(album_label).size(12.0).color(theme::subtext())); // 语句
                if ghost_button(ui, "全部折叠").clicked() { // 条件分支
                    actions.push(Action::CollapseAllAlbums); // 语句
                } // 块结束
                if ghost_button(ui, "全部展开").clicked() { // 条件分支
                    actions.push(Action::ExpandAllAlbums); // 语句
                } // 块结束
            }); // 语句
        } else { // 语句
            ui.horizontal(|ui| { // 语句
                ui.label(RichText::new(album_label).size(12.0).color(theme::subtext())); // 语句
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                    if ghost_button(ui, "全部折叠").clicked() { // 条件分支
                        actions.push(Action::CollapseAllAlbums); // 语句
                    } // 块结束
                    if ghost_button(ui, "全部展开").clicked() { // 条件分支
                        actions.push(Action::ExpandAllAlbums); // 语句
                    } // 块结束
                }); // 语句
            }); // 语句
        } // 块结束
        ui.add_space(8.0); // 语句

        // Snapshot the albums to a local list so we can call `draw_card(&mut self)`
        // while iterating without holding a borrow on `self.albums`.
        // Each entry: (key, name, images, collapsed, display_limit).
        let mut sections: Vec<(String, String, Vec<PathBuf>, bool, usize)> = Vec::new(); // 变量 sections
        for a in &self.albums { // 循环
            let imgs: Vec<PathBuf> = a.images.iter().filter(|p| matches(p)).cloned().collect(); // 变量 imgs
            if imgs.is_empty() && !q.is_empty() { // 条件分支
                continue; // 语句
            } // 块结束
            let collapsed = self.cfg.collapsed_folders.contains(&a.key); // 变量 collapsed
            let limit = *self.album_limits.get(&a.key).unwrap_or(&PAGE_IMAGES); // 变量 limit
            sections.push((a.key.clone(), a.name.clone(), imgs, collapsed, limit)); // 语句
        } // 块结束

        let current_path = self.current_path(); // 变量 current_path
        let avail = ui.available_width(); // 变量 avail
        // Fluid grid: 2–3 columns on compact screens, fixed cells on desktop.
        let (cols, cell_w) = if self.compact { // 调用 let
            let n = if avail < 470.0 { 2 } else { 3 }; // 变量 n
            let cw = ((avail - GAP * (n as f32 - 1.0)) / n as f32 - CARD_PAD * 2.0).max(120.0); // 变量 cw
            (n, cw) // 语句
        } else { // 语句
            let item_w = CELL_W + CARD_PAD * 2.0; // 变量 item_w
            let n = (((avail + GAP) / (item_w + GAP)).floor() as usize).max(1); // 变量 n
            (n, CELL_W) // 语句
        }; // 块结束
        let mut toggles: Vec<String> = Vec::new(); // 变量 toggles
        let mut load_more: Vec<String> = Vec::new(); // 变量 load_more

        ui.scope(|ui| { // 相册列表统一交由外层滚动区域处理
                if sections.is_empty() { // 条件分支
                    ui.add_space(24.0); // 语句
                    ui.label( // 语句
                        RichText::new(if q.is_empty() { // 调用 new
                            "暂无相册" // 语句
                        } else { // 语句
                            "没有匹配的图片" // 语句
                        }) // 语句
                        .size(13.0) // 语句
                        .color(theme::subtext()), // 语句
                    ); // 块结束
                    return; // 返回
                } // 块结束
                for (key, name, imgs, collapsed, limit) in &sections { // 循环
                    let arrow = if *collapsed { "▶" } else { "▼" }; // 变量 arrow
                    ui.horizontal(|ui| { // 语句
                        let label = RichText::new(format!("{arrow}  📁  {name}")) // 变量 label
                            .size(13.5) // 语句
                            .color(theme::text()); // 语句
                        let resp = ui.add( // 变量 resp
                            egui::Button::new(label) // 调用 new
                                .frame(false) // 语句
                                .min_size(egui::vec2(0.0, 26.0)), // 语句
                        ); // 块结束
                        if resp.clicked() { // 条件分支
                            toggles.push(key.clone()); // 语句
                        } // 块结束
                        chip( // 调用 chip
                            ui, // 语句
                            &format!("{} 张", imgs.len()), // 语句
                            theme::accent_soft(), // 字段 theme
                            theme::accent(), // 字段 theme
                        ); // 块结束
                    }); // 语句
                    if !*collapsed { // 条件分支
                        ui.add_space(4.0); // 语句
                        // Only build up to `limit` cards; large folders page in.
                        let shown = (*limit).min(imgs.len()); // 变量 shown
                        let mut idx = 0usize; // 变量 idx
                        while idx < shown { // 循环
                            ui.horizontal(|ui| { // 语句
                                for _ in 0..cols { // 循环
                                    if idx >= shown { // 条件分支
                                        break; // 语句
                                    } // 块结束
                                    let path = imgs[idx].clone(); // 变量 path
                                    self.draw_card(ui, &path, current_path.as_deref(), actions, cell_w); // 语句
                                    idx += 1; // 语句
                                } // 块结束
                            }); // 语句
                            ui.add_space(GAP); // 语句
                        } // 块结束
                        if imgs.len() > shown { // 条件分支
                            let remain = imgs.len() - shown; // 变量 remain
                            ui.add_space(2.0); // 语句
                            if ghost_button(ui, &format!("加载更多（还有 {remain} 张）")).clicked() { // 条件分支
                                load_more.push(key.clone()); // 语句
                            } // 块结束
                        } // 块结束
                    } // 块结束
                    ui.add_space(6.0); // 语句
                    ui.separator(); // 语句
                    ui.add_space(6.0); // 语句
                } // 块结束
            }); // 语句

        // Apply collapse toggles requested while iterating.
        for k in toggles { // 循环
            if !self.cfg.collapsed_folders.remove(&k) { // 条件分支
                self.cfg.collapsed_folders.insert(k); // 语句
            } // 块结束
        } // 块结束
        // Grow the page for albums whose "load more" button was clicked.
        for k in load_more { // 循环
            let e = self.album_limits.entry(k).or_insert(PAGE_IMAGES); // 变量 e
            *e += PAGE_IMAGES; // 语句
        } // 块结束
    } // 块结束

    /// Banner shown when a scan hits folders it cannot read. Offers to retry
    /// (triggers the OS prompt where applicable) or jump to the authorization
    /// page / reveal the folder (platform-specific).
    fn ui_permission_banner(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 ui_permission_banner
        let g = crate::permissions::guidance(); // 变量 g
        let count = self.permission_issues.len(); // 变量 count
        egui::Frame::none() // 调用 none
            .fill(theme::accent_soft()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::accent())) // 语句
            .rounding(Rounding::same(12.0)) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 12.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.horizontal(|ui| { // 语句
                    ui.label(RichText::new("🔒").size(16.0)); // 语句
                    ui.label( // 语句
                        RichText::new(format!("{count} 个目录无访问权限 · {}", g.title)) // 调用 new
                            .size(13.5) // 语句
                            .strong() // 语句
                            .color(theme::text()), // 语句
                    ); // 块结束
                }); // 语句
                ui.add_space(4.0); // 语句
                for p in self.permission_issues.iter().take(3) { // 循环
                    ui.label( // 语句
                        RichText::new(format!("• {}", ellipsis(&p.display().to_string(), 80))) // 调用 new
                            .size(11.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                } // 块结束
                if count > 3 { // 条件分支
                    ui.label( // 语句
                        RichText::new(format!("… 其余 {} 个目录", count - 3)) // 调用 new
                            .size(11.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                } // 块结束
                ui.add_space(4.0); // 语句
                ui.label( // 语句
                    RichText::new(g.detail) // 调用 new
                        .size(11.5) // 语句
                        .color(theme::subtext()), // 语句
                ); // 块结束
                ui.add_space(8.0); // 语句
                ui.horizontal(|ui| { // 语句
                    if primary_button(ui, "请求权限 / 重试").clicked() { // 条件分支
                        actions.push(Action::RetryPermissions); // 语句
                    } // 块结束
                    if ghost_button(ui, g.action).clicked() { // 条件分支
                        actions.push(Action::OpenPermissionPage); // 语句
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if ghost_button(ui, "忽略").clicked() { // 条件分支
                            actions.push(Action::DismissPermissions); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
    } // 块结束

    fn empty_state(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 empty_state
        ui.vertical_centered(|ui| { // 语句
            ui.add_space(70.0); // 语句
            let (r, _) = ui.allocate_exact_size(Vec2::new(104.0, 104.0), egui::Sense::hover()); // 调用 let
            ui.painter().circle_filled(r.center(), 52.0, theme::accent_soft()); // 语句
            ui.painter().circle_stroke(r.center(), 51.0, Stroke::new(1.0_f32, theme::accent())); // 语句
            ui.painter().text( // 语句
                r.center(), // 语句
                egui::Align2::CENTER_CENTER, // 字段 egui
                "🖼", // 语句
                egui::FontId::proportional(42.0), // 字段 egui
                theme::accent(), // 字段 theme
            ); // 块结束
            ui.add_space(16.0); // 语句
            ui.label( // 语句
                RichText::new("还没有壁纸") // 调用 new
                    .size(20.0) // 语句
                    .strong() // 语句
                    .color(theme::text()), // 语句
            ); // 块结束
            ui.add_space(6.0); // 语句
            ui.label( // 语句
                RichText::new("选择一个包含图片（或动态壁纸视频）的文件夹，即可开始使用固定壁纸或轮播") // 调用 new
                    .size(13.0) // 语句
                    .color(theme::subtext()), // 语句
            ); // 块结束
            ui.add_space(18.0); // 语句
            if primary_button(ui, "选择壁纸文件夹…").clicked() { // 条件分支
                actions.push(Action::PickFolder); // 语句
            } // 块结束
        }); // 语句
    } // 块结束

    fn draw_card( // 函数 draw_card
        &mut self, // 语句
        ui: &mut egui::Ui, // 字段 ui
        path: &Path, // 字段 path
        current: Option<&Path>, // 字段 current
        actions: &mut Vec<Action>, // 字段 actions
        cell_w: f32, // 字段 cell_w
    ) { // 语句
        let img_h = (cell_w * 0.58).round().max(70.0); // 变量 img_h
        let is_current = current == Some(path); // 变量 is_current
        let is_video = crate::thumbs::is_video_file(path); // 变量 is_video
        let base_stroke = if is_current { // 变量 base_stroke
            Stroke::new(2.0_f32, theme::accent()) // 调用 new
        } else { // 语句
            Stroke::new(1.0_f32, theme::border()) // 调用 new
        }; // 块结束
        let inner = egui::Frame::none() // 变量 inner
            .fill(theme::card()) // 语句
            .stroke(base_stroke) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .inner_margin(egui::Margin::same(CARD_PAD + 2.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .show(ui, |ui| { // 语句
                ui.vertical(|ui| { // 语句
                    ui.set_min_width(cell_w); // 语句
                    ui.set_max_width(cell_w); // 语句
                    let (rect, resp) = // 调用 let
                        ui.allocate_exact_size(Vec2::new(cell_w, img_h), egui::Sense::click()); // 语句
                    // Lazy-load: only decode thumbnails for cards actually on
                    // screen (or about to be), not all 3000 scanned files.
                    // 视频（动态壁纸）没有图片缩略图，跳过解码请求。
                    if !is_video // 条件分支
                        && ui.is_rect_visible(rect) // 语句
                        && !self.tex_cache.contains_key(path) // 语句
                        && !self.thumb.is_requested(path) // 语句
                    { // 语句
                        self.thumb.request(path); // 语句
                    } // 块结束
                    { // 语句
                        let painter = ui.painter(); // 变量 painter
                        painter.rect_filled(rect, Rounding::same(9.0), theme::extreme()); // 语句
                        if is_video { // 条件分支
                            // 动态壁纸：占位符 + 播放图标。
                            painter.text( // 语句
                                egui::pos2(rect.center().x, rect.center().y - 8.0), // 字段 egui
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "▶", // 语句
                                egui::FontId::proportional(30.0), // 字段 egui
                                theme::accent(), // 字段 theme
                            ); // 块结束
                            painter.text( // 语句
                                egui::pos2(rect.center().x, rect.center().y + 18.0), // 字段 egui
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "动态壁纸", // 语句
                                egui::FontId::proportional(11.0), // 字段 egui
                                theme::subtext(), // 字段 theme
                            ); // 块结束
                        } else { // 语句
                            match self.tex_cache.get(path) { // 分支匹配
                                Some(Some(tex)) => { // 匹配分支
                                    let ts = tex.size(); // 变量 ts
                                    let tw = ts[0] as f32; // 变量 tw
                                    let th = ts[1] as f32; // 变量 th
                                    if tw > 0.0 && th > 0.0 { // 条件分支
                                        let s = (rect.width() / tw).min(rect.height() / th); // 变量 s
                                        let size = Vec2::new(tw * s, th * s); // 变量 size
                                        let ir = egui::Rect::from_center_size(rect.center(), size); // 变量 ir
                                        painter.image( // 语句
                                            tex.id(), // 语句
                                            ir, // 语句
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), // 字段 egui
                                            Color32::WHITE, // 字段 Color32
                                        ); // 块结束
                                    } // 块结束
                                } // 块结束
                                Some(None) => { // 匹配分支
                                    painter.text( // 语句
                                        rect.center(), // 语句
                                        egui::Align2::CENTER_CENTER, // 字段 egui
                                        "无法预览", // 语句
                                        egui::FontId::proportional(12.0), // 字段 egui
                                        theme::subtext(), // 字段 theme
                                    ); // 块结束
                                } // 块结束
                                None => { // 匹配分支
                                    painter.text( // 语句
                                        rect.center(), // 语句
                                        egui::Align2::CENTER_CENTER, // 字段 egui
                                        "载入中…", // 语句
                                        egui::FontId::proportional(12.0), // 字段 egui
                                        theme::subtext(), // 字段 theme
                                    ); // 块结束
                                } // 块结束
                            } // 块结束
                        } // 块结束
                        // Hover overlay with a call-to-action pill.
                        if resp.hovered() { // 条件分支
                            painter.rect_filled( // 语句
                                rect, // 语句
                                Rounding::same(9.0), // 字段 Rounding
                                Color32::from_black_alpha(70), // 字段 Color32
                            ); // 块结束
                            let pill_w = if is_video { 118.0 } else { 96.0 }; // 变量 pill_w
                            let pill = // 变量 pill
                                egui::Rect::from_center_size(rect.center(), Vec2::new(pill_w, 30.0)); // 调用 from_center_size
                            painter.rect_filled(pill, Rounding::same(15.0), theme::accent()); // 语句
                            painter.text( // 语句
                                pill.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                if is_video { "设为动态壁纸" } else { "设为壁纸" }, // 语句
                                egui::FontId::proportional(12.5), // 字段 egui
                                Color32::WHITE, // 字段 Color32
                            ); // 块结束
                        } // 块结束
                        // Current badge, top-right of the thumbnail.
                        if is_current { // 条件分支
                            let badge = egui::Rect::from_min_size( // 变量 badge
                                egui::pos2(rect.right() - 52.0, rect.top() + 8.0), // 字段 egui
                                Vec2::new(44.0, 20.0), // 字段 Vec2
                            ); // 块结束
                            painter.rect_filled(badge, Rounding::same(10.0), theme::ok()); // 语句
                            painter.text( // 语句
                                badge.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "当前", // 语句
                                egui::FontId::proportional(11.0), // 字段 egui
                                Color32::WHITE, // 字段 Color32
                            ); // 块结束
                        } // 块结束
                        // Playlist badge, top-left (only when a custom playlist
                        // exists, so the default "all library" stays uncluttered).
                        if !self.cfg.slideshow.is_empty() && self.in_playlist(path) { // 条件分支
                            let badge = egui::Rect::from_min_size( // 变量 badge
                                egui::pos2(rect.left() + 8.0, rect.top() + 8.0), // 字段 egui
                                Vec2::new(50.0, 20.0), // 字段 Vec2
                            ); // 块结束
                            painter.rect_filled(badge, Rounding::same(10.0), theme::accent()); // 语句
                            painter.text( // 语句
                                badge.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "🎞 轮播", // 语句
                                egui::FontId::proportional(10.5), // 字段 egui
                                Color32::WHITE, // 字段 Color32
                            ); // 块结束
                        } // 块结束
                    } // 块结束
                    if resp.hovered() { // 条件分支
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
                    } // 块结束
                    if resp.clicked() { // 条件分支
                        actions.push(Action::SetPath(path.to_path_buf())); // 语句
                    } // 块结束
                    ui.add_space(9.0); // 语句
                    let name = path // 变量 name
                        .file_name() // 语句
                        .map(|s| s.to_string_lossy().to_string()) // 语句
                        .unwrap_or_default(); // 语句
                    ui.add(egui::Label::new( // 语句
                        RichText::new(ellipsis(&name, 20)) // 调用 new
                            .size(12.5) // 语句
                            .strong() // 语句
                            .color(theme::text()), // 语句
                    )); // 语句
                    ui.add_space(4.0); // 语句
                    ui.horizontal_wrapped(|ui| { // 语句
                        let ext = path // 变量 ext
                            .extension() // 语句
                            .and_then(|s| s.to_str()) // 语句
                            .map(|s| s.to_ascii_uppercase()) // 语句
                            .unwrap_or_default(); // 语句
                        egui::Frame::none() // 调用 none
                            .fill(theme::extreme()) // 语句
                            .rounding(Rounding::same(6.0)) // 语句
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0)) // 语句
                            .show(ui, |ui| { // 语句
                                ui.label(RichText::new(ext).size(9.5).color(theme::subtext())); // 语句
                            }); // 语句
                        if is_current { // 条件分支
                            ui.label(RichText::new("● 当前").size(11.0).color(theme::ok())); // 语句
                        } else if ui // 条件分支
                            .add( // 语句
                                egui::Button::new( // 调用 new
                                    RichText::new(if is_video { "设为动态壁纸" } else { "设为壁纸" }) // 调用 new
                                        .size(11.0) // 语句
                                        .color(theme::accent()), // 字段 RichText
                                ) // 块结束
                                .fill(theme::accent_soft()) // 语句
                                .rounding(Rounding::same(7.0)), // 语句
                            ) // 块结束
                            .clicked() // 语句
                        { // 语句
                            actions.push(Action::SetPath(path.to_path_buf())); // 语句
                        } // 块结束
                        // 视频条目：额外提供「设为锁屏」，由视频截取一帧作为锁屏海报。
                        if is_video // 条件分支
                            && ui // 语句
                                .add( // 语句
                                    egui::Button::new( // 调用 new
                                        RichText::new("设为锁屏").size(11.0).color(theme::subtext()), // 调用 new
                                    ) // 块结束
                                    .fill(theme::extreme()) // 语句
                                    .rounding(Rounding::same(7.0)), // 语句
                                ) // 块结束
                                .on_hover_text("从视频截取一帧作为锁屏壁纸") // 语句
                                .clicked() // 语句
                        { // 语句
                            actions.push(Action::LockVideo(path.to_path_buf())); // 语句
                        } // 块结束
                        // Add / remove this picture from the slideshow playlist.
                        let in_pl = self.in_playlist(path); // 变量 in_pl
                        let (lbl, fill, col) = if in_pl { // 调用 let
                            ("移出轮播", theme::extreme(), theme::subtext()) // 语句
                        } else { // 语句
                            ("+ 轮播", theme::accent_soft(), theme::accent()) // 语句
                        }; // 块结束
                        if ui // 条件分支
                            .add( // 语句
                                egui::Button::new(RichText::new(lbl).size(11.0).color(col)) // 调用 new
                                    .fill(fill) // 语句
                                    .rounding(Rounding::same(7.0)), // 语句
                            ) // 块结束
                            .on_hover_text(if in_pl { // 语句
                                "从轮播清单移除" // 语句
                            } else { // 语句
                                "加入轮播清单" // 语句
                            }) // 语句
                            .clicked() // 语句
                        { // 语句
                            if in_pl { // 条件分支
                                self.remove_from_playlist(path); // 语句
                            } else { // 语句
                                self.add_to_playlist(path); // 语句
                            } // 块结束
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
        // Accent outline on hover, drawn over the card.
        if inner.response.hovered() && !is_current { // 条件分支
            ui.painter().rect_stroke( // 语句
                inner.response.rect, // 语句
                Rounding::same(14.0), // 字段 Rounding
                Stroke::new(1.5_f32, theme::accent()), // 字段 Stroke
            ); // 块结束
        } // 块结束
    } // 块结束

    fn ui_slideshow(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 ui_slideshow
        // Prefer the previewed picture so an online/daily wallpaper (not in the
        // library index) still shows here as the "current" slideshow image.
        let cur = self // 变量 cur
            .preview // 语句
            .as_ref() // 语句
            .map(|(p, _)| p.clone()) // 语句
            .or_else(|| self.current_path()); // 语句
        let w = ui.available_width(); // 变量 w
        let h = preview_img_height(ui.ctx(), 0.56); // 变量 h

        // Preview card ------------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(16.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 4.0), // 字段 offset
                blur: 16.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .show(ui, |ui| { // 语句
                let (rect, _resp) = // 调用 let
                    ui.allocate_exact_size(Vec2::new(w - 2.0, h), egui::Sense::hover()); // 语句
                { // 语句
                    let painter = ui.painter(); // 变量 painter
                    painter.rect_filled(rect, Rounding::same(16.0), theme::extreme()); // 语句
                    let inner = rect.shrink(12.0); // 变量 inner
                    let mut painted = false; // 变量 painted
                    if let (Some((p, tex)), Some(c)) = (&self.preview, &cur) { // 条件分支
                        if p == c { // 条件分支
                            let ts = tex.size(); // 变量 ts
                            let tw = ts[0] as f32; // 变量 tw
                            let th = ts[1] as f32; // 变量 th
                            if tw > 0.0 && th > 0.0 { // 条件分支
                                let s = (inner.width() / tw).min(inner.height() / th); // 变量 s
                                let size = Vec2::new(tw * s, th * s); // 变量 size
                                let ir = egui::Rect::from_center_size(inner.center(), size); // 变量 ir
                                painter.image( // 语句
                                    tex.id(), // 语句
                                    ir, // 语句
                                    egui::Rect::from_min_max( // 调用 from_min_max
                                        egui::pos2(0.0, 0.0), // 字段 egui
                                        egui::pos2(1.0, 1.0), // 字段 egui
                                    ), // 语句
                                    Color32::WHITE, // 字段 Color32
                                ); // 块结束
                                painted = true; // 语句
                            } // 块结束
                        } // 块结束
                    } // 块结束
                    if !painted { // 条件分支
                        // 视频（动态壁纸）没有静态预览，显示播放占位符。
                        let is_video = cur.as_deref().map(crate::thumbs::is_video_file).unwrap_or(false); // 变量 is_video
                        painter.text( // 语句
                            egui::pos2(rect.center().x, rect.center().y - 12.0), // 字段 egui
                            egui::Align2::CENTER_CENTER, // 字段 egui
                            if is_video { "▶" } else { "🖼" }, // 语句
                            egui::FontId::proportional(46.0), // 字段 egui
                            theme::subtext(), // 字段 theme
                        ); // 块结束
                        if is_video { // 条件分支
                            painter.text( // 语句
                                egui::pos2(rect.center().x, rect.center().y + 30.0), // 字段 egui
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                "动态壁纸（视频）", // 语句
                                egui::FontId::proportional(12.0), // 字段 egui
                                theme::subtext(), // 字段 theme
                            ); // 块结束
                        } // 块结束
                    } // 块结束
                    // status chip (top-left)
                    let (chip_txt, chip_fill, chip_col) = if self.cfg.mode == Mode::Slideshow { // 调用 let
                        if self.cfg.running { // 条件分支
                            ("● 正在轮播", theme::accent2(), Color32::WHITE) // 语句
                        } else { // 语句
                            ("已暂停", theme::extreme(), theme::subtext()) // 语句
                        } // 块结束
                    } else { // 语句
                        ("● 固定壁纸", theme::accent_soft(), theme::accent()) // 语句
                    }; // 块结束
                    let g = painter.layout_no_wrap( // 变量 g
                        chip_txt.to_string(), // 语句
                        egui::FontId::proportional(11.5), // 字段 egui
                        chip_col, // 语句
                    ); // 块结束
                    let cw = g.size().x + 22.0; // 变量 cw
                    let pill = egui::Rect::from_min_size( // 变量 pill
                        egui::pos2(rect.left() + 14.0, rect.top() + 14.0), // 字段 egui
                        egui::vec2(cw, 26.0), // 字段 egui
                    ); // 块结束
                    painter.rect_filled(pill, Rounding::same(13.0), chip_fill); // 语句
                    painter.galley( // 语句
                        egui::pos2(pill.left() + 11.0, pill.center().y - g.size().y * 0.5), // 字段 egui
                        g, // 语句
                        chip_col, // 语句
                    ); // 块结束
                    // filename pill (bottom-left)
                    if let Some(pth) = &cur { // 条件分支
                        let name = pth // 变量 name
                            .file_name() // 语句
                            .map(|s| s.to_string_lossy().to_string()) // 语句
                            .unwrap_or_default(); // 语句
                        let g2 = painter.layout_no_wrap( // 变量 g2
                            ellipsis(&name, 46), // 调用 ellipsis
                            egui::FontId::proportional(12.0), // 字段 egui
                            Color32::WHITE, // 字段 Color32
                        ); // 块结束
                        let pw = g2.size().x + 22.0; // 变量 pw
                        let p2 = egui::Rect::from_min_size( // 变量 p2
                            egui::pos2(rect.left() + 14.0, rect.bottom() - 40.0), // 字段 egui
                            egui::vec2(pw, 26.0), // 字段 egui
                        ); // 块结束
                        painter.rect_filled(p2, Rounding::same(13.0), Color32::from_black_alpha(120)); // 语句
                        painter.galley( // 语句
                            egui::pos2(p2.left() + 11.0, p2.center().y - g2.size().y * 0.5), // 字段 egui
                            g2, // 语句
                            Color32::WHITE, // 字段 Color32
                        ); // 块结束
                    } // 块结束
                } // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句

        // Controls card -----------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 14.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.horizontal(|ui| { // 语句
                    ui.label(RichText::new("播放模式").size(12.0).color(theme::subtext())); // 语句
                    ui.add_space(4.0); // 语句
                    if segment(ui, self.cfg.mode == Mode::Fixed, "固定壁纸") { // 条件分支
                        self.cfg.mode = Mode::Fixed; // 语句
                    } // 块结束
                    if segment(ui, self.cfg.mode == Mode::Slideshow, "轮播") { // 条件分支
                        self.cfg.mode = Mode::Slideshow; // 语句
                        // 选择「轮播」即视为要自动轮播：直接开始，避免停在「已暂停」
                        // 状态、只有点「下一张」才换（用户反馈的「轮播不生效」）。
                        self.cfg.running = true; // 语句
                        self.last_switch = Instant::now(); // 语句
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if self.cfg.mode == Mode::Slideshow { // 条件分支
                            let mut sh = self.cfg.shuffle; // 变量 sh
                            if ui.checkbox(&mut sh, "随机顺序").changed() { // 条件分支
                                self.cfg.shuffle = sh; // 语句
                            } // 块结束
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(10.0); // 语句
                if self.cfg.mode == Mode::Slideshow { // 条件分支
                    ui.horizontal(|ui| { // 语句
                        let start = if self.cfg.running { "暂停轮播" } else { "▶  开始轮播" }; // 变量 start
                        if ui // 条件分支
                            .add( // 语句
                                egui::Button::new( // 调用 new
                                    RichText::new(start).size(13.0).color(Color32::WHITE).strong(), // 字段 RichText
                                ) // 块结束
                                .fill(theme::accent()) // 语句
                                .rounding(Rounding::same(10.0)) // 语句
                                .min_size(Vec2::new(124.0, 38.0)), // 语句
                            ) // 块结束
                            .clicked() // 语句
                        { // 语句
                            self.cfg.running = !self.cfg.running; // 语句
                            if self.cfg.running { // 条件分支
                                self.last_switch = Instant::now(); // 语句
                            } // 块结束
                        } // 块结束
                        ui.add_space(4.0); // 语句
                        if ghost_button(ui, "⏮ 上一张").clicked() { // 条件分支
                            actions.push(Action::Prev); // 语句
                        } // 块结束
                        if ghost_button(ui, "⏭ 下一张").clicked() { // 条件分支
                            actions.push(Action::Next); // 语句
                        } // 块结束
                        if ghost_button(ui, "🎲 随机").clicked() { // 条件分支
                            actions.push(Action::Random); // 语句
                        } // 块结束
                    }); // 语句
                    ui.add_space(12.0); // 语句
                    ui.horizontal(|ui| { // 语句
                        ui.label(RichText::new("间隔").size(12.5).color(theme::subtext())); // 语句
                        ui.add( // 语句
                            egui::Slider::new(&mut self.cfg.interval_secs, 5..=86400) // 调用 new
                                .logarithmic(true) // 语句
                                .show_value(false) // 语句
                                .text(""), // 语句
                        ); // 块结束
                        chip( // 调用 chip
                            ui, // 语句
                            &fmt_duration(self.cfg.interval_secs), // 语句
                            theme::accent_soft(), // 字段 theme
                            theme::accent(), // 字段 theme
                        ); // 块结束
                    }); // 语句
                    ui.add_space(8.0); // 语句
                    ui.horizontal_wrapped(|ui| { // 语句
                        for (label, secs) in PRESETS { // 循环
                            let selected = self.cfg.interval_secs == *secs; // 变量 selected
                            let fill = if selected { theme::accent() } else { theme::extreme() }; // 变量 fill
                            let color = if selected { Color32::WHITE } else { theme::text() }; // 变量 color
                            if ui // 条件分支
                                .add( // 语句
                                    egui::Button::new(RichText::new(*label).size(12.0).color(color)) // 调用 new
                                        .fill(fill) // 语句
                                        .rounding(Rounding::same(16.0)) // 语句
                                        .min_size(Vec2::new(0.0, 28.0)), // 语句
                                ) // 块结束
                                .clicked() // 语句
                            { // 语句
                                self.cfg.interval_secs = *secs; // 语句
                            } // 块结束
                        } // 块结束
                    }); // 语句
                    if self.cfg.running { // 条件分支
                        ui.add_space(12.0); // 语句
                        let iv = self.cfg.interval_secs.max(1); // 变量 iv
                        let el = self.last_switch.elapsed().as_secs(); // 变量 el
                        let frac = (el as f32 / iv as f32).clamp(0.0, 1.0); // 变量 frac
                        let remain = iv.saturating_sub(el); // 变量 remain
                        ui.add( // 语句
                            egui::ProgressBar::new(frac) // 调用 new
                                .desired_width(ui.available_width()) // 语句
                                .fill(theme::accent()) // 语句
                                .text(format!("距离下次更换：{}", fmt_duration(remain))), // 语句
                        ); // 块结束
                    } // 块结束
                } else { // 语句
                    ui.horizontal(|ui| { // 语句
                        if ghost_button(ui, "换下一张").clicked() { // 条件分支
                            actions.push(Action::Next); // 语句
                        } // 块结束
                        if ghost_button(ui, "🎲 随机一张").clicked() { // 条件分支
                            actions.push(Action::Random); // 语句
                        } // 块结束
                    }); // 语句
                    ui.add_space(6.0); // 语句
                    ui.label( // 语句
                        RichText::new("固定模式：在「本地壁纸库」点击任意图片即可设为桌面壁纸。") // 调用 new
                            .size(12.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                } // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句
        self.draw_slideshow_playlist(ui, actions); // 语句
    } // 块结束

    /// "轮播清单" panel: shows the pictures the slideshow will cycle and lets the
    /// operator remove them or reset to the whole library.
    fn draw_slideshow_playlist(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 draw_slideshow_playlist
        let using_all = self.cfg.slideshow.is_empty(); // 变量 using_all
        let stored = if using_all { // 变量 stored
            self.images.clone() // 语句
        } else { // 语句
            self.cfg.slideshow.clone() // 语句
        }; // 块结束
        let n = stored.len(); // 变量 n
        let mut to_remove: Option<PathBuf> = None; // 变量 to_remove
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(16.0, 14.0)) // 语句
            .show(ui, |ui| { // 语句
                ui.horizontal(|ui| { // 语句
                    ui.label( // 语句
                        RichText::new("轮播清单") // 调用 new
                            .size(14.0) // 语句
                            .strong() // 语句
                            .color(theme::text()), // 语句
                    ); // 块结束
                    chip(ui, &format!("{n} 张"), theme::accent_soft(), theme::accent()); // 调用 chip
                    if using_all { // 条件分支
                        chip(ui, "默认 · 全部本地壁纸库", theme::extreme(), theme::subtext()); // 调用 chip
                    } // 块结束
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if !using_all && ghost_button(ui, "恢复为全部本地壁纸库").clicked() { // 条件分支
                            self.cfg.slideshow.clear(); // 语句
                            self.status = "轮播已恢复为全部本地壁纸库".to_string(); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(4.0); // 语句
                ui.label( // 语句
                    RichText::new(if using_all { // 调用 new
                        "尚未自定义清单：当前轮播本地壁纸库全部图片。可在「本地壁纸库」或「每日壁纸」点「加入轮播 / 移出轮播」来自定义。" // 语句
                    } else { // 语句
                        "以下图片将按顺序（或随机）轮播；点「移除」可从清单去掉，「设为当前」立即切换。" // 语句
                    }) // 语句
                    .size(11.5) // 语句
                    .color(theme::subtext()), // 语句
                ); // 块结束
                ui.add_space(10.0); // 语句
                if stored.is_empty() { // 条件分支
                    ui.label( // 语句
                        RichText::new("清单为空，请到「本地壁纸库」或「每日壁纸」添加图片。") // 调用 new
                            .size(12.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                    return; // 返回
                } // 块结束
                // Cap rows rendered for very large lists; the rest stay in the
                // rotation but are managed via the library cards.
                let cap = stored.len().min(400); // 变量 cap
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| { // 调用 vertical
                    for path in stored.iter().take(cap) { // 循环
                        ui.horizontal(|ui| { // 语句
                            let (rect, resp) = ui.allocate_exact_size( // 调用 let
                                Vec2::new(64.0, 40.0), // 字段 Vec2
                                egui::Sense::click(), // 字段 egui
                            ); // 块结束
                            if !crate::thumbs::is_video_file(path) // 条件分支
                                && ui.is_rect_visible(rect) // 语句
                                && !self.tex_cache.contains_key(path) // 语句
                                && !self.thumb.is_requested(path) // 语句
                            { // 语句
                                self.thumb.request(path); // 语句
                            } // 块结束
                            { // 语句
                                let painter = ui.painter(); // 变量 painter
                                painter.rect_filled(rect, Rounding::same(6.0), theme::extreme()); // 语句
                                if crate::thumbs::is_video_file(path) { // 条件分支
                                    painter.text( // 语句
                                        rect.center(), // 语句
                                        egui::Align2::CENTER_CENTER, // 字段 egui
                                        "▶", // 语句
                                        egui::FontId::proportional(16.0), // 字段 egui
                                        theme::accent(), // 字段 theme
                                    ); // 块结束
                                } else if let Some(Some(tex)) = self.tex_cache.get(path) { // 条件分支
                                    let ts = tex.size(); // 变量 ts
                                    let (tw, th) = (ts[0] as f32, ts[1] as f32); // 调用 let
                                    if tw > 0.0 && th > 0.0 { // 条件分支
                                        let s = (rect.width() / tw).min(rect.height() / th); // 变量 s
                                        let ir = egui::Rect::from_center_size( // 变量 ir
                                            rect.center(), // 语句
                                            Vec2::new(tw * s, th * s), // 字段 Vec2
                                        ); // 块结束
                                        painter.image( // 语句
                                            tex.id(), // 语句
                                            ir, // 语句
                                            egui::Rect::from_min_max( // 调用 from_min_max
                                                egui::pos2(0.0, 0.0), // 字段 egui
                                                egui::pos2(1.0, 1.0), // 字段 egui
                                            ), // 语句
                                            Color32::WHITE, // 字段 Color32
                                        ); // 块结束
                                    } // 块结束
                                } // 块结束
                            } // 块结束
                            if resp.clicked() { // 条件分支
                                actions.push(Action::SetPath(path.clone())); // 语句
                            } // 块结束
                            let name = path // 变量 name
                                .file_name() // 语句
                                .map(|s| s.to_string_lossy().to_string()) // 语句
                                .unwrap_or_default(); // 语句
                            ui.add_space(8.0); // 语句
                            ui.add(egui::Label::new( // 语句
                                RichText::new(ellipsis(&name, 30)) // 调用 new
                                    .size(12.0) // 语句
                                    .color(theme::text()), // 语句
                            )); // 语句
                            ui.with_layout( // 语句
                                egui::Layout::right_to_left(egui::Align::Center), // 字段 egui
                                |ui| { // 语句
                                    if ghost_button(ui, "移除").clicked() { // 条件分支
                                        to_remove = Some(path.clone()); // 语句
                                    } // 块结束
                                    if ghost_button(ui, "设为当前").clicked() { // 条件分支
                                        actions.push(Action::SetPath(path.clone())); // 语句
                                    } // 块结束
                                }, // 块结束
                            ); // 块结束
                        }); // 语句
                        ui.add_space(4.0); // 语句
                    } // 块结束
                    if stored.len() > cap { // 条件分支
                        ui.label( // 语句
                            RichText::new(format!( // 调用 new
                                "仅显示前 {cap} 张（共 {} 张），其余仍在轮播；可在本地壁纸库卡片上管理。", // 语句
                                stored.len() // 语句
                            )) // 语句
                            .size(11.0) // 语句
                            .color(theme::subtext()), // 语句
                        ); // 块结束
                    } // 块结束
                }); // 语句
            }); // 语句
        if let Some(p) = to_remove { // 条件分支
            self.remove_from_playlist(&p); // 语句
        } // 块结束
    } // 块结束

    fn ui_settings(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) { // 函数 ui_settings
        page_title(ui, "设置", "个性化壁纸的显示与启动方式"); // 调用 page_title
        ui.add_space(14.0); // 语句

        // Appearance & display ---------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "外观与显示"); // 调用 section_header
                ui.add_space(12.0); // 语句
                ui.horizontal(|ui| { // 语句
                    setting_label(ui, "显示方式", "填充 / 适应 / 拉伸 / 居中 / 平铺"); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        let before_fit = self.cfg.fit; // 变量 before_fit
                        let fit_text = if self.compact { // 变量 fit_text
                            self.cfg.fit.short() // 语句
                        } else { // 语句
                            self.cfg.fit.label() // 语句
                        }; // 块结束
                        egui::ComboBox::from_id_salt("fit_combo") // 调用 from_id_salt
                            .selected_text(fit_text) // 语句
                            .width(if self.compact { 116.0 } else { 200.0 }) // 语句
                            .show_ui(ui, |ui| { // 语句
                                for f in Fit::all() { // 循环
                                    let t = if self.compact { f.short() } else { f.label() }; // 变量 t
                                    ui.selectable_value(&mut self.cfg.fit, f, t); // 语句
                                } // 块结束
                            }); // 语句
                        if self.cfg.fit != before_fit { // 条件分支
                            // Apply immediately to the current wallpaper.
                            actions.push(Action::ReapplyCurrent); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(8.0); // 语句
                ui.separator(); // 语句
                ui.add_space(8.0); // 语句
                ui.horizontal(|ui| { // 语句
                    setting_label(ui, "主题", "浅色 / 深色，即时生效"); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if segment(ui, self.cfg.theme == 1, "深色") { // 条件分支
                            self.cfg.theme = 1; // 语句
                        } // 块结束
                        if segment(ui, self.cfg.theme == 0, "浅色") { // 条件分支
                            self.cfg.theme = 0; // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
        ui.add_space(12.0); // 语句

        // Behaviour ---------------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "行为"); // 调用 section_header
                ui.add_space(12.0); // 语句
                ui.horizontal(|ui| { // 语句
                    setting_label(ui, "扫描包含子文件夹", "载入本地壁纸库时递归子目录"); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        let mut rc = self.cfg.recursive; // 变量 rc
                        if ui.checkbox(&mut rc, "").changed() { // 条件分支
                            self.cfg.recursive = rc; // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
                // Autostart is desktop-only (login items / Run key / .desktop).
                if !crate::platform::is_mobile() { // 条件分支
                    ui.add_space(8.0); // 语句
                    ui.separator(); // 语句
                    ui.add_space(8.0); // 语句
                    ui.horizontal(|ui| { // 语句
                        setting_label(ui, "开机自动启动", "登录系统后自动运行本程序"); // 调用 setting_label
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                            let mut auto = self.cfg.autostart; // 变量 auto
                            if ui.checkbox(&mut auto, "").changed() { // 条件分支
                                match crate::autostart::set_enabled(auto) { // 分支匹配
                                    Ok(_) => { // 匹配分支
                                        self.cfg.autostart = auto; // 语句
                                        self.status = if auto { // 语句
                                            "已开启开机自启".to_string() // 语句
                                        } else { // 语句
                                            "已关闭开机自启".to_string() // 语句
                                        }; // 块结束
                                    } // 块结束
                                    Err(e) => { // 匹配分支
                                        self.cfg.autostart = !auto; // 语句
                                        self.status = format!("开机自启设置失败：{e}"); // 语句
                                    } // 块结束
                                } // 块结束
                            } // 块结束
                        }); // 语句
                    }); // 语句
                } // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句

        // Resident & hotkey -------------------------------------------------
        if !crate::platform::is_mobile() { // 条件分支
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "托盘与快捷键"); // 调用 section_header
                ui.add_space(4.0); // 语句
                ui.label( // 语句
                    RichText::new("关闭窗口（右上角 ×）后会隐藏到系统托盘：Windows 右下角通知区域 / macOS 菜单栏 / Linux 状态区，程序继续在后台运行。托盘图标会尽量保持常驻显示；单击托盘图标可重新打开窗口，右键菜单可退出。") // 调用 new
                        .size(11.5) // 语句
                        .color(theme::subtext()), // 语句
                ); // 块结束
                #[cfg(windows)] // 语句
                { // 语句
                    if ghost_button(ui, "打开托盘设置（把图标设为始终显示）").clicked() { // 条件分支
                        // Use rundll32 (GUI subsystem) so no console window flashes.
                        let _ = std::process::Command::new("rundll32.exe") // 变量 _
                            .args(["url.dll,FileProtocolHandler", "ms-settings:taskbar"]) // 语句
                            .spawn(); // 语句
                    } // 块结束
                } // 块结束
                ui.add_space(12.0); // 语句
                ui.horizontal(|ui| { // 语句
                    let desc = if self.tray.is_some() { // 变量 desc
                        "托盘图标已就绪" // 语句
                    } else { // 语句
                        "当前环境无系统托盘，将改用全局快捷键隐藏 / 唤回" // 语句
                    }; // 块结束
                    setting_label(ui, "系统托盘", desc); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if self.tray.is_some() { // 条件分支
                            chip(ui, "可用", theme::accent_soft(), theme::ok()); // 调用 chip
                        } else { // 语句
                            chip(ui, "不可用", theme::extreme(), theme::subtext()); // 调用 chip
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(8.0); // 语句
                ui.separator(); // 语句
                ui.add_space(8.0); // 语句
                ui.horizontal(|ui| { // 语句
                    setting_label(ui, "关闭时隐藏到托盘", "关闭窗口后隐藏到通知区域，轮播 / 每日壁纸继续运行"); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        let mut cb = self.cfg.close_to_background; // 变量 cb
                        if ui.checkbox(&mut cb, "").changed() { // 条件分支
                            self.cfg.close_to_background = cb; // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(8.0); // 语句
                ui.separator(); // 语句
                ui.add_space(8.0); // 语句
                let reg = self.hotkey.is_registered(); // 变量 reg
                ui.horizontal(|ui| { // 语句
                    let desc = if reg { // 变量 desc
                        format!("{}：显示 / 隐藏窗口", self.hotkey.label()) // 语句
                    } else { // 语句
                        "注册失败：快捷键可能被其它软件占用".to_string() // 语句
                    }; // 块结束
                    setting_label(ui, "全局快捷键", &desc); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if reg { // 条件分支
                            let txt = format!("已注册 · {}", self.hotkey.label()); // 变量 txt
                            chip(ui, &txt, theme::accent_soft(), theme::ok()); // 调用 chip
                        } else { // 语句
                            let txt = format!("注册失败 (code {})", self.hotkey.error_code()); // 变量 txt
                            chip(ui, &txt, theme::extreme(), theme::subtext()); // 调用 chip
                        } // 块结束
                    }); // 语句
                }); // 语句
                ui.add_space(8.0); // 语句
                ui.separator(); // 语句
                ui.add_space(8.0); // 语句
                ui.horizontal(|ui| { // 语句
                    setting_label(ui, "隐藏 / 退出", "隐藏到托盘继续运行，或彻底退出应用"); // 调用 setting_label
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { // 语句
                        if ghost_button(ui, "退出应用").clicked() { // 条件分支
                            actions.push(Action::Quit); // 语句
                        } // 块结束
                        if ghost_button(ui, "隐藏到托盘").clicked() { // 条件分支
                            actions.push(Action::HideToTray); // 语句
                        } // 块结束
                    }); // 语句
                }); // 语句
            }); // 语句
        ui.add_space(12.0); // 语句
        } // 块结束

        // Display & compatibility -------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "显示与兼容"); // 调用 section_header
                ui.add_space(10.0); // 语句
                let (pw, ph) = self.primary_res; // 调用 let
                let res_txt = if pw > 0 && ph > 0 { // 变量 res_txt
                    format!("主屏 {}×{}", pw, ph) // 语句
                } else { // 语句
                    "主屏分辨率未知".to_string() // 语句
                }; // 块结束
                ui.label( // 语句
                    RichText::new(format!( // 调用 new
                        "检测到 {} 台显示器 · {} · 当前主题 {}", // 语句
                        self.monitors, // 语句
                        res_txt, // 语句
                        if self.cfg.theme == 1 { "深色" } else { "浅色" } // 条件分支
                    )) // 语句
                    .size(12.0) // 语句
                    .color(theme::text()), // 语句
                ); // 块结束
                ui.add_space(6.0); // 语句
                ui.label( // 语句
                    RichText::new("壁纸将应用到所有显示器（Windows 通过系统 API）。高 DPI 缩放由系统自动适配。") // 调用 new
                        .size(11.0) // 语句
                        .color(theme::subtext()), // 语句
                ); // 块结束
            }); // 语句
        ui.add_space(12.0); // 语句

        // Lock screen -------------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "锁屏壁纸"); // 调用 section_header
                ui.add_space(4.0); // 语句
                ui.label( // 语句
                    RichText::new("把当前壁纸同时设为系统锁屏。Windows 通过 PersonalizationCSP 写入 HKLM，首次点击会弹出系统管理员授权（UAC），点「是」后自动完成，无需手动以管理员身份运行；Ubuntu / GNOME 通过 gsettings 设置锁屏 picture-uri；macOS 锁屏跟随系统壁纸。") // 调用 new
                        .size(11.5) // 语句
                        .color(theme::subtext()), // 语句
                ); // 块结束
                ui.add_space(12.0); // 语句
                ui.horizontal(|ui| { // 语句
                    if primary_button(ui, "将当前壁纸设为锁屏").clicked() { // 条件分支
                        self.set_lock_from_current(); // 语句
                    } // 块结束
                    ui.label( // 语句
                        RichText::new("当前壁纸会同步应用到锁屏") // 调用 new
                            .size(11.0) // 语句
                            .color(theme::subtext()), // 语句
                    ); // 块结束
                }); // 语句
            }); // 语句
        ui.add_space(12.0); // 语句

        // Config file -------------------------------------------------------
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(18.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "配置文件"); // 调用 section_header
                ui.add_space(10.0); // 语句
                egui::Frame::none() // 调用 none
                    .fill(theme::extreme()) // 语句
                    .rounding(Rounding::same(8.0)) // 语句
                    .inner_margin(egui::Margin::symmetric(11.0, 7.0)) // 语句
                    .show(ui, |ui| { // 语句
                        ui.label( // 语句
                            RichText::new(crate::config::config_path().display().to_string()) // 调用 new
                                .size(12.0) // 语句
                                .color(theme::subtext()), // 语句
                        ); // 块结束
                    }); // 语句
                ui.add_space(10.0); // 语句
                ui.horizontal(|ui| { // 语句
                    if ghost_button(ui, "打开配置目录").clicked() { // 条件分支
                        actions.push(Action::OpenConfig); // 语句
                    } // 块结束
                    if primary_button(ui, "立即保存").clicked() { // 条件分支
                        actions.push(Action::Save); // 语句
                    } // 块结束
                }); // 语句
            }); // 语句
        ui.add_space(12.0); // 语句
        ui.label( // 语句
            RichText::new("提示：Windows 使用系统 API 设置壁纸并写入显示方式；Linux 使用 GNOME(gsettings，含 picture-options) 或 feh；macOS 通过 System Events。开机自启在 Windows 写入 Run 注册表、macOS 写入 LaunchAgent、Linux 写入 autostart .desktop。") // 调用 new
                .size(11.5) // 语句
                .color(theme::subtext()), // 语句
        ); // 块结束
    } // 块结束

    fn ui_about(&mut self, ui: &mut egui::Ui) { // 函数 ui_about
        ui.vertical_centered(|ui| { // 语句
            ui.add_space(12.0); // 语句
            let (r, _) = ui.allocate_exact_size(Vec2::new(84.0, 84.0), egui::Sense::hover()); // 调用 let
            ui.painter().rect_filled(r, Rounding::same(22.0), theme::accent()); // 语句
            ui.painter().text( // 语句
                r.center(), // 语句
                egui::Align2::CENTER_CENTER, // 字段 egui
                "🖼", // 语句
                egui::FontId::proportional(40.0), // 字段 egui
                Color32::WHITE, // 字段 Color32
            ); // 块结束
            ui.add_space(12.0); // 语句
            ui.label( // 语句
                RichText::new("Wallpaper Studio") // 调用 new
                    .size(24.0) // 语句
                    .strong() // 语句
                    .color(theme::text()), // 语句
            ); // 块结束
            ui.add_space(6.0); // 语句
            let vtxt = format!("v{} · {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS); // 变量 vtxt
            let g = ui // 变量 g
                .painter() // 语句
                .layout_no_wrap(vtxt, egui::FontId::proportional(11.5), theme::accent()); // 语句
            let pw = g.size().x + 24.0; // 变量 pw
            let (vr, _) = ui.allocate_exact_size(Vec2::new(pw, 24.0), egui::Sense::hover()); // 调用 let
            ui.painter().rect_filled(vr, Rounding::same(12.0), theme::accent_soft()); // 语句
            ui.painter().galley( // 语句
                egui::pos2(vr.left() + 12.0, vr.center().y - g.size().y * 0.5), // 字段 egui
                g, // 语句
                theme::accent(), // 字段 theme
            ); // 块结束
            ui.add_space(10.0); // 语句
            ui.label( // 语句
                RichText::new("跨平台壁纸工具：Windows / macOS / Linux 桌面 + iOS / Android 自适应界面，支持固定壁纸与轮播。") // 调用 new
                    .size(13.0) // 语句
                    .color(theme::subtext()), // 语句
            ); // 块结束
            ui.add_space(6.0); // 语句
            let cap = crate::platform::wallpaper_support(); // 变量 cap
            ui.horizontal(|ui| { // 语句
                ui.label( // 语句
                    RichText::new(format!("当前平台：{}", crate::platform::name())) // 调用 new
                        .size(11.5) // 语句
                        .color(theme::subtext()), // 语句
                ); // 块结束
                let (fill, col, txt) = if cap.can_set { // 调用 let
                    (theme::accent_soft(), theme::ok(), format!("{} · 可设置", cap.label)) // 语句
                } else { // 语句
                    (theme::extreme(), theme::subtext(), format!("{} · 仅浏览", cap.label)) // 语句
                }; // 块结束
                chip(ui, &txt, fill, col); // 调用 chip
            }); // 语句
            ui.label(RichText::new(cap.note).size(11.0).color(theme::subtext())); // 语句
        }); // 语句
        ui.add_space(18.0); // 语句
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(20.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "功能一览"); // 调用 section_header
                ui.add_space(10.0); // 语句
                let features = [ // 变量 features
                    ("📁", "批量载入", "从本地文件夹载入图片，可递归子目录"), // 语句
                    ("📌", "固定壁纸", "点击任意图片即设为桌面壁纸"), // 语句
                    ("🎞", "自动轮播", "按间隔更换，支持顺序 / 随机，动态壁纸视频亦可加入"), // 语句
                    ("🖥", "显示方式", "填充 / 适应 / 拉伸 / 居中 / 平铺"), // 语句
                    ("🎨", "主题切换", "浅色 / 深色主题，即时生效"), // 语句
                    ("💾", "配置保存", "设置自动持久化，开机自启可选"), // 语句
                ]; // 语句
                ui.columns(2, |cols| { // 语句
                    for (i, (icon, title, desc)) in features.iter().enumerate() { // 循环
                        let col = &mut cols[i % 2]; // 变量 col
                        col.horizontal(|ui| { // 语句
                            let (ir, _) = ui // 调用 let
                                .allocate_exact_size(Vec2::new(30.0, 30.0), egui::Sense::hover()); // 语句
                            ui.painter() // 语句
                                .rect_filled(ir, Rounding::same(9.0), theme::accent_soft()); // 语句
                            ui.painter().text( // 语句
                                ir.center(), // 语句
                                egui::Align2::CENTER_CENTER, // 字段 egui
                                *icon, // 语句
                                egui::FontId::proportional(15.0), // 字段 egui
                                theme::accent(), // 字段 theme
                            ); // 块结束
                            ui.vertical(|ui| { // 语句
                                ui.label( // 语句
                                    RichText::new(*title) // 调用 new
                                        .size(12.5) // 语句
                                        .strong() // 语句
                                        .color(theme::text()), // 语句
                                ); // 块结束
                                ui.label(RichText::new(*desc).size(11.0).color(theme::subtext())); // 语句
                            }); // 语句
                        }); // 语句
                        col.add_space(8.0); // 语句
                    } // 块结束
                }); // 语句
            }); // 语句
        ui.add_space(18.0); // 语句
        egui::Frame::none() // 调用 none
            .fill(theme::card()) // 语句
            .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
            .rounding(Rounding::same(14.0)) // 语句
            .shadow(egui::epaint::Shadow { // 语句
                offset: egui::vec2(0.0, 3.0), // 字段 offset
                blur: 12.0, // 字段 blur
                spread: 0.0, // 字段 spread
                color: theme::shadow(), // 字段 color
            }) // 语句
            .inner_margin(egui::Margin::symmetric(20.0, 16.0)) // 语句
            .show(ui, |ui| { // 语句
                section_header(ui, "下载（各平台）"); // 调用 section_header
                ui.add_space(10.0); // 语句
                ui.label( // 语句
                    RichText::new("下载最新版安装包（点击「下载」在浏览器打开）：") // 调用 new
                        .size(12.0) // 语句
                        .color(theme::text()), // 语句
                ); // 块结束
                ui.add_space(6.0); // 语句
                let links = [ // 变量 links
                    ( // 语句
                        "Windows (x64) · .zip", // 语句
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-windows-x64.zip", // 语句
                    ), // 语句
                    ( // 语句
                        "macOS (Universal) · .tar.gz", // 语句
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-macos-universal.tar.gz", // 语句
                    ), // 语句
                    ( // 语句
                        "Linux / Ubuntu (x64) · .tar.gz", // 语句
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-linux-x64.tar.gz", // 语句
                    ), // 语句
                    ( // 语句
                        "全部版本 · Release 页面", // 语句
                        "https://github.com/shuying-code/wallpaper-rs/releases/latest", // 语句
                    ), // 语句
                ]; // 语句
                for (label, url) in links { // 循环
                    ui.horizontal(|ui| { // 语句
                        if ghost_button(ui, "⬇ 下载").clicked() { // 条件分支
                            open_url(url); // 调用 open_url
                        } // 块结束
                        ui.label(RichText::new(label).size(12.0).color(theme::accent())); // 语句
                        ui.label(RichText::new(url).size(10.0).color(theme::subtext())); // 语句
                    }); // 语句
                    ui.add_space(4.0); // 语句
                } // 块结束
            }); // 语句
    } // 块结束
} // 块结束

impl eframe::App for App { // 实现
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) { // 函数 update
        // 本帧开始时的配置基线：必须在处理任何后台任务结果（下载完成 → 加入轮播 /
        // 在线壁纸日期等）之前取，否则这些异步改动会被算进基线，末尾的自动保存
        // 认为「没有变化」而不落盘——表现为「＋ 轮播(原视频)」提示已加入、重启后却丢失。
        let snap = Snapshot::of(&self.cfg); // 变量 snap
        if self.applied_theme != self.cfg.theme { // 条件分支
            theme::apply(ctx, self.cfg.theme == 1); // 调用 apply
            self.applied_theme = self.cfg.theme; // 语句
        } // 块结束

        // 适配笔记本等不同屏幕尺寸：首帧把窗口收进当前显示器的可用范围
        // （预留任务栏 / 标题栏），只缩小不放大，也不打扰已最大化的窗口。
        if !self.window_fitted && self.shot.is_none() { // 条件分支
            self.window_fitted = true; // 语句
            let (mon, maximized, cur) = ctx.input(|i| { // 调用 let
                let v = i.viewport(); // 变量 v
                (v.monitor_size, v.maximized, v.inner_rect.map(|r| r.size())) // 语句
            }); // 块结束
            if let (Some(mon), Some(cur)) = (mon, cur) { // 条件分支
                if !maximized.unwrap_or(false) && mon.x > 1.0 && mon.y > 1.0 { // 条件分支
                    let max = egui::vec2((mon.x - 48.0).max(360.0), (mon.y - 96.0).max(560.0)); // 变量 max
                    let want = egui::vec2(cur.x.min(max.x), cur.y.min(max.y)); // 变量 want
                    if (want.x - cur.x).abs() > 1.0 || (want.y - cur.y).abs() > 1.0 { // 条件分支
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(want)); // 语句
                    } // 块结束
                } // 块结束
            } // 块结束
        } // 块结束

        // Screen-size adaptation screenshot mode (`--shots <dir>`): drive the
        // resize / capture state machine every frame and keep repainting.
        if self.shot.is_some() { // 条件分支
            ctx.request_repaint(); // 语句
            if self.shot_pre(ctx) { // 条件分支
                return; // 返回
            } // 块结束
        } // 块结束

        // System tray: react to menu / click actions queued by the tray handlers.
        let tray_actions = self.tray.as_ref().map(|t| t.drain()).unwrap_or_default(); // 变量 tray_actions
        for action in tray_actions { // 循环
            match action { // 分支匹配
                crate::tray::TrayAction::Show => self.show_window(ctx), // 匹配分支
                crate::tray::TrayAction::Hide => self.hide_to_tray(ctx), // 匹配分支
                crate::tray::TrayAction::Quit => { // 匹配分支
                    // Tray "Quit" must really terminate the background process
                    // and remove the notification-area icon, not just hide.
                    self.hard_exit(); // 语句
                } // 块结束
            } // 块结束
        } // 块结束

        // Keep `window_hidden` in sync with the OS-level hidden state (tray menu
        // actions are applied directly on the native thread).
        #[cfg(windows)] // 语句
        { // 语句
            self.window_hidden = crate::winhide::is_hidden(); // 语句
        } // 块结束

        // Global hotkey: show / hide the window (bonus shortcut, tray optional).
        if self.hotkey.take_toggle() { // 条件分支
            if self.window_hidden { // 条件分支
                self.show_window(ctx); // 语句
                self.status = "已唤回窗口".to_string(); // 语句
            } else { // 语句
                self.hide_to_tray(ctx); // 语句
            } // 块结束
        } // 块结束

        // "X" close -> hide to tray instead of quitting (unless the user chose
        // Quit). We can swallow the close as long as there is a way to bring the
        // window back: the system tray, or the global hotkey as a fallback.
        let can_restore = self.tray.is_some() || self.hotkey.is_registered(); // 变量 can_restore
        if should_background_on_close( // 条件分支
            ctx.input(|i| i.viewport().close_requested()), // 语句
            self.cfg.close_to_background, // 语句
            can_restore, // 语句
            self.force_quit, // 语句
        ) { // 语句
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose); // 语句
            self.hide_to_tray(ctx); // 语句
        } // 块结束

        // Poll background full-disk scan.
        let scan_result = self.scan_rx.as_ref().and_then(|rx| rx.try_recv().ok()); // 变量 scan_result
        if let Some(found) = scan_result { // 条件分支
            self.scan_rx = None; // 语句
            self.scanning = false; // 语句
            self.thumb.reset(); // 语句
            self.tex_cache.clear(); // 语句
            self.tex_order.clear(); // 语句
            self.preview = None; // 语句
            self.images = found; // 语句
            self.albums = crate::albums::build_albums(&self.images, None); // 语句
            self.permission_issues.clear(); // 语句
            self.current = if self.images.is_empty() { None } else { Some(0) }; // 语句
            self.status = if self.images.is_empty() { // 语句
                "全盘扫描完成，但未找到符合条件的壁纸".to_string() // 语句
            } else { // 语句
                format!( // 语句
                    "全盘扫描完成：符合壁纸要求 {} 张{}", // 语句
                    self.images.len(), // 语句
                    if self.cfg.wallpaper_only { // 条件分支
                        "（已过滤图标/小图/竖图等）" // 语句
                    } else { // 语句
                        "" // 语句
                    } // 块结束
                ) // 块结束
            }; // 块结束
            self.last_switch = Instant::now(); // 语句
        } // 块结束

        // Poll background online-wallpaper fetch.
        let online_result = self.online_rx.as_ref().and_then(|rx| rx.try_recv().ok()); // 变量 online_result
        if let Some(res) = online_result { // 条件分支
            self.online_rx = None; // 语句
            self.online_loading = false; // 语句
            match res { // 分支匹配
                Ok(img) => { // 匹配分支
                    if let Some(t) = crate::thumbs::load_image(&img.path, 1400) { // 条件分支
                        let ci = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba); // 变量 ci
                        let tex = ctx.load_texture( // 变量 tex
                            format!("online:{}", img.path.display()), // 语句
                            ci, // 语句
                            egui::TextureOptions::LINEAR, // 字段 egui
                        ); // 块结束
                        self.online_tex = Some(tex); // 语句
                    } else { // 语句
                        self.online_tex = None; // 语句
                    } // 块结束
                    self.cfg.last_online_date = img.date.clone(); // 语句
                    self.status = format!("已获取在线壁纸：{}", img.title); // 语句
                    self.online_img = Some(img); // 语句
                    if self.cfg.online_auto_set { // 条件分支
                        self.apply_online_wallpaper(); // 语句
                    } // 块结束
                } // 块结束
                Err(e) => { // 匹配分支
                    self.status = format!("在线壁纸获取失败：{e}"); // 语句
                } // 块结束
            } // 块结束
        } // 块结束

        // Daily online wallpaper: bootstrap on launch, then refresh every 24h.
        if self.cfg.daily_online && !self.online_loading { // 条件分支
            if !self.online_boot_checked { // 条件分支
                self.online_boot_checked = true; // 语句
                self.start_online_fetch(0); // 语句
            } else if self.last_online_auto.elapsed() >= Duration::from_secs(86_400) { // 条件分支
                self.last_online_auto = Instant::now(); // 语句
                self.start_online_fetch(0); // 语句
            } // 块结束
        } // 块结束

        // 网上壁纸库：列表请求结果 → 换页 / 更新网格。
        if let Some(res) = self.web_list_rx.as_ref().and_then(|rx| rx.try_recv().ok()) { // 条件分支
            self.web_list_rx = None; // 语句
            self.web_loading = false; // 语句
            match res { // 分支匹配
                Ok(p) => { // 匹配分支
                    self.web_error = None; // 语句
                    self.web_page = p.page.max(1); // 语句
                    self.web_total_pages = p.total_pages; // 语句
                    self.web_has_more = p.has_more; // 语句
                    if p.search_id.is_some() { // 条件分支
                        self.web_search_id = p.search_id; // 语句
                    } // 块结束
                    self.web_items = p.items; // 语句
                    self.web_thumb.reset(); // 语句
                    self.web_thumbs.clear(); // 语句
                    self.web_thumb_order.clear(); // 语句
                    self.web_preview_id = None; // 语句
                    self.web_preview_tex = None; // 语句
                    let src = self.web_source.min(crate::websrc::SOURCES.len() - 1); // 变量 src
                    let name = crate::websrc::SOURCES[src].name; // 变量 name
                    self.status = if self.web_total_pages > 0 { // 语句
                        format!("{name}：第 {} / {} 页，本页 {} 张", self.web_page, self.web_total_pages, self.web_items.len()) // 语句
                    } else { // 语句
                        format!("{name}：第 {} 页，本页 {} 张", self.web_page, self.web_items.len()) // 语句
                    }; // 块结束
                } // 块结束
                Err(e) => { // 匹配分支
                    self.web_error = Some(e.clone()); // 语句
                    self.status = format!("网上壁纸库获取失败：{e}"); // 语句
                } // 块结束
            } // 块结束
        } // 块结束

        // 网上壁纸库：缩略图下载结果 → 纹理。
        for (url, t) in self.web_thumb.poll() { // 循环
            match t { // 分支匹配
                Some(t) => { // 匹配分支
                    let img = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba); // 变量 img
                    let tex = ctx.load_texture( // 变量 tex
                        format!("webt:{url}"), // 语句
                        img, // 语句
                        egui::TextureOptions::LINEAR, // 字段 egui
                    ); // 块结束
                    if !self.web_thumbs.contains_key(&url) { // 条件分支
                        self.web_thumb_order.push(url.clone()); // 语句
                    } // 块结束
                    self.web_thumbs.insert(url, Some(tex)); // 语句
                } // 块结束
                None => { // 匹配分支
                    if !self.web_thumbs.contains_key(&url) { // 条件分支
                        self.web_thumb_order.push(url.clone()); // 语句
                    } // 块结束
                    self.web_thumbs.insert(url, None); // 语句
                } // 块结束
            } // 块结束
        } // 块结束
        self.enforce_web_tex_cap(); // 语句

        // 网上壁纸库：原图下载结果 → 预览纹理 + 待办动作。
        if let Some((id, res)) = self.web_full_rx.as_ref().and_then(|rx| rx.try_recv().ok()) { // 条件分支
            self.web_full_rx = None; // 语句
            self.web_busy = false; // 语句
            match res { // 分支匹配
                Ok(full) => { // 匹配分支
                    if self.web_preview_id == Some(id) { // 条件分支
                        if let Some(t) = crate::thumbs::load_image(&full.path, 1600) { // 条件分支
                            let ci = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba); // 变量 ci
                            let tex = ctx.load_texture( // 变量 tex
                                format!("webp:{}", full.path.display()), // 语句
                                ci, // 语句
                                egui::TextureOptions::LINEAR, // 字段 egui
                            ); // 块结束
                            self.web_preview_tex = Some(tex); // 语句
                        } // 块结束
                    } // 块结束
                    self.status = format!("原图已就绪：{}", full.title); // 语句
                    self.web_full.insert(id, Some(full.clone())); // 语句
                    if let Some((pid, kind)) = self.web_pending { // 条件分支
                        if pid == id { // 条件分支
                            self.web_pending = None; // 语句
                            self.apply_web_full(kind, &full); // 语句
                        } // 块结束
                    } // 块结束
                } // 块结束
                Err(e) => { // 匹配分支
                    self.web_full.insert(id, None); // 语句
                    self.web_pending = None; // 语句
                    self.status = format!("原图下载失败：{e}"); // 语句
                } // 块结束
            } // 块结束
        } // 块结束

        // 网上壁纸库：动态壁纸（mp4）下载结果。
        if let Some(res) = self.web_video_rx.as_ref().and_then(|rx| rx.try_recv().ok()) { // 条件分支
            self.web_video_rx = None; // 语句
            self.web_video_busy = false; // 语句
            let after = std::mem::take(&mut self.web_video_after); // 变量 after
            match res { // 分支匹配
                Ok(path) => { // 匹配分支
                    match after { // 分支匹配
                        WebVideoAfter::Live => self.with_mpv_or_download(AfterMpv::LiveWallpaper(path)), // 匹配分支
                        WebVideoAfter::Playlist => { // 匹配分支
                            self.add_to_playlist(&path); // 语句
                            self.status = format!("原视频已加入轮播：{}", path.display()); // 语句
                        } // 块结束
                        WebVideoAfter::Lock => self.with_mpv_or_download(AfterMpv::LockVideo(path)), // 匹配分支
                        WebVideoAfter::Save => { // 匹配分支
                            self.status = format!("动态壁纸已下载到 {}", path.display()); // 语句
                        } // 块结束
                    } // 块结束
                } // 块结束
                Err(e) => self.status = format!("动态壁纸下载失败：{e}"), // 匹配分支
            } // 块结束
        } // 块结束

        // mpv 按需下载结果：完成后若有待办动作（设为动态壁纸 / 锁屏）就执行。
        if let Some(res) = self.mpv_dl_rx.as_ref().and_then(|rx| rx.try_recv().ok()) { // 条件分支
            self.mpv_dl_rx = None; // 语句
            self.mpv_dl_busy = false; // 语句
            match res { // 分支匹配
                Ok(path) => { // 匹配分支
                    if let Some(after) = self.mpv_after.take() { // 条件分支
                        self.run_after_mpv(after); // 语句
                    } else { // 语句
                        self.status = format!("mpv 已就绪：{}", path.display()); // 语句
                    } // 块结束
                } // 块结束
                Err(e) => { // 匹配分支
                    self.mpv_after = None; // 语句
                    self.status = format!("mpv 不可用：{e}"); // 语句
                } // 块结束
            } // 块结束
        } // 块结束

        // 首次进入「网上壁纸库」时自动载入第一页。
        if self.tab == Tab::Web4k && !self.web_loaded_once && !self.web_loading { // 条件分支
            self.web_loaded_once = true; // 语句
            self.web_load(1); // 语句
        } // 块结束

        // drain finished thumbnails
        for (p, t) in self.thumb.poll() { // 循环
            match t { // 分支匹配
                Some(t) => { // 匹配分支
                    let img = ColorImage::from_rgba_unmultiplied([t.w, t.h], &t.rgba); // 变量 img
                    let tex = ctx.load_texture( // 变量 tex
                        format!("thumb:{}", p.display()), // 语句
                        img, // 语句
                        egui::TextureOptions::LINEAR, // 字段 egui
                    ); // 块结束
                    if !self.tex_cache.contains_key(&p) { // 条件分支
                        self.tex_order.push(p.clone()); // 语句
                    } // 块结束
                    self.tex_cache.insert(p, Some(tex)); // 语句
                } // 块结束
                None => { // 匹配分支
                    if !self.tex_cache.contains_key(&p) { // 条件分支
                        self.tex_order.push(p.clone()); // 语句
                    } // 块结束
                    self.tex_cache.insert(p, None); // 语句
                } // 块结束
            } // 块结束
        } // 块结束
        self.enforce_tex_cap(); // 语句

        // keyboard shortcuts
        if self.slide_count() > 1 { // 条件分支
            let (right, left) = ctx.input(|i| { // 调用 let
                ( // 语句
                    i.key_pressed(egui::Key::ArrowRight), // 语句
                    i.key_pressed(egui::Key::ArrowLeft), // 语句
                ) // 块结束
            }); // 语句
            if right { // 条件分支
                self.advance(ctx); // 语句
            } else if left { // 条件分支
                self.step_back(ctx); // 语句
            } // 块结束
        } // 块结束

        // ensure preview for current
        if let Some(p) = self.current_path() { // 条件分支
            if !crate::thumbs::is_video_file(&p) // 条件分支
                && self.preview.as_ref().map(|(q, _)| q != &p).unwrap_or(true) // 语句
            { // 语句
                self.load_preview(ctx, &p); // 语句
            } // 块结束
        } // 块结束

        // 轮播首次开始时，后台预热动态壁纸的首帧海报（供进入动态壁纸时秒切）。
        if self.cfg.mode == Mode::Slideshow && self.cfg.running && !self.posters_warmed { // 条件分支
            self.posters_warmed = true; // 语句
            self.precache_live_posters(); // 语句
        } // 块结束

        // slideshow ticking
        if self.cfg.mode == Mode::Slideshow && self.cfg.running && self.slide_count() > 0 { // 条件分支
            let iv = Duration::from_secs(self.cfg.interval_secs.max(1)); // 变量 iv
            if self.last_switch.elapsed() >= iv { // 条件分支
                self.advance(ctx); // 语句
            } // 块结束
            let remain = iv.saturating_sub(self.last_switch.elapsed()); // 变量 remain
            ctx.request_repaint_after(remain.min(Duration::from_millis(400))); // 语句
        } else if !self.images.is_empty() { // 条件分支
            ctx.request_repaint_after(Duration::from_millis(350)); // 语句
        } // 块结束

        // Responsive: use the compact, touch-friendly layout on phones/tablets
        // (or a narrow desktop window), and the sidebar layout otherwise.
        self.compact = // 语句
            crate::platform::is_mobile() || ctx.screen_rect().width() < COMPACT_BREAK; // 调用 is_mobile
        let (touch_h, pad) = if self.compact { // 调用 let
            (46.0, egui::vec2(16.0, 10.0)) // 语句
        } else { // 语句
            (34.0, egui::vec2(15.0, 8.0)) // 语句
        }; // 块结束
        ctx.style_mut(|s| { // 语句
            s.spacing.interact_size.y = touch_h; // 语句
            s.spacing.button_padding = pad; // 语句
        }); // 语句

        let mut actions: Vec<Action> = Vec::new(); // 变量 actions
        self.draw_header(ctx, &mut actions); // 语句
        if self.compact { // 条件分支
            self.draw_bottom_nav(ctx); // 语句
        } else { // 语句
            self.draw_sidebar(ctx); // 语句
        } // 块结束
        self.draw_status_bar(ctx); // 语句
        self.draw_center(ctx, &mut actions); // 语句
        self.apply(ctx, actions); // 语句

        let snap2 = Snapshot::of(&self.cfg); // 变量 snap2
        if snap != snap2 { // 条件分支
            self.cfg.save(); // 语句
        } // 块结束

        self.shot_post(ctx); // 语句
    } // 块结束

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) { // 函数 on_exit
        self.cfg.save(); // 语句
    } // 块结束
} // 块结束

const PRESETS: &[(&str, u64)] = &[ // 常量 PRESETS
    ("10秒", 10), // 语句
    ("30秒", 30), // 语句
    ("1分", 60), // 语句
    ("5分", 300), // 语句
    ("15分", 900), // 语句
    ("30分", 1800), // 语句
    ("1时", 3600), // 语句
    ("6时", 21600), // 语句
    ("1天", 86400), // 语句
]; // 语句

fn chip(ui: &mut egui::Ui, text: &str, fill: Color32, color: Color32) { // 函数 chip
    egui::Frame::none() // 调用 none
        .fill(fill) // 语句
        .rounding(Rounding::same(20.0)) // 语句
        .inner_margin(egui::Margin::symmetric(10.0, 3.0)) // 语句
        .show(ui, |ui| { // 语句
            ui.label(RichText::new(text).size(11.5).color(color).strong()); // 语句
        }); // 语句
} // 块结束

/// Rounded chip used for the selected folder path.
fn folder_chip(ui: &mut egui::Ui, text: &str) { // 函数 folder_chip
    egui::Frame::none() // 调用 none
        .fill(theme::extreme()) // 语句
        .rounding(Rounding::same(8.0)) // 语句
        .inner_margin(egui::Margin::symmetric(11.0, 5.0)) // 语句
        .show(ui, |ui| { // 语句
            ui.label(RichText::new(text).size(12.5).color(theme::text())); // 语句
        }); // 语句
} // 块结束

/// Rounded search box used on the library toolbar.
fn search_field(ui: &mut egui::Ui, text: &mut String, width: f32) { // 函数 search_field
    egui::Frame::none() // 调用 none
        .fill(theme::extreme()) // 语句
        .stroke(Stroke::new(1.0_f32, theme::border())) // 语句
        .rounding(Rounding::same(8.0)) // 语句
        .inner_margin(egui::Margin::symmetric(9.0, 4.0)) // 语句
        .show(ui, |ui| { // 语句
            ui.add( // 语句
                egui::TextEdit::singleline(text) // 调用 singleline
                    .hint_text("按文件名过滤…") // 语句
                    .desired_width(width) // 语句
                    .frame(false), // 语句
            ); // 块结束
        }); // 语句
} // 块结束

fn tab_from_index(i: usize) -> Tab { // 函数 tab_from_index
    match i { // 分支匹配
        0 => Tab::Library, // 匹配分支
        1 => Tab::Slideshow, // 匹配分支
        2 => Tab::Online, // 匹配分支
        3 => Tab::Settings, // 匹配分支
        4 => Tab::About, // 匹配分支
        _ => Tab::Web4k, // 匹配分支
    } // 块结束
} // 块结束

fn page_title(ui: &mut egui::Ui, title: &str, subtitle: &str) { // 函数 page_title
    ui.horizontal(|ui| { // 语句
        let (bar, _) = ui.allocate_exact_size(Vec2::new(4.0, 34.0), egui::Sense::hover()); // 调用 let
        ui.painter().rect_filled(bar, Rounding::same(2.0), theme::accent()); // 语句
        ui.add_space(6.0); // 语句
        ui.vertical(|ui| { // 语句
            ui.label(RichText::new(title).size(19.0).strong().color(theme::text())); // 语句
            ui.label(RichText::new(subtitle).size(11.5).color(theme::subtext())); // 语句
        }); // 语句
    }); // 语句
} // 块结束

fn section_header(ui: &mut egui::Ui, text: &str) { // 函数 section_header
    ui.horizontal(|ui| { // 语句
        let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 16.0), egui::Sense::hover()); // 调用 let
        ui.painter().rect_filled(bar, Rounding::same(1.5), theme::accent()); // 语句
        ui.add_space(4.0); // 语句
        ui.label(RichText::new(text).size(14.0).strong().color(theme::text())); // 语句
    }); // 语句
} // 块结束

fn setting_label(ui: &mut egui::Ui, title: &str, desc: &str) { // 函数 setting_label
    ui.vertical(|ui| { // 语句
        ui.label(RichText::new(title).size(13.0).strong().color(theme::text())); // 语句
        ui.label(RichText::new(desc).size(11.0).color(theme::subtext())); // 语句
    }); // 语句
} // 块结束

fn nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> bool { // 函数 nav_button
    let height = 42.0; // 变量 height
    let (rect, resp) = // 调用 let
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click()); // 语句
    let hovered = resp.hovered(); // 变量 hovered
    let t = ui.ctx().animate_bool(resp.id, hovered); // 变量 t
    let painter = ui.painter(); // 变量 painter
    if selected { // 条件分支
        painter.rect_filled(rect, Rounding::same(11.0), theme::accent_soft()); // 语句
        let bar = egui::Rect::from_min_size( // 变量 bar
            egui::pos2(rect.left(), rect.top() + 9.0), // 字段 egui
            egui::vec2(3.0, height - 18.0), // 字段 egui
        ); // 块结束
        painter.rect_filled(bar, Rounding::same(2.0), theme::accent()); // 语句
    } else if t > 0.01 { // 条件分支
        let base = theme::accent_soft(); // 变量 base
        let c = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), (60.0 * t) as u8); // 变量 c
        painter.rect_filled(rect, Rounding::same(11.0), c); // 语句
    } // 块结束
    let icon_rect = egui::Rect::from_center_size( // 变量 icon_rect
        egui::pos2(rect.left() + 28.0, rect.center().y), // 字段 egui
        egui::vec2(26.0, 26.0), // 字段 egui
    ); // 块结束
    let icon_bg = if selected { theme::accent() } else { theme::extreme() }; // 变量 icon_bg
    painter.rect_filled(icon_rect, Rounding::same(8.0), icon_bg); // 语句
    let icon_color = if selected { Color32::WHITE } else { theme::subtext() }; // 变量 icon_color
    painter.text( // 语句
        icon_rect.center(), // 语句
        egui::Align2::CENTER_CENTER, // 字段 egui
        icon, // 语句
        egui::FontId::proportional(14.0), // 字段 egui
        icon_color, // 语句
    ); // 块结束
    let txt_color = if selected { // 变量 txt_color
        theme::accent() // 调用 accent
    } else if hovered { // 条件分支
        theme::text() // 调用 text
    } else { // 语句
        theme::subtext() // 调用 subtext
    }; // 块结束
    let galley = painter.layout_no_wrap( // 变量 galley
        label.to_string(), // 语句
        egui::FontId::proportional(14.0), // 字段 egui
        txt_color, // 语句
    ); // 块结束
    painter.galley( // 语句
        egui::pos2(rect.left() + 50.0, rect.center().y - galley.size().y * 0.5), // 字段 egui
        galley, // 语句
        txt_color, // 语句
    ); // 块结束
    if hovered { // 条件分支
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
    } // 块结束
    resp.clicked() // 语句
} // 块结束

/// A single bottom-nav item: stacked icon + label with a soft selected pill.
fn bottom_nav_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool, width: f32) -> bool { // 函数 bottom_nav_button
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 54.0), egui::Sense::click()); // 调用 let
    let hovered = resp.hovered(); // 变量 hovered
    if selected { // 条件分支
        ui.painter() // 语句
            .rect_filled(rect.shrink2(egui::vec2(6.0, 4.0)), Rounding::same(12.0), theme::accent_soft()); // 语句
    } else if hovered { // 条件分支
        ui.painter() // 语句
            .rect_filled(rect.shrink2(egui::vec2(6.0, 4.0)), Rounding::same(12.0), theme::extreme()); // 语句
    } // 块结束
    let color = if selected { theme::accent() } else { theme::subtext() }; // 变量 color
    let center = rect.center(); // 变量 center
    ui.painter().text( // 语句
        egui::pos2(center.x, center.y - 10.0), // 字段 egui
        egui::Align2::CENTER_CENTER, // 字段 egui
        icon, // 语句
        egui::FontId::proportional(18.0), // 字段 egui
        color, // 语句
    ); // 块结束
    ui.painter().text( // 语句
        egui::pos2(center.x, center.y + 12.0), // 字段 egui
        egui::Align2::CENTER_CENTER, // 字段 egui
        label, // 语句
        egui::FontId::proportional(11.0), // 字段 egui
        color, // 语句
    ); // 块结束
    if hovered { // 条件分支
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
    } // 块结束
    resp.clicked() // 语句
} // 块结束

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response { // 函数 primary_button
    ui.add( // 语句
        egui::Button::new(RichText::new(text).size(12.5).color(Color32::WHITE).strong()) // 调用 new
            .fill(theme::accent()) // 语句
            .rounding(Rounding::same(9.0)) // 语句
            .min_size(Vec2::new(0.0, 30.0)), // 语句
    ) // 块结束
} // 块结束

fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response { // 函数 ghost_button
    ui.add( // 语句
        egui::Button::new(RichText::new(text).size(12.5).color(theme::text())) // 调用 new
            .fill(theme::extreme()) // 语句
            .rounding(Rounding::same(9.0)) // 语句
            .min_size(Vec2::new(0.0, 30.0)), // 语句
    ) // 块结束
} // 块结束

fn theme_toggle(ui: &mut egui::Ui, label: &str) -> bool { // 函数 theme_toggle
    let (rect, resp) = // 调用 let
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click()); // 语句
    let hovered = resp.hovered(); // 变量 hovered
    let fill = if hovered { theme::accent_soft() } else { theme::extreme() }; // 变量 fill
    ui.painter().rect_filled(rect, Rounding::same(9.0), fill); // 语句
    let color = if hovered { theme::accent() } else { theme::text() }; // 变量 color
    let galley = // 变量 galley
        ui.painter() // 语句
            .layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), color); // 语句
    ui.painter().galley( // 语句
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y * 0.5), // 字段 egui
        galley, // 语句
        color, // 语句
    ); // 块结束
    if hovered { // 条件分支
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
    } // 块结束
    resp.clicked() // 语句
} // 块结束

/// A small pill-style segmented option (e.g. 浅色 / 深色), themed to the palette.
fn segment(ui: &mut egui::Ui, selected: bool, text: &str) -> bool { // 函数 segment
    let galley = ui // 变量 galley
        .painter() // 语句
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(12.5), theme::text()); // 语句
    let textw = (galley.size().x + 26.0).max(40.0); // 变量 textw
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(textw, 28.0), egui::Sense::click()); // 调用 let
    let fill = if selected { // 变量 fill
        theme::accent_soft() // 调用 accent_soft
    } else if resp.hovered() { // 条件分支
        theme::extreme() // 调用 extreme
    } else { // 语句
        Color32::TRANSPARENT // 语句
    }; // 块结束
    if fill != Color32::TRANSPARENT { // 条件分支
        ui.painter().rect_filled(rect, Rounding::same(9.0), fill); // 语句
    } // 块结束
    let color = if selected { theme::accent() } else { theme::subtext() }; // 变量 color
    ui.painter().text( // 语句
        rect.center(), // 语句
        egui::Align2::CENTER_CENTER, // 字段 egui
        text, // 语句
        egui::FontId::proportional(12.5), // 字段 egui
        color, // 语句
    ); // 块结束
    if resp.hovered() { // 条件分支
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
    } // 块结束
    resp.clicked() // 语句
} // 块结束

/// 卡片角落的收藏星标：已收藏为实心 ★（强调色），未收藏为空心 ☆。
fn star_button(ui: &mut egui::Ui, fav: bool) -> bool { // 函数 star_button
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 20.0), egui::Sense::click()); // 调用 let
    let color = if fav { // 变量 color
        theme::accent() // 调用 accent
    } else if resp.hovered() { // 条件分支
        theme::text() // 调用 text
    } else { // 语句
        theme::subtext() // 调用 subtext
    }; // 块结束
    ui.painter().text( // 语句
        rect.center(), // 语句
        egui::Align2::CENTER_CENTER, // 字段 egui
        if fav { "★" } else { "☆" }, // 语句
        egui::FontId::proportional(15.0), // 字段 egui
        color, // 语句
    ); // 块结束
    if resp.hovered() { // 条件分支
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); // 语句
    } // 块结束
    resp.on_hover_text(if fav { "取消收藏" } else { "收藏" }).clicked() // 语句
} // 块结束

fn collect_images( // 函数 collect_images
    dir: &Path, // 字段 dir
    recursive: bool, // 字段 recursive
    out: &mut Vec<PathBuf>, // 字段 out
    denied: &mut Vec<PathBuf>, // 字段 denied
) { // 语句
    let rd = match std::fs::read_dir(dir) { // 变量 rd
        Ok(r) => r, // 匹配分支
        Err(e) => { // 匹配分支
            if e.kind() == std::io::ErrorKind::PermissionDenied { // 条件分支
                denied.push(dir.to_path_buf()); // 语句
            } // 块结束
            return; // 返回
        } // 块结束
    }; // 块结束
    for e in rd.flatten() { // 循环
        let p = e.path(); // 变量 p
        if p.is_dir() { // 条件分支
            if recursive { // 条件分支
                collect_images(&p, recursive, out, denied); // 调用 collect_images
            } // 块结束
        } else if is_media(&p) { // 条件分支
            out.push(p); // 语句
        } // 块结束
    } // 块结束
} // 块结束

/// 图片或视频（动态壁纸）。视频扩展名见 [`crate::thumbs::is_video_file`]。
fn is_media(p: &Path) -> bool { // 函数 is_media
    if crate::thumbs::is_video_file(p) { // 条件分支
        return true; // 返回
    } // 块结束
    match p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()) { // 分支匹配
        Some(ext) => matches!( // 匹配分支
            ext.as_str(), // 语句
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "tif" | "tiff" // 语句
        ), // 语句
        None => false, // 匹配分支
    } // 块结束
} // 块结束

fn fmt_duration(secs: u64) -> String { // 函数 fmt_duration
    if secs >= 86400 { // 条件分支
        format!("{}天{}时", secs / 86400, (secs % 86400) / 3600) // 语句
    } else if secs >= 3600 { // 条件分支
        format!("{}时{}分", secs / 3600, (secs % 3600) / 60) // 语句
    } else if secs >= 60 { // 条件分支
        format!("{}分{}秒", secs / 60, secs % 60) // 语句
    } else { // 语句
        format!("{}秒", secs) // 语句
    } // 块结束
} // 块结束

fn ellipsis(s: &str, max: usize) -> String { // 函数 ellipsis
    let cs: Vec<char> = s.chars().collect(); // 变量 cs
    if cs.len() <= max { // 条件分支
        s.to_string() // 语句
    } else { // 语句
        let mut t: String = cs[..max].iter().collect(); // 变量 t
        t.push('…'); // 语句
        t // 语句
    } // 块结束
} // 块结束

fn open_path(path: &Path) { // 函数 open_path
    #[cfg(windows)] // 语句
    { // 语句
        let _ = std::process::Command::new("explorer").arg(path).spawn(); // 变量 _
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        let _ = std::process::Command::new("open").arg(path).spawn(); // 变量 _
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        let _ = std::process::Command::new("xdg-open").arg(path).spawn(); // 变量 _
    } // 块结束
} // 块结束

/// Open a URL in the system default browser.
fn open_url(url: &str) { // 函数 open_url
    #[cfg(windows)] // 语句
    { // 语句
        let _ = std::process::Command::new("cmd") // 变量 _
            .args(["/C", "start", "", url]) // 语句
            .spawn(); // 语句
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        let _ = std::process::Command::new("open").arg(url).spawn(); // 变量 _
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        let _ = std::process::Command::new("xdg-open").arg(url).spawn(); // 变量 _
    } // 块结束
} // 块结束

/// Decide whether a window-close should be swallowed into the tray.
/// `can_restore` means there is some way to bring the window back (system tray
/// or global hotkey); if not, and the user explicitly quit, we fall through to a
/// real exit so the app can never get stuck un-closable and unrecoverable.
fn should_background_on_close( // 函数 should_background_on_close
    close_requested: bool, // 字段 close_requested
    close_to_background: bool, // 字段 close_to_background
    can_restore: bool, // 字段 can_restore
    force_quit: bool, // 字段 force_quit
) -> bool { // 语句
    close_requested && close_to_background && can_restore && !force_quit // 语句
} // 块结束

/// Return the oldest keys that must be dropped so `order` fits within `cap`.
fn evict_plan(order: &[PathBuf], cap: usize) -> Vec<PathBuf> { // 函数 evict_plan
    if order.len() <= cap { // 条件分支
        return Vec::new(); // 返回
    } // 块结束
    order[..order.len() - cap].to_vec() // 语句
} // 块结束

/// 网格列数：紧凑布局固定 2–3 列，桌面布局按可用宽度自适应（宽屏笔记本一屏放更多张）。
fn grid_cols(avail: f32, compact: bool) -> usize {
    if compact {
        if avail < 470.0 {
            2
        } else {
            3
        }
    } else {
        let item_w = CELL_W + CARD_PAD * 2.0;
        (((avail + GAP) / (item_w + GAP)).floor() as usize).max(1)
    }
}

/// 网格单元宽度：紧凑布局把可用宽度按列数均分，桌面布局用固定宽度。
fn grid_cell_w(avail: f32, compact: bool) -> f32 {
    if compact {
        let n = grid_cols(avail, compact) as f32;
        ((avail - GAP * (n - 1.0)) / n - CARD_PAD * 2.0).max(120.0)
    } else {
        CELL_W
    }
}

/// 预览大图区高度：按窗口高度自适应。矮屏笔记本给更矮的预览，避免把下方列表挤出
/// 可视区（这样在大图预览打开时列表依然能正常上下滑动查看）。
fn preview_img_height(ctx: &egui::Context, frac: f32) -> f32 {
    let vh = ctx.screen_rect().height();
    (vh * frac).clamp(150.0, 420.0)
}

/// 常驻可见的垂直滚动区：滚动条始终显示（内容不足一屏也保留轨道），
/// 并占满可用高度。这样右侧「可上下滑动」一目了然，点击预览 / 操作后也不会消失。
fn vscroll() -> egui::ScrollArea {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::containers::scroll_area::ScrollBarVisibility::AlwaysVisible)
}

/// 「窄桌面」布局判定：窗口宽度介于紧凑阈值与笔记本阈值之间（1366×768 / 1280×720
/// 等小尺寸笔记本，或高 DPI 缩放后的较小逻辑宽度）。用于收紧侧栏与页面留白。
fn is_narrow(ctx: &egui::Context) -> bool {
    let w = ctx.screen_rect().width();
    w >= COMPACT_BREAK && w < NARROW_BREAK
}

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::{evict_plan, should_background_on_close}; // 引入依赖
    use std::path::PathBuf; // 引入依赖

    #[test] // 语句
    fn close_logic_matrix() { // 函数 close_logic_matrix
        // Normal close with everything enabled -> background.
        assert!(should_background_on_close(true, true, true, false)); // 语句
        // Explicit quit always exits.
        assert!(!should_background_on_close(true, true, true, true)); // 语句
        // No way to restore the window -> must exit.
        assert!(!should_background_on_close(true, true, false, false)); // 语句
        // Setting off -> exit.
        assert!(!should_background_on_close(true, false, true, false)); // 语句
        // No close request -> nothing to do.
        assert!(!should_background_on_close(false, true, true, false)); // 语句
    } // 块结束

    #[test] // 语句
    fn eviction_is_bounded_and_oldest_first() { // 函数 eviction_is_bounded_and_oldest_first
        let order: Vec<PathBuf> = (0..10).map(|i| PathBuf::from(format!("{i}.png"))).collect(); // 变量 order
        // Under cap -> nothing evicted.
        assert!(evict_plan(&order, 10).is_empty()); // 语句
        assert!(evict_plan(&order, 20).is_empty()); // 语句
        // Over cap -> drop the oldest (len - cap) entries.
        let plan = evict_plan(&order, 7); // 变量 plan
        assert_eq!(plan.len(), 3); // 语句
        assert_eq!(plan[0], PathBuf::from("0.png")); // 语句
        assert_eq!(plan[2], PathBuf::from("2.png")); // 语句
    } // 块结束
} // 块结束
