//! System tray icon (Windows notification area / macOS menu bar / Linux status
//! area). The main window is *hidden* here when the user clicks its "X", so the
//! app keeps running (slideshow / daily wallpaper) while staying out of the way.
//!
//! `tray-icon` delivers menu and icon events through global handlers. We attach
//! handlers once at start-up that push an action into a shared queue and ask
//! egui to repaint, so the UI thread picks the action up on its next frame.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[cfg(not(target_os = "android"))]
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
#[cfg(not(target_os = "android"))]
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

/// An action requested from the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Show and focus the main window.
    Show,
    /// Hide the main window to the tray.
    Hide,
    /// Really exit the process (not just hide).
    Quit,
}

/// Owns the live tray icon and drains the actions produced by its handlers.
pub struct Tray {
    /// Kept alive for the lifetime of the app; dropping it removes the icon.
    #[cfg(not(target_os = "android"))]
    _icon: tray_icon::TrayIcon,
    queue: Arc<Mutex<VecDeque<TrayAction>>>,
}

impl Tray {
    /// Build the tray icon + context menu. Returns `None` if the platform has no
    /// usable tray backend (the caller then falls back to the old behaviour).
    pub fn install(ctx: egui::Context, window: isize) -> Option<Tray> {
        // Android has no system tray: report "no tray" so the app falls back to
        // normal window behaviour (hide-to-tray buttons are hidden on mobile).
        #[cfg(target_os = "android")]
        {
            let _ = ctx;
            let _ = window;
            return None;
        }
        #[cfg(not(target_os = "android"))]
        {
        // The main window HWND (Windows) so tray actions can be applied directly,
        // without waiting for the egui frame loop.
        let win_hwnd = window;
        #[cfg(not(windows))]
        let _ = win_hwnd;
        let menu = Menu::new();
        let show = MenuItem::new("显示主界面", true, None);
        let hide = MenuItem::new("隐藏到托盘", true, None);
        let quit = MenuItem::new("退出 wallpaper-rs", true, None);
        menu.append(&show).ok()?;
        menu.append(&hide).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit).ok()?;

        let show_id = show.id().clone();
        let hide_id = hide.id().clone();
        let quit_id = quit.id().clone();

        // Ask Windows to keep our tray icon in the always-visible part of the
        // notification area (instead of the Win11 overflow "hidden icons" menu).
        // Best-effort: the system may only apply it after Explorer restarts or the
        // user signs in again, so we also retry shortly after the icon is added.
        promote_tray_icon();
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("wallpaper-rs · 桌面壁纸")
            .with_icon(make_icon())
            // Left click restores the window; the menu is on right click only.
            .with_menu_on_left_click(false)
            .build()
            .ok()?;
        // The shell may create the NotifyIconSettings registry entry only after the
        // icon is registered; retry a few times in the background to pin it.
        promote_tray_icon_later();

        let queue: Arc<Mutex<VecDeque<TrayAction>>> = Arc::new(Mutex::new(VecDeque::new()));

        // Menu clicks -> actions.
        {
            let queue = queue.clone();
            let ctx = ctx.clone();
            MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
                let action = if ev.id == show_id {
                    Some(TrayAction::Show)
                } else if ev.id == hide_id {
                    Some(TrayAction::Hide)
                } else if ev.id == quit_id {
                    Some(TrayAction::Quit)
                } else {
                    None
                };
                if let Some(a) = action {
                    // Apply immediately on the native thread so the action works
                    // even if the egui frame loop is idle (e.g. hidden to tray).
                    #[cfg(windows)]
                    match a {
                        TrayAction::Show => unsafe { crate::winhide::show_from_tray(win_hwnd) },
                        TrayAction::Hide => unsafe { crate::winhide::hide_to_tray(win_hwnd) },
                        TrayAction::Quit => {
                            // Safety net: exit even if the UI loop never wakes.
                            std::thread::spawn(|| {
                                // Kill sibling instances first, then ourselves.
                                crate::instances::terminate_other_instances();
                                std::thread::sleep(std::time::Duration::from_millis(2500));
                                std::process::exit(0);
                            });
                        }
                    }
                    // Also enqueue so the UI can sync state / status text.
                    queue.lock().unwrap().push_back(a);
                    ctx.request_repaint();
                }
            }));
        }

        // Left click / double click on the icon -> show the window.
        {
            let queue = queue.clone();
            let ctx = ctx.clone();
            let win_hwnd_lc = win_hwnd;
            TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
                let show = matches!(
                    ev,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );
                if show {
                    #[cfg(windows)]
                    unsafe {
                        crate::winhide::show_from_tray(win_hwnd_lc)
                    };
                    queue.lock().unwrap().push_back(TrayAction::Show);
                    ctx.request_repaint();
                }
            }));
        }

        Some(Tray { _icon: icon, queue })
        }
    }

    /// Take every action queued since the last call (non-blocking).
    pub fn drain(&self) -> Vec<TrayAction> {
        let mut q = self.queue.lock().unwrap();
        q.drain(..).collect()
    }
}

