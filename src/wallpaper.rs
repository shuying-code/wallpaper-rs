//! Cross-platform "set the desktop wallpaper" implementations.

use serde::{Deserialize, Serialize}; // 引入依赖
use std::path::Path; // 引入依赖

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)] // 语句
pub enum Fit { // 枚举 Fit
    Fill, // 语句
    Fit, // 语句
    Stretch, // 语句
    Center, // 语句
    Tile, // 语句
} // 块结束
impl Default for Fit { // 实现
    fn default() -> Self { // 函数 default
        Fit::Fill // 语句
    } // 块结束
} // 块结束
impl Fit { // 实现
    pub fn label(&self) -> &'static str { // 函数 label
        match self { // 分支匹配
            Fit::Fill => "填充 (Fill)", // 匹配分支
            Fit::Fit => "适应 (Fit)", // 匹配分支
            Fit::Stretch => "拉伸 (Stretch)", // 匹配分支
            Fit::Center => "居中 (Center)", // 匹配分支
            Fit::Tile => "平铺 (Tile)", // 匹配分支
        } // 块结束
    } // 块结束
    pub fn all() -> [Fit; 5] { // 函数 all
        [Fit::Fill, Fit::Fit, Fit::Stretch, Fit::Center, Fit::Tile] // 语句
    } // 块结束
    /// Short Chinese-only label for narrow / mobile layouts.
    pub fn short(&self) -> &'static str { // 函数 short
        match self { // 分支匹配
            Fit::Fill => "填充", // 匹配分支
            Fit::Fit => "适应", // 匹配分支
            Fit::Stretch => "拉伸", // 匹配分支
            Fit::Center => "居中", // 匹配分支
            Fit::Tile => "平铺", // 匹配分支
        } // 块结束
    } // 块结束
} // 块结束

/// Set the desktop wallpaper to `path`. `fit` controls the display style on
/// Windows (registry) and Linux (GNOME `picture-options`); macOS applies the image.
pub fn set_wallpaper(path: &Path, fit: Fit) -> Result<(), String> { // 函数 set_wallpaper
    #[cfg(windows)] // 语句
    { // 语句
        return windows_impl(path, fit); // 返回
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        let _ = fit; // 变量 _
        return macos_impl(path); // 返回
    } // 块结束
    #[cfg(target_os = "android")] // 语句
    { // 语句
        // On Android the wallpaper is applied through WallpaperManager over JNI
        // (`SET_WALLPAPER` permission); `fit` is handled by the system crop.
        let _ = fit; // 变量 _
        return crate::android_jni::set_wallpaper(path, false); // 返回
    } // 块结束
    #[cfg(target_os = "ios")] // 语句
    { // 语句
        let _ = fit; // 变量 _
        return Err("iOS 不允许 App 直接更改壁纸，请把图片导出到相册后手动设为墙纸".to_string()); // 返回
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))] // 语句
    { // 语句
        return linux_impl(path, fit); // 返回
    } // 块结束
    #[allow(unreachable_code)] // 语句
    Err("当前平台暂不支持设置壁纸".to_string()) // 调用 Err
} // 块结束

/// Set the **lock screen** image (best-effort, platform-specific).
///
/// - Windows: writes `PersonalizationCSP` under HKEY_LOCAL_MACHINE (needs admin);
///   on failure the caller can retry via [`crate::elevate`] using the UAC prompt.
/// - Ubuntu / GNOME (Linux): `org.gnome.desktop.screensaver picture-uri`.
/// - macOS: the lock screen wallpaper follows the system; not separately settable.
pub fn set_lock_screen(path: &Path) -> Result<(), LockError> { // 函数 set_lock_screen
    #[cfg(windows)] // 语句
    { // 语句
        return windows_lock(path); // 返回
    } // 块结束
    #[cfg(target_os = "macos")] // 语句
    { // 语句
        let _ = path; // 变量 _
        return Err(LockError::plain("macOS 锁屏壁纸跟随系统，暂不支持单独设置")); // 返回
    } // 块结束
    #[cfg(target_os = "android")] // 语句
    { // 语句
        // `WallpaperManager.FLAG_LOCK`. Some OEM ROMs restrict this; failures
        // are surfaced to the user instead of silently ignored.
        return crate::android_jni::set_wallpaper(path, true).map_err(LockError::plain); // 返回
    } // 块结束
    #[cfg(target_os = "ios")] // 语句
    { // 语句
        let _ = path; // 变量 _
        return Err(LockError::plain("iOS 不允许 App 更改锁屏壁纸")); // 返回
    } // 块结束
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))] // 语句
    { // 语句
        return linux_lock(path).map_err(LockError::plain); // 返回
    } // 块结束
    #[allow(unreachable_code)] // 语句
    Err(LockError::plain("当前平台暂不支持设置锁屏壁纸")) // 调用 Err
} // 块结束

