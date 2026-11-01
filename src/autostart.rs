//! Launch-at-login (autostart) integration for Windows / macOS / Linux.
//!
//! The persisted source of truth is `Config::autostart`; `set_enabled` applies it
//! to the platform. Keeping the OS integration best-effort and non-fatal keeps the
//! app usable even in restricted environments.

use std::path::PathBuf;

/// Enable or disable launching the app at login.
pub fn set_enabled(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        return windows_set(enable);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_set(enable);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return linux_set(enable);
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持开机自启".to_string())
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("无法获取可执行文件路径：{e}"))
}

// ---- Windows -------------------------------------------------------------

#[cfg(windows)]
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize;
#[cfg(windows)]
const KEY_SET_VALUE: u32 = 0x0002;
#[cfg(windows)]
const REG_SZ: u32 = 1;

#[cfg(windows)]
#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        h_key: isize,
        lp_sub_key: *const u16,
        ul_options: u32,
        sam_desired: u32,
        phk_result: *mut isize,
    ) -> i32;
    fn RegSetValueExW(
        h_key: isize,
        lp_value_name: *const u16,
        reserved: u32,
        dw_type: u32,
        lp_data: *const u8,
        cb_data: u32,
    ) -> i32;
    fn RegDeleteValueW(h_key: isize, lp_value_name: *const u16) -> i32;
    fn RegCloseKey(h_key: isize) -> i32;
}

#[cfg(windows)]
fn windows_set(enable: bool) -> Result<(), String> {
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "wallpaper-rs"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut hkey: isize = 0;
    let rc = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey)
    };
    if rc != 0 {
        return Err(format!("打开注册表失败 code={rc}"));
    }

    let rc2 = unsafe {
        if enable {
            let exe = current_exe()?;
            let cmd = format!("\"{}\"", exe.display());
            let data: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            RegSetValueExW(
                hkey,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(hkey, value_name.as_ptr())
        }
    };
    unsafe {
        RegCloseKey(hkey);
    }

    // Deleting a non-existent value returns ERROR_FILE_NOT_FOUND (2); treat as success.
    if rc2 == 0 || (!enable && rc2 == 2) {
        Ok(())
    } else {
        Err(format!("写入注册表失败 code={rc2}"))
    }
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
fn macos_set(enable: bool) -> Result<(), String> {
    let path = macos_plist_path();
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let exe = current_exe()?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.shuying-code.wallpaper-rs</string>
  <key>ProgramArguments</key>
  <array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
        exe.display()
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, plist).map_err(|e| e.to_string())
}

// ---- Linux ---------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_desktop_path() -> PathBuf {
    let cfg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    cfg.join("autostart").join("wallpaper-rs.desktop")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_set(enable: bool) -> Result<(), String> {
    let path = linux_desktop_path();
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let exe = current_exe()?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Wallpaper Studio\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, entry).map_err(|e| e.to_string())
}
