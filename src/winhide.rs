//! Windows-only window helpers to "hide to tray" without stopping the event
//! loop. We deliberately keep the HWND *visible* to Windows (so it keeps
//! receiving WM_PAINT and egui keeps ticking) but move it far off-screen and
//! drop it from the taskbar / Alt+Tab via `WS_EX_TOOLWINDOW`.

use std::ffi::c_void; // 引入依赖
use std::sync::atomic::{AtomicBool, Ordering}; // 引入依赖
use std::sync::Mutex; // 引入依赖

#[repr(C)] // 语句
#[derive(Clone, Copy, Default)] // 语句
struct Rect { // 结构体 Rect
    left: i32, // 字段 left
    top: i32, // 字段 top
    right: i32, // 字段 right
    bottom: i32, // 字段 bottom
} // 块结束

/// The window's on-screen placement, remembered so we can put it back exactly
/// where the user had it.
#[derive(Clone, Copy, Default)] // 语句
pub struct Placement { // 结构体 Placement
    left: i32, // 字段 left
    top: i32, // 字段 top
    width: i32, // 字段 width
    height: i32, // 字段 height
} // 块结束

/// Whether the window is currently parked in the tray (hidden from the user but
/// still alive so timers/tray keep running).
static HIDDEN: AtomicBool = AtomicBool::new(false); // 常量 HIDDEN
/// On-screen placement saved when hiding so we can put the window back.
static PLACEMENT: Mutex<Option<Placement>> = Mutex::new(None); // 常量 PLACEMENT

/// True while the app is hidden to the tray.
pub fn is_hidden() -> bool { // 函数 is_hidden
    HIDDEN.load(Ordering::SeqCst) // 语句
} // 块结束

const GWL_EXSTYLE: i32 = -20; // 常量 GWL_EXSTYLE
const WS_EX_TOOLWINDOW: isize = 0x0000_0080; // 常量 WS_EX_TOOLWINDOW
const WS_EX_APPWINDOW: isize = 0x0004_0000; // 常量 WS_EX_APPWINDOW
const WS_EX_TRANSPARENT: isize = 0x0000_0020; // 常量 WS_EX_TRANSPARENT
const WS_EX_LAYERED: isize = 0x0008_0000; // 常量 WS_EX_LAYERED
const LWA_ALPHA: u32 = 0x0000_0002; // 常量 LWA_ALPHA
const SWP_NOSIZE: u32 = 0x0001; // 常量 SWP_NOSIZE
const SWP_NOMOVE: u32 = 0x0002; // 常量 SWP_NOMOVE
const SWP_NOZORDER: u32 = 0x0004; // 常量 SWP_NOZORDER
const SWP_NOACTIVATE: u32 = 0x0010; // 常量 SWP_NOACTIVATE
const SWP_FRAMECHANGED: u32 = 0x0020; // 常量 SWP_FRAMECHANGED
const SWP_SHOWWINDOW: u32 = 0x0040; // 常量 SWP_SHOWWINDOW
const SW_RESTORE: i32 = 9; // 常量 SW_RESTORE
const SW_MINIMIZE: i32 = 6; // 常量 SW_MINIMIZE
const SW_HIDE: i32 = 0; // 常量 SW_HIDE
const SW_SHOWNOACTIVATE: i32 = 4; // 常量 SW_SHOWNOACTIVATE
const RDW_INVALIDATE: u32 = 0x0001; // 常量 RDW_INVALIDATE
const RDW_ERASE: u32 = 0x0004; // 常量 RDW_ERASE
const RDW_ALLCHILDREN: u32 = 0x0080; // 常量 RDW_ALLCHILDREN
const RDW_FRAME: u32 = 0x0400; // 常量 RDW_FRAME

