//! Windows 动态壁纸宿主。
//!
//! Windows 没有把视频设为桌面壁纸的原生 API，因此这里自建一个宿主：
//! 1. 通过 `Progman` 的 `0x052C` 消息让桌面生成/刷新 `WorkerW`，找到承载桌面图标
//!    (`SHELLDLL_DefView`) 的那个 `WorkerW` 之后的同级 `WorkerW` —— 即图标层「下面」
//!    的那层窗口；
//! 2. 在它下面 `CreateWindowExW` 一个无边框子窗口，铺满整个虚拟桌面；
//! 3. 调用 **mpv**（`--wid=<hwnd>`）把 MP4 循环渲染进该窗口，实现动态壁纸。
//!
//! 宿主窗口属于本进程，因此动态壁纸会在 App 运行期间常驻（App 支持托盘常驻后台）。
//! mpv 通过 PATH / 常见安装目录自动查找，也可用环境变量 `WALLPAPER_MPV` 指定路径。
//! 非 Windows 平台仅提供「不支持」的空实现。

#[cfg(not(windows))]
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub use imp::{
    active_video, ensure_mpv, mpv_available, poster_frame, set_live_wallpaper, stop, supported,
};

#[cfg(not(windows))]
pub fn supported() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_live_wallpaper(_video: &Path) -> Result<(), String> {
    Err("动态壁纸宿主目前仅支持 Windows".to_string())
}

#[cfg(not(windows))]
pub fn stop() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn active_video() -> Option<PathBuf> {
    None
}

/// 从视频中截取一帧作为静态海报图（PNG），用于「动态壁纸锁屏」。
///
/// Windows 锁屏只能设置静态图片、无法直接播放视频，因此这里用 mpv 把视频的
/// 一帧导出为图片，再交给锁屏设置逻辑。
#[cfg(not(windows))]
pub fn poster_frame(_video: &Path, _out: &Path) -> Result<(), String> {
    Err("动态壁纸锁屏目前仅支持 Windows".to_string())
}

