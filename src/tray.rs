//! System tray icon (Windows notification area / macOS menu bar / Linux status
//! area). The main window is *hidden* here when the user clicks its "X", so the
//! app keeps running (slideshow / daily wallpaper) while staying out of the way.
//!
//! `tray-icon` delivers menu and icon events through global handlers. We attach
//! handlers once at start-up that push an action into a shared queue and ask
//! egui to repaint, so the UI thread picks the action up on its next frame.

use std::collections::VecDeque; // 引入依赖
use std::sync::{Arc, Mutex}; // 引入依赖

#[cfg(not(target_os = "android"))] // 语句
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem}; // 引入依赖
#[cfg(not(target_os = "android"))] // 语句
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}; // 引入依赖

/// An action requested from the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // 语句
pub enum TrayAction { // 枚举 TrayAction
    /// Show and focus the main window.
    Show, // 语句
    /// Hide the main window to the tray.
    Hide, // 语句
    /// Really exit the process (not just hide).
    Quit, // 语句
} // 块结束

/// Owns the live tray icon and drains the actions produced by its handlers.
pub struct Tray { // 结构体 Tray
    /// Kept alive for the lifetime of the app; dropping it removes the icon.
    #[cfg(not(target_os = "android"))] // 语句
    _icon: tray_icon::TrayIcon, // 字段 _icon
    queue: Arc<Mutex<VecDeque<TrayAction>>>, // 字段 queue
} // 块结束

