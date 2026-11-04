//! Folder read-permission probing and platform-specific authorization guidance.
//!
//! When a scan hits a folder it cannot read we don't silently drop it: the UI
//! shows a banner that lets the operator either retry (which, on macOS, triggers
//! the system privacy prompt for protected folders) or jump to the platform's
//! authorization page / reveal the folder so they can grant access.

use std::path::Path; // 引入依赖

#[derive(Debug, Clone, Copy, PartialEq, Eq)] // 语句
pub enum PermissionState { // 枚举 PermissionState
    Granted, // 语句
    Denied, // 语句
    NotFound, // 语句
} // 块结束

/// Probe whether we can list a directory. This is a cheap `read_dir` (it does
/// not read file contents) and maps errors to a coarse state for the UI.
pub fn probe_dir(dir: &Path) -> PermissionState { // 函数 probe_dir
    match std::fs::read_dir(dir) { // 分支匹配
        Ok(_) => PermissionState::Granted, // 匹配分支
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => PermissionState::NotFound, // 匹配分支
        Err(_) => PermissionState::Denied, // 匹配分支
    } // 块结束
} // 块结束

/// Human-facing guidance for resolving a permission problem on this platform.
#[derive(Debug, Clone, Copy)] // 语句
pub struct Guidance { // 结构体 Guidance
    pub title: &'static str, // 字段 title
    pub detail: &'static str, // 字段 detail
    pub action: &'static str, // 字段 action
} // 块结束

pub fn guidance() -> Guidance { // 函数 guidance
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        Guidance { // 语句
            title: "需要「完全磁盘访问」权限", // 字段 title
            detail: "macOS 保护了桌面、文稿、下载等目录。请打开 系统设置 → 隐私与安全性 → 完全磁盘访问，勾选本发明，然后点「重试」。首次访问受保护目录时系统也会弹出授权框，选择「允许」即可。", // 字段 detail
            action: "打开授权设置", // 字段 action
        } // 块结束
    } // 块结束
    #[cfg(windows)] // 语句
    { // 语句
        Guidance { // 语句
            title: "需要文件夹读取权限", // 字段 title
            detail: "当前用户没有读取该文件夹的权限。可在资源管理器中右键该文件夹 → 属性 → 安全，为当前用户添加「读取」权限后点「重试」；必要时以管理员身份运行。", // 字段 detail
            action: "打开文件夹位置", // 字段 action
        } // 块结束
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        Guidance { // 语句
            title: "需要文件夹读取权限", // 字段 title
            detail: "当前用户没有读取该文件夹的权限。请为当前用户添加读取权限（例如 chmod/chown），或把壁纸放到本人可读的目录后点「重试」。", // 字段 detail
            action: "打开文件夹位置", // 字段 action
        } // 块结束
    } // 块结束
} // 块结束

/// Open the OS authorization page (macOS) or reveal the problematic folder.
pub fn open_authorization(dir: Option<&Path>) { // 函数 open_authorization
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        let _ = std::process::Command::new("open") // 变量 _
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") // 语句
            .spawn(); // 语句
        let _ = dir; // 变量 _
    } // 块结束
    #[cfg(windows)] // 语句
    { // 语句
        if let Some(d) = dir { // 条件分支
            let _ = std::process::Command::new("explorer").arg(d).spawn(); // 变量 _
        } // 块结束
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        if let Some(d) = dir { // 条件分支
            let _ = std::process::Command::new("xdg-open").arg(d).spawn(); // 变量 _
        } // 块结束
    } // 块结束
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn probe_detects_granted_and_not_found() { // 函数 probe_detects_granted_and_not_found
        let dir = std::env::temp_dir(); // 变量 dir
        assert_eq!(probe_dir(&dir), PermissionState::Granted); // 语句
        let missing = dir.join("definitely_missing_wp_dir_zzz"); // 变量 missing
        assert_eq!(probe_dir(&missing), PermissionState::NotFound); // 语句
    } // 块结束
} // 块结束