/// Failure from [`set_lock_screen`].
///
/// Besides the human-readable message it carries a flag telling the caller
/// whether the write was rejected purely for lack of administrator rights — the
/// UI then re-runs the action through the Windows UAC prompt instead of just
/// showing an error.
#[derive(Debug, Clone)] // 语句
pub struct LockError { // 结构体 LockError
    pub message: String, // 字段 message
    /// `true` when the failure is an access-denied that elevation could fix.
    pub needs_admin: bool, // 字段 needs_admin
} // 块结束

impl LockError { // 实现
    fn plain(message: impl Into<String>) -> Self { // 函数 plain
        Self { // 语句
            message: message.into(), // 字段 message
            needs_admin: false, // 字段 needs_admin
        } // 块结束
    } // 块结束
    #[cfg(windows)] // 语句
    fn denied(message: impl Into<String>) -> Self { // 函数 denied
        Self { // 语句
            message: message.into(), // 字段 message
            needs_admin: true, // 字段 needs_admin
        } // 块结束
    } // 块结束
} // 块结束

impl std::fmt::Display for LockError { // 实现
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { // 函数 fmt
        f.write_str(&self.message) // 语句
    } // 块结束
} // 块结束

impl std::error::Error for LockError {} // 实现

#[cfg(windows)] // 语句
fn windows_lock(path: &Path) -> Result<(), LockError> { // 函数 windows_lock
    use std::os::windows::ffi::OsStrExt; // 引入依赖
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PersonalizationCSP" // 变量 subkey
        .encode_utf16() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let mut hkey: isize = 0; // 变量 hkey
    let rc = unsafe { // 变量 rc
        RegCreateKeyExW( // 调用 RegCreateKeyExW
            HKEY_LOCAL_MACHINE, // 语句
            subkey.as_ptr(), // 语句
            0, // 语句
            std::ptr::null(), // 字段 std
            0, // 语句
            KEY_SET_VALUE, // 语句
            std::ptr::null_mut(), // 字段 std
            &mut hkey, // 语句
            std::ptr::null_mut(), // 字段 std
        ) // 块结束
    }; // 块结束
    if rc != 0 { // 条件分支
        return Err(lock_reg_error(rc)); // 返回
    } // 块结束
    let pwide: Vec<u16> = path // 变量 pwide
        .as_os_str() // 语句
        .encode_wide() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let set_sz = |name: &str, data: &[u16]| -> i32 { // 变量 set_sz
        let nw: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect(); // 变量 nw
        unsafe { // 语句
            RegSetValueExW( // 调用 RegSetValueExW
                hkey, // 语句
                nw.as_ptr(), // 语句
                0, // 语句
                REG_SZ, // 语句
                data.as_ptr() as *const u8, // 语句
                (data.len() * 2) as u32, // 语句
            ) // 块结束
        } // 块结束
    }; // 块结束
    let mut last = 0; // 变量 last
    last |= set_sz("LockScreenImagePath", &pwide); // 语句
    last |= set_sz("LockScreenImageUrl", &pwide); // 语句
    // LockScreenImageStatus = 1 (DWORD)
    { // 语句
        let nw: Vec<u16> = "LockScreenImageStatus" // 变量 nw
            .encode_utf16() // 语句
            .chain(std::iter::once(0)) // 语句
            .collect(); // 语句
        let val: u32 = 1; // 变量 val
        let rc2 = unsafe { // 变量 rc2
            RegSetValueExW( // 调用 RegSetValueExW
                hkey, // 语句
                nw.as_ptr(), // 语句
                0, // 语句
                REG_DWORD, // 语句
                &val as *const u32 as *const u8, // 语句
                4, // 语句
            ) // 块结束
        }; // 块结束
        last |= rc2; // 语句
    } // 块结束
    unsafe { // 语句
        RegCloseKey(hkey); // 调用 RegCloseKey
    } // 块结束
    if last != 0 { // 条件分支
        Err(lock_reg_error(last)) // 调用 Err
    } else { // 语句
        Ok(()) // 调用 Ok
    } // 块结束
} // 块结束

