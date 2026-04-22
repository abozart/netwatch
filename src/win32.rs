//! Thin Win32 helpers shared across modules. Windows-only by construction —
//! everything here wraps an API in `windows_sys::Win32`, so there's no
//! `cfg(not(windows))` path; callers that need to build cross-platform
//! already gate their use of this module with `cfg(windows)`.

use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

/// Locate the netwatch window's HWND by its title. Returns `None` if no
/// window with that title is present (e.g. on first frame before the window
/// is registered).
///
/// Used by:
/// - `NetWatchApp::update` to resolve `self.hwnd` once at startup for
///   Win32-level toggles (click-through, hide-from-taskbar).
/// - `tray::toggle_window_visibility` to show/hide without going through
///   eframe's viewport-command queue (which can idle while hidden).
/// - `single_instance::focus_existing_window` to surface the pre-existing
///   instance when a second launch is attempted.
///
/// The title "netwatch" is set via `ViewportBuilder::with_title` in
/// `main.rs`; keep these two in sync.
pub fn find_netwatch_hwnd() -> Option<isize> {
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
