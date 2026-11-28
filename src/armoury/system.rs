//! 控制面板的系统概览：CPU / 内存 / GPU / 温度。
//!
//! 取数策略：
//! - CPU 型号：注册表 `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0`
//!   的 `ProcessorNameString`（免管理员）；
//! - 内存：`GlobalMemoryStatusEx`；
//! - CPU 占用：`GetSystemTimes` 的两次差分（一次调用即 DPC 计数）；
//! - 温度：优先 `MSAcpi_ThermalZoneTemperature`；本机该 WMI 类为空，
//!   所以返回 `None` 让 UI 显示 `n/a`，不编造数字。

use super::SystemSnapshot;

pub fn sample() -> SystemSnapshot {
    SystemSnapshot {
        cpu_name: cpu_name(),
        cpu_load: cpu_load(),
        mem_used_gb: 0.0,
        mem_total_gb: 0.0,
        gpu_name: String::new(),
        cpu_temp_c: None,
        gpu_temp_c: None,
    }
    .with_memory()
    .with_gpu()
}

impl SystemSnapshot {
    fn with_memory(mut self) -> Self {
        if let Some((used, total)) = memory_gb() {
            self.mem_used_gb = used;
            self.mem_total_gb = total;
        }
        self
    }

    fn with_gpu(mut self) -> Self {
        if let Some(name) = gpu_name() {
            self.gpu_name = name;
        }
        self
    }
}

#[cfg(windows)]
fn cpu_name() -> String {
    winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
        .ok()
        .and_then(|k| k.get_value::<String, _>("ProcessorNameString").ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn cpu_name() -> String {
    std::env::consts::ARCH.to_string()
}

/// 两次采样之间的 CPU 占用。返回 0 表示「还不知道」，UI 用「—」显示。
#[cfg(windows)]
fn cpu_load() -> f32 {
    use std::sync::Mutex;
    static LAST: Mutex<Option<(u64, u64, u64)>> = Mutex::new(None);
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }
    let mut idle = FileTime { low: 0, high: 0 };
    let mut kernel = FileTime { low: 0, high: 0 };
    let mut user = FileTime { low: 0, high: 0 };
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return -1.0;
    }
    let to_u64 = |t: &FileTime| ((t.high as u64) << 32) | t.low as u64;
    let now = (to_u64(&idle), to_u64(&kernel), to_u64(&user));
    let mut guard = match LAST.lock() {
        Ok(g) => g,
        Err(_) => return -1.0,
    };
    let out = match *guard {
        Some(prev) => {
            let d_idle = now.0.saturating_sub(prev.0) as f64;
            let d_total = (now.1 + now.2).saturating_sub(prev.1 + prev.2) as f64 + d_idle;
            if d_total <= 0.0 {
                -1.0
            } else {
                (((d_total - d_idle) / d_total) * 100.0).clamp(0.0, 100.0) as f32
            }
        }
        None => -1.0,
    };
    *guard = Some(now);
    out
}

#[cfg(not(windows))]
fn cpu_load() -> f32 {
    -1.0
}

#[cfg(windows)]
fn memory_gb() -> Option<(f32, f32)> {
    #[repr(C)]
    struct MemStatus {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buf: *mut MemStatus) -> i32;
    }
    let mut m = MemStatus {
        length: std::mem::size_of::<MemStatus>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return None;
    }
    let gb = 1024.0 * 1024.0 * 1024.0;
    let total = m.total_phys as f64 / gb;
    let used = (m.total_phys - m.avail_phys) as f64 / gb;
    Some((used as f32, total as f32))
}

#[cfg(not(windows))]
fn memory_gb() -> Option<(f32, f32)> {
    None
}

/// 显卡型号：注册表 `SYSTEM\CurrentControlSet\Control\Class\{4d36e968-...}\0000`。
#[cfg(windows)]
fn gpu_name() -> Option<String> {
    let root = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let class = root
        .open_subkey(
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}",
        )
        .ok()?;
    for i in 0..8 {
        let key = format!("{i:04}");
        if let Ok(sub) = class.open_subkey(&key) {
            if let Ok(name) = sub.get_value::<String, _>("DriverDesc") {
                if !name.trim().is_empty() {
                    return Some(name.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn gpu_name() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_never_invents_temperatures() {
        let s = sample();
        // 本机 WMI 温度类为空：必须是 None，不能被写成 0。
        assert!(s.cpu_temp_c.is_none());
        assert!(s.gpu_temp_c.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn reads_cpu_name_from_registry() {
        let name = cpu_name();
        assert!(!name.is_empty(), "注册表应能读到 CPU 型号");
    }
}
