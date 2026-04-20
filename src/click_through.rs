//! Toggle `WS_EX_TRANSPARENT` on the netwatch window so mouse events pass
//! through to whatever's underneath. `WS_EX_LAYERED` must stay set (eframe
//! enables it via `with_transparent(true)`) — we only flip TRANSPARENT.
//!
//! Critical UX note: while click-through is on, the user cannot click anything
//! in netwatch — not even the tray icon's underlying window. The global hotkey
//! registered for `FeatureToggle::ClickThrough` (default Ctrl+Alt+Shift+T) is
//! the only way back, so its registration must not silently fail.

#[cfg(windows)]
pub fn set(hwnd: isize, on: bool) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    let hwnd = hwnd as HWND;
    unsafe {
        let mut styles = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        // Ensure LAYERED stays on; some code paths outside eframe could clear it.
        styles |= WS_EX_LAYERED;
        if on {
            styles |= WS_EX_TRANSPARENT;
        } else {
            styles &= !WS_EX_TRANSPARENT;
        }
        SetWindowLongW(hwnd, GWL_EXSTYLE, styles as i32);
    }
}

#[cfg(not(windows))]
pub fn set(_hwnd: isize, _on: bool) {}

/// Flip `WS_EX_TOOLWINDOW` on the netwatch window. Toolwindows are excluded
/// from both the taskbar and Alt-Tab, which is what "Hide from taskbar" gives
/// the user. Unlike the click-through flag, taskbar inclusion is only re-read
/// by the shell during window show, so we cycle the visibility around the
/// style change to force the taskbar to refresh.
#[cfg(windows)]
pub fn set_toolwindow(hwnd: isize, on: bool) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, IsWindowVisible, SetWindowLongW, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOW,
        WS_EX_TOOLWINDOW,
    };

    let hwnd = hwnd as HWND;
    unsafe {
        let was_visible = IsWindowVisible(hwnd) != 0;
        if was_visible {
            ShowWindow(hwnd, SW_HIDE);
        }
        let mut styles = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if on {
            styles |= WS_EX_TOOLWINDOW;
        } else {
            styles &= !WS_EX_TOOLWINDOW;
        }
        SetWindowLongW(hwnd, GWL_EXSTYLE, styles as i32);
        if was_visible {
            ShowWindow(hwnd, SW_SHOW);
        }
    }
}

#[cfg(not(windows))]
pub fn set_toolwindow(_hwnd: isize, _on: bool) {}
