//! Startup-time coordination for every running copy of the app.
//!
//! Only one copy should keep running. On startup the new process compares its
//! version with every sibling process:
//!
//! * newer than all siblings: terminate them and continue starting;
//! * older than, or equal to, the best sibling: ask that sibling to show its
//!   main window and exit immediately.
//!
//! A process-wide lock closes the gap between the scan and the first window.

const EXE_STEM: &str = "wallpaper-rs";
const STARTUP_ATTEMPTS: usize = 10;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);
const SHOW_REQUEST_PREFIX: &str = "show-main-";

#[cfg(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios"))))]
pub fn claim_single_instance(current_version: &str, show_existing: bool) -> bool {
    desktop::claim_single_instance(current_version, show_existing)
}

#[cfg(not(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios")))))]
pub fn claim_single_instance(current_version: &str, show_existing: bool) -> bool {
    let _ = (current_version, show_existing);
    true
}

#[cfg(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios"))))]
pub fn take_show_request() -> bool {
    desktop::take_show_request()
}

#[cfg(not(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios")))))]
pub fn take_show_request() -> bool {
    false
}

#[cfg(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios"))))]
pub fn terminate_other_instances() {
    desktop::terminate_other_instances();
}

#[cfg(not(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios")))))]
pub fn terminate_other_instances() {}

#[cfg(any(windows, all(unix, not(target_os = "android"), not(target_os = "ios"))))]
mod desktop {
    use semver::Version;
    use std::io::Read;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{EXE_STEM, RETRY_DELAY, SHOW_REQUEST_PREFIX, STARTUP_ATTEMPTS};

    const VERSION_QUERY_TIMEOUT: Duration = Duration::from_secs(2);

    #[derive(Debug, Clone)]
    struct RunningCopy {
        pid: u32,
        version: Option<Version>,
    }

    struct ProcessEntry {
        pid: u32,
        name: String,
        path: Option<PathBuf>,
    }

    fn looks_like_our_build(exe_name: &str) -> bool {
        let base = exe_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(exe_name)
            .to_ascii_lowercase();
        let stem = base.strip_suffix(".exe").unwrap_or(&base);
        stem.starts_with(EXE_STEM) || stem.starts_with("wallpaper_rs")
    }

    pub fn claim_single_instance(current_version: &str, show_existing: bool) -> bool {
        let Some(current) = Version::parse(current_version.trim()).ok() else {
            return true;
        };
        let self_pid = std::process::id();

        for _ in 0..STARTUP_ATTEMPTS {
            let copies = running_copies(self_pid);
            if copies.is_empty() {
                if platform::acquire_lock() {
                    return true;
                }
                std::thread::sleep(RETRY_DELAY);
                continue;
            }

            let best = best_copy(&copies).expect("copies is not empty");
            if current_is_newer(&current, best) {
                let survivors = terminate_copies(&copies);
                if let Some(survivor) = best_copy(&survivors) {
                    forward_to(survivor, show_existing);
                    return false;
                }
                continue;
            }

            if best.version.as_ref() == Some(&current) && platform::acquire_lock() {
                let survivors = terminate_copies(&copies);
                if let Some(survivor) = best_copy(&survivors) {
                    forward_to(survivor, show_existing);
                    return false;
                }
                return true;
            }

            for copy in &copies {
                if copy.pid != best.pid {
                    let _ = platform::terminate(copy.pid);
                }
            }
            forward_to(best, show_existing);
            return false;
        }

        if let Some(best) = best_copy(&running_copies(self_pid)) {
            forward_to(best, show_existing);
        }
        false
    }

    pub fn terminate_other_instances() {
        let self_pid = std::process::id();
        for entry in process_entries() {
            if entry.pid != self_pid && looks_like_our_build(&entry.name) {
                let _ = platform::terminate(entry.pid);
            }
        }
    }

    pub fn take_show_request() -> bool {
        let path = show_request_path(std::process::id());
        if !path.is_file() {
            return false;
        }
        let _ = std::fs::remove_file(&path);
        true
    }

