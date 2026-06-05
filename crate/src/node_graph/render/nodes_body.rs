//! Node body rendering (mutable pass) and pin-shape drawing.
//!
//! Split out of [`super::nodes`] to keep that module under the 500-line cap
//! (CLAUDE.md). The pin-shape helper lives here too because it is only invoked
//! from node rendering. Everything is `pub(super)` so the immutable node pass
//! and `render::mod` can reach it.

use dear_imgui_rs::Ui;

use crate::utils::color::rgb_arr as c32;

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::InteractionState;
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;

// ─── Node body rendering (mutable pass — needs &mut T) ──────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn render_node_body<T>(
    graph: &mut Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    ui: &Ui,
    node_id: NodeId,
    base_font_size: f32,
    orig_spacing: [f32; 2],
) {
    let Some(node) = graph.get_node(node_id) else {
        return;
    };
    if !node.open || !viewer.has_body(&node.value) {
        return;
    }

    let vp = &state.viewport;
    let zoom = vp.zoom;
    let num_inputs = viewer.inputs(&node.value);
    let num_outputs = viewer.outputs(&node.value);
    let node_width = viewer
        .node_width(&node.value)
        .unwrap_or(config.node_min_width);

    let [sx, sy] = vp.graph_to_screen(node.pos);
    let sw = node_width * zoom;
    let header_bottom = sy + config.node_header_height * zoom;
    let pin_start_y = header_bottom + config.node_padding_v * zoom;
    let pin_count = num_inputs.max(num_outputs) as f32;
    let body_y = pin_start_y + pin_count * config.pin_spacing * zoom;
    let body_x = sx + config.node_padding_h * zoom;

    // Compute body height and clip rect before any mutable re-borrow of graph.
    let body_h_override = viewer.body_height(&node.value);
    let node_h = config.node_height(num_inputs, num_outputs, true, true, body_h_override) * zoom;
    let node_bottom = sy + node_h;
    let clip_min = [sx + 1.0, body_y];
    let clip_max = [sx + sw - 1.0, node_bottom - 1.0];

    // Save CursorMaxPos so body widgets don't expand the parent window's
    // content boundaries.  Without this, nodes below the canvas bottom would
    // push CursorMaxPos beyond the window, causing ImGui to auto-scroll on
    // the next frame — which shifts canvas_pos and breaks all coordinates.
    let saved_cursor_max = unsafe {
        let window = dear_imgui_rs::sys::igGetCurrentWindow();
        debug_assert!(!window.is_null(), "igGetCurrentWindow returned null");
        (*window).DC.CursorMaxPos
    };

    // Scale font for body widgets via igPushFont FFI (ImGui 1.92+ dynamic fonts).
    // NOTE: Direct struct access ((*ctx).FontSize) has wrong field offsets in
    // the Rust bindings — igGetFontSize()/igPushFont() are the only safe API.
    let scaled_size = (base_font_size * zoom).round().clamp(1.0, 256.0);
    ui.push_font_with_size(None, scaled_size);

    let _spacing_token = ui.push_style_var(dear_imgui_rs::StyleVar::ItemSpacing([
        orig_spacing[0] * zoom,
        orig_spacing[1] * zoom,
    ]));

    ui.set_cursor_screen_pos([body_x, body_y]);

    // Clip body content to node bounds so widgets can't overflow visually.
    // Need &mut T — re-borrow as mutable (immutable `node` borrow ends above).
    ui.with_clip_rect(clip_min, clip_max, true, || {
        // group() is CRITICAL: it sets DC.Indent = DC.GroupOffset based on the
        // current cursor X (body_x).  Without this, after the first text line
        // ImGui resets cursor X to window->Pos.x + Indent.x (the window's left
        // edge), so lines 2+ render at a fixed screen position instead of inside
        // the node — causing body content to appear/disappear based on pan.
        ui.group(|| {
            if let Some(node_mut) = graph.get_node_mut(node_id) {
                viewer.render_body(ui, &mut node_mut.value, node_id);
            }
        });
    });

    // Restore font via igPopFont (matches push_font_with_size above).
    unsafe {
        dear_imgui_rs::sys::igPopFont();
    }

    // Restore CursorMaxPos to prevent content expansion / scroll.
    unsafe {
        let window = dear_imgui_rs::sys::igGetCurrentWindow();
        debug_assert!(!window.is_null(), "igGetCurrentWindow returned null");
        if !window.is_null() {
            (*window).DC.CursorMaxPos = saved_cursor_max;
        }
    }
}

// ─── Pin shape rendering ─────────────────────────────────────────────────────

pub(super) fn render_pin(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    config: &NodeGraphConfig,
    zoom: f32,
    center: [f32; 2],
    info: &PinInfo,
    hovered: bool,
) {
    let r = config.pin_radius * zoom;
    let fill = if hovered {
        config.colors.pin_hovered
    } else {
        info.fill
    };

    match info.shape {
        PinShape::Circle => {
            draw.add_circle(center, r, c32(fill, 255))
                .num_segments(12)
                .filled(true)
                .build();
            draw.add_circle(center, r, c32(info.stroke, 200))
                .num_segments(12)
                .build();
        }
        PinShape::Square => {
            let half = r * 0.75;
            draw.add_rect(
                [center[0] - half, center[1] - half],
                [center[0] + half, center[1] + half],
                c32(fill, 255),
            )
            .rounding(1.5)
            .filled(true)
            .build();
            draw.add_rect(
                [center[0] - half, center[1] - half],
                [center[0] + half, center[1] + half],
                c32(info.stroke, 200),
            )
            .rounding(1.5)
            .filled(false)
            .build();
        }
        PinShape::Triangle => {
            let h = r * 1.1;
            let p1 = [center[0] + h, center[1]];
            let p2 = [center[0] - h * 0.5, center[1] - h * 0.87];
            let p3 = [center[0] - h * 0.5, center[1] + h * 0.87];
            draw.add_triangle(p1, p2, p3, c32(fill, 255))
                .filled(true)
                .build();
            draw.add_triangle(p1, p2, p3, c32(info.stroke, 200))
                .filled(false)
                .build();
        }
        PinShape::Diamond => {
            let d = r * 0.9;
            let pts: [[f32; 2]; 4] = [
                [center[0], center[1] - d],
                [center[0] + d, center[1]],
                [center[0], center[1] + d],
                [center[0] - d, center[1]],
            ];
            // allocate once, clone for second draw call (bindings require owned Vec)
            let pts_vec = pts.to_vec();
            draw.add_polyline(pts_vec.clone(), c32(fill, 255))
                .filled(true)
                .build();
            draw.add_polyline(pts_vec, c32(info.stroke, 200)).build();
        }
    }
}
