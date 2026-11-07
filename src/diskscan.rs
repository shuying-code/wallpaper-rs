//! Bounded background full-disk scan for image files.
//!
//! Scans all fixed drive roots (Windows) while skipping well-known system /
//! cache directories, collecting up to a cap of images. Results are sent back
//! over a channel so the UI stays responsive.

use std::path::{Path, PathBuf}; // 引入依赖
use std::sync::mpsc::{channel, Receiver}; // 引入依赖

const FILE_CAP: usize = 3000; // 常量 FILE_CAP
const VISIT_CAP: u64 = 600_000; // 常量 VISIT_CAP
const MIN_BYTES: u64 = 20_000; // 常量 MIN_BYTES

const SKIP_DIRS: &[&str] = &[ // 常量 SKIP_DIRS
    "windows", // 语句
    "program files", // 语句
    "program files (x86)", // 语句
    "programdata", // 语句
    "$recycle.bin", // 语句
    "system volume information", // 语句
    "node_modules", // 语句
    ".git", // 语句
    "appdata", // 语句
    "windows.old", // 语句
    "recovery", // 语句
    "$windows.~bt", // 语句
    "$windows.~ws", // 语句
    "config.msi", // 语句
]; // 语句

/// Start a background scan. When `wallpaper_only` is true, only wallpaper-grade
/// images (see [`crate::thumbs::wallpaper_suitable`]) are returned.
pub fn start(wallpaper_only: bool) -> Receiver<Vec<PathBuf>> { // 函数 start
    let (tx, rx) = channel(); // 调用 let
    std::thread::spawn(move || { // 调用 spawn
        let mut found: Vec<PathBuf> = Vec::new(); // 变量 found
        let mut visited: u64 = 0; // 变量 visited
        for root in roots() { // 循环
            if found.len() >= FILE_CAP || visited > VISIT_CAP { // 条件分支
                break; // 语句
            } // 块结束
            walk(&root, &mut found, &mut visited); // 调用 walk
        } // 块结束
        found.sort(); // 语句
        found.dedup(); // 语句
        if wallpaper_only { // 条件分支
            // 视频（动态壁纸）无法用图片头判断尺寸，直接保留。
            found.retain(|p| { // 调用 retain
                crate::thumbs::is_video_file(p) || crate::thumbs::is_wallpaper_file(p) // 语句
            }); // 语句
        } // 块结束
        let _ = tx.send(found); // 变量 _
    }); // 语句
    rx // 语句
} // 块结束

#[cfg(windows)] // 语句
fn roots() -> Vec<PathBuf> { // 函数 roots
    let mut v = Vec::new(); // 变量 v
    for d in b'A'..=b'Z' { // 循环
        let p = format!("{}:\\", d as char); // 变量 p
        if Path::new(&p).exists() { // 条件分支
            v.push(PathBuf::from(p)); // 语句
        } // 块结束
    } // 块结束
    v // 语句
} // 块结束

#[cfg(not(windows))] // 语句
fn roots() -> Vec<PathBuf> { // 函数 roots
    let mut v = vec![PathBuf::from("/")]; // 变量 v
    if let Some(h) = std::env::var_os("HOME") { // 条件分支
        v.push(PathBuf::from(h)); // 语句
    } // 块结束
    v // 语句
} // 块结束

fn walk(dir: &Path, found: &mut Vec<PathBuf>, visited: &mut u64) { // 函数 walk
    if found.len() >= FILE_CAP || *visited > VISIT_CAP { // 条件分支
        return; // 返回
    } // 块结束
    let rd = match std::fs::read_dir(dir) { // 变量 rd
        Ok(r) => r, // 匹配分支
        Err(_) => return, // 匹配分支
    }; // 块结束
    for e in rd.flatten() { // 循环
        *visited += 1; // 语句
        if *visited > VISIT_CAP || found.len() >= FILE_CAP { // 条件分支
            return; // 返回
        } // 块结束
        let p = e.path(); // 变量 p
        let md = match e.metadata() { // 变量 md
            Ok(m) => m, // 匹配分支
            Err(_) => continue, // 匹配分支
        }; // 块结束
        if md.is_dir() { // 条件分支
            let name = p // 变量 name
                .file_name() // 语句
                .map(|s| s.to_string_lossy().to_ascii_lowercase()) // 语句
                .unwrap_or_default(); // 语句
            if SKIP_DIRS.contains(&name.as_str()) { // 条件分支
                continue; // 语句
            } // 块结束
            walk(&p, found, visited); // 调用 walk
        } else if md.is_file() && md.len() >= MIN_BYTES && is_media(&p) { // 条件分支
            found.push(p); // 语句
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

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn walk_filters_size_extension_and_skip_dirs() { // 函数 walk_filters_size_extension_and_skip_dirs
        let root = std::env::temp_dir().join("wp_diskscan_test"); // 变量 root
        let _ = std::fs::remove_dir_all(&root); // 变量 _
        let big = vec![0u8; (MIN_BYTES + 1024) as usize]; // 变量 big
        let small = vec![0u8; 512]; // 变量 small

        std::fs::create_dir_all(&root).unwrap(); // 调用 create_dir_all
        std::fs::create_dir_all(root.join("sub")).unwrap(); // 调用 create_dir_all
        std::fs::create_dir_all(root.join("node_modules")).unwrap(); // 调用 create_dir_all
        std::fs::write(root.join("a.png"), &big).unwrap(); // found
        std::fs::write(root.join("tiny.png"), &small).unwrap(); // too small -> skip
        std::fs::write(root.join("notes.txt"), &big).unwrap(); // not image -> skip
        std::fs::write(root.join("sub").join("b.jpg"), &big).unwrap(); // recursive -> found
        std::fs::write(root.join("node_modules").join("c.png"), &big).unwrap(); // skipped dir

        let mut found = Vec::new(); // 变量 found
        let mut visited = 0u64; // 变量 visited
        walk(&root, &mut found, &mut visited); // 调用 walk
        found.sort(); // 语句

        let names: Vec<String> = found // 变量 names
            .iter() // 语句
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string()) // 语句
            .collect(); // 语句
        assert!(names.contains(&"a.png".to_string()), "root image should be found"); // 语句
        assert!(names.contains(&"b.jpg".to_string()), "nested image should be found"); // 语句
        assert!(!names.contains(&"tiny.png".to_string()), "undersized file skipped"); // 语句
        assert!(!names.contains(&"notes.txt".to_string()), "non-image skipped"); // 语句
        assert!(!names.contains(&"c.png".to_string()), "node_modules must be skipped"); // 语句

        let _ = std::fs::remove_dir_all(&root); // 变量 _
    } // 块结束
} // 块结束
