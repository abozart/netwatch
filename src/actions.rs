use anyhow::Result;
use std::path::Path;

use crate::elevate::{ps_quote, run_elevated_powershell};

/// Stable firewall-rule name prefix so we can enumerate our own rules later.
pub const RULE_PREFIX: &str = "netwatch-block";

fn rule_name_for(exe: &Path) -> String {
    let stem = exe
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{RULE_PREFIX}-{stem}")
}

/// Add an outbound Block firewall rule for the given exe.
pub fn block_firewall(exe: &Path) -> Result<i32> {
    let name = rule_name_for(exe);
    let script = format!(
        "if (-not (Get-NetFirewallRule -DisplayName '{name}' -ErrorAction SilentlyContinue)) {{\
           New-NetFirewallRule -DisplayName '{name}' -Direction Outbound \
             -Program {exe} -Action Block -Profile Any | Out-Null\
         }}",
        name = name,
        exe = ps_quote(exe),
    );
    run_elevated_powershell(&script)
}

/// Remove our outbound Block firewall rule for the given exe (if present).
pub fn unblock_firewall(exe: &Path) -> Result<i32> {
    let name = rule_name_for(exe);
    let script = format!(
        "Get-NetFirewallRule -DisplayName '{name}' -ErrorAction SilentlyContinue | \
           Remove-NetFirewallRule -ErrorAction SilentlyContinue"
    );
    run_elevated_powershell(&script)
}

/// Stop the service and set startup type to Disabled.
pub fn disable_service(name: &str) -> Result<i32> {
    let safe = name.replace('\'', "''");
    let script = format!(
        "Stop-Service -Name '{n}' -Force -ErrorAction SilentlyContinue; \
         Set-Service -Name '{n}' -StartupType Disabled",
        n = safe
    );
    run_elevated_powershell(&script)
}

/// Restore the service to Automatic startup and start it.
pub fn enable_service(name: &str) -> Result<i32> {
    let safe = name.replace('\'', "''");
    let script = format!(
        "Set-Service -Name '{n}' -StartupType Automatic; \
         Start-Service -Name '{n}' -ErrorAction SilentlyContinue",
        n = safe
    );
    run_elevated_powershell(&script)
}

/// Kill the given PID. Elevated because some processes (services/system) require it.
pub fn kill_process(pid: u32) -> Result<i32> {
    let script = format!("Stop-Process -Id {pid} -Force");
    run_elevated_powershell(&script)
}

/// Disable a scheduled task by full path (e.g. `\Mozilla\Firefox Default …`).
/// Stops it from re-launching at its trigger times.
pub fn disable_scheduled_task(full_path: &str) -> Result<i32> {
    let safe = full_path.replace('\'', "''");
    let script = format!("Disable-ScheduledTask -TaskPath (Split-Path '{p}' -Parent) -TaskName (Split-Path '{p}' -Leaf) | Out-Null", p = safe);
    run_elevated_powershell(&script)
}

/// Re-enable a scheduled task previously disabled.
pub fn enable_scheduled_task(full_path: &str) -> Result<i32> {
    let safe = full_path.replace('\'', "''");
    let script = format!("Enable-ScheduledTask -TaskPath (Split-Path '{p}' -Parent) -TaskName (Split-Path '{p}' -Leaf) | Out-Null", p = safe);
    run_elevated_powershell(&script)
}
