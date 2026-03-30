//! Stats overlay, minimap, and graph bounds computation.

use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::InteractionState;
use super::super::viewer::NodeGraphViewer;

// ─── Stats overlay ───────────────────────────────────────────────────────────

pub(super) fn render_stats_overlay<T>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &Graph<T>,
    state: &mut InteractionState,
    config: &NodeGraphConfig,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
) {
    use std::fmt::Write;
    let node_count = graph.node_count();
    let wire_count = graph.wire_count();
    let zoom_pct = (state.viewport.zoom * 100.0).round() as u32;
    let sel = state.selected.len();

    // Reuse scratch buffer — zero alloc after first frame.
    state.fmt_buf.clear();
    let _ = write!(state.fmt_buf, "Nodes: {}  Wires: {}  Zoom: {}%", node_count, wire_count, zoom_pct);
    let line1_end = state.fmt_buf.len();
    if sel > 0 {
        let _ = write!(state.fmt_buf, "\nSelected: {sel}");
    }
    let line1 = &state.fmt_buf[..line1_end];
    let line2 = if sel > 0 { &state.fmt_buf[line1_end + 1..] } else { "" };

    let pad_x = 10.0;
    let pad_y = 6.0;
    // Use actual font metrics for correct sizing across all font sizes.
    let sz1 = calc_text_size(line1);
    let sz2 = if sel > 0 { calc_text_size(line2) } else { [0.0, 0.0] };
    let text_h = sz1[1];
    let box_w = sz1[0].max(sz2[0]) + pad_x * 2.0;
    let box_h = if sel > 0 {
        text_h * 2.0 + 4.0 + pad_y * 2.0
    } else {
        text_h + pad_y * 2.0
    };

    let margin = config.stats_overlay_margin;
    let (bx, by) = match config.stats_overlay_corner {
        0 => (canvas_pos[0] + margin, canvas_pos[1] + margin),
        2 => (canvas_pos[0] + margin, canvas_pos[1] + canvas_size[1] - box_h - margin),
        3 => (
            canvas_pos[0] + canvas_size[0] - box_w - margin,
            canvas_pos[1] + canvas_size[1] - box_h - margin,
        ),
        _ => (canvas_pos[0] + canvas_size[0] - box_w - margin, canvas_pos[1] + margin), // top-right
    };

    // Background pill
    draw.add_rect([bx, by], [bx + box_w, by + box_h], c32([0x10, 0x10, 0x1a], 200))
        .rounding(5.0)
        .filled(true)
        .build();
    // Subtle border
    draw.add_rect([bx, by], [bx + box_w, by + box_h], c32([0x5b, 0x9b, 0xd5], 60))
        .rounding(5.0)
        .filled(false)
        .build();

    // Line 1: stats text
    draw.add_text(
        [bx + pad_x, by + pad_y],
        c32([0xb0, 0xc8, 0xe8], 220),
        line1,
    );

    // Line 2: selection (accent color)
    if !line2.is_empty() {
        draw.add_text(
            [bx + pad_x, by + pad_y + text_h + 4.0],
            c32([0x7b, 0xd5, 0x9b], 240),
            line2,
        );
    }
}

// ─── Mini-map ────────────────────────────────────────────────────────────────

