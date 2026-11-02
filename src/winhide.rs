//! Windows-only window helpers to "hide to tray" without stopping the event
//! loop. We deliberately keep the HWND *visible* to Windows (so it keeps
//! receiving WM_PAINT and egui keeps ticking) but move it far off-screen and
//! drop it from the taskbar / Alt+Tab via `WS_EX_TOOLWINDOW`.

use std::ffi::c_void;

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

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_SHOWWINDOW: u32 = 0x0040;
const SW_RESTORE: i32 = 9;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;

/// Hide the window: remember its placement, drop it from the taskbar and move
/// it off every monitor. Returns the saved placement, or `None` on failure.
pub unsafe fn hide(hwnd: isize) -> Option<Placement> {
    if hwnd == 0 {
        return None;
    }
    let h = hwnd as *mut c_void;
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
    SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW);
    // Park it fully outside the virtual desktop (works with any monitor layout).
    let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
    let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
    let x = vx - placement.width.max(100) - 200;
    let y = vy - placement.height.max(100) - 200;
    SetWindowPos(
        h,
        std::ptr::null_mut(),
        x,
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
    Some(placement)
}

/// Restore the window to its remembered placement, re-enable the normal taskbar
/// entry and bring it to the foreground.
pub unsafe fn show(hwnd: isize, p: &Placement) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as *mut c_void;
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    SetWindowLongPtrW(h, GWL_EXSTYLE, ex & !WS_EX_TOOLWINDOW);
    SetWindowPos(
        h,
        std::ptr::null_mut(),
        p.left,
        p.top,
        p.width,
        p.height,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
    );
    ShowWindow(h, SW_RESTORE);
    SetForegroundWindow(h);
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
    fn GetSystemMetrics(index: i32) -> i32;
}