/// A small procedurally-drawn 32×32 icon (amber→teal disc with a dark mountain
/// silhouette) so no external image asset is required at build time.
#[cfg(not(target_os = "android"))]
fn make_icon() -> tray_icon::Icon {
    const N: u32 = 32;
    let mut rgba = vec![0u8; (N * N * 4) as usize];
    let c = (N as f32 - 1.0) / 2.0;
    let radius = 15.0_f32;
    for y in 0..N {
        for x in 0..N {
            let i = ((y * N + x) * 4) as usize;
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            if dx * dx + dy * dy > radius * radius {
                continue; // transparent outside the disc
            }
            let t = y as f32 / (N as f32 - 1.0);
            let top = (232.0, 176.0, 75.0);
            let bot = (79.0, 209.0, 197.0);
            let col = (
                (top.0 + (bot.0 - top.0) * t) as u8,
                (top.1 + (bot.1 - top.1) * t) as u8,
                (top.2 + (bot.2 - top.2) * t) as u8,
            );
            let (fx, fy) = (x as f32, y as f32);
            let main_peak = fy > 9.0 && fy < 25.0 && (fx - 15.0).abs() < (fy - 9.0) * 0.85;
            let side_peak = fy > 15.0 && fy < 25.0 && (fx - 22.0).abs() < (fy - 15.0) * 0.7;
            if main_peak || side_peak {
                rgba[i..i + 4].copy_from_slice(&[18, 20, 24, 255]);
            } else {
                rgba[i..i + 4].copy_from_slice(&[col.0, col.1, col.2, 255]);
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, N, N).expect("32x32 RGBA icon is valid")
}

/// Ask Windows to show our notification-area icon permanently (not tucked into
/// the Win11 "show hidden icons" overflow). Windows keeps this per-icon state in
/// `HKCU\Control Panel\NotifyIconSettings\<hash>` as `IsPromoted` (1 = shown).
///
/// This is best-effort and has no supported API: setting the flag may only take
/// effect after Explorer restarts or the user signs in again. Any failure is
/// silently ignored so it can never break startup.
#[cfg(windows)]
fn promote_tray_icon() {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // NOTE: don't `canonicalize` here — on Windows it returns a verbatim path with
    // a `\\?\` prefix, which would never match the plain `ExecutablePath` value
    // the shell stores in the registry.
    let exe_l = exe.to_string_lossy().to_lowercase();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(root) = hkcu.open_subkey_with_flags(
        r"Control Panel\NotifyIconSettings",
        KEY_READ | KEY_WRITE,
    ) else {
        return;
    };
    for name in root.enum_keys().flatten() {
        let Ok(key) = root.open_subkey_with_flags(&name, KEY_READ | KEY_WRITE) else {
            continue;
        };
        let Ok(path) = key.get_value::<String, _>("ExecutablePath") else {
            continue;
        };
        if path.to_lowercase() == exe_l {
            let _ = key.set_value("IsPromoted", &1u32);
        }
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
fn promote_tray_icon() {}

/// Retry `promote_tray_icon` for a little while after start-up: the shell may
/// create the per-icon registry entry only once the icon has been added.
#[allow(dead_code)]
fn promote_tray_icon_later() {
    std::thread::spawn(|| {
        for delay_ms in [500u64, 1500, 3000, 6000, 12000] {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            promote_tray_icon();
        }
    });
}
