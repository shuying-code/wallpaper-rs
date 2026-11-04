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
pub const ACTION_LOCK_SCREEN: &str = "lock-screen"; // 常量 ACTION_LOCK_SCREEN

/// CLI switch marking this process as an elevated helper (not a normal launch).
const HELPER_FLAG: &str = "--elevated-action"; // 常量 HELPER_FLAG

/// Outcome of asking the OS to run an action elevated.
#[derive(Debug, Clone)] // 语句
pub enum ElevationResult { // 枚举 ElevationResult
    /// The elevated copy completed the action successfully.
    Ok, // 语句
    /// The user dismissed or declined the UAC prompt.
    Cancelled, // 语句
    /// Elevation could not start, or the elevated action itself failed.
    Failed(String), // 调用 Failed
} // 块结束

/// If `args` marks a process launched by [`run_elevated`], return
/// `(action, args)` so `main` can perform the privileged work and exit. `None`
/// for a normal user-facing launch.
pub fn pending_helper_call(args: &[String]) -> Option<(String, Vec<String>)> { // 函数 pending_helper_call
    let pos = args.iter().position(|a| a == HELPER_FLAG)?; // 变量 pos
    let action = args.get(pos + 1)?.clone(); // 变量 action
    let rest = args.get(pos + 2..).map(|s| s.to_vec()).unwrap_or_default(); // 变量 rest
    Some((action, rest)) // 调用 Some
} // 块结束

/// Run one privileged action inside the elevated helper. Returns a process exit
/// code (0 = success) that [`run_elevated`] reads back.
pub fn run_helper_action(action: &str, args: &[String]) -> i32 { // 函数 run_helper_action
    match action { // 分支匹配
        ACTION_LOCK_SCREEN => match args.first() { // 匹配分支
            Some(p) => match crate::wallpaper::set_lock_screen(std::path::Path::new(p)) { // 匹配分支
                Ok(()) => 0, // 匹配分支
                Err(e) => { // 匹配分支
                    eprintln!("elevated action '{action}' failed: {e}"); // 语句
                    1 // 语句
                } // 块结束
            }, // 块结束
            None => { // 匹配分支
                eprintln!("elevated action '{action}': missing path argument"); // 语句
                2 // 语句
            } // 块结束
        }, // 块结束
        other => { // 匹配分支
            eprintln!("unknown elevated action: {other}"); // 语句
            3 // 语句
        } // 块结束
    } // 块结束
} // 块结束

/// True when this process already has administrator rights. Always `true` on
/// non-Windows platforms (there is nothing to elevate against).
#[cfg(windows)] // 语句
pub fn is_elevated() -> bool { // 函数 is_elevated
    use std::ffi::c_void; // 引入依赖
    unsafe { // 语句
        let mut token: *mut c_void = std::ptr::null_mut(); // 变量 token
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 { // 条件分支
            return false; // 返回
        } // 块结束
        let mut elevation: u32 = 0; // 变量 elevation
        let mut ret_len: u32 = 0; // 变量 ret_len
        let ok = GetTokenInformation( // 变量 ok
            token, // 语句
            TOKEN_ELEVATION, // 语句
            &mut elevation as *mut u32 as *mut c_void, // 语句
            std::mem::size_of::<u32>() as u32, // 字段 std
            &mut ret_len, // 语句
        ); // 块结束
        CloseHandle(token); // 调用 CloseHandle
        ok != 0 && elevation != 0 // 语句
    } // 块结束
} // 块结束

#[cfg(not(windows))] // 语句
pub fn is_elevated() -> bool { // 函数 is_elevated
    true // 语句
} // 块结束

