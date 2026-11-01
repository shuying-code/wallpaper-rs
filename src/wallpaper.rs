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
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return linux_impl(path, fit);
    }
    #[allow(unreachable_code)]
    Err("当前平台暂不支持设置壁纸".to_string())
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

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide.as_ptr() as *mut std::ffi::c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };
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
const SPI_SETDESKWALLPAPER: u32 = 0x0014;
#[cfg(windows)]
const SPIF_UPDATEINIFILE: u32 = 0x0001;
#[cfg(windows)]
const SPIF_SENDCHANGE: u32 = 0x0002;
#[cfg(windows)]
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize;
#[cfg(windows)]
const KEY_SET_VALUE: u32 = 0x0002;
#[cfg(windows)]
const REG_SZ: u32 = 1;

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
