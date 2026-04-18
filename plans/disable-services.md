# Plan: Investigate-and-stop in netwatch

## Context

Telemetry/polling shows up in netwatch as ordinary processes — there's no visible difference between a service-hosted PID, a scheduled-task-launched exe, a Run-key startup entry, or a service-spawned child. The user shouldn't have to learn that taxonomy: they see a noisy row, they want it to stop. So netwatch should do the classification itself and offer the right "make this go away" action(s) per process.

This replaces the earlier service-only design — turning what would have been three or four separate UIs into one unified flow.

**Out of scope for v1:** firewall rules, killing processes outright (use Task Manager), bulk operations across many processes, undo across reboots beyond what persistence already covers.

## User flow

1. Right-click any row in netwatch → **"Investigate & stop…"**
2. A small popup opens labeled with the process name. While it queries, it shows "Investigating…".
3. Findings appear as a checklist:
   ```
   firefox.exe (PID 8964)
   ☐ Hosted service: <none>
   ☐ Scheduled task: Mozilla\Firefox Default Browser Agent 308046B0AF4A39CB
   ☐ Run key: HKCU\Software\Microsoft\Windows\CurrentVersion\Run → "C:\Program Files\Mozilla Firefox\firefox.exe ..."
   ☐ Parent process: explorer.exe (not actionable)
   ```
4. User checks one or more boxes → clicks **Apply** → single UAC prompt → netwatch performs all selected actions.
5. Row updates: traffic drops to 0, badges show what was disabled.

If the popup finds nothing actionable (e.g. Firefox itself, parent = explorer.exe), it says so plainly and offers a "Block in firewall" fallback (deferred to v2 — link out for now).

## Architecture

```
GUI process (asInvoker)             Elevated child (UAC on demand)
  ┌──────────────────┐                ┌───────────────────────────────────┐
  │ netwatch.exe     │  runas UAC →   │ netwatch.exe --apply <plan-json>  │
  │ ETW + UI         │                │ Executes batch of SCM/Task/Reg ops│
  │ Investigate UI   │  ←─ exit code  │ Prints structured result, exits   │
  └──────────────────┘                └───────────────────────────────────┘
```

Same binary, two roles. The privileged child takes a JSON blob describing the actions to perform, runs them in one shot, prints results to stdout. **One UAC prompt per Apply**, regardless of how many actions the user checked.

## Components

### 1. Responsibility detector — `src/investigate.rs` (new)

Single entry point:

```rust
pub fn investigate(pid: u32, exe_path: &Path) -> Investigation;

pub struct Investigation {
    pub pid: u32,
    pub exe_path: PathBuf,
    pub services: Vec<ServiceInfo>,        // services this PID is hosting
    pub tasks: Vec<ScheduledTaskInfo>,     // tasks whose Action targets this exe
    pub run_keys: Vec<RunKeyEntry>,        // Run/RunOnce entries pointing at exe
    pub parent: Option<ParentInfo>,        // PPID + name; flag if parent is a service
}
```

Each subfield is independently fetchable. Sub-modules:

- `services.rs` — `EnumServicesStatusExW` + filter by ProcessId. (Same as the prior service-only plan.)
- `tasks.rs` — Task Scheduler 2.0 COM (`ITaskService`) or shell-out to `Get-ScheduledTask | ? { $_.Actions.Execute -like "*$exe*" }`. Shell-out is ~30 lines; COM is ~150 lines but no PowerShell dependency. **Start with shell-out.**
- `run_keys.rs` — read `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, `RunOnce`, and the same under `HKLM`. Use `winreg` crate (small, well-maintained). Filter entries whose value contains the exe path.
- `parent.rs` — `sysinfo` already exposes `parent()`; resolve to name; if name is `services.exe` or a known service host, mark the parent as service-backed and recurse to suggest disabling that service instead.

### 2. Action executor — `src/main.rs` argv dispatch

Before GUI init, check argv:

```rust
if args.get(1).map(String::as_str) == Some("--apply") {
    let plan_json = args.get(2).expect("missing plan");
    return apply::run(plan_json);  // exits with non-zero on any failure
}
```

`apply::run` parses a JSON plan like:

```json
{
  "actions": [
    { "kind": "svc_disable",  "name": "LogiSyncSvc" },
    { "kind": "task_disable", "path": "\\Mozilla\\Firefox Default Browser Agent ..." },
    { "kind": "runkey_remove","hive": "HKCU", "key": "...", "value": "OneDrive" }
  ]
}
```

Executes each via the matching Windows API:

| Action kind         | Implementation                                                                    |
|---------------------|-----------------------------------------------------------------------------------|
| `svc_disable`       | `OpenServiceW` + `ControlService(STOP)` + `ChangeServiceConfigW(SERVICE_DISABLED)`|
| `svc_enable`        | `ChangeServiceConfigW(SERVICE_AUTO_START)` + `StartServiceW`                      |
| `task_disable`      | COM `ITaskService::GetFolder` + `IRegisteredTask::put_Enabled(VARIANT_FALSE)`     |
| `task_enable`       | same, `VARIANT_TRUE`                                                              |
| `runkey_remove`     | back up value to `%APPDATA%\netwatch\runkey-backup.json`, then delete it          |
| `runkey_restore`    | re-create from backup                                                             |

Stdout: JSON result `{ "results": [{ "ok": true }, { "ok": false, "error": "…" }] }`.

### 3. Elevation helper — `src/elevate.rs` (new)

```rust
pub fn run_elevated_apply(plan: &Plan) -> Result<Vec<ActionResult>>;
```

Implementation: `ShellExecuteExW` with `lpVerb = "runas"`, `lpFile = current_exe()`, `lpParameters = "--apply <json>"`. Wait on the child handle, capture stdout via a temp file (since elevated processes can't share parent stdout pipes via ShellExecute), parse the JSON result.

Add `Win32_UI_Shell` to `windows-sys` features.

### 4. Investigate popup — `src/app.rs`

Right-click row → context menu → **"Investigate & stop…"** → opens an `egui::Window` (modal-ish) anchored near the row.

Popup contents:
- Header: process name + PID + exe path
- Async-fetched investigation result (spawn a thread on open; show spinner until it returns)
- Checkbox list of actionable findings, grouped by kind, with a one-line description per item
- Greyed informational rows for non-actionable findings (parent process info, etc.)
- **Apply** button (disabled until ≥1 box checked) and **Cancel**
- Status footer for results after Apply

State: a `Option<InvestigationView>` field on `NetWatchApp` holding the popup state. Closes on Cancel or successful Apply.

### 5. Visual indicators — `src/app.rs`

In the Process column, append small dim tags:

- `[svc]` — has at least one hosted service
- `[task]` — has at least one matching scheduled task
- `[startup]` — has a Run-key entry
- `[blocked]` — netwatch has disabled at least one of its responsibilities (red/orange)

Tags are derived from a cheaper background sweep (every 10–15 s, just enough to keep the badges accurate) — not a full per-row Investigate.

### 6. Persistence — `src/managed.rs` (new)

File: `%APPDATA%\netwatch\managed.json`

```json
{
  "managed": [
    { "kind": "svc",      "id": "LogiSyncSvc",     "ts": "2026-04-16T22:00:00Z" },
    { "kind": "task",     "id": "\\Mozilla\\…",    "ts": "2026-04-16T22:01:00Z" },
    { "kind": "runkey",   "hive": "HKCU", "key": "…", "value": "OneDrive",
      "backup": "C:\\Users\\…\\OneDrive.exe /background", "ts": "…" }
  ]
}
```

Used to render `[blocked]` badges and to enable a future "Restore all" UI. Actual current state is always re-queried from the OS (so manual changes outside netwatch don't lie).

`runkey-backup.json` is separate so the privileged subcommand can write to it without touching the main managed file.

### 7. Cargo.toml additions

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Foundation", "Win32_Security", "Win32_System_Threading",
    "Win32_System_Diagnostics_Etw",
    # NEW for this plan:
    "Win32_System_Services",
    "Win32_UI_Shell",
    "Win32_System_Com",            # for Task Scheduler COM later (optional)
] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
winreg = "0.52"                    # Run-key reads/writes
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }
```

