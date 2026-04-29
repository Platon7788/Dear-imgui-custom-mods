//! Vector glyphs for the standard titlebar buttons.
//!
//! All icons are drawn into a `[-r, +r]` unit space centred at `(cx, cy)`
//! so they render crisp at any DPI without depending on a glyph font.
//!
//! Centres are pixel-snapped (`.floor() + 0.5`) so 1.5-px-thick strokes
//! land symmetrically on the same row of pixels — without snapping a
//! `cx = 14.5` that comes from an odd-height titlebar would split the
//! line across two rows and look fuzzy / asymmetric.

use dear_imgui_rs::DrawListMut;

/// Round (cx, cy) onto the nearest pixel-grid line so 1.5-px strokes
/// hit the same pixel row from both sides. We round *down + 0.5* (not
/// just `.round()`) because draw-list strokes are centred — a stroke
/// at `y = 0.5` paints rows `[0, 1]`, while at `y = 0.0` it paints
/// rows `[-1, 0]` which clips above the button. Always paint at the
/// half-pixel for a crisp, symmetric line at any DPI.
#[inline]
fn snap(p: f32) -> f32 {
    p.floor() + 0.5
}

pub(super) fn draw_close(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    // Circle-with-X — matches Vex0r's `mdi-close-circle-outline` glyph
    // but drawn as draw-list primitives so it is font-independent.
    let cx = snap(cx);
    let cy = snap(cy);
    d.add_circle([cx, cy], r * 0.85, col)
        .thickness(1.2)
        // 24 segments — visibly polygonal at the previous default of
        // 20 on small radii (~6 px); smooth at 24 with negligible cost.
        .num_segments(24)
        .build();
    let s = r * 0.40;
    d.add_line([cx - s, cy - s], [cx + s, cy + s], col)
        .thickness(1.5)
        .build();
    d.add_line([cx + s, cy - s], [cx - s, cy + s], col)
        .thickness(1.5)
        .build();
}

pub(super) fn draw_maximize(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let cx = snap(cx);
    let cy = snap(cy);
    let p = r * 0.72;
    d.add_rect([cx - p, cy - p], [cx + p, cy + p], col)
        .thickness(1.5)
        .build();
}

pub(super) fn draw_restore(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32, bg: u32) {
    let cx = snap(cx);
    let cy = snap(cy);
    let p = r * 0.72;
    let sh = r * 0.38;
    d.add_rect([cx - p + sh, cy - p - sh], [cx + p + sh, cy + p - sh], col)
        .thickness(1.2)
        .build();
    d.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], bg)
        .filled(true)
        .build();
    d.add_rect([cx - p, cy - p + sh], [cx + p - sh, cy + p + sh], col)
        .thickness(1.5)
        .build();
}

pub(super) fn draw_minimize(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    // Centred horizontal stroke. Previously offset by `r * 0.40` to mimic
    // a "_" sit-on-baseline glyph — but on a 28-px titlebar that pushed
    // the line ~2 px below centre and broke vertical alignment with the
    // maximize square and close circle. Now sits exactly at `cy`.
    let cx = snap(cx);
    let cy = snap(cy);
    let p = r * 0.72;
    d.add_line([cx - p, cy], [cx + p, cy], col)
        .thickness(1.5)
        .build();
}
