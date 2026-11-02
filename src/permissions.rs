//! Folder read-permission probing and platform-specific authorization guidance.
//!
//! When a scan hits a folder it cannot read we don't silently drop it: the UI
//! shows a banner that lets the operator either retry (which, on macOS, triggers
//! the system privacy prompt for protected folders) or jump to the platform's
//! authorization page / reveal the folder so they can grant access.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied,
    NotFound,
}

/// Probe whether we can list a directory. This is a cheap `read_dir` (it does
/// not read file contents) and maps errors to a coarse state for the UI.
pub fn probe_dir(dir: &Path) -> PermissionState {
    match std::fs::read_dir(dir) {
        Ok(_) => PermissionState::Granted,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => PermissionState::NotFound,
        Err(_) => PermissionState::Denied,
    }
}

/// Human-facing guidance for resolving a permission problem on this platform.
#[derive(Debug, Clone, Copy)]
pub struct Guidance {
    pub title: &'static str,
    pub detail: &'static str,
    pub action: &'static str,
}

pub fn guidance() -> Guidance {
    #[cfg(target_os = "macos")]
    {
        Guidance {
            title: "需要「完全磁盘访问」权限",
            detail: "macOS 保护了桌面、文稿、下载等目录。请打开 系统设置 → 隐私与安全性 → 完全磁盘访问，勾选本发明，然后点「重试」。首次访问受保护目录时系统也会弹出授权框，选择「允许」即可。",
            action: "打开授权设置",
        }
    }
    #[cfg(windows)]
    {
        Guidance {
            title: "需要文件夹读取权限",
            detail: "当前用户没有读取该文件夹的权限。可在资源管理器中右键该文件夹 → 属性 → 安全，为当前用户添加「读取」权限后点「重试」；必要时以管理员身份运行。",
            action: "打开文件夹位置",
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Guidance {
            title: "需要文件夹读取权限",
            detail: "当前用户没有读取该文件夹的权限。请为当前用户添加读取权限（例如 chmod/chown），或把壁纸放到本人可读的目录后点「重试」。",
            action: "打开文件夹位置",
        }
    }
}

/// Open the OS authorization page (macOS) or reveal the problematic folder.
pub fn open_authorization(dir: Option<&Path>) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
        let _ = dir;
    }
    #[cfg(windows)]
    {
        if let Some(d) = dir {
            let _ = std::process::Command::new("explorer").arg(d).spawn();
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(d) = dir {
            let _ = std::process::Command::new("xdg-open").arg(d).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_detects_granted_and_not_found() {
        let dir = std::env::temp_dir();
        assert_eq!(probe_dir(&dir), PermissionState::Granted);
        let missing = dir.join("definitely_missing_wp_dir_zzz");
        assert_eq!(probe_dir(&missing), PermissionState::NotFound);
    }
}
