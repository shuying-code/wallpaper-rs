//! 音乐 / 视频媒体库与 mpv 播放控制。
//!
//! 这里刻意不引入新的解码依赖：项目已经为动态壁纸准备了 mpv，播放器和
//! 媒体库都复用它。扫描只在后台线程做文件枚举，UI 线程只消费结果；播放
//! 则通过 mpv 的 JSON IPC 通道发送命令、读取进度，避免卡住 egui 的事件循环。

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 一次性载入的媒体文件上限，避免超大音乐目录把内存撑爆。
pub const MEDIA_CAP: usize = 4000;

const AUDIO_EXT: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "aiff", "aif", "ape", "wv", "mka",
];
const VIDEO_EXT: &[&str] = &[
    "mp4", "webm", "mkv", "mov", "m4v", "avi", "wmv", "flv", "ts", "mpg", "mpeg", "ogv", "3gp",
];

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// 媒体种类。音乐页只列音频，视频页只列视频；两者共用同一套播放控制。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Video,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Audio => "音乐",
            Kind::Video => "视频",
        }
    }
}

/// 媒体库中的一个文件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    /// 所在目录，列表里作为副标题显示。
    pub folder: String,
    pub kind: Kind,
    /// 文件大小（字节），用于排序 / 说明。
    pub bytes: u64,
}

impl Track {
    fn new(path: PathBuf, kind: Kind) -> Self {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let folder = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "媒体".to_string());
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            path,
            title,
            folder,
            kind,
            bytes,
        }
    }
}

/// 扫描结果：每个种类单独返回，页面切换时互不影响。
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub audio: Vec<Track>,
    pub video: Vec<Track>,
    pub scanned_dirs: Vec<PathBuf>,
}

impl Library {
    pub fn tracks(&self, kind: Kind) -> &[Track] {
        match kind {
            Kind::Audio => &self.audio,
            Kind::Video => &self.video,
        }
    }

    pub fn position(&self, kind: Kind, id: u64) -> Option<usize> {
        self.tracks(kind).iter().position(|t| t.id == id)
    }
}

/// 后台扫描。空目录列表时返回空库（页面据此提示「添加文件夹」）。
pub fn scan_async(dirs: Vec<PathBuf>) -> Receiver<Library> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan_blocking(dirs, None));
    });
    rx
}

/// 只扫描指定种类的媒体，音乐 / 视频各自维护目录时互不串库。
pub fn scan_async_kind(dirs: Vec<PathBuf>, kind: Kind) -> Receiver<Library> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan_blocking(dirs, Some(kind)));
    });
    rx
}

/// 一次构建两个库：音频只走音乐目录，视频只走视频目录，互不串库。
pub fn scan_libraries_async(
    audio_dirs: Vec<PathBuf>,
    video_dirs: Vec<PathBuf>,
) -> Receiver<Library> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan_libraries_blocking(audio_dirs, video_dirs));
    });
    rx
}

/// 阻塞版本，供已经在后台线程中的调用方复用。
pub fn scan_blocking(dirs: Vec<PathBuf>, only: Option<Kind>) -> Library {
    let mut lib = Library::default();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        lib.scanned_dirs.push(dir.clone());
        if lib.audio.len() >= MEDIA_CAP && lib.video.len() >= MEDIA_CAP {
            break;
        }
        walk(&dir, &mut lib, 0, only);
    }
    sort_tracks(&mut lib.audio);
    sort_tracks(&mut lib.video);
    lib.audio.truncate(MEDIA_CAP);
    lib.video.truncate(MEDIA_CAP);
    lib
}

pub fn scan_libraries_blocking(audio_dirs: Vec<PathBuf>, video_dirs: Vec<PathBuf>) -> Library {
    let mut lib = Library::default();
    for dir in audio_dirs {
        if !dir.is_dir() {
            continue;
        }
        lib.scanned_dirs.push(dir.clone());
        if lib.audio.len() >= MEDIA_CAP {
            break;
        }
        walk(&dir, &mut lib, 0, Some(Kind::Audio));
    }
    for dir in video_dirs {
        if !dir.is_dir() {
            continue;
        }
        lib.scanned_dirs.push(dir.clone());
        if lib.video.len() >= MEDIA_CAP {
            break;
        }
        walk(&dir, &mut lib, 0, Some(Kind::Video));
    }
    sort_tracks(&mut lib.audio);
    sort_tracks(&mut lib.video);
    lib.audio.truncate(MEDIA_CAP);
    lib.video.truncate(MEDIA_CAP);
    lib
}