impl Tray { // 实现
    /// Build the tray icon + context menu. Returns `None` if the platform has no
    /// usable tray backend (the caller then falls back to the old behaviour).
    pub fn install(ctx: egui::Context, window: isize) -> Option<Tray> { // 函数 install
        // Android has no system tray: report "no tray" so the app falls back to
        // normal window behaviour (hide-to-tray buttons are hidden on mobile).
        #[cfg(target_os = "android")] // 语句
        { // 语句
            let _ = ctx; // 变量 _
            let _ = window; // 变量 _
            return None; // 返回
        } // 块结束
        #[cfg(not(target_os = "android"))] // 语句
        { // 语句
        // The main window HWND (Windows) so tray actions can be applied directly,
        // without waiting for the egui frame loop.
        let win_hwnd = window; // 变量 win_hwnd
        #[cfg(not(windows))] // 语句
        let _ = win_hwnd; // 变量 _
        let menu = Menu::new(); // 变量 menu
        let show = MenuItem::new("显示主界面", true, None); // 变量 show
        let hide = MenuItem::new("隐藏到托盘", true, None); // 变量 hide
        let quit = MenuItem::new("退出 wallpaper-rs", true, None); // 变量 quit
        menu.append(&show).ok()?; // 语句
        menu.append(&hide).ok()?; // 语句
        menu.append(&PredefinedMenuItem::separator()).ok()?; // 语句
        menu.append(&quit).ok()?; // 语句

        let show_id = show.id().clone(); // 变量 show_id
        let hide_id = hide.id().clone(); // 变量 hide_id
        let quit_id = quit.id().clone(); // 变量 quit_id

        // Ask Windows to keep our tray icon in the always-visible part of the
        // notification area (instead of the Win11 overflow "hidden icons" menu).
        // Best-effort: the system may only apply it after Explorer restarts or the
        // user signs in again, so we also retry shortly after the icon is added.
        promote_tray_icon(); // 调用 promote_tray_icon
        let icon = TrayIconBuilder::new() // 变量 icon
            .with_menu(Box::new(menu)) // 语句
            .with_tooltip("wallpaper-rs · 桌面壁纸") // 语句
            .with_icon(make_icon()) // 语句
            // Left click restores the window; the menu is on right click only.
            .with_menu_on_left_click(false) // 语句
            .build() // 语句
            .ok()?; // 语句
        // The shell may create the NotifyIconSettings registry entry only after the
        // icon is registered; retry a few times in the background to pin it.
        promote_tray_icon_later(); // 调用 promote_tray_icon_later

        let queue: Arc<Mutex<VecDeque<TrayAction>>> = Arc::new(Mutex::new(VecDeque::new())); // 变量 queue

        // Menu clicks -> actions.
        { // 语句
            let queue = queue.clone(); // 变量 queue
            let ctx = ctx.clone(); // 变量 ctx
            MenuEvent::set_event_handler(Some(move |ev: MenuEvent| { // 调用 set_event_handler
                let action = if ev.id == show_id { // 变量 action
                    Some(TrayAction::Show) // 调用 Some
                } else if ev.id == hide_id { // 条件分支
                    Some(TrayAction::Hide) // 调用 Some
                } else if ev.id == quit_id { // 条件分支
                    Some(TrayAction::Quit) // 调用 Some
                } else { // 语句
                    None // 语句
                }; // 块结束
                if let Some(a) = action { // 条件分支
                    // Apply immediately on the native thread so the action works
                    // even if the egui frame loop is idle (e.g. hidden to tray).
                    #[cfg(windows)] // 语句
                    match a { // 分支匹配
                        TrayAction::Show => unsafe { crate::winhide::show_from_tray(win_hwnd) }, // 匹配分支
                        TrayAction::Hide => unsafe { crate::winhide::hide_to_tray(win_hwnd) }, // 匹配分支
                        TrayAction::Quit => { // 匹配分支
                            // Safety net: exit even if the UI loop never wakes.
                            std::thread::spawn(|| { // 调用 spawn
                                // Kill sibling instances first, then ourselves.
                                crate::instances::terminate_other_instances(); // 调用 terminate_other_instances
                                std::thread::sleep(std::time::Duration::from_millis(2500)); // 调用 sleep
                                std::process::exit(0); // 调用 exit
                            }); // 语句
                        } // 块结束
                    } // 块结束
                    // Also enqueue so the UI can sync state / status text.
                    queue.lock().unwrap().push_back(a); // 语句
                    ctx.request_repaint(); // 语句
                } // 块结束
            })); // 语句
        } // 块结束

        // Left click / double click on the icon -> show the window.
        { // 语句
            let queue = queue.clone(); // 变量 queue
            let ctx = ctx.clone(); // 变量 ctx
            let win_hwnd_lc = win_hwnd; // 变量 win_hwnd_lc
            TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| { // 调用 set_event_handler
                let show = matches!( // 变量 show
                    ev, // 语句
                    TrayIconEvent::Click { // 语句
                        button: MouseButton::Left, // 字段 button
                        button_state: MouseButtonState::Up, // 字段 button_state
                        .. // 语句
                    } | TrayIconEvent::DoubleClick { // 语句
                        button: MouseButton::Left, // 字段 button
                        .. // 语句
                    } // 块结束
                ); // 块结束
                if show { // 条件分支
                    #[cfg(windows)] // 语句
                    unsafe { // 语句
                        crate::winhide::show_from_tray(win_hwnd_lc) // 调用 show_from_tray
                    }; // 块结束
                    queue.lock().unwrap().push_back(TrayAction::Show); // 语句
                    ctx.request_repaint(); // 语句
                } // 块结束
            })); // 语句
        } // 块结束

        Some(Tray { _icon: icon, queue }) // 调用 Some
        } // 块结束
    } // 块结束

    /// Take every action queued since the last call (non-blocking).
    pub fn drain(&self) -> Vec<TrayAction> { // 函数 drain
        let mut q = self.queue.lock().unwrap(); // 变量 q
        q.drain(..).collect() // 语句
    } // 块结束
} // 块结束

