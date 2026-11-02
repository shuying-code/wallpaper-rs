//! Global hotkey (Ctrl+Alt+W) and background ticker helpers.
//!
//! Windows uses `RegisterHotKey` on a dedicated message-loop thread. macOS and
//! Linux (X11) use the cross-platform `global-hotkey` crate (registered on the
//! main thread, which macOS requires). A global flag is set on press and the UI
//! thread toggles window visibility.

use std::sync::atomic::AtomicI32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(all(not(windows), not(target_os = "android")))]
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

pub struct Hotkey {
    toggle: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
    err: Arc<AtomicI32>,
    chosen: Arc<AtomicI32>,
    /// id of the hotkey that actually registered (non-Windows path); -1 if none.
    #[cfg(all(not(windows), not(target_os = "android")))]
    gh_id: i32,
    /// Kept alive for the lifetime of the app so the registration persists.
    #[cfg(all(not(windows), not(target_os = "android")))]
    _manager: Option<global_hotkey::GlobalHotKeyManager>,
}

impl Hotkey {
    /// Returns true once if the hotkey was pressed since the last call.
    pub fn take_toggle(&self) -> bool {
        // Non-Windows: drain the global-hotkey event channel for a matching press.
        #[cfg(all(not(windows), not(target_os = "android")))]
        {
            if self.gh_id >= 0 {
                while let Ok(ev) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
                    if ev.state() == global_hotkey::HotKeyState::Pressed
                        && ev.id() as i32 == self.gh_id
                    {
                        self.toggle.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
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
        Hotkey {
            toggle,
            registered,
            err,
            chosen,
        }
    }
    #[cfg(all(not(windows), not(target_os = "android")))]
    {
        let _ = ctx;
        // Must be created on the main thread (macOS requirement). eframe calls
        // App::new on the main thread, so this is safe.
        let mut manager = None;
        let mut reg = false;
        let mut ch: i32 = -1;
        let mut ghid: i32 = -1;
        let mut errc: i32 = 0;
        match global_hotkey::GlobalHotKeyManager::new() {
            Ok(m) => {
                let cands: [(Modifiers, Code, i32); 3] = [
                    (Modifiers::CONTROL | Modifiers::ALT, Code::KeyW, 0),
                    (
                        Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT,
                        Code::KeyW,
                        1,
                    ),
                    (Modifiers::CONTROL | Modifiers::ALT, Code::F9, 2),
                ];
                for (mods, code, idx) in cands {
                    let hk = HotKey::new(Some(mods), code);
                    if m.register(hk).is_ok() {
                        ghid = hk.id() as i32;
                        ch = idx;
                        reg = true;
                        break;
                    }
                }
                manager = Some(m);
            }
            Err(_) => {
                errc = -1;
            }
        }
        registered.store(reg, Ordering::SeqCst);
        err.store(errc, Ordering::SeqCst);
        chosen.store(ch, Ordering::SeqCst);
        Hotkey {
            toggle,
            registered,
            err,
            chosen,
            gh_id: ghid,
            _manager: manager,
        }
    }
    #[cfg(target_os = "android")]
    {
        // No global hotkeys on mobile.
        let _ = ctx;
        Hotkey {
            toggle,
            registered,
            err,
            chosen,
        }
    }
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
