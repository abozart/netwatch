# GitHub setup for first push + release

You don't have a remote yet. Two paths — pick one.

## Path A: web UI (no `gh` CLI install needed)

1. Open <https://github.com/new>
2. **Owner**: `abozart` (must match the `repository` field in `Cargo.toml`)
3. **Repository name**: `netwatch`
4. **Description**: copy the `description` line from `Cargo.toml`
5. **Public** (or Private — pre-release works either way)
6. **DON'T** initialize with README/license/.gitignore — we already have them
7. Click **Create repository**
8. Back in PowerShell at the project root, hook the remote up:
   ```powershell
   git remote add origin https://github.com/abozart/netwatch.git
   git branch -M main
   git push -u origin main --tags
   ```
9. Browse to <https://github.com/abozart/netwatch/releases/new?tag=v0.1.0-alpha.1>
10. Title: `v0.1.0-alpha.1 - first alpha`
11. Open `plans\release-0.1.0-alpha.1.md`, copy everything under the `## Body`
    section, paste into the release description
12. Replace `<PASTE_SHA256_HERE>` with the SHA-256 the release script printed
    (or recompute: `certutil -hashfile target\release\netwatch.exe SHA256`)
13. Drag `target\release\netwatch.exe` onto the **Attach binaries** zone
14. Tick **This is a pre-release**
15. Click **Publish release**

## Path B: `gh` CLI (faster for repeat releases)

1. Install GitHub CLI: <https://cli.github.com/> (or `winget install --id GitHub.cli`)
2. Sign in: `gh auth login`
3. Create the repo and push in one shot:
   ```powershell
   gh repo create abozart/netwatch --public --source=. --remote=origin --push
   git push origin --tags
   ```
4. Create the release:
   ```powershell
   gh release create v0.1.0-alpha.1 --prerelease `
       --title "v0.1.0-alpha.1 - first alpha" `
       --notes-file plans\release-0.1.0-alpha.1.md `
       "target\release\netwatch.exe"
   ```
5. Edit the release in the web UI to swap `<PASTE_SHA256_HERE>` for the
   actual hash (the gh CLI doesn't template the body for us).

## After publishing

- Smoke test on a clean machine: download from the release page, accept UAC,
  confirm the chart populates and tray icon appears.
- Watch <https://github.com/abozart/netwatch/issues> for early feedback.