    fn running_copies(self_pid: u32) -> Vec<RunningCopy> {
        process_entries()
            .into_iter()
            .filter(|entry| entry.pid != self_pid && looks_like_our_build(&entry.name))
            .map(|entry| RunningCopy {
                pid: entry.pid,
                version: entry.path.as_deref().and_then(query_version),
            })
            .collect()
    }

    fn terminate_copies(copies: &[RunningCopy]) -> Vec<RunningCopy> {
        copies
            .iter()
            .filter(|copy| !platform::terminate(copy.pid))
            .cloned()
            .collect()
    }

    fn current_is_newer(current: &Version, best: &RunningCopy) -> bool {
        best.version
            .as_ref()
            .is_some_and(|version| current > version)
    }

    fn best_copy(copies: &[RunningCopy]) -> Option<&RunningCopy> {
        copies.iter().reduce(|best, next| {
            if copy_precedes(next, best) {
                next
            } else {
                best
            }
        })
    }

    fn copy_precedes(left: &RunningCopy, right: &RunningCopy) -> bool {
        match (&left.version, &right.version) {
            (None, None) => left.pid < right.pid,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(a), Some(b)) => a > b || (a == b && left.pid < right.pid),
        }
    }

    fn forward_to(copy: &RunningCopy, show_existing: bool) {
        if show_existing {
            #[cfg(windows)]
            platform::allow_foreground(copy.pid);
            let _ = request_show(copy.pid);
        }
    }

    fn show_request_path(pid: u32) -> PathBuf {
        crate::config::config_dir().join(format!("{SHOW_REQUEST_PREFIX}{pid}.request"))
    }

    fn request_show(pid: u32) -> std::io::Result<()> {
        let target = show_request_path(pid);
        let dir = target
            .parent()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?;
        std::fs::create_dir_all(dir)?;
        let temp = dir.join(format!(
            ".{SHOW_REQUEST_PREFIX}{pid}-{}.tmp",
            std::process::id()
        ));
        std::fs::write(
            &temp,
            format!("target_pid={pid}\nrequested_by={}\n", std::process::id()),
        )?;
        let _ = std::fs::remove_file(&target);
        std::fs::rename(&temp, &target)
    }

    fn parse_version_output(output: &str) -> Option<Version> {
        output
            .split_whitespace()
            .filter_map(|part| Version::parse(part).ok())
            .last()
    }

    fn query_version(path: &std::path::Path) -> Option<Version> {
        let mut command = Command::new(path);
        command
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }

        let mut child = command.spawn().ok()?;
        let deadline = Instant::now() + VERSION_QUERY_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let mut output = String::new();
                    child.stdout.take()?.read_to_string(&mut output).ok()?;
                    return parse_version_output(&output);
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    #[cfg(windows)]
    fn process_entries() -> Vec<ProcessEntry> {
        windows::process_entries()
    }

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
    fn process_entries() -> Vec<ProcessEntry> {
        unix::process_entries()
    }

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
    use unix as platform;
    #[cfg(windows)]
    use windows as platform;

    #[cfg(windows)]
    mod windows {
        use std::ffi::{c_void, OsString};
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;
        use std::sync::Mutex;

        use super::ProcessEntry;

        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const PROCESS_TERMINATE: u32 = 0x0001;
        const SYNCHRONIZE: u32 = 0x0010_0000;
        const WAIT_OBJECT_0: u32 = 0;
        const WAIT_TIMEOUT: u32 = 258;
        const ERROR_ALREADY_EXISTS: u32 = 183;
        const INSTANCE_MUTEX: &str = "Local\\wallpaper-rs-single-instance-v1";

        pub fn allow_foreground(pid: u32) {
            unsafe {
                AllowSetForegroundWindow(pid);
            }
        }

        pub fn process_entries() -> Vec<ProcessEntry> {
            let mut entries = Vec::new();
            crate::wallpaper::for_each_running_process(|name, pid| {
                let path = if super::looks_like_our_build(name) {
                    full_image_path(pid)
                } else {
                    None
                };
                entries.push(ProcessEntry {
                    pid,
                    name: name.to_string(),
                    path,
                });
            });
            entries
        }

        pub fn acquire_lock() -> bool {
            static HELD: Mutex<Option<isize>> = Mutex::new(None);
            let mut held = match HELD.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if held.is_some() {
                return true;
            }

            let name = wide(INSTANCE_MUTEX);
            unsafe {
                SetLastError(0);
                let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
                if handle.is_null() {
                    return false;
                }
                if GetLastError() == ERROR_ALREADY_EXISTS as i32 {
                    CloseHandle(handle);
                    return false;
                }
                *held = Some(handle as isize);
            }
            true
        }

        pub fn terminate(pid: u32) -> bool {
            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE, 0, pid);
                if handle.is_null() {
                    return !process_alive(pid);
                }
                let _ = TerminateProcess(handle, 1);
                let wait = WaitForSingleObject(handle, 3000);
                CloseHandle(handle);
                wait == WAIT_OBJECT_0 || !process_alive(pid)
            }
        }

        fn process_alive(pid: u32) -> bool {
            unsafe {
                let handle = OpenProcess(SYNCHRONIZE, 0, pid);
                if handle.is_null() {
                    return false;
                }
                let wait = WaitForSingleObject(handle, 0);
                CloseHandle(handle);
                wait == WAIT_TIMEOUT
            }
        }

        fn full_image_path(pid: u32) -> Option<PathBuf> {
            unsafe {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    return None;
                }
                let mut buffer = vec![0u16; 32_768];
                let mut len = buffer.len() as u32;
                let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut len);
                CloseHandle(handle);
                if ok == 0 {
                    return None;
                }
                buffer.truncate(len as usize);
                Some(PathBuf::from(OsString::from_wide(&buffer)))
            }
        }

        fn wide(value: &str) -> Vec<u16> {
            value.encode_utf16().chain(std::iter::once(0)).collect()
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(desired_access: u32, inherit_handle: i32, pid: u32) -> *mut c_void;
            fn QueryFullProcessImageNameW(
                process: *mut c_void,
                flags: u32,
                exe_name: *mut u16,
                size: *mut u32,
            ) -> i32;
            fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
            fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
            fn CreateMutexW(
                attributes: *const c_void,
                initial_owner: i32,
                name: *const u16,
            ) -> *mut c_void;
            fn CloseHandle(handle: *mut c_void) -> i32;
            fn GetLastError() -> i32;
            fn SetLastError(error: u32);
        }

        #[link(name = "user32")]
        extern "system" {
            fn AllowSetForegroundWindow(pid: u32) -> i32;
        }
    }

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
    mod unix {
        use std::fs::{File, OpenOptions};
        use std::os::unix::io::AsRawFd;
        use std::sync::Mutex;
        use std::time::{Duration, Instant};

        use super::ProcessEntry;

        const LOCK_EX: i32 = 2;
        const LOCK_NB: i32 = 4;
        const SIGKILL: i32 = 9;
        const SIGTERM: i32 = 15;

        pub fn process_entries() -> Vec<ProcessEntry> {
            #[cfg(target_os = "macos")]
            {
                macos_process_entries()
            }
            #[cfg(not(target_os = "macos"))]
            {
                procfs_process_entries()
            }
        }

        pub fn acquire_lock() -> bool {
            static HELD: Mutex<Option<File>> = Mutex::new(None);
            let mut held = match HELD.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if held.is_some() {
                return true;
            }

            let path = std::env::temp_dir().join("wallpaper-rs-single-instance.lock");
            let Ok(file) = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(path)
            else {
                return false;
            };
            let locked = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0;
            if locked {
                *held = Some(file);
            }
            locked
        }

        pub fn terminate(pid: u32) -> bool {
            if !process_alive(pid) {
                return true;
            }
            unsafe {
                kill(pid as i32, SIGTERM);
            }
            if wait_until_gone(pid, Duration::from_millis(1500)) {
                return true;
            }
            unsafe {
                kill(pid as i32, SIGKILL);
            }
            wait_until_gone(pid, Duration::from_millis(1500))
        }

        fn wait_until_gone(pid: u32, timeout: Duration) -> bool {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if !process_alive(pid) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            !process_alive(pid)
        }

        fn process_alive(pid: u32) -> bool {
            let result = unsafe { kill(pid as i32, 0) };
            result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(1)
        }

        #[cfg(target_os = "macos")]
        fn macos_process_entries() -> Vec<ProcessEntry> {
            use std::path::PathBuf;

            let Ok(output) = std::process::Command::new("ps")
                .args(["-axo", "pid=,comm="])
                .output()
            else {
                return Vec::new();
            };
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| {
                    let (pid, command) = line.trim().split_once(char::is_whitespace)?;
                    let pid = pid.parse().ok()?;
                    let path = PathBuf::from(command.trim());
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| command.trim().to_string());
                    Some(ProcessEntry {
                        pid,
                        name,
                        path: Some(path),
                    })
                })
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        fn procfs_process_entries() -> Vec<ProcessEntry> {
            let Ok(entries) = std::fs::read_dir("/proc") else {
                return Vec::new();
            };
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let pid = entry.file_name().to_string_lossy().parse().ok()?;
                    let dir = entry.path();
                    let path = std::fs::read_link(dir.join("exe")).ok();
                    let name = path
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .map(|name| name.to_string_lossy().into_owned())
                        .or_else(|| std::fs::read_to_string(dir.join("comm")).ok())
                        .map(|name| name.trim().to_string())?;
                    Some(ProcessEntry { pid, name, path })
                })
                .collect()
        }

        extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
            fn kill(pid: i32, signal: i32) -> i32;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            best_copy, copy_precedes, current_is_newer, parse_version_output, RunningCopy,
        };
        use semver::Version;

        fn copy(pid: u32, version: &str) -> RunningCopy {
            RunningCopy {
                pid,
                version: Some(Version::parse(version).unwrap()),
            }
        }

        #[test]
        fn parses_version_from_cli_output() {
            assert_eq!(
                parse_version_output("wallpaper-rs 0.14.13\r\n"),
                Some(Version::new(0, 14, 13))
            );
            assert_eq!(parse_version_output("not a version"), None);
        }

        #[test]
        fn newer_current_replaces_older_instances() {
            let current = Version::new(0, 15, 0);
            let running = copy(10, "0.14.13");
            assert!(current_is_newer(&current, &running));
        }

        #[test]
        fn equal_version_never_replaces_running_instance() {
            let current = Version::new(0, 14, 13);
            let running = copy(10, "0.14.13");
            assert!(!current_is_newer(&current, &running));
        }

        #[test]
        fn older_current_never_replaces_newer_instance() {
            let current = Version::new(0, 14, 12);
            let running = copy(10, "0.14.13");
            assert!(!current_is_newer(&current, &running));
        }

        #[test]
        fn unknown_running_version_is_not_killed() {
            let current = Version::new(99, 0, 0);
            let running = RunningCopy {
                pid: 10,
                version: None,
            };
            assert!(!current_is_newer(&current, &running));
        }

        #[test]
        fn best_copy_prefers_highest_version_then_lowest_pid() {
            let copies = vec![
                copy(30, "0.14.12"),
                copy(20, "0.14.13"),
                copy(10, "0.14.13"),
            ];
            assert_eq!(best_copy(&copies).unwrap().pid, 10);
            assert!(copy_precedes(&copy(10, "0.15.0"), &copy(5, "0.14.99")));
            assert!(copy_precedes(
                &RunningCopy {
                    pid: 10,
                    version: None,
                },
                &copy(5, "99.0.0")
            ));
        }
    }
}
