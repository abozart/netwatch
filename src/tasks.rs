//! Scheduled-task discovery for the right-click menu.
//!
//! Many telemetry/updater exes (`MicrosoftEdgeUpdate.exe`, Adobe updaters,
//! Mozilla agents, vendor crash reporters, …) aren't services — they're
//! launched by Windows Task Scheduler. Disabling the task is the right
//! "make this stop coming back" action, parallel to disabling a service.
//!
//! We snapshot the full task list once on demand (right-click → menu open),
//! filter by the row's exe path, and surface matches in the popup.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::elevate::run_powershell_capture;

#[derive(Debug, Clone, Deserialize)]
pub struct TaskInfo {
    /// e.g. "\\Mozilla\\Firefox Default Browser Agent 308046B0AF4A39CB"
    #[serde(rename = "FullPath")]
    pub full_path: String,
    /// "Ready", "Disabled", "Running", etc.
    #[serde(rename = "State")]
    #[allow(dead_code)]
    pub state: String,
}

/// Return all scheduled tasks whose first action's `Execute` field matches
/// the given exe path (case-insensitive). Synchronous — caller pays the
/// PowerShell launch cost (~300–500 ms).
pub fn find_tasks_for(exe: &Path) -> Result<Vec<TaskInfo>> {
    let exe_str = exe.to_string_lossy();
    // PS-quote: wrap in single quotes, double internal single quotes.
    let exe_quoted = exe_str.replace('\'', "''");
    let script = format!(
        "$target = '{exe}'; \
         $tasks = Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object {{ \
             $_.Actions | Where-Object {{ $_.Execute -and ($_.Execute.Trim('\"') -ieq $target) }} \
         }}; \
         $rows = @(); \
         foreach ($t in $tasks) {{ \
             $rows += [pscustomobject]@{{ \
                 FullPath = ($t.TaskPath + $t.TaskName); \
                 State    = $t.State.ToString() \
             }} \
         }}; \
         $rows | ConvertTo-Json -Compress",
        exe = exe_quoted
    );

    let out = run_powershell_capture(&script)?;
    parse_rows(&out)
}

fn parse_rows(out: &str) -> Result<Vec<TaskInfo>> {
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    // ConvertTo-Json emits a single object (not an array) when there's one row.
    let rows: Vec<TaskInfo> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)?
    } else {
        vec![serde_json::from_str(trimmed)?]
    };
    Ok(rows)
}
