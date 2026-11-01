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

/// Start a background scan. When `wallpaper_only` is true, only wallpaper-grade
/// images (see [`crate::thumbs::wallpaper_suitable`]) are returned.
pub fn start(wallpaper_only: bool) -> Receiver<Vec<PathBuf>> {
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
        if wallpaper_only {
            found.retain(|p| crate::thumbs::is_wallpaper_file(p));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_filters_size_extension_and_skip_dirs() {
        let root = std::env::temp_dir().join("wp_diskscan_test");
        let _ = std::fs::remove_dir_all(&root);
        let big = vec![0u8; (MIN_BYTES + 1024) as usize];
        let small = vec![0u8; 512];

        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(root.join("a.png"), &big).unwrap(); // found
        std::fs::write(root.join("tiny.png"), &small).unwrap(); // too small -> skip
        std::fs::write(root.join("notes.txt"), &big).unwrap(); // not image -> skip
        std::fs::write(root.join("sub").join("b.jpg"), &big).unwrap(); // recursive -> found
        std::fs::write(root.join("node_modules").join("c.png"), &big).unwrap(); // skipped dir

        let mut found = Vec::new();
        let mut visited = 0u64;
        walk(&root, &mut found, &mut visited);
        found.sort();

        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"a.png".to_string()), "root image should be found");
        assert!(names.contains(&"b.jpg".to_string()), "nested image should be found");
        assert!(!names.contains(&"tiny.png".to_string()), "undersized file skipped");
        assert!(!names.contains(&"notes.txt".to_string()), "non-image skipped");
        assert!(!names.contains(&"c.png".to_string()), "node_modules must be skipped");

        let _ = std::fs::remove_dir_all(&root);
    }
}
