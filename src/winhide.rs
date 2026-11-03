//! Windows-only window helpers to "hide to tray" without stopping the event
//! loop. We deliberately keep the HWND *visible* to Windows (so it keeps
//! receiving WM_PAINT and egui keeps ticking) but move it far off-screen and
//! drop it from the taskbar / Alt+Tab via `WS_EX_TOOLWINDOW`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// The window's on-screen placement, remembered so we can put it back exactly
/// where the user had it.
#[derive(Clone, Copy, Default)]
pub struct Placement {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

/// Whether the window is currently parked in the tray (hidden from the user but
/// still alive so timers/tray keep running).
static HIDDEN: AtomicBool = AtomicBool::new(false);
/// On-screen placement saved when hiding so we can put the window back.
static PLACEMENT: Mutex<Option<Placement>> = Mutex::new(None);

/// True while the app is hidden to the tray.
pub fn is_hidden() -> bool {
    HIDDEN.load(Ordering::SeqCst)
}

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const WS_EX_APPWINDOW: isize = 0x0004_0000;
const WS_EX_TRANSPARENT: isize = 0x0000_0020;
const WS_EX_LAYERED: isize = 0x0008_0000;
const LWA_ALPHA: u32 = 0x0000_0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_SHOWWINDOW: u32 = 0x0040;
const SW_RESTORE: i32 = 9;
const SW_MINIMIZE: i32 = 6;
const SW_HIDE: i32 = 0;
const SW_SHOWNOACTIVATE: i32 = 4;
const RDW_INVALIDATE: u32 = 0x0001;
const RDW_ERASE: u32 = 0x0004;
const RDW_ALLCHILDREN: u32 = 0x0080;
const RDW_FRAME: u32 = 0x0400;

/// Hide the window: remember its placement, drop it from the taskbar / Alt+Tab
/// and make it invisible while keeping it on-screen (so the egui event loop and
/// the slideshow keep running). Returns the saved placement, or `None` on failure.
pub unsafe fn hide(hwnd: isize) -> Option<Placement> {
    if hwnd == 0 {
        return None;
    }
    let h = hwnd as *mut c_void;
    // A minimized window reports a bogus window rect; restore it first so the
    // saved placement (used when bringing the window back) is meaningful.
    if IsIconic(h) != 0 {
        ShowWindow(h, SW_RESTORE);
    }
    let mut r = Rect::default();
    if GetWindowRect(h, &mut r) == 0 {
        return None;
    }
    let placement = Placement {
        left: r.left,
        top: r.top,
        width: (r.right - r.left).max(0),
        height: (r.bottom - r.top).max(0),
    };
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    // Keep the window on-screen but fully transparent + click-through so the OS
    // still delivers paint messages and the egui loop keeps running. Drop
    // WS_EX_APPWINDOW first, otherwise it keeps forcing a taskbar button even
    // when WS_EX_TOOLWINDOW is set.
    SetWindowLongPtrW(
        h,
        GWL_EXSTYLE,
        (ex & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
    );
    SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA);
    ShowWindow(h, SW_HIDE);
    ShowWindow(h, SW_SHOWNOACTIVATE);

    // Verify the transparent overlay actually took effect. On a few GPU / DPI /
    // OS combinations the layered constant-alpha trick is silently ignored, which
    // would leave the window fully visible — the user then sees "隐藏到托盘" do
    // nothing at all. In that case fall back to a plain hide so the window is
    // definitely gone from the screen (the tray menu / hotkey still restore it,
    // because those run on the native thread and do not need the paint loop).
    let mut key: u32 = 0;
    let mut alpha: u8 = 255;
    let mut flags: u32 = 0;
    if GetLayeredWindowAttributes(h, &mut key, &mut alpha, &mut flags) == 0 || alpha != 0 {
        ShowWindow(h, SW_HIDE);
    }
    Some(placement)
}

/// Restore the window, re-enable the normal taskbar entry, centre it on the
/// primary monitor's work area (so it never hides under the taskbar) and bring
/// it to the foreground.
pub unsafe fn show(hwnd: isize, p: &Placement) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as *mut c_void;
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    SetWindowLongPtrW(
        h,
        GWL_EXSTYLE,
        (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW,
    );
    // Defensive: if the window was left layered while hiding, clearing the style
    // is not always enough on its own — force the alpha back to fully opaque so
    // the window can never come back fully transparent (invisible).
    if ex & WS_EX_LAYERED != 0 {
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);
    }
    // Force a re-evaluation so the normal taskbar button comes back.
    ShowWindow(h, SW_HIDE);
    // Centre on the primary monitor's work area (screen minus taskbar).
    let mut wa = Rect::default();
    let ok = SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        &mut wa as *mut Rect as *mut c_void,
        0,
    );
    let mut w = p.width;
    let mut hgt = p.height;
    if w <= 0 {
        w = 1000;
    }
    if hgt <= 0 {
        hgt = 700;
    }
    let (mut x, mut y) = (p.left, p.top);
    if ok != 0 {
        let avail_w = (wa.right - wa.left).max(0);
        let avail_h = (wa.bottom - wa.top).max(0);
        x = wa.left + ((avail_w - w) / 2).max(0);
        y = wa.top + ((avail_h - hgt) / 2).max(0);
    }
    SetWindowPos(
        h,
        std::ptr::null_mut(),
        x,
        y,
        w,
        hgt,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
    );
    ShowWindow(h, SW_RESTORE);
    // Removing WS_EX_LAYERED can leave stale pixels; repaint the whole window.
    RedrawWindow(
        h,
        std::ptr::null(),
        std::ptr::null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN,
    );
    raise_to_foreground(h);
}