fn sort_tracks(list: &mut [Track]) {
    list.sort_by(|a, b| {
        a.folder
            .to_ascii_lowercase()
            .cmp(&b.folder.to_ascii_lowercase())
            .then_with(|| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            })
    });
}

fn walk(dir: &Path, lib: &mut Library, depth: usize, only: Option<Kind>) {
    if depth > 24 {
        return;
    }
    let Ok(reader) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in reader.flatten() {
        if lib.audio.len() >= MEDIA_CAP && lib.video.len() >= MEDIA_CAP {
            return;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if is_skip_dir(&path) {
                continue;
            }
            walk(&path, lib, depth + 1, only);
            continue;
        }
        if !meta.is_file() || meta.len() < 4_096 {
            continue;
        }
        if let Some(kind) = kind_of(&path) {
            if only.is_some_and(|k| k != kind) {
                continue;
            }
            let list = match kind {
                Kind::Audio => &mut lib.audio,
                Kind::Video => &mut lib.video,
            };
            if list.len() < MEDIA_CAP {
                list.push(Track::new(path, kind));
            }
        }
    }
}

fn is_skip_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        name.as_str(),
        "windows"
            | "program files"
            | "program files (x86)"
            | "programdata"
            | "$recycle.bin"
            | "system volume information"
            | "node_modules"
            | ".git"
            | "appdata"
            | "windows.old"
            | "recovery"
    )
}

/// 按扩展名识别媒体类型。与 `thumbs::is_video_file` 保持一致，音频额外覆盖常见格式。
pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())?;
    if AUDIO_EXT.contains(&ext.as_str()) {
        Some(Kind::Audio)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(Kind::Video)
    } else {
        None
    }
}

/// 播放器对 UI 暴露的状态快照。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub active: bool,
    /// 当前文件自然播放结束；UI 消费一次后会自动切下一首。
    pub ended: bool,
    pub paused: bool,
    pub path: Option<PathBuf>,
    pub title: String,
    pub time: f64,
    pub duration: f64,
    pub volume: f64,
    pub muted: bool,
    pub error: Option<String>,
}

