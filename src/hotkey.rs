//! Global hotkey (Ctrl+Alt+W) and background ticker helpers.
//!
//! Windows uses `RegisterHotKey` on a dedicated message-loop thread. macOS and
//! Linux (X11) use the cross-platform `global-hotkey` crate (registered on the
//! main thread, which macOS requires). A global flag is set on press and the UI
//! thread toggles window visibility.

use std::sync::atomic::AtomicI32; // 引入依赖
use std::sync::atomic::{AtomicBool, Ordering}; // 引入依赖
use std::sync::Arc; // 引入依赖

#[cfg(all(not(windows), not(target_os = "android")))] // 语句
use global_hotkey::hotkey::{Code, HotKey, Modifiers}; // 引入依赖

pub struct Hotkey { // 结构体 Hotkey
    toggle: Arc<AtomicBool>, // 字段 toggle
    registered: Arc<AtomicBool>, // 字段 registered
    err: Arc<AtomicI32>, // 字段 err
    chosen: Arc<AtomicI32>, // 字段 chosen
    /// id of the hotkey that actually registered (non-Windows path); -1 if none.
    #[cfg(all(not(windows), not(target_os = "android")))] // 语句
    gh_id: i32, // 字段 gh_id
    /// Kept alive for the lifetime of the app so the registration persists.
    #[cfg(all(not(windows), not(target_os = "android")))] // 语句
    _manager: Option<global_hotkey::GlobalHotKeyManager>, // 字段 _manager
} // 块结束

impl Hotkey { // 实现
    /// Returns true once if the hotkey was pressed since the last call.
    pub fn take_toggle(&self) -> bool { // 函数 take_toggle
        // Non-Windows: drain the global-hotkey event channel for a matching press.
        #[cfg(all(not(windows), not(target_os = "android")))] // 语句
        { // 语句
            if self.gh_id >= 0 { // 条件分支
                while let Ok(ev) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() { // 循环
                    if ev.state() == global_hotkey::HotKeyState::Pressed // 条件分支
                        && ev.id() as i32 == self.gh_id // 语句
                    { // 语句
                        self.toggle.store(true, Ordering::SeqCst); // 语句
                    } // 块结束
                } // 块结束
            } // 块结束
        } // 块结束
        self.toggle.swap(false, Ordering::SeqCst) // 语句
    } // 块结束
    pub fn is_registered(&self) -> bool { // 函数 is_registered
        self.registered.load(Ordering::SeqCst) // 语句
    } // 块结束
    pub fn error_code(&self) -> i32 { // 函数 error_code
        self.err.load(Ordering::SeqCst) // 语句
    } // 块结束
    /// Human-readable label of the hotkey that actually registered.
    pub fn label(&self) -> String { // 函数 label
        let i = self.chosen.load(Ordering::SeqCst); // 变量 i
        if i >= 0 && (i as usize) < CANDIDATES.len() { // 条件分支
            CANDIDATES[i as usize].2.to_string() // 语句
        } else { // 语句
            "不可用".to_string() // 语句
        } // 块结束
    } // 块结束
} // 块结束

/// Candidate (modifiers, virtual-key, label). Tried in order; the first that the
/// OS accepts wins (Ctrl+Alt+W is often already taken by other software).
const CANDIDATES: &[(u32, u32, &str)] = &[ // 常量 CANDIDATES
    (0x1 | 0x2 | 0x4000, 0x57, "Ctrl+Alt+W"), // 语句
    (0x1 | 0x2 | 0x4 | 0x4000, 0x57, "Ctrl+Alt+Shift+W"), // 语句
    (0x1 | 0x2 | 0x4000, 0x78, "Ctrl+Alt+F9"), // 语句
    (0x1 | 0x2 | 0x4 | 0x4000, 0x78, "Ctrl+Alt+Shift+F9"), // 语句
    (0x1 | 0x2 | 0x4000, 0x77, "Ctrl+Alt+F8"), // 语句
]; // 语句

