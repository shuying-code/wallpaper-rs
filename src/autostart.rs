//! Launch-at-login (autostart) integration for Windows / macOS / Linux.
//!
//! The persisted source of truth is `Config::autostart`; `set_enabled` applies it
//! to the platform. Keeping the OS integration best-effort and non-fatal keeps the
//! app usable even in restricted environments.

use std::path::PathBuf; // 引入依赖

/// Enable or disable launching the app at login.
pub fn set_enabled(enable: bool) -> Result<(), String> { // 函数 set_enabled
    #[cfg(windows)] // 语句
    { // 语句
        return windows_set(enable); // 返回
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        return macos_set(enable); // 返回
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos")))] // 语句
    { // 语句
        return linux_set(enable); // 返回
    } // 块结束
    #[allow(unreachable_code)] // 语句
    Err("当前平台不支持开机自启".to_string()) // 调用 Err
} // 块结束

fn current_exe() -> Result<PathBuf, String> { // 函数 current_exe
    std::env::current_exe().map_err(|e| format!("无法获取可执行文件路径：{e}")) // 调用 current_exe
} // 块结束

// ---- Windows -------------------------------------------------------------

#[cfg(windows)] // 语句
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize; // 常量 HKEY_CURRENT_USER
#[cfg(windows)] // 语句
const KEY_SET_VALUE: u32 = 0x0002; // 常量 KEY_SET_VALUE
#[cfg(windows)] // 语句
const REG_SZ: u32 = 1; // 常量 REG_SZ

#[cfg(windows)] // 语句
#[link(name = "advapi32")] // 语句
extern "system" { // 语句
    fn RegOpenKeyExW( // 函数 RegOpenKeyExW
        h_key: isize, // 字段 h_key
        lp_sub_key: *const u16, // 字段 lp_sub_key
        ul_options: u32, // 字段 ul_options
        sam_desired: u32, // 字段 sam_desired
        phk_result: *mut isize, // 字段 phk_result
    ) -> i32; // 语句
    fn RegSetValueExW( // 函数 RegSetValueExW
        h_key: isize, // 字段 h_key
        lp_value_name: *const u16, // 字段 lp_value_name
        reserved: u32, // 字段 reserved
        dw_type: u32, // 字段 dw_type
        lp_data: *const u8, // 字段 lp_data
        cb_data: u32, // 字段 cb_data
    ) -> i32; // 语句
    fn RegDeleteValueW(h_key: isize, lp_value_name: *const u16) -> i32; // 函数 RegDeleteValueW
    fn RegCloseKey(h_key: isize) -> i32; // 函数 RegCloseKey
} // 块结束

#[cfg(windows)] // 语句
fn windows_set(enable: bool) -> Result<(), String> { // 函数 windows_set
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run" // 变量 subkey
        .encode_utf16() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let value_name: Vec<u16> = "wallpaper-rs" // 变量 value_name
        .encode_utf16() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句

    let mut hkey: isize = 0; // 变量 hkey
    let rc = unsafe { // 变量 rc
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) // 调用 RegOpenKeyExW
    }; // 块结束
    if rc != 0 { // 条件分支
        return Err(format!("打开注册表失败 code={rc}")); // 返回
    } // 块结束

    let rc2 = unsafe { // 变量 rc2
        if enable { // 条件分支
            let exe = current_exe()?; // 变量 exe
            let cmd = format!("\"{}\"", exe.display()); // 变量 cmd
            let data: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect(); // 变量 data
            RegSetValueExW( // 调用 RegSetValueExW
                hkey, // 语句
                value_name.as_ptr(), // 语句
                0, // 语句
                REG_SZ, // 语句
                data.as_ptr() as *const u8, // 语句
                (data.len() * 2) as u32, // 语句
            ) // 块结束
        } else { // 语句
            RegDeleteValueW(hkey, value_name.as_ptr()) // 调用 RegDeleteValueW
        } // 块结束
    }; // 块结束
    unsafe { // 语句
        RegCloseKey(hkey); // 调用 RegCloseKey
    } // 块结束

    // Deleting a non-existent value returns ERROR_FILE_NOT_FOUND (2); treat as success.
    if rc2 == 0 || (!enable && rc2 == 2) { // 条件分支
        Ok(()) // 调用 Ok
    } else { // 语句
        Err(format!("写入注册表失败 code={rc2}")) // 调用 Err
    } // 块结束
} // 块结束

// ---- macOS ---------------------------------------------------------------

#[cfg(target_os = "macos")] // 语句
fn macos_plist_path() -> PathBuf { // 函数 macos_plist_path
    let home = std::env::var_os("HOME") // 变量 home
        .map(PathBuf::from) // 语句
        .unwrap_or_else(|| PathBuf::from(".")); // 语句
    home.join("Library") // 语句
        .join("LaunchAgents") // 语句
        .join("com.shuying-code.wallpaper-rs.plist") // 语句
} // 块结束

#[cfg(target_os = "macos")] // 语句
fn macos_set(enable: bool) -> Result<(), String> { // 函数 macos_set
    let path = macos_plist_path(); // 变量 path
    if !enable { // 条件分支
        if path.exists() { // 条件分支
            std::fs::remove_file(&path).map_err(|e| e.to_string())?; // 调用 remove_file
        } // 块结束
        return Ok(()); // 返回
    } // 块结束
    let exe = current_exe()?; // 变量 exe
    let plist = format!( // 变量 plist
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
"#, // 语句
        exe.display() // 语句
    ); // 块结束
    if let Some(dir) = path.parent() { // 条件分支
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; // 调用 create_dir_all
    } // 块结束
    std::fs::write(&path, plist).map_err(|e| e.to_string()) // 调用 write
} // 块结束

// ---- Linux ---------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))] // 语句
fn linux_desktop_path() -> PathBuf { // 函数 linux_desktop_path
    let cfg = std::env::var_os("XDG_CONFIG_HOME") // 变量 cfg
        .map(PathBuf::from) // 语句
        .unwrap_or_else(|| { // 语句
            std::env::var_os("HOME") // 调用 var_os
                .map(PathBuf::from) // 语句
                .unwrap_or_else(|| PathBuf::from(".")) // 语句
                .join(".config") // 语句
        }); // 语句
    cfg.join("autostart").join("wallpaper-rs.desktop") // 语句
} // 块结束

#[cfg(all(unix, not(target_os = "macos")))] // 语句
fn linux_set(enable: bool) -> Result<(), String> { // 函数 linux_set
    let path = linux_desktop_path(); // 变量 path
    if !enable { // 条件分支
        if path.exists() { // 条件分支
            std::fs::remove_file(&path).map_err(|e| e.to_string())?; // 调用 remove_file
        } // 块结束
        return Ok(()); // 返回
    } // 块结束
    let exe = current_exe()?; // 变量 exe
    let entry = format!( // 变量 entry
        "[Desktop Entry]\nType=Application\nName=Wallpaper Studio\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n", // 语句
        exe.display() // 语句
    ); // 块结束
    if let Some(dir) = path.parent() { // 条件分支
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; // 调用 create_dir_all
    } // 块结束
    std::fs::write(&path, entry).map_err(|e| e.to_string()) // 调用 write
} // 块结束