impl State {
    pub fn progress(&self) -> f32 {
        if self.duration > 0.0 {
            (self.time / self.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
}

pub fn format_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--:--".to_string();
    }
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// mpv 进程与 IPC 客户端。
///
/// 只能用 `Mpv::start` 创建；退出时 `Drop` 会结束子进程，避免常驻后台。
pub struct Mpv {
    child: Child,
    writer: Option<Box<dyn Write + Send>>,
    events: Option<std::thread::JoinHandle<()>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    rx: Receiver<Answer>,
    pending: VecDeque<i64>,
    next_id: i64,
    pub state: State,
    volume_before_mute: f64,
    clock: PollClock,
}

#[derive(Debug)]
enum Answer {
    Ok {
        id: i64,
        data: Option<serde_json::Value>,
    },
    Err {
        id: i64,
        error: String,
    },
    Event {
        name: String,
        value: Option<serde_json::Value>,
    },
    Eof,
}

impl Mpv {
    /// 启动一个无窗口 mpv 实例并连接 IPC。`video_window` 为 `Some` 时视频会
    /// 渲染进宿主窗口（桌面播放器则传 `None`，由 mpv 自己创建窗口）。
    pub fn start(video: bool) -> Result<Self, String> {
        Self::start_impl(video, None, 80.0, false)
    }

    /// 启动专用直播 mpv：直接以 HLS 地址开窗（`idle=no`），因此用户关闭
    /// mpv 窗口或流结束时进程会退出，会话层能可靠收尾。不会把地址写进
    /// 状态或错误文本。
    pub fn start_live(
        stream_url: &str,
        title: &str,
        volume: f64,
        muted: bool,
    ) -> Result<Self, String> {
        if !safe_live_url(stream_url) {
            return Err("直播地址不是安全的 HTTPS HLS".to_string());
        }
        Self::start_impl(true, Some((stream_url, title)), volume, muted)
    }

    fn start_impl(
        video: bool,
        live: Option<(&str, &str)>,
        initial_volume: f64,
        initial_muted: bool,
    ) -> Result<Self, String> {
        let exe = crate::livewall::ensure_mpv()?;
        let pipe = ipc_name();
        let initial_volume = initial_volume.clamp(0.0, 130.0);
        let is_live = live.is_some();
        let mut cmd = Command::new(&exe);
        cmd.arg("--no-config")
            .arg("--no-terminal")
            .arg("--keep-open=no")
            .arg(format!("--input-ipc-server={pipe}"))
            .arg(format!("--volume={initial_volume:.0}"))
            .arg("--hwdec=auto-safe")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if initial_muted {
            cmd.arg("--mute=yes");
        }
        if let Some((_, title)) = live {
            cmd.arg("--idle=no")
                .arg("--cache=yes")
                .arg("--demuxer-max-bytes=128MiB")
                .arg("--network-timeout=15")
                .arg(format!("--title={title}"))
                .arg(
                    "--http-header-fields=Referer: https://yangshipin.cn/\r\n\
                     User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/131.0.0.0 Safari/537.36",
                );
        } else {
            cmd.arg("--idle=yes")
                .arg("--title=Wallpaper Studio · 媒体播放器");
        }
        if !video {
            cmd.arg("--force-window=no").arg("--audio-display=no");
        } else {
            cmd.arg("--force-window=yes");
        }
        if let Some((stream_url, _)) = live {
            cmd.arg(stream_url);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("启动 mpv 失败：{e}"))?;
        let mut pair = None;
        // Windows 命名管道需要 mpv 先创建；Unix socket 同理，轮询几百毫秒即可。
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(p) = connect(&pipe) {
                pair = Some(p);
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("mpv 启动后立即退出（{status}）"));
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        let Some((writer, stream)) = pair else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("无法连接 mpv IPC（播放器初始化超时）".to_string());
        };
        // 命名管道客户端在 Rust 的 `File` 写入上可能阻塞；给写端加超时包装，
        // 保证任何异常都不会把 UI 线程卡死。
        let writer: IpcWriter = Box::new(TimeoutWriter::new(writer));
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut mpv = Self {
            child,
            writer: Some(writer),
            events: None,
            stop,
            rx,
            pending: VecDeque::new(),
            next_id: 1,
            state: State {
                active: true,
                title: live.map(|(_, title)| title.to_string()).unwrap_or_default(),
                volume: initial_volume,
                muted: initial_muted,
                ..State::default()
            },
            volume_before_mute: if initial_volume > 0.0 {
                initial_volume
            } else {
                80.0
            },
            clock: PollClock::default(),
        };
        // 先启动读线程，再发命令：mpv 启动时会先写一批事件，读端必须已在消费。
        let stop_thread = Arc::clone(&mpv.stop);
        let events = std::thread::spawn(move || {
            let mut reader = stream;
            let mut line = String::new();
            loop {
                if stop_thread.load(Ordering::Relaxed) {
                    break;
                }
                line.clear();
                let Ok(n) = std::io::BufRead::read_line(&mut reader, &mut line) else {
                    break;
                };
                if n == 0 {
                    break;
                }
                if let Some(ans) = parse_answer(line.trim_end()) {
                    if tx.send(ans).is_err() {
                        break;
                    }
                }
            }
            let _ = tx.send(Answer::Eof);
        });
        mpv.events = Some(events);
        let _ = mpv.command(&["set_property", "pause", "no"]);
        let _ = mpv.observe(!is_live);
        Ok(mpv)
    }

    /// 子进程 PID，供诊断 / 测试确认独立播放器窗口归属。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn play(&mut self, path: &Path) -> Result<(), String> {
        if !path.is_file() {
            return Err(format!("文件不存在：{}", path.display()));
        }
        self.command(&["loadfile", &path.to_string_lossy(), "replace"])?;
        self.state.path = Some(path.to_path_buf());
        self.state.title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.state.time = 0.0;
        self.state.duration = 0.0;
        self.state.paused = false;
        self.state.ended = false;
        self.state.error = None;
        Ok(())
    }

    pub fn toggle_pause(&mut self) -> Result<(), String> {
        let pause = !self.state.paused;
        self.command(&["set_property", "pause", if pause { "yes" } else { "no" }])?;
        self.state.paused = pause;
        Ok(())
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        self.command(&["set_property", "pause", if paused { "yes" } else { "no" }])?;
        self.state.paused = paused;
        Ok(())
    }

    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        let t = seconds.max(0.0);
        self.command(&["seek", &format!("{t:.3}"), "absolute"])?;
        self.state.time = t;
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<(), String> {
        let v = volume.clamp(0.0, 130.0);
        self.command(&["set_property", "volume", &format!("{v:.0}")])?;
        self.state.volume = v;
        if v > 0.0 {
            self.volume_before_mute = v;
        }
        Ok(())
    }

    pub fn toggle_mute(&mut self) -> Result<(), String> {
        let muted = !self.state.muted;
        self.set_muted(muted)
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        self.command(&["set_property", "mute", if muted { "yes" } else { "no" }])?;
        self.state.muted = muted;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.command(&["stop"])?;
        self.state.paused = false;
        self.state.active = false;
        self.state.ended = false;
        self.state.time = 0.0;
        self.state.duration = 0.0;
        Ok(())
    }