/// Map a Win32 registry error code from the lock-screen write into a
/// [`LockError`]. `ERROR_ACCESS_DENIED` (5) means we simply are not elevated.
#[cfg(windows)] // 语句
fn lock_reg_error(rc: i32) -> LockError { // 函数 lock_reg_error
    if rc == 5 { // 条件分支
        LockError::denied(format!("写入锁屏设置被拒绝，需要管理员权限 (code {rc})")) // 调用 denied
    } else { // 语句
        LockError::plain(format!("写入锁屏设置失败 (code {rc})")) // 调用 plain
    } // 块结束
} // 块结束

#[cfg(all(unix, not(target_os = "macos")))] // 语句
fn linux_lock(path: &Path) -> Result<(), String> { // 函数 linux_lock
    let uri = format!("file://{}", path.display()); // 变量 uri
    let mut ok = false; // 变量 ok
    for key in ["picture-uri", "picture-uri-dark"] { // 循环
        let s = std::process::Command::new("gsettings") // 变量 s
            .args(["set", "org.gnome.desktop.screensaver", key, &uri]) // 语句
            .status() // 语句
            .map(|s| s.success()) // 语句
            .unwrap_or(false); // 语句
        ok |= s; // 语句
    } // 块结束
    if ok { // 条件分支
        Ok(()) // 调用 Ok
    } else { // 语句
        Err("Ubuntu / GNOME 需要 gsettings 才能设置锁屏壁纸".to_string()) // 调用 Err
    } // 块结束
} // 块结束

#[cfg(windows)] // 语句
fn windows_impl(path: &Path, fit: Fit) -> Result<(), String> { // 函数 windows_impl
    use std::os::windows::ffi::OsStrExt; // 引入依赖

    // Wallpaper display style via HKCU\Control Panel\Desktop
    let (style, tile) = match fit { // 调用 let
        Fit::Center => ("0", "0"), // 匹配分支
        Fit::Stretch => ("2", "0"), // 匹配分支
        Fit::Fit => ("6", "0"), // 匹配分支
        Fit::Fill => ("10", "0"), // 匹配分支
        Fit::Tile => ("0", "1"), // 匹配分支
    }; // 块结束
    set_reg_sz("WallpaperStyle", style)?; // 调用 set_reg_sz
    set_reg_sz("TileWallpaper", tile)?; // 调用 set_reg_sz
    // Explicitly persist the wallpaper path; some shells/COM surfaces only pick
    // up the change immediately when this value is present alongside SPI.
    let _ = set_reg_sz("WallPaper", &path.to_string_lossy()); // 变量 _

    let wide: Vec<u16> = path // 变量 wide
        .as_os_str() // 语句
        .encode_wide() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let call = || unsafe { // 变量 call
        SystemParametersInfoW( // 调用 SystemParametersInfoW
            SPI_SETDESKWALLPAPER, // 语句
            0, // 语句
            wide.as_ptr() as *mut std::ffi::c_void, // 语句
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE, // 语句
        ) // 块结束
    }; // 块结束
    let mut ok = call(); // 变量 ok
    // If the readback doesn't match yet, nudge once more (some configurations
    // need a second call before the desktop repaints immediately).
    if ok != 0 && !same_path(current_wallpaper().as_deref(), path) { // 条件分支
        std::thread::sleep(std::time::Duration::from_millis(60)); // 调用 sleep
        ok = call(); // 语句
    } // 块结束
    if ok == 0 { // 条件分支
        Err(format!( // 调用 Err
            "SystemParametersInfoW 失败：{}", // 语句
            std::io::Error::last_os_error() // 调用 last_os_error
        )) // 语句
    } else { // 语句
        Ok(()) // 调用 Ok
    } // 块结束
} // 块结束

#[cfg(windows)] // 语句
fn same_path(a: Option<&Path>, b: &Path) -> bool { // 函数 same_path
    match a { // 分支匹配
        Some(p) => p.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()), // 匹配分支
        None => false, // 匹配分支
    } // 块结束
} // 块结束

