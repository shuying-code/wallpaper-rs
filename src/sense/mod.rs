//! 人脸感知功能域：三态状态机、离开判定、锁定动作。
//!
//! 设计约束（见 `智能感知与UI重设计方案.md` 第 3 节）：
//! - 状态机是**纯函数**，不依赖摄像头也能单测；
//! - 摄像头采集失败时退化到「键鼠空闲检测」，仍然能真实完成人走锁屏；
//! - 「人回自动解锁」在普通用户态做不到（LogonUI 是 secure desktop），
//!   默认只做「唤醒 + 提示」，不假装成功。

use crate::config::{LockAction, SenseCfg};
use std::time::{Duration, Instant};

pub mod blur;
pub mod camera;
pub mod stats;

pub use stats::DailyStats;

/// 感知状态。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Presence {
    /// 启动后尚未拿到首帧，绝不触发锁屏。
    #[default]
    Unknown,
    /// 在场（含短时丢帧宽限）。
    Present,
    /// 已判定离开，正在倒计时。
    Countdown,
    /// 已锁定。
    Locked,
    /// 检测到未授权人脸：本次会话不再自动解锁。
    Stranger,
}

impl Presence {
    pub fn label(self) -> &'static str {
        match self {
            Presence::Unknown => "等待首帧",
            Presence::Present => "在场",
            Presence::Countdown => "正在离开",
            Presence::Locked => "已锁定",
            Presence::Stranger => "陌生人",
        }
    }
}

/// 一次识别结果。
#[derive(Clone, Debug)]
pub struct FaceHit {
    pub name: String,
    /// 余弦相似度，0–1。
    pub similarity: f32,
    /// 是否为已授权人脸。
    pub authorized: bool,
}

/// 喂给状态机的事件。
#[derive(Clone, Debug)]
pub enum SenseEvent {
    /// 检测到一张人脸。
    Face(FaceHit),
    /// 本帧没有检测到人脸。
    NoFace,
    /// 键鼠 / 触控有输入。
    InputActivity,
    /// 时间推进（由 UI 每帧调用，内部按 wall clock 计算）。
    Tick,
    /// 用户手动「立即锁定」。
    ManualLock,
    /// 用户手动「我还在」。
    ManualPresent,
}

/// 状态机输出的副作用，由 UI / 执行器消费。
#[derive(Clone, Debug, PartialEq)]
pub enum SenseEffect {
    /// 进入倒计时，附剩余秒数。
    EnterCountdown(u32),
    /// 取消倒计时（人回来了）。
    CancelCountdown,
    /// 立即执行锁定动作序列。
    Lock,
    /// 解锁提示（务实路线：唤醒屏幕 + 提示，不注入凭据）。
    UnlockHint,
    /// 壁纸虚化开关。
    BlurWallpaper(bool),
    /// 记录一条事件。
    Log(String),
}

/// 对外只读状态，供 UI 渲染。
#[derive(Clone, Debug, Default)]
pub struct SenseStatus {
    pub presence: Presence,
    /// 连续在场的时长。
    pub present_for: Duration,
    /// 连续未检测到人脸的时长。
    pub away_for: Duration,
    /// 当前识别到的人脸。
    pub face: Option<FaceHit>,
    /// 倒计时总时长（秒）。
    pub countdown_total: u32,
    /// 距离锁屏还剩多久。
    pub countdown_left: Duration,
    /// 最近一条事件文案。
    pub last_event: String,
    /// 本次会话是否被陌生人脸锁死。
    pub stranger_latched: bool,
}

/// 状态机。时间基准用注入的 `Instant`，便于单测。
pub struct SenseEngine {
    presence: Presence,
    present_since: Option<Instant>,
    miss_since: Option<Instant>,
    countdown_deadline: Option<Instant>,
    countdown_total: u32,
    /// 倒计时到点后进入的「二次确认」宽限截止时间。
    confirm_deadline: Option<Instant>,
    face: Option<FaceHit>,
    stranger_latched: bool,
    last_event: String,
    last_input: Instant,
}

impl Default for SenseEngine {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl SenseEngine {
    pub fn new(now: Instant) -> Self {
        Self {
            presence: Presence::Unknown,
            present_since: None,
            miss_since: None,
            countdown_deadline: None,
            countdown_total: 0,
            confirm_deadline: None,
            face: None,
            stranger_latched: false,
            last_event: "等待首帧".to_string(),
            last_input: now,
        }
    }

    pub fn presence(&self) -> Presence {
        self.presence
    }

