//! Node and pin rendering: frame, header, pins, body.

use dear_imgui_rs::Ui;

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
                let pin_id = InPinId { node: node_id, input: i };
                state.input_pin_pos.insert(pin_id, [sx, mid_y]);
            }
            for i in 0..num_outputs {
                let pin_id = OutPinId { node: node_id, output: i };
                state.output_pin_pos.insert(pin_id, [sx + sw, mid_y]);
            }
        } else {
            // Expanded: pins along left/right edges below header
            let header_bottom = sy + config.node_header_height * zoom;
            let pin_start_y = header_bottom + config.node_padding_v * zoom;
            for i in 0..num_inputs {
                let pin_id = InPinId { node: node_id, input: i };
                let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
                let px = sx + config.pin_offset * zoom;
                state.input_pin_pos.insert(pin_id, [px, py]);
            }
            for i in 0..num_outputs {
                let pin_id = OutPinId { node: node_id, output: i };
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
    let node_h = config.node_height(num_inputs, num_outputs, has_body, node.open, viewer.body_height(&node.value));

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
        draw.add_text_with_font(font, scaled_fs, [bx, by], c32(colors.collapse_btn, 200), btn_icon, 0.0, None);
    }

    // ── Title text ───────────────────────────────────────────────────────
    let raw_title_sz = calc_text_size(title);
    let title_sz = [raw_title_sz[0] * zoom, raw_title_sz[1] * zoom];
    let title_offset_x = if config.node_collapsible { 16.0 * zoom } else { 0.0 };
    let title_x = sx + title_offset_x + (sw - title_offset_x - title_sz[0]) * 0.5;
    let title_y = sy + (config.node_header_height * zoom - title_sz[1]) * 0.5;
    draw.add_text_with_font(font, scaled_fs, [title_x, title_y], c32(colors.text, 255), title, 0.0, None);

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
        let pin_id = InPinId { node: node_id, input: i };
        let pin_info = viewer.input_pin(&node.value, i);
        let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
        let px = sx + config.pin_offset * zoom;
        let screen_pos = [px, py];

        let pin_hovered = state.hovered == HoveredElement::InputPin(pin_id);
        if simplify_pins {
            let r = config.pin_radius * zoom * 0.6;
            let fill = if pin_hovered { colors.pin_hovered } else { pin_info.fill };
            draw.add_circle([px, py], r, c32(fill, 255))
                .num_segments(6)
                .filled(true)
                .build();
        } else {
            render_pin(draw, config, zoom, screen_pos, &pin_info, pin_hovered);
        }

        if show_labels {
            let label = viewer.input_label(&node.value, i);
            if !label.is_empty() {
                let lx = px + config.pin_radius * zoom + 4.0 * zoom;
                let ly = py - calc_text_size(label)[1] * zoom * 0.5;
                draw.add_text_with_font(font, scaled_fs, [lx, ly], c32(colors.text_muted, 220), label, 0.0, None);
            }
        }
    }

    // Output pins
    for i in 0..num_outputs {
        let pin_id = OutPinId { node: node_id, output: i };
        let pin_info = viewer.output_pin(&node.value, i);
        let py = pin_start_y + (i as f32 + 0.5) * config.pin_spacing * zoom;
        let px = sx + sw - config.pin_offset * zoom;
        let screen_pos = [px, py];

        let pin_hovered = state.hovered == HoveredElement::OutputPin(pin_id);
        if simplify_pins {
            let r = config.pin_radius * zoom * 0.6;
            let fill = if pin_hovered { colors.pin_hovered } else { pin_info.fill };
            draw.add_circle([px, py], r, c32(fill, 255))
                .num_segments(6)
                .filled(true)
                .build();
        } else {
            render_pin(draw, config, zoom, screen_pos, &pin_info, pin_hovered);
        }

        if show_labels {
            let label = viewer.output_label(&node.value, i);
            if !label.is_empty() {
                let raw_lsz = calc_text_size(label);
                let lx = px - config.pin_radius * zoom - 4.0 * zoom - raw_lsz[0] * zoom;
                let ly = py - raw_lsz[1] * zoom * 0.5;
                draw.add_text_with_font(font, scaled_fs, [lx, ly], c32(colors.text_muted, 220), label, 0.0, None);
            }
        }
    }
}

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
    let node_width = viewer.node_width(&node.value).unwrap_or(config.node_min_width);

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

    let _spacing_token = ui.push_style_var(
        dear_imgui_rs::StyleVar::ItemSpacing([orig_spacing[0] * zoom, orig_spacing[1] * zoom]),
    );

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

fn render_pin(
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
