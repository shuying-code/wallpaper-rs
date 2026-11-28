//! Hidden WebView2 host process and the small JSON-Lines protocol it shares
//! with the main wallpaper-rs process.

use super::{BridgeEvent, ChannelCatalog};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use url::Url;

#[derive(Debug)]
pub(crate) enum HostEvent {
    Ready,
    Bridge(BridgeEvent),
    Official { fullscreen: bool },
    Error(String),
    Closed,
}

#[derive(Debug)]
pub(crate) enum HostCommand {
    Native,
    Official { fullscreen: bool },
    Fullscreen,
    Windowed,
    Volume(f64),
    Mute(bool),
    Stop,
}

pub(crate) struct LiveHost {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<HostEvent>,
    closed: bool,
}

impl LiveHost {
    pub(crate) fn spawn(pid: &str) -> Result<Self, String> {
        super::validate_pid(pid).map_err(|error| error.to_string())?;
        let executable =
            std::env::current_exe().map_err(|error| format!("定位直播宿主程序失败：{error}"))?;
        let mut command = Command::new(executable);
        command
            .arg(super::HOST_HELPER_ARG)
            .arg(pid)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("启动直播宿主失败：{error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "直播宿主 stdin 不可用".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "直播宿主 stdout 不可用".to_string())?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                match parse_host_line(&line) {
                    Some(event) => {
                        if tx.send(event).is_err() {
                            break;
                        }
                    }
                    None => {
                        let _ =
                            tx.send(HostEvent::Error("直播宿主返回了无法识别的消息".to_string()));
                    }
                }
            }
            let _ = tx.send(HostEvent::Closed);
        });
        Ok(Self {
            child,
            stdin,
            rx,
            closed: false,
        })
    }

    pub(crate) fn send(&mut self, command: HostCommand) -> Result<(), String> {
        if self.closed {
            return Err("直播宿主已结束".to_string());
        }
        let value = match command {
            HostCommand::Native => serde_json::json!({ "cmd": "native" }),
            HostCommand::Official { fullscreen } => {
                serde_json::json!({ "cmd": "official", "fullscreen": fullscreen })
            }
            HostCommand::Fullscreen => serde_json::json!({ "cmd": "fullscreen" }),
            HostCommand::Windowed => serde_json::json!({ "cmd": "windowed" }),
            HostCommand::Volume(volume) => {
                serde_json::json!({ "cmd": "volume", "volume": volume })
            }
            HostCommand::Mute(mute) => serde_json::json!({ "cmd": "mute", "mute": mute }),
            HostCommand::Stop => serde_json::json!({ "cmd": "stop" }),
        };
        writeln!(self.stdin, "{value}")
            .and_then(|_| self.stdin.flush())
            .map_err(|error| format!("向直播宿主发送命令失败：{error}"))
    }

    pub(crate) fn try_recv(&self) -> Option<HostEvent> {
        self.rx.try_recv().ok()
    }

    pub(crate) fn is_alive(&mut self) -> bool {
        if self.closed {
            return false;
        }
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                self.closed = true;
                false
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.send(HostCommand::Stop);
        self.closed = true;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(450);
        while std::time::Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for LiveHost {
    fn drop(&mut self) {
        self.stop();
    }
}

fn parse_host_line(line: &str) -> Option<HostEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    match value.get("type")?.as_str()? {
        "host_ready" => Some(HostEvent::Ready),
        "host_official" => Some(HostEvent::Official {
            fullscreen: value
                .get("fullscreen")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        }),
        "host_error" => Some(HostEvent::Error(
            value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("直播宿主未知错误")
                .to_string(),
        )),
        "host_closed" => Some(HostEvent::Closed),
        "bridge" => {
            let payload = value.get("payload")?;
            let raw = serde_json::to_string(payload).ok()?;
            BridgeEvent::from_ipc(&raw).ok().map(HostEvent::Bridge)
        }
        _ => None,
    }
}

#[cfg(windows)]
pub(crate) use windows_impl::run_helper_cli;

