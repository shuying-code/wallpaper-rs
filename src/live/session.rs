//! 直播会话状态机：隐藏 WebView2 鉴权宿主、mpv 原生播放与官方播放器回退。
//!
//! 会话层不把播放地址写入配置、日志或界面快照。地址只存在于桥事件和
//! mpv 子进程命令行中；失败提示只包含频道与原因，不包含 token / query。

use super::bridge::{BridgeEvent, LiveInfo};
use super::host::{HostCommand, HostEvent, LiveHost};
use super::model::{Channel, ChannelCatalog};
use crate::media::Mpv;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const VOLUME_DEFAULT: f64 = 80.0;

/// 直播会话阶段。`Playing` 表示 mpv 原生窗口，`Official` 表示 WebView2
/// 官方播放器。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LivePhase {
    #[default]
    Idle,
    Authenticating,
    StartingPlayer,
    Playing,
    Official,
    Stopped,
    Failed,
}

impl LivePhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "待播放",
            Self::Authenticating => "正在连接官方页面",
            Self::StartingPlayer => "正在启动播放器",
            Self::Playing => "原生播放中",
            Self::Official => "官方播放器中",
            Self::Stopped => "已停止",
            Self::Failed => "播放失败",
        }
    }

    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Authenticating | Self::StartingPlayer | Self::Playing | Self::Official
        )
    }
}

/// UI 只读快照，避免页面持有宿主或 mpv 的可变引用。
#[derive(Clone, Debug, Default)]
pub struct LiveSnapshot {
    pub phase: LivePhase,
    pub channel: Option<Channel>,
    pub definition: Option<String>,
    pub volume: f64,
    pub muted: bool,
    pub fullscreen: bool,
    pub host_ready: bool,
    pub supported: bool,
    pub elapsed: Option<Duration>,
    pub error: Option<String>,
    pub route_note: Option<String>,
    pub catalog_error: Option<String>,
}

impl LiveSnapshot {
    pub fn active(&self) -> bool {
        self.phase.active()
    }

    pub fn can_change_window(&self) -> bool {
        matches!(self.phase, LivePhase::Playing | LivePhase::Official)
    }
}

pub struct LiveController {
    catalog: ChannelCatalog,
    catalog_error: Option<String>,
    selected: Option<Channel>,
    phase: LivePhase,
    host: Option<LiveHost>,
    host_ready: bool,
    mpv: Option<Mpv>,
    mpv_start: Option<Receiver<Result<Mpv, String>>>,
    definition: Option<String>,
    volume: f64,
    muted: bool,
    native_fullscreen: bool,
    official_fullscreen: bool,
    started_at: Option<Instant>,
    error: Option<String>,
    route_note: Option<String>,
}

impl Default for LiveController {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveController {
    pub fn new() -> Self {
        let (catalog, catalog_error) = match ChannelCatalog::bundled() {
            Ok(catalog) => (catalog, None),
            Err(error) => (ChannelCatalog::default(), Some(error.to_string())),
        };
        Self {
            catalog,
            catalog_error,
            selected: None,
            phase: LivePhase::Idle,
            host: None,
            host_ready: false,
            mpv: None,
            mpv_start: None,
            definition: None,
            volume: VOLUME_DEFAULT,
            muted: false,
            native_fullscreen: false,
            official_fullscreen: true,
            started_at: None,
            error: None,
            route_note: None,
        }
    }

    pub fn channels(&self) -> &[Channel] {
        &self.catalog.channels
    }

    pub fn channel(&self, pid: &str) -> Option<&Channel> {
        self.catalog.find_pid(pid)
    }

    pub fn snapshot(&self) -> LiveSnapshot {
        LiveSnapshot {
            phase: self.phase,
            channel: self.selected.clone(),
            definition: self.definition.clone(),
            volume: self.volume,
            muted: self.muted,
            fullscreen: match self.phase {
                LivePhase::Playing => self.native_fullscreen,
                LivePhase::Official => self.official_fullscreen,
                _ => false,
            },
            host_ready: self.host_ready,
            supported: super::supported(),
            elapsed: self.started_at.map(|started| started.elapsed()),
            error: self.error.clone(),
            route_note: self.route_note.clone(),
            catalog_error: self.catalog_error.clone(),
        }
    }

