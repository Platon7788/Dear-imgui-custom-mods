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

/// Bold standalone × glyph. No surrounding circle — the project
/// owner specifically asked for a clean cross on 2026-04-29 after
/// trying the spoked-progress + text-label variants. Heavier
/// stroke (`1.8`) than the historic close cross to balance the
/// removal of the outer ring.
pub(super) fn draw_close(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let cx = snap(cx);
    let cy = snap(cy);
    // Arm length sized off the unit-r canvas. `0.65×r` reaches well
    // past where the old circle sat (`0.85×r` outline + `0.40×r`
    // arms inside) so the standalone × reads at the same visual
    // weight as the previous circle-with-X.
    let s = r * 0.65;
    d.add_line([cx - s, cy - s], [cx + s, cy + s], col)
        .thickness(1.8)
        .build();
    d.add_line([cx + s, cy - s], [cx - s, cy + s], col)
        .thickness(1.8)
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

/// "Doc-window" restore glyph — single window outline with a thin
/// filled bar at the top edge (the title bar). Replaces the historic
/// cascade pattern (two overlapping squares) on 2026-04-30 — the
/// project owner asked for the document-with-titlebar look so the
/// restore state reads as "windowed mode" rather than the generic
/// "switch task" cascade icon. Same outer footprint as
/// [`draw_maximize`] so the two glyphs share visual weight.
pub(super) fn draw_restore(d: &DrawListMut<'_>, cx: f32, cy: f32, r: f32, col: u32) {
    let cx = snap(cx);
    let cy = snap(cy);
    let p = r * 0.72;
    d.add_rect([cx - p, cy - p], [cx + p, cy + p], col)
        .thickness(1.5)
        .build();
    let bar_h = r * 0.30;
    d.add_rect([cx - p, cy - p], [cx + p, cy - p + bar_h], col)
        .filled(true)
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
