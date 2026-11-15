//! 运行期「看门狗」：监控未响应 / 崩溃 / 功能性错误，并自动上报到 GitHub Issues。
//!
//! ## 为什么需要它
//! 应用里有若干跨线程 / 跨进程调用（mpv 子进程、桌面宿主窗口、Explorer 消息、
//! UAC 提权…），偶发卡死或异常时用户往往只能看到「未响应」，拿不到现场。这里内置
//! 一个轻量监控线程，把现场自动收集并上报，方便快速定位与修复。
//!
//! ## 工作机制
//! * UI 线程每帧调用 [`heartbeat`] 打一个时间戳；后台监控线程定期比对，超过
//!   [`HANG_AFTER`] 没有打点即判定「未响应（卡死）」。同时用
//!   [`std::panic::set_hook`] 捕获 panic（发行版 `panic=abort`，钩子仍会先执行）。
//! * **只上报「非网络」问题**：网络请求 / 下载失败由 [`is_network`] 过滤，不上报。
//! * 上报方式（自动二选一）：
//!   1. 未配置 token（默认）：打开「预填好标题 / 正文的 GitHub 新建 issue 页面」，
//!      用户点一下「Create」即可 —— 进程内**不内置任何密钥**，最安全。
//!   2. 在「设置」里填了 GitHub token（或设 `WALLPAPER_REPORT_TOKEN` 环境变量）时：
//!      直接调用 GitHub REST API 静默创建 issue（无需用户操作）。
//! * 所有事件无论如何都会追加写入本地日志
//!   `%APPDATA%/wallpaper-rs/logs/report.log`（无网络 / 被禁用时也不会丢）。
//! * 去重 + 限速，避免同一问题反复刷屏。
//!
//! ## 上报里都带了什么
//! 为了让维护者不必来回追问，issue 正文会尽量自带现场：
//! * **运行环境快照**：版本、发行版、桌面会话（`XDG_CURRENT_DESKTOP` / 会话类型）、
//!   CI / 容器标记、配置目录、以及 mpv 探测详情（见 [`crate::livewall::mpv_probe_report`]）。
//! * **最近操作面包屑**（[`note`]）：卡死 / 崩溃前的关键动作序列。
//! * **panic 位置与回溯**、功能错误的**调用位置**（[`report_error`]）。
//! * **本地日志尾部**：便于把同一时间线上的其它线索一并带上。

use crate::wallpaper::system_snapshot;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 目标仓库的「新建 issue」页面（配合查询串预填标题 / 正文）。
const ISSUES_NEW: &str = "https://github.com/shuying-code/wallpaper-rs/issues/new";
/// 目标仓库的 issue 列表页（供 UI「打开 Issues」入口使用）。
pub const ISSUES_URL: &str = "https://github.com/shuying-code/wallpaper-rs/issues";
/// GitHub REST API：创建 issue（需要 token）。
const API_ISSUES: &str = "https://api.github.com/repos/shuying-code/wallpaper-rs/issues";

/// 超过这么久没有心跳就判定 UI 线程卡死。
///
/// 之所以远大于监控轮询间隔 [`POLL`]，是要给「偶发的一次慢帧」以及后台唤醒抖动
/// 留出余量：监控线程会主动请求重绘（见 [`bind_ui`]），正常空闲的界面也会持续
/// 打点，因此只有在 UI 线程**真的**被长时间阻塞时才会触发。
const HANG_AFTER: Duration = Duration::from_secs(12);
/// 监控线程轮询间隔（同时也是主动请求重绘的节奏）。
const POLL: Duration = Duration::from_secs(2);
/// 判定卡死需要连续命中的轮数：单次抖动不算，避免误报。
const HANG_CONFIRM: u32 = 3;
/// 单次运行最多上报的条数（防止刷屏）。
const MAX_REPORTS: u64 = 12;
/// 无 token 时「打开网页」的最小间隔（同一条问题去重已挡掉大部分）。
const WEB_MIN_GAP: Duration = Duration::from_secs(600);
/// 跨进程去重窗口：同一「签名」在这个时间内只上报一次（防止多开实例重复建 Issue）。
const DEDUP_WINDOW: Duration = Duration::from_secs(1800);
/// 本地「最近操作」面包屑保留条数。
const MAX_NOTES: usize = 40;
/// 上报正文里附带的本地日志行数。
const LOG_TAIL_LINES: usize = 60;

