//! Group wallpaper images into albums by their containing folder.
//!
//! "Album" = one folder. With a recursive scan the album key is the folder path
//! relative to the workspace root, so nested folders become separate albums and
//! the key stays stable across runs (used for the collapse/expand state).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album {
    /// Stable identifier (relative folder path, or the absolute path when there
    /// is no workspace root, e.g. after a full-disk scan).
    pub key: String,
    /// Display name: the folder's own name.
    pub name: String,
    /// Absolute folder path.
    pub dir: PathBuf,
    /// Images in this album (sorted by path).
    pub images: Vec<PathBuf>,
}

/// Group `images` by their parent folder. `root` is the workspace folder used to
/// compute relative album keys; pass `None` to key albums by absolute path.
pub fn build_albums(images: &[PathBuf], root: Option<&Path>) -> Vec<Album> {
    let mut map: BTreeMap<String, Album> = BTreeMap::new();
    for p in images {
        let parent = p.parent().unwrap_or_else(|| Path::new(""));
        let key = album_key(parent, root);
        let name = album_name(parent, root);
        let entry = map.entry(key.clone()).or_insert_with(|| Album {
            key,
            name,
            dir: parent.to_path_buf(),
            images: Vec::new(),
        });
        entry.images.push(p.clone());
    }
    let mut albums: Vec<Album> = map.into_values().collect();
    for a in &mut albums {
        a.images.sort();
    }
    // Root album (key ".") first, then folders alphabetically.
    albums.sort_by(|a, b| {
        let ra = a.key != ".";
        let rb = b.key != ".";
        ra.cmp(&rb)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.key.cmp(&b.key))
    });
    albums
}

fn album_key(dir: &Path, root: Option<&Path>) -> String {
    if let Some(r) = root {
        if let Ok(rel) = dir.strip_prefix(r) {
            let s = rel.to_string_lossy().replace('\\', "/");
            return if s.is_empty() { ".".to_string() } else { s };
        }
    }
    dir.to_string_lossy().replace('\\', "/")
}

fn album_name(dir: &Path, root: Option<&Path>) -> String {
    if let Some(r) = root {
        if dir == r {
            if let Some(n) = r.file_name() {
                return n.to_string_lossy().to_string();
            }
        }
    }
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_folder_and_uses_relative_keys() {
        let root = Path::new("C:/pics");
        let imgs = vec![
            PathBuf::from("C:/pics/b.jpg"),
            PathBuf::from("C:/pics/a.jpg"),
            PathBuf::from("C:/pics/山峦/x.jpg"),
            PathBuf::from("C:/pics/山峦/y.jpg"),
            PathBuf::from("C:/pics/海洋/z.jpg"),
        ];
        let albums = build_albums(&imgs, Some(root));
        assert_eq!(albums.len(), 3);

        // Root album first and holds exactly the two root-level images.
        assert_eq!(albums[0].key, ".");
        assert_eq!(albums[0].name, "pics");
        assert_eq!(albums[0].images.len(), 2);
        // Images are sorted within an album.
        assert_eq!(albums[0].images[0], PathBuf::from("C:/pics/a.jpg"));

        // Nested folders get relative keys and their own names.
        let names: Vec<&str> = albums.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"山峦"));
        assert!(names.contains(&"海洋"));
        let mountain = albums.iter().find(|a| a.name == "山峦").unwrap();
        assert_eq!(mountain.key, "山峦");
        assert_eq!(mountain.images.len(), 2);
    }

    #[test]
    fn without_root_keys_by_absolute_parent() {
        let imgs = vec![PathBuf::from("C:/a/p.jpg"), PathBuf::from("C:/b/q.jpg")];
        let albums = build_albums(&imgs, None);
        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].key, "C:/a");
        assert_eq!(albums[1].key, "C:/b");
    }
}
