//! On-demand privilege elevation (Windows UAC).
//!
//! A few actions need administrator rights — currently setting the Windows lock
//! screen image, which writes `HKLM\...\PersonalizationCSP`. Rather than telling
//! the user to "run as administrator", the app relaunches a short-lived copy of
//! itself through the UAC consent prompt (`ShellExecuteExW` with the `runas`
//! verb). That elevated copy performs the requested action and exits, and this
//! process waits for it and reports the outcome — so from the user's point of
//! view they just click the button, approve the prompt, and the action finishes.
//!
//! Protocol: the elevated copy is started as
//!
//! ```text
//! wallpaper-rs.exe --elevated-action <action> [args...]
//! ```
//!
//! `main` detects [`HELPER_FLAG`] before opening any window, runs
//! [`run_helper_action`], and exits with a status code (0 = success) that
//! [`run_elevated`] reads back from the child process handle.

/// Action id: set the system lock screen image. Payload: one filesystem path.
pub const ACTION_LOCK_SCREEN: &str = "lock-screen";

/// CLI switch marking this process as an elevated helper (not a normal launch).
const HELPER_FLAG: &str = "--elevated-action";

/// Outcome of asking the OS to run an action elevated.
#[derive(Debug, Clone)]
pub enum ElevationResult {
    /// The elevated copy completed the action successfully.
    Ok,
    /// The user dismissed or declined the UAC prompt.
    Cancelled,
    /// Elevation could not start, or the elevated action itself failed.
    Failed(String),
}

/// If `args` marks a process launched by [`run_elevated`], return
/// `(action, args)` so `main` can perform the privileged work and exit. `None`
/// for a normal user-facing launch.
pub fn pending_helper_call(args: &[String]) -> Option<(String, Vec<String>)> {
    let pos = args.iter().position(|a| a == HELPER_FLAG)?;
    let action = args.get(pos + 1)?.clone();
    let rest = args.get(pos + 2..).map(|s| s.to_vec()).unwrap_or_default();
    Some((action, rest))
}

/// Run one privileged action inside the elevated helper. Returns a process exit
/// code (0 = success) that [`run_elevated`] reads back.
pub fn run_helper_action(action: &str, args: &[String]) -> i32 {
    match action {
        ACTION_LOCK_SCREEN => match args.first() {
            Some(p) => match crate::wallpaper::set_lock_screen(std::path::Path::new(p)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("elevated action '{action}' failed: {e}");
                    1
                }
            },
            None => {
                eprintln!("elevated action '{action}': missing path argument");
                2
            }
        },
        other => {
            eprintln!("unknown elevated action: {other}");
            3
        }
    }
}

/// True when this process already has administrator rights. Always `true` on
/// non-Windows platforms (there is nothing to elevate against).
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::ffi::c_void;
    unsafe {
        let mut token: *mut c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: u32 = 0;
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut elevation as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}

/// Relaunch this executable elevated (UAC) to perform `action`, wait for it to
/// finish, and report the result. Blocks the calling thread while the elevated
/// copy runs.
#[cfg(windows)]
pub fn run_elevated(action: &str, args: &[String]) -> ElevationResult {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return ElevationResult::Failed(format!("无法获取程序路径：{e}")),
    };

    // Parameter string: `<helper flag> <action> [quoted args...]`.
    let mut params = format!("{} {}", quote(HELPER_FLAG), quote(action));
    for a in args {
        params.push(' ');
        params.push_str(&quote(a));
    }

    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
    let file: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let params_w: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();

    let mut sei: ShellExecuteInfoW = unsafe { std::mem::zeroed() };
    sei.cb_size = std::mem::size_of::<ShellExecuteInfoW>() as u32;
    sei.f_mask = SEE_MASK_NOCLOSEPROCESS;
    sei.lp_verb = verb.as_ptr();
    sei.lp_file = file.as_ptr();
    sei.lp_parameters = params_w.as_ptr();
    sei.n_show = SW_HIDE;

    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 {
        // Read the error immediately after the call (via the std shim so we
        // don't duplicate the `GetLastError` FFI declaration used elsewhere).
        let err = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(0);
        // 1223 = ERROR_CANCELLED → the user declined the UAC prompt.
        return if err == ERROR_CANCELLED {
            ElevationResult::Cancelled
        } else {
            ElevationResult::Failed(format!("请求管理员权限失败 (code {err})"))
        };
    }

    // Wait for the elevated copy and read its exit code.
    let code = unsafe {
        let h: *mut c_void = sei.h_process;
        if h.is_null() {
            return ElevationResult::Failed("提升权限后未能获取子进程句柄".to_string());
        }
        WaitForSingleObject(h, INFINITE);
        let mut code: u32 = 1;
        GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        code
    };
    if code == 0 {
        ElevationResult::Ok
    } else {
        ElevationResult::Failed(format!("提升权限后执行失败 (code {code})"))
    }
}

