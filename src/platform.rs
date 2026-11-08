//! Platform detection and mobile capability model.
//!
//! The desktop build (Windows / macOS / Linux) can set the system wallpaper
//! directly. On mobile the situation differs per OS:
//!
//! * Android — an app may request the `SET_WALLPAPER` permission and then hand
//!   the bitmap to `WallpaperManager`. That needs a JNI bridge (see the mobile
//!   build notes), so this module only reports the capability here.
//! * iOS — Apple does **not** allow third-party apps to change the wallpaper
//!   programmatically; the only path is the system Photos "Set as Wallpaper"
//!   flow, which the user performs manually.
//!
//! The UI uses this to label the platform and, when running on a phone/tablet,
//! to show an accurate note instead of pretending the wallpaper was applied.

#[allow(dead_code)] // 语句
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // 语句
pub enum Os { // 枚举 Os
    Windows, // 语句
    MacOs, // 语句
    Linux, // 语句
    Android, // 语句
    Ios, // 语句
    Other, // 语句
} // 块结束

pub fn os() -> Os { // 函数 os
    #[cfg(windows)] // 语句
    { // 语句
        Os::Windows // 语句
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        Os::MacOs // 语句
    } // 块结束
    #[cfg(target_os = "android")] // 语句
    { // 语句
        Os::Android // 语句
    } // 块结束
    #[cfg(target_os = "ios")] // 语句
    { // 语句
        Os::Ios // 语句
    } // 块结束
    #[cfg(all( // 语句
        unix, // 语句
        not(target_os = "macos"), // 调用 not
        not(target_os = "android"), // 调用 not
        not(target_os = "ios") // 调用 not
    ))] // 语句
    { // 语句
        Os::Linux // 语句
    } // 块结束
    #[cfg(not(any( // 语句
        windows, // 语句
        target_os = "macos", // 语句
        target_os = "android", // 语句
        target_os = "ios", // 语句
        all( // 调用 all
            unix, // 语句
            not(target_os = "macos"), // 调用 not
            not(target_os = "android"), // 调用 not
            not(target_os = "ios") // 调用 not
        ) // 块结束
    )))] // 语句
    { // 语句
        Os::Other // 语句
    } // 块结束
} // 块结束

pub fn name() -> &'static str { // 函数 name
    match os() { // 分支匹配
        Os::Windows => "Windows", // 匹配分支
        Os::MacOs => "macOS", // 匹配分支
        Os::Linux => "Linux", // 匹配分支
        Os::Android => "Android", // 匹配分支
        Os::Ios => "iOS", // 匹配分支
        Os::Other => "Unknown", // 匹配分支
    } // 块结束
} // 块结束

/// True on phone/tablet targets (Android / iOS). Used to force the compact,
/// touch-friendly layout regardless of window width.
pub fn is_mobile() -> bool { // 函数 is_mobile
    matches!(os(), Os::Android | Os::Ios) // 语句
} // 块结束

/// What "set wallpaper" can do on this platform, and how to explain it to users.
#[allow(dead_code)] // 语句
#[derive(Debug, Clone, Copy)] // 语句
pub struct WallpaperSupport { // 结构体 WallpaperSupport
    /// Whether the app can apply the system wallpaper itself.
    pub can_set: bool, // 字段 can_set
    /// Short label for the settings/about UI.
    pub label: &'static str, // 字段 label
    /// One-line explanation of the capability / limitation.
    pub note: &'static str, // 字段 note
} // 块结束

pub fn wallpaper_support() -> WallpaperSupport { // 函数 wallpaper_support
    match os() { // 分支匹配
        Os::Windows | Os::MacOs | Os::Linux => WallpaperSupport { // 匹配分支
            can_set: true, // 字段 can_set
            label: "桌面壁纸", // 字段 label
            note: "可直接把所选图片设置为系统桌面壁纸。", // 字段 note
        }, // 块结束
        // Android：已声明 SET_WALLPAPER 权限，并通过 JNI 调用 WallpaperManager
        // 设置主屏 / 锁屏壁纸（见 `android_jni`）。
        Os::Android => WallpaperSupport { // 匹配分支
            can_set: true, // 字段 can_set
            label: "Android 壁纸", // 字段 label
            note: "可直接把所选图片设为系统主屏，或设为锁屏壁纸（通过 WallpaperManager）。", // 字段 note
        }, // 块结束
        // iOS forbids programmatic wallpaper changes.
        Os::Ios => WallpaperSupport { // 匹配分支
            can_set: false, // 字段 can_set
            label: "iOS 壁纸", // 字段 label
            note: "iOS 不允许 App 直接更改壁纸：请用「导出到相册」保存图片，再到「照片」里手动「用作墙纸」。", // 字段 note
        }, // 块结束
        Os::Other => WallpaperSupport { // 匹配分支
            can_set: false, // 字段 can_set
            label: "壁纸", // 字段 label
            note: "当前平台暂不支持直接设置系统壁纸。", // 字段 note
        }, // 块结束
    } // 块结束
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn desktop_os_is_not_mobile() { // 函数 desktop_os_is_not_mobile
        // This test only ever runs on a desktop CI target.
        assert!(!is_mobile()); // 语句
        let s = wallpaper_support(); // 变量 s
        assert!(s.can_set); // 语句
    } // 块结束

    #[test] // 语句
    fn name_is_non_empty() { // 函数 name_is_non_empty
        assert!(!name().is_empty()); // 语句
    } // 块结束
} // 块结束
