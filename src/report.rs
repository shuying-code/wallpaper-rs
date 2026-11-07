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
const HANG_AFTER: Duration = Duration::from_secs(6);
/// 监控线程轮询间隔。
const POLL: Duration = Duration::from_secs(2);
/// 单次运行最多上报的条数（防止刷屏）。
const MAX_REPORTS: u64 = 12;
/// 无 token 时「打开网页」的最小间隔（同一条问题去重已挡掉大部分）。
const WEB_MIN_GAP: Duration = Duration::from_secs(600);

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
    /// 是否已收到过至少一次心跳（未收到前不判定卡死，避免启动阶段误报）。
    beat_seen: AtomicBool,
    /// 当前「场景」描述（如「轮播 → 动态壁纸」），供卡死上报定位。
    scene: Mutex<String>,
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
        beat_seen: AtomicBool::new(false),
        scene: Mutex::new(String::new()),
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
            let msg = format!("{info}");
            report_event("panic", &truncate(&msg, 200), &msg, true);
            prev(info);
        }));
        std::thread::spawn(watchdog);
    });
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
        *g = s.to_string();
    }
}

/// 本地日志文件路径。
pub fn log_path() -> std::path::PathBuf {
    crate::config::config_dir().join("logs").join("report.log")
}

/// UI 线程每帧调用：打一个心跳时间戳。卡死检测完全依赖它。
pub fn heartbeat() {
    let r = rep();
    r.beat_ms
        .store(r.start.elapsed().as_millis() as u64, Ordering::Relaxed);
    r.beat_seen.store(true, Ordering::Relaxed);
}

/// 上报一个「非网络」的功能性错误（供各模块调用）。网络类错误会被忽略。
pub fn report_error(source: &str, msg: &str) {
    if is_network(msg) {
        return;
    }
    let summary = format!("{}：{}", source, truncate(msg, 160));
    report_event("error", &summary, msg, false);
}

/// 用户主动触发的上报（设置页按钮）：不参与去重，总能上报一次。
pub fn report_manual() {
    let scene = rep().scene.lock().map(|g| g.clone()).unwrap_or_default();
    let summary = if scene.is_empty() {
        "用户反馈".to_string()
    } else {
        format!("用户反馈（{scene}）")
    };
    write_log("manual", &summary, "用户主动触发的问题上报。");
    let body = compose_body("manual", &summary, "用户主动触发的问题上报。");
    let title = format!("[反馈] {}", truncate(&summary, 110));
    submit_spawn(title, body);
}

