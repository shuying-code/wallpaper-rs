//! 华硕硬件域：Aura 灯效（官方 COM）、Fan Xpert（只读 XML）、设备清单。
//!
//! 全部结论来自本机实测（见 `ac_api_findings.md`）：
//! - Aura：`AsRogAuraService.ServiceMediator.1`，免管理员，读 / 写均已验证；
//! - Fan：`\\.\Asusgio3` 的 SDDL 只给 SYSTEM / Administrators，
//!   `AsusFanControlService` 对普通会话返回 `E_ACCESSDENIED`，
//!   所以**首版只读**——读标定表与曲线，改配置只写本地，不假装能下发；
//! - 任何设备 IO 都不允许跑在 `update()` 里，统一走后台线程按 `poll_ms` 采样。

pub mod aura;
pub mod fan;
pub mod system;
pub mod xml;

use crate::config::ArmouryCfg;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Aura 服务探测结果，直接驱动 UI 的空状态文案。
#[derive(Clone, Debug, PartialEq)]
pub enum AuraHealth {
    /// 服务可用，已读到设备清单。
    Ready,
    /// 非 Windows。
    Unsupported(&'static str),
    /// 服务未安装 / 未运行。
    Missing(String),
}

impl AuraHealth {
    pub fn ok(&self) -> bool {
        matches!(self, AuraHealth::Ready)
    }
    pub fn summary(&self) -> String {
        match self {
            AuraHealth::Ready => "Aura 服务在线".to_string(),
            AuraHealth::Unsupported(why) => format!("当前平台不支持：{why}"),
            AuraHealth::Missing(why) => format!("Aura 服务不可用：{why}"),
        }
    }
}

/// 一台灯效设备。
#[derive(Clone, Debug, PartialEq)]
pub struct AuraDevice {
    pub kind: String,
    pub model: String,
    pub manufacture: String,
    pub leds: u32,
    pub width: u32,
    pub height: u32,
    pub index: u32,
}

impl AuraDevice {
    /// 稳定的逐设备配置键：类型 + 设备序号。
    pub fn key(&self) -> String {
        Self::key_for(&self.kind, self.index)
    }

    pub fn key_for(kind: &str, index: u32) -> String {
        format!("{kind}#{index}")
    }

    pub fn display_name(&self) -> String {
        if self.model.is_empty() || self.model == "Unkown" {
            format!("{} #{}", self.kind, self.index + 1)
        } else if self.kind == "AddressableStrip" {
            format!("{} #{}", self.model, self.index + 1)
        } else {
            self.model.clone()
        }
    }
    pub fn icon(&self) -> &'static str {
        match self.kind.as_str() {
            "Mainboard_Master" => "▦",
            "AddressableStrip" => "✳",
            "WallPaper" => "◱",
            _ => "◈",
        }
    }
}

/// 一种灯效模式（来自 `get_QueryAllDeviceCap`）。
#[derive(Clone, Debug, PartialEq)]
pub struct LightMode {
    pub key: u32,
    pub name: String,
    pub can_color: bool,
    pub can_speed: bool,
    pub can_direction: bool,
}

/// Aura 实时状态快照。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuraSnapshot {
    pub health: Option<AuraHealth>,
    pub devices: Vec<AuraDevice>,
    pub modes: Vec<LightMode>,
    pub led_count: u32,
    /// 当前是否是独占态（独占时 Armoury Crate 自己会失联）。
    pub exclusive: bool,
    /// 当前模式名（解析自状态 XML）。
    pub active_mode: Option<String>,
    /// 当前颜色（`#RRGGBB`），状态 XML 里是 HSL。
    pub active_color: Option<String>,
    pub brightness: u8,
    pub speed: u8,
    /// 状态 XML 的 Group S0 中出现过 LED 节点的设备键。
    ///
    /// 只有这些设备能在同步组里被单独熄灭；例如本机 WallPaper 不在 Group 中，
    /// UI 会禁用它的开关并明说 Aura 没有提供独立通道。
    pub switchable_devices: std::collections::BTreeSet<String>,
    /// 采样时间（用于 UI 显示「x 秒前更新」）。
    pub sampled: Option<std::time::Instant>,
}

/// 风扇通道（只读标定数据）。
#[derive(Clone, Debug, PartialEq)]
pub struct FanChannel {
    pub index: u32,
    pub name: String,
    pub max_rpm: u32,
    pub min_duty: u32,
    pub curve: Vec<(u8, u8)>,
}

