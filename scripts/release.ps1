# Release pipeline for netwatch.
# Run from elevated PowerShell at the project root:
#   .\scripts\release.ps1
#
# Refuses to run if there are uncommitted changes or netwatch.exe is alive.
# Does NOT push or publish to GitHub — prints the next manual steps.

[CmdletBinding()]
param(
    [switch]$SkipTests,
    [switch]$AllowDirty
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

function Section($title) {
    Write-Host ""
    Write-Host ("==> $title") -ForegroundColor Cyan
}

# 1. Pre-flight: no running instance.
Section "Checking for running netwatch.exe"
$running = Get-Process netwatch -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "netwatch is running (PID $($running.Id)). Stop it before releasing." -ForegroundColor Red
    exit 1
}
Write-Host "OK"

# 2. Pre-flight: clean working tree (unless overridden).
if (-not $AllowDirty) {
    Section "Checking git status is clean"
    $dirty = git status --porcelain
    if ($dirty) {
        Write-Host "Working tree is dirty:" -ForegroundColor Red
        Write-Host $dirty
        Write-Host "Commit or stash before releasing, or pass -AllowDirty." -ForegroundColor Yellow
        exit 1
    }
    Write-Host "OK"
}

# 3. Tests.
if (-not $SkipTests) {
    Section "cargo test"
    cargo test
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    Section "cargo clippy --release -- -D warnings"
    cargo clippy --release -- -D warnings
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# 4. Release build.
Section "cargo build --release"
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 5. Resolve version from Cargo.toml.
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$tag = "v$version"
$exePath = Join-Path $ProjectRoot "target\release\netwatch.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "Expected build output not found: $exePath" -ForegroundColor Red
    exit 1
}

# 6. SHA-256.
Section "Computing SHA-256"
$hash = (Get-FileHash -Algorithm SHA256 $exePath).Hash.ToLower()
$size = (Get-Item $exePath).Length
$sizeMb = [math]::Round($size / 1MB, 2)
Write-Host "  exe:    $exePath"
Write-Host "  size:   $sizeMb MB ($size bytes)"
Write-Host "  sha256: $hash"

# 7. Verify version resource on the exe matches Cargo version.
Section "Verifying embedded version"
$exeVersion = (Get-Item $exePath).VersionInfo.ProductVersion
if ($exeVersion -ne $version) {
    Write-Host "Mismatch: Cargo says '$version' but exe says '$exeVersion'." -ForegroundColor Red
    Write-Host "Did build.rs run? Try a clean build: cargo clean; .\scripts\release.ps1" -ForegroundColor Yellow
    exit 1
}
Write-Host "OK ($exeVersion)"

# 8. Tag (idempotent — won't replace an existing tag).
Section "Git tag $tag"
$existing = git tag --list $tag
if ($existing) {
    Write-Host "Tag $tag already exists. Skipping." -ForegroundColor Yellow
} else {
    git tag -a $tag -m "netwatch $version"
    Write-Host "Created tag $tag"
}

# 9. Print next steps. We deliberately do NOT push or `gh release create`
#    so you can verify everything one more time before going public.
Section "Next manual steps"
@"
1. Push the tag and main:
       git push origin main --tags

2. Create the GitHub release:
       gh release create $tag --prerelease ``
           --title "$tag - first alpha" ``
           --notes-file plans\release-$version.md ``
           "$exePath"

   (Or use the web UI: GitHub → Releases → Draft new from tag $tag.
    Paste plans\release-$version.md as the body, replacing
    <PASTE_SHA256_HERE> with $hash, then upload the exe.)

3. Smoke test the published download on a clean machine.
"@ | Write-Host

Write-Host ""
Write-Host "Release pipeline complete." -ForegroundColor Green
