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
const EXE_STEM: &str = "wallpaper-rs";

/// True when a (lower-cased) executable file name looks like one of our builds:
/// `wallpaper-rs` (cargo bin) and `wallpaper-rs-windows-x64` (release dist) both
/// match; the underscore variant is kept for safety.
#[cfg(windows)]
fn looks_like_our_build(exe_name_lower: &str) -> bool {
    let stem = exe_name_lower
        .strip_suffix(".exe")
        .unwrap_or(exe_name_lower);
    stem.starts_with(EXE_STEM) || stem.starts_with("wallpaper_rs")
}

/// Best-effort: terminate every other process whose executable is a build of
/// this app. Never touches the current process.
pub fn terminate_other_instances() {
    #[cfg(windows)]
    windows::terminate_others();
    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
    unix::terminate_others();
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    const PROCESS_TERMINATE: u32 = 0x0001;

    pub fn terminate_others() {
        let self_pid = std::process::id();
        crate::wallpaper::for_each_running_process(|name, pid| {
            if pid == self_pid || !super::looks_like_our_build(name) {
                return;
            }
            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                if !handle.is_null() {
                    TerminateProcess(handle, 0);
                    CloseHandle(handle);
                }
            }
        });
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(dw_desired_access: u32, inherit_handle: i32, pid: u32) -> *mut c_void;
        fn TerminateProcess(handle: *mut c_void, exit_code: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
}

#[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
mod unix {
    /// Best-effort sibling cleanup via `pkill`. `pkill` never signals itself,
    /// but it may signal the current process too; that is harmless because the
    /// caller exits right afterwards.
    pub fn terminate_others() {
        let _ = std::process::Command::new("pkill")
            .args(["-f", "wallpaper-rs"])
            .status();
    }
}