/// Known third-party programs that take over the desktop wallpaper. If one is
/// running it can override a wallpaper we set (the registry/SPI changes but the
/// user sees no visual change). Returned as friendly display names.
pub fn conflicting_wallpaper_managers() -> Vec<String> { // 函数 conflicting_wallpaper_managers
    #[cfg(windows)] // 语句
    { // 语句
        const SIGS: &[(&str, &str)] = &[ // 常量 SIGS
            ("aura wallpaper", "华硕 Aura Wallpaper"), // 语句
            ("wallpaper engine", "Wallpaper Engine"), // 语句
            ("wallpaper32", "Wallpaper Engine"), // 语句
            ("wallpaper64", "Wallpaper Engine"), // 语句
            ("dynamic theme", "Dynamic Theme"), // 语句
            ("deskscapes", "DeskScapes"), // 语句
            ("lively", "Lively Wallpaper"), // 语句
            ("upupoo", "UPUPOO / 火萤"), // 语句
        ]; // 语句
        let mut out: Vec<String> = Vec::new(); // 变量 out
        for_each_running_process(|name, _pid| { // 调用 for_each_running_process
            for (key, disp) in SIGS { // 循环
                if name.contains(key) && !out.iter().any(|x| x == disp) { // 条件分支
                    out.push((*disp).to_string()); // 语句
                } // 块结束
            } // 块结束
        }); // 语句
        out // 语句
    } // 块结束
    #[cfg(not(windows))] // 语句
    { // 语句
        Vec::new() // 调用 new
    } // 块结束
} // 块结束

/// Enumerate every running process, invoking `f(lowercase exe name, pid)` for
/// each. Windows only; a no-op on other platforms.
#[cfg(windows)] // 语句
pub(crate) fn for_each_running_process(mut f: impl FnMut(&str, u32)) { // 调用 pub
    unsafe { // 语句
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0); // 变量 snap
        if snap as isize == -1 { // 条件分支
            return; // 返回
        } // 块结束
        let mut e: ProcessEntry32W = std::mem::zeroed(); // 变量 e
        e.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32; // 语句
        if Process32FirstW(snap, &mut e) != 0 { // 条件分支
            loop { // 循环
                let len = e // 变量 len
                    .sz_exe_file // 语句
                    .iter() // 语句
                    .position(|&c| c == 0) // 语句
                    .unwrap_or(e.sz_exe_file.len()); // 语句
                let name = String::from_utf16_lossy(&e.sz_exe_file[..len]).to_ascii_lowercase(); // 变量 name
                f(&name, e.th32_process_id); // 调用 f
                if Process32NextW(snap, &mut e) == 0 { // 条件分支
                    break; // 语句
                } // 块结束
            } // 块结束
        } // 块结束
        CloseHandle(snap); // 调用 CloseHandle
    } // 块结束
} // 块结束

#[cfg(windows)] // 语句
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002; // 常量 TH32CS_SNAPPROCESS

#[cfg(windows)] // 语句
#[repr(C)] // 语句
struct ProcessEntry32W { // 结构体 ProcessEntry32W
    dw_size: u32, // 字段 dw_size
    cnt_usage: u32, // 字段 cnt_usage
    th32_process_id: u32, // 字段 th32_process_id
    th32_default_heap_id: usize, // 字段 th32_default_heap_id
    th32_module_id: u32, // 字段 th32_module_id
    cnt_threads: u32, // 字段 cnt_threads
    th32_parent_process_id: u32, // 字段 th32_parent_process_id
    pc_pri_class_base: i32, // 字段 pc_pri_class_base
    dw_flags: u32, // 字段 dw_flags
    sz_exe_file: [u16; 260], // 字段 sz_exe_file
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "kernel32")] // 语句
extern "system" { // 语句
    fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> *mut std::ffi::c_void; // 函数 CreateToolhelp32Snapshot
    fn Process32FirstW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32; // 函数 Process32FirstW
    fn Process32NextW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32; // 函数 Process32NextW
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32; // 函数 CloseHandle
} // 块结束