/// Hide the window: remember its placement, drop it from the taskbar / Alt+Tab
/// and make it invisible while keeping it on-screen (so the egui event loop and
/// the slideshow keep running). Returns the saved placement, or `None` on failure.
pub unsafe fn hide(hwnd: isize) -> Option<Placement> { // 语句
    if hwnd == 0 { // 条件分支
        return None; // 返回
    } // 块结束
    let h = hwnd as *mut c_void; // 变量 h
    // A minimized window reports a bogus window rect; restore it first so the
    // saved placement (used when bringing the window back) is meaningful.
    if IsIconic(h) != 0 { // 条件分支
        ShowWindow(h, SW_RESTORE); // 调用 ShowWindow
    } // 块结束
    let mut r = Rect::default(); // 变量 r
    if GetWindowRect(h, &mut r) == 0 { // 条件分支
        return None; // 返回
    } // 块结束
    let placement = Placement { // 变量 placement
        left: r.left, // 字段 left
        top: r.top, // 字段 top
        width: (r.right - r.left).max(0), // 字段 width
        height: (r.bottom - r.top).max(0), // 字段 height
    }; // 块结束
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE); // 变量 ex
    // Keep the window on-screen but fully transparent + click-through so the OS
    // still delivers paint messages and the egui loop keeps running. Drop
    // WS_EX_APPWINDOW first, otherwise it keeps forcing a taskbar button even
    // when WS_EX_TOOLWINDOW is set.
    SetWindowLongPtrW( // 调用 SetWindowLongPtrW
        h, // 语句
        GWL_EXSTYLE, // 语句
        (ex & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT, // 语句
    ); // 块结束
    SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA); // 调用 SetLayeredWindowAttributes
    ShowWindow(h, SW_HIDE); // 调用 ShowWindow
    ShowWindow(h, SW_SHOWNOACTIVATE); // 调用 ShowWindow

    // Verify the transparent overlay actually took effect. On a few GPU / DPI /
    // OS combinations the layered constant-alpha trick is silently ignored, which
    // would leave the window fully visible — the user then sees "隐藏到托盘" do
    // nothing at all. In that case fall back to a plain hide so the window is
    // definitely gone from the screen (the tray menu / hotkey still restore it,
    // because those run on the native thread and do not need the paint loop).
    let mut key: u32 = 0; // 变量 key
    let mut alpha: u8 = 255; // 变量 alpha
    let mut flags: u32 = 0; // 变量 flags
    if GetLayeredWindowAttributes(h, &mut key, &mut alpha, &mut flags) == 0 || alpha != 0 { // 条件分支
        ShowWindow(h, SW_HIDE); // 调用 ShowWindow
    } // 块结束
    Some(placement) // 调用 Some
} // 块结束

/// Restore the window, re-enable the normal taskbar entry, centre it on the
/// primary monitor's work area (so it never hides under the taskbar) and bring
/// it to the foreground.
pub unsafe fn show(hwnd: isize, p: &Placement) { // 语句
    if hwnd == 0 { // 条件分支
        return; // 返回
    } // 块结束
    let h = hwnd as *mut c_void; // 变量 h
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE); // 变量 ex
    SetWindowLongPtrW( // 调用 SetWindowLongPtrW
        h, // 语句
        GWL_EXSTYLE, // 语句
        (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW, // 语句
    ); // 块结束
    // Defensive: if the window was left layered while hiding, clearing the style
    // is not always enough on its own — force the alpha back to fully opaque so
    // the window can never come back fully transparent (invisible).
    if ex & WS_EX_LAYERED != 0 { // 条件分支
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA); // 调用 SetLayeredWindowAttributes
    } // 块结束
    // Force a re-evaluation so the normal taskbar button comes back.
    ShowWindow(h, SW_HIDE); // 调用 ShowWindow
    // Centre on the primary monitor's work area (screen minus taskbar).
    let mut wa = Rect::default(); // 变量 wa
    let ok = SystemParametersInfoW( // 变量 ok
        SPI_GETWORKAREA, // 语句
        0, // 语句
        &mut wa as *mut Rect as *mut c_void, // 语句
        0, // 语句
    ); // 块结束
    let mut w = p.width; // 变量 w
    let mut hgt = p.height; // 变量 hgt
    if w <= 0 { // 条件分支
        w = 1000; // 语句
    } // 块结束
    if hgt <= 0 { // 条件分支
        hgt = 700; // 语句
    } // 块结束
    let (mut x, mut y) = (p.left, p.top); // 调用 let
    if ok != 0 { // 条件分支
        let avail_w = (wa.right - wa.left).max(0); // 变量 avail_w
        let avail_h = (wa.bottom - wa.top).max(0); // 变量 avail_h
        x = wa.left + ((avail_w - w) / 2).max(0); // 语句
        y = wa.top + ((avail_h - hgt) / 2).max(0); // 语句
    } // 块结束
    SetWindowPos( // 调用 SetWindowPos
        h, // 语句
        std::ptr::null_mut(), // 字段 std
        x, // 语句
        y, // 语句
        w, // 语句
        hgt, // 语句
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED, // 语句
    ); // 块结束
    ShowWindow(h, SW_RESTORE); // 调用 ShowWindow
    // Removing WS_EX_LAYERED can leave stale pixels; repaint the whole window.
    RedrawWindow( // 调用 RedrawWindow
        h, // 语句
        std::ptr::null(), // 字段 std
        std::ptr::null_mut(), // 字段 std
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN, // 语句
    ); // 块结束
    raise_to_foreground(h); // 调用 raise_to_foreground
} // 块结束