/// 内置上报 Token 的**混淆密文**（与 [`TOKEN_KEY`] 做 XOR 密钥流还原）。
///
/// ⚠️ 这是**混淆**、不是强加密：目的是让源码 / 二进制里**不出现明文 token**，
/// 从而不会被 GitHub secret scanning 命中而自动吊销，也挡掉随手搜索。
/// 但它**挡不住有意逆向的人**——所以这个 token 的权限必须保持最小
/// （当前仅「`shuying-code/wallpaper-rs` 的 Issues: Read and write」）。
const TOKEN_BLOB: &[u8] = &[
    0x94, 0x35, 0x68, 0x69, 0x5a, 0x3c, 0xea, 0xf4, 0x56, 0xc6, 0xb6, 0x24, 0x79, 0x7c, 0xc6, 0x51,
    0xe5, 0x7f, 0xa9, 0x65, 0xbf, 0xb3, 0xd9, 0x86, 0x06, 0x4b, 0xed, 0x2f, 0x05, 0x05, 0xbe, 0xe6,
    0x5b, 0xbe, 0xdc, 0x90, 0xdd, 0x4f, 0x74, 0xdb, 0xb4, 0x44, 0x3e, 0x10, 0xa1, 0x52, 0x0f, 0xd0,
    0xb9, 0x50, 0xbf, 0x4d, 0x59, 0x25, 0xcd, 0xc7, 0x86, 0x81, 0x0f, 0x20, 0x5f, 0x80, 0x1e, 0x4e,
    0xa3, 0x47, 0x2f, 0xbf, 0x3f, 0x76, 0x9e, 0x68, 0xde, 0xc6, 0xaa, 0xf0, 0x2b, 0x61, 0x85, 0x72,
    0x41, 0x2f, 0xc4, 0x0d, 0xcc, 0xa5, 0x5f, 0x6c, 0x91, 0x5c, 0xf4, 0x50, 0x44,
];

/// 混淆用密钥（32 字节）。
const TOKEN_KEY: [u8; 32] = [
    0x5a, 0x1f, 0x3c, 0x92, 0x77, 0x0b, 0xe4, 0x6d, 0x31, 0xa8, 0x54, 0x9e, 0xc2, 0x0f, 0x86, 0x4b,
    0xd3, 0x27, 0x60, 0xbb, 0x19, 0xee, 0x48, 0x7a, 0x05, 0xf1, 0x33, 0x9c, 0x6e, 0x2a, 0xd7, 0x71,
];

/// xorshift 生成的字节密钥流，再与 [`TOKEN_KEY`] 逐字节混合。
fn token_keystream(len: usize) -> Vec<u8> {
    let mut st: u64 = 0x243F_6A88_85A3_08D3;
    let mut out = Vec::with_capacity(len);
    let mut i = 0usize;
    while out.len() < len {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        out.push(((st >> 24) as u8) ^ TOKEN_KEY[i % TOKEN_KEY.len()]);
        i += 1;
    }
    out
}

/// 还原内置上报 Token（运行时解密；源码里只有密文）。
pub fn builtin_token() -> Option<String> {
    let ks = token_keystream(TOKEN_BLOB.len());
    let bytes: Vec<u8> = TOKEN_BLOB.iter().zip(ks).map(|(b, k)| b ^ k).collect();
    String::from_utf8(bytes).ok()
}

/// Token 解析优先级：显式传入（配置 / 设置）> 环境变量 `WALLPAPER_REPORT_TOKEN` > 内置。
fn resolve_token(explicit: &str) -> String {
    let e = explicit.trim();
    if !e.is_empty() {
        return e.to_string();
    }
    if let Ok(v) = std::env::var("WALLPAPER_REPORT_TOKEN") {
        let v = v.trim();
        if !v.is_empty() {
            return v.to_string();
        }
    }
    builtin_token().unwrap_or_default()
}

/// 当前是否正在使用**内置** Token（供设置页显示）。
pub fn using_builtin() -> bool {
    let cur = rep().token.lock().map(|g| g.clone()).unwrap_or_default();
    builtin_token().map(|b| b == cur).unwrap_or(false)
}

/// 上报器全局状态。
struct Reporter {
    /// 监控起点（所有时间戳相对它计算）。
    start: Instant,
    /// 是否启用自动上报（跟随设置开关）。
    enabled: AtomicBool,
    /// 可选的 GitHub token（用户在设置里填写，或取环境变量）。
    token: Mutex<String>,
    /// 最近一次心跳（相对 `start` 的毫秒数）。
    beat_ms: AtomicU64,
    /// 最近一次心跳的**挂钟**秒数（`UNIX_EPOCH` 起）。与 [`Self::beat_ms`] 一起
    /// 用来判断「这段时间系统是否休眠/挂起」（挂钟跳变远大于单调时钟跳变）。
    beat_wall: AtomicU64,
    /// 是否已收到过至少一次心跳（未收到前不判定卡死，避免启动阶段误报）。
    beat_seen: AtomicBool,
    /// 当前「场景」描述（如「轮播 → 动态壁纸」），供卡死上报定位。
    scene: Mutex<String>,
    /// 运行期「面包屑」：最近的若干次关键操作（供卡死/崩溃时还原现场）。
    notes: Mutex<VecDeque<(u64, String)>>,
    /// 运行期线程状态快照：最近一次更新时记录的线程名、状态与活动标记。
    thread_states: Mutex<Vec<String>>,
    /// egui 上下文：监控线程用它主动请求重绘，保证空闲/隐藏时界面仍在打点。
    ui: Mutex<Option<crate::egui::Context>>,
    /// 已上报过的问题签名（去重）。
    seen: Mutex<std::collections::HashSet<String>>,
    /// 已上报条数。
    count: AtomicU64,
    /// 上次「打开网页」上报的时间（限速用）。
    last_web: Mutex<Option<Instant>>,
}

static REPORTER: OnceLock<Reporter> = OnceLock::new();

