//! Shared procedurally-generated cyan ring icon. Used by the tray icon, the
//! window/taskbar icon, and (at build time) the exe's embedded .ico.
//!
//! Regenerating the bits in code rather than shipping a .png keeps the binary
//! self-contained and makes it trivial to render at any target size.

/// Render the ring at `size`×`size` pixels as RGBA8 (4 bytes per pixel).
/// Transparent background, cyan ring with 1-px inner anti-aliasing via a
/// simple distance falloff.
pub fn ring_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let cx = s / 2.0;
    let r_outer = (s / 2.0) - (s * 0.06).max(1.0);
    let ring_width = (s * 0.16).max(2.0);
    let r_inner = r_outer - ring_width;

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cx;
            let dist = (dx * dx + dy * dy).sqrt();

            // Soft edges: 1-pixel AA band on inner and outer radii.
            let alpha = if dist > r_outer + 1.0 || dist < r_inner - 1.0 {
                0.0
            } else if dist > r_outer {
                1.0 - (dist - r_outer)
            } else if dist < r_inner {
                1.0 - (r_inner - dist)
            } else {
                1.0
            };

            if alpha > 0.0 {
                rgba[i] = 140;
                rgba[i + 1] = 220;
                rgba[i + 2] = 255;
                rgba[i + 3] = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    rgba
}
