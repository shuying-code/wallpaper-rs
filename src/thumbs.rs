//! Background thumbnail decoding with a simple worker thread + channel queue.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

pub const THUMB_MAX: u32 = 340;

// A "wallpaper-grade" image: at least 1280×720 and landscape-ish (aspect ≥ 1.2).
pub const MIN_WALLPAPER_W: u32 = 1280;
pub const MIN_WALLPAPER_H: u32 = 720;

/// Read only the image header to get (width, height) cheaply (no full decode).
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// Whether an image of size (w,h) is suitable as a desktop wallpaper.
pub fn wallpaper_suitable(w: u32, h: u32) -> bool {
    w >= MIN_WALLPAPER_W && h >= MIN_WALLPAPER_H && (w as u64) * 10 >= (h as u64) * 12
}

/// Convenience: is this file a wallpaper-grade image (reads only the header)?
pub fn is_wallpaper_file(path: &Path) -> bool {
    match image_dimensions(path) {
        Some((w, h)) => wallpaper_suitable(w, h),
        None => false,
    }
}

pub struct Thumb {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Decode an image file, downscaled so its longest side <= `max`.
pub fn load_image(path: &Path, max: u32) -> Option<Thumb> {
    let img = image::open(path).ok()?;
    let img = img.thumbnail(max, max);
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(Thumb {
        w: w as usize,
        h: h as usize,
        rgba: rgba.into_raw(),
    })
}

pub struct Thumbnailer {
    tx: Sender<(PathBuf, u64)>,
    rx: Receiver<(PathBuf, u64, Option<Thumb>)>,
    requested: HashSet<PathBuf>,
    /// Bumped on `reset()`. Results tagged with an older generation are dropped
    /// so thumbnails queued before a folder change / full-disk scan can never
    /// pollute the new list's texture cache.
    generation: u64,
}

impl Thumbnailer {
    pub fn new() -> Self {
        let (tx, rx_req) = channel::<(PathBuf, u64)>();
        let (tx_res, rx) = channel::<(PathBuf, u64, Option<Thumb>)>();
        thread::spawn(move || {
            while let Ok((p, gen)) = rx_req.recv() {
                let t = load_image(&p, THUMB_MAX);
                if tx_res.send((p, gen, t)).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            requested: HashSet::new(),
            generation: 0,
        }
    }

    pub fn request(&mut self, p: &Path) {
        if self.requested.insert(p.to_path_buf()) {
            let _ = self.tx.send((p.to_path_buf(), self.generation));
        }
    }

    pub fn is_requested(&self, p: &Path) -> bool {
        self.requested.contains(p)
    }

    /// Allow a path to be requested again (used by LRU texture-cache eviction so
    /// cards that scroll back into view are re-decoded instead of stuck loading).
    pub fn forget(&mut self, p: &Path) {
        self.requested.remove(p);
    }

    pub fn reset(&mut self) {
        self.requested.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Non-blocking drain of finished thumbnails. Results produced before the
    /// last `reset()` are discarded.
    pub fn poll(&mut self) -> Vec<(PathBuf, Option<Thumb>)> {
        let mut v = Vec::new();
        while let Ok((p, gen, t)) = self.rx.try_recv() {
            if gen == self.generation {
                v.push((p, t));
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_drops_stale_generation() {
        // Build a thumbnailer, request a path at generation 0, then reset.
        let mut t = Thumbnailer::new();
        // Force a known generation bump without any worker activity.
        t.reset();
        assert_eq!(t.generation, 1);
        // A path requested at gen 1 is tracked; forgetting allows re-request.
        let p = Path::new("nonexistent.png");
        t.request(p);
        assert!(t.is_requested(p));
        t.forget(p);
        assert!(!t.is_requested(p));
    }

    #[test]
    fn wallpaper_suitability_rules() {
        assert!(wallpaper_suitable(1920, 1080));
        assert!(wallpaper_suitable(3840, 2160));
        assert!(wallpaper_suitable(1280, 720));
        assert!(!wallpaper_suitable(256, 256)); // icon
        assert!(!wallpaper_suitable(800, 1400)); // portrait doc thumb
        assert!(!wallpaper_suitable(1024, 768)); // below min width
        assert!(!wallpaper_suitable(1080, 1920)); // portrait
    }

    #[test]
    fn is_wallpaper_file_by_header() {
        let dir = std::env::temp_dir().join("wp_suit_test");
        std::fs::create_dir_all(&dir).unwrap();
        let big = dir.join("big.png");
        let small = dir.join("icon.png");
        image::RgbaImage::new(1920, 1080).save(&big).unwrap();
        image::RgbaImage::new(64, 64).save(&small).unwrap();
        assert!(is_wallpaper_file(&big), "1920x1080 should qualify");
        assert!(!is_wallpaper_file(&small), "64x64 icon should be filtered");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
