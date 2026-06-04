//! Header-icon drawing primitives.
//!
//! Each of the four [`DialogIcon`](super::DialogIcon) variants (Warning,
//! Error, Info, Question) is rendered as crisp draw-list geometry so the
//! dialog never depends on an icon font being loaded into the atlas.
//! These are crate-private helpers consumed by `render_confirm_dialog`.

use super::*;

pub(super) fn draw_icon_warning(
    draw: &DrawListMut,
    cx: f32,
    cy: f32,
    r: f32,
    col: u32,
    bg_col: u32,
) {
    // Equilateral triangle pointing up, centred at (cx, cy).
    // Heights: top = cy - r, base = cy + r*0.6  (visually centred).
    let h = r * 1.6;
    let half_base = h * 0.577; // tan(30°) ≈ 0.577
    let top_y = cy - r;
    let base_y = top_y + h;

    let p_top = [cx, top_y];
    let p_bl = [cx - half_base, base_y];
    let p_br = [cx + half_base, base_y];

    // Filled triangle background
    draw.add_triangle(p_top, p_bl, p_br, col)
        .filled(true)
        .build();
    // "!" drawn in bg color on top of the filled triangle
    let bang_top = cy - r * 0.22;
    let bang_bot = cy + r * 0.20;
    let dot_y = cy + r * 0.42;
    draw.add_line([cx, bang_top], [cx, bang_bot], bg_col)
        .thickness(2.2)
        .build();
    draw.add_circle([cx, dot_y], 1.6, bg_col)
        .filled(true)
        .build();
}

pub(super) fn draw_icon_error(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    let d = r * 0.42;
    draw.add_line([cx - d, cy - d], [cx + d, cy + d], col)
        .thickness(2.0)
        .build();
    draw.add_line([cx + d, cy - d], [cx - d, cy + d], col)
        .thickness(2.0)
        .build();
}

pub(super) fn draw_icon_info(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    draw.add_circle([cx, cy - r * 0.35], 1.8, col)
        .filled(true)
        .build();
    draw.add_line([cx, cy - r * 0.10], [cx, cy + r * 0.45], col)
        .thickness(2.0)
        .build();
}

pub(super) fn draw_icon_question(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_circle([cx, cy], r, col).thickness(2.0).build();
    let qx = cx;
    draw.add_line([qx - r * 0.20, cy - r * 0.35], [qx, cy - r * 0.50], col)
        .thickness(2.0)
        .build();
    draw.add_line([qx, cy - r * 0.50], [qx + r * 0.20, cy - r * 0.35], col)
        .thickness(2.0)
        .build();
    draw.add_line([qx + r * 0.20, cy - r * 0.35], [qx, cy - r * 0.10], col)
        .thickness(2.0)
        .build();
    draw.add_line([qx, cy - r * 0.10], [qx, cy + r * 0.05], col)
        .thickness(2.0)
        .build();
    draw.add_circle([qx, cy + r * 0.30], 1.8, col)
        .filled(true)
        .build();
}
