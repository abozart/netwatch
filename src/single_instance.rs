//! Single-instance enforcement via a named Win32 mutex.
//!
//! If another netwatch.exe is already running, we surface its window (restore
//! from tray/minimized, then bring to foreground) and tell the caller to bail
//! out of startup — no second UI, tray icon, ETW session, or hotkey registry.

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};

/// Returns `true` if this process got the lock and should proceed with
/// startup. Returns `false` if another instance already holds the lock — in
/// which case we've already tried to focus that existing window, and the
/// caller should exit.
pub fn acquire_or_focus_existing() -> bool {
    let name: Vec<u16> = "netwatch-single-instance\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        // Fail open: if mutex creation itself fails (unusual), don't block
        // the user from launching netwatch.
        return true;
    }
    let last_err = unsafe { GetLastError() };
    if last_err == ERROR_ALREADY_EXISTS {
        focus_existing_window();
        return false;
    }
    // HANDLE is Copy and has no Drop, so the mutex isn't auto-closed when
    // `handle` falls out of scope. We never call CloseHandle; the kernel
    // releases the mutex when this process exits, which is exactly what we
    // want for the lifetime of the lock.
    let _ = handle;
    true
}

fn focus_existing_window() {
    let Some(hwnd_isize) = crate::win32::find_netwatch_hwnd() else {
        return;
    };
    let hwnd = hwnd_isize as *mut core::ffi::c_void;
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
}