#[cfg(not(windows))]
pub fn run_elevated(_action: &str, _args: &[String]) -> ElevationResult {
    ElevationResult::Failed("当前平台不需要或不支持提权".to_string())
}

/// Quote one token for a Windows command line (used to build the helper's
/// parameters, so paths with spaces survive intact).
#[cfg(windows)]
fn quote(s: &str) -> String {
    if s.is_empty() || s.contains(' ') || s.contains('"') || s.contains('\t') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(windows)]
const TOKEN_QUERY: u32 = 0x0008;
#[cfg(windows)]
const TOKEN_ELEVATION: u32 = 20;
#[cfg(windows)]
const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
#[cfg(windows)]
const SW_HIDE: i32 = 0;
#[cfg(windows)]
const INFINITE: u32 = 0xFFFF_FFFF;
#[cfg(windows)]
const ERROR_CANCELLED: i32 = 1223;

#[cfg(windows)]
#[repr(C)]
struct ShellExecuteInfoW {
    cb_size: u32,
    f_mask: u32,
    hwnd: *mut std::ffi::c_void,
    lp_verb: *const u16,
    lp_file: *const u16,
    lp_parameters: *const u16,
    lp_directory: *const u16,
    n_show: i32,
    h_inst_app: *mut std::ffi::c_void,
    lp_id_list: *mut std::ffi::c_void,
    lp_class: *const u16,
    hkey_class: *mut std::ffi::c_void,
    dw_hot_key: u32,
    /// Union `{ hIcon; hMonitor }` in the Win32 header.
    h_icon_or_monitor: *mut std::ffi::c_void,
    h_process: *mut std::ffi::c_void,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn WaitForSingleObject(h_handle: *mut std::ffi::c_void, ms: u32) -> u32;
    fn GetExitCodeProcess(h_process: *mut std::ffi::c_void, code: *mut u32) -> i32;
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(
        process: *mut std::ffi::c_void,
        desired_access: u32,
        token: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn GetTokenInformation(
        token: *mut std::ffi::c_void,
        info_class: u32,
        info: *mut std::ffi::c_void,
        info_len: u32,
        ret_len: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteExW(info: *mut ShellExecuteInfoW) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_flag_roundtrips() {
        // A normal launch (no flag) must not look like a helper call.
        assert!(HELPER_FLAG.starts_with("--"));
        assert_eq!(ACTION_LOCK_SCREEN, "lock-screen");
        assert!(pending_helper_call(&["app.exe".into(), "--version".into()]).is_none());
        let call = pending_helper_call(&[
            "app.exe".into(),
            HELPER_FLAG.into(),
            ACTION_LOCK_SCREEN.into(),
            "C:\\pic one.png".into(),
        ]);
        assert_eq!(
            call,
            Some((
                ACTION_LOCK_SCREEN.to_string(),
                vec!["C:\\pic one.png".to_string()]
            ))
        );
    }
}