/// Relaunch this executable elevated (UAC) to perform `action`, wait for it to
/// finish, and report the result. Blocks the calling thread while the elevated
/// copy runs.
#[cfg(windows)] // 语句
pub fn run_elevated(action: &str, args: &[String]) -> ElevationResult { // 函数 run_elevated
    use std::ffi::c_void; // 引入依赖
    use std::os::windows::ffi::OsStrExt; // 引入依赖

    let exe = match std::env::current_exe() { // 变量 exe
        Ok(p) => p, // 匹配分支
        Err(e) => return ElevationResult::Failed(format!("无法获取程序路径：{e}")), // 匹配分支
    }; // 块结束

    // Parameter string: `<helper flag> <action> [quoted args...]`.
    let mut params = format!("{} {}", quote(HELPER_FLAG), quote(action)); // 变量 params
    for a in args { // 循环
        params.push(' '); // 语句
        params.push_str(&quote(a)); // 语句
    } // 块结束

    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect(); // 变量 verb
    let file: Vec<u16> = exe // 变量 file
        .as_os_str() // 语句
        .encode_wide() // 语句
        .chain(std::iter::once(0)) // 语句
        .collect(); // 语句
    let params_w: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect(); // 变量 params_w

    let mut sei: ShellExecuteInfoW = unsafe { std::mem::zeroed() }; // 变量 sei
    sei.cb_size = std::mem::size_of::<ShellExecuteInfoW>() as u32; // 语句
    sei.f_mask = SEE_MASK_NOCLOSEPROCESS; // 语句
    sei.lp_verb = verb.as_ptr(); // 语句
    sei.lp_file = file.as_ptr(); // 语句
    sei.lp_parameters = params_w.as_ptr(); // 语句
    sei.n_show = SW_HIDE; // 语句

    let ok = unsafe { ShellExecuteExW(&mut sei) }; // 变量 ok
    if ok == 0 { // 条件分支
        // Read the error immediately after the call (via the std shim so we
        // don't duplicate the `GetLastError` FFI declaration used elsewhere).
        let err = std::io::Error::last_os_error() // 变量 err
            .raw_os_error() // 语句
            .unwrap_or(0); // 语句
        // 1223 = ERROR_CANCELLED → the user declined the UAC prompt.
        return if err == ERROR_CANCELLED { // 返回
            ElevationResult::Cancelled // 语句
        } else { // 语句
            ElevationResult::Failed(format!("请求管理员权限失败 (code {err})")) // 调用 Failed
        }; // 块结束
    } // 块结束

    // Wait for the elevated copy and read its exit code.
    let code = unsafe { // 变量 code
        let h: *mut c_void = sei.h_process; // 变量 h
        if h.is_null() { // 条件分支
            return ElevationResult::Failed("提升权限后未能获取子进程句柄".to_string()); // 返回
        } // 块结束
        WaitForSingleObject(h, INFINITE); // 调用 WaitForSingleObject
        let mut code: u32 = 1; // 变量 code
        GetExitCodeProcess(h, &mut code); // 调用 GetExitCodeProcess
        CloseHandle(h); // 调用 CloseHandle
        code // 语句
    }; // 块结束
    if code == 0 { // 条件分支
        ElevationResult::Ok // 语句
    } else { // 语句
        ElevationResult::Failed(format!("提升权限后执行失败 (code {code})")) // 调用 Failed
    } // 块结束
} // 块结束

#[cfg(not(windows))] // 语句
pub fn run_elevated(_action: &str, _args: &[String]) -> ElevationResult { // 函数 run_elevated
    ElevationResult::Failed("当前平台不需要或不支持提权".to_string()) // 调用 Failed
} // 块结束

/// Quote one token for a Windows command line (used to build the helper's
/// parameters, so paths with spaces survive intact).
#[cfg(windows)] // 语句
fn quote(s: &str) -> String { // 函数 quote
    if s.is_empty() || s.contains(' ') || s.contains('"') || s.contains('\t') { // 条件分支
        format!("\"{}\"", s.replace('"', "\\\"")) // 语句
    } else { // 语句
        s.to_string() // 语句
    } // 块结束
} // 块结束

