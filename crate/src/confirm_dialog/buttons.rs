//! In-button glyph primitives and the custom icon button.
//!
//! [`ButtonGlyph`] enumerates the small glyphs drawn inside the cancel /
//! confirm cells (✕ / power / check), each rendered as draw-list geometry.
//! [`icon_button`] is the crate-private invisible-button + draw-list combo
//! used for both dialog buttons so they share one hover/active/text layout.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ButtonGlyph {
    None,
    X,
    Power,
    Check,
}

pub(super) fn draw_glyph_x(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    draw.add_line([cx - r, cy - r], [cx + r, cy + r], col)
        .thickness(1.8)
        .build();
    draw.add_line([cx + r, cy - r], [cx - r, cy + r], col)
        .thickness(1.8)
        .build();
}

pub(super) fn draw_glyph_power(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    // Open arc (gap at top) approximated with short line segments + vertical bar.
    //
    // Geometry: start angle is -π/2 (top of the circle) shifted clockwise by
    // `GAP_RAD` radians; end angle mirrors that on the other side. The
    // resulting open mouth at the top has angular width `2 * GAP_RAD` —
    // ~63° at 0.55 rad, which leaves room for the vertical bar without
    // touching the arc's endpoints.
    const GAP_RAD: f32 = 0.55;
    let segs = 18;
    let start = -std::f32::consts::FRAC_PI_2 + GAP_RAD;
    let end = 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2 - GAP_RAD;
    let step = (end - start) / segs as f32;
    let mut prev = [cx + r * start.cos(), cy + r * start.sin()];
    for i in 1..=segs {
        let a = start + step * i as f32;
        let p = [cx + r * a.cos(), cy + r * a.sin()];
        draw.add_line(prev, p, col).thickness(1.8).build();
        prev = p;
    }
    // Vertical bar at top — fits inside the gap left by GAP_RAD.
    draw.add_line([cx, cy - r - 1.0], [cx, cy - r * 0.15], col)
        .thickness(1.8)
        .build();
}

pub(super) fn draw_glyph_check(draw: &DrawListMut, cx: f32, cy: f32, r: f32, col: u32) {
    let p1 = [cx - r, cy + r * 0.05];
    let p2 = [cx - r * 0.30, cy + r * 0.55];
    let p3 = [cx + r, cy - r * 0.55];
    draw.add_line(p1, p2, col).thickness(2.0).build();
    draw.add_line(p2, p3, col).thickness(2.0).build();
}

// ── Custom icon button ───────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn icon_button(
    ui: &Ui,
    id: &str,
    label: &str,
    size: [f32; 2],
    glyph: ButtonGlyph,
    bg: [f32; 4],
    bg_hov: [f32; 4],
    bg_act: [f32; 4],
    text_col: [f32; 4],
    rounding: f32,
    icon_scale: f32,
) -> bool {
    let pos = ui.cursor_screen_pos();
    let pressed = ui.invisible_button(id, size);
    let hovered = ui.is_item_hovered();
    let active = ui.is_item_active();
    let cur_bg = if active {
        bg_act
    } else if hovered {
        bg_hov
    } else {
        bg
    };

    let draw = ui.get_window_draw_list();
    draw.add_rect(pos, [pos[0] + size[0], pos[1] + size[1]], c32(cur_bg))
        .filled(true)
        .rounding(rounding)
        .build();

    let text_size = calc_text_size(label);
    let text_col_u32 = c32(text_col);

    if matches!(glyph, ButtonGlyph::None) {
        let tx = pos[0] + (size[0] - text_size[0]) * 0.5;
        let ty = pos[1] + (size[1] - text_size[1]) * 0.5;
        draw.add_text([tx, ty], text_col_u32, label);
    } else {
        let icon_r = size[1] * icon_scale;
        let gap = 8.0;
        let group_w = icon_r * 2.0 + gap + text_size[0];
        let group_x = pos[0] + (size[0] - group_w) * 0.5;
        let icon_cx = group_x + icon_r;
        let icon_cy = pos[1] + size[1] * 0.5;
        match glyph {
            ButtonGlyph::X => draw_glyph_x(&draw, icon_cx, icon_cy, icon_r, text_col_u32),
            ButtonGlyph::Power => draw_glyph_power(&draw, icon_cx, icon_cy, icon_r, text_col_u32),
            ButtonGlyph::Check => draw_glyph_check(&draw, icon_cx, icon_cy, icon_r, text_col_u32),
            ButtonGlyph::None => {}
        }
        let tx = icon_cx + icon_r + gap;
        let ty = pos[1] + (size[1] - text_size[1]) * 0.5;
        draw.add_text([tx, ty], text_col_u32, label);
    }

    pressed
}