pub(super) fn render_minimap<T>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
    viewer: &dyn NodeGraphViewer<T>,
) {
    let colors = &config.colors;
    let mm = config.minimap_size;
    let margin = config.minimap_margin;

    // Corner position
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

    // Background
    draw.add_rect(mm_pos, [mm_pos[0] + mm[0], mm_pos[1] + mm[1]], c32(colors.minimap_bg, 200))
        .rounding(4.0)
        .filled(true)
        .build();
    draw.add_rect(mm_pos, [mm_pos[0] + mm[0], mm_pos[1] + mm[1]], c32(colors.minimap_outline, 150))
        .rounding(4.0)
        .filled(false)
        .build();

    // Compute graph bounding box (using real node sizes)
    let Some((min_x, min_y, max_x, max_y)) = graph_bounds(graph, config, viewer, 100.0) else {
        return;
    };

    let graph_w = max_x - min_x;
    let graph_h = max_y - min_y;
    let scale = (mm[0] / graph_w).min(mm[1] / graph_h);

    // Center the content within the minimap
    let content_w = graph_w * scale;
    let content_h = graph_h * scale;
    let off_x = (mm[0] - content_w) * 0.5;
    let off_y = (mm[1] - content_h) * 0.5;

    // Render wires as thin lines
    for wire in graph.wires() {
        if let (Some(out_node), Some(in_node)) = (
            graph.get_node(wire.out_pin.node),
            graph.get_node(wire.in_pin.node),
        ) {
            let out_w = viewer.node_width(&out_node.value).unwrap_or(config.node_min_width);
            let out_h = config.node_height(
                viewer.inputs(&out_node.value),
                viewer.outputs(&out_node.value),
                viewer.has_body(&out_node.value),
                out_node.open,
                viewer.body_height(&out_node.value),
            );
            let in_h = config.node_height(
                viewer.inputs(&in_node.value),
                viewer.outputs(&in_node.value),
                viewer.has_body(&in_node.value),
                in_node.open,
                viewer.body_height(&in_node.value),
            );
            let from = [
                mm_pos[0] + off_x + (out_node.pos[0] + out_w - min_x) * scale,
                mm_pos[1] + off_y + (out_node.pos[1] + out_h * 0.5 - min_y) * scale,
            ];
            let to = [
                mm_pos[0] + off_x + (in_node.pos[0] - min_x) * scale,
                mm_pos[1] + off_y + (in_node.pos[1] + in_h * 0.5 - min_y) * scale,
            ];
            draw.add_line(from, to, c32([0x60, 0x60, 0x80], 100))
                .thickness(1.0)
                .build();
        }
    }

    // Render nodes as colored rectangles with headers
    for (nid, node) in graph.nodes() {
        let w = viewer.node_width(&node.value).unwrap_or(config.node_min_width);
        let h = config.node_height(
            viewer.inputs(&node.value),
            viewer.outputs(&node.value),
            viewer.has_body(&node.value),
            node.open,
            viewer.body_height(&node.value),
        );
        let nx = mm_pos[0] + off_x + (node.pos[0] - min_x) * scale;
        let ny = mm_pos[1] + off_y + (node.pos[1] - min_y) * scale;
        let nw = (w * scale).max(3.0);
        let nh = (h * scale).max(3.0);

        // Node body
        let body_color = if state.is_selected(nid) {
            c32(colors.node_bg_selected, 220)
        } else {
            c32(colors.node_bg, 200)
        };
        draw.add_rect([nx, ny], [nx + nw, ny + nh], body_color)
            .rounding(1.0)
            .filled(true)
            .build();

        // Header color tint (top portion)
        let header_h = (config.node_header_height * scale).max(2.0).min(nh * 0.5);
        if let Some(hc) = viewer.header_color(&node.value) {
            draw.add_rect([nx, ny], [nx + nw, ny + header_h], c32(hc, 200))
                .rounding(1.0)
                .filled(true)
                .build();
        }

        // Border — selected nodes get highlight
        let border_color = if state.is_selected(nid) {
            c32(colors.node_border_selected, 255)
        } else {
            c32(colors.node_border, 120)
        };
        let border_thick = if state.is_selected(nid) { 2.0 } else { 1.0 };
        draw.add_rect([nx, ny], [nx + nw, ny + nh], border_color)
            .rounding(1.0)
            .filled(false)
            .thickness(border_thick)
            .build();
    }

    // ── Reticle (optical sight) at camera center ──
    let vp = &state.viewport;
    let cam_center_graph = [
        -vp.offset[0] / vp.zoom + canvas_size[0] * 0.5 / vp.zoom,
        -vp.offset[1] / vp.zoom + canvas_size[1] * 0.5 / vp.zoom,
    ];
    let cx = mm_pos[0] + off_x + (cam_center_graph[0] - min_x) * scale;
    let cy = mm_pos[1] + off_y + (cam_center_graph[1] - min_y) * scale;

    // Check if reticle overlaps any node — invert color on overlap
    let mut over_node = false;
    for (_, node) in graph.nodes() {
        let nw = viewer.node_width(&node.value).unwrap_or(config.node_min_width);
        let nh = config.node_height(
            viewer.inputs(&node.value),
            viewer.outputs(&node.value),
            viewer.has_body(&node.value),
            node.open,
            viewer.body_height(&node.value),
        );
        let nx0 = mm_pos[0] + off_x + (node.pos[0] - min_x) * scale;
        let ny0 = mm_pos[1] + off_y + (node.pos[1] - min_y) * scale;
        let nx1 = nx0 + nw * scale;
        let ny1 = ny0 + nh * scale;
        if cx >= nx0 && cx <= nx1 && cy >= ny0 && cy <= ny1 {
            over_node = true;
            break;
        }
    }

    // Normal: cyan/amber reticle. Over node: inverted bright white/magenta
    let (c_outer, c_inner, c_dot) = if over_node {
        (c32([0xff, 0x40, 0x80], 255), c32([0xff, 0xff, 0xff], 240), c32([0xff, 0x60, 0xa0], 255))
    } else {
        (c32([0x00, 0xd4, 0xaa], 200), c32([0x00, 0xd4, 0xaa], 140), c32([0x00, 0xff, 0xcc], 255))
    };

    let r_outer = 8.0;
    let r_inner = 4.0;
    let gap = 2.5; // gap between inner circle and crosshair lines
    let arm = r_outer + 3.0;

    // Outer ring
    draw.add_circle([cx, cy], r_outer, c_outer)
        .num_segments(24)
        .thickness(1.5)
        .build();

    // Inner ring
    draw.add_circle([cx, cy], r_inner, c_inner)
        .num_segments(16)
        .thickness(1.0)
        .build();

    // Crosshair arms (4 lines with gap near center)
    // Right
    draw.add_line([cx + gap, cy], [cx + arm, cy], c_outer).thickness(1.5).build();
    // Left
    draw.add_line([cx - arm, cy], [cx - gap, cy], c_outer).thickness(1.5).build();
    // Down
    draw.add_line([cx, cy + gap], [cx, cy + arm], c_outer).thickness(1.5).build();
    // Up
    draw.add_line([cx, cy - arm], [cx, cy - gap], c_outer).thickness(1.5).build();

    // Small tick marks at 45° diagonals on outer ring
    let d45 = r_outer * 0.707; // cos(45°)
    let tick = 2.5;
    let d45_in = (r_outer - tick) * 0.707;
    draw.add_line([cx + d45_in, cy + d45_in], [cx + d45, cy + d45], c_inner).thickness(1.0).build();
    draw.add_line([cx - d45_in, cy + d45_in], [cx - d45, cy + d45], c_inner).thickness(1.0).build();
    draw.add_line([cx + d45_in, cy - d45_in], [cx + d45, cy - d45], c_inner).thickness(1.0).build();
    draw.add_line([cx - d45_in, cy - d45_in], [cx - d45, cy - d45], c_inner).thickness(1.0).build();

    // Center dot
    draw.add_circle([cx, cy], 1.5, c_dot)
        .num_segments(8)
        .filled(true)
        .build();
}

// ─── Graph bounds ────────────────────────────────────────────────────────────

/// Compute the graph bounding box with padding, returning `(min_x, min_y, max_x, max_y)`.
/// Returns `None` if the graph is empty.
pub(super) fn graph_bounds<T>(
    graph: &Graph<T>,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    pad: f32,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (_, node) in graph.nodes() {
        let w = viewer.node_width(&node.value).unwrap_or(config.node_min_width);
        let h = config.node_height(
            viewer.inputs(&node.value),
            viewer.outputs(&node.value),
            viewer.has_body(&node.value),
            node.open,
            viewer.body_height(&node.value),
        );
        min_x = min_x.min(node.pos[0]);
        min_y = min_y.min(node.pos[1]);
        max_x = max_x.max(node.pos[0] + w);
        max_y = max_y.max(node.pos[1] + h);
    }
    if min_x >= max_x || min_y >= max_y {
        return None;
    }
    Some((min_x - pad, min_y - pad, max_x + pad, max_y + pad))
}
