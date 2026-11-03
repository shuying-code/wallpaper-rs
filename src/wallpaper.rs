//! Cross-platform "set the desktop wallpaper" implementations.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
}
impl Default for Fit {
    fn default() -> Self {
        Fit::Fill
    }
}
impl Fit {
    pub fn label(&self) -> &'static str {
        match self {
            Fit::Fill => "填充 (Fill)",
            Fit::Fit => "适应 (Fit)",
            Fit::Stretch => "拉伸 (Stretch)",
            Fit::Center => "居中 (Center)",
            Fit::Tile => "平铺 (Tile)",
        }
    }
    pub fn all() -> [Fit; 5] {
        [Fit::Fill, Fit::Fit, Fit::Stretch, Fit::Center, Fit::Tile]
    }
    /// Short Chinese-only label for narrow / mobile layouts.
    pub fn short(&self) -> &'static str {
        match self {
            Fit::Fill => "填充",
            Fit::Fit => "适应",
            Fit::Stretch => "拉伸",
            Fit::Center => "居中",
            Fit::Tile => "平铺",
        }
    }
}

/// Set the desktop wallpaper to `path`. `fit` controls the display style on
/// Windows (registry) and Linux (GNOME `picture-options`); macOS applies the image.
pub fn set_wallpaper(path: &Path, fit: Fit) -> Result<(), String> {
    #[cfg(windows)]
    {
        return windows_impl(path, fit);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = fit;
        return macos_impl(path);
    }
    #[cfg(target_os = "android")]
    {
        // On Android the wallpaper is applied from the mobile host via
        // WallpaperManager (JNI); a plain desktop binary cannot do it here.
        let _ = fit;
        return Err("Android 需在移动端宿主中通过 WallpaperManager 设置壁纸（需 SET_WALLPAPER 权限）".to_string());
    }
    #[cfg(target_os = "ios")]
    {
        let _ = fit;
        return Err("iOS 不允许 App 直接更改壁纸，请把图片导出到相册后手动设为墙纸".to_string());
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))]
    {
        return linux_impl(path, fit);
    }
    #[allow(unreachable_code)]
    Err("当前平台暂不支持设置壁纸".to_string())
}

/// Set the **lock screen** image (best-effort, platform-specific).
///
/// - Windows: writes `PersonalizationCSP` under HKEY_LOCAL_MACHINE (needs admin);
///   on failure the caller can retry via [`crate::elevate`] using the UAC prompt.
/// - Ubuntu / GNOME (Linux): `org.gnome.desktop.screensaver picture-uri`.
/// - macOS: the lock screen wallpaper follows the system; not separately settable.
pub fn set_lock_screen(path: &Path) -> Result<(), LockError> {
    #[cfg(windows)]
    {
        return windows_lock(path);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = path;
        return Err(LockError::plain("macOS 锁屏壁纸跟随系统，暂不支持单独设置"));
    }
    #[cfg(target_os = "android")]
    {
        let _ = path;
        return Err(LockError::plain("Android 暂不支持单独设置锁屏壁纸"));
    }
    #[cfg(target_os = "ios")]
    {
        let _ = path;
        return Err(LockError::plain("iOS 不允许 App 更改锁屏壁纸"));
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android"), not(target_os = "ios")))]
    {
        return linux_lock(path).map_err(LockError::plain);
    }
    #[allow(unreachable_code)]
    Err(LockError::plain("当前平台暂不支持设置锁屏壁纸"))
}

/// Failure from [`set_lock_screen`].
///
/// Besides the human-readable message it carries a flag telling the caller
/// whether the write was rejected purely for lack of administrator rights — the
/// UI then re-runs the action through the Windows UAC prompt instead of just
/// showing an error.
#[derive(Debug, Clone)]
pub struct LockError {
    pub message: String,
    /// `true` when the failure is an access-denied that elevation could fix.
    pub needs_admin: bool,
}

impl LockError {
    fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            needs_admin: false,
        }
    }
    #[cfg(windows)]
    fn denied(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            needs_admin: true,
        }
    }
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LockError {}