/// Bring `h` to the foreground as reliably as Windows allows.
///
/// Plain `SetForegroundWindow` is frequently rejected by the Windows foreground
/// lock (the call is only honoured when the calling process is already the
/// foreground process). When the user picks "显示主界面" from the tray menu, or
/// triggers the global hotkey, that is not guaranteed, so we attach our input
/// queue to the current foreground thread for the duration of the call.
unsafe fn raise_to_foreground(h: *mut c_void) { // 语句
    let fg = GetForegroundWindow(); // 变量 fg
    let fg_thread = if fg.is_null() { // 变量 fg_thread
        0 // 语句
    } else { // 语句
        GetWindowThreadProcessId(fg, std::ptr::null_mut()) // 调用 GetWindowThreadProcessId
    }; // 块结束
    let this_thread = GetCurrentThreadId(); // 变量 this_thread
    let attached = fg_thread != 0 && fg_thread != this_thread; // 变量 attached
    if attached { // 条件分支
        AttachThreadInput(this_thread, fg_thread, 1); // 调用 AttachThreadInput
    } // 块结束
    BringWindowToTop(h); // 调用 BringWindowToTop
    SetForegroundWindow(h); // 调用 SetForegroundWindow
    if attached { // 条件分支
        AttachThreadInput(this_thread, fg_thread, 0); // 调用 AttachThreadInput
    } // 块结束
} // 块结束

/// Hide the window to the tray: keep the HWND on-screen (so the egui event loop
/// and repaints keep running) but make it fully transparent and click-through,
/// and drop it from the taskbar / Alt+Tab. Remember the placement for restoring.
///
/// We intentionally do *not* move the window off every monitor: Windows then
/// stops delivering paint messages, which froze the egui loop and made the tray
/// menu (show / hide) appear unresponsive.
pub unsafe fn hide_to_tray(hwnd: isize) { // 语句
    if hwnd == 0 { // 条件分支
        return; // 返回
    } // 块结束
    if let Some(p) = hide(hwnd) { // 条件分支
        *PLACEMENT.lock().unwrap() = Some(p); // 语句
        HIDDEN.store(true, Ordering::SeqCst); // 语句
    } // 块结束
} // 块结束

/// Restore the window from the tray and bring it to the foreground.
pub unsafe fn show_from_tray(hwnd: isize) { // 语句
    if hwnd == 0 { // 条件分支
        return; // 返回
    } // 块结束
    let placement = PLACEMENT.lock().unwrap().take(); // 变量 placement
    match placement { // 分支匹配
        Some(p) => show(hwnd, &p), // 匹配分支
        None => restore(hwnd), // 匹配分支
    } // 块结束
    HIDDEN.store(false, Ordering::SeqCst); // 语句
} // 块结束

/// Minimize the window to the taskbar, keeping a normal taskbar button while the
/// process (slideshow / daily wallpaper) keeps running in the background.
///
/// Unlike `hide`, this never removes the window from the taskbar/Alt+Tab, so the
/// user can always find and reopen the app by clicking its taskbar button.
pub unsafe fn minimize(hwnd: isize) { // 语句
    if hwnd == 0 { // 条件分支
        return; // 返回
    } // 块结束
    let h = hwnd as *mut c_void; // 变量 h
    // Ensure the window is a normal (taskbar-visible) window again, in case it was
    // previously hidden to the tray with WS_EX_TOOLWINDOW still set.
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE); // 变量 ex
    if ex & (WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT) != 0 { // 条件分支
        SetWindowLongPtrW( // 调用 SetWindowLongPtrW
            h, // 语句
            GWL_EXSTYLE, // 语句
            (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW, // 语句
        ); // 块结束
        SetWindowPos( // 调用 SetWindowPos
            h, // 语句
            std::ptr::null_mut(), // 字段 std
            0, // 语句
            0, // 语句
            0, // 语句
            0, // 语句
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED, // 语句
        ); // 块结束
    } // 块结束
    ShowWindow(h, SW_MINIMIZE); // 调用 ShowWindow
} // 块结束