    /// 开始频道。会先结束上一路直播，确保同一时间只有一个鉴权宿主和一个
    /// 直播 mpv，避免两个声音源重叠。
    pub fn play(&mut self, pid: &str) {
        if !super::supported() {
            self.fail_with("电视直播目前仅支持 Windows（需要 WebView2）".to_string());
            return;
        }
        let Some(channel) = self.catalog.find_pid(pid).cloned() else {
            self.fail_with("频道不存在或目录已损坏".to_string());
            return;
        };
        if channel.requires_membership() {
            self.fail_with("该频道需要会员，应用不绕过付费限制".to_string());
            return;
        }

        self.end_session(false);
        self.selected = Some(channel.clone());
        self.phase = LivePhase::Authenticating;
        self.host_ready = false;
        self.definition = None;
        self.started_at = None;
        self.error = None;
        self.route_note = Some("正在由官方页面完成鉴权，请稍候".to_string());

        match LiveHost::spawn(&channel.pid) {
            Ok(host) => self.host = Some(host),
            Err(error) => {
                self.fail_with(format!("无法启动直播宿主：{error}"));
            }
        }
    }

    pub fn stop(&mut self) {
        self.end_session(true);
        self.error = None;
        self.route_note = None;
    }

    /// 每帧调用。宿主事件与 mpv 启动都在后台，本函数只做非阻塞收取。
    pub fn tick(&mut self) {
        let mut events = Vec::new();
        if let Some(host) = self.host.as_mut() {
            while let Some(event) = host.try_recv() {
                events.push(event);
                if events.len() >= 64 {
                    break;
                }
            }
            if !host.is_alive() && events.is_empty() {
                events.push(HostEvent::Closed);
            }
        }
        for event in events {
            self.handle_host_event(event);
            if self.host.is_none() {
                break;
            }
        }

        let mut start_result = None;
        if let Some(rx) = self.mpv_start.as_ref() {
            match rx.try_recv() {
                Ok(result) => start_result = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    start_result = Some(Err("直播播放器线程意外结束".to_string()))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(result) = start_result {
            self.mpv_start = None;
            match result {
                Ok(mpv) => {
                    if self.phase == LivePhase::StartingPlayer && self.host.is_some() {
                        self.mpv = Some(mpv);
                        self.phase = LivePhase::Playing;
                        self.started_at = Some(Instant::now());
                        if self.native_fullscreen {
                            if let Some(mpv) = self.mpv.as_mut() {
                                let _ = mpv.set_fullscreen(true);
                            }
                        }
                    }
                }
                Err(error) => {
                    self.fallback_official(format!("原生播放器不可用，已改用官方播放器：{error}"));
                }
            }
        }

        self.poll_mpv();
    }

    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 100.0);
        if let Some(mpv) = self.mpv.as_mut() {
            let _ = mpv.set_volume(self.volume);
        }
        if let Some(host) = self.host.as_mut() {
            let _ = host.send(HostCommand::Volume(self.volume));
        }
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if let Some(mpv) = self.mpv.as_mut() {
            let _ = mpv.set_muted(self.muted);
        }
        if let Some(host) = self.host.as_mut() {
            let _ = host.send(HostCommand::Mute(self.muted));
        }
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        match self.phase {
            LivePhase::Playing => {
                self.native_fullscreen = fullscreen;
                if let Some(mpv) = self.mpv.as_mut() {
                    let _ = mpv.set_fullscreen(fullscreen);
                }
            }
            LivePhase::StartingPlayer => {
                self.native_fullscreen = fullscreen;
            }
            LivePhase::Official => {
                if let Some(host) = self.host.as_mut() {
                    let command = if fullscreen {
                        HostCommand::Fullscreen
                    } else {
                        HostCommand::Windowed
                    };
                    if host.send(command).is_ok() {
                        self.official_fullscreen = fullscreen;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_host_event(&mut self, event: HostEvent) {
        match event {
            HostEvent::Ready => {
                self.host_ready = true;
                if self.phase == LivePhase::Authenticating {
                    self.route_note = Some("官方页面已就绪，正在等待直播流".to_string());
                }
            }
            HostEvent::Bridge(BridgeEvent::LiveInfo(info)) => {
                // 官方页面可能重复上报同一路流。只在首次鉴权阶段接受，
                // 避免已经在 mpv / 官方播放器里的会话被重启。
                if self.phase == LivePhase::Authenticating {
                    self.handle_live_info(info);
                }
            }
            HostEvent::Bridge(BridgeEvent::Playback(status)) => {
                if status.kind == "playback_error" {
                    if let Some(message) = status.message {
                        self.error = Some(format!("官方播放器提示：{message}"));
                    }
                }
            }
            HostEvent::Bridge(BridgeEvent::Error { message, .. }) => {
                self.error = Some(format!("官方页面：{message}"));
            }
            HostEvent::Bridge(BridgeEvent::Ready { .. })
            | HostEvent::Bridge(BridgeEvent::Ignored { .. }) => {}
            HostEvent::Official { fullscreen } => {
                self.official_fullscreen = fullscreen;
                if self.phase != LivePhase::Failed {
                    self.phase = LivePhase::Official;
                }
            }
            HostEvent::Error(message) => {
                self.fail_with(message);
            }
            HostEvent::Closed => {
                let was_failed = self.phase == LivePhase::Failed;
                self.end_session(false);
                if was_failed {
                    self.phase = LivePhase::Failed;
                } else {
                    self.phase = LivePhase::Stopped;
                    self.route_note = Some("播放器窗口已关闭".to_string());
                }
            }
        }
    }

    fn handle_live_info(&mut self, info: LiveInfo) {
        let Some(channel) = self.selected.as_ref() else {
            return;
        };
        match route_decision(&info, &channel.pid) {
            RouteDecision::Ignore => {}
            RouteDecision::Insecure => {
                self.fallback_official("官方接口返回的地址不是 HTTPS，已拒绝交给 mpv".to_string());
            }
            RouteDecision::Official => {
                self.definition = definition_label(&info);
                self.fallback_official("该频道由官方加密播放器播放".to_string());
            }
            RouteDecision::Native => {
                self.start_native(info, channel.name.clone());
            }
        }
    }

    fn start_native(&mut self, info: LiveInfo, title: String) {
        let Some(host) = self.host.as_mut() else {
            self.fail_with("直播宿主已结束，无法继续播放".to_string());
            return;
        };
        let _ = host.send(HostCommand::Native);
        self.definition = definition_label(&info);
        self.phase = LivePhase::StartingPlayer;
        self.started_at = Some(Instant::now());
        self.error = None;
        self.route_note = Some("清晰流交给独立 mpv 窗口播放".to_string());

        let url = info.play_url.as_str().to_string();
        let volume = self.volume;
        let muted = self.muted;
        let (tx, rx) = mpsc::channel();
        self.mpv_start = Some(rx);
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut mpv = Mpv::start_live(&url, &title, volume, muted)?;
                // 给 mpv 一小段时间报告“立即退出”。HLS 正常加载时进程会
                // 持续存活，不需要等视频真正出帧。
                while started.elapsed() < Duration::from_millis(180) {
                    std::thread::sleep(Duration::from_millis(30));
                }
                mpv.poll();
                if !mpv.is_running() {
                    return Err("mpv 启动后立即退出（可能被上游拒绝）".to_string());
                }
                if mpv.state.ended {
                    return Err("mpv 未能打开直播流".to_string());
                }
                Ok(mpv)
            }))
            .unwrap_or_else(|_| Err("直播播放器初始化异常".to_string()));
            let _ = tx.send(result);
        });
    }

