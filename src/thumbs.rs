//! Background thumbnail decoding with a simple worker thread + channel queue.

use std::collections::HashSet; // 引入依赖
use std::path::{Path, PathBuf}; // 引入依赖
use std::sync::mpsc::{channel, Receiver, Sender}; // 引入依赖
use std::thread; // 引入依赖

pub const THUMB_MAX: u32 = 340; // 常量 THUMB_MAX

// A "wallpaper-grade" image: at least 1280×720 and landscape-ish (aspect ≥ 1.2).
pub const MIN_WALLPAPER_W: u32 = 1280; // 常量 MIN_WALLPAPER_W
pub const MIN_WALLPAPER_H: u32 = 720; // 常量 MIN_WALLPAPER_H

/// Read only the image header to get (width, height) cheaply (no full decode).
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> { // 函数 image_dimensions
    image::ImageReader::open(path) // 调用 open
        .ok()? // 语句
        .with_guessed_format() // 语句
        .ok()? // 语句
        .into_dimensions() // 语句
        .ok() // 语句
} // 块结束

/// Whether an image of size (w,h) is suitable as a desktop wallpaper.
pub fn wallpaper_suitable(w: u32, h: u32) -> bool { // 函数 wallpaper_suitable
    w >= MIN_WALLPAPER_W && h >= MIN_WALLPAPER_H && (w as u64) * 10 >= (h as u64) * 12 // 语句
} // 块结束

/// Convenience: is this file a wallpaper-grade image (reads only the header)?
pub fn is_wallpaper_file(path: &Path) -> bool { // 函数 is_wallpaper_file
    match image_dimensions(path) { // 分支匹配
        Some((w, h)) => wallpaper_suitable(w, h), // 匹配分支
        None => false, // 匹配分支
    } // 块结束
} // 块结束

/// 是否为视频（动态壁纸）文件。这些文件由 `livewall`（Windows + mpv）循环
/// 渲染为桌面壁纸，也可以像静态图片一样加入轮播清单。
///
/// 只按扩展名判定：无需预解码即可在扫描 / 过滤时快速区分静态图与视频。
pub fn is_video_file(path: &Path) -> bool { // 函数 is_video_file
    match path.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()) { // 分支匹配
        Some(ext) => matches!( // 匹配分支
            ext.as_str(), // 语句
            "mp4" | "webm" | "mkv" | "mov" | "m4v" | "avi" | "wmv" | "flv" | "ts"
                | "mpg" | "mpeg" | "ogv" | "3gp" // 语句
        ), // 语句
        None => false, // 匹配分支
    } // 块结束
} // 块结束

pub struct Thumb { // 结构体 Thumb
    pub w: usize, // 字段 w
    pub h: usize, // 字段 h
    pub rgba: Vec<u8>, // 字段 rgba
} // 块结束

/// Decode an image file, downscaled so its longest side <= `max`.
pub fn load_image(path: &Path, max: u32) -> Option<Thumb> { // 函数 load_image
    let img = image::open(path).ok()?; // 变量 img
    let img = img.thumbnail(max, max); // 变量 img
    let rgba = img.to_rgba8(); // 变量 rgba
    let (w, h) = rgba.dimensions(); // 调用 let
    Some(Thumb { // 调用 Some
        w: w as usize, // 字段 w
        h: h as usize, // 字段 h
        rgba: rgba.into_raw(), // 字段 rgba
    }) // 语句
} // 块结束

pub struct Thumbnailer { // 结构体 Thumbnailer
    tx: Sender<(PathBuf, u64)>, // 字段 tx
    rx: Receiver<(PathBuf, u64, Option<Thumb>)>, // 字段 rx
    requested: HashSet<PathBuf>, // 字段 requested
    /// Bumped on `reset()`. Results tagged with an older generation are dropped
    /// so thumbnails queued before a folder change / full-disk scan can never
    /// pollute the new list's texture cache.
    generation: u64, // 字段 generation
} // 块结束

impl Thumbnailer { // 实现
    pub fn new() -> Self { // 函数 new
        let (tx, rx_req) = channel::<(PathBuf, u64)>(); // 调用 let
        let (tx_res, rx) = channel::<(PathBuf, u64, Option<Thumb>)>(); // 调用 let
        thread::spawn(move || { // 调用 spawn
            while let Ok((p, gen)) = rx_req.recv() { // 循环
                let t = load_image(&p, THUMB_MAX); // 变量 t
                if tx_res.send((p, gen, t)).is_err() { // 条件分支
                    break; // 语句
                } // 块结束
            } // 块结束
        }); // 语句
        Self { // 语句
            tx, // 语句
            rx, // 语句
            requested: HashSet::new(), // 字段 requested
            generation: 0, // 字段 generation
        } // 块结束
    } // 块结束

