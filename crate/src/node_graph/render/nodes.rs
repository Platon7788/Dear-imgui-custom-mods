//! Node and pin rendering: frame, header, pins, body.
//!
//! The mutable body pass and pin-shape drawing live in the sibling
//! [`super::nodes_body`] module; this file owns the pin position pre-pass and
//! the immutable node frame/header/pin pass.

use crate::icons;
use crate::utils::color::rgb_arr as c32;
use crate::utils::text::calc_text_size;

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::{HoveredElement, InteractionState};
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;

// ─── Pin position pre-pass ───────────────────────────────────────────────────

/// Compute pin screen positions for ALL nodes (not just visible ones).
/// Wires can connect visible nodes to off-screen nodes, so all positions are needed.
pub(super) fn precompute_all_pin_positions<T>(
    graph: &Graph<T>,
    state: &mut InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
) {
    let vp = &state.viewport;
    let zoom = vp.zoom;

    for (node_id, node) in graph.nodes() {
        let num_inputs = viewer.inputs(&node.value);
        let num_outputs = viewer.outputs(&node.value);
        if num_inputs == 0 && num_outputs == 0 {
            continue;
        }

        let node_width = viewer
            .node_width(&node.value)
            .unwrap_or(config.node_min_width);
        let [sx, sy] = vp.graph_to_screen(node.pos);
        let sw = node_width * zoom;

        if !node.open {
            // Collapsed: pins at header mid-height edges
            let mid_y = sy + config.node_header_height * zoom * 0.5;
            for i in 0..num_inputs {
                let pin_id = InPinId {
                    node: node_id,
                    input: i,
                };
                state.input_pin_pos.insert(pin_id, [sx, mid_y]);
            }
            for i in 0..num_outputs {
                let pin_id = OutPinId {
                    node: node_id,
                    output: i,
                };
                state.output_pin_pos.insert(pin_id, [sx + sw, mid_y]);
            }
        } else {
            // Expanded: pins along left/right edges below header
            let header_bottom = sy + config.node_header_height * zoom;
            let pin_start_y = header_bottom + config.node_padding_v * zoom;
            for i in 0..num_inputs {
                let pin_id = InPinId {
                    node: node_id,
                    input: i,
                };
                let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
                let px = sx + config.pin_offset * zoom;
                state.input_pin_pos.insert(pin_id, [px, py]);
            }
            for i in 0..num_outputs {
                let pin_id = OutPinId {
                    node: node_id,
                    output: i,
                };
                let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
                let px = sx + sw - config.pin_offset * zoom;
                state.output_pin_pos.insert(pin_id, [px, py]);
            }
        }
    }
}

