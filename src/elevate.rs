use anyhow::{bail, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// Run a PowerShell script elevated (triggers UAC if we're not already admin).
/// Returns when the elevated PowerShell exits; its exit code is returned.
pub fn run_elevated_powershell(script: &str) -> Result<i32> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, WaitForSingleObject, INFINITE,
    };
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let verb = wide("runas");
    let file = wide("powershell.exe");
    // -NoProfile avoids user profile side effects. -ExecutionPolicy Bypass lets arbitrary
    // inline scripts run in the elevated context without hitting policy.
    let args = format!(
        "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command {}",
        wrap_script(script)
    );
    let args_w = wide(&args);

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = args_w.as_ptr();
    info.nShow = SW_HIDE;

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let e = std::io::Error::last_os_error();
        bail!("ShellExecuteExW failed: {e}");
    }
    if info.hProcess.is_null() {
        bail!("ShellExecuteExW returned no process handle (user may have cancelled UAC)");
    }

    unsafe {
        let waited = WaitForSingleObject(info.hProcess, INFINITE);
        if waited != WAIT_OBJECT_0 {
            CloseHandle(info.hProcess);
            bail!("WaitForSingleObject failed");
        }
        let mut code: u32 = 0;
        let got = GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        if got == 0 {
            bail!("GetExitCodeProcess failed");
        }
        Ok(code as i32)
    }
}

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Wrap a script so it survives being passed as a single `-Command` arg.
/// PowerShell's `-Command` joins remaining tokens; wrapping with `&{ ... }`
/// lets multi-statement scripts work and preserves quoting.
fn wrap_script(script: &str) -> String {
    format!("\"& {{ {} }}\"", script.replace('"', "`\""))
}

/// Run an unelevated PowerShell script, capturing stdout. Used for read-only queries.
pub fn run_powershell_capture(script: &str) -> Result<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let out = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !out.status.success() {
        bail!(
            "powershell exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Quote a Windows path for PowerShell single-quoted string context.
pub fn ps_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "''"))
}