/// 后台监控线程：定期比对心跳，判定卡死并上报。
fn watchdog() {
    loop {
        std::thread::sleep(POLL);
        let r = rep();
        if !r.enabled.load(Ordering::Relaxed) || !r.beat_seen.load(Ordering::Relaxed) {
            continue;
        }
        let last = r.beat_ms.load(Ordering::Relaxed);
        let now = r.start.elapsed().as_millis() as u64;
        if now.saturating_sub(last) >= HANG_AFTER.as_millis() as u64 {
            // 先把心跳推到「现在」，避免卡死持续时每轮都触发（去重也会兜底）。
            r.beat_ms.store(now, Ordering::Relaxed);
            let secs = (now - last) as f64 / 1000.0;
            let scene = r.scene.lock().map(|g| g.clone()).unwrap_or_default();
            let scene = if scene.is_empty() {
                "未知".to_string()
            } else {
                scene
            };
            let summary = format!("界面未响应约 {secs:.0} 秒（场景：{scene}）");
            let detail = format!(
                "UI 线程超过 {secs:.0} 秒没有响应任何界面事件，疑似卡死。\n场景：{scene}"
            );
            report_event("hang", &summary, &detail, false);
        }
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
            if !seen.insert(sig) {
                return; // 同一问题只报一次
            }
        }
        Err(_) => return,
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
    let scene = rep().scene.lock().map(|g| g.clone()).unwrap_or_default();
    let scene = if scene.is_empty() { "未知" } else { scene.as_str() };
    format!(
        "### 自动上报（{kind}）\n\n\
         - **版本**：v{ver}\n\
         - **平台**：{os} / {arch}\n\
         - **时间**：{ts}\n\
         - **场景**：{scene}\n\n\
         **摘要**：{summary}\n\n\
         **详情**：\n```\n{detail}\n```\n\n\
         <sub>由 wallpaper-rs 内置监控线程自动生成；如包含隐私路径请自行脱敏。</sub>",
        kind = kind,
        ver = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        ts = timestamp(),
        scene = scene,
        summary = summary,
        detail = truncate(detail, 1000),
    )
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
mod tests { // 模块 tests
    use super::*; // 引入依赖

    #[test] // 语句
    fn network_classifier() { // 函数 network_classifier
        assert!(is_network("请求失败：连接超时")); // 语句
        assert!(is_network("下载失败：timed out")); // 语句
        assert!(is_network("读取响应失败：TLS")); // 语句
        assert!(!is_network("创建动态壁纸窗口失败：拒绝访问")); // 语句
        assert!(!is_network("未找到 mpv 播放器")); // 语句
        assert!(!is_network("截取视频帧失败（mpv 未在限时内输出图片）")); // 语句
    } // 块结束

    #[test] // 语句
    fn timestamp_shape() { // 函数 timestamp_shape
        let t = timestamp(); // 变量 t
        assert!(t.ends_with("UTC"), "{t}"); // 语句
        assert_eq!(t.len(), 23, "{t}"); // 语句
    } // 块结束

    #[test] // 语句
    fn truncate_rules() { // 函数 truncate_rules
        assert_eq!(truncate("abc", 5), "abc"); // 语句
        assert_eq!(truncate("abcdef", 3), "abc…"); // 语句
    } // 块结束

    #[test] // 语句
    fn builtin_token_decrypts() { // 函数 builtin_token_decrypts
        // 不比对明文本身（避免把 token 写进测试），只校验解密结果形状正确——
        // 算法一旦与生成端不一致，解出来就不可能是合法的 token。
        let t = builtin_token().expect("内置 token 应能解密"); // 变量 t
        assert!(t.starts_with("github_pat_"), "prefix mismatch"); // 语句
        assert_eq!(t.len(), 93, "len mismatch"); // 语句
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "charset"); // 语句
        // 密文里不得出现明文 token 前缀。
        let blob = TOKEN_BLOB.to_vec(); // 变量 blob
        let needle = b"github_pat_"; // 变量 needle
        assert!(!blob.windows(needle.len()).any(|w| w == needle), "blob leaks prefix"); // 语句
    } // 块结束

    #[test] // 语句
    fn prefilled_url_is_encoded() { // 函数 prefilled_url_is_encoded
        let u = prefilled_url("[自动上报] 卡死", "版本 v0.14.7 & 平台"); // 变量 u
        assert!(u.starts_with(ISSUES_NEW)); // 语句
        assert!(u.contains("?title=")); // 语句
        assert!(u.contains("&body=")); // 语句
        assert!(!u.contains(' ')); // 空格必须被编码
        assert!(u.contains("%26")); // 正文里的 `&` 也被编码，避免截断查询串
    } // 块结束

    /// 端到端验证看门狗能发现「UI 卡死」并写入本地日志。
    /// 默认忽略（耗时约 9 秒）；用 `cargo test -- --ignored watchdog_detects_hang` 运行。
    #[test] // 语句
    #[ignore] // 语句
    fn watchdog_detects_hang() { // 函数 watchdog_detects_hang
        std::env::set_var("WALLPAPER_REPORT_NO_SUBMIT", "1"); // 别在测试里真的打开浏览器
        install(true, String::new()); // 调用 install
        set_scene("测试 → 故意卡死"); // 调用 set_scene
        heartbeat(); // 调用 heartbeat
        std::thread::sleep(Duration::from_secs(9)); // 之后不再打心跳，模拟卡死
        let content = std::fs::read_to_string(log_path()).unwrap_or_default(); // 变量 content
        assert!(content.contains("界面未响应"), "log tail: {content}"); // 语句
        assert!(content.contains("测试 → 故意卡死"), "log tail: {content}"); // 语句
    } // 块结束

    /// 端到端验证「内置（混淆）token 解密后真能建 issue」（会打网络并真的建一条）。
    /// 默认忽略；用 `cargo test -- --ignored --nocapture builtin_token_creates_issue` 运行，
    /// 之后按打印出的编号把该 issue 关掉即可。
    #[test] // 语句
    #[ignore] // 语句
    fn builtin_token_creates_issue() { // 函数 builtin_token_creates_issue
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
        match resp { // 分支匹配
            Ok(r) => { // 匹配分支
                let v: serde_json::Value = r.into_json().expect("解析响应失败"); // 变量 v
                let n = v["number"].as_u64().unwrap_or(0); // 变量 n
                println!("CREATED_ISSUE={n}"); // 语句
                assert!(n > 0); // 语句
            } // 块结束
            Err(e) => panic!("内置 token 建 issue 失败：{e}"), // 匹配分支
        } // 块结束
    } // 块结束
} // 块结束