/// 非 Windows：没有动态壁纸宿主，也就没有 mpv。
#[cfg(not(windows))]
pub fn mpv_available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn ensure_mpv() -> Result<PathBuf, String> {
    Err("动态壁纸目前仅支持 Windows".to_string())
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::sync::{Mutex, OnceLock};

    type Hwnd = isize;

    /// 当前宿主：桌面层窗口 + 播放它的 mpv 子进程。
    struct Host {
        window: Hwnd,
        child: Child,
        video: PathBuf,
    }

    static HOST: Mutex<Option<Host>> = Mutex::new(None);

    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;

    const WS_CHILD: u32 = 0x4000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
    const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const HWND_BOTTOM: isize = 1;
    const SMTO_NORMAL: u32 = 0x0000;
    const WM_SPAWN_WORKER: u32 = 0x052C;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    const CLASS_NAME: &str = "WallpaperRsLiveHost";

    pub fn supported() -> bool {
        true
    }

    /// 当前正在播放的动态壁纸（mpv 若已自行退出则清空并返回 `None`）。
    pub fn active_video() -> Option<PathBuf> {
        let mut guard = HOST.lock().ok()?;
        if let Some(h) = guard.as_mut() {
            match h.child.try_wait() {
                Ok(Some(_)) => {
                    unsafe { destroy_window(h.window) };
                    *guard = None;
                }
                _ => return Some(h.video.clone()),
            }
        }
        None
    }

    /// 停止动态壁纸：结束 mpv 并销毁桌面层窗口。
    pub fn stop() -> Result<(), String> {
        let mut guard = HOST.lock().map_err(|_| "动态壁纸状态锁失效".to_string())?;
        if let Some(mut h) = guard.take() {
            let _ = h.child.kill();
            let _ = h.child.wait();
            unsafe { destroy_window(h.window) };
        }
        Ok(())
    }

    /// 设为动态壁纸：准备宿主窗口并用 mpv 播放 `video`。
    pub fn set_live_wallpaper(video: &Path) -> Result<(), String> {
        if !video.is_file() {
            return Err(format!("视频文件不存在：{}", video.display()));
        }
        let mpv = find_mpv().ok_or_else(|| {
            "未找到 mpv 播放器：请安装 mpv，或用环境变量 WALLPAPER_MPV 指定 mpv.exe 路径".to_string()
        })?;
        // 先清理上一个宿主（含旧窗口与旧 mpv）。
        stop()?;
        let window = unsafe { create_host_window() }?;
        let child = spawn_mpv(&mpv, window, video)?;
        let mut guard = HOST.lock().map_err(|_| "动态壁纸状态锁失效".to_string())?;
        *guard = Some(Host {
            window,
            child,
            video: video.to_path_buf(),
        });
        Ok(())
    }

    /// 从视频中截取一帧作为静态海报图（PNG），用于「动态壁纸锁屏」。
    ///
    /// 用 mpv 抽取：`mpv --no-config --no-audio --o=<out> --frames=1 --start=<secs> <video>`。
    /// `--start` 跳过开头可能存在的黑帧；输出格式由 `--o` 的扩展名推断（`.png` → PNG）。
    pub fn poster_frame(video: &Path, out: &Path) -> Result<(), String> {
        if !video.is_file() {
            return Err(format!("视频文件不存在：{}", video.display()));
        }
        let mpv = find_mpv().ok_or_else(|| {
            "未找到 mpv 播放器：请安装 mpv，或用环境变量 WALLPAPER_MPV 指定 mpv.exe 路径".to_string()
        })?;
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("创建海报目录失败：{e}"))?;
        }
        let _ = std::fs::remove_file(out);
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new(mpv);
        cmd.arg("--no-config")
            .arg("--no-audio")
            .arg("--no-osc")
            .arg("--no-input-default-bindings")
            .arg(format!("--o={}", out.display()))
            .arg("--frames=1")
            .arg("--start=1.0")
            .arg(video);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| format!("启动 mpv 失败：{e}"))?;
        let status = child.wait().map_err(|e| format!("等待 mpv 失败：{e}"))?;
        if out.is_file() {
            Ok(())
        } else {
            Err(format!(
                "截取视频帧失败（mpv 退出码 {:?}）",
                status.code()
            ))
        }
    }

    /// 在桌面层创建铺满虚拟桌面的无边框子窗口。
    ///
    /// 依次尝试候选父窗口：经典 `WorkerW` 图标层 → `Progman` 桌面窗口，用第一个
    /// 创建成功的。部分系统（如 Win11 24H2，桌面图标直接由 `Progman` 承载）不允许
    /// 跨进程把子窗口挂到 `WorkerW`（返回「拒绝访问」），但 `Progman` 可以，因此
    /// 不能写死为 `WorkerW`。创建成功后把窗口压到同级最底部——位于桌面图标之下，
    /// 只在壁纸上显示，不遮挡图标。
    unsafe fn create_host_window() -> Result<Hwnd, String> {
        spawn_worker();
        let inst = GetModuleHandleW(std::ptr::null());
        ensure_class(inst);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);
        let class = to_wide(CLASS_NAME);
        let mut parents: Vec<Hwnd> = Vec::new();
        if let Some(wk) = classic_workerw() {
            parents.push(wk);
        }
        let progman = progman_window();
        if progman != 0 {
            parents.push(progman);
        }
        let mut last = std::io::Error::from_raw_os_error(0);
        for parent in parents {
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                class.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                w,
                h,
                parent,
                0,
                inst,
                std::ptr::null_mut(),
            );
            if hwnd != 0 {
                // 关键：把宿主窗口插到「桌面图标层」的正下方——位于壁纸之上、图标之下。
                // 不能直接压到同级最底部（HWND_BOTTOM）：Win11 上桌面壁纸由 Progman
                // 的同级 WorkerW 绘制，压到底部会被壁纸盖住——mpv 明明在跑、桌面却看不到
                // 动态壁纸（即「轮播动态壁纸不生效 / 原视频不可用」）。经典方案里父窗口
                // 本身就是图标后的 WorkerW，找不到 SHELLDLL_DefView，退回底部即可。
                let icons = FindWindowExW(
                    parent,
                    0,
                    to_wide("SHELLDLL_DefView").as_ptr(),
                    std::ptr::null(),
                );
                let insert_after = if icons != 0 {
                    icons as *mut c_void
                } else {
                    HWND_BOTTOM as *mut c_void
                };
                SetWindowPos(
                    hwnd as *mut c_void,
                    insert_after,
                    0,
                    0,
                    w,
                    h,
                    SWP_NOACTIVATE,
                );
                return Ok(hwnd);
            }
            last = std::io::Error::last_os_error();
        }
        Err(format!("创建动态壁纸窗口失败：{last}"))
    }

    unsafe fn destroy_window(hwnd: Hwnd) {
        if hwnd != 0 && IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
    }

    /// 注册窗口类（进程内只注册一次）。
    fn ensure_class(inst: Hwnd) {
        static READY: OnceLock<()> = OnceLock::new();
        READY.get_or_init(|| unsafe {
            let class = to_wide(CLASS_NAME);
            let mut wc: WndClassExW = std::mem::zeroed();
            wc.cb_size = std::mem::size_of::<WndClassExW>() as u32;
            wc.lpfn_wndproc = Some(wnd_proc);
            wc.h_instance = inst;
            wc.lpsz_class_name = class.as_ptr();
            RegisterClassExW(&wc);
        });
    }

    unsafe extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    /// 让 `Progman` 生成/刷新 `WorkerW`（经典桌面图标层的宿主）。
    unsafe fn spawn_worker() {
        let progman = progman_window();
        if progman != 0 {
            let mut res: usize = 0;
            SendMessageTimeoutW(progman, WM_SPAWN_WORKER, 0, 0, SMTO_NORMAL, 1000, &mut res);
        }
    }

    /// 经典方案：`SHELLDLL_DefView` 所在窗口「之后」的同级 `WorkerW`（图标层宿主）。
    unsafe fn classic_workerw() -> Option<Hwnd> {
        let mut found: Hwnd = 0;
        EnumWindows(Some(enum_proc), &mut found as *mut Hwnd as isize);
        (found != 0).then_some(found)
    }

    /// 桌面窗口 `Progman`：跨进程挂子窗口最兼容的宿主。
    unsafe fn progman_window() -> Hwnd {
        FindWindowW(to_wide("Progman").as_ptr(), std::ptr::null())
    }

    unsafe extern "system" fn enum_proc(top: Hwnd, lparam: isize) -> i32 {
        let out = lparam as *mut Hwnd;
        let defview = FindWindowExW(
            top,
            0,
            to_wide("SHELLDLL_DefView").as_ptr(),
            std::ptr::null(),
        );
        if defview != 0 {
            let worker = FindWindowExW(0, top, to_wide("WorkerW").as_ptr(), std::ptr::null());
            if worker != 0 {
                *out = worker;
                return 0; // 找到即停止枚举
            }
        }
        1
    }

    /// 查找 mpv：环境变量 → PATH → 常见安装目录。
    fn find_mpv() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("WALLPAPER_MPV") {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Some(pb);
            }
        }
        // 随程序捆绑的 mpv：`<exe 所在目录>/mpv/mpv.exe` 或同目录的 `mpv.exe`。
        // 便携包把 mpv 放在 exe 旁边的 `mpv/` 子目录，优先于系统 PATH 使用。
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let bundled = dir.join("mpv").join("mpv.exe");
                if bundled.is_file() {
                    return Some(bundled);
                }
                let sibling = dir.join("mpv.exe");
                if sibling.is_file() {
                    return Some(sibling);
                }
            }
        }
        if let Some(p) = which_in_path("mpv.exe") {
            return Some(p);
        }
        let mut cands: Vec<PathBuf> = Vec::new();
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            cands.push(local.join("Programs").join("mpv").join("mpv.exe"));
            cands.push(local.join("Microsoft").join("WinGet").join("Links").join("mpv.exe"));
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            cands.push(
                PathBuf::from(profile)
                    .join("scoop")
                    .join("shims")
                    .join("mpv.exe"),
            );
        }
        cands.push(PathBuf::from(r"C:\Program Files\mpv\mpv.exe"));
        cands.push(PathBuf::from(r"C:\Program Files (x86)\mpv\mpv.exe"));
        cands.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin\mpv.exe"));
        if let Some(p) = cands.into_iter().find(|p| p.is_file()) {
            return Some(p);
        }
        // 按需下载的 mpv 缓存（见 ensure_mpv）：已解压到用户缓存目录时直接使用。
        if let Some(p) = cached_mpv() {
            return Some(p);
        }
        None
    }

    /// 缓存的 mpv 目录：`%LOCALAPPDATA%\wallpaper-rs\mpv-runtime`。
    fn mpv_cache_dir() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))?;
        Some(base.join("wallpaper-rs").join("mpv-runtime"))
    }

    /// 之前由 [`ensure_mpv`] 下载并解压好的 mpv（若存在）。
    fn cached_mpv() -> Option<PathBuf> {
        let dir = mpv_cache_dir()?;
        [dir.join("mpv.exe"), dir.join("mpv").join("mpv.exe")]
            .into_iter()
            .find(|p| p.is_file())
    }

    /// mpv 是否已就绪（本机已安装、PATH 中可用，或已下载到缓存）。
    pub fn mpv_available() -> bool {
        find_mpv().is_some()
    }

    /// 确保有可用的 mpv：本机已安装则直接返回；否则从多个候选地址（官方直连 + 多个
    /// GitHub 加速镜像，见 [`mpv_download_urls`]）下载便携版并解压到用户缓存目录
    /// （`%LOCALAPPDATA%\wallpaper-rs\mpv-runtime`），不随安装包分发。
    ///
    /// **阻塞调用**（含网络下载与解压），应在后台线程中执行。
    pub fn ensure_mpv() -> Result<PathBuf, String> {
        if let Some(p) = find_mpv() {
            return Ok(p);
        }
        let dir = mpv_cache_dir().ok_or_else(|| "无法确定 mpv 缓存目录".to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建 mpv 缓存目录失败：{e}"))?;
        let archive = dir.join("mpv-download.7z");
        // 先并发探测各地址的响应速度，挑最快的那个下载；其余作为兜底。
        let urls = order_by_speed(mpv_download_urls());
        download_from_any(&urls, &archive)?;
        extract_7z(&archive, &dir)?;
        let _ = std::fs::remove_file(&archive);
        // 只保留运行所需文件，删掉文档 / 安装脚本 / 更新器，省空间。
        for junk in [
            "doc",
            "installer",
            "mpv-register.bat",
            "mpv-unregister.bat",
            "updater.bat",
        ] {
            let p = dir.join(junk);
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
        cached_mpv().ok_or_else(|| "下载完成但未找到 mpv.exe".to_string())
    }

    /// 解析便携 mpv 的下载地址：优先 GitHub 最新发行版中的 x86_64 `.7z`（排除 `-v3-`
    /// 变体以保证老 CPU 兼容），失败时回退到内置的固定版本地址。
    fn resolve_mpv_url() -> String {
        const FALLBACK: &str =
            "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20261107/mpv-x86_64-20261107-git-9028187538.7z";
        let call = ureq::get(
            "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest",
        )
        .set("User-Agent", "wallpaper-rs")
        .timeout(std::time::Duration::from_secs(30))
        .call();
        if let Ok(resp) = call {
            if let Ok(v) = resp.into_json::<serde_json::Value>() {
                if let Some(assets) = v.get("assets").and_then(|a| a.as_array()) {
                    for a in assets {
                        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if name.starts_with("mpv-x86_64-")
                            && name.ends_with(".7z")
                            && !name.contains("-v3-")
                        {
                            if let Some(u) = a.get("browser_download_url").and_then(|u| u.as_str())
                            {
                                return u.to_string();
                            }
                        }
                    }
                }
            }
        }
        FALLBACK.to_string()
    }

    /// 国内常用的 GitHub 加速镜像前缀（均为「前缀 + 原始 GitHub 地址」形式）。
    /// 直连 GitHub 缓慢 / 被墙时自动改用这些镜像；已按实测可用性与速度排序。
    const GH_PROXIES: &[&str] = &[
        "https://ghfast.top/",
        "https://gh-proxy.com/",
        "https://hk.gh-proxy.com/",
        "https://ghproxy.net/",
        "https://mirror.ghproxy.com/",
    ];

    /// mpv 的候选下载地址列表：
    /// 1. 环境变量 `WALLPAPER_MPV_URL` 指定的自定义地址（若设置）；
    /// 2. 多个 GitHub 加速镜像（同一个文件的代理地址）；
    /// 3. 官方直连地址。
    ///
    /// 这里只给出候选集合；实际下载前会由 [`order_by_speed`] **并发测速**，
    /// 按「最快 → 最慢」排序，优先用最快的地址下载，失败再顺次回退。
    fn mpv_download_urls() -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        if let Ok(u) = std::env::var("WALLPAPER_MPV_URL") {
            let u = u.trim().to_string();
            if !u.is_empty() {
                urls.push(u);
            }
        }
        let direct = resolve_mpv_url();
        // 镜像前缀 + 原始地址（这些代理站点会把请求转发到 GitHub）。
        for p in GH_PROXIES {
            urls.push(format!("{p}{direct}"));
        }
        // 官方直连兜底。
        urls.push(direct);
        urls
    }

    /// 探测单个地址的速度：发一个只取前 4 KB 的 Range 请求，返回「从发起请求到
    /// 收完这一小段」的耗时。只有当返回内容确实是 mpv 的 7z 开头（魔数校验通过）
    /// 才视为可用；超时 / 出错 / 返回错误页都返回 `None`。
    fn probe_speed(url: &str) -> Option<std::time::Duration> {
        use std::io::Read;
        let agent = ureq::AgentBuilder::new()
            .user_agent("wallpaper-rs")
            .timeout_connect(std::time::Duration::from_secs(6))
            .timeout_read(std::time::Duration::from_secs(8))
            .build();
        let t0 = std::time::Instant::now();
        let resp = agent.get(url).set("Range", "bytes=0-4095").call().ok()?;
        let mut buf = Vec::new();
        resp.into_reader()
            .take(4096)
            .read_to_end(&mut buf)
            .ok()?;
        (buf.len() >= 6 && buf[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C])
            .then(|| t0.elapsed())
    }

    /// 并发探测所有候选地址，按速度从快到慢排序返回。探测失败的地址（可能只是
    /// 瞬时抖动或不允许 Range）排在最后，仍作为下载兜底。
    fn order_by_speed(urls: Vec<String>) -> Vec<String> {
        if urls.len() <= 1 {
            return urls;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        for (i, u) in urls.iter().enumerate() {
            let u = u.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send((i, probe_speed(&u)));
            });
        }
        drop(tx);
        let mut ok: Vec<(usize, std::time::Duration)> = Vec::new();
        let mut bad: Vec<usize> = Vec::new();
        for (i, d) in rx {
            match d {
                Some(dt) => ok.push((i, dt)),
                None => bad.push(i),
            }
        }
        ok.sort_by_key(|(_, dt)| *dt);
        let mut out: Vec<String> = ok.into_iter().map(|(i, _)| urls[i].clone()).collect();
        for i in bad {
            out.push(urls[i].clone());
        }
        out
    }

    /// 依次尝试 `urls` 中的地址下载到 `dest`，任意一个成功即返回。
    fn download_from_any(urls: &[String], dest: &Path) -> Result<(), String> {
        if urls.is_empty() {
            return Err("没有可用的 mpv 下载地址".to_string());
        }
        let mut last = String::new();
        for url in urls {
            match download_file(url, dest) {
                Ok(()) => {
                    // 某些镜像在出错时会返回 HTTP 200 的错误页；只认真正是 7z 的内容。
                    if is_7z(dest) {
                        return Ok(());
                    }
                    last = format!("下载内容不是有效的 7z 压缩包（{url}）");
                    let _ = std::fs::remove_file(dest);
                }
                Err(e) => {
                    // 清掉半截文件，避免影响下一次尝试。
                    let _ = std::fs::remove_file(dest.with_extension("part"));
                    last = e;
                }
            }
        }
        Err(format!("所有 mpv 下载地址均失败（最后错误：{last}）"))
    }

    /// 判断文件是否为 7z 压缩包（魔数 `37 7A BC AF 27 1C`）。
    fn is_7z(path: &Path) -> bool {
        use std::io::Read;
        let mut buf = [0u8; 6];
        match std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut buf)) {
            Ok(()) => buf == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C],
            Err(_) => false,
        }
    }

    /// 下载 `url` 到 `dest`（先写 `.part` 再改名，避免半截文件被误用）。
    fn download_file(url: &str, dest: &Path) -> Result<(), String> {
        // 用「单次读取超时」而非「整体超时」：慢速网络下 30 MB 可能要十几分钟，
        // 整体超时会把正常下载也掐断；只在数据停滞（单次读取超时）时才算失败。
        let agent = ureq::AgentBuilder::new()
            .user_agent("wallpaper-rs")
            .timeout_connect(std::time::Duration::from_secs(30))
            .timeout_read(std::time::Duration::from_secs(120))
            .timeout(std::time::Duration::from_secs(3600))
            .build();
        let resp = agent
            .get(url)
            .call()
            .map_err(|e| format!("下载 mpv 失败：{e}"))?;
        let mut reader = resp.into_reader();
        let tmp = dest.with_extension("part");
        let mut file =
            std::fs::File::create(&tmp).map_err(|e| format!("创建下载文件失败：{e}"))?;
        std::io::copy(&mut reader, &mut file).map_err(|e| format!("写入下载文件失败：{e}"))?;
        drop(file);
        std::fs::rename(&tmp, dest).map_err(|e| format!("保存下载文件失败：{e}"))?;
        Ok(())
    }

    /// 用系统自带的 `tar.exe`（bsdtar / libarchive，Windows 10 1803+ 内置，支持 7z）
    /// 解压 `.7z` 到 `dir`。
    fn extract_7z(archive: &Path, dir: &Path) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("tar");
        cmd.arg("-xf").arg(archive).arg("-C").arg(dir);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let status = cmd
            .status()
            .map_err(|e| format!("调用系统 tar 解压失败：{e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("解压 mpv 失败（系统 tar 无法处理该压缩包）".to_string())
        }
    }

    fn which_in_path(exe: &str) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(exe))
            .find(|p| p.is_file())
    }

    /// 用 mpv 把 `video` 循环渲染进 `window`。
    fn spawn_mpv(mpv: &Path, window: Hwnd, video: &Path) -> Result<Child, String> {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new(mpv);
        cmd.arg(format!("--wid={window}"))
            .arg("--loop-file=inf")
            .arg("--no-audio")
            .arg("--no-osc")
            .arg("--no-input-default-bindings")
            .arg("--no-border")
            .arg("--keep-open=yes")
            .arg("--really-quiet")
            .arg("--vo=gpu")
            .arg("--hwdec=auto-safe")
            .arg(video);
        cmd.creation_flags(CREATE_NO_WINDOW);
        // 诊断用：设置 `WALLPAPER_MPV_LOG` 可让 mpv 把日志写入该文件。
        // （mpv.exe 是 GUI 子系统程序，默认无 stdout/stderr，必须用 --log-file。）
        if let Ok(log) = std::env::var("WALLPAPER_MPV_LOG") {
            cmd.arg(format!("--log-file={log}"));
        }
        cmd.spawn().map_err(|e| format!("启动 mpv 失败：{e}"))
    }

    /// 生成以 NUL 结尾的 UTF-16 字符串。
    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[repr(C)]
    struct WndClassExW {
        cb_size: u32,
        style: u32,
        lpfn_wndproc: Option<unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize>,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: isize,
        h_icon: isize,
        h_cursor: isize,
        hbr_background: isize,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: isize,
    }

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class: *const u16, window: *const u16) -> Hwnd;
        fn FindWindowExW(
            parent: Hwnd,
            child_after: Hwnd,
            class: *const u16,
            window: *const u16,
        ) -> Hwnd;
        fn EnumWindows(
            cb: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
            lparam: isize,
        ) -> i32;
        fn SendMessageTimeoutW(
            hwnd: Hwnd,
            msg: u32,
            wparam: usize,
            lparam: isize,
            flags: u32,
            timeout: u32,
            out: *mut usize,
        ) -> isize;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: Hwnd,
            menu: isize,
            inst: isize,
            param: *mut c_void,
        ) -> Hwnd;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn RegisterClassExW(wc: *const WndClassExW) -> u16;
        fn GetSystemMetrics(index: i32) -> i32;
        fn IsWindow(hwnd: Hwnd) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::Duration;

        /// 端到端冒烟：把 `WALLPAPER_TEST_MP4` 指向的视频设为动态壁纸，确认「宿主窗口 +
        /// mpv 子进程」能正常起停。需要本机已安装 mpv 且处于交互式桌面会话，故默认
        /// `#[ignore]`，手动运行：
        ///   $env:WALLPAPER_TEST_MP4="D:\..\sample.mp4"; cargo test --lib livewall_roundtrip -- --ignored --nocapture
        #[test]
        #[ignore]
        fn livewall_roundtrip() {
            let video = std::env::var("WALLPAPER_TEST_MP4")
                .expect("先设置 WALLPAPER_TEST_MP4 指向一个 .mp4 文件");
            let video = PathBuf::from(video);
            assert!(video.is_file(), "视频不存在：{}", video.display());
            assert!(supported(), "当前平台不支持动态壁纸");

            set_live_wallpaper(&video).expect("set_live_wallpaper 失败");
            std::thread::sleep(Duration::from_secs(3));
            let active = active_video();
            eprintln!("active_video（设置后）= {active:?}");
            assert_eq!(active.as_deref(), Some(video.as_path()));

            // 让 mpv 播放几秒，便于肉眼确认桌面图标层之下确有画面。
            std::thread::sleep(Duration::from_secs(6));
            stop().expect("stop 失败");
            assert!(active_video().is_none(), "停止后应无活动动态壁纸");
            eprintln!("live wallpaper roundtrip OK");
        }

        /// 解压 `.7z` 的离线冒烟：先把 `WALLPAPER_TEST_7Z` 指向便携 mpv 压缩包，再
        ///   cargo test --lib extract_7z_smoke -- --ignored --nocapture
        #[test]
        #[ignore]
        fn extract_7z_smoke() {
            let z = std::env::var("WALLPAPER_TEST_7Z").expect("先设置 WALLPAPER_TEST_7Z");
            let dir = std::env::temp_dir().join("wprs_extract_smoke");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            extract_7z(Path::new(&z), &dir).expect("解压失败");
            assert!(dir.join("mpv.exe").is_file(), "解压后应存在 mpv.exe");
            eprintln!("extract 7z OK -> {}", dir.display());
        }

        /// 下载 + 解压 mpv 的端到端冒烟（走网络，默认 `#[ignore]`）：可先用
        /// `WALLPAPER_TEST_CACHE` 指定一个干净的缓存目录，再
        ///   cargo test --lib ensure_mpv_smoke -- --ignored --nocapture
        #[test]
        #[ignore]
        fn ensure_mpv_smoke() {
            if let Ok(dir) = std::env::var("WALLPAPER_TEST_CACHE") {
                std::env::set_var("LOCALAPPDATA", &dir);
            }
            std::env::remove_var("WALLPAPER_MPV");
            let p = ensure_mpv().expect("ensure_mpv 失败");
            let len = p.metadata().map(|m| m.len()).unwrap_or(0);
            eprintln!("mpv = {}（{} 字节）", p.display(), len);
            assert!(p.is_file(), "应得到可用的 mpv.exe");
        }

        /// 测速排序冒烟（只发小 Range 请求，不解压）：打印各候选地址的探测结果与排序。
        ///   cargo test --lib order_by_speed_smoke -- --ignored --nocapture
        #[test]
        #[ignore]
        fn order_by_speed_smoke() {
            let urls = mpv_download_urls();
            for u in &urls {
                eprintln!("probe {:>8?}  {}", probe_speed(u), u);
            }
            let ordered = order_by_speed(urls);
            eprintln!("--- 排序后（最快在前）---");
            for (i, u) in ordered.iter().enumerate() {
                eprintln!("{}: {}", i + 1, u);
            }
        }
    }
}
