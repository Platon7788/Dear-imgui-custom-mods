//! Severity icons drawn via `DrawListMut` — font-independent.
//!
//! Each drawer takes the glyph center `(cx, cy)`, radius `r`, foreground
//! color (packed `u32`), and an optional background fill color. The body
//! is rendered centered in a circle/triangle of radius `r`.

use dear_imgui_rs::DrawListMut;

use super::config::Severity;

/// Draw a severity-specific icon at `(cx, cy)` with radius `r`.
///
/// `fill` is the accent color (packed `u32`) — used for the circle/triangle
/// stroke + glyph. `bg` is the surrounding toast background, used as the
/// "cut-out" color for filled-icon glyphs (Warning `!`, etc.).
pub(crate) fn draw_severity(
    draw: &DrawListMut,
    sev: Severity,
    cx: f32,
    cy: f32,
    r: f32,
    fill: u32,
    bg: u32,
) {
    match sev {
        Severity::Info => draw_info(draw, cx, cy, r, fill),
        Severity::Success => draw_success(draw, cx, cy, r, fill),
        Severity::Warning => draw_warning(draw, cx, cy, r, fill, bg),
        Severity::Error => draw_error(draw, cx, cy, r, fill),
        Severity::Debug => draw_debug(draw, cx, cy, r, fill),
    }
}

/// Info — filled circle with a lowercase "i" cut out in `bg` color.
fn draw_info(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).filled(true).build();
    // 'i' drawn as two overlapping small elements in white.
    let white = 0xFFFF_FFFF;
    draw.add_circle([cx, cy - r * 0.38], 1.6, white)
        .filled(true)
        .build();
    draw.add_line([cx, cy - r * 0.10], [cx, cy + r * 0.48], white)
        .thickness(2.0)
        .build();
}

/// Success — filled circle with a white checkmark.
fn draw_success(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).filled(true).build();
    let white = 0xFFFF_FFFF;
    // Two-line check: (-0.45, 0.05) → (-0.10, 0.40) → (0.55, -0.30) of r
    let p1 = [cx - r * 0.45, cy + r * 0.05];
    let p2 = [cx - r * 0.10, cy + r * 0.40];
    let p3 = [cx + r * 0.55, cy - r * 0.30];
    draw.add_line(p1, p2, white).thickness(2.4).build();
    draw.add_line(p2, p3, white).thickness(2.4).build();
}

/// Warning — filled triangle with a "!" cut out in `bg` color.
fn draw_warning(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32, bg: u32) {
    let h = r * 1.7;
    let half_base = h * 0.577;
    let top_y = cy - r * 0.85;
    let base_y = top_y + h;

    let p_top = [cx, top_y];
    let p_bl = [cx - half_base, base_y];
    let p_br = [cx + half_base, base_y];
    draw.add_triangle(p_top, p_bl, p_br, col)
        .filled(true)
        .build();

    // "!" in background color on top of the filled triangle.
    let bang_top = cy - r * 0.15;
    let bang_bot = cy + r * 0.28;
    let dot_y = cy + r * 0.52;
    draw.add_line([cx, bang_top], [cx, bang_bot], bg)
        .thickness(2.2)
        .build();
    draw.add_circle([cx, dot_y], 1.6, bg).filled(true).build();
}

/// Error — filled circle with a white "×".
fn draw_error(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).filled(true).build();
    let white = 0xFFFF_FFFF;
    let d = r * 0.42;
    draw.add_line([cx - d, cy - d], [cx + d, cy + d], white)
        .thickness(2.2)
        .build();
    draw.add_line([cx + d, cy - d], [cx - d, cy + d], white)
        .thickness(2.2)
        .build();
}

/// Debug — outlined circle with three horizontal dots (ellipsis).
fn draw_debug(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    let dy = cy;
    let dx = r * 0.42;
    draw.add_circle([cx - dx, dy], 1.6, col)
        .filled(true)
        .build();
    draw.add_circle([cx, dy], 1.6, col).filled(true).build();
    draw.add_circle([cx + dx, dy], 1.6, col)
        .filled(true)
        .build();
}

/// Draw a "×" close glyph centered at `(cx, cy)` with arm length `r`.
pub(crate) fn draw_close_x(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_line([cx - r, cy - r], [cx + r, cy + r], col)
        .thickness(1.6)
        .build();
    draw.add_line([cx + r, cy - r], [cx - r, cy + r], col)
        .thickness(1.6)
        .build();
}