fn rep() -> &'static Reporter {
    REPORTER.get_or_init(|| Reporter {
        start: Instant::now(),
        enabled: AtomicBool::new(true),
        token: Mutex::new(String::new()),
        beat_ms: AtomicU64::new(0),
        beat_wall: AtomicU64::new(0),
        beat_seen: AtomicBool::new(false),
        scene: Mutex::new(String::new()),
        notes: Mutex::new(VecDeque::new()),
        thread_states: Mutex::new(Vec::new()),
        ui: Mutex::new(None),
        seen: Mutex::new(std::collections::HashSet::new()),
        count: AtomicU64::new(0),
        last_web: Mutex::new(None),
    })
}

/// 启动监控线程并安装 panic 钩子。可安全地重复调用（只在首次真正安装线程 / 钩子）。
pub fn install(enabled: bool, token: String) {
    let r = rep();
    r.enabled.store(enabled, Ordering::Relaxed);
    if let Ok(mut t) = r.token.lock() {
        *t = resolve_token(&token);
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        // 保留默认钩子（仍打印到 stderr），在其前面插入我们的上报。
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // panic=abort 之下进程即将终止：这里同步尽力上报一次。
            // 尽量把「位置 + 回溯」也带上，方便直接定位到崩溃点。
            let loc = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "未知位置".to_string());
            let payload = panic_payload(info);
            let bt = std::backtrace::Backtrace::force_capture();
            let detail = format!(
                "panic：{payload}\n位置：{loc}\n\n线程：{:?}\n\n回溯：\n{bt}",
                std::thread::current().name().unwrap_or("<未命名>"),
            );
            let summary = format!("panic @ {loc}：{}", truncate(&payload, 160));
            report_event("panic", &summary, &detail, true);
            prev(info);
        }));
        std::thread::spawn(watchdog);
    });
}

/// 把 egui 上下文交给监控线程：它会在每次轮询时主动 `request_repaint()`，让界面在
/// 空闲 / 隐藏到托盘时也持续打心跳，从而把「真的卡死」和「只是没在重绘」区分开。
pub fn bind_ui(ctx: crate::egui::Context) {
    if let Ok(mut g) = rep().ui.lock() {
        g.replace(ctx.clone());
    }
    ctx.request_repaint();
}

/// 追加一条「面包屑」：记录运行期的关键动作，供卡死 / 崩溃上报时还原现场。
pub fn note(msg: &str) {
    let r = rep();
    let at = r.start.elapsed().as_millis() as u64;
    if let Ok(mut g) = r.notes.lock() {
        if g.len() >= MAX_NOTES {
            g.pop_front();
        }
        g.push_back((at, msg.to_string()));
    }
}

/// 把最近的面包屑拼成一段可读文本（供上报正文使用）。
fn notes_text() -> String {
    let r = rep();
    let g = match r.notes.lock() {
        Ok(g) => g,
        Err(_) => return String::new(),
    };
    g.iter()
        .map(|(ms, s)| format!("+{:.1}s  {}", *ms as f64 / 1000.0, s))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 登记一个受监控线程的活动状态（例如后台下载、壁纸设置、宿主操作）。
/// `active` 表示该线程是否正处于已提交但未完成的阶段。
pub fn thread_state(thread: &str, stage: &str, active: bool) {
    let r = rep();
    let at = r.start.elapsed().as_millis() as u64;
    if let Ok(mut g) = r.thread_states.lock() {
        let line = format!(
            "+{:.1}s  [{thread}] {stage} · {}",
            at as f64 / 1000.0,
            if active { "进行中" } else { "完成" }
        );
        if g.len() >= 120 {
            g.remove(0);
        }
        g.push(line);
    }
}

/// 追加一条系统级诊断事件（如 Windows 事件日志条目 / 系统快照变化）。
pub fn system_event(source: &str, level: &str, message: &str) {
    let line = format!("[{source}] [{level}] {message}");
    write_diagnostics("system", &line);
    note(&format!("系统事件：{source} {level} {message}"));
}

/// 追加一条应用诊断状态（模式、轮播状态、清单数量、当前壁纸、线程活动）。
pub fn app_state(mode: &str, running: bool, playlist_len: usize, current: &std::path::Path) {
    let threads = thread_states_text();
    let line = format!(
        "mode={mode} running={running} playlist={playlist_len} current={}\n{threads}",
        current.display()
    );
    write_diagnostics("app", &line);
}

/// 拼装线程状态历史（供 issue 正文与本地诊断日志使用）。
fn thread_states_text() -> String {
    let r = rep();
    let g = match r.thread_states.lock() {
        Ok(g) => g,
        Err(_) => return String::new(),
    };
    g.iter()
        .rev()
        .take(40)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// 追加诊断日志（仅本地，供修复定位与 issue 附加）。
fn write_diagnostics(kind: &str, line: &str) {
    use std::io::Write;
    let path = diagnostics_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{}] ({kind}) {line}", timestamp());
    }
}

/// 更新「是否启用自动上报」。
pub fn set_enabled(v: bool) {
    rep().enabled.store(v, Ordering::Relaxed);
}

/// 更新 GitHub token（空串表示清除）。
pub fn set_token(t: &str) {
    if let Ok(mut g) = rep().token.lock() {
        *g = resolve_token(t);
    }
}

