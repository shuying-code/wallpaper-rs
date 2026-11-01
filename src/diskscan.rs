//! Bounded background full-disk scan for image files.
//!
//! Scans all fixed drive roots (Windows) while skipping well-known system /
//! cache directories, collecting up to a cap of images. Results are sent back
//! over a channel so the UI stays responsive.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

const FILE_CAP: usize = 3000;
const VISIT_CAP: u64 = 600_000;
const MIN_BYTES: u64 = 20_000;

const SKIP_DIRS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
    "node_modules",
    ".git",
    "appdata",
    "windows.old",
    "recovery",
    "$windows.~bt",
    "$windows.~ws",
    "config.msi",
];

pub fn start() -> Receiver<Vec<PathBuf>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut found: Vec<PathBuf> = Vec::new();
        let mut visited: u64 = 0;
        for root in roots() {
            if found.len() >= FILE_CAP || visited > VISIT_CAP {
                break;
            }
            walk(&root, &mut found, &mut visited);
        }
        found.sort();
        found.dedup();
        let _ = tx.send(found);
    });
    rx
}

#[cfg(windows)]
fn roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in b'A'..=b'Z' {
        let p = format!("{}:\\", d as char);
        if Path::new(&p).exists() {
            v.push(PathBuf::from(p));
        }
    }
    v
}

#[cfg(not(windows))]
fn roots() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/")];
    if let Some(h) = std::env::var_os("HOME") {
        v.push(PathBuf::from(h));
    }
    v
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>, visited: &mut u64) {
    if found.len() >= FILE_CAP || *visited > VISIT_CAP {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for e in rd.flatten() {
        *visited += 1;
        if *visited > VISIT_CAP || found.len() >= FILE_CAP {
            return;
        }
        let p = e.path();
        let md = match e.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.is_dir() {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(&p, found, visited);
        } else if md.is_file() && md.len() >= MIN_BYTES && is_image(&p) {
            found.push(p);
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