/// A small procedurally-drawn 32×32 icon (amber→teal disc with a dark mountain
/// silhouette) so no external image asset is required at build time.
#[cfg(not(target_os = "android"))] // 语句
fn make_icon() -> tray_icon::Icon { // 函数 make_icon
    const N: u32 = 32; // 常量 N
    let mut rgba = vec![0u8; (N * N * 4) as usize]; // 变量 rgba
    let c = (N as f32 - 1.0) / 2.0; // 变量 c
    let radius = 15.0_f32; // 变量 radius
    for y in 0..N { // 循环
        for x in 0..N { // 循环
            let i = ((y * N + x) * 4) as usize; // 变量 i
            let dx = x as f32 - c; // 变量 dx
            let dy = y as f32 - c; // 变量 dy
            if dx * dx + dy * dy > radius * radius { // 条件分支
                continue; // transparent outside the disc
            } // 块结束
            let t = y as f32 / (N as f32 - 1.0); // 变量 t
            let top = (232.0, 176.0, 75.0); // 变量 top
            let bot = (79.0, 209.0, 197.0); // 变量 bot
            let col = ( // 变量 col
                (top.0 + (bot.0 - top.0) * t) as u8, // 语句
                (top.1 + (bot.1 - top.1) * t) as u8, // 语句
                (top.2 + (bot.2 - top.2) * t) as u8, // 语句
            ); // 块结束
            let (fx, fy) = (x as f32, y as f32); // 调用 let
            let main_peak = fy > 9.0 && fy < 25.0 && (fx - 15.0).abs() < (fy - 9.0) * 0.85; // 变量 main_peak
            let side_peak = fy > 15.0 && fy < 25.0 && (fx - 22.0).abs() < (fy - 15.0) * 0.7; // 变量 side_peak
            if main_peak || side_peak { // 条件分支
                rgba[i..i + 4].copy_from_slice(&[18, 20, 24, 255]); // 语句
            } else { // 语句
                rgba[i..i + 4].copy_from_slice(&[col.0, col.1, col.2, 255]); // 语句
            } // 块结束
        } // 块结束
    } // 块结束
    tray_icon::Icon::from_rgba(rgba, N, N).expect("32x32 RGBA icon is valid") // 调用 from_rgba
} // 块结束

/// Ask Windows to show our notification-area icon permanently (not tucked into
/// the Win11 "show hidden icons" overflow). Windows keeps this per-icon state in
/// `HKCU\Control Panel\NotifyIconSettings\<hash>` as `IsPromoted` (1 = shown).
///
/// This is best-effort and has no supported API: setting the flag may only take
/// effect after Explorer restarts or the user signs in again. Any failure is
/// silently ignored so it can never break startup.
#[cfg(windows)] // 语句
fn promote_tray_icon() { // 函数 promote_tray_icon
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE}; // 引入依赖
    use winreg::RegKey; // 引入依赖

    let Ok(exe) = std::env::current_exe() else { // 变量 Ok
        return; // 返回
    }; // 块结束
    // NOTE: don't `canonicalize` here — on Windows it returns a verbatim path with
    // a `\\?\` prefix, which would never match the plain `ExecutablePath` value
    // the shell stores in the registry.
    let exe_l = exe.to_string_lossy().to_lowercase(); // 变量 exe_l

    let hkcu = RegKey::predef(HKEY_CURRENT_USER); // 变量 hkcu
    let Ok(root) = hkcu.open_subkey_with_flags( // 变量 Ok
        r"Control Panel\NotifyIconSettings", // 语句
        KEY_READ | KEY_WRITE, // 语句
    ) else { // 语句
        return; // 返回
    }; // 块结束
    for name in root.enum_keys().flatten() { // 循环
        let Ok(key) = root.open_subkey_with_flags(&name, KEY_READ | KEY_WRITE) else { // 变量 Ok
            continue; // 语句
        }; // 块结束
        let Ok(path) = key.get_value::<String, _>("ExecutablePath") else { // 变量 Ok
            continue; // 语句
        }; // 块结束
        if path.to_lowercase() == exe_l { // 条件分支
            let _ = key.set_value("IsPromoted", &1u32); // 变量 _
        } // 块结束
    } // 块结束
} // 块结束

#[cfg(not(windows))] // 语句
#[allow(dead_code)] // 语句
fn promote_tray_icon() {} // 函数 promote_tray_icon

/// Retry `promote_tray_icon` for a little while after start-up: the shell may
/// create the per-icon registry entry only once the icon has been added.
#[allow(dead_code)] // 语句
fn promote_tray_icon_later() { // 函数 promote_tray_icon_later
    std::thread::spawn(|| { // 调用 spawn
        for delay_ms in [500u64, 1500, 3000, 6000, 12000] { // 循环
            std::thread::sleep(std::time::Duration::from_millis(delay_ms)); // 调用 sleep
            promote_tray_icon(); // 调用 promote_tray_icon
        } // 块结束
    }); // 语句
} // 块结束