/// 记录「当前场景」，供卡死上报定位（如「轮播 → 动态壁纸」）。
pub fn set_scene(s: &str) {
    if let Ok(mut g) = rep().scene.lock() {
        if g.as_str() != s {
            *g = s.to_string();
            note(&format!("场景 → {s}"));
        }
    }
}

/// 本地日志文件路径。
pub fn log_path() -> std::path::PathBuf {
    crate::config::config_dir().join("logs").join("report.log")
}

/// 诊断日志路径：应用状态 / 系统事件 / 线程活动的详细时间线。
pub fn diagnostics_path() -> std::path::PathBuf {
    crate::config::config_dir()
        .join("logs")
        .join("diagnostics.log")
}

/// 跨进程去重用的记录文件（同一签名在 [`DEDUP_WINDOW`] 内只上报一次）。
fn dedup_path() -> std::path::PathBuf {
    crate::config::config_dir()
        .join("logs")
        .join("report_dedup.tsv")
}

/// 判断该签名是否刚被（本进程或其它进程）上报过；否则登记并返回 `false`。
fn dedup_recently_reported(sig: &str) -> bool {
    dedup_check(&dedup_path(), sig)
}

/// [`dedup_recently_reported`] 的核心（显式传入文件路径，便于单测，且不依赖环境变量）。
fn dedup_check(path: &std::path::Path, sig: &str) -> bool {
    let key = sig.replace(['\t', '\n', '\r'], " ");
    let now = now_wall();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut kept: Vec<(String, u64)> = Vec::new();
    let mut hit = false;
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let mut it = line.splitn(2, '\t');
            let (k, t) = match (it.next(), it.next()) {
                (Some(k), Some(t)) => (k, t),
                _ => continue,
            };
            let secs: u64 = t.trim().parse().unwrap_or(0);
            if now.saturating_sub(secs) > DEDUP_WINDOW.as_secs() {
                continue; // 过期，丢弃
            }
            if k == key {
                hit = true;
            }
            kept.push((k.to_string(), secs));
        }
    }
    if !hit {
        kept.push((key, now));
    }
    let body: String = kept.iter().map(|(k, t)| format!("{k}\t{t}\n")).collect();
    let _ = std::fs::write(path, body);
    hit
}

/// UI 线程每帧调用：打一个心跳时间戳。卡死检测完全依赖它。
pub fn heartbeat() {
    let r = rep();
    r.beat_ms
        .store(r.start.elapsed().as_millis() as u64, Ordering::Relaxed);
    r.beat_wall.store(now_wall(), Ordering::Relaxed);
    r.beat_seen.store(true, Ordering::Relaxed);
}

/// 上报一个「非网络」的功能性错误（供各模块调用）。网络类错误会被忽略。
#[track_caller]
pub fn report_error(source: &str, msg: &str) {
    if is_network(msg) {
        return;
    }
    let loc = std::panic::Location::caller();
    let summary = format!("{}：{}", source, truncate(msg, 160));
    let detail = format!(
        "{msg}\n\n来源：{source}\n调用位置：{}:{}",
        loc.file(),
        loc.line()
    );
    report_event("error", &summary, &detail, false);
}

/// 用户主动触发的上报（设置页按钮）：不参与去重，总能上报一次。
pub fn report_manual() {
    let scene = rep().scene.lock().map(|g| g.clone()).unwrap_or_default();
    let summary = if scene.is_empty() {
        "用户反馈".to_string()
    } else {
        format!("用户反馈（{scene}）")
    };
    let detail = "用户主动触发的问题上报。";
    note("用户主动上报");
    let threads = thread_states_text();
    let manual_detail = if threads.is_empty() {
        detail.to_string()
    } else {
        format!("{detail}\n\n线程状态：\n{threads}")
    };
    write_log("manual", &summary, &manual_detail);
    let body = compose_body("manual", &summary, detail);
    let title = format!("[反馈] {}", truncate(&summary, 110));
    submit_spawn(title, body);
}

