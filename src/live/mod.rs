//! 电视直播：从 `ysp-live` 嵌入的频道目录、官方页面鉴权桥与播放会话。
//!
//! 主程序不直接创建第二个事件循环。Windows 下会以内部参数重新启动自身，
//! 在独立进程里运行 WebView2：官方页面完成鉴权后，清晰流交给 mpv；加密流
//! 或 mpv 不可用时，同一宿主窗口显示官方播放器。这样 egui 主循环不会被
//! WebView2 的消息泵阻塞，退出直播时也能可靠回收子进程。

mod bridge;
mod error;
mod host;
mod model;
mod session;

pub use bridge::{BridgeEvent, LiveInfo, PlaybackStatus, StreamFormat};
pub use error::CoreError;
pub use model::{validate_pid, Channel, ChannelCatalog, ChannelKind};
pub use session::{LiveController, LivePhase, LiveSnapshot};

/// 内部 WebView2 宿主参数。它不是公开 CLI，只由 `LiveController` 拉起。
pub const HOST_HELPER_ARG: &str = "--live-host-helper";

/// 在 eframe 启动前拦截内部宿主参数。返回 `Some(exit_code)` 表示当前进程
/// 应当直接退出，不能再初始化单实例锁或 GUI。
pub fn handle_internal_args(args: &[String]) -> Option<i32> {
    if !is_host_helper_invocation(args) {
        return None;
    }
    let pid = args.get(2).map(String::as_str).unwrap_or_default();
    Some(host::run_helper_cli(pid))
}

fn is_host_helper_invocation(args: &[String]) -> bool {
    args.get(1).map(String::as_str) == Some(HOST_HELPER_ARG)
}

pub fn supported() -> bool {
    cfg!(windows)
}

pub fn support_note() -> &'static str {
    if supported() {
        "Windows：WebView2 官方鉴权 + mpv / 官方播放器"
    } else {
        "电视直播目前仅支持 Windows"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_available() {
        let catalog = ChannelCatalog::bundled().expect("bundled channel catalog");
        assert_eq!(catalog.channels.len(), 64);
        assert_eq!(catalog.cctv().count(), 40);
        assert_eq!(catalog.satellite().count(), 24);
        assert_eq!(
            catalog
                .channels
                .iter()
                .filter(|channel| !channel.requires_membership())
                .count(),
            51
        );
    }

    #[test]
    fn internal_args_are_not_public() {
        let args = vec![
            "wallpaper-rs.exe".to_string(),
            HOST_HELPER_ARG.to_string(),
            "600001859".to_string(),
        ];
        assert!(is_host_helper_invocation(&args));
        assert!(!is_host_helper_invocation(&["wallpaper-rs.exe".into()]));
        assert!(!is_host_helper_invocation(&[
            "wallpaper-rs.exe".into(),
            "--tab".into(),
            "live".into(),
        ]));
    }
}
