//! 摄像头采集与推理（里程碑 M2）。
//!
//! 本版**不伪装**已经能识别人脸：仓库当前没有摄像头依赖（没有 `nokhwa` /
//! `windows` Media Foundation 绑定，也没有 ONNX Runtime）。因此：
//! - `CameraWorker::start` 会如实返回 `Err(原因)`；
//! - `App` 收到 `Err` 后退化到「系统空闲检测」——这条通道是真实可用的，
//!   人走锁屏依然能工作，只是判定依据从「人脸消失」变成「无键鼠输入」；
//! - UI 必须把这两者的差别写清楚，不能让用户以为人脸识别已经在跑。

use crate::config::SenseCfg;
use std::sync::mpsc::{channel, Receiver};

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

/// 摄像头线程。
pub struct CameraWorker {
    pub device: String,
    rx: Receiver<CamMsg>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CameraWorker {
    /// 首版恒定失败：没有采集后端。失败原因直接展示到 UI。
    pub fn start(_cfg: &SenseCfg) -> Result<Self, String> {
        let (tx, rx) = channel::<CamMsg>();
        // 立刻回报一次失败，让 UI 的第一帧就有真实状态。
        let _ = tx.send(CamMsg::Failed(
            "本版未编译摄像头采集后端（需 Media Foundation + ONNX），已退化到系统空闲检测"
                .to_string(),
        ));
        Ok(Self {
            device: "CAM 01".to_string(),
            rx,
            stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    pub fn poll(&self) -> Option<CamMsg> {
        self.rx.try_recv().ok()
    }

    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
