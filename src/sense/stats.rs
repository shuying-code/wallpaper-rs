//! 今日统计与事件日志（落地到配置目录，跨进程保留当天数据）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 一天的统计。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DailyStats {
    /// 本地日期 `YYYYMMDD`。
    pub date: String,
    pub locked: u32,
    pub unlock_hints: u32,
    pub strangers: u32,
    pub away_events: u32,
}

fn path() -> PathBuf {
    crate::config::config_dir().join("sense-stats.json")
}

fn today() -> String {
    // 不引入 chrono：用系统时间换算 UTC 日期即可（只用于「今天」分桶）。
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    // 1970-01-01 起的天数 → 日期（Howard Hinnant 的 civil_from_days）。
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}")
}

impl DailyStats {
    /// 读取当天统计；日期变了就清零。
    pub fn load() -> Self {
        let t = today();
        let mut s = std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str::<DailyStats>(&s).ok())
            .unwrap_or_default();
        if s.date != t {
            s = DailyStats {
                date: t,
                ..Default::default()
            };
        }
        s
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::config::config_dir());
        if let Ok(s) = serde_json::to_string(self) {
            let _ = std::fs::write(path(), s);
        }
    }

    pub fn bump_lock(&mut self) {
        self.locked = self.locked.saturating_add(1);
        self.save();
    }
    pub fn bump_unlock_hint(&mut self) {
        self.unlock_hints = self.unlock_hints.saturating_add(1);
        self.save();
    }
    pub fn bump_stranger(&mut self) {
        self.strangers = self.strangers.saturating_add(1);
        self.save();
    }
    pub fn bump_away(&mut self) {
        self.away_events = self.away_events.saturating_add(1);
        self.save();
    }
}

/// `HH:MM` 本地时间（用于事件时间线）。
pub fn clock() -> String {
    // 用本地时区偏移：Windows 上 TZ 由系统决定，这里取 UTC+8 的常见情形交给
    // 系统 「GetLocalTime」；非 Windows 回落 UTC。
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            millis: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(t: *mut SystemTime);
        }
        let mut t = SystemTime {
            year: 0,
            month: 0,
            day_of_week: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
        };
        unsafe { GetLocalTime(&mut t) };
        return format!("{:02}:{:02}", t.hour, t.minute);
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let s = (secs % 86_400) as u32;
        format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_eight_digits() {
        let t = today();
        assert_eq!(t.len(), 8, "{t}");
        assert!(t.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn counters_saturate() {
        let mut s = DailyStats {
            locked: u32::MAX,
            ..Default::default()
        };
        s.bump_lock();
        assert_eq!(s.locked, u32::MAX);
    }
}
