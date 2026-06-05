//! Hit testing and interactive minimap navigation for the node graph.
//!
//! Split out of [`super::input`] to keep that module under the 500-line cap
//! (CLAUDE.md). These helpers are `pub(super)` so `handle_input` can reach
//! them.

use dear_imgui_rs::{MouseButton, Ui};

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::{HoveredElement, InteractionState};
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;
use super::comments;
use super::math;
use super::overlays;

// ─── Hit testing ─────────────────────────────────────────────────────────────

pub(super) fn hit_test<T>(
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    mouse: [f32; 2],
    wire_aabbs: &[math::NodeAABB],
) -> HoveredElement {
    let vp = &state.viewport;
    let pin_hit_r = config.pin_hit_radius * vp.zoom;
    let pin_hit_r_sq = pin_hit_r * pin_hit_r;

    // Check pins first (smallest targets, highest priority)
    for (&pin_id, &pos) in &state.input_pin_pos {
        let dx = mouse[0] - pos[0];
        let dy = mouse[1] - pos[1];
        if dx * dx + dy * dy <= pin_hit_r_sq {
            return HoveredElement::InputPin(pin_id);
        }
    }
    for (&pin_id, &pos) in &state.output_pin_pos {
        let dx = mouse[0] - pos[0];
        let dy = mouse[1] - pos[1];
        if dx * dx + dy * dy <= pin_hit_r_sq {
            return HoveredElement::OutputPin(pin_id);
        }
    }

    // Check nodes (in reverse draw order — top first)
    for &node_id in state.draw_order.iter().rev() {
        if let Some(node) = graph.get_node(node_id) {
            let [sx, sy] = vp.graph_to_screen(node.pos);
            let node_w = viewer
                .node_width(&node.value)
                .unwrap_or(config.node_min_width);
            let ni = viewer.inputs(&node.value);
            let no = viewer.outputs(&node.value);
            let hb = viewer.has_body(&node.value);
            let node_h = config.node_height(ni, no, hb, node.open, viewer.body_height(&node.value));

            let sw = node_w * vp.zoom;
            let sh = node_h * vp.zoom;

            if mouse[0] >= sx && mouse[0] < sx + sw && mouse[1] >= sy && mouse[1] < sy + sh {
                return HoveredElement::Node(node_id);
            }
        }
    }

    // Check wires (scaled hover distance, obstacle-aware hit testing)
    let wire_dist = config.wire_hover_distance * vp.zoom;
    for wire in graph.wires() {
        let Some(from_pos) = state.find_output_pos(wire.out_pin) else {
            continue;
        };
        let Some(to_pos) = state.find_input_pos(wire.in_pin) else {
            continue;
        };

        // Use per-wire style for correct hit testing
        let wire_style = if let Some(node) = graph.get_node(wire.out_pin.node) {
            let info = viewer.output_pin(&node.value, wire.out_pin.output);
            info.wire_style.unwrap_or(config.wire_style)
        } else {
            config.wire_style
        };

        if math::wire_hit_test(
            from_pos,
            to_pos,
            mouse,
            wire_dist,
            wire_style,
            config,
            wire_aabbs,
            wire.out_pin.node,
            wire.in_pin.node,
        ) {
            return HoveredElement::Wire(wire.out_pin, wire.in_pin);
        }
    }

    // Comments are the lowest-priority hit target: only reached when no node,
    // pin, or wire was hit. Iterate in reverse index order so the last-added
    // (top-most) comment wins when they overlap.
    comment_hit_test(graph, state, config, mouse)
}

// ─── Comment hit testing ──────────────────────────────────────────────────────

