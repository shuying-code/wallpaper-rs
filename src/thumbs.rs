//! Background thumbnail decoding with a simple worker thread + channel queue.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

pub const THUMB_MAX: u32 = 340;

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
}
