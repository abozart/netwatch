use anyhow::Result;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::actions::RULE_PREFIX;
use crate::elevate::run_powershell_capture;
use crate::state::AppState;

/// Flipped by [`shutdown`] so the refresher thread spawned by
/// [`spawn_refresher`] exits its poll loop instead of surviving past
/// process exit. Paired with `etw::TICK_STOP` for symmetry; both signal the
/// same event (app is going away).
static REFRESH_STOP: AtomicBool = AtomicBool::new(false);

/// Tell the background refresher thread to stop. Best-effort: if the
/// thread is currently mid-PowerShell-capture it'll finish that call
/// before checking the flag. `on_exit`'s `std::process::exit` would kill
/// it forcibly either way; this just makes graceful shutdown possible.
pub fn shutdown() {
    REFRESH_STOP.store(true, Ordering::Relaxed);
}

#[derive(Debug, Deserialize)]
struct SvcRow {
    #[serde(rename = "ProcessId")]
    process_id: u32,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "DisplayName")]
    #[allow(dead_code)]
    display_name: Option<String>,
    #[serde(rename = "State")]
    #[allow(dead_code)]
    state: Option<String>,
    #[serde(rename = "StartMode")]
    #[allow(dead_code)]
    start_mode: Option<String>,
}

fn refresh_services_once() -> Result<HashMap<u32, Vec<String>>> {
    // Only include rows where ProcessId > 0 (running services).
    let out = run_powershell_capture(
        "Get-CimInstance Win32_Service | Where-Object ProcessId -gt 0 | \
         Select-Object ProcessId,Name,DisplayName,State,StartMode | \
         ConvertTo-Json -Compress -Depth 2",
    )?;
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return Ok(HashMap::new());
    }

    // ConvertTo-Json emits a bare object when there is only one row; normalise to an array.
    let rows: Vec<SvcRow> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)?
    } else {
        vec![serde_json::from_str(trimmed)?]
    };

    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    for r in rows {
        if r.process_id == 0 {
            continue;
        }
        map.entry(r.process_id).or_default().push(r.name);
    }
    Ok(map)
}

fn refresh_firewall_blocks_once() -> Result<HashSet<PathBuf>> {
    // List program paths of firewall rules we own (name starts with our prefix).
    // Get-NetFirewallApplicationFilter needs to be pipelined from Get-NetFirewallRule.
    let script = format!(
        "$rules = Get-NetFirewallRule -DisplayName '{RULE_PREFIX}*' -ErrorAction SilentlyContinue;\
         if ($rules) {{ $rules | Get-NetFirewallApplicationFilter | Select-Object -ExpandProperty Program }}"
    );
    let out = run_powershell_capture(&script)?;
    let mut set = HashSet::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("Any") {
            continue;
        }
        set.insert(PathBuf::from(line));
    }
    Ok(set)
}

/// Spawn a background thread that refreshes service map and firewall-blocked set.
pub fn spawn_refresher(state: Arc<RwLock<AppState>>) {
    std::thread::spawn(move || loop {
        if REFRESH_STOP.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(map) = refresh_services_once() {
            state.write().services = map;
        }
        if let Ok(set) = refresh_firewall_blocks_once() {
            state.write().fw_blocked = set;
        }
        // Split the sleep so `shutdown()` takes effect within ~250 ms
        // instead of after the full refresh interval (8 s default).
        let ticks = crate::defaults::SERVICE_REFRESH_INTERVAL_SECS * 4;
        for _ in 0..ticks {
            if REFRESH_STOP.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}