#[cfg(windows)] // 语句
const SPI_SETDESKWALLPAPER: u32 = 0x0014; // 常量 SPI_SETDESKWALLPAPER
#[cfg(windows)] // 语句
const SPI_GETDESKWALLPAPER: u32 = 0x0073; // 常量 SPI_GETDESKWALLPAPER
#[cfg(windows)] // 语句
const SPIF_UPDATEINIFILE: u32 = 0x0001; // 常量 SPIF_UPDATEINIFILE
#[cfg(windows)] // 语句
const SPIF_SENDCHANGE: u32 = 0x0002; // 常量 SPIF_SENDCHANGE
#[cfg(windows)] // 语句
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize; // 常量 HKEY_CURRENT_USER
#[cfg(windows)] // 语句
const HKEY_LOCAL_MACHINE: isize = 0x80000002u32 as i32 as isize; // 常量 HKEY_LOCAL_MACHINE
#[cfg(windows)] // 语句
const KEY_SET_VALUE: u32 = 0x0002; // 常量 KEY_SET_VALUE
#[cfg(windows)] // 语句
const REG_SZ: u32 = 1; // 常量 REG_SZ
#[cfg(windows)] // 语句
const REG_DWORD: u32 = 4; // 常量 REG_DWORD

#[cfg(windows)] // 语句
#[link(name = "user32")] // 语句
extern "system" { // 语句
    fn SystemParametersInfoW( // 函数 SystemParametersInfoW
        u_action: u32, // 字段 u_action
        u_param: u32, // 字段 u_param
        pv_param: *mut std::ffi::c_void, // 字段 pv_param
        f_win_ini: u32, // 字段 f_win_ini
    ) -> i32; // 语句
} // 块结束

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
    fn RegCreateKeyExW( // 函数 RegCreateKeyExW
        h_key: isize, // 字段 h_key
        lp_sub_key: *const u16, // 字段 lp_sub_key
        reserved: u32, // 字段 reserved
        lp_class: *const u16, // 字段 lp_class
        dw_options: u32, // 字段 dw_options
        sam_desired: u32, // 字段 sam_desired
        lp_security_attributes: *mut std::ffi::c_void, // 字段 lp_security_attributes
        phk_result: *mut isize, // 字段 phk_result
        lpdw_disposition: *mut u32, // 字段 lpdw_disposition
    ) -> i32; // 语句
    fn RegCloseKey(h_key: isize) -> i32; // 函数 RegCloseKey
} // 块结束

#[cfg(windows)] // 语句
fn set_reg_sz(name: &str, val: &str) -> Result<(), String> { // 函数 set_reg_sz
    let subkey: Vec<u16> = "Control Panel\\Desktop" // 变量 subkey
        .encode_utf16() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let value_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect(); // 变量 value_name
    let data: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect(); // 变量 data
    let mut hkey: isize = 0; // 变量 hkey
    let rc = unsafe { // 变量 rc
        RegOpenKeyExW( // 调用 RegOpenKeyExW
            HKEY_CURRENT_USER, // 语句
            subkey.as_ptr(), // 语句
            0, // 语句
            KEY_SET_VALUE, // 语句
            &mut hkey, // 语句
        ) // 块结束
    }; // 块结束
    if rc != 0 { // 条件分支
        return Err(format!("RegOpenKeyExW 失败 code={rc}")); // 返回
    } // 块结束
    let rc2 = unsafe { // 变量 rc2
        RegSetValueExW( // 调用 RegSetValueExW
            hkey, // 语句
            value_name.as_ptr(), // 语句
            0, // 语句
            REG_SZ, // 语句
            data.as_ptr() as *const u8, // 语句
            (data.len() * 2) as u32, // 语句
        ) // 块结束
    }; // 块结束
    unsafe { // 语句
        RegCloseKey(hkey); // 调用 RegCloseKey
    } // 块结束
    if rc2 != 0 { // 条件分支
        Err(format!("RegSetValueExW 失败 code={rc2}")) // 调用 Err
    } else { // 语句
        Ok(()) // 调用 Ok
    } // 块结束
} // 块结束

#[cfg(target_os = "macos")] // 语句
fn macos_impl(path: &Path) -> Result<(), String> { // 函数 macos_impl
    let p = path.to_string_lossy().replace('"', "\\\""); // 变量 p
    let script = format!( // 变量 script
        "tell application \"System Events\" to set picture of every desktop to \"{}\"", // 语句
        p // 语句
    ); // 块结束
    let out = std::process::Command::new("osascript") // 变量 out
        .arg("-e") // 语句
        .arg(&script) // 语句
        .output() // 语句
        .map_err(|e| e.to_string())?; // 语句
    if out.status.success() { // 条件分支
        Ok(()) // 调用 Ok
    } else { // 语句
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) // 调用 Err
    } // 块结束
} // 块结束

