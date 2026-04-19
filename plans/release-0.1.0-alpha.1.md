# Release notes: netwatch 0.1.0-alpha.1

> Paste the **Body** section below into the GitHub Releases form. Use the
> **Title** as the release title. Mark the release as **pre-release**.

---

## Title

`v0.1.0-alpha.1 — first alpha`

## Body

First alpha of **netwatch**, a tiny always-on-top Windows network activity
watcher with per-process bandwidth, firewall/service actions, and a floating
transparent chart.

Single self-contained x64 `netwatch.exe` (~4.6 MB), no installer, no
external runtime dependencies (statically linked CRT).

### What's in it

- Live total-throughput chart and per-process table (PID, name, up/s,
  down/s, totals). Click headers to sort.
- Right-click a process → **Block in firewall**, **Disable service**
  (when applicable), **Kill process**, copy PID/path.
- `[svc]` and `[blocked]` row badges so noisy processes are easy to
  spot at a glance.
- Transparent draggable window with opacity slider.
- Options menu: always on top, show title bar, show processes, pause,
  click-through, minimize-to-tray-on-close. Each toggle has a global
  hotkey slot.
- System tray icon (Show/Hide, Toggle click-through, Quit). Tray
  events run on a dedicated thread so they keep working even with the
  window hidden.
- Click-through safety hotkey hardcoded to **`Ctrl+Alt+Shift+T`** so
  you can always escape a stuck click-through state.
- Chromed window is resizable; chromeless mode is a fixed-size
  floating utility. Chart grows with the window when the process
  table is hidden.
- Settings persisted to `%APPDATA%\netwatch\settings.json`.
- ETW session leak auto-cleanup at startup (enumerates active
  sessions, stops any leftover `netwatch-etw-*`).

### Setup (one-time)

ETW for per-process bandwidth needs Performance Log Users group
membership to avoid a UAC prompt every launch. In an **elevated**
PowerShell:

```powershell
Add-LocalGroupMember -Group "Performance Log Users" -Member $env:USERNAME
```

Sign out and back in for the membership to take effect.

### Verify the download

```powershell
certutil -hashfile netwatch.exe SHA256
```

Expected SHA-256: `65c696b36c1716d0b54e0bccab94ee77468e194250b6704bb5fe37ea4e08b425`

### Known issues

- No hotkey conflict detection — binding failures show only an in-app
  banner. Workaround: pick a different combo.
- Tray icon is a programmatic cyan ring; final art TBD.
- Not code-signed; SmartScreen will warn on first launch ("More info →
  Run anyway").
- ETW diagnostic banner is always visible in alpha for triage. Will be
  hidden behind a debug flag in beta.
- Network actions (Block / Disable service / Kill process) require UAC.
- Tested only on Windows 11 x64. Earlier Windows builds may work.

### Reporting bugs

<https://github.com/abozart/netwatch/issues>

Please include: netwatch version (Properties → Details), Windows build
(`winver`), whether you're in the Performance Log Users group, repro
steps, screenshot.

---

## Pre-release checklist (do these before the GitHub release)

- [ ] No running `netwatch.exe` (so build can write the file)
- [ ] `cargo test` passes (4 tests)
- [ ] `cargo clippy --release -- -D warnings` clean
- [ ] `cargo build --release` succeeds
- [ ] Note SHA-256: `certutil -hashfile target\release\netwatch.exe SHA256`
- [ ] Smoke test the exe: launch, see numbers populate, right-click
      action works, settings persist after restart
- [ ] CHANGELOG date is today (currently 2026-04-18)
- [ ] Commit + tag: `git add -A && git commit -m "..." && git tag v0.1.0-alpha.1`
- [ ] `git push -u origin main --tags`
- [ ] GitHub → Releases → Draft a new release from tag `v0.1.0-alpha.1`
- [ ] Title and Body from above
- [ ] Replace `<PASTE_SHA256_HERE>` with the actual hash
- [ ] Upload `target\release\netwatch.exe` as a release asset
- [ ] Mark **This is a pre-release**
- [ ] Publish