#[cfg(windows)]
fn windows_lock(path: &Path) -> Result<(), LockError> {
    use std::os::windows::ffi::OsStrExt;
    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PersonalizationCSP"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut hkey: isize = 0;
    let rc = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null_mut(),
            &mut hkey,
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return Err(lock_reg_error(rc));
    }
    let pwide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let set_sz = |name: &str, data: &[u16]| -> i32 {
        let nw: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            RegSetValueExW(
                hkey,
                nw.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * 2) as u32,
            )
        }
    };
    let mut last = 0;
    last |= set_sz("LockScreenImagePath", &pwide);
    last |= set_sz("LockScreenImageUrl", &pwide);
    // LockScreenImageStatus = 1 (DWORD)
    {
        let nw: Vec<u16> = "LockScreenImageStatus"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let val: u32 = 1;
        let rc2 = unsafe {
            RegSetValueExW(
                hkey,
                nw.as_ptr(),
                0,
                REG_DWORD,
                &val as *const u32 as *const u8,
                4,
            )
        };
        last |= rc2;
    }
    unsafe {
        RegCloseKey(hkey);
    }
    if last != 0 {
        Err(lock_reg_error(last))
    } else {
        Ok(())
    }
}

/// Map a Win32 registry error code from the lock-screen write into a
/// [`LockError`]. `ERROR_ACCESS_DENIED` (5) means we simply are not elevated.
#[cfg(windows)]
fn lock_reg_error(rc: i32) -> LockError {
    if rc == 5 {
        LockError::denied(format!("写入锁屏设置被拒绝，需要管理员权限 (code {rc})"))
    } else {
        LockError::plain(format!("写入锁屏设置失败 (code {rc})"))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_lock(path: &Path) -> Result<(), String> {
    let uri = format!("file://{}", path.display());
    let mut ok = false;
    for key in ["picture-uri", "picture-uri-dark"] {
        let s = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.screensaver", key, &uri])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        ok |= s;
    }
    if ok {
        Ok(())
    } else {
        Err("Ubuntu / GNOME 需要 gsettings 才能设置锁屏壁纸".to_string())
    }
}

#[cfg(windows)]
fn windows_impl(path: &Path, fit: Fit) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    // Wallpaper display style via HKCU\Control Panel\Desktop
    let (style, tile) = match fit {
        Fit::Center => ("0", "0"),
        Fit::Stretch => ("2", "0"),
        Fit::Fit => ("6", "0"),
        Fit::Fill => ("10", "0"),
        Fit::Tile => ("0", "1"),
    };
    set_reg_sz("WallpaperStyle", style)?;
    set_reg_sz("TileWallpaper", tile)?;
    // Explicitly persist the wallpaper path; some shells/COM surfaces only pick
    // up the change immediately when this value is present alongside SPI.
    let _ = set_reg_sz("WallPaper", &path.to_string_lossy());

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let call = || unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide.as_ptr() as *mut std::ffi::c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };
    let mut ok = call();
    // If the readback doesn't match yet, nudge once more (some configurations
    // need a second call before the desktop repaints immediately).
    if ok != 0 && !same_path(current_wallpaper().as_deref(), path) {
        std::thread::sleep(std::time::Duration::from_millis(60));
        ok = call();
    }
    if ok == 0 {
        Err(format!(
            "SystemParametersInfoW 失败：{}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn same_path(a: Option<&Path>, b: &Path) -> bool {
    match a {
        Some(p) => p.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()),
        None => false,
    }
}

/// Known third-party programs that take over the desktop wallpaper. If one is
/// running it can override a wallpaper we set (the registry/SPI changes but the
/// user sees no visual change). Returned as friendly display names.
pub fn conflicting_wallpaper_managers() -> Vec<String> {
    #[cfg(windows)]
    {
        const SIGS: &[(&str, &str)] = &[
            ("aura wallpaper", "华硕 Aura Wallpaper"),
            ("wallpaper engine", "Wallpaper Engine"),
            ("wallpaper32", "Wallpaper Engine"),
            ("wallpaper64", "Wallpaper Engine"),
            ("dynamic theme", "Dynamic Theme"),
            ("deskscapes", "DeskScapes"),
            ("lively", "Lively Wallpaper"),
            ("upupoo", "UPUPOO / 火萤"),
        ];
        let mut out: Vec<String> = Vec::new();
        for_each_running_process(|name, _pid| {
            for (key, disp) in SIGS {
                if name.contains(key) && !out.iter().any(|x| x == disp) {
                    out.push((*disp).to_string());
                }
            }
        });
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Enumerate every running process, invoking `f(lowercase exe name, pid)` for
/// each. Windows only; a no-op on other platforms.
#[cfg(windows)]
pub(crate) fn for_each_running_process(mut f: impl FnMut(&str, u32)) {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap as isize == -1 {
            return;
        }
        let mut e: ProcessEntry32W = std::mem::zeroed();
        e.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if Process32FirstW(snap, &mut e) != 0 {
            loop {
                let len = e
                    .sz_exe_file
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.sz_exe_file.len());
                let name = String::from_utf16_lossy(&e.sz_exe_file[..len]).to_ascii_lowercase();
                f(&name, e.th32_process_id);
                if Process32NextW(snap, &mut e) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
}

#[cfg(windows)]
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;

#[cfg(windows)]
#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(h_snapshot: *mut std::ffi::c_void, lppe: *mut ProcessEntry32W) -> i32;
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
const SPI_SETDESKWALLPAPER: u32 = 0x0014;
#[cfg(windows)]
const SPI_GETDESKWALLPAPER: u32 = 0x0073;
#[cfg(windows)]
const SPIF_UPDATEINIFILE: u32 = 0x0001;
#[cfg(windows)]
const SPIF_SENDCHANGE: u32 = 0x0002;
#[cfg(windows)]
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize;
#[cfg(windows)]
const HKEY_LOCAL_MACHINE: isize = 0x80000002u32 as i32 as isize;
#[cfg(windows)]
const KEY_SET_VALUE: u32 = 0x0002;
#[cfg(windows)]
const REG_SZ: u32 = 1;
#[cfg(windows)]
const REG_DWORD: u32 = 4;

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn SystemParametersInfoW(
        u_action: u32,
        u_param: u32,
        pv_param: *mut std::ffi::c_void,
        f_win_ini: u32,
    ) -> i32;
}

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
    fn RegCreateKeyExW(
        h_key: isize,
        lp_sub_key: *const u16,
        reserved: u32,
        lp_class: *const u16,
        dw_options: u32,
        sam_desired: u32,
        lp_security_attributes: *mut std::ffi::c_void,
        phk_result: *mut isize,
        lpdw_disposition: *mut u32,
    ) -> i32;
    fn RegCloseKey(h_key: isize) -> i32;
}

#[cfg(windows)]
fn set_reg_sz(name: &str, val: &str) -> Result<(), String> {
    let subkey: Vec<u16> = "Control Panel\\Desktop"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let data: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: isize = 0;
    let rc = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };
    if rc != 0 {
        return Err(format!("RegOpenKeyExW 失败 code={rc}"));
    }
    let rc2 = unsafe {
        RegSetValueExW(
            hkey,
            value_name.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            (data.len() * 2) as u32,
        )
    };
    unsafe {
        RegCloseKey(hkey);
    }
    if rc2 != 0 {
        Err(format!("RegSetValueExW 失败 code={rc2}"))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn macos_impl(path: &Path) -> Result<(), String> {
    let p = path.to_string_lossy().replace('"', "\\\"");
    let script = format!(
        "tell application \"System Events\" to set picture of every desktop to \"{}\"",
        p
    );
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_impl(path: &Path, fit: Fit) -> Result<(), String> {
    let uri = format!("file://{}", path.display());
    let set = |key: &str| -> bool {
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", key, &uri])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    // GNOME picture-options: none | wallpaper(tile) | centered | scaled(fit) |
    // stretched | zoom(fill) | spanned
    let option = match fit {
        Fit::Fill => "zoom",
        Fit::Fit => "scaled",
        Fit::Stretch => "stretched",
        Fit::Center => "centered",
        Fit::Tile => "wallpaper",
    };
    let set_option = || -> bool {
        std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.background",
                "picture-options",
                option,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if set("picture-uri") || set("picture-uri-dark") {
        let _ = set_option();
        return Ok(());
    }
    let feh = std::process::Command::new("feh")
        .arg("--bg-fill")
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if feh {
        Ok(())
    } else {
        Err("Linux 需要 GNOME(gsettings) 或 feh 才能设置壁纸".to_string())
    }
}

// ---- Display / monitor info ------------------------------------------------

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(n_index: i32) -> i32;
}

/// Number of monitors currently attached (best-effort).
pub fn monitor_count() -> usize {
    #[cfg(windows)]
    {
        // SM_CMONITORS = 80
        let n = unsafe { GetSystemMetrics(80) };
        if n > 0 {
            n as usize
        } else {
            1
        }
    }
    #[cfg(not(windows))]
    {
        1
    }
}

/// Primary display resolution in physical pixels (best-effort; 0,0 if unknown).
pub fn primary_resolution() -> (i32, i32) {
    #[cfg(windows)]
    {
        unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) }
    }
    #[cfg(not(windows))]
    {
        (0, 0)
    }
}

/// Path of the current desktop wallpaper (best-effort; Windows only).
pub fn current_wallpaper() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let mut buf = vec![0u16; 1024];
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buf.len() as u32,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                0,
            )
        };
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..len]);
        if s.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(s))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
