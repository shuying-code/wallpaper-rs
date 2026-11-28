//! 摄像头采集与推理（里程碑 M2）。
//!
//! 本版**不伪装**已经能识别人脸：仓库当前没有摄像头依赖（没有 `nokhwa` /
//! `windows` Media Foundation 绑定，也没有 ONNX Runtime）。因此：
//! - `CameraWorker::start` 会如实返回 `Err(原因)`；
//! - `App` 收到 `Err` 后退化到「系统空闲检测」——这条通道是真实可用的，
//!   人走锁屏依然能工作，只是判定依据从「人脸消失」变成「无键鼠输入」；
//! - UI 必须把这两者的差别写清楚，不能让用户以为人脸识别已经在跑。

use crate::config::SenseCfg;

/// 一帧识别结果。
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    /// 已授权人脸的名字（未授权时为空）。
    pub name: String,
    pub similarity: f32,
    pub authorized: bool,
    /// 相对画面的归一化框 `(x, y, w, h)`。
    pub bbox: (f32, f32, f32, f32),
}

/// 采集线程回传的消息。
#[derive(Clone, Debug)]
pub enum CamMsg {
    /// 本帧无人脸。
    Empty,
    Hit(Detection),
    /// 采集失败（设备拔出 / 被占用）。
    Failed(String),
}

/// 采集线程句柄。当前构建不会创建实例，保留类型以便后续 M2 接入。
pub struct CameraWorker {
    pub device: String,
}

impl CameraWorker {
    /// 首版没有采集后端：启动直接返回真实原因，绝不把“没有后端”包装成成功。
    /// 调用方（`App`）据此保留 `sense_cam = None`，退化到键鼠空闲检测。
    pub fn start(_cfg: &SenseCfg) -> Result<Self, String> {
        Err(
            "摄像头采集尚未接入（里程碑 M2）；当前使用键鼠空闲检测，人脸相关开关仅保存配置"
                .to_string(),
        )
    }

    /// 当前构建没有采集线程，因此永远没有帧可轮询。
    pub fn poll(&self) -> Option<CamMsg> {
        None
    }

    pub fn stop(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_reports_missing_backend_instead_of_success() {
        match CameraWorker::start(&SenseCfg::default()) {
            Ok(_) => panic!("missing camera backend must not start successfully"),
            Err(e) => {
                assert!(e.contains("尚未接入"), "{e}");
                assert!(e.contains("键鼠空闲检测"), "{e}");
            }
        }
    }
}