    pub fn request(&mut self, p: &Path) { // 函数 request
        if self.requested.insert(p.to_path_buf()) { // 条件分支
            let _ = self.tx.send((p.to_path_buf(), self.generation)); // 变量 _
        } // 块结束
    } // 块结束

    pub fn is_requested(&self, p: &Path) -> bool { // 函数 is_requested
        self.requested.contains(p) // 语句
    } // 块结束

    /// Allow a path to be requested again (used by LRU texture-cache eviction so
    /// cards that scroll back into view are re-decoded instead of stuck loading).
    pub fn forget(&mut self, p: &Path) { // 函数 forget
        self.requested.remove(p); // 语句
    } // 块结束

    pub fn reset(&mut self) { // 函数 reset
        self.requested.clear(); // 语句
        self.generation = self.generation.wrapping_add(1); // 语句
    } // 块结束

    /// Non-blocking drain of finished thumbnails. Results produced before the
    /// last `reset()` are discarded.
    pub fn poll(&mut self) -> Vec<(PathBuf, Option<Thumb>)> { // 函数 poll
        let mut v = Vec::new(); // 变量 v
        while let Ok((p, gen, t)) = self.rx.try_recv() { // 循环
            if gen == self.generation { // 条件分支
                v.push((p, t)); // 语句
            } // 块结束
        } // 块结束
        v // 语句
    } // 块结束
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn reset_drops_stale_generation() { // 函数 reset_drops_stale_generation
        // Build a thumbnailer, request a path at generation 0, then reset.
        let mut t = Thumbnailer::new(); // 变量 t
        // Force a known generation bump without any worker activity.
        t.reset(); // 语句
        assert_eq!(t.generation, 1); // 语句
        // A path requested at gen 1 is tracked; forgetting allows re-request.
        let p = Path::new("nonexistent.png"); // 变量 p
        t.request(p); // 语句
        assert!(t.is_requested(p)); // 语句
        t.forget(p); // 语句
        assert!(!t.is_requested(p)); // 语句
    } // 块结束

    #[test] // 语句
    fn wallpaper_suitability_rules() { // 函数 wallpaper_suitability_rules
        assert!(wallpaper_suitable(1920, 1080)); // 语句
        assert!(wallpaper_suitable(3840, 2160)); // 语句
        assert!(wallpaper_suitable(1280, 720)); // 语句
        assert!(!wallpaper_suitable(256, 256)); // icon
        assert!(!wallpaper_suitable(800, 1400)); // portrait doc thumb
        assert!(!wallpaper_suitable(1024, 768)); // below min width
        assert!(!wallpaper_suitable(1080, 1920)); // portrait
    } // 块结束

    #[test] // 语句
    fn is_wallpaper_file_by_header() { // 函数 is_wallpaper_file_by_header
        let dir = std::env::temp_dir().join("wp_suit_test"); // 变量 dir
        std::fs::create_dir_all(&dir).unwrap(); // 调用 create_dir_all
        let big = dir.join("big.png"); // 变量 big
        let small = dir.join("icon.png"); // 变量 small
        image::RgbaImage::new(1920, 1080).save(&big).unwrap(); // 调用 new
        image::RgbaImage::new(64, 64).save(&small).unwrap(); // 调用 new
        assert!(is_wallpaper_file(&big), "1920x1080 should qualify"); // 语句
        assert!(!is_wallpaper_file(&small), "64x64 icon should be filtered"); // 语句
        let _ = std::fs::remove_dir_all(&dir); // 变量 _
    } // 块结束

    #[test] // 语句
    fn video_detected_by_extension() { // 函数 video_detected_by_extension
        assert!(is_video_file(Path::new("a.mp4"))); // 语句
        assert!(is_video_file(Path::new("C:/x/B.MP4"))); // 大小写不敏感
        assert!(is_video_file(Path::new("clip.webm"))); // 语句
        assert!(!is_video_file(Path::new("photo.jpg"))); // 语句
        assert!(!is_video_file(Path::new("noext"))); // 语句
    } // 块结束
} // 块结束