    fn fallback_official(&mut self, note: String) {
        self.mpv_start = None;
        self.mpv = None;
        let Some(host) = self.host.as_mut() else {
            self.fail_with("直播宿主已结束，无法打开官方播放器".to_string());
            return;
        };
        let fullscreen = self.official_fullscreen;
        if let Err(error) = host.send(HostCommand::Official { fullscreen }) {
            self.fail_with(error);
            return;
        }
        self.phase = LivePhase::Official;
        self.started_at.get_or_insert_with(Instant::now);
        self.error = None;
        self.route_note = Some(note);
        let _ = host.send(HostCommand::Volume(self.volume));
        let _ = host.send(HostCommand::Mute(self.muted));
    }

    fn poll_mpv(&mut self) {
        let Some(mpv) = self.mpv.as_mut() else {
            return;
        };
        mpv.poll();
        let running = mpv.is_running();
        let ended = mpv.state.ended;
        if running {
            return;
        }
        self.mpv = None;
        if self.phase != LivePhase::Playing {
            return;
        }
        if ended {
            self.fallback_official("原生直播流已结束，已改用官方播放器".to_string());
        } else {
            self.end_session(false);
            self.phase = LivePhase::Stopped;
            self.route_note = Some("播放器窗口已关闭，本次直播已结束".to_string());
        }
    }