#[cfg(all(unix, not(target_os = "macos")))] // 语句
fn linux_impl(path: &Path, fit: Fit) -> Result<(), String> { // 函数 linux_impl
    let uri = format!("file://{}", path.display()); // 变量 uri
    let set = |key: &str| -> bool { // 变量 set
        std::process::Command::new("gsettings") // 调用 new
            .args(["set", "org.gnome.desktop.background", key, &uri]) // 语句
            .status() // 语句
            .map(|s| s.success()) // 语句
            .unwrap_or(false) // 语句
    }; // 块结束
    // GNOME picture-options: none | wallpaper(tile) | centered | scaled(fit) |
    // stretched | zoom(fill) | spanned
    let option = match fit { // 变量 option
        Fit::Fill => "zoom", // 匹配分支
        Fit::Fit => "scaled", // 匹配分支
        Fit::Stretch => "stretched", // 匹配分支
        Fit::Center => "centered", // 匹配分支
        Fit::Tile => "wallpaper", // 匹配分支
    }; // 块结束
    let set_option = || -> bool { // 变量 set_option
        std::process::Command::new("gsettings") // 调用 new
            .args([ // 语句
                "set", // 语句
                "org.gnome.desktop.background", // 语句
                "picture-options", // 语句
                option, // 语句
            ]) // 语句
            .status() // 语句
            .map(|s| s.success()) // 语句
            .unwrap_or(false) // 语句
    }; // 块结束
    if set("picture-uri") || set("picture-uri-dark") { // 条件分支
        let _ = set_option(); // 变量 _
        return Ok(()); // 返回
    } // 块结束
    let feh = std::process::Command::new("feh") // 变量 feh
        .arg("--bg-fill") // 语句
        .arg(path) // 语句
        .status() // 语句
        .map(|s| s.success()) // 语句
        .unwrap_or(false); // 语句
    if feh { // 条件分支
        Ok(()) // 调用 Ok
    } else { // 语句
        Err("Linux 需要 GNOME(gsettings) 或 feh 才能设置壁纸".to_string()) // 调用 Err
    } // 块结束
} // 块结束

// ---- Display / monitor info ------------------------------------------------

#[cfg(windows)] // 语句
#[link(name = "user32")] // 语句
extern "system" { // 语句
    fn GetSystemMetrics(n_index: i32) -> i32; // 函数 GetSystemMetrics
} // 块结束

/// Number of monitors currently attached (best-effort).
pub fn monitor_count() -> usize { // 函数 monitor_count
    #[cfg(windows)] // 语句
    { // 语句
        // SM_CMONITORS = 80
        let n = unsafe { GetSystemMetrics(80) }; // 变量 n
        if n > 0 { // 条件分支
            n as usize // 语句
        } else { // 语句
            1 // 语句
        } // 块结束
    } // 块结束
    #[cfg(not(windows))] // 语句
    { // 语句
        1 // 语句
    } // 块结束
} // 块结束

/// Primary display resolution in physical pixels (best-effort; 0,0 if unknown).
pub fn primary_resolution() -> (i32, i32) { // 函数 primary_resolution
    #[cfg(windows)] // 语句
    { // 语句
        unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) } // 语句
    } // 块结束
    #[cfg(not(windows))] // 语句
    { // 语句
        (0, 0) // 语句
    } // 块结束
} // 块结束

/// Path of the current desktop wallpaper (best-effort; Windows only).
pub fn current_wallpaper() -> Option<std::path::PathBuf> { // 函数 current_wallpaper
    #[cfg(windows)] // 语句
    { // 语句
        let mut buf = vec![0u16; 1024]; // 变量 buf
        let ok = unsafe { // 变量 ok
            SystemParametersInfoW( // 调用 SystemParametersInfoW
                SPI_GETDESKWALLPAPER, // 语句
                buf.len() as u32, // 语句
                buf.as_mut_ptr() as *mut std::ffi::c_void, // 语句
                0, // 语句
            ) // 块结束
        }; // 块结束
        if ok == 0 { // 条件分支
            return None; // 返回
        } // 块结束
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len()); // 变量 len
        let s = String::from_utf16_lossy(&buf[..len]); // 变量 s
        if s.is_empty() { // 条件分支
            None // 语句
        } else { // 语句
            Some(std::path::PathBuf::from(s)) // 调用 Some
        } // 块结束
    } // 块结束
    #[cfg(not(windows))] // 语句
    { // 语句
        None // 语句
    } // 块结束
} // 块结束
