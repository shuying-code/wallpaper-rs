//! 页面层：把「数据」画成 UI，并返回需要 `App` 代劳的动作。
//!
//! 页面只读快照、只改配置，不碰硬件、不起线程——这样即使用 `--shots`
//! 截图模式在没有 Aura 服务的机器上跑，也只会显示说明卡而不会阻塞。

pub mod hardware;
pub mod media;
pub mod sense;

use crate::armoury::{AuraDevice, AuraHealth, AuraSnapshot, FanSnapshot, LightMode};
use crate::sense::SenseStatus;

/// 页面请求 `App` 执行的动作。
#[derive(Clone, Debug, PartialEq)]
pub enum PageAction {
    /// 切换主题（立即重绘）。
    SetDark(bool),
    /// 立即落盘。
    Save,
    /// 异步下发灯效（约 10 秒，不能阻塞 UI）。
    ApplyAura,
    /// 后台重新采样硬件。
    RefreshHardware,
    /// 立即锁屏。
    LockNow,
    /// 「我还在」：重置离开计时。
    StillHere,
    /// 唤醒屏幕（务实路线的自动解锁）。
    WakeScreen,
    /// 打开授权人脸管理（目前是说明 + 占位）。
    ManageFaces,
    /// 打开 Windows 锁屏设置页。
    OpenLockSettings,
}

/// 感知页需要的全部只读数据。
#[derive(Clone, Debug, Default)]
pub struct SenseView {
    pub status: SenseStatus,
    /// 摄像头链路说明。当前版本恒为「采集后端未接入」，不把它当成可用设备。
    pub camera: Option<String>,
    /// 采集后端是否真的在出帧；只有它为 `true` 才能展示识别指标。
    pub camera_live: bool,
    /// 摄像头链路的真实状态说明（未接入 / 启动失败 / 出帧中）。
    pub camera_status: String,
    /// 采集失败的原因（来自摄像头启动错误），成功时为 `None`。
    pub camera_error: Option<String>,
    /// 系统空闲秒数（真实的退化通道）。
    pub idle_secs: Option<u64>,
    /// 最近事件（倒序）。
    pub events: Vec<(String, String)>,
    /// 今日统计。
    pub locked_today: u32,
    pub unlock_hints_today: u32,
    pub strangers_today: u32,
    pub away_today: u32,
    /// 本机是否能真正锁屏。
    pub can_lock: bool,
}

/// 硬件页需要的只读数据。
#[derive(Clone, Debug, Default)]
pub struct HardwareView {
    pub aura: AuraSnapshot,
    pub fans: FanSnapshot,
    pub system: crate::armoury::SystemSnapshot,
}

impl HardwareView {
    pub fn aura_ready(&self) -> bool {
        self.aura
            .health
            .as_ref()
            .map(AuraHealth::ok)
            .unwrap_or(false)
    }
    pub fn aura_note(&self) -> Option<String> {
        self.aura.health.as_ref().map(|h| h.summary())
    }
    pub fn devices(&self) -> &[AuraDevice] {
        &self.aura.devices
    }
    pub fn modes(&self) -> &[LightMode] {
        &self.aura.modes
    }
}

/// 统一的页面留白（内容区左右）。
pub fn gutter(narrow: bool) -> f32 {
    if narrow {
        12.0
    } else {
        18.0
    }
}
