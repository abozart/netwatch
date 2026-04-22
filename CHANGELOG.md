# Changelog

All notable changes to netwatch. Newest first.

## 0.1.0-alpha.2 — 2026-04-22

Second alpha. Internal refactor plus a new user-facing option and
several hardening fixes surfaced by two audit passes.

### Added
- **Chart style** option (Line / Area / Bars) selectable from the
  Options menu and persisted to `settings.json`. Line is the existing
  thin sparkline; Area fills to the y=0 baseline; Bars interleaves up
  and dn at each sample (width 0.45 with ±0.25 offset, ~90 % of the
  per-sample slot).
- Options menu now renders toggles **2 per line** with a full-width
  fallback for an odd tail. Chart-style selector sits above the
  toggle grid.
- Tray icon now renders Segoe UI Bold at 9 px via `ab_glyph`
  (read from `%WINDIR%\Fonts` at startup) — proper glyphs instead of
  hand-tuned bitmaps. Falls back to the static ring icon if the font
  can't be loaded.
- Chart peak labels never overlap: Dn sits above the chart baseline,
  Up sits below, separated by the full 22 px peak-label strip.

### Changed
- **Refactor**: every widget moved out of `src/app.rs` into
  `src/ui/*.rs` (chart, titlebar, options_menu, table, action_menu,
  resize_grip, hotkey_modal). `app.rs` drops from ~1240 lines to
  ~350 lines of pure lifecycle/orchestration. Method-call sites in
  `update()` are byte-identical — widgets attach to `NetWatchApp`
  via split `impl` blocks.
- `find_netwatch_hwnd` deduped into `src/win32.rs` (was copy-pasted
  across `app.rs`, `tray.rs`, and `single_instance.rs`).
- Tray tooltip / icon format: compact 3-char display with decimal
  promotion — a 313 KB/s rate now shows as `.3M` instead of silently
  capping at `99K`.

### Fixed
- Firewall rule scripts now escape embedded single quotes in the
  exe's basename before interpolating into the PowerShell literal,
  matching the existing treatment of service and scheduled-task
  names. An exe whose filename contained `'` could otherwise break
  the script.
- ETW + services refresher threads respond to an `AtomicBool`
  shutdown flag instead of relying on `std::process::exit` to kill
  them. Tear-down now takes effect within ~100–250 ms.
- `main()` installs a panic hook that calls `etw::shutdown` /
  `services::shutdown` before `panic = "abort"` kills the process —
  prevents an `AlreadyExist` failure-to-launch on the next run if
  any thread panics.
- Click-through `WS_EX_TRANSPARENT` flag is no longer written via
  `SetWindowLongW` every frame — cached last value, only re-applied
  on change.
- Tray event thread logs the `recv` error to stderr before exiting
  instead of silently dropping out of the loop.
- Tray icon renderer returns `Option<Vec<u8>>`; the caller skips the
  Win32 update when the font is unavailable so the static ring icon
  stays visible instead of going blank.

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
- Embedded manifest requests `RequireAdministrator` so the ETW
  kernel-network provider is always reachable on first launch (UAC
  prompts every run; can be swapped to `AsInvoker` + Performance Log
  Users group for prompt-free use — see README).
- ETW session auto-cleanup on startup via `QueryAllTracesW`: enumerates
  every active session and stops any whose name starts with
  `netwatch-etw`, preventing the "N leaked PID-named sessions compete
  for provider events and starve the new one" failure mode.
- `defaults.rs` as a single-source-of-truth for every hardcoded default
  (opacity bounds, window sizes, chart heights, icon sizes, timing
  intervals, safety hotkey). Every other module reads from it.
- Chart height grows with window height when the process table is
  hidden; stays fixed when visible.
- ETW diagnostic banner with `started`, `seen`, `matched`, `parse_err`
  counters — visible only in debug builds (release builds rely on the
  `etw_error` banner for failures).
- Right-click → **Disable scheduled task: <name>** when the row's exe
  is referenced by one or more Windows Task Scheduler tasks (catches
  Mozilla / Edge / vendor updaters that aren't services). Re-enable
  available via the same menu.
- Conditional manifest: release builds request `RequireAdministrator`,
  debug builds (and `cargo test`) stay `AsInvoker` so the test harness
  can launch unattended.
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
