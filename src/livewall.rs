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
pub use imp::{active_video, set_live_wallpaper, stop, supported};

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
        // 「单文件自带 mpv」发行版：释放内嵌的 mpv 到用户缓存后再使用。
        #[cfg(feature = "bundled-mpv")]
        if let Some(p) = materialize_bundled_mpv() {
            return Some(p);
        }
        None
    }

    /// 内嵌 mpv（`bundled-mpv` feature）：把编进可执行文件、且经过 gzip 压缩的 mpv
    /// 解压并释放到用户缓存目录，返回 `mpv.exe` 路径。用于单文件、免安装、自带 mpv 的
    /// 发行版。压缩由 `build.rs` 在构建期完成（mpv ~116 MB → ~45 MB），大幅减小单文件体积。
    #[cfg(feature = "bundled-mpv")]
    fn materialize_bundled_mpv() -> Option<PathBuf> {
        use std::io::Read;
        const MPV_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mpv.exe.gz"));
        const DLL_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/d3dcompiler_43.dll.gz"));
        // 解压后的原始大小（由 build.rs 通过 rustc-env 注入）。
        let mpv_len: u64 = env!("WPRS_MPV_LEN").parse().ok()?;
        let dll_len: u64 = env!("WPRS_DLL_LEN").parse().ok()?;
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))?;
        let dir = base.join("wallpaper-rs").join("mpv-runtime");
        std::fs::create_dir_all(&dir).ok()?;
        let exe = dir.join("mpv.exe");
        let dll = dir.join("d3dcompiler_43.dll");
        // 目标文件已存在且大小正确则跳过（避免每次启动都解压上百 MB）。
        let ready = |p: &Path, n: u64| p.metadata().map(|m| m.len() == n).unwrap_or(false);
        if !ready(&exe, mpv_len) {
            let mut out = Vec::with_capacity(mpv_len as usize);
            flate2::read::GzDecoder::new(MPV_GZ).read_to_end(&mut out).ok()?;
            std::fs::write(&exe, &out).ok()?;
        }
        if !ready(&dll, dll_len) {
            let mut out = Vec::with_capacity(dll_len as usize);
            flate2::read::GzDecoder::new(DLL_GZ).read_to_end(&mut out).ok()?;
            std::fs::write(&dll, &out).ok()?;
        }
        exe.is_file().then_some(exe)
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
    }
}
