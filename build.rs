fn main() {
    #[cfg(windows)]
    windows_build();

    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(windows)]
fn windows_build() {
    use embed_manifest::manifest::ExecutionLevel;
    use embed_manifest::{embed_manifest, new_manifest};
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;

    // 1. Embed the app manifest so we run as-invoker (no auto-UAC).
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // Release builds request elevation (ETW kernel-network needs it).
        // Debug builds (incl. `cargo test`) stay as-invoker so they can run
        // from cargo without needing UAC each time.
        let level = if std::env::var("DEBUG").as_deref() == Ok("true") {
            ExecutionLevel::AsInvoker
        } else {
            ExecutionLevel::RequireAdministrator
        };
        let manifest = new_manifest("netwatch").requested_execution_level(level);
        embed_manifest(manifest).expect("failed to embed manifest");
    }

    // 2. Generate a multi-size .ico for Explorer / Start menu.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("netwatch.ico");
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in &[16u32, 32, 48, 64, 128, 256] {
        let rgba = ring_rgba(size);
        let image = ico::IconImage::from_rgba_data(size, size, rgba);
        icon_dir.add_entry(
            ico::IconDirEntry::encode(&image).expect("encode ico entry"),
        );
    }
    let mut buf = Vec::new();
    icon_dir
        .write(&mut Cursor::new(&mut buf))
        .expect("write ico");
    fs::write(&ico_path, &buf).expect("write netwatch.ico");

    // 3. Write the .rc file (ICON + VERSIONINFO) and compile + embed it.
    let version = env!("CARGO_PKG_VERSION");
    let version_nums = version_to_nums(version);
    let rc_path = out_dir.join("netwatch.rc");
    // Escape backslashes for the .rc string literal (icon path uses them).
    let ico_for_rc = ico_path.to_string_lossy().replace('\\', "\\\\");
    let rc = format!(
        r#"#include <winver.h>

1 ICON "{ico}"

1 VERSIONINFO
FILEVERSION     {major},{minor},{patch},{build}
PRODUCTVERSION  {major},{minor},{patch},{build}
FILEFLAGSMASK   0x3fL
FILEFLAGS       0x0L
FILEOS          0x40004L
FILETYPE        0x1L
FILESUBTYPE     0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName",      "abozart\0"
            VALUE "FileDescription",  "netwatch - Windows network activity watcher\0"
            VALUE "FileVersion",      "{version}\0"
            VALUE "InternalName",     "netwatch\0"
            VALUE "LegalCopyright",   "Copyright (C) 2026 abozart. MIT License.\0"
            VALUE "OriginalFilename", "netwatch.exe\0"
            VALUE "ProductName",      "netwatch\0"
            VALUE "ProductVersion",   "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        ico = ico_for_rc,
        major = version_nums.0,
        minor = version_nums.1,
        patch = version_nums.2,
        build = version_nums.3,
        version = version,
    );
    fs::write(&rc_path, rc).expect("write netwatch.rc");

    match embed_resource::compile(&rc_path, embed_resource::NONE) {
        embed_resource::CompilationResult::Ok => {}
        embed_resource::CompilationResult::NotAttempted(msg) => {
            println!("cargo:warning=resource not embedded: {msg}");
        }
        embed_resource::CompilationResult::NotWindows => {
            println!("cargo:warning=resource not embedded: not Windows");
        }
        embed_resource::CompilationResult::Failed(msg) => {
            panic!("embed-resource failed: {msg}");
        }
    }
}

/// Convert "0.1.0-alpha.1" → (0, 1, 0, 1) — last slot is the prerelease number
/// if present, else 0. Used for FILEVERSION / PRODUCTVERSION numeric fields,
/// which must be 4-part integers.
#[cfg(windows)]
fn version_to_nums(v: &str) -> (u16, u16, u16, u16) {
    let (core, pre) = v.split_once('-').unwrap_or((v, ""));
    let mut parts = core.split('.').map(|p| p.parse::<u16>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    // Try to pull a trailing number out of the prerelease tag.
    let build = pre
        .rsplit('.')
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (major, minor, patch, build)
}

// Must mirror src/icon.rs::ring_rgba — build.rs can't depend on the main crate.
#[cfg(windows)]
fn ring_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let cx = s / 2.0;
    let r_outer = (s / 2.0) - (s * 0.06).max(1.0);
    let ring_width = (s * 0.18).max(2.0);
    let r_inner = r_outer - ring_width;
    let r_dot = (s * 0.16).max(1.5);

    let cyan = [120u8, 215, 255];
    let dot = [220u8, 250, 255];

    fn aa(dist: f32, edge: f32) -> f32 {
        let d = edge - dist;
        d.clamp(0.0, 1.0)
    }

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cx;
            let dist = (dx * dx + dy * dy).sqrt();

            let ring_alpha = (aa(dist, r_outer) - aa(dist, r_inner)).clamp(0.0, 1.0);
            let dot_alpha = aa(dist, r_dot);

            let (color, alpha) = if dot_alpha > 0.0 {
                let combined = (dot_alpha + ring_alpha * (1.0 - dot_alpha)).min(1.0);
                (dot, combined)
            } else if ring_alpha > 0.0 {
                (cyan, ring_alpha)
            } else {
                continue;
            };

            rgba[i] = color[0];
            rgba[i + 1] = color[1];
            rgba[i + 2] = color[2];
            rgba[i + 3] = (alpha * 255.0) as u8;
        }
    }
    rgba
}