    /// 推进状态机。`now` 由调用方提供，保证可测试。
    pub fn step(&mut self, now: Instant, ev: &SenseEvent, cfg: &SenseCfg) -> Vec<SenseEffect> {
        let mut out = Vec::new();
        let grace = Duration::from_secs(cfg.grace_secs as u64);
        match ev {
            SenseEvent::InputActivity => {
                self.last_input = now;
                if self.presence == Presence::Countdown {
                    self.presence = Presence::Present;
                    self.countdown_deadline = None;
                    self.confirm_deadline = None;
                    self.present_since = Some(now);
                    self.miss_since = None;
                    out.push(SenseEffect::CancelCountdown);
                    out.push(SenseEffect::Log("检测到输入，取消离开倒计时".into()));
                }
            }
            SenseEvent::Face(hit) => {
                self.face = Some(hit.clone());
                if !hit.authorized {
                    if cfg.stranger_lock && self.presence != Presence::Stranger {
                        self.presence = Presence::Stranger;
                        self.stranger_latched = true;
                        self.countdown_deadline = None;
                        self.confirm_deadline = None;
                        out.push(SenseEffect::Lock);
                        out.push(SenseEffect::Log(format!(
                            "检测到陌生人脸（{}），已立即锁定",
                            hit.name
                        )));
                    }
                    return out;
                }
                // 锁定态下只有「已授权人脸」能发出解锁提示，且被陌生人锁死时不做。
                if self.presence == Presence::Locked {
                    if cfg.auto_unlock && !self.stranger_latched && !cfg.require_pin {
                        out.push(SenseEffect::UnlockHint);
                    }
                    return out;
                }
                if self.presence == Presence::Stranger {
                    return out;
                }
                if self.presence != Presence::Present {
                    out.push(SenseEffect::Log(format!("检测到 {} 在场", hit.name)));
                }
                self.presence = Presence::Present;
                self.miss_since = None;
                self.countdown_deadline = None;
                self.confirm_deadline = None;
                self.countdown_total = 0;
                if self.present_since.is_none() {
                    self.present_since = Some(now);
                }
            }
            SenseEvent::NoFace => {
                self.face = None;
                match self.presence {
                    Presence::Present => {
                        let miss = *self.miss_since.get_or_insert(now);
                        // 宽限期内的丢帧不算离开。
                        let away = now.saturating_duration_since(miss);
                        let need = Duration::from_secs(cfg.away_secs as u64);
                        if away + grace >= need && cfg.away_secs > 0 {
                            self.presence = Presence::Countdown;
                            self.countdown_total = cfg.away_secs;
                            self.countdown_deadline =
                                Some(now + Duration::from_secs(cfg.away_secs as u64));
                            self.confirm_deadline = None;
                            out.push(SenseEffect::EnterCountdown(cfg.away_secs));
                            out.push(SenseEffect::Log(format!(
                                "离线 {} 秒，开始离开倒计时",
                                cfg.away_secs
                            )));
                        }
                    }
                    Presence::Unknown => {
                        // 启动即无人脸不锁，只记录。
                        self.present_since = None;
                    }
                    _ => {}
                }
            }
            SenseEvent::Tick => {
                if self.presence == Presence::Countdown {
                    if let Some(deadline) = self.confirm_deadline {
                        // 二次确认窗口：仍然无人脸才真正锁屏。
                        if now >= deadline {
                            self.presence = Presence::Locked;
                            self.countdown_deadline = None;
                            self.confirm_deadline = None;
                            out.push(SenseEffect::Lock);
                            out.push(SenseEffect::Log("二次确认仍无人，执行锁定".into()));
                        }
                    } else if let Some(deadline) = self.countdown_deadline {
                        if now >= deadline {
                            if cfg.confirm_second_pass {
                                let grace = Duration::from_secs(cfg.grace_secs.max(1) as u64);
                                self.confirm_deadline = Some(now + grace);
                                out.push(SenseEffect::Log(format!(
                                    "倒计时结束，再确认 {} 秒",
                                    grace.as_secs()
                                )));
                            } else {
                                self.presence = Presence::Locked;
                                self.countdown_deadline = None;
                                out.push(SenseEffect::Lock);
                                out.push(SenseEffect::Log("离开超时，执行锁定".into()));
                            }
                        }
                    }
                }
                // 键鼠活动续期：有输入就重置离开计时。
                if cfg.input_reset_secs > 0
                    && self.presence == Presence::Present
                    && now.saturating_duration_since(self.last_input) < Duration::from_secs(1)
                {
                    self.miss_since = None;
                }
            }
            SenseEvent::ManualLock => {
                self.presence = Presence::Locked;
                self.countdown_deadline = None;
                self.confirm_deadline = None;
                self.stranger_latched = false;
                out.push(SenseEffect::Lock);
                out.push(SenseEffect::Log("手动锁定".into()));
            }
            SenseEvent::ManualPresent => {
                self.presence = Presence::Present;
                self.present_since = Some(now);
                self.miss_since = None;
                self.countdown_deadline = None;
                self.confirm_deadline = None;
                self.countdown_total = 0;
                out.push(SenseEffect::CancelCountdown);
                out.push(SenseEffect::Log("我还在：重置离开计时".into()));
            }
        }
        if let Some(SenseEffect::Log(l)) = out.last() {
            self.last_event = l.clone();
        }
        out
    }