## File touch list

- New: [src/investigate.rs](src/investigate.rs) — orchestrator
- New: [src/services.rs](src/services.rs) — service discovery
- New: [src/tasks.rs](src/tasks.rs) — scheduled task discovery
- New: [src/run_keys.rs](src/run_keys.rs) — Run-key discovery
- New: [src/elevate.rs](src/elevate.rs) — UAC helper
- New: [src/apply.rs](src/apply.rs) — action executor (privileged subcommand)
- New: [src/managed.rs](src/managed.rs) — persistence
- Modified: [src/main.rs](src/main.rs) — `--apply` subcommand dispatch
- Modified: [src/app.rs](src/app.rs) — context menu, Investigate popup, indicators
- Modified: [src/state.rs](src/state.rs) — managed-set + per-PID indicator cache
- Modified: [Cargo.toml](Cargo.toml) — features, winreg, serde

## Verification

1. `cargo build --release` succeeds.
2. Launch as normal user — no UAC.
3. Right-click `firefox.exe` → "Investigate & stop…" → popup shows the Mozilla Default Browser Agent scheduled task.
4. Check the box → Apply → single UAC prompt → success status appears.
5. Confirm in `taskschd.msc`: that task is disabled.
6. Right-click a svchost row hosting LogiSyncSvc → Investigate → shows the service. Apply → service stopped + disabled. Row gets `[blocked]` badge.
7. Right-click `OneDrive.exe` → Investigate → shows the HKCU Run entry. Apply → entry removed (backup written). Restart Windows → OneDrive doesn't autostart.
8. Use the (future) "Restore all" button — entries return, badges clear.
9. Manually re-enable a service in services.msc → restart netwatch → badge correctly absent.

## Effort estimate

- Discovery modules (services, tasks, run_keys, parent): ~3 hr
- Investigate orchestrator + popup UI: ~2 hr
- Apply subcommand (3 action kinds via Win API/COM/registry): ~3 hr
- Elevation helper: ~1 hr
- Persistence + badges: ~1.5 hr
- Polish + verification: ~1.5 hr

Total: roughly **1.5 days**, vs ~half day for the prior service-only design — the extra time is mostly the COM-or-shell-out for tasks plus the popup UI.

## Open design questions

- **Inline checklist vs. wizard popup?** A modal popup is more discoverable; an inline expandable row is more compact. Recommendation: modal popup for v1; revisit if it feels heavy.
- **Auto-investigate on hover, or only on right-click?** Auto would be nice but spawns a Get-ScheduledTask shell every hover — too costly. Stick with right-click.
- **Show "Block in firewall" as a v1 fallback?** Would require dragging in firewall logic ahead of its own plan. Defer.
- **Group rows by exe path so we don't investigate the same firefox.exe N times for N PIDs?** Cache investigation results by exe-path with a short TTL (e.g. 30 s). Worth doing on day one — code is small.

## Out of scope reminder

- Process kill (use Task Manager — netwatch shouldn't reinvent it)
- Firewall rules (separate, smaller plan if you ever want them)
- Network isolation per app (would require WFP driver — not happening)
- Editing service ACLs / TrustedInstaller-protected services (Windows blocks these for good reason)