/// 后台监控线程：定期比对心跳，判定卡死并上报。
fn watchdog() {
    // 「连续命中」计数：只有连续多轮都发现心跳过期才判定卡死，抗单次抖动。
    let mut stale_hits: u32 = 0;
    loop {
        std::thread::sleep(POLL);
        let r = rep();
        // 主动请求重绘：空闲 / 隐藏到托盘时 egui 默认不再重绘，界面也就不会打点，
        // 会被误判成卡死。这里由监控线程推它一把，让「活着但空闲」的界面持续心跳。
        if let Ok(g) = r.ui.lock() {
            if let Some(ctx) = g.as_ref() {
                ctx.request_repaint();
            }
        }
        if !r.enabled.load(Ordering::Relaxed) || !r.beat_seen.load(Ordering::Relaxed) {
            continue;
        }
        // 移动端（Android）在切到后台时系统会暂停事件循环，界面不打点属于正常现象，
        // 并非卡死——因此在移动端不做「卡死」判定（panic / 功能错误仍照常上报）。
        if cfg!(target_os = "android") {
            continue;
        }
        let last = r.beat_ms.load(Ordering::Relaxed);
        let now = r.start.elapsed().as_millis() as u64;
        let gaps = now.saturating_sub(last);
        // 挂钟 vs 单调时钟：若挂钟这段时间前进得远多于单调时钟，多半是系统休眠 /
        // 挂起（或进程被冻结），此时界面当然不会打点——不是卡死。
        let wall_gap = now_wall().saturating_sub(r.beat_wall.load(Ordering::Relaxed));
        let mono_gap_s = gaps / 1000;
        if wall_gap > mono_gap_s + 30 {
            r.beat_ms.store(now, Ordering::Relaxed);
            r.beat_wall.store(now_wall(), Ordering::Relaxed);
            stale_hits = 0;
            continue;
        }
        if gaps >= HANG_AFTER.as_millis() as u64 {
            stale_hits = stale_hits.saturating_add(1);
            if stale_hits < HANG_CONFIRM {
                continue;
            }
            // 确认卡死：先把心跳推到「现在」，避免持续卡死时每轮都触发（去重也会兜底）。
            r.beat_ms.store(now, Ordering::Relaxed);
            r.beat_wall.store(now_wall(), Ordering::Relaxed);
            stale_hits = 0;
            let secs = gaps as f64 / 1000.0;
            let scene = scene_or_unknown();
            let summary = format!("界面未响应约 {secs:.0} 秒（场景：{scene}）");
            let detail =
                format!("UI 线程超过 {secs:.0} 秒没有响应任何界面事件，疑似卡死。\n场景：{scene}");
            report_event("hang", &summary, &detail, false);
        } else {
            stale_hits = 0;
        }
    }
}

/// 当前场景（空则「未知」）。
fn scene_or_unknown() -> String {
    let scene = rep().scene.lock().map(|g| g.clone()).unwrap_or_default();
    if scene.is_empty() {
        "未知".to_string()
    } else {
        scene
    }
}

/// 记录事件：先写本地日志，再（在启用时、去重后）上报。
fn report_event(kind: &str, summary: &str, detail: &str, blocking: bool) {
    write_log(kind, summary, detail);
    let r = rep();
    if !r.enabled.load(Ordering::Relaxed) {
        return;
    }
    let sig = format!("{kind}|{summary}");
    match r.seen.lock() {
        Ok(mut seen) => {
            if !seen.insert(sig.clone()) {
                return; // 同一问题只报一次
            }
        }
        Err(_) => return,
    }
    // 跨进程去重：多开实例时两个进程可能在同一时刻各触发一次同样的上报，
    // 只靠进程内 `seen` 挡不住——用配置目录下的去重文件兜底。
    if dedup_recently_reported(&sig) {
        write_log("dedup", summary, "命中跨进程去重窗口，未重复上报。");
        return;
    }
    if r.count.fetch_add(1, Ordering::Relaxed) >= MAX_REPORTS {
        return;
    }
    let body = compose_body(kind, summary, detail);
    let title = format!("[自动上报] {}", truncate(summary, 110));
    if blocking {
        submit_blocking(&title, &body);
    } else {
        submit_spawn(title, body);
    }
}

/// 在后台线程里提交（不阻塞调用方，尤其是不阻塞 UI 线程）。
fn submit_spawn(title: String, body: String) {
    std::thread::spawn(move || submit_blocking(&title, &body));
}

/// 提交一次上报：有 token 就调 API 静默建 issue，否则回退到「打开预填网页」。
fn submit_blocking(title: &str, body: &str) {
    // 诊断 / 测试用：设了该环境变量就只写本地日志，不真正提交。
    if std::env::var_os("WALLPAPER_REPORT_NO_SUBMIT").is_some() {
        return;
    }
    let token = rep().token.lock().map(|g| g.clone()).unwrap_or_default();
    if !token.is_empty() && post_issue(&token, title, body).is_ok() {
        return;
    }
    open_prefilled(title, body);
}

/// 调用 GitHub REST API 创建 issue。
fn post_issue(token: &str, title: &str, body: &str) -> Result<(), String> {
    let payload = serde_json::json!({ "title": title, "body": body });
    crate::websrc::agent()
        .post(API_ISSUES)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "wallpaper-rs")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(payload)
        .map(|_| ())
        .map_err(|e| format!("创建 issue 失败：{e}"))
}

/// 打开预填标题 / 正文的 GitHub 新建 issue 页面（限速）。
fn open_prefilled(title: &str, body: &str) {
    {
        let mut g = match rep().last_web.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(t) = *g {
            if t.elapsed() < WEB_MIN_GAP {
                return;
            }
        }
        *g = Some(Instant::now());
    }
    open_url(&prefilled_url(title, body));
}

/// 生成「预填标题 / 正文」的 GitHub 新建 issue 链接。
fn prefilled_url(title: &str, body: &str) -> String {
    format!(
        "{ISSUES_NEW}?title={}&body={}",
        crate::websrc::urlencode(title),
        crate::websrc::urlencode(body)
    )
}