/// Fan Xpert 只读快照。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FanSnapshot {
    /// 读取失败时的原因（普通会话没有 ASUS 目录 / 文件）。
    pub error: Option<String>,
    pub channels: Vec<FanChannel>,
    pub profiles: Vec<String>,
    /// 当前生效预设（从配置读，不是设备读）。
    pub active_profile: String,
    /// 是否有权限真正下发曲线（本机：false）。
    pub can_write: bool,
    pub sampled: Option<std::time::Instant>,
}

/// 系统概览：控制面板的 4 个指标。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemSnapshot {
    pub cpu_name: String,
    pub cpu_load: f32,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub gpu_name: String,
    /// 温度读数；没有可信来源时保持 `None`，UI 显示 `n/a` 而不是 0。
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
}

/// 后台采样线程回传的一条消息。
#[derive(Clone, Debug)]
pub enum ArmouryMsg {
    Aura(AuraSnapshot),
    Fans(FanSnapshot),
    System(SystemSnapshot),
}

/// 采样线程句柄。
pub struct ArmouryWorker {
    rx: Receiver<ArmouryMsg>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 硬件页是否可见。不可见时暂停 Aura / 风扇采样，省掉跨进程 COM 调用。
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 需要立即重采一次（用户点「刷新」）。
    kick: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub snap_aura: AuraSnapshot,
    pub snap_fans: FanSnapshot,
    pub snap_system: SystemSnapshot,
}

impl ArmouryWorker {
    /// 启动采样线程。`poll_ms` 来自配置，最小 300ms。
    pub fn start(cfg: ArmouryCfg) -> Self {
        let (tx, rx) = channel::<ArmouryMsg>();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let kick = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s = stop.clone();
        let a = active.clone();
        let k = kick.clone();
        std::thread::spawn(move || {
            sample_loop(tx, s, a, k, cfg);
        });
        Self {
            rx,
            stop,
            active,
            kick,
            snap_aura: AuraSnapshot::default(),
            snap_fans: FanSnapshot::default(),
            snap_system: SystemSnapshot::default(),
        }
    }

    /// 每帧调用：把后台采到的消息合并进快照（非阻塞）。
    pub fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ArmouryMsg::Aura(s) => self.snap_aura = s,
                ArmouryMsg::Fans(s) => self.snap_fans = s,
                ArmouryMsg::System(s) => self.snap_system = s,
            }
        }
    }

    /// 请求立即重新采样（例如用户点了「刷新」）。
    pub fn request_refresh(&self) {
        self.kick.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// 硬件页可见时开启 Aura / 风扇采样。
    pub fn set_active(&self, on: bool) {
        self.active.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for ArmouryWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 采样主循环。硬件 IO 全部在这里（后台线程）。
fn sample_loop(
    tx: Sender<ArmouryMsg>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    kick: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cfg: ArmouryCfg,
) {
    let interval = Duration::from_millis(cfg.poll_ms.max(300));
    let mut aura_slow = 0u32;
    let mut last_aura: Option<AuraSnapshot> = None;
    let mut last_fans: Option<FanSnapshot> = None;
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let active_now = active.load(std::sync::atomic::Ordering::Relaxed);
        let kicked = kick.swap(false, std::sync::atomic::Ordering::Relaxed);
        crate::report::thread_state("armoury", "硬件采样（Aura / 风扇）", true);
        // 系统概览：每轮都便宜，直接刷。
        let sys = system::sample();
        if tx.send(ArmouryMsg::System(sys)).is_err() {
            break;
        }
        // 硬件页不可见：不碰 COM / 设备文件，只维持心跳。
        if !active_now && !kicked {
            crate::report::thread_state("armoury", "硬件采样（Aura / 风扇）", false);
            std::thread::sleep(interval);
            continue;
        }
        // Aura：COM 调用较重（LocalServer 跨进程），每 3 轮刷一次。
        if aura_slow % 3 == 0 || kicked {
            let snap = aura::sample(&cfg);
            if last_aura.as_ref() != Some(&snap) || aura_slow == 0 || kicked {
                last_aura = Some(snap.clone());
                if tx.send(ArmouryMsg::Aura(snap)).is_err() {
                    break;
                }
            }
        }
        aura_slow = aura_slow.wrapping_add(1);
        // 风扇：只读几个小文件，每 10 轮（约 10s）刷一次即可。
        if aura_slow % 10 == 0 || kicked {
            let fans = fan::sample(&cfg);
            if last_fans.as_ref() != Some(&fans) || kicked {
                last_fans = Some(fans.clone());
                if tx.send(ArmouryMsg::Fans(fans)).is_err() {
                    break;
                }
            }
        }
        crate::report::thread_state("armoury", "硬件采样（Aura / 风扇）", false);
        std::thread::sleep(interval);
    }
}
