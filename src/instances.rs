//! Terminate every *other* running copy of the app.
//!
//! The tray menu's "退出 wallpaper-rs" is meant to be a real, whole-app exit —
//! not just an exit for the one instance whose icon was clicked. In practice
//! users end up with several copies alive at once (double-clicking the exe,
//! autostart plus a manual start, an older build left in the tray, ...). Each
//! copy runs its own slideshow timer and holds its own tray icon, so quitting a
//! single one looked like "退出 does nothing": the wallpaper kept rotating and a
//! wallpaper-rs process stayed in the task manager.
//!
//! So before the current process exits we ask the OS to kill its siblings. The
//! current process is always skipped (the caller terminates it itself right
//! after), and every failure is ignored — this must never be able to block the
//! quit.

/// Executable base name shared by every build of this app (`wallpaper-rs.exe`,
/// `wallpaper-rs-windows-x64.exe`, ...). Matched case-insensitively on the stem.
const EXE_STEM: &str = "wallpaper-rs"; // 常量 EXE_STEM

/// True when a (lower-cased) executable file name looks like one of our builds:
/// `wallpaper-rs` (cargo bin) and `wallpaper-rs-windows-x64` (release dist) both
/// match; the underscore variant is kept for safety.
#[cfg(windows)] // 语句
fn looks_like_our_build(exe_name_lower: &str) -> bool { // 函数 looks_like_our_build
    let stem = exe_name_lower // 变量 stem
        .strip_suffix(".exe") // 语句
        .unwrap_or(exe_name_lower); // 语句
    stem.starts_with(EXE_STEM) || stem.starts_with("wallpaper_rs") // 语句
} // 块结束

/// Best-effort: terminate every other process whose executable is a build of
/// this app. Never touches the current process.
pub fn terminate_other_instances() { // 函数 terminate_other_instances
    #[cfg(windows)] // 语句
    windows::terminate_others(); // 调用 terminate_others
    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))] // 语句
    unix::terminate_others(); // 调用 terminate_others
} // 块结束

#[cfg(windows)] // 语句
mod windows { // 模块 windows
    use std::ffi::c_void; // 引入依赖

    const PROCESS_TERMINATE: u32 = 0x0001; // 常量 PROCESS_TERMINATE

    pub fn terminate_others() { // 函数 terminate_others
        let self_pid = std::process::id(); // 变量 self_pid
        crate::wallpaper::for_each_running_process(|name, pid| { // 调用 for_each_running_process
            if pid == self_pid || !super::looks_like_our_build(name) { // 条件分支
                return; // 返回
            } // 块结束
            unsafe { // 语句
                let handle = OpenProcess(PROCESS_TERMINATE, 0, pid); // 变量 handle
                if !handle.is_null() { // 条件分支
                    TerminateProcess(handle, 0); // 调用 TerminateProcess
                    CloseHandle(handle); // 调用 CloseHandle
                } // 块结束
            } // 块结束
        }); // 语句
    } // 块结束

    #[link(name = "kernel32")] // 语句
    extern "system" { // 语句
        fn OpenProcess(dw_desired_access: u32, inherit_handle: i32, pid: u32) -> *mut c_void; // 函数 OpenProcess
        fn TerminateProcess(handle: *mut c_void, exit_code: u32) -> i32; // 函数 TerminateProcess
        fn CloseHandle(handle: *mut c_void) -> i32; // 函数 CloseHandle
    } // 块结束
} // 块结束

#[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))] // 语句
mod unix { // 模块 unix
    /// Best-effort sibling cleanup via `pkill`. `pkill` never signals itself,
    /// but it may signal the current process too; that is harmless because the
    /// caller exits right afterwards.
    pub fn terminate_others() { // 函数 terminate_others
        let _ = std::process::Command::new("pkill") // 变量 _
            .args(["-f", "wallpaper-rs"]) // 语句
            .status(); // 语句
    } // 块结束
} // 块结束
