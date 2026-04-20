//! System-tray icon + context menu. Menu events are drained on a dedicated
//! background thread so they keep working even while the window is hidden
//! (when eframe's update loop may idle). Actions are applied directly via
//! Win32 for show/hide/always-on-top so we don't depend on the egui viewport
//! command pipeline being alive.

use anyhow::Result;
use parking_lot::RwLock;
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::features::FeatureToggle;
use crate::state::AppState;

pub struct Tray {
    icon: TrayIcon, // keep alive for lifetime of app; also used for tooltip updates
    pub show_hide_id: MenuId,
    pub toggle_click_id: MenuId,
    pub quit_id: MenuId,
}

impl Tray {
    pub fn new() -> Result<Self> {
        let show_hide = MenuItem::new("Show / hide", true, None);
        let toggle_click = MenuItem::new("Toggle click-through", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let show_hide_id = show_hide.id().clone();
        let toggle_click_id = toggle_click.id().clone();
        let quit_id = quit.id().clone();

        let menu = Menu::new();
        menu.append(&show_hide)?;
        menu.append(&toggle_click)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit)?;

        let icon = make_icon()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("netwatch")
            .build()?;

        Ok(Self {
            icon: tray,
            show_hide_id,
            toggle_click_id,
            quit_id,
        })
    }

    /// Update the tray-icon hover tooltip. Called once per UI tick from
    /// `NetWatchApp::update` with the live Up/Dn rates so the user can read
    /// the current speed by hovering over the tray icon alone.
    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }

    /// Spawn the background thread that processes tray menu events. Lives for
    /// the duration of the process (no graceful shutdown — `Quit` calls
    /// `std::process::exit` which terminates everything).
    pub fn spawn_event_thread(
        &self,
        state: Arc<RwLock<AppState>>,
        ctx: eframe::egui::Context,
    ) {
        let show_hide_id = self.show_hide_id.clone();
        let toggle_click_id = self.toggle_click_id.clone();
        let quit_id = self.quit_id.clone();

        std::thread::spawn(move || loop {
            // Blocking recv so we don't spin. Every action below triggers a
            // repaint so the UI reflects any state mutation promptly.
            let Ok(ev) = MenuEvent::receiver().recv() else {
                break;
            };
            if ev.id == quit_id {
                // Save settings synchronously so opacity, window geometry,
                // etc. persist before death. Window rect lives in AppState
                // because this runs off-thread from NetWatchApp — see
                // NetWatchApp::update where it's refreshed each frame.
                let (size, pos, snap) = {
                    let s = state.read();
                    (
                        s.last_window_size,
                        s.last_window_pos,
                        crate::settings::Settings::capture_from(&s),
                    )
                };
                let snap = snap.with_window_rect(size, pos);
                snap.save();
                #[cfg(windows)]
                crate::etw::shutdown();
                std::process::exit(0);
            } else if ev.id == show_hide_id {
                #[cfg(windows)]
                toggle_window_visibility(&state);
                ctx.request_repaint();
            } else if ev.id == toggle_click_id {
                let new_val = {
                    let mut st = state.write();
                    let cur = FeatureToggle::ClickThrough.get(&st);
                    FeatureToggle::ClickThrough.write_state(&mut st, !cur);
                    !cur
                };
                #[cfg(windows)]
                if let Some(hwnd) = find_netwatch_hwnd() {
                    crate::click_through::set(hwnd, new_val);
                }
                ctx.request_repaint();
            }
        });
    }
}

fn make_icon() -> Result<Icon> {
    let size = crate::defaults::TRAY_ICON_SIZE;
    let rgba = crate::icon::ring_rgba(size);
    Ok(Icon::from_rgba(rgba, size, size)?)
}

#[cfg(windows)]
fn find_netwatch_hwnd() -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let title: Vec<u16> = "netwatch"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        None
    } else {
        Some(hwnd as isize)
    }
}

/// Flip the window between hidden and visible via direct Win32, bypassing
/// egui's viewport command queue (which may be idle while the window is
/// hidden). Re-applies AlwaysOnTop on show because Windows can drop the
/// topmost flag while a window is hidden.
#[cfg(windows)]
fn toggle_window_visibility(state: &Arc<RwLock<AppState>>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_NOTOPMOST,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOW,
    };
    let Some(hwnd_isize) = find_netwatch_hwnd() else {
        return;
    };
    let hwnd = hwnd_isize as *mut core::ffi::c_void;
    unsafe {
        if IsWindowVisible(hwnd) != 0 {
            ShowWindow(hwnd, SW_HIDE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
            let on_top = FeatureToggle::AlwaysOnTop.get(&state.read());
            let insert_after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            SetForegroundWindow(hwnd);
        }
    }
}