#[cfg(windows)] // 语句
const TOKEN_QUERY: u32 = 0x0008; // 常量 TOKEN_QUERY
#[cfg(windows)] // 语句
const TOKEN_ELEVATION: u32 = 20; // 常量 TOKEN_ELEVATION
#[cfg(windows)] // 语句
const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040; // 常量 SEE_MASK_NOCLOSEPROCESS
#[cfg(windows)] // 语句
const SW_HIDE: i32 = 0; // 常量 SW_HIDE
#[cfg(windows)] // 语句
const INFINITE: u32 = 0xFFFF_FFFF; // 常量 INFINITE
#[cfg(windows)] // 语句
const ERROR_CANCELLED: i32 = 1223; // 常量 ERROR_CANCELLED

#[cfg(windows)] // 语句
#[repr(C)] // 语句
struct ShellExecuteInfoW { // 结构体 ShellExecuteInfoW
    cb_size: u32, // 字段 cb_size
    f_mask: u32, // 字段 f_mask
    hwnd: *mut std::ffi::c_void, // 字段 hwnd
    lp_verb: *const u16, // 字段 lp_verb
    lp_file: *const u16, // 字段 lp_file
    lp_parameters: *const u16, // 字段 lp_parameters
    lp_directory: *const u16, // 字段 lp_directory
    n_show: i32, // 字段 n_show
    h_inst_app: *mut std::ffi::c_void, // 字段 h_inst_app
    lp_id_list: *mut std::ffi::c_void, // 字段 lp_id_list
    lp_class: *const u16, // 字段 lp_class
    hkey_class: *mut std::ffi::c_void, // 字段 hkey_class
    dw_hot_key: u32, // 字段 dw_hot_key
    /// Union `{ hIcon; hMonitor }` in the Win32 header.
    h_icon_or_monitor: *mut std::ffi::c_void, // 字段 h_icon_or_monitor
    h_process: *mut std::ffi::c_void, // 字段 h_process
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "kernel32")] // 语句
extern "system" { // 语句
    fn GetCurrentProcess() -> *mut std::ffi::c_void; // 函数 GetCurrentProcess
    fn WaitForSingleObject(h_handle: *mut std::ffi::c_void, ms: u32) -> u32; // 函数 WaitForSingleObject
    fn GetExitCodeProcess(h_process: *mut std::ffi::c_void, code: *mut u32) -> i32; // 函数 GetExitCodeProcess
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32; // 函数 CloseHandle
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "advapi32")] // 语句
extern "system" { // 语句
    fn OpenProcessToken( // 函数 OpenProcessToken
        process: *mut std::ffi::c_void, // 字段 process
        desired_access: u32, // 字段 desired_access
        token: *mut *mut std::ffi::c_void, // 字段 token
    ) -> i32; // 语句
    fn GetTokenInformation( // 函数 GetTokenInformation
        token: *mut std::ffi::c_void, // 字段 token
        info_class: u32, // 字段 info_class
        info: *mut std::ffi::c_void, // 字段 info
        info_len: u32, // 字段 info_len
        ret_len: *mut u32, // 字段 ret_len
    ) -> i32; // 语句
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "shell32")] // 语句
extern "system" { // 语句
    fn ShellExecuteExW(info: *mut ShellExecuteInfoW) -> i32; // 函数 ShellExecuteExW
} // 块结束

#[cfg(test)] // 语句
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn helper_flag_roundtrips() { // 函数 helper_flag_roundtrips
        // A normal launch (no flag) must not look like a helper call.
        assert!(HELPER_FLAG.starts_with("--")); // 语句
        assert_eq!(ACTION_LOCK_SCREEN, "lock-screen"); // 语句
        assert!(pending_helper_call(&["app.exe".into(), "--version".into()]).is_none()); // 语句
        let call = pending_helper_call(&[ // 变量 call
            "app.exe".into(), // 语句
            HELPER_FLAG.into(), // 语句
            ACTION_LOCK_SCREEN.into(), // 语句
            "C:\\pic one.png".into(), // 语句
        ]); // 块结束
        assert_eq!( // 语句
            call, // 语句
            Some(( // 调用 Some
                ACTION_LOCK_SCREEN.to_string(), // 语句
                vec!["C:\\pic one.png".to_string()] // 语句
            )) // 语句
        ); // 块结束
    } // 块结束
} // 块结束
