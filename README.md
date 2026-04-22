# netwatch

A tiny always-on-top Windows network activity watcher. Shows a live
shrinking/expanding chart of total up/down throughput with peak and
running-average reference lines, plus a per-process table of bytes/sec,
with right-click actions to **block in firewall**, **disable service**,
or **kill process**. Transparent, draggable, pins on top, minimizes to
the system tray; the tray icon's hover tooltip mirrors the current
Up/Dn rate so you can glance at it without opening the window.

Single-binary (~4 MB), no installer, no dependencies outside Windows.

> Status: **0.1.0-alpha.2** — early and rough around the edges. Feedback
> and bug reports welcome.

## Requirements

- **Windows 11 x64** (Windows 10 should work but isn't tested every build)
- **Admin elevation on launch** — the ETW kernel-network provider that
  feeds the per-process bandwidth requires it; netwatch requests UAC
  via its embedded manifest, so a consent prompt fires every time.

### (Optional) Suppress the UAC prompt

If the per-launch prompt gets old, add yourself to the local
**Performance Log Users** group once and switch netwatch to
`asInvoker`. In an **elevated** PowerShell:

```powershell
Add-LocalGroupMember -Group "Performance Log Users" -Member $env:USERNAME
```

Then sign out + in. To make netwatch stop requesting elevation, change
`build.rs`'s `ExecutionLevel::RequireAdministrator` to `AsInvoker` and
rebuild. (We default to RequireAdministrator because it works on every
machine without any setup.)

## Download & run

1. Grab `netwatch.exe` from the [releases page](https://github.com/abozart/netwatch/releases).
2. (Optional) Verify the download:
   ```powershell
   certutil -hashfile netwatch.exe SHA256
   ```
   and compare to the SHA-256 published on the release page.
3. Double-click to launch, or run from PowerShell:
   ```powershell
   .\netwatch.exe
   ```

Settings live at `%APPDATA%\netwatch\settings.json`. Delete that file to
reset to defaults.

## Using it

- **Drag** anywhere in the custom title bar to move the window.
- **Hover the chart** to see the sample's wall-clock time and speed.
- **Resize** from the bottom-right corner. When the native title bar is
  hidden (see Options), a custom hatched grip appears there instead.
- **Right-click a process row** for the action menu:
  - **Block network (firewall)** — adds an outbound Windows Firewall
    Block rule for that exe. Needs UAC.
  - **Disable service** (only shown if the PID is a service host) —
    stops the service and sets startup to Disabled. Needs UAC.
  - **Kill process** — `Stop-Process -Force`. Needs UAC for privileged
    processes.
  - **Copy PID** / **Copy exe path** — unelevated, handy for scripts.
- **Click column headers** to sort (ascending/descending toggles).
- **Options menu** (top-right). Each toggle has a **[Set]** button for
  binding a global hotkey.
  - Opacity slider
  - Always on top
  - Show title bar (native OS chrome)
  - Show processes (table below the chart)
  - Pause (freeze sampling)
  - Click-through (mouse events pass through; the custom titlebar
    hides while this is on — `Ctrl+Alt+Shift+T` toggles it off)
  - Minimize to tray on close (intercepts the X button)
  - Show peak/avg lines (orange peak + purple running average)
  - Show chart axes & grid
  - Show background (hide the dark fill for a fully transparent overlay)
  - Hide from taskbar (also removes from Alt-Tab via `WS_EX_TOOLWINDOW`)
- **Title-bar buttons**:
  - **`_`** — hide to system tray (window disappears; restore from tray).
  - **`X`** — quit by default. If **minimize to tray on close** is
    enabled in Options, X hides to tray instead.
- **System tray icon** — right-click for **Show / hide**, **Toggle
  click-through**, **Quit**. The tooltip shows the live Up/Dn rate.
- **Window geometry persists**: size and on-screen position are saved
  on every quit path (custom X, native X, tray Quit) and restored on
  the next launch.
- **Single-instance**: launching `netwatch.exe` while it's already
  running just brings the existing window to the foreground.

### The critical hotkey

Click-through passes all mouse input through to whatever is behind
netwatch. Once it's on you can't click netwatch to reach the tray icon.
The hardcoded default **`Ctrl+Alt+Shift+T`** always toggles it off
(and on) globally. Rebind it if you want, but keep it bound to
something.

## Known issues in alpha

- **No hotkey conflict detection** — if another app owns a combo,
  binding fails silently with just a status banner.
- **Tray icon is placeholder art** (a programmatically drawn cyan ring).
- **No SmartScreen signature** — first launch on a fresh machine may
  show "Windows protected your PC". Click "More info" → "Run anyway".
- **ETW session leak** on crashes — if netwatch crashes hard,
  `logman stop netwatch-etw-<pid> -ets` in admin PS clears any
  leftover session. The code auto-cleans new `netwatch-etw-*` sessions
  at startup, so this is mostly cosmetic.

## Reporting bugs

File issues at <https://github.com/abozart/netwatch/issues>. Please
include:
- Netwatch version (Options → (shown in title bar), or
  `netwatch.exe` → Properties → Details)
- Windows build (`winver`)
- Whether you're in the Performance Log Users group or running as admin
- Minimal repro steps; screenshot helps

## Building from source

```powershell
rustup default stable
cd netwatch
cargo test           # runs regression tests (settings + FeatureToggle)
cargo build --release
.\target\release\netwatch.exe
```

Rust 1.94+ is known to work. Build artifacts live in `target/` (~1 GB
when fresh, mostly dep object files — `cargo clean` resets it).

## License

[MIT](LICENSE) — do what you want, keep the copyright notice.