    pub fn status(&self, now: Instant) -> SenseStatus {
        SenseStatus {
            presence: self.presence,
            present_for: self
                .present_since
                .map(|t| now.saturating_duration_since(t))
                .unwrap_or_default(),
            away_for: self
                .miss_since
                .map(|t| now.saturating_duration_since(t))
                .unwrap_or_default(),
            face: self.face.clone(),
            countdown_total: self.countdown_total,
            countdown_left: self
                .confirm_deadline
                .or(self.countdown_deadline)
                .map(|d| d.saturating_duration_since(now))
                .unwrap_or_default(),
            last_event: self.last_event.clone(),
            stranger_latched: self.stranger_latched,
        }
    }
}

/// 系统空闲秒数（键鼠无输入）。摄像头不可用时的真实退化通道。
pub fn idle_seconds() -> Option<u64> {
    platform_idle_seconds()
}

/// 前台窗口是否覆盖整块显示器（视频 / 游戏 / 演示）。仅 Windows 有可靠实现；
/// 其它平台返回 `false`，避免误判成「一直在全屏」而永久跳过判定。
pub fn foreground_fullscreen() -> bool {
    platform_foreground_fullscreen()
}

#[cfg(windows)]
fn platform_foreground_fullscreen() -> bool {
    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: MonitorRect,
        rc_work: MonitorRect,
        flags: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct MonitorRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
        fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
        fn GetClassNameW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
        fn IsWindowVisible(hwnd: isize) -> i32;
    }
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    let hwnd = crate::winhide::foreground_window();
    if hwnd == 0 || unsafe { IsWindowVisible(hwnd) } == 0 {
        return false;
    }
    // 桌面 / 任务栏本身不算全屏内容。
    let mut class_buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32) };
    if n > 0 {
        let class = String::from_utf16_lossy(&class_buf[..n as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
    }

    let Some((left, top, right, bottom)) = crate::winhide::window_rect(hwnd) else {
        return false;
    };
    let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if mon == 0 {
        return false;
    }
    let mut mi = MonitorInfo {
        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
        rc_monitor: MonitorRect::default(),
        rc_work: MonitorRect::default(),
        flags: 0,
    };
    if unsafe { GetMonitorInfoW(mon, &mut mi) } == 0 {
        return false;
    }
    let tol = 2;
    let m = &mi.rc_monitor;
    left <= m.left + tol && top <= m.top + tol && right >= m.right - tol && bottom >= m.bottom - tol
}

#[cfg(not(windows))]
fn platform_foreground_fullscreen() -> bool {
    false
}

#[cfg(windows)]
fn platform_idle_seconds() -> Option<u64> {
    #[repr(C)]
    struct LastInputInfo {
        cb_size: u32,
        dw_time: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(pli: *mut LastInputInfo) -> i32;
        fn GetTickCount() -> u32;
    }
    let mut li = LastInputInfo {
        cb_size: std::mem::size_of::<LastInputInfo>() as u32,
        dw_time: 0,
    };
    if unsafe { GetLastInputInfo(&mut li) } == 0 {
        return None;
    }
    let now = unsafe { GetTickCount() };
    let idle = now.wrapping_sub(li.dw_time) as u64 / 1000;
    Some(idle)
}

#[cfg(not(windows))]
fn platform_idle_seconds() -> Option<u64> {
    None
}

/// 执行「锁定动作」序列。返回每一步的结果，UI 按实际结果展示。
pub fn run_lock_actions(actions: &[LockAction]) -> Vec<(LockAction, Result<(), String>)> {
    actions.iter().map(|a| (*a, run_lock_action(*a))).collect()
}

pub fn run_lock_action(action: LockAction) -> Result<(), String> {
    match action {
        LockAction::Blur => Ok(()),
        LockAction::MonitorOff => monitor_off(),
        LockAction::Mute => toggle_mute(),
        LockAction::Lock => lock_workstation(),
    }
}

/// 唤醒显示器 + 轻微移动光标。这是务实路线上「人回自动解锁」能做的全部：
/// 锁屏是 `LogonUI.exe` 的 secure desktop，第三方进程无法绘制、无法注入凭据。
pub fn wake_screen() -> Result<(), String> {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn mouse_event(flags: u32, dx: i32, dy: i32, data: u32, extra: usize);
        }
        const MOUSEEVENTF_MOVE: u32 = 0x0001;
        // 移动 1px 再移回：足以让系统认为有输入并点亮显示器，又不干扰用户操作。
        unsafe {
            mouse_event(MOUSEEVENTF_MOVE, 1, 0, 0, 0);
            mouse_event(MOUSEEVENTF_MOVE, -1, 0, 0, 0);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("当前平台不支持唤醒".to_string())
    }
}