/// Restore a window that was minimized with `minimize` (or otherwise minimized):
/// clear the tool-window style, un-minimize and bring it to the foreground.
pub unsafe fn restore(hwnd: isize) { // 语句
    if hwnd == 0 { // 条件分支
        return; // 返回
    } // 块结束
    let h = hwnd as *mut c_void; // 变量 h
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE); // 变量 ex
    SetWindowLongPtrW( // 调用 SetWindowLongPtrW
        h, // 语句
        GWL_EXSTYLE, // 语句
        (ex & !(WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT)) | WS_EX_APPWINDOW, // 语句
    ); // 块结束
    if ex & WS_EX_LAYERED != 0 { // 条件分支
        SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA); // 调用 SetLayeredWindowAttributes
    } // 块结束
    ShowWindow(h, SW_RESTORE); // 调用 ShowWindow
    RedrawWindow( // 调用 RedrawWindow
        h, // 语句
        std::ptr::null(), // 字段 std
        std::ptr::null_mut(), // 字段 std
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN, // 语句
    ); // 块结束
    raise_to_foreground(h); // 调用 raise_to_foreground
} // 块结束

/// Whether the window is currently minimized.
pub unsafe fn is_minimized(hwnd: isize) -> bool { // 语句
    hwnd != 0 && IsIconic(hwnd as *mut c_void) != 0 // 语句
} // 块结束

#[link(name = "user32")] // 语句
extern "system" { // 语句
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32; // 函数 GetWindowRect
    fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize; // 函数 GetWindowLongPtrW
    fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize; // 函数 SetWindowLongPtrW
    fn SetWindowPos( // 函数 SetWindowPos
        hwnd: *mut c_void, // 字段 hwnd
        insert_after: *mut c_void, // 字段 insert_after
        x: i32, // 字段 x
        y: i32, // 字段 y
        cx: i32, // 字段 cx
        cy: i32, // 字段 cy
        flags: u32, // 字段 flags
    ) -> i32; // 语句
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32; // 函数 ShowWindow
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32; // 函数 SetForegroundWindow
    fn GetForegroundWindow() -> *mut c_void; // 函数 GetForegroundWindow
    fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32; // 函数 GetWindowThreadProcessId
    fn AttachThreadInput(id_attach: u32, id_attach_to: u32, attach: i32) -> i32; // 函数 AttachThreadInput
    fn BringWindowToTop(hwnd: *mut c_void) -> i32; // 函数 BringWindowToTop
    fn IsIconic(hwnd: *mut c_void) -> i32; // 函数 IsIconic
    fn SetLayeredWindowAttributes(hwnd: *mut c_void, color_key: u32, alpha: u8, flags: u32) -> i32; // 函数 SetLayeredWindowAttributes
    fn GetLayeredWindowAttributes( // 函数 GetLayeredWindowAttributes
        hwnd: *mut c_void, // 字段 hwnd
        color_key: *mut u32, // 字段 color_key
        alpha: *mut u8, // 字段 alpha
        flags: *mut u32, // 字段 flags
    ) -> i32; // 语句
    fn RedrawWindow( // 函数 RedrawWindow
        hwnd: *mut c_void, // 字段 hwnd
        update_rect: *const Rect, // 字段 update_rect
        update_region: *mut c_void, // 字段 update_region
        flags: u32, // 字段 flags
    ) -> i32; // 语句
    fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, win_ini: u32) -> i32; // 函数 SystemParametersInfoW
} // 块结束

const SPI_GETWORKAREA: u32 = 0x0030; // 常量 SPI_GETWORKAREA

#[link(name = "kernel32")] // 语句
extern "system" { // 语句
    fn GetCurrentThreadId() -> u32; // 函数 GetCurrentThreadId
} // 块结束