    /// 视频窗口的放大 / 全屏由 mpv 自己处理，UI 只发命令。
    pub fn toggle_fullscreen(&mut self) -> Result<(), String> {
        self.command(&["cycle", "fullscreen"])?;
        Ok(())
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), String> {
        self.command(&[
            "set_property",
            "fullscreen",
            if fullscreen { "yes" } else { "no" },
        ])?;
        Ok(())
    }

    /// 非阻塞读取 IPC 回复并更新状态。每帧调用一次即可。
    pub fn poll(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(Answer::Ok { id, data }) => {
                    if self.pending.front() == Some(&id) {
                        self.pending.pop_front();
                    }
                    if let Some(v) = data {
                        apply_data(&mut self.state, &v);
                    }
                }
                Ok(Answer::Err { id, error }) => {
                    if self.pending.front() == Some(&id) {
                        self.pending.pop_front();
                    }
                    self.state.error = Some(error);
                }
                Ok(Answer::Event { name, value }) => {
                    if let Some(v) = value {
                        apply_event(&mut self.state, &name, &v);
                    }
                }
                Ok(Answer::Eof) | Err(TryRecvError::Disconnected) => {
                    self.state.active = false;
                    self.state.paused = false;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        // 文件真正加载出 duration 之前查询会被 mpv 回成 property unavailable，
        // 不要把它当成播放错误显示在 UI 上。
        if self.state.active
            && self.state.duration > 0.0
            && self.clock.due(Duration::from_millis(350))
        {
            let _ = self.command(&["get_property", "time-pos"]);
        }
        if self.state.duration > 0.0 {
            // 让进度条在没有事件时也平滑推进；真实值仍以 mpv 的 time-pos 为准。
            self.state.time = self.state.time.min(self.state.duration);
        }
    }

    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                self.state.active = false;
                false
            }
            Err(_) => false,
        }
    }

    /// 有界关闭：不向可能已卡死的管道写命令，直接终止子进程并限时回收。
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.writer = None;
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            }
        }
        if let Some(t) = self.events.take() {
            // 读线程可能仍阻塞在命名管道上：用短轮询代替无界 join。
            let deadline = Instant::now() + Duration::from_millis(250);
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }

    /// 自然播放结束只触发一次（由 UI 决定是否切下一首）。
    pub fn take_ended(&mut self) -> bool {
        if self.state.ended {
            self.state.ended = false;
            true
        } else {
            false
        }
    }

    fn command(&mut self, args: &[&str]) -> Result<(), String> {
        let args = args
            .iter()
            .map(|a| serde_json::Value::String((*a).to_string()))
            .collect::<Vec<_>>();
        self.command_json(&args)
    }

    fn command_json(&mut self, args: &[serde_json::Value]) -> Result<(), String> {
        let Some(w) = self.writer.as_mut() else {
            return Err("播放器 IPC 已关闭".to_string());
        };
        let mut line = format!("{{\"command\":[");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            line.push_str(&serde_json::to_string(a).map_err(|e| e.to_string())?);
        }
        line.push_str(&format!("],\"request_id\":{}}}", self.next_id));
        line.push('\n');
        w.write_all(line.as_bytes())
            .and_then(|_| w.flush())
            .map_err(|e| format!("向 mpv 发送命令失败：{e}"))?;
        self.pending.push_back(self.next_id);
        self.next_id += 1;
        Ok(())
    }

    fn observe(&mut self, include_path: bool) -> Result<(), String> {
        let mut properties = vec![
            "time-pos",
            "duration",
            "pause",
            "volume",
            "mute",
            "media-title",
            "idle-active",
        ];
        if include_path {
            properties.insert(properties.len() - 1, "path");
        }
        for (index, prop) in properties.into_iter().enumerate() {
            self.command_json(&[
                serde_json::Value::String("observe_property".to_string()),
                serde_json::Value::from(index as i64 + 1),
                serde_json::Value::String(prop.to_string()),
            ])?;
        }
        Ok(())
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn apply_data(state: &mut State, v: &serde_json::Value) {
    let Some(obj) = v.as_object() else {
        return;
    };
    for (key, val) in obj {
        apply_property(state, key, val);
    }
}

fn apply_event(state: &mut State, name: &str, v: &serde_json::Value) {
    let Some(obj) = v.as_object() else {
        return;
    };
    let prop = obj.get("name").and_then(|n| n.as_str()).unwrap_or(name);
    if let Some(val) = obj.get("data") {
        apply_property(state, prop, val);
    }
    if name == "end-file" {
        state.active = false;
        let reason = obj
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or_default();
        // 用户关闭播放器窗口或主动停止时会收到 `quit` / `stop`；只有流真正
        // 结束或解码失败才应触发“已结束 / 切下一首 / 官方回退”逻辑。
        state.ended = matches!(reason, "eof" | "error" | "redirect");
    }
    if name == "idle" {
        state.active = false;
        state.time = 0.0;
    }
}

fn apply_property(state: &mut State, key: &str, val: &serde_json::Value) {
    match key {
        "time-pos" => {
            if let Some(t) = val.as_f64() {
                state.time = t.max(0.0);
            }
        }
        "duration" => {
            if let Some(d) = val.as_f64() {
                state.duration = d.max(0.0);
            }
        }
        "pause" => {
            state.paused = val.as_bool().unwrap_or(false);
        }
        "volume" => {
            if let Some(v) = val.as_f64() {
                state.volume = v;
            }
        }
        "mute" => {
            state.muted = val.as_bool().unwrap_or(false);
        }
        "media-title" => {
            if let Some(t) = val.as_str() {
                state.title = t.to_string();
            }
        }
        "path" => {
            if let Some(p) = val.as_str() {
                if !p.is_empty() {
                    state.path = Some(PathBuf::from(p));
                    state.active = true;
                }
            }
        }
        "idle-active" => {
            if val.as_bool().unwrap_or(false) {
                state.active = false;
            }
        }
        _ => {}
    }
}

fn parse_answer(line: &str) -> Option<Answer> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if let Some(event) = v.get("event").and_then(|e| e.as_str()) {
        return Some(Answer::Event {
            name: event.to_string(),
            value: Some(v),
        });
    }
    let id = v.get("request_id").and_then(|i| {
        i.as_i64()
            .or_else(|| i.as_u64().map(|n| n as i64))
            .or_else(|| i.as_str().and_then(|s| s.parse::<i64>().ok()))
    })?;
    if v.get("error").and_then(|e| e.as_str()).unwrap_or("success") != "success" {
        return Some(Answer::Err {
            id,
            error: v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("mpv 返回错误")
                .to_string(),
        });
    }
    Some(Answer::Ok {
        id,
        data: v.get("data").cloned(),
    })
}

