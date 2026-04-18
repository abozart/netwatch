# netwatch

A tiny always-on-top Windows network activity watcher. Shows a live
shrinking/expanding chart of total up/down throughput plus a per-process
table of bytes/sec, with right-click actions to **block in firewall**,
**disable service**, or **kill process**. Transparent, draggable, pins
on top, minimizes to the system tray.

Single-binary (~4 MB), no installer, no dependencies outside Windows.

> Status: **0.1.0-alpha.1** — early and rough around the edges. Feedback
> and bug reports welcome.

## Requirements

- **Windows 11 x64** (Windows 10 should work but isn't tested every build)
- One-time ETW setup so you don't get a UAC prompt on every launch (see below)

## One-time setup: ETW access

Netwatch uses the `Microsoft-Windows-Kernel-Network` ETW provider for
per-process byte counts. That provider requires either Administrator or
membership in the local **Performance Log Users** group.

The cleanest path (no UAC prompts afterward) is to add yourself to that
group once. In an **elevated** PowerShell:

```powershell
Add-LocalGroupMember -Group "Performance Log Users" -Member $env:USERNAME
```

Then **sign out and back in** (group membership only takes effect at
logon). Launch `netwatch.exe` normally — no UAC.

If you skip this, netwatch will show a red banner explaining the same
command. You can also right-click netwatch.exe → "Run as administrator"
for a one-off.

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

- **Drag** anywhere in the title bar to move the window.
- **Right-click a process row** for the action menu:
  - **Block network (firewall)** — adds an outbound Windows Firewall
    Block rule for that exe. Needs UAC.
  - **Disable service** (only shown if the PID is a service host) —
    stops the service and sets startup to Disabled. Needs UAC.
  - **Kill process** — `Stop-Process -Force`. Needs UAC for privileged
    processes.
  - **Copy PID** / **Copy exe path** — unelevated, handy for scripts.
- **Click column headers** to sort (ascending/descending toggles).
- **Options menu** (top-right): opacity slider, always-on-top, show
  title bar, show processes, pause, click-through, and **minimize to
  tray on close**. Each toggle has a **[Set]** button for binding a
  global hotkey.
- **Title-bar buttons**:
  - **`_`** — hide to system tray (window disappears; restore from tray).
  - **`X`** — quit by default. If **minimize to tray on close** is
    enabled in Options, X hides to tray instead.
- **System tray icon** — right-click for **Show / hide**, **Toggle
  click-through**, **Quit**.

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
- **Startup may show an ETW diagnostic banner** (`ETW: started=true seen=0 …`) —
  this is intentional for alpha so you can tell the data pipeline is alive.
  Will be hidden behind a debug flag in beta.
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