#[cfg(windows)]
pub fn lock_workstation() -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn LockWorkStation() -> i32;
    }
    if unsafe { LockWorkStation() } != 0 {
        Ok(())
    } else {
        Err("LockWorkStation 调用失败".to_string())
    }
}

#[cfg(not(windows))]
pub fn lock_workstation() -> Result<(), String> {
    Err("当前平台不支持锁屏".to_string())
}

#[cfg(windows)]
pub fn monitor_off() -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn SendMessageA(hwnd: isize, msg: u32, w: usize, l: isize) -> isize;
    }
    const WM_SYSCOMMAND: u32 = 0x0112;
    const SC_MONITORPOWER: usize = 0xF170;
    const HWND_BROADCAST: isize = 0xFFFF;
    unsafe {
        SendMessageA(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, 2);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn monitor_off() -> Result<(), String> {
    Err("当前平台不支持息屏".to_string())
}

#[cfg(windows)]
pub fn toggle_mute() -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
    }
    const VK_VOLUME_MUTE: u8 = 0xAD;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    unsafe {
        keybd_event(VK_VOLUME_MUTE, 0, 0, 0);
        keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn toggle_mute() -> Result<(), String> {
    Err("当前平台不支持静音".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(away: u32) -> SenseCfg {
        let mut c = SenseCfg::default();
        c.away_secs = away;
        c.grace_secs = 3;
        c.auto_unlock = true;
        c
    }

    fn hit(authorized: bool) -> FaceHit {
        FaceHit {
            name: "张 Z".into(),
            similarity: 0.98,
            authorized,
        }
    }

    #[test]
    fn unknown_never_locks_on_startup() {
        let mut e = SenseEngine::new(Instant::now());
        let t0 = Instant::now();
        let out = e.step(t0 + Duration::from_secs(60), &SenseEvent::NoFace, &cfg(10));
        assert!(out.is_empty());
        assert_eq!(e.presence(), Presence::Unknown);
    }

    #[test]
    fn face_present_then_away_enters_countdown_and_locks() {
        let mut e = SenseEngine::new(Instant::now());
        // 本用例覆盖「倒计时到点即锁」的基线路径；二次确认另有专门用例。
        let mut c = cfg(10);
        c.confirm_second_pass = false;
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        assert_eq!(e.presence(), Presence::Present);
        // 丢帧：进入 present 后第一次 miss 记录起点
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Present);
        // 超过 away + grace → 倒计时
        let out = e.step(t0 + Duration::from_secs(14), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        assert!(out
            .iter()
            .any(|x| matches!(x, SenseEffect::EnterCountdown(_))));
        // 倒计时未到点：不锁
        let out = e.step(t0 + Duration::from_secs(15), &SenseEvent::Tick, &c);
        assert!(out.is_empty());
        // 到点锁屏
        let out = e.step(t0 + Duration::from_secs(25), &SenseEvent::Tick, &c);
        assert_eq!(e.presence(), Presence::Locked);
        assert!(out.contains(&SenseEffect::Lock));
    }

    #[test]
    fn face_returns_cancels_countdown() {
        let mut e = SenseEngine::new(Instant::now());
        let c = cfg(5);
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        e.step(t0 + Duration::from_secs(9), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        let out = e.step(
            t0 + Duration::from_secs(10),
            &SenseEvent::Face(hit(true)),
            &c,
        );
        assert_eq!(e.presence(), Presence::Present);
        assert!(!out.contains(&SenseEffect::Lock));
    }

    #[test]
    fn stranger_locks_and_blocks_auto_unlock() {
        let mut e = SenseEngine::new(Instant::now());
        let c = cfg(10);
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(false)), &c);
        assert_eq!(e.presence(), Presence::Stranger);
        // 再次看到已授权人脸也不解锁
        let out = e.step(
            t0 + Duration::from_secs(2),
            &SenseEvent::Face(hit(true)),
            &c,
        );
        assert!(!out.contains(&SenseEffect::UnlockHint));
        assert_eq!(e.presence(), Presence::Stranger);
    }

    #[test]
    fn manual_lock_then_authorized_face_emits_unlock_hint() {
        let mut e = SenseEngine::new(Instant::now());
        let c = cfg(10);
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::ManualLock, &c);
        assert_eq!(e.presence(), Presence::Locked);
        let out = e.step(
            t0 + Duration::from_secs(3),
            &SenseEvent::Face(hit(true)),
            &c,
        );
        assert!(out.contains(&SenseEffect::UnlockHint));
    }

    #[test]
    fn require_pin_suppresses_unlock_hint() {
        let mut e = SenseEngine::new(Instant::now());
        let mut c = cfg(10);
        c.require_pin = true;
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::ManualLock, &c);
        let out = e.step(
            t0 + Duration::from_secs(3),
            &SenseEvent::Face(hit(true)),
            &c,
        );
        assert!(!out.contains(&SenseEffect::UnlockHint));
    }

    #[test]
    fn input_activity_resets_countdown() {
        let mut e = SenseEngine::new(Instant::now());
        let c = cfg(5);
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        e.step(t0 + Duration::from_secs(9), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        let out = e.step(t0 + Duration::from_secs(10), &SenseEvent::InputActivity, &c);
        assert!(out.contains(&SenseEffect::CancelCountdown));
    }

    /// 二次确认：倒计时到点先不锁，宽限期结束后仍无人脸才锁；
    /// 宽限期内人脸回来则取消。
    #[test]
    fn second_pass_delays_lock_until_grace_expires() {
        let mut e = SenseEngine::new(Instant::now());
        let mut c = cfg(5);
        c.grace_secs = 2;
        c.confirm_second_pass = true;
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        e.step(t0 + Duration::from_secs(9), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Countdown);

        // 倒计时到点：进入二次确认，不立即锁。
        let out = e.step(t0 + Duration::from_secs(15), &SenseEvent::Tick, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        assert!(!out.contains(&SenseEffect::Lock));

        // 宽限期内回来：取消，不锁。
        let out = e.step(
            t0 + Duration::from_secs(16),
            &SenseEvent::Face(hit(true)),
            &c,
        );
        assert_eq!(e.presence(), Presence::Present);
        assert!(!out.contains(&SenseEffect::Lock));
    }

    /// 二次确认宽限结束仍无人：锁定。
    #[test]
    fn second_pass_locks_after_grace_when_still_away() {
        let mut e = SenseEngine::new(Instant::now());
        let mut c = cfg(5);
        c.grace_secs = 2;
        c.confirm_second_pass = true;
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        e.step(t0 + Duration::from_secs(9), &SenseEvent::NoFace, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        e.step(t0 + Duration::from_secs(15), &SenseEvent::Tick, &c);
        assert_eq!(e.presence(), Presence::Countdown);
        let out = e.step(t0 + Duration::from_secs(18), &SenseEvent::Tick, &c);
        assert_eq!(e.presence(), Presence::Locked);
        assert!(out.contains(&SenseEffect::Lock));
    }

    /// 关闭二次确认时，倒计时到点立即锁。
    #[test]
    fn no_second_pass_locks_at_deadline() {
        let mut e = SenseEngine::new(Instant::now());
        let mut c = cfg(5);
        c.confirm_second_pass = false;
        let t0 = Instant::now();
        e.step(t0, &SenseEvent::Face(hit(true)), &c);
        e.step(t0 + Duration::from_secs(1), &SenseEvent::NoFace, &c);
        e.step(t0 + Duration::from_secs(9), &SenseEvent::NoFace, &c);
        let out = e.step(t0 + Duration::from_secs(15), &SenseEvent::Tick, &c);
        assert_eq!(e.presence(), Presence::Locked);
        assert!(out.contains(&SenseEffect::Lock));
    }
}
