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
    pub fn install(ctx: egui::Context) -> Option<Tray> {
        // Android has no system tray: report "no tray" so the app falls back to
        // normal window behaviour (hide-to-tray buttons are hidden on mobile).
        #[cfg(target_os = "android")]
        {
            let _ = ctx;
            return None;
        }
        #[cfg(not(target_os = "android"))]
        {
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

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("wallpaper-rs · 桌面壁纸")
            .with_icon(make_icon())
            // Left click restores the window; the menu is on right click only.
            .with_menu_on_left_click(false)
            .build()
            .ok()?;

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
                    queue.lock().unwrap().push_back(a);
                    ctx.request_repaint();
                }
            }));
        }

        // Left click / double click on the icon -> show the window.
        {
            let queue = queue.clone();
            let ctx = ctx.clone();
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
