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
    tx: Sender<PathBuf>,
    rx: Receiver<(PathBuf, Option<Thumb>)>,
    requested: HashSet<PathBuf>,
}

impl Thumbnailer {
    pub fn new() -> Self {
        let (tx, rx_req) = channel::<PathBuf>();
        let (tx_res, rx) = channel::<(PathBuf, Option<Thumb>)>();
        thread::spawn(move || {
            while let Ok(p) = rx_req.recv() {
                let t = load_image(&p, THUMB_MAX);
                if tx_res.send((p, t)).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            requested: HashSet::new(),
        }
    }

    pub fn request(&mut self, p: &Path) {
        if self.requested.insert(p.to_path_buf()) {
            let _ = self.tx.send(p.to_path_buf());
        }
    }

    pub fn reset(&mut self) {
        self.requested.clear();
    }

    /// Non-blocking drain of finished thumbnails.
    pub fn poll(&mut self) -> Vec<(PathBuf, Option<Thumb>)> {
        let mut v = Vec::new();
        while let Ok(x) = self.rx.try_recv() {
            v.push(x);
        }
        v
    }
}