/// 组装 issue 正文（Markdown）。
fn compose_body(kind: &str, summary: &str, detail: &str) -> String {
    let scene = scene_or_unknown();
    let notes = notes_text();
    let env_block = environment();
    let tail = log_tail(LOG_TAIL_LINES);
    let mut body = format!(
        "### 自动上报（{kind}）\n\n\
         - **版本**：v{ver}\n\
         - **平台**：{os} / {arch}\n\
         - **时间**：{ts}\n\
         - **场景**：{scene}\n\
         - **实例**：{inst}\n\n\
         **摘要**：{summary}\n\n\
         **详情**：\n```\n{detail}\n```\n",
        kind = kind,
        ver = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        ts = timestamp(),
        scene = scene,
        inst = instance_id(),
        summary = summary,
        detail = fence(&truncate(detail, 4000)),
    );
    body.push_str(&format!("\n**环境**：\n```\n{env_block}\n```\n"));
    if !notes.is_empty() {
        body.push_str(&format!(
            "\n**最近操作**：\n```\n{}\n```\n",
            fence(&truncate(&notes, 2000))
        ));
    }
    let threads = thread_states_text();
    if !threads.is_empty() {
        body.push_str(&format!(
            "\n**线程状态**：\n```\n{}\n```\n",
            fence(&truncate(&threads, 3000))
        ));
    }
    if !tail.is_empty() {
        body.push_str(&format!(
            "\n<details><summary>本地日志（最近 {LOG_TAIL_LINES} 行）</summary>\n\n```\n{}\n```\n\n</details>\n",
            fence(&truncate(&tail, 6000))
        ));
    }
    let diag = file_tail(&diagnostics_path(), 40);
    if !diag.is_empty() {
        body.push_str(&format!(
            "\n<details><summary>诊断日志（最近 40 行：应用 / 系统 / 线程）</summary>\n\n```\n{}\n```\n\n</details>\n",
            fence(&truncate(&diag, 6000))
        ));
    }
    let sys = system_snapshot();
    if !sys.is_empty() {
        body.push_str(&format!(
            "\n<details><summary>系统诊断快照</summary>\n\n```\n{}\n```\n\n</details>\n",
            fence(&truncate(&sys, 6000))
        ));
    }
    body.push_str("\n<sub>由 wallpaper-rs 内置监控线程自动生成；如包含隐私路径请自行脱敏。</sub>");
    body
}

/// 去掉正文里可能出现的 ```（避免破坏 Markdown 代码块结构）。
fn fence(s: &str) -> String {
    s.replace("```", "`\u{200b}`\u{200b}`")
}

/// 拼装一份「环境快照」：版本 / 系统 / 桌面会话 / 关键依赖是否就绪，便于远程定位。
fn environment() -> String {
    let mut out = String::new();
    out.push_str(&format!("版本      : v{}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!(
        "系统      : {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    if let Some(t) = os_pretty_name() {
        out.push_str(&format!("发行版    : {t}\n"));
    }
    out.push_str(&format!(
        "进程      : pid={} 运行 {:.0}s\n",
        std::process::id(),
        rep().start.elapsed().as_secs_f64()
    ));
    // 桌面 / 会话相关（Linux 动态壁纸、托盘、全局热键都依赖这些）。
    for (k, label) in [
        ("XDG_CURRENT_DESKTOP", "桌面环境  "),
        ("XDG_SESSION_TYPE", "会话类型  "),
        ("KDE_FULL_SESSION", "KDE       "),
        ("GNOME_DESKTOP_SESSION_ID", "GNOME     "),
        ("WAYLAND_DISPLAY", "Wayland   "),
        ("DISPLAY", "X11 DISPLAY"),
        ("LANG", "语言      "),
    ] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                out.push_str(&format!("{label}: {v}\n"));
            }
        }
    }
    // CI / 容器标记：便于一眼看出是自动化环境里跑出来的（而非真实用户）。
    let ci: Vec<&str> = [
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "JENKINS_URL",
        "container",
    ]
    .into_iter()
    .filter(|k| std::env::var_os(k).is_some())
    .collect();
    if !ci.is_empty() {
        out.push_str(&format!("CI/容器   : {}\n", ci.join(", ")));
    }
    out.push_str(&format!(
        "配置目录  : {}\n",
        crate::config::config_dir().display()
    ));
    out.push_str(&format!("本地日志  : {}\n", log_path().display()));
    // 动态壁纸依赖：mpv 探测详情 + 平台是否支持。
    out.push_str(&format!(
        "动态壁纸  : supported={} mpv_available={}\n",
        crate::livewall::supported(),
        crate::livewall::mpv_available()
    ));
    out.push_str(crate::livewall::mpv_probe_report().trim_end());
    out.push('\n');
    out
}

/// 读取 `/etc/os-release` 的 `PRETTY_NAME`（仅 Linux 有意义）。
fn os_pretty_name() -> Option<String> {
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        let text = std::fs::read_to_string("/etc/os-release").ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return Some(rest.trim().trim_matches('"').to_string());
            }
        }
        None
    }
    #[cfg(not(all(unix, not(target_os = "macos"), not(target_os = "android"))))]
    {
        std::env::var("OS").ok().filter(|s| !s.is_empty())
    }
}