    fn fail_with(&mut self, message: String) {
        self.end_session(false);
        self.phase = LivePhase::Failed;
        self.error = Some(message);
        self.route_note = None;
    }

    /// `mark_stopped`：用户主动停止时保留频道并显示“已停止”；内部切台时
    /// 回到 Idle，随后由 `play` 设置新状态。
    fn end_session(&mut self, mark_stopped: bool) {
        self.mpv_start = None;
        self.mpv = None;
        if let Some(mut host) = self.host.take() {
            host.stop();
        }
        self.host_ready = false;
        self.native_fullscreen = false;
        self.official_fullscreen = true;
        self.started_at = None;
        if mark_stopped {
            self.phase = LivePhase::Stopped;
        } else if self.phase != LivePhase::Failed {
            self.phase = LivePhase::Idle;
        }
    }
}

impl Drop for LiveController {
    fn drop(&mut self) {
        self.mpv_start = None;
        self.mpv = None;
        if let Some(mut host) = self.host.take() {
            host.stop();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteDecision {
    Ignore,
    Insecure,
    Official,
    Native,
}

fn route_decision(info: &LiveInfo, selected_pid: &str) -> RouteDecision {
    if info
        .live_pid
        .as_deref()
        .is_some_and(|pid| pid != selected_pid)
    {
        return RouteDecision::Ignore;
    }
    if !info.play_url.is_https() {
        return RouteDecision::Insecure;
    }
    if info.encrypted() {
        RouteDecision::Official
    } else {
        RouteDecision::Native
    }
}

fn definition_label(info: &LiveInfo) -> Option<String> {
    let definition = info.definition.as_deref()?;
    info.formats
        .iter()
        .find(|format| format.fn_name.as_deref() == Some(definition))
        .map(|format| format.label())
        .or_else(|| Some(definition.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::bridge::{PlayUrl, StreamFormat};

    fn info(url: &str, encrypt: Option<i64>, live_pid: Option<&str>) -> LiveInfo {
        LiveInfo {
            play_url: PlayUrl::parse(url).unwrap(),
            definition: Some("fhd".into()),
            formats: vec![StreamFormat {
                id: Some(1),
                encrypt,
                fn_name: Some("fhd".into()),
                fnname: Some("蓝光 1080P".into()),
                defnname: None,
                defnrate: Some("1080P".into()),
            }],
            live_pid: live_pid.map(str::to_string),
            encrypt,
        }
    }

    #[test]
    fn routes_clear_https_stream_to_native_player() {
        let info = info(
            "https://live.example.cctv.cn/a/channel.m3u8?token=secret",
            Some(0),
            Some("600152137"),
        );
        assert_eq!(route_decision(&info, "600152137"), RouteDecision::Native);
    }

    #[test]
    fn routes_encrypted_stream_to_official_player() {
        let info = info(
            "https://live.example.cctv.cn/a/channel.m3u8?token=secret",
            Some(2),
            Some("600001859"),
        );
        assert_eq!(route_decision(&info, "600001859"), RouteDecision::Official);
    }

    #[test]
    fn rejects_http_even_if_bridge_parser_accepts_it() {
        let info = info("http://live.example.cctv.cn/a/channel.m3u8", Some(0), None);
        assert_eq!(route_decision(&info, "600001859"), RouteDecision::Insecure);
    }

    #[test]
    fn ignores_stale_channel_callback() {
        let info = info(
            "https://live.example.cctv.cn/a/channel.m3u8",
            Some(0),
            Some("other"),
        );
        assert_eq!(route_decision(&info, "600001859"), RouteDecision::Ignore);
    }

    #[test]
    fn definition_uses_human_readable_label() {
        let info = info("https://live.example.cctv.cn/a/channel.m3u8", Some(0), None);
        assert_eq!(definition_label(&info).as_deref(), Some("蓝光 1080P"));
    }

    #[test]
    fn controller_snapshot_has_bundled_catalog() {
        let controller = LiveController::new();
        assert_eq!(controller.channels().len(), 64);
        assert_eq!(
            controller
                .channels()
                .iter()
                .filter(|channel| !channel.requires_membership())
                .count(),
            51
        );
        assert_eq!(controller.snapshot().volume, VOLUME_DEFAULT);
    }
}
