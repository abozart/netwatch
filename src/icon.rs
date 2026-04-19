//! Shared procedurally-generated cyan ring icon. Used by the tray icon, the
//! window/taskbar icon, and (at build time) the exe's embedded .ico.
//!
//! Regenerating the bits in code rather than shipping a .png keeps the binary
//! self-contained and makes it trivial to render at any target size.

/// Render the netwatch icon at `size`×`size` pixels as RGBA8.
///
/// Transparent background. Composition:
/// - thick outer cyan ring (the "watch" boundary)
/// - bright center dot (the "node" / activity pulse)
///
/// Anti-aliased via a 1-px distance falloff at every shape edge.
pub fn ring_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let cx = s / 2.0;
    let r_outer = (s / 2.0) - (s * 0.06).max(1.0);
    let ring_width = (s * 0.18).max(2.0);
    let r_inner = r_outer - ring_width;
    let r_dot = (s * 0.16).max(1.5);

    let cyan = [120u8, 215, 255]; // ring color
    let dot = [220u8, 250, 255]; // center color, slightly warmer

    fn aa(dist: f32, edge: f32) -> f32 {
        // Returns 1.0 inside, 0.0 outside, linearly faded in the 1-px AA band.
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

            // Ring: alpha is "inside outer" minus "inside inner".
            let ring_alpha = (aa(dist, r_outer) - aa(dist, r_inner)).clamp(0.0, 1.0);
            // Center dot.
            let dot_alpha = aa(dist, r_dot);

            // Composite: dot on top of ring.
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