/// Bring `h` to the foreground as reliably as Windows allows.
///
/// Plain `SetForegroundWindow` is frequently rejected by the Windows foreground
/// lock (the call is only honoured when the calling process is already the
/// foreground process). When the user picks "显示主界面" from the tray menu, or
/// triggers the global hotkey, that is not guaranteed, so we attach our input
/// queue to the current foreground thread for the duration of the call.
unsafe fn raise_to_foreground(h: *mut c_void) {
    let fg = GetForegroundWindow();
    let fg_thread = if fg.is_null() {
        0
    } else {
        GetWindowThreadProcessId(fg, std::ptr::null_mut())
    };
    let this_thread = GetCurrentThreadId();
    let attached = fg_thread != 0 && fg_thread != this_thread;
    if attached {
        AttachThreadInput(this_thread, fg_thread, 1);
    }
    BringWindowToTop(h);
    SetForegroundWindow(h);
    if attached {
        AttachThreadInput(this_thread, fg_thread, 0);
    }
}

/// Hide the window to the tray: keep the HWND on-screen (so the egui event loop
/// and repaints keep running) but make it fully transparent and click-through,
/// and drop it from the taskbar / Alt+Tab. Remember the placement for restoring.
///
/// We intentionally do *not* move the window off every monitor: Windows then
/// stops delivering paint messages, which froze the egui loop and made the tray
/// menu (show / hide) appear unresponsive.
pub unsafe fn hide_to_tray(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    if let Some(p) = hide(hwnd) {
        *PLACEMENT.lock().unwrap() = Some(p);
        HIDDEN.store(true, Ordering::SeqCst);
    }
}

/// Restore the window from the tray and bring it to the foreground.
pub unsafe fn show_from_tray(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let placement = PLACEMENT.lock().unwrap().take();
    match placement {
        Some(p) => show(hwnd, &p),
        None => restore(hwnd),
    }
    HIDDEN.store(false, Ordering::SeqCst);
}

/// Minimize the window to the taskbar, keeping a normal taskbar button while the
/// process (slideshow / daily wallpaper) keeps running in the background.
///
/// Unlike `hide`, this never removes the window from the taskbar/Alt+Tab, so the
/// user can always find and reopen the app by clicking its taskbar button.
pub unsafe fn minimize(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as *mut c_void;
    // Ensure the window is a normal (taskbar-visible) window again, in case it was
    // previously hidden to the tray with WS_EX_TOOLWINDOW still set.
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    if ex & (WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT) != 0 {
        SetWindowLongPtrW(
            h,
            GWL_EXSTYLE,
            (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW,
        );
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    ShowWindow(h, SW_MINIMIZE);
}

/// Restore a window that was minimized with `minimize` (or otherwise minimized):
/// clear the tool-window style, un-minimize and bring it to the foreground.
pub unsafe fn restore(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as *mut c_void;
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    SetWindowLongPtrW(
        h,
        GWL_EXSTYLE,
        (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW,
    );
    if ex & WS_EX_LAYERED != 0 {
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);
    }
    ShowWindow(h, SW_RESTORE);
    RedrawWindow(
        h,
        std::ptr::null(),
        std::ptr::null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN,
    );
    raise_to_foreground(h);
}

/// Whether the window is currently minimized.
pub unsafe fn is_minimized(hwnd: isize) -> bool {
    hwnd != 0 && IsIconic(hwnd as *mut c_void) != 0
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
    fn SetWindowPos(
        hwnd: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    fn AttachThreadInput(id_attach: u32, id_attach_to: u32, attach: i32) -> i32;
    fn BringWindowToTop(hwnd: *mut c_void) -> i32;
    fn IsIconic(hwnd: *mut c_void) -> i32;
    fn SetLayeredWindowAttributes(hwnd: *mut c_void, color_key: u32, alpha: u8, flags: u32) -> i32;
    fn GetLayeredWindowAttributes(
        hwnd: *mut c_void,
        color_key: *mut u32,
        alpha: *mut u8,
        flags: *mut u32,
    ) -> i32;
    fn RedrawWindow(
        hwnd: *mut c_void,
        update_rect: *const Rect,
        update_region: *mut c_void,
        flags: u32,
    ) -> i32;
    fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, win_ini: u32) -> i32;
}

const SPI_GETWORKAREA: u32 = 0x0030;

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThreadId() -> u32;
}
