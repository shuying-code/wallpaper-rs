//! Launch-at-login (autostart) integration.
//!
//! `Config::autostart` is the source of truth. At startup the app reconciles
//! that setting with the actual OS registration, which repairs stale entries
//! left by an older executable path and removes entries after the user turns
//! the option off.

#[cfg(not(target_os = "android"))]
use std::path::PathBuf;

const AUTOSTART_ARG: &str = "--autostart";

/// Whether this platform has an autostart implementation.
pub fn supported() -> bool {
    #[cfg(any(
        windows,
        target_os = "macos",
        target_os = "android",
        all(unix, not(target_os = "macos"), not(target_os = "android"))
    ))]
    {
        true
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "android",
        all(unix, not(target_os = "macos"), not(target_os = "android"))
    )))]
    {
        false
    }
}

/// True when this process was launched by the autostart entry.
pub fn started_from_autostart() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
        || std::env::var_os("WALLPAPER_RS_AUTOSTART").is_some_and(|value| !value.is_empty())
}

/// Apply the requested state and verify the result.
pub fn set_enabled(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        return windows_set(enable);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_set(enable);
    }
    #[cfg(target_os = "android")]
    {
        return android_set(enable);
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        return linux_set(enable);
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持开机自启".to_string())
}

/// Re-apply the persisted setting. This intentionally rewrites enabled
/// entries so an executable that was moved or renamed is repaired at launch.
pub fn reconcile(enable: bool) -> Result<(), String> {
    set_enabled(enable)
}

/// Return whether the OS currently has an entry for this app.
#[allow(dead_code)]
pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        return windows_is_enabled();
    }
    #[cfg(target_os = "macos")]
    {
        return macos_plist_path().is_file();
    }
    #[cfg(target_os = "android")]
    {
        return crate::android_jni::is_autostart_enabled().unwrap_or(false);
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        return linux_desktop_path().is_file();
    }
    #[allow(unreachable_code)]
    false
}

#[cfg(any(
    windows,
    target_os = "macos",
    all(unix, not(target_os = "macos"), not(target_os = "android"))
))]
fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| format!("无法获取可执行文件路径：{error}"))
}

// ---- Windows -------------------------------------------------------------

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "wallpaper-rs";

#[cfg(windows)]
fn windows_command() -> Result<String, String> {
    let exe = current_exe()?;
    Ok(format!("\"{}\" {AUTOSTART_ARG}", exe.display()))
}

#[cfg(windows)]
fn windows_run_key(write: bool) -> Result<winreg::RegKey, String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let access = if write {
        KEY_READ | KEY_WRITE
    } else {
        KEY_READ
    };
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, access)
        .map_err(|error| format!("打开 Windows 自启注册表失败：{error}"))
}

#[cfg(windows)]
fn windows_is_enabled() -> bool {
    let Ok(expected) = windows_command() else {
        return false;
    };
    let Ok(key) = windows_run_key(false) else {
        return false;
    };
    key.get_value::<String, _>(RUN_VALUE)
        .map(|actual| actual.trim() == expected.trim())
        .unwrap_or(false)
}

#[cfg(windows)]
fn windows_set(enable: bool) -> Result<(), String> {
    let key = windows_run_key(true)?;
    if !enable {
        if let Err(error) = key.delete_value(RUN_VALUE) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("删除 Windows 自启项失败：{error}"));
            }
        }
        if windows_is_enabled() {
            return Err("关闭 Windows 自启后回读仍存在，请检查注册表权限".to_string());
        }
        return Ok(());
    }

    let expected = windows_command()?;
    key.set_value(RUN_VALUE, &expected)
        .map_err(|error| format!("写入 Windows 自启项失败：{error}"))?;
    let actual = key
        .get_value::<String, _>(RUN_VALUE)
        .map_err(|error| format!("回读 Windows 自启项失败：{error}"))?;
    if actual.trim() != expected.trim() {
        return Err(format!(
            "Windows 自启项回读不一致：期望 {expected}，实际 {actual}"
        ));
    }
    Ok(())
}

// ---- macOS ---------------------------------------------------------------

#[cfg(target_os = "macos")]
fn macos_plist_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library")
        .join("LaunchAgents")
        .join("com.shuying-code.wallpaper-rs.plist")
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn macos_set(enable: bool) -> Result<(), String> {
    let path = macos_plist_path();
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("删除 macOS 自启项失败：{error}"))?;
        }
        return Ok(());
    }

    let exe = xml_escape(&current_exe()?.to_string_lossy());
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.shuying-code.wallpaper-rs</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>{AUTOSTART_ARG}</string>
  </array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("创建 macOS LaunchAgents 目录失败：{error}"))?;
    }
    std::fs::write(&path, plist).map_err(|error| format!("写入 macOS 自启项失败：{error}"))?;
    let written = std::fs::read_to_string(&path)
        .map_err(|error| format!("回读 macOS 自启项失败：{error}"))?;
    if !written.contains(AUTOSTART_ARG) {
        return Err("macOS 自启项缺少 --autostart 标记".to_string());
    }
    Ok(())
}

// ---- Linux ---------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn linux_desktop_path() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    config.join("autostart").join("wallpaper-rs.desktop")
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn desktop_exec_quote(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn linux_set(enable: bool) -> Result<(), String> {
    let path = linux_desktop_path();
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("删除 Linux 自启项失败：{error}"))?;
        }
        return Ok(());
    }

    let exe = desktop_exec_quote(&current_exe()?);
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Wallpaper Studio\nExec=\"{exe}\" {AUTOSTART_ARG}\nX-GNOME-Autostart-enabled=true\n"
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("创建 Linux autostart 目录失败：{error}"))?;
    }
    std::fs::write(&path, entry).map_err(|error| format!("写入 Linux 自启项失败：{error}"))?;
    let written = std::fs::read_to_string(&path)
        .map_err(|error| format!("回读 Linux 自启项失败：{error}"))?;
    if !written.contains(AUTOSTART_ARG) {
        return Err("Linux 自启项缺少 --autostart 标记".to_string());
    }
    Ok(())
}

// ---- Android -------------------------------------------------------------

#[cfg(target_os = "android")]
fn android_set(enable: bool) -> Result<(), String> {
    crate::android_jni::set_autostart(enable)?;
    let actual = crate::android_jni::is_autostart_enabled()?;
    if actual != enable {
        return Err(format!(
            "Android 自启设置回读不一致：期望 {enable}，实际 {actual}"
        ));
    }
    Ok(())
}
