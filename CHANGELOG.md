# Changelog

All notable changes to netwatch. Newest first.

## 0.1.0-alpha.1 — 2026-04-18

First alpha. Everything below is functional but rough.

### Added
- Live total-throughput chart (up + down sparklines, y-axis auto-rescales).
- Per-process table with PID, name, up/s, down/s, up total, down total.
  Click column headers to sort (asc/desc toggle). Fixed column widths so
  values don't reflow as magnitudes change.
- Right-click process row → **Block network (firewall)** /
  **Disable service** / **Kill process** / **Copy PID** /
  **Copy exe path**. Popup snapshots the target so re-sorting the table
  while the menu is open can't redirect actions to a different process.
- Process row indicators: `[svc]` (hosts one or more services),
  `[blocked]` (netwatch has an outbound firewall rule for this exe).
- Transparent window with per-pixel alpha; opacity slider (15%–100%).
- Draggable anywhere in the title bar; subtle lighter menu-bar fill.
- Options menu toggles for: always on top, show title bar, show
  processes, pause, click-through, and **minimize to tray on close**.
  Each has a **[Set]** button that captures a global hotkey combo.
- Title-bar buttons: **`_`** explicitly hides to tray; **`X`** quits by
  default (or hides to tray if the Options toggle is enabled).
- System tray icon with **Show/Hide**, **Toggle click-through**,
  **Quit**. Tray events run on a dedicated thread so they work even
  while the window is hidden.
- Hardcoded safety hotkey `Ctrl+Alt+Shift+T` for click-through (always
  bound by default so the user can escape a stuck click-through state).
- Settings persisted to `%APPDATA%\netwatch\settings.json`: opacity,
  toggles, sort state, per-feature hotkey bindings.
- ETW session auto-cleanup on startup via `QueryAllTracesW`: enumerates
  every active session and stops any whose name starts with
  `netwatch-etw`, preventing the "N leaked PID-named sessions compete
  for provider events and starve the new one" failure mode.
- `defaults.rs` as a single-source-of-truth for every hardcoded default
  (opacity bounds, window sizes, chart heights, icon sizes, timing
  intervals, safety hotkey). Every other module reads from it.
- Chart height grows with window height when the process table is
  hidden; stays fixed when visible.
- ETW diagnostic banner showing `started`, `seen`, `matched`,
  `parse_err` counters so alpha users can triage data-pipeline issues
  without attaching a debugger.
- `FeatureToggle` enum as the single source of truth for every
  user-toggleable option; Options UI, tray, hotkey manager, and
  settings all iterate it.
- Regression tests (`cargo test`):
  - `settings_roundtrip_identity`
  - `settings_defaults_include_click_through_hotkey`
  - `feature_toggle_setter_roundtrip`
  - `feature_toggle_settings_keys_unique`
- Single self-contained x64 exe (~4.6 MB) — statically links the MSVC
  C runtime via `+crt-static`, no external DLL dependencies.

### Known issues
- No hotkey conflict detection; binding failures show only a banner.
- Minimize-to-tray on X close is surprising on first use.
- Tray icon is a programmatic cyan ring — placeholder art.
- Not code-signed; SmartScreen may warn on first launch.
- PowerShell shell-out for scheduled tasks / services isn't gated on
  the user's privilege level — actions can fail with unfriendly errors.