// ─── Node rendering (immutable pass — frame, pins, title) ────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn render_node_immutable<T>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    node_id: NodeId,
    font: &dear_imgui_rs::fonts::Font,
    base_font_size: f32,
) {
    let Some(node) = graph.get_node(node_id) else {
        return;
    };
    let colors = &config.colors;
    let vp = &state.viewport;
    let zoom = vp.zoom;
    let is_selected = state.is_selected(node_id);
    let is_hovered = state.hovered == HoveredElement::Node(node_id);

    let title = viewer.title(&node.value);
    let num_inputs = viewer.inputs(&node.value);
    let num_outputs = viewer.outputs(&node.value);
    let has_body = viewer.has_body(&node.value);

    let node_width = viewer
        .node_width(&node.value)
        .unwrap_or(config.node_min_width);
    let node_h = config.node_height(
        num_inputs,
        num_outputs,
        has_body,
        node.open,
        viewer.body_height(&node.value),
    );

    // Screen-space position & size
    let [sx, sy] = vp.graph_to_screen(node.pos);
    let sw = node_width * zoom;
    let sh = node_h * zoom;

    let p0 = [sx, sy];
    let p1 = [sx + sw, sy + sh];

    // ── Node shadow ──────────────────────────────────────────────────────
    if config.node_shadow {
        let off = config.node_shadow_offset * zoom;
        draw.add_rect(
            [sx + off, sy + off],
            [sx + sw + off, sy + sh + off],
            c32([0x00, 0x00, 0x00], config.node_shadow_alpha),
        )
        .rounding(config.node_rounding * zoom)
        .filled(true)
        .build();
    }

    // ── Node body background ─────────────────────────────────────────────
    let bg_color = if is_selected {
        colors.node_bg_selected
    } else if is_hovered {
        colors.node_bg_hovered
    } else {
        colors.node_bg
    };
    draw.add_rect(p0, p1, c32(bg_color, 240))
        .rounding(config.node_rounding * zoom)
        .filled(true)
        .build();

    // ── Header background ────────────────────────────────────────────────
    let header_color = viewer
        .header_color(&node.value)
        .unwrap_or(colors.node_header_bg);
    let header_bottom = sy + config.node_header_height * zoom;

    // Top half with rounding
    draw.add_rect(p0, [sx + sw, header_bottom], c32(header_color, 230))
        .rounding(config.node_rounding * zoom)
        .filled(true)
        .build();
    // Fill bottom corners of header (overlap with body)
    let overlap = config.node_rounding * zoom;
    if overlap > 0.0 && node.open {
        draw.add_rect(
            [sx, header_bottom - overlap],
            [sx + sw, header_bottom],
            c32(header_color, 230),
        )
        .filled(true)
        .build();
    }

    // Header separator
    if node.open {
        draw.add_line(
            [sx + 4.0, header_bottom],
            [sx + sw - 4.0, header_bottom],
            c32(colors.node_border, 120),
        )
        .build();
    }

    // Scaled font size for proportional text rendering
    let scaled_fs = base_font_size * zoom;

    // ── Collapse button ──────────────────────────────────────────────────
    if config.node_collapsible {
        let btn_icon = if node.open {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        let isz = calc_text_size(btn_icon);
        let bx = sx + 4.0 * zoom;
        let by = sy + (config.node_header_height * zoom - isz[1] * zoom) * 0.5;
        draw.add_text_with_font(
            font,
            scaled_fs,
            [bx, by],
            c32(colors.collapse_btn, 200),
            btn_icon,
            0.0,
            None,
        );
    }

    // ── Title text ───────────────────────────────────────────────────────
    let raw_title_sz = calc_text_size(title);
    let title_sz = [raw_title_sz[0] * zoom, raw_title_sz[1] * zoom];
    let title_offset_x = if config.node_collapsible {
        16.0 * zoom
    } else {
        0.0
    };
    let title_x = sx + title_offset_x + (sw - title_offset_x - title_sz[0]) * 0.5;
    let title_y = sy + (config.node_header_height * zoom - title_sz[1]) * 0.5;
    draw.add_text_with_font(
        font,
        scaled_fs,
        [title_x, title_y],
        c32(colors.text, 255),
        title,
        0.0,
        None,
    );

    // ── Border ───────────────────────────────────────────────────────────
    let border_color = if is_selected {
        colors.node_border_selected
    } else {
        colors.node_border
    };
    let border_thick = if is_selected {
        config.node_border_thickness * 1.5
    } else {
        config.node_border_thickness
    };
    draw.add_rect(p0, p1, c32(border_color, 200))
        .rounding(config.node_rounding * zoom)
        .filled(false)
        .thickness(border_thick)
        .build();

    // ── Pins (only when expanded) ────────────────────────────────────────
    // Pin positions are already computed by precompute_pin_positions().
    if !node.open {
        return;
    }

    let pin_start_y = header_bottom + config.node_padding_v * zoom;
    let show_labels = zoom >= config.lod_hide_labels_zoom;
    let simplify_pins = zoom < config.lod_simplify_pins_zoom;

    // Input pins
    for i in 0..num_inputs {
        let pin_id = InPinId {
            node: node_id,
            input: i,
        };
        let pin_info = viewer.input_pin(&node.value, i);
        let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
        let px = sx + config.pin_offset * zoom;
        let screen_pos = [px, py];

        let pin_hovered = state.hovered == HoveredElement::InputPin(pin_id);
        if simplify_pins {
            let r = config.pin_radius * zoom * 0.6;
            let fill = if pin_hovered {
                colors.pin_hovered
            } else {
                pin_info.fill
            };
            draw.add_circle([px, py], r, c32(fill, 255))
                .num_segments(6)
                .filled(true)
                .build();
        } else {
            super::nodes_body::render_pin(draw, config, zoom, screen_pos, &pin_info, pin_hovered);
        }

        if show_labels {
            let label = viewer.input_label(&node.value, i);
            if !label.is_empty() {
                let lx = px + config.pin_radius * zoom + 4.0 * zoom;
                let ly = py - calc_text_size(label)[1] * zoom * 0.5;
                draw.add_text_with_font(
                    font,
                    scaled_fs,
                    [lx, ly],
                    c32(colors.text_muted, 220),
                    label,
                    0.0,
                    None,
                );
            }
        }
    }

    // Output pins
    for i in 0..num_outputs {
        let pin_id = OutPinId {
            node: node_id,
            output: i,
        };
        let pin_info = viewer.output_pin(&node.value, i);
        let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
        let px = sx + sw - config.pin_offset * zoom;
        let screen_pos = [px, py];

        let pin_hovered = state.hovered == HoveredElement::OutputPin(pin_id);
        if simplify_pins {
            let r = config.pin_radius * zoom * 0.6;
            let fill = if pin_hovered {
                colors.pin_hovered
            } else {
                pin_info.fill
            };
            draw.add_circle([px, py], r, c32(fill, 255))
                .num_segments(6)
                .filled(true)
                .build();
        } else {
            super::nodes_body::render_pin(draw, config, zoom, screen_pos, &pin_info, pin_hovered);
        }

        if show_labels {
            let label = viewer.output_label(&node.value, i);
            if !label.is_empty() {
                let raw_lsz = calc_text_size(label);
                let lx = px - config.pin_radius * zoom - 4.0 * zoom - raw_lsz[0] * zoom;
                let ly = py - raw_lsz[1] * zoom * 0.5;
                draw.add_text_with_font(
                    font,
                    scaled_fs,
                    [lx, ly],
                    c32(colors.text_muted, 220),
                    label,
                    0.0,
                    None,
                );
            }
        }
    }
}

// Node body rendering (mutable pass) and pin-shape drawing live in the sibling
// `nodes_body` module to keep this file under the 500-line cap.
pub(super) use super::nodes_body::render_node_body;
