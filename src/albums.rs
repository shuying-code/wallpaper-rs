//! Group wallpaper images into albums by their containing folder.
//!
//! "Album" = one folder. With a recursive scan the album key is the folder path
//! relative to the workspace root, so nested folders become separate albums and
//! the key stays stable across runs (used for the collapse/expand state).

use std::collections::BTreeMap; // 引入依赖
use std::path::{Path, PathBuf}; // 引入依赖

#[derive(Debug, Clone, PartialEq, Eq)] // 语句
pub struct Album { // 结构体 Album
    /// Stable identifier (relative folder path, or the absolute path when there
    /// is no workspace root, e.g. after a full-disk scan).
    pub key: String, // 字段 key
    /// Display name: the folder's own name.
    pub name: String, // 字段 name
    /// Absolute folder path.
    pub dir: PathBuf, // 字段 dir
    /// Images in this album (sorted by path).
    pub images: Vec<PathBuf>, // 字段 images
} // 块结束

/// Group `images` by their parent folder. `root` is the workspace folder used to
/// compute relative album keys; pass `None` to key albums by absolute path.
pub fn build_albums(images: &[PathBuf], root: Option<&Path>) -> Vec<Album> { // 函数 build_albums
    let mut map: BTreeMap<String, Album> = BTreeMap::new(); // 变量 map
    for p in images { // 循环
        let parent = p.parent().unwrap_or_else(|| Path::new("")); // 变量 parent
        let key = album_key(parent, root); // 变量 key
        let name = album_name(parent, root); // 变量 name
        let entry = map.entry(key.clone()).or_insert_with(|| Album { // 变量 entry
            key, // 语句
            name, // 语句
            dir: parent.to_path_buf(), // 字段 dir
            images: Vec::new(), // 字段 images
        }); // 语句
        entry.images.push(p.clone()); // 语句
    } // 块结束
    let mut albums: Vec<Album> = map.into_values().collect(); // 变量 albums
    for a in &mut albums { // 循环
        a.images.sort(); // 语句
    } // 块结束
    // Root album (key ".") first, then folders alphabetically.
    albums.sort_by(|a, b| { // 语句
        let ra = a.key != "."; // 变量 ra
        let rb = b.key != "."; // 变量 rb
        ra.cmp(&rb) // 语句
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())) // 语句
            .then_with(|| a.key.cmp(&b.key)) // 语句
    }); // 语句
    albums // 语句
} // 块结束

fn album_key(dir: &Path, root: Option<&Path>) -> String { // 函数 album_key
    if let Some(r) = root { // 条件分支
        if let Ok(rel) = dir.strip_prefix(r) { // 条件分支
            let s = rel.to_string_lossy().replace('\\', "/"); // 变量 s
            return if s.is_empty() { ".".to_string() } else { s }; // 返回
        } // 块结束
    } // 块结束
    dir.to_string_lossy().replace('\\', "/") // 语句
} // 块结束

fn album_name(dir: &Path, root: Option<&Path>) -> String { // 函数 album_name
    if let Some(r) = root { // 条件分支
        if dir == r { // 条件分支
            if let Some(n) = r.file_name() { // 条件分支
                return n.to_string_lossy().to_string(); // 返回
            } // 块结束
        } // 块结束
    } // 块结束
    dir.file_name() // 语句
        .map(|n| n.to_string_lossy().to_string()) // 语句
        .unwrap_or_else(|| dir.display().to_string()) // 语句
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn groups_by_folder_and_uses_relative_keys() { // 函数 groups_by_folder_and_uses_relative_keys
        let root = Path::new("C:/pics"); // 变量 root
        let imgs = vec![ // 变量 imgs
            PathBuf::from("C:/pics/b.jpg"), // 字段 PathBuf
            PathBuf::from("C:/pics/a.jpg"), // 字段 PathBuf
            PathBuf::from("C:/pics/山峦/x.jpg"), // 字段 PathBuf
            PathBuf::from("C:/pics/山峦/y.jpg"), // 字段 PathBuf
            PathBuf::from("C:/pics/海洋/z.jpg"), // 字段 PathBuf
        ]; // 语句
        let albums = build_albums(&imgs, Some(root)); // 变量 albums
        assert_eq!(albums.len(), 3); // 语句

        // Root album first and holds exactly the two root-level images.
        assert_eq!(albums[0].key, "."); // 语句
        assert_eq!(albums[0].name, "pics"); // 语句
        assert_eq!(albums[0].images.len(), 2); // 语句
        // Images are sorted within an album.
        assert_eq!(albums[0].images[0], PathBuf::from("C:/pics/a.jpg")); // 语句

        // Nested folders get relative keys and their own names.
        let names: Vec<&str> = albums.iter().map(|a| a.name.as_str()).collect(); // 变量 names
        assert!(names.contains(&"山峦")); // 语句
        assert!(names.contains(&"海洋")); // 语句
        let mountain = albums.iter().find(|a| a.name == "山峦").unwrap(); // 变量 mountain
        assert_eq!(mountain.key, "山峦"); // 语句
        assert_eq!(mountain.images.len(), 2); // 语句
    } // 块结束

    #[test] // 语句
    fn without_root_keys_by_absolute_parent() { // 函数 without_root_keys_by_absolute_parent
        let imgs = vec![PathBuf::from("C:/a/p.jpg"), PathBuf::from("C:/b/q.jpg")]; // 变量 imgs
        let albums = build_albums(&imgs, None); // 变量 albums
        assert_eq!(albums.len(), 2); // 语句
        assert_eq!(albums[0].key, "C:/a"); // 语句
        assert_eq!(albums[1].key, "C:/b"); // 语句
    } // 块结束
} // 块结束