#[cfg(not(windows))]
pub(crate) fn run_helper_cli(_pid: &str) -> i32 {
    eprintln!("电视直播的 WebView2 宿主仅支持 Windows");
    2
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tao::dpi::{LogicalSize, PhysicalSize};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::window::{Fullscreen, WindowBuilder};
    use wry::WebViewBuilder;

    const AUTH_TIMEOUT: Duration = Duration::from_secs(60);
    const POLL_INTERVAL: Duration = Duration::from_millis(40);

    #[derive(Debug, Clone)]
    enum HelperCommand {
        Native,
        Official { fullscreen: bool },
        Fullscreen,
        Windowed,
        Volume(f64),
        Mute(bool),
        Stop,
    }

    pub(crate) fn run_helper_cli(pid: &str) -> i32 {
        match run(pid) {
            Ok(()) => 0,
            Err(error) => {
                emit(&serde_json::json!({
                    "type": "host_error",
                    "message": error,
                }));
                1
            }
        }
    }

    fn run(pid: &str) -> Result<(), String> {
        let catalog =
            ChannelCatalog::bundled().map_err(|error| format!("读取内置频道失败：{error}"))?;
        let channel = catalog
            .find_pid(pid)
            .ok_or_else(|| "未知频道 PID".to_string())?;
        let page_url = channel
            .page_url()
            .map_err(|error| format!("生成官方页面地址失败：{error}"))?;

        let event_loop = EventLoopBuilder::<HelperCommand>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let command = match value.get("cmd").and_then(serde_json::Value::as_str) {
                    Some("native") => Some(HelperCommand::Native),
                    Some("official") => Some(HelperCommand::Official {
                        fullscreen: value
                            .get("fullscreen")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true),
                    }),
                    Some("fullscreen") => Some(HelperCommand::Fullscreen),
                    Some("windowed") => Some(HelperCommand::Windowed),
                    Some("volume") => Some(HelperCommand::Volume(
                        value
                            .get("volume")
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(80.0),
                    )),
                    Some("mute") => Some(HelperCommand::Mute(
                        value
                            .get("mute")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                    )),
                    Some("stop") => Some(HelperCommand::Stop),
                    _ => None,
                };
                if let Some(command) = command {
                    if proxy.send_event(command).is_err() {
                        break;
                    }
                }
            }
        });

        let window = WindowBuilder::new()
            .with_title("Wallpaper Studio · 电视直播")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_visible(false)
            .with_decorations(true)
            .build(&event_loop)
            .map_err(|error| format!("创建直播窗口失败：{error}"))?;

        let bridge_events = Arc::new(Mutex::new(VecDeque::<String>::new()));
        let handler_events = Arc::clone(&bridge_events);
        let ipc = move |request: wry::http::Request<String>| {
            let raw = request.body();
            if raw.len() <= 64 * 1024 && BridgeEvent::from_ipc(raw).is_ok() {
                if let Ok(mut events) = handler_events.lock() {
                    if events.len() < 128 {
                        events.push_back(raw.clone());
                    }
                }
            }
        };

        let data_dir = webview_data_dir()?;
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| format!("创建 WebView2 数据目录失败：{error}"))?;
        let mut web_context = wry::WebContext::new(Some(data_dir));
        let webview = WebViewBuilder::new_with_web_context(&mut web_context)
            .with_url(&page_url)
            .with_autoplay(true)
            .with_initialization_script(include_str!("../../assets/live/auth_bridge.js"))
            .with_navigation_handler(|url| navigation_allowed(&url))
            .with_ipc_handler(ipc)
            .build(&window)
            .map_err(|error| format!("创建 WebView2 失败：{error}"))?;

        emit(&serde_json::json!({ "type": "host_ready" }));
        let mut got_info = false;
        let mut official = false;
        let deadline = Instant::now() + AUTH_TIMEOUT;
        let mut exiting = false;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + POLL_INTERVAL);

            if !got_info && Instant::now() >= deadline {
                emit(&serde_json::json!({
                    "type": "host_error",
                    "message": "官方页面在 60 秒内未返回可用直播流",
                }));
                *control_flow = ControlFlow::Exit;
                return;
            }

            let mut pending = Vec::new();
            if let Ok(mut events) = bridge_events.lock() {
                while let Some(raw) = events.pop_front() {
                    pending.push(raw);
                }
            }
            for raw in pending {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                    continue;
                };
                let kind = value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if kind == "live_info" {
                    got_info = true;
                    pause_official(&webview);
                }
                emit(&serde_json::json!({ "type": "bridge", "payload": value }));
            }

            match event {
                Event::UserEvent(command) => match command {
                    HelperCommand::Native => {
                        official = false;
                        window.set_fullscreen(None);
                        window.set_decorations(true);
                        window.set_visible(false);
                        pause_official(&webview);
                    }
                    HelperCommand::Official { fullscreen: want } => {
                        official = true;
                        show_official(&window, &webview, want);
                        emit(&serde_json::json!({
                            "type": "host_official",
                            "fullscreen": want,
                        }));
                    }
                    HelperCommand::Fullscreen if official => {
                        show_official(&window, &webview, true);
                        emit(&serde_json::json!({
                            "type": "host_official",
                            "fullscreen": true,
                        }));
                    }
                    HelperCommand::Windowed if official => {
                        show_official(&window, &webview, false);
                        emit(&serde_json::json!({
                            "type": "host_official",
                            "fullscreen": false,
                        }));
                    }
                    HelperCommand::Volume(volume) => {
                        let volume = volume.clamp(0.0, 100.0) / 100.0;
                        let script = format!(
                            "document.querySelectorAll('video,audio').forEach(function(v){{\
                             try{{v.volume={volume:.3};}}catch(_){{}}}});"
                        );
                        let _ = webview.evaluate_script(&script);
                    }
                    HelperCommand::Mute(mute) => {
                        let script = format!(
                            "document.querySelectorAll('video,audio').forEach(function(v){{\
                             try{{v.muted={};}}catch(_){{}}}});",
                            if mute { "true" } else { "false" }
                        );
                        let _ = webview.evaluate_script(&script);
                    }
                    HelperCommand::Stop => {
                        exiting = true;
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                },
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    if !exiting {
                        exiting = true;
                        emit(&serde_json::json!({ "type": "host_closed" }));
                    }
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }

    fn webview_data_dir() -> Result<PathBuf, String> {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .map(|base| base.join("wallpaper-rs").join("webview2"))
            .ok_or_else(|| "无法确定 WebView2 数据目录".to_string())
    }

    fn pause_official(webview: &wry::WebView) {
        let script = "window.__yspSetNativeMode&&window.__yspSetNativeMode(true);\
                      window.__yspPauseOfficialVideo&&window.__yspPauseOfficialVideo();";
        let _ = webview.evaluate_script(script);
    }

    fn show_official(window: &tao::window::Window, webview: &wry::WebView, fullscreen: bool) {
        window.set_visible(true);
        if fullscreen {
            window.set_decorations(false);
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            window.set_fullscreen(None);
            window.set_decorations(true);
            window.set_inner_size(PhysicalSize::new(1280, 720));
        }
        window.set_focus();
        let script = "window.__yspSetNativeMode&&window.__yspSetNativeMode(false);\
                      window.__yspUseOfficialPlayer&&window.__yspUseOfficialPlayer();\
                      window.__yspFocusOfficialVideo&&window.__yspFocusOfficialVideo();";
        let _ = webview.evaluate_script(script);
    }

    fn emit(value: &serde_json::Value) {
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{value}");
        let _ = stdout.flush();
    }
}