type IpcWriter = Box<dyn Write + Send>;
type IpcReader = Box<dyn BufRead + Send>;
type IpcPair = (IpcWriter, IpcReader);

/// 写端包装：实际写入放到独立线程，调用方最多等一小段时间。
/// Windows 命名管道在不可写时会阻塞 `write_all`，这里保证播放器 API 不会挂住 UI。
struct TimeoutWriter {
    tx: mpsc::Sender<Vec<u8>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TimeoutWriter {
    fn new(mut inner: IpcWriter) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let handle = std::thread::spawn(move || {
            while let Ok(bytes) = rx.recv() {
                if inner.write_all(&bytes).and_then(|_| inner.flush()).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            handle: Some(handle),
        }
    }
}

impl Write for TimeoutWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx
            .send(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "IPC 写线程已退出"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for TimeoutWriter {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        let _ = std::mem::replace(&mut self.tx, tx);
        if let Some(h) = self.handle.take() {
            let deadline = Instant::now() + Duration::from_millis(200);
            while !h.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if h.is_finished() {
                let _ = h.join();
            }
        }
    }
}

#[cfg(windows)]
fn ipc_name() -> String {
    format!(
        r"\\.\pipe\wallpaper-rs-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(not(windows))]
fn ipc_name() -> String {
    std::env::temp_dir()
        .join(format!(
            "wallpaper-rs-{}-{}.sock",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
        .to_string_lossy()
        .to_string()
}

#[cfg(windows)]
fn connect(name: &str) -> Option<IpcPair> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name)
        .ok()?;
    let reader = file.try_clone().ok()?;
    Some((
        Box::new(file),
        Box::new(BufReader::new(reader)) as IpcReader,
    ))
}

#[cfg(not(windows))]
fn connect(name: &str) -> Option<IpcPair> {
    use std::os::unix::net::UnixStream;
    let stream = UnixStream::connect(name).ok()?;
    let read = stream.try_clone().ok()?;
    Some((
        Box::new(stream),
        Box::new(BufReader::new(read)) as IpcReader,
    ))
}

/// 获取当前系统时间点，供页面显示「正在播放」脉冲等轻动画。
pub fn now_tick() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

/// 把磁盘路径显示成短名，避免页面被长路径撑破。
pub fn short_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// 扫描时跳过不可读目录的共享状态（供 UI 显示「已扫描 N 个目录」）。
pub fn dirs_label(dirs: &[PathBuf]) -> String {
    match dirs.len() {
        0 => "尚未添加媒体文件夹".to_string(),
        1 => dirs[0].display().to_string(),
        n => format!("{} 个文件夹 · {} …", n, dirs[0].display()),
    }
}

/// 按扩展名推断媒体类型，供外部（如拖放）复用。
pub fn kind_from_path(path: &Path) -> Option<Kind> {
    kind_of(path)
}

/// 给播放器页面提供「播放列表内下一首」的纯函数，便于测试。
pub fn next_index(len: usize, current: Option<usize>, step: isize, repeat: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let Some(current) = current else {
        return Some(if step < 0 { len - 1 } else { 0 });
    };
    let cur = current as isize;
    let mut next = cur + step;
    if next < 0 || next >= len as isize {
        if !repeat {
            return None;
        }
        next = next.rem_euclid(len as isize);
    }
    Some(next as usize)
}

/// UI 侧只需要知道「当前是哪个实例」时用的轻量 ID。
pub fn fresh_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// 供测试 / 诊断：mpv 是否可用。
pub fn available() -> bool {
    crate::livewall::mpv_available()
}

/// 直播只接受 HTTPS 的 HLS 清单。上游桥允许 HTTP 以便兼容旧接口，但真正
/// 交给 mpv 前必须再收紧一次；这里只返回布尔值，不把地址带进错误文本。
fn safe_live_url(url: &str) -> bool {
    let without_fragment = url.split('#').next().unwrap_or_default();
    let path = without_fragment
        .split('?')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    url.starts_with("https://") && path.ends_with(".m3u8")
}

/// 媒体进度更新的节流器：避免每帧都向 mpv 发查询。
pub struct PollClock {
    last: Instant,
}

impl Default for PollClock {
    fn default() -> Self {
        Self {
            last: Instant::now() - Duration::from_secs(5),
        }
    }
}

impl PollClock {
    pub fn due(&mut self, every: Duration) -> bool {
        if self.last.elapsed() >= every {
            self.last = Instant::now();
            true
        } else {
            false
        }
    }
}

/// 一个极简的共享游标：页面切换时记住上次选中的媒体。
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub audio: Option<u64>,
    pub video: Option<u64>,
}

impl Selection {
    pub fn get(&self, kind: Kind) -> Option<u64> {
        match kind {
            Kind::Audio => self.audio,
            Kind::Video => self.video,
        }
    }

    pub fn set(&mut self, kind: Kind, id: Option<u64>) {
        match kind {
            Kind::Audio => self.audio = id,
            Kind::Video => self.video = id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_kinds_by_extension() {
        assert_eq!(kind_of(Path::new("a.MP3")), Some(Kind::Audio));
        assert_eq!(kind_of(Path::new("a.flac")), Some(Kind::Audio));
        assert_eq!(kind_of(Path::new("a.MP4")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("a.mkv")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("a.txt")), None);
    }

    #[test]
    fn live_stream_requires_https_hls() {
        assert!(safe_live_url(
            "https://live.example.cctv.cn/path/channel.m3u8?token=secret"
        ));
        assert!(safe_live_url(
            "https://live.example.cctv.cn/path/channel.M3U8"
        ));
        assert!(!safe_live_url(
            "http://live.example.cctv.cn/path/channel.m3u8"
        ));
        assert!(!safe_live_url(
            "https://live.example.cctv.cn/path/channel.mp4"
        ));
        assert!(!safe_live_url("file:///tmp/channel.m3u8"));
    }

    #[test]
    fn closing_player_is_not_treated_as_stream_end() {
        let mut state = State::default();
        let event = serde_json::json!({"event": "end-file", "reason": "quit"});
        apply_event(&mut state, "end-file", &event);
        assert!(!state.active);
        assert!(!state.ended);
    }

    #[test]
    fn natural_eof_still_counts_as_stream_end() {
        let mut state = State::default();
        let event = serde_json::json!({"event": "end-file", "reason": "eof"});
        apply_event(&mut state, "end-file", &event);
        assert!(!state.active);
        assert!(state.ended);
    }

    #[test]
    fn progress_is_bounded() {
        let s = State {
            time: 12.0,
            duration: 60.0,
            ..State::default()
        };
        assert!((s.progress() - 0.2).abs() < 1e-6);
        let s = State {
            time: 90.0,
            duration: 60.0,
            ..State::default()
        };
        assert_eq!(s.progress(), 1.0);
        assert_eq!(State::default().progress(), 0.0);
    }

    #[test]
    fn time_formatting() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(3661.0), "1:01:01");
        assert_eq!(format_time(f64::NAN), "--:--");
    }

    #[test]
    fn playlist_stepping() {
        assert_eq!(next_index(0, None, 1, true), None);
        assert_eq!(next_index(3, Some(1), 1, false), Some(2));
        assert_eq!(next_index(3, Some(2), 1, false), None);
        assert_eq!(next_index(3, Some(2), 1, true), Some(0));
        assert_eq!(next_index(3, Some(0), -1, true), Some(2));
        assert_eq!(next_index(3, None, 1, true), Some(0));
        assert_eq!(next_index(3, None, -1, true), Some(2));
    }

    #[test]
    fn scan_finds_audio_and_video() {
        let root = std::env::temp_dir().join(format!("wp_media_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let bytes = vec![0u8; 8_192];
        std::fs::write(root.join("a.mp3"), &bytes).unwrap();
        std::fs::write(root.join("b.mp4"), &bytes).unwrap();
        std::fs::write(root.join("sub").join("c.flac"), &bytes).unwrap();
        std::fs::write(root.join("notes.txt"), &bytes).unwrap();

        let lib = scan_async(vec![root.clone()]).recv().unwrap();
        assert_eq!(lib.audio.len(), 2);
        assert_eq!(lib.video.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn libraries_do_not_mix_audio_and_video_dirs() {
        let root = std::env::temp_dir().join(format!("wp_media_split_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let music = root.join("music");
        let videos = root.join("videos");
        std::fs::create_dir_all(&music).unwrap();
        std::fs::create_dir_all(&videos).unwrap();
        let bytes = vec![0u8; 8_192];
        std::fs::write(music.join("song.mp3"), &bytes).unwrap();
        std::fs::write(music.join("wrong.mp4"), &bytes).unwrap();
        std::fs::write(videos.join("clip.mkv"), &bytes).unwrap();
        std::fs::write(videos.join("wrong.flac"), &bytes).unwrap();

        let lib = scan_libraries_blocking(vec![music.clone()], vec![videos.clone()]);
        assert_eq!(lib.audio.len(), 1);
        assert_eq!(lib.audio[0].title, "song");
        assert_eq!(lib.video.len(), 1);
        assert_eq!(lib.video[0].title, "clip");
        assert!(lib.scanned_dirs.contains(&music));
        assert!(lib.scanned_dirs.contains(&videos));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn player_roundtrip_with_real_mpv() {
        if !available() {
            eprintln!("skipping: mpv not available");
            return;
        }
        let wav = std::env::temp_dir().join(format!("wp_media_play_{}.wav", std::process::id()));
        let mut bytes = Vec::new();
        let rate = 44_100u32;
        let secs = 2u32;
        let data_len = rate * secs * 4;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..(rate * secs) {
            let sample =
                ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(&wav, &bytes).unwrap();

        eprintln!("media-test: start");
        let mut player = Mpv::start(false).expect("start mpv");
        eprintln!("media-test: started");
        player.set_volume(0.0).unwrap();
        eprintln!("media-test: volume set");
        player.play(&wav).unwrap();
        eprintln!("media-test: play sent");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got_duration = false;
        while Instant::now() < deadline {
            player.poll();
            if player.state.duration > 0.0 {
                got_duration = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(80));
        }
        eprintln!("media-test: duration={}", player.state.duration);
        assert!(got_duration, "mpv should report duration");
        assert_eq!(player.state.path.as_deref(), Some(wav.as_path()));
        player.toggle_pause().unwrap();
        eprintln!("media-test: pause sent");
        std::thread::sleep(Duration::from_millis(200));
        player.poll();
        assert!(player.state.paused, "pause state should round-trip");
        player.seek(0.5).unwrap();
        let seek_deadline = Instant::now() + Duration::from_secs(3);
        let mut sought = false;
        while Instant::now() < seek_deadline {
            player.poll();
            if player.state.time >= 0.45 {
                sought = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        eprintln!("media-test: seek={}", player.state.time);
        assert!(sought, "seek should move playback position");
        player.stop().unwrap();
        eprintln!("media-test: stop sent");
        drop(player);
        eprintln!("media-test: dropped");
        let _ = std::fs::remove_file(&wav);
    }

    /// 真实视频窗口（mpv 独立窗口）+ seek / 暂停 / 退出清理。
    ///
    /// 用一段 2 秒、160x120 的未压缩 AVI，避免测试依赖网络素材。
    #[test]
    fn player_opens_real_video_window_and_exits_cleanly() {
        if !available() {
            eprintln!("skipping: mpv not available");
            return;
        }
        let avi = std::env::temp_dir().join(format!("wp_media_video_{}.avi", std::process::id()));
        write_test_avi(&avi, 160, 120, 10, 2);

        let started = Instant::now();
        let mut player = Mpv::start(true).expect("start video mpv");
        let start_ms = started.elapsed().as_millis();
        let pid = player.pid();
        player.set_volume(0.0).unwrap();
        player.play(&avi).unwrap();

        let deadline = Instant::now() + Duration::from_secs(6);
        let mut ready_ms = None;
        while Instant::now() < deadline {
            player.poll();
            if player.state.duration > 0.0 && player.state.path.is_some() {
                ready_ms = Some(started.elapsed().as_millis());
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        assert!(
            ready_ms.is_some(),
            "mpv should load the video and report duration"
        );
        assert!(player.state.active, "video should be active");

        player.seek(0.6).unwrap();
        let seek_deadline = Instant::now() + Duration::from_secs(3);
        let mut sought = false;
        while Instant::now() < seek_deadline {
            player.poll();
            if player.state.time >= 0.55 {
                sought = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        assert!(sought, "video seek should move playback position");

        player.set_paused(true).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        player.poll();
        assert!(player.state.paused, "video pause should round-trip");

        let stopped = Instant::now();
        drop(player);
        let drop_ms = stopped.elapsed().as_millis();
        eprintln!(
            "media-video: start={start_ms}ms ready={}ms drop={drop_ms}ms pid={pid}",
            ready_ms.unwrap_or_default()
        );
        assert!(drop_ms < 3000, "drop must be bounded");
        let mut still_alive = false;
        for _ in 0..20 {
            let out = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output();
            if let Ok(out) = out {
                let text = String::from_utf8_lossy(&out.stdout);
                if text.contains(&pid.to_string()) {
                    still_alive = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!still_alive, "mpv child process must exit after drop");
        let _ = std::fs::remove_file(&avi);
    }

    fn write_test_avi(path: &Path, w: u32, h: u32, fps: u32, secs: u32) {
        let n = fps * secs;
        let row = w * 3;
        let pad = (4 - row % 4) % 4;
        let row_padded = row + pad;
        let frame_size = 40 + row_padded * h;
        let movi_payload = 4 + n * (8 + frame_size);
        let strl_payload = 4 + (8 + 56) + (8 + 40);
        let hdrl_payload = 4 + (8 + 56) + (8 + strl_payload);
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(4 + 8 + hdrl_payload + 8 + movi_payload).to_le_bytes());
        b.extend_from_slice(b"AVI ");
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&(hdrl_payload as u32).to_le_bytes());
        b.extend_from_slice(b"hdrl");
        b.extend_from_slice(b"avih");
        b.extend_from_slice(&56u32.to_le_bytes());
        b.extend_from_slice(&(1_000_000 / fps).to_le_bytes());
        b.extend_from_slice(&(row_padded * h * fps).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0x10u32.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&(frame_size as u32).to_le_bytes());
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        for _ in 0..4 {
            b.extend_from_slice(&0u32.to_le_bytes());
        }
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&(strl_payload as u32).to_le_bytes());
        b.extend_from_slice(b"strl");
        b.extend_from_slice(b"strh");
        b.extend_from_slice(&56u32.to_le_bytes());
        b.extend_from_slice(b"vids");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&fps.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&(frame_size as u32).to_le_bytes());
        b.extend_from_slice(&u32::MAX.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..4 {
            b.extend_from_slice(&0u16.to_le_bytes());
        }
        b.extend_from_slice(b"strf");
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(row_padded * h).to_le_bytes());
        for _ in 0..4 {
            b.extend_from_slice(&0u32.to_le_bytes());
        }
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&(movi_payload as u32).to_le_bytes());
        b.extend_from_slice(b"movi");
        let mut row_bytes = vec![0u8; row_padded as usize];
        for frame in 0..n {
            b.extend_from_slice(b"00db");
            b.extend_from_slice(&(frame_size as u32).to_le_bytes());
            b.extend_from_slice(&40u32.to_le_bytes());
            b.extend_from_slice(&w.to_le_bytes());
            b.extend_from_slice(&h.to_le_bytes());
            b.extend_from_slice(&1u16.to_le_bytes());
            b.extend_from_slice(&24u16.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&(row_padded * h).to_le_bytes());
            for _ in 0..4 {
                b.extend_from_slice(&0u32.to_le_bytes());
            }
            for x in 0..w {
                let v = ((x + frame * 12) % 255) as u8;
                let i = (x * 3) as usize;
                row_bytes[i] = v;
                row_bytes[i + 1] = 255u8.wrapping_sub(v);
                row_bytes[i + 2] = 128;
            }
            for _ in 0..h {
                b.extend_from_slice(&row_bytes);
            }
        }
        std::fs::write(path, b).unwrap();
    }
}
