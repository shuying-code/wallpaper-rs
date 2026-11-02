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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    Android,
    Ios,
    Other,
}

pub fn os() -> Os {
    #[cfg(windows)]
    {
        Os::Windows
    }
    #[cfg(target_os = "macos")]
    {
        Os::MacOs
    }
    #[cfg(target_os = "android")]
    {
        Os::Android
    }
    #[cfg(target_os = "ios")]
    {
        Os::Ios
    }
    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        Os::Linux
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "android",
        target_os = "ios",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "android"),
            not(target_os = "ios")
        )
    )))]
    {
        Os::Other
    }
}

pub fn name() -> &'static str {
    match os() {
        Os::Windows => "Windows",
        Os::MacOs => "macOS",
        Os::Linux => "Linux",
        Os::Android => "Android",
        Os::Ios => "iOS",
        Os::Other => "Unknown",
    }
}

/// True on phone/tablet targets (Android / iOS). Used to force the compact,
/// touch-friendly layout regardless of window width.
pub fn is_mobile() -> bool {
    matches!(os(), Os::Android | Os::Ios)
}

/// What "set wallpaper" can do on this platform, and how to explain it to users.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct WallpaperSupport {
    /// Whether the app can apply the system wallpaper itself.
    pub can_set: bool,
    /// Short label for the settings/about UI.
    pub label: &'static str,
    /// One-line explanation of the capability / limitation.
    pub note: &'static str,
}

pub fn wallpaper_support() -> WallpaperSupport {
    match os() {
        Os::Windows | Os::MacOs | Os::Linux => WallpaperSupport {
            can_set: true,
            label: "桌面壁纸",
            note: "可直接把所选图片设置为系统桌面壁纸。",
        },
        // Android: needs the SET_WALLPAPER permission + JNI bridge in the mobile
        // build; the UI still lets users browse and pick images.
        Os::Android => WallpaperSupport {
            can_set: false,
            label: "Android 壁纸",
            note: "Android 需在移动端构建中申请 SET_WALLPAPER 权限并通过 WallpaperManager 设置；当前构建可浏览与缩放预览，设置动作在移动宿主中完成。",
        },
        // iOS forbids programmatic wallpaper changes.
        Os::Ios => WallpaperSupport {
            can_set: false,
            label: "iOS 壁纸",
            note: "iOS 不允许 App 直接更改壁纸：请用「导出到相册」保存图片，再到「照片」里手动「用作墙纸」。",
        },
        Os::Other => WallpaperSupport {
            can_set: false,
            label: "壁纸",
            note: "当前平台暂不支持直接设置系统壁纸。",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_os_is_not_mobile() {
        // This test only ever runs on a desktop CI target.
        assert!(!is_mobile());
        let s = wallpaper_support();
        assert!(s.can_set);
    }

    #[test]
    fn name_is_non_empty() {
        assert!(!name().is_empty());
    }
}