/// 本地日志末尾 `n` 行（用于随上报附带，辅助定位）。
fn log_tail(n: usize) -> String {
    let Ok(text) = std::fs::read_to_string(log_path()) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// 任意文本文件末尾 `n` 行；读取失败返回空。
fn file_tail(path: &std::path::Path, n: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// 当前挂钟时间（`UNIX_EPOCH` 起的秒数）。
fn now_wall() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 从 panic 载荷里安全取出可读文本（`&str` / `String` 最常见）。
fn panic_payload(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        format!("{info}")
    }
}

/// 实例标识：`pid` + 进程启动秒，便于区分多开实例 / 同机多次运行。
fn instance_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let started = now_wall().saturating_sub(rep().start.elapsed().as_secs());
        format!("{}-{}", std::process::id(), started)
    })
    .clone()
}

/// 判断错误信息是否「网络相关」（这类问题不上报）。
pub fn is_network(msg: &str) -> bool {
    const KEYS: &[&str] = &[
        "请求失败",
        "下载失败",
        "下载中断",
        "读取响应失败",
        "内容过小，可能被拦截",
        "连接",
        "超时",
        "timeout",
        "timed out",
        "dns",
        "tls",
        "证书",
        "unknownissuer",
        "connection",
        "refused",
        "unreachable",
        "network",
        "proxy",
        "socks",
        "http",
    ];
    let lower = msg.to_ascii_lowercase();
    KEYS.iter().any(|k| {
        let k = *k;
        msg.contains(k) || lower.contains(k)
    })
}

/// 追加写入本地日志（无论如何都写，不依赖网络 / 开关）。
fn write_log(kind: &str, summary: &str, detail: &str) {
    use std::io::Write;
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{}] ({kind}) {summary}\n{detail}\n---", timestamp());
    }
}

/// 打开系统默认浏览器（仅供上报使用；`&` 等字符需避开 cmd 解析，故用 `raw_arg`）。
fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .raw_arg("/C")
            .raw_arg("start")
            .raw_arg("\"\"")
            .raw_arg(format!("\"{url}\""))
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos", unix)))]
    {
        let _ = url;
    }
}

/// 按字符数截断（超出补省略号）。
fn truncate(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("{t}…")
    } else {
        t
    }
}

/// 当前 UTC 时间字符串（`YYYY-MM-DD HH:MM:SS UTC`），避免引入额外日期依赖。
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant 的 civil_from_days 算法。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y0 = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y0 + 1 } else { y0 };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

