//! Comment / group box rendering and geometry.
//!
//! A comment box is a colored, semi-transparent rectangle drawn *behind* the
//! nodes, with a title-bar caption. It is purely an annotation: it is not a
//! node, not part of the graph slab, and not connectable.
//!
//! This module owns the screen-space geometry helpers ([`screen_rect`],
//! [`title_bar_height`], [`resize_handle_rect`]) so that rendering (here) and
//! hit testing (in `input.rs`) always agree on where the title bar and resize
//! handle live.

use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::InteractionState;

// ─── Layout constants ────────────────────────────────────────────────────────

/// Title-bar height in graph space (scaled by zoom at render time).
pub(super) const TITLE_BAR_H: f32 = 22.0;
/// Resize-handle side length in graph space (scaled by zoom at render time).
pub(super) const RESIZE_HANDLE: f32 = 14.0;
/// Minimum comment width in graph space.
pub(crate) const MIN_W: f32 = 60.0;
/// Minimum comment height in graph space.
pub(crate) const MIN_H: f32 = 40.0;

/// Fill alpha for the comment body (low — drawn behind nodes).
const FILL_ALPHA: u8 = 40;
/// Title-bar alpha (slightly higher than the body).
const TITLE_ALPHA: u8 = 90;
/// Border alpha.
const BORDER_ALPHA: u8 = 200;
/// Title text alpha.
const TEXT_ALPHA: u8 = 230;

// ─── Geometry helpers (shared with input hit testing) ────────────────────────

/// Screen-space rectangle `[x0, y0, x1, y1]` for a comment.
#[inline]
pub(super) fn screen_rect(state: &InteractionState, pos: [f32; 2], size: [f32; 2]) -> [f32; 4] {
    let vp = &state.viewport;
    let [sx, sy] = vp.graph_to_screen(pos);
    let sw = size[0] * vp.zoom;
    let sh = size[1] * vp.zoom;
    [sx, sy, sx + sw, sy + sh]
}

/// Title-bar height in screen pixels (scaled by zoom).
#[inline]
pub(super) fn title_bar_height(state: &InteractionState) -> f32 {
    TITLE_BAR_H * state.viewport.zoom
}

/// Screen-space resize-handle rectangle `[x0, y0, x1, y1]` at the
/// bottom-right corner of the comment.
#[inline]
pub(super) fn resize_handle_rect(rect: [f32; 4], zoom: f32) -> [f32; 4] {
    let h = RESIZE_HANDLE * zoom;
    [rect[2] - h, rect[3] - h, rect[2], rect[3]]
}

// ─── Rendering ───────────────────────────────────────────────────────────────

/// Render all comment boxes behind the nodes.
///
/// Comments fully outside the canvas clip rect are culled. Drawing happens via
/// the window draw list; the caller is responsible for clipping to the canvas.
pub(super) fn render_comments<T>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
    font: &dear_imgui_rs::fonts::Font,
    base_font_size: f32,
) {
    let zoom = state.viewport.zoom;
    let scaled_fs = base_font_size * zoom;
    let canvas_max = [
        canvas_pos[0] + canvas_size[0],
        canvas_pos[1] + canvas_size[1],
    ];

    for comment in graph.comments() {
        let rect = screen_rect(state, comment.pos, comment.size);

        // LOD / culling: skip comments fully outside the canvas.
        if rect[2] < canvas_pos[0]
            || rect[0] > canvas_max[0]
            || rect[3] < canvas_pos[1]
            || rect[1] > canvas_max[1]
        {
            continue;
        }

        let color = comment.color;
        let rounding = config.node_rounding * zoom;
        let title_h = title_bar_height(state);
        let title_bottom = (rect[1] + title_h).min(rect[3]);

        // ── Body fill (low alpha) ──────────────────────────────────────────
        draw.add_rect(
            [rect[0], rect[1]],
            [rect[2], rect[3]],
            c32(color, FILL_ALPHA),
        )
        .rounding(rounding)
        .filled(true)
        .build();

        // ── Title bar strip (slightly higher alpha) ────────────────────────
        draw.add_rect(
            [rect[0], rect[1]],
            [rect[2], title_bottom],
            c32(color, TITLE_ALPHA),
        )
        .rounding(rounding)
        .filled(true)
        .build();
        // Square off the bottom corners of the title strip so it reads as a bar.
        if rounding > 0.0 {
            draw.add_rect(
                [rect[0], (title_bottom - rounding).max(rect[1])],
                [rect[2], title_bottom],
                c32(color, TITLE_ALPHA),
            )
            .filled(true)
            .build();
        }

        // ── Title caption ──────────────────────────────────────────────────
        if !comment.text.is_empty() && zoom >= config.lod_hide_labels_zoom {
            let raw_sz = calc_text_size(&comment.text);
            let tx = rect[0] + 6.0 * zoom;
            let ty = rect[1] + (title_h - raw_sz[1] * zoom) * 0.5;
            // Clip the caption to the title bar so long captions can't overflow.
            let clip = [rect[0], rect[1], rect[2] - 2.0 * zoom, title_bottom];
            draw.add_text_with_font(
                font,
                scaled_fs,
                [tx, ty],
                c32(config.colors.text, TEXT_ALPHA),
                &comment.text,
                0.0,
                Some(clip),
            );
        }

        // ── Border ─────────────────────────────────────────────────────────
        draw.add_rect(
            [rect[0], rect[1]],
            [rect[2], rect[3]],
            c32(color, BORDER_ALPHA),
        )
        .rounding(rounding)
        .filled(false)
        .thickness(1.0)
        .build();

        // ── Resize handle (bottom-right corner) ────────────────────────────
        let h = resize_handle_rect(rect, zoom);
        draw.add_triangle(
            [h[2], h[1]],
            [h[2], h[3]],
            [h[0], h[3]],
            c32(color, BORDER_ALPHA),
        )
        .filled(true)
        .build();
    }
}