/// Hit-test comment boxes. Returns the most specific element under the cursor:
/// resize handle > title bar > body. Comments are checked only after nodes,
/// pins, and wires miss, so they never steal interaction from nodes.
fn comment_hit_test<T>(
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    mouse: [f32; 2],
) -> HoveredElement {
    let zoom = state.viewport.zoom;
    for (index, comment) in graph.comments().iter().enumerate().rev() {
        let rect = comments::screen_rect(state, comment.pos, comment.size);
        // Outside the comment rectangle entirely.
        if mouse[0] < rect[0] || mouse[0] >= rect[2] || mouse[1] < rect[1] || mouse[1] >= rect[3] {
            continue;
        }

        // Resize handle (bottom-right) takes priority.
        let handle = comments::resize_handle_rect(rect, zoom);
        if mouse[0] >= handle[0]
            && mouse[1] >= handle[1]
            && mouse[0] < handle[2]
            && mouse[1] < handle[3]
        {
            // Suppress resizing when zoomed out far enough that labels hide,
            // matching node LOD behaviour and avoiding a fiddly tiny handle.
            if config.lod_hide_labels_zoom <= zoom {
                return HoveredElement::CommentResize(index);
            }
        }

        // Title bar → move target.
        let title_bottom = rect[1] + comments::title_bar_height(state);
        if mouse[1] < title_bottom {
            return HoveredElement::CommentTitle(index);
        }

        // Body → context-menu target only (no drag).
        return HoveredElement::CommentBody(index);
    }
    HoveredElement::None
}

// ─── Collapse button hit test ────────────────────────────────────────────────

pub(super) fn is_collapse_button_hit<T>(
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    node_id: NodeId,
    mouse: [f32; 2],
) -> bool {
    let Some(node) = graph.get_node(node_id) else {
        return false;
    };
    let vp = &state.viewport;
    let zoom = vp.zoom;
    let [sx, sy] = vp.graph_to_screen(node.pos);

    let btn_x = sx;
    let btn_y = sy;
    let btn_w = 18.0 * zoom;
    let btn_h = config.node_header_height * zoom;

    mouse[0] >= btn_x && mouse[0] < btn_x + btn_w && mouse[1] >= btn_y && mouse[1] < btn_y + btn_h
}

// ─── Interactive minimap input ───────────────────────────────────────────────

pub(super) fn handle_minimap_input<T>(
    graph: &Graph<T>,
    state: &mut InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    ui: &Ui,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
) {
    let mm = config.minimap_size;
    let margin = config.minimap_margin;
    let mm_pos = match config.minimap_corner {
        0 => [canvas_pos[0] + margin, canvas_pos[1] + margin],
        1 => [
            canvas_pos[0] + canvas_size[0] - mm[0] - margin,
            canvas_pos[1] + margin,
        ],
        2 => [
            canvas_pos[0] + margin,
            canvas_pos[1] + canvas_size[1] - mm[1] - margin,
        ],
        _ => [
            canvas_pos[0] + canvas_size[0] - mm[0] - margin,
            canvas_pos[1] + canvas_size[1] - mm[1] - margin,
        ],
    };

    let mouse = ui.io().mouse_pos();
    let in_minimap = mouse[0] >= mm_pos[0]
        && mouse[0] < mm_pos[0] + mm[0]
        && mouse[1] >= mm_pos[1]
        && mouse[1] < mm_pos[1] + mm[1];

    if ui.is_mouse_clicked(MouseButton::Left) && in_minimap {
        state.minimap_dragging = true;
    }
    if ui.is_mouse_released(MouseButton::Left) {
        state.minimap_dragging = false;
    }

    // Allow drag to continue even when mouse leaves minimap bounds — clamp position.
    if !state.minimap_dragging {
        return;
    }

    let Some((min_x, min_y, max_x, max_y)) = overlays::graph_bounds(graph, config, viewer, 100.0)
    else {
        return;
    };

    let graph_w = max_x - min_x;
    let graph_h = max_y - min_y;
    let scale = (mm[0] / graph_w).min(mm[1] / graph_h);
    let content_w = graph_w * scale;
    let content_h = graph_h * scale;
    let off_x = (mm[0] - content_w) * 0.5;
    let off_y = (mm[1] - content_h) * 0.5;

    // Mouse position → graph-space coordinate (clamped to graph bounds)
    let local_x = (mouse[0] - mm_pos[0] - off_x).clamp(0.0, content_w);
    let local_y = (mouse[1] - mm_pos[1] - off_y).clamp(0.0, content_h);
    let graph_x = local_x / scale + min_x;
    let graph_y = local_y / scale + min_y;

    // Center the viewport on this graph-space point
    let zoom = state.viewport.zoom;
    state.viewport.offset[0] = -(graph_x * zoom) + canvas_size[0] * 0.5;
    state.viewport.offset[1] = -(graph_y * zoom) + canvas_size[1] * 0.5;
}