/// 官方页面鉴权只允许在电视业务域名内导航，避免被页面脚本带到任意
/// 第三方站点（包括 `yangshipin.cn.evil.example` 这类后缀伪装）。
pub(crate) fn navigation_allowed(raw: &str) -> bool {
    if raw.eq_ignore_ascii_case("about:blank") {
        return true;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    const ALLOWED_ROOTS: [&str; 4] = ["yangshipin.cn", "cctv.com", "cctv.cn", "cntv.cn"];
    ALLOWED_ROOTS
        .iter()
        .any(|root| host == *root || host.ends_with(&format!(".{root}")))
}

#[cfg(test)]
mod tests {
    use super::navigation_allowed;

    #[test]
    fn navigation_is_limited_to_official_television_domains() {
        for url in [
            "https://yangshipin.cn/tv/home?pid=600001859",
            "https://www.yangshipin.cn/",
            "https://passport.cctv.com/login",
            "https://tv.cctv.cn/live",
        ] {
            assert!(navigation_allowed(url), "{url}");
        }
        assert!(navigation_allowed("about:blank"));
        for url in [
            "http://yangshipin.cn/tv/home",
            "https://yangshipin.cn.evil.example/tv",
            "https://evil.example/?next=yangshipin.cn",
            "javascript:alert(1)",
            "file:///C:/Windows/win.ini",
        ] {
            assert!(!navigation_allowed(url), "{url}");
        }
    }
}
