//! Global hotkey (Ctrl+Alt+W) and background ticker helpers.
//!
//! The hotkey is registered on a dedicated thread with its own message loop so
//! it works regardless of which app has focus. A global flag is set on press and
//! the UI thread toggles window visibility.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::AtomicI32;
use std::sync::Arc;

#[derive(Clone)]
pub struct Hotkey {
    toggle: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
    err: Arc<AtomicI32>,
    chosen: Arc<AtomicI32>,
}

impl Hotkey {
    /// Returns true once if the hotkey was pressed since the last call.
    pub fn take_toggle(&self) -> bool {
        self.toggle.swap(false, Ordering::SeqCst)
    }
    pub fn is_registered(&self) -> bool {
        self.registered.load(Ordering::SeqCst)
    }
    pub fn error_code(&self) -> i32 {
        self.err.load(Ordering::SeqCst)
    }
    /// Human-readable label of the hotkey that actually registered.
    pub fn label(&self) -> String {
        let i = self.chosen.load(Ordering::SeqCst);
        if i >= 0 && (i as usize) < CANDIDATES.len() {
            CANDIDATES[i as usize].2.to_string()
        } else {
            "不可用".to_string()
        }
    }
}

/// Candidate (modifiers, virtual-key, label). Tried in order; the first that the
/// OS accepts wins (Ctrl+Alt+W is often already taken by other software).
const CANDIDATES: &[(u32, u32, &str)] = &[
    (0x1 | 0x2 | 0x4000, 0x57, "Ctrl+Alt+W"),
    (0x1 | 0x2 | 0x4 | 0x4000, 0x57, "Ctrl+Alt+Shift+W"),
    (0x1 | 0x2 | 0x4000, 0x78, "Ctrl+Alt+F9"),
    (0x1 | 0x2 | 0x4 | 0x4000, 0x78, "Ctrl+Alt+Shift+F9"),
    (0x1 | 0x2 | 0x4000, 0x77, "Ctrl+Alt+F8"),
];

/// Spawn the hotkey listener thread. Returns a handle; `is_registered()` tells
/// whether the OS accepted the hotkey (may fail if another app owns it).
pub fn start(ctx: eframe::egui::Context) -> Hotkey {
    let toggle = Arc::new(AtomicBool::new(false));
    let registered = Arc::new(AtomicBool::new(false));
    let err = Arc::new(AtomicI32::new(0));
    let chosen = Arc::new(AtomicI32::new(-1));

    #[cfg(windows)]
    {
        let t = toggle.clone();
        let r = registered.clone();
        let e = err.clone();
        let ch = chosen.clone();
        std::thread::spawn(move || unsafe {
            let mut ok = 0;
            for (idx, (mods, vk, _label)) in CANDIDATES.iter().enumerate() {
                ok = RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, *mods, *vk);
                if ok != 0 {
                    ch.store(idx as i32, Ordering::SeqCst);
                    break;
                }
            }
            r.store(ok != 0, Ordering::SeqCst);
            if ok == 0 {
                e.store(GetLastError(), Ordering::SeqCst);
                return;
            }
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY {
                    t.store(true, Ordering::SeqCst);
                    ctx.request_repaint();
                }
            }
        });
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
    }

    Hotkey { toggle, registered, err, chosen }
}

#[cfg(windows)]
const HOTKEY_ID: i32 = 1;
#[cfg(windows)]
const WM_HOTKEY: u32 = 0x0312;

#[cfg(windows)]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[cfg(windows)]
#[repr(C)]
struct Msg {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: Point,
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(h_wnd: *mut std::ffi::c_void, id: i32, fs_modifiers: u32, vk: u32) -> i32;
    fn GetMessageW(
        lp_msg: *mut Msg,
        h_wnd: *mut std::ffi::c_void,
        w_msg_filter_min: u32,
        w_msg_filter_max: u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> i32;
}