/// Spawn the hotkey listener thread. Returns a handle; `is_registered()` tells
/// whether the OS accepted the hotkey (may fail if another app owns it).
pub fn start(ctx: eframe::egui::Context) -> Hotkey { // 函数 start
    let toggle = Arc::new(AtomicBool::new(false)); // 变量 toggle
    let registered = Arc::new(AtomicBool::new(false)); // 变量 registered
    let err = Arc::new(AtomicI32::new(0)); // 变量 err
    let chosen = Arc::new(AtomicI32::new(-1)); // 变量 chosen

    #[cfg(windows)] // 语句
    { // 语句
        let t = toggle.clone(); // 变量 t
        let r = registered.clone(); // 变量 r
        let e = err.clone(); // 变量 e
        let ch = chosen.clone(); // 变量 ch
        std::thread::spawn(move || unsafe { // 调用 spawn
            let mut ok = 0; // 变量 ok
            for (idx, (mods, vk, _label)) in CANDIDATES.iter().enumerate() { // 循环
                ok = RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, *mods, *vk); // 语句
                if ok != 0 { // 条件分支
                    ch.store(idx as i32, Ordering::SeqCst); // 语句
                    break; // 语句
                } // 块结束
            } // 块结束
            r.store(ok != 0, Ordering::SeqCst); // 语句
            if ok == 0 { // 条件分支
                e.store(GetLastError(), Ordering::SeqCst); // 语句
                return; // 返回
            } // 块结束
            let mut msg: Msg = std::mem::zeroed(); // 变量 msg
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 { // 循环
                if msg.message == WM_HOTKEY { // 条件分支
                    t.store(true, Ordering::SeqCst); // 语句
                    ctx.request_repaint(); // 语句
                } // 块结束
            } // 块结束
        }); // 语句
        Hotkey { // 语句
            toggle, // 语句
            registered, // 语句
            err, // 语句
            chosen, // 语句
        } // 块结束
    } // 块结束
    #[cfg(all(not(windows), not(target_os = "android")))] // 语句
    { // 语句
        let _ = ctx; // 变量 _
        // Must be created on the main thread (macOS requirement). eframe calls
        // App::new on the main thread, so this is safe.
        let mut manager = None; // 变量 manager
        let mut reg = false; // 变量 reg
        let mut ch: i32 = -1; // 变量 ch
        let mut ghid: i32 = -1; // 变量 ghid
        let mut errc: i32 = 0; // 变量 errc
        match global_hotkey::GlobalHotKeyManager::new() { // 分支匹配
            Ok(m) => { // 匹配分支
                let cands: [(Modifiers, Code, i32); 3] = [ // 变量 cands
                    (Modifiers::CONTROL | Modifiers::ALT, Code::KeyW, 0), // 语句
                    ( // 语句
                        Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT, // 字段 Modifiers
                        Code::KeyW, // 字段 Code
                        1, // 语句
                    ), // 语句
                    (Modifiers::CONTROL | Modifiers::ALT, Code::F9, 2), // 语句
                ]; // 语句
                for (mods, code, idx) in cands { // 循环
                    let hk = HotKey::new(Some(mods), code); // 变量 hk
                    if m.register(hk).is_ok() { // 条件分支
                        ghid = hk.id() as i32; // 语句
                        ch = idx; // 语句
                        reg = true; // 语句
                        break; // 语句
                    } // 块结束
                } // 块结束
                manager = Some(m); // 语句
            } // 块结束
            Err(_) => { // 匹配分支
                errc = -1; // 语句
            } // 块结束
        } // 块结束
        registered.store(reg, Ordering::SeqCst); // 语句
        err.store(errc, Ordering::SeqCst); // 语句
        chosen.store(ch, Ordering::SeqCst); // 语句
        Hotkey { // 语句
            toggle, // 语句
            registered, // 语句
            err, // 语句
            chosen, // 语句
            gh_id: ghid, // 字段 gh_id
            _manager: manager, // 字段 _manager
        } // 块结束
    } // 块结束
    #[cfg(target_os = "android")] // 语句
    { // 语句
        // No global hotkeys on mobile.
        let _ = ctx; // 变量 _
        Hotkey { // 语句
            toggle, // 语句
            registered, // 语句
            err, // 语句
            chosen, // 语句
        } // 块结束
    } // 块结束
} // 块结束

#[cfg(windows)] // 语句
const HOTKEY_ID: i32 = 1; // 常量 HOTKEY_ID
#[cfg(windows)] // 语句
const WM_HOTKEY: u32 = 0x0312; // 常量 WM_HOTKEY

#[cfg(windows)] // 语句
#[repr(C)] // 语句
struct Point { // 结构体 Point
    x: i32, // 字段 x
    y: i32, // 字段 y
} // 块结束

#[cfg(windows)] // 语句
#[repr(C)] // 语句
struct Msg { // 结构体 Msg
    hwnd: *mut std::ffi::c_void, // 字段 hwnd
    message: u32, // 字段 message
    w_param: usize, // 字段 w_param
    l_param: isize, // 字段 l_param
    time: u32, // 字段 time
    pt: Point, // 字段 pt
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "user32")] // 语句
extern "system" { // 语句
    fn RegisterHotKey(h_wnd: *mut std::ffi::c_void, id: i32, fs_modifiers: u32, vk: u32) -> i32; // 函数 RegisterHotKey
    fn GetMessageW( // 函数 GetMessageW
        lp_msg: *mut Msg, // 字段 lp_msg
        h_wnd: *mut std::ffi::c_void, // 字段 h_wnd
        w_msg_filter_min: u32, // 字段 w_msg_filter_min
        w_msg_filter_max: u32, // 字段 w_msg_filter_max
    ) -> i32; // 语句
} // 块结束

#[cfg(windows)] // 语句
#[link(name = "kernel32")] // 语句
extern "system" { // 语句
    fn GetLastError() -> i32; // 函数 GetLastError
} // 块结束
