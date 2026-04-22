//! Render a dynamic tray icon showing current up/dn rates as colored text.
//! Rasterizes Segoe UI Bold (loaded once from %WINDIR%\Fonts) via ab_glyph,
//! so the tray text uses Windows' own UI font instead of hand-tuned bitmap
//! glyphs. Direction is encoded by color (blue = up, green = dn), so no
//! +/- prefix is needed and every pixel is available for the value.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::sync::OnceLock;

const ICON_SIZE: u32 = crate::defaults::TRAY_ICON_SIZE;

/// Pixel height requested from ab_glyph. Tuned so that two stacked lines of
/// digits fit inside a 16-pixel icon without the top line's cap or the
/// bottom line's descenders being clipped.
const PX_SIZE: f32 = 9.0;

/// Load the font once per process. Falls back to `None` if Segoe UI Bold
/// can't be read; callers render nothing in that case, letting the static
/// ring icon (set at tray startup) remain visible.
fn font() -> Option<&'static FontVec> {
    static FONT: OnceLock<Option<FontVec>> = OnceLock::new();
    FONT.get_or_init(|| {
        let windir = std::env::var_os("WINDIR")?;
        // Try Bold first for legibility at 9 px; fall back to Regular if Bold
        // is missing (e.g. stripped-down Windows images).
        for name in ["segoeuib.ttf", "segoeui.ttf"] {
            let path = std::path::Path::new(&windir).join("Fonts").join(name);
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(f) = FontVec::try_from_vec(bytes) {
                    return Some(f);
                }
            }
        }
        None
    })
    .as_ref()
}

/// Format the rate as a short string for the tray. Uses a 2-digit integer
/// when it fits ("99K"), otherwise promotes to the next unit with a leading
/// decimal (".3M") so values above 99 in any unit still display a meaningful
/// magnitude instead of being silently capped. Direction is encoded by
/// color, not sign.
fn compact(bps: f64) -> String {
    const K: f64 = 1024.0;
    const M: f64 = K * K;
    const G: f64 = M * K;

    fn two_digit(value: f64, unit: char) -> String {
        if value < 10.0 {
            format!("{}{unit}", value.round().clamp(1.0, 9.0) as u32)
        } else {
            format!("{}{unit}", value.round().clamp(10.0, 99.0) as u32)
        }
    }
    fn decimal(value_in_next: f64, next_unit: char) -> String {
        let tenths = (value_in_next * 10.0).round().clamp(1.0, 9.0) as u32;
        format!(".{tenths}{next_unit}")
    }

    if bps < K {
        let n = bps.round() as u32;
        if n < 100 {
            format!("{n}B")
        } else {
            decimal(bps / K, 'K')
        }
    } else if bps < 100.0 * K {
        two_digit(bps / K, 'K')
    } else if bps < M {
        decimal(bps / M, 'M')
    } else if bps < 100.0 * M {
        two_digit(bps / M, 'M')
    } else if bps < G {
        decimal(bps / G, 'G')
    } else {
        two_digit(bps / G, 'G')
    }
}

/// Rasterize `text` onto `buf` in `color`, centered horizontally, with the
/// font's baseline placed at `baseline_y`. Alpha per pixel is coverage × 255;
/// Windows alpha-blends the tray icon against the taskbar background.
fn render_line(buf: &mut [u8], font: &FontVec, text: &str, baseline_y: f32, color: [u8; 4]) {
    let scale = PxScale::from(PX_SIZE);
    let scaled = font.as_scaled(scale);

    let total_w: f32 = text
        .chars()
        .map(|ch| scaled.h_advance(font.glyph_id(ch)))
        .sum();
    let mut cursor_x = (ICON_SIZE as f32 - total_w) / 2.0;

    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
        let advance = scaled.h_advance(gid);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let x0 = bounds.min.x as i32;
            let y0 = bounds.min.y as i32;
            outlined.draw(|dx, dy, coverage| {
                let px = x0 + dx as i32;
                let py = y0 + dy as i32;
                if px < 0 || py < 0 || px >= ICON_SIZE as i32 || py >= ICON_SIZE as i32 {
                    return;
                }
                let off = ((py as u32 * ICON_SIZE + px as u32) * 4) as usize;
                let new_a = (coverage * 255.0) as u8;
                // Keep whichever alpha is higher so adjacent glyph edges from
                // the same line don't dim overlapping fringes; the two lines
                // are positioned not to overlap vertically.
                if new_a > buf[off + 3] {
                    buf[off] = color[0];
                    buf[off + 1] = color[1];
                    buf[off + 2] = color[2];
                    buf[off + 3] = new_a;
                }
            });
        }
        cursor_x += advance;
    }
}

/// Render a 16×16 RGBA tray icon: upload on top (blue), download on bottom
/// (green). Returns `None` if Segoe UI failed to load so callers can keep
/// the previously-set tray icon (the static ring) visible — a blank square
/// is worse than a slightly-stale glyph.
pub fn render(up_bps: f64, dn_bps: f64) -> Option<Vec<u8>> {
    let font = font()?;
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    let up_text = compact(up_bps);
    let dn_text = compact(dn_bps);

    // Match NetWatchApp::up_dn_labels / peak labels: up in blue, dn in green.
    let up_color = [140, 200, 255, 255];
    let dn_color = [140, 255, 170, 255];

    // Baselines tuned for 9-px Segoe UI Bold: top line's cap lands at y≈0,
    // bottom line's baseline at the icon's bottom edge, leaving ~1 px
    // breathing room between the two rows of digits.
    render_line(&mut buf, font, &up_text, 7.0, up_color);
    render_line(&mut buf, font, &dn_text, 15.0, dn_color);

    Some(buf)
}