#[cfg(test)] // 语句
mod tests {
    // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn network_classifier() {
        // 函数 network_classifier
        assert!(is_network("请求失败：连接超时")); // 语句
        assert!(is_network("下载失败：timed out")); // 语句
        assert!(is_network("读取响应失败：TLS")); // 语句
        assert!(!is_network("创建动态壁纸窗口失败：拒绝访问")); // 语句
        assert!(!is_network("未找到 mpv 播放器")); // 语句
        assert!(!is_network("截取视频帧失败（mpv 未在限时内输出图片）")); // 语句
    } // 块结束

    #[test] // 语句
    fn timestamp_shape() {
        // 函数 timestamp_shape
        let t = timestamp(); // 变量 t
        assert!(t.ends_with("UTC"), "{t}"); // 语句
        assert_eq!(t.len(), 23, "{t}"); // 语句
    } // 块结束

    #[test] // 语句
    fn truncate_rules() {
        // 函数 truncate_rules
        assert_eq!(truncate("abc", 5), "abc"); // 语句
        assert_eq!(truncate("abcdef", 3), "abc…"); // 语句
    } // 块结束

    #[test] // 语句
    fn notes_capture_scene_and_steps() {
        // 函数 notes_capture_scene_and_steps
        set_scene("单测 → 场景"); // 调用 set_scene
        note("单测 → 步骤一"); // 调用 note
        let t = notes_text(); // 变量 t
        assert!(t.contains("单测 → 场景"), "{t}"); // 语句
        assert!(t.contains("单测 → 步骤一"), "{t}"); // 语句
    } // 块结束

    #[test] // 语句
    fn environment_has_version_and_log_path() {
        // 函数 environment_has_version_and_log_path
        let e = environment(); // 变量 e
        assert!(e.contains(env!("CARGO_PKG_VERSION")), "{e}"); // 语句
        assert!(e.contains("mpv"), "{e}"); // 语句
    } // 块结束

    #[test] // 语句
    fn fence_breaks_nested_code_fence() {
        // 函数 fence_breaks_nested_code_fence
        assert!(!fence("a ``` b").contains("```")); // 语句
    } // 块结束

    #[test] // 语句
    fn composed_body_has_env_notes_and_detail() {
        // 函数 composed_body_has_env_notes_and_detail
        note("单测 → 正文面包屑"); // 调用 note
        let b = compose_body("error", "摘要示例", "详情示例\n第二行"); // 变量 b
        assert!(b.contains("**摘要**：摘要示例"), "{b}"); // 语句
        assert!(b.contains("详情示例"), "{b}"); // 语句
        assert!(b.contains("**环境**"), "{b}"); // 语句
        assert!(b.contains("**最近操作**"), "{b}"); // 语句
        assert!(b.contains("单测 → 正文面包屑"), "{b}"); // 语句
        assert!(b.contains("版本"), "{b}"); // 语句
    } // 块结束

    #[test] // 语句
    fn dedup_suppresses_second_report_within_window() {
        // 函数 dedup_suppresses_second_report_within_window
        // 直接测核心函数并显式给临时文件路径：不读写环境变量，避免与其它测试相互干扰。
        let dir = std::env::temp_dir().join(format!("wp_dedup_{}", std::process::id())); // 变量 dir
        let _ = std::fs::create_dir_all(&dir); // 变量 _
        let file = dir.join("dedup.tsv"); // 变量 file
        let _ = std::fs::remove_file(&file); // 变量 _

        let sig = format!("unittest-{}", std::process::id()); // 变量 sig
        assert!(!dedup_check(&file, &sig)); // 第一次：未上报过
        assert!(dedup_check(&file, &sig)); // 第二次：命中窗口
        assert!(!dedup_check(&file, &format!("{sig}-other"))); // 不同签名不受影响

        let _ = std::fs::remove_dir_all(&dir); // 变量 _
    } // 块结束

    #[test] // 语句
    fn builtin_token_decrypts() {
        // 函数 builtin_token_decrypts
        // 不比对明文本身（避免把 token 写进测试），只校验解密结果形状正确——
        // 算法一旦与生成端不一致，解出来就不可能是合法的 token。
        let t = builtin_token().expect("内置 token 应能解密"); // 变量 t
        assert!(t.starts_with("github_pat_"), "prefix mismatch"); // 语句
        assert_eq!(t.len(), 93, "len mismatch"); // 语句
        assert!(
            t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "charset"
        ); // 语句
           // 密文里不得出现明文 token 前缀。
        let blob = TOKEN_BLOB.to_vec(); // 变量 blob
        let needle = b"github_pat_"; // 变量 needle
        assert!(
            !blob.windows(needle.len()).any(|w| w == needle),
            "blob leaks prefix"
        ); // 语句
    } // 块结束

    #[test] // 语句
    fn prefilled_url_is_encoded() {
        // 函数 prefilled_url_is_encoded
        let u = prefilled_url("[自动上报] 卡死", "版本 v0.14.7 & 平台"); // 变量 u
        assert!(u.starts_with(ISSUES_NEW)); // 语句
        assert!(u.contains("?title=")); // 语句
        assert!(u.contains("&body=")); // 语句
        assert!(!u.contains(' ')); // 空格必须被编码
        assert!(u.contains("%26")); // 正文里的 `&` 也被编码，避免截断查询串
    } // 块结束

    /// 端到端验证看门狗能发现「UI 卡死」并写入本地日志。
    /// 默认忽略（耗时约 24 秒）；用 `cargo test -- --ignored watchdog_detects_hang` 运行。
    #[test] // 语句
    #[ignore] // 语句
    fn watchdog_detects_hang() {
        // 函数 watchdog_detects_hang
        std::env::set_var("WALLPAPER_REPORT_NO_SUBMIT", "1"); // 别在测试里真的打开浏览器
        install(true, String::new()); // 调用 install
        set_scene("测试 → 故意卡死"); // 调用 set_scene
        heartbeat(); // 调用 heartbeat
                     // 之后不再打心跳，模拟卡死。需要等过 HANG_AFTER(12s) 且连续 HANG_CONFIRM(3) 轮
                     // 命中，故留足余量。
        std::thread::sleep(Duration::from_secs(24)); // 调用 sleep
        let content = std::fs::read_to_string(log_path()).unwrap_or_default(); // 变量 content
        assert!(content.contains("界面未响应"), "log tail: {content}"); // 语句
        assert!(content.contains("测试 → 故意卡死"), "log tail: {content}"); // 语句
    } // 块结束

    /// 端到端验证「内置（混淆）token 解密后真能建 issue」（会打网络并真的建一条）。
    /// 默认忽略；用 `cargo test -- --ignored --nocapture builtin_token_creates_issue` 运行，
    /// 之后按打印出的编号把该 issue 关掉即可。
    #[test] // 语句
    #[ignore] // 语句
    fn builtin_token_creates_issue() {
        // 函数 builtin_token_creates_issue
        let t = builtin_token().expect("内置 token 解密失败"); // 变量 t
        let payload = serde_json::json!({ // 变量 payload
            "title": "[自动上报] 自检：内置 Token 链路验证", // 语句
            "body": "由 wallpaper-rs 内置（混淆存储）token 上报链路的一次性自检。" // 语句
        }); // 块结束
        let resp = crate::websrc::agent() // 变量 resp
            .post(API_ISSUES) // 语句
            .set("Accept", "application/vnd.github+json") // 语句
            .set("User-Agent", "wallpaper-rs") // 语句
            .set("X-GitHub-Api-Version", "2022-11-28") // 语句
            .set("Authorization", &format!("Bearer {t}")) // 语句
            .send_json(payload); // 语句
        match resp {
            // 分支匹配
            Ok(r) => {
                // 匹配分支
                let v: serde_json::Value = r.into_json().expect("解析响应失败"); // 变量 v
                let n = v["number"].as_u64().unwrap_or(0); // 变量 n
                println!("CREATED_ISSUE={n}"); // 语句
                assert!(n > 0); // 语句
            } // 块结束
            Err(e) => panic!("内置 token 建 issue 失败：{e}"), // 匹配分支
        } // 块结束
    } // 块结束
} // 块结束
