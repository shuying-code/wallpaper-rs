//! 硬件域四页：控制面板 / 设备 / 灯效 & Aura Sync / Fan Xpert。

pub mod dashboard;
pub mod devices;
pub mod fan;
pub mod lighting;

use crate::config::ArmouryCfg;
use crate::pages::{HardwareView, PageAction};
use crate::theme;
use eframe::egui;

/// 四页共用的上下文（借用，便于同时传配置与动作队列）。
pub struct HwCtx<'a> {
    pub view: &'a HardwareView,
    pub cfg: &'a mut ArmouryCfg,
    pub actions: &'a mut Vec<PageAction>,
    pub compact: bool,
    /// 灯效下发中（约 10 秒）。
    pub applying: bool,
    /// 最近一次下发结果。
    pub apply_msg: Option<(bool, String)>,
}

/// 移动端 / 窄窗的硬件页说明卡。
///
/// 用户明确要求手机端不做硬件四页。这里如实说明原因（通道不可达），
/// 而不是灰掉假装存在。
pub fn mobile_notice(ui: &mut egui::Ui, tab: crate::app::Tab) {
    let (title, body) = match tab {
        crate::app::Tab::Lighting => (
            "灯效 & Aura Sync 仅在桌面端提供",
            "Aura 是 Windows LocalServer32 COM（LightingService.exe），手机进程内没有这条通道；\
             远程遥控机箱灯需要另建配对与鉴权，属于另一个产品。",
        ),
        crate::app::Tab::Fan => (
            "Fan Xpert 仅在桌面端提供",
            "风扇通道是 \\\\.\\Asusgio3 内核设备，SDDL 只允许 SYSTEM / Administrators；\
             手机端既无驱动也无提权入口。",
        ),
        crate::app::Tab::Dashboard => (
            "控制面板仅在桌面端提供",
            "温度 / 转速来自主板驱动与 ASUS 服务，手机端读不到这些传感器。",
        ),
        _ => (
            "设备清单仅在桌面端提供",
            "Aura 设备枚举依赖本机 COM 服务；手机端没有可枚举的灯效设备。",
        ),
    };
    ui.label(
        egui::RichText::new(title)
            .size(15.0)
            .strong()
            .color(theme::text()),
    );
    ui.add_space(6.0);
    ui.label(egui::RichText::new(body).size(11.5).color(theme::subtext()));
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(
            "移动端只保留「壁纸 / 感知 / 策略 / 设置」四页，硬件控制请在桌面端操作。",
        )
        .size(10.5)
        .color(theme::faint()),
    );
}
