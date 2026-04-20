//! Rendering functions for the node graph editor.
//!
//! Split into sub-modules for maintainability:
//! - [`grid`] — canvas grid rendering
//! - [`nodes`] — node frame, pin, and body rendering
//! - [`wires`] — wire routing, rendering, math, and hit testing
//! - [`input`] — mouse/keyboard input handling
//! - [`overlays`] — stats overlay and minimap

mod grid;
mod input;
mod math;
mod nodes;
mod overlays;
mod wires;

use dear_imgui_rs::Ui;

use crate::utils::color::rgb_arr as c32;

use super::config::NodeGraphConfig;
use super::graph::Graph;
use super::state::{HoveredElement, InteractionState};
use super::types::*;
use super::viewer::NodeGraphViewer;

// ─── Main render entry point ─────────────────────────────────────────────────

/// Render the entire node graph. Returns actions for this frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_graph<T>(
    graph: &mut Graph<T>,
    state: &mut InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    ui: &Ui,
    canvas_pos: [f32; 2],
    canvas_size: [f32; 2],
    canvas_hovered: bool,
) -> Vec<GraphAction> {
    let mut actions: Vec<GraphAction> = Vec::new();

    // Reset per-frame state
    state.viewport.canvas_origin = canvas_pos;
    state.hovered = HoveredElement::None;
    state.input_pin_pos.clear();
    state.output_pin_pos.clear();

    // ── Smooth zoom interpolation ─────────────────────────────────────────
    if config.smooth_zoom {
        let dt = ui.io().delta_time();
        let diff = state.zoom_target - state.viewport.zoom;
        if diff.abs() > 0.001 {
            state.viewport.zoom += diff * (1.0 - (-config.smooth_zoom_speed * dt).exp());
        } else {
            state.viewport.zoom = state.zoom_target;
        }
    }

    // Tooltip hover tracking is done AFTER handle_input (which sets state.hovered).

    // Nodes are registered in draw_order via NodeGraph::add_node().
    // Direct graph mutations (ng.graph.insert_node) must be followed by ng.state.ensure_in_draw_order(id).
    // Remove dead nodes from draw order (handles external remove_node calls).
    state.draw_order.retain(|id| {
        let alive = graph.get_node(*id).is_some();
        if !alive {
            state.draw_order_set.remove(id);
        }
        alive
    });

    let draw = ui.get_window_draw_list();
    let colors = &config.colors;

    let canvas_max = [
        canvas_pos[0] + canvas_size[0],
        canvas_pos[1] + canvas_size[1],
    ];

    // ── Canvas background ────────────────────────────────────────────────
    draw.add_rect(canvas_pos, canvas_max, c32(colors.canvas_bg, 255))
        .filled(true)
        .build();

    // ── Grid ─────────────────────────────────────────────────────────────
    if config.show_grid {
        grid::render_grid(&draw, config, &state.viewport, canvas_pos, canvas_size);
    }

    // Snapshot draw order into scratch buffer (zero alloc after first frame).
    state.scratch_draw_order.clear();
    state
        .scratch_draw_order
        .extend_from_slice(&state.draw_order);

    // ── Frustum culling ──────────────────────────────────────────────────
    let vp = &state.viewport;
    let zoom = vp.zoom;
    let margin = 50.0;
    let vp_left = -vp.offset[0] / zoom - margin;
    let vp_top = -vp.offset[1] / zoom - margin;
    let vp_right = vp_left + canvas_size[0] / zoom + margin * 2.0;
    let vp_bottom = vp_top + canvas_size[1] / zoom + margin * 2.0;

    state.scratch_visible.clear();
    for &id in &state.scratch_draw_order {
        if let Some(node) = graph.get_node(id) {
            let nw = viewer
                .node_width(&node.value)
                .unwrap_or(config.node_min_width);
            let nh = config.node_height(
                viewer.inputs(&node.value),
                viewer.outputs(&node.value),
                viewer.has_body(&node.value),
                node.open,
                viewer.body_height(&node.value),
            );
            if node.pos[0] + nw > vp_left
                && node.pos[0] < vp_right
                && node.pos[1] + nh > vp_top
                && node.pos[1] < vp_bottom
            {
                state.scratch_visible.push(id);
            }
        }
    }

    // ── Font reference for scaled text rendering ────────────────────────
    // Text in immutable pass is drawn via add_text_with_font() with explicit
    // font_size * zoom. This avoids modifying global FontSize which would
    // break rect/line rendering in the clip rect block.
    let font = ui.current_font();
    let base_font_size = ui.current_font_size();

    // ── Pre-compute wire obstacle AABBs (once per frame, shared by rendering + hit testing) ──
    let mut aabb_buf: Vec<math::NodeAABB> = Vec::with_capacity(graph.node_count() as usize);
    math::collect_node_aabbs(graph, state, config, viewer, &mut aabb_buf);

    // ── Clip to canvas ───────────────────────────────────────────────────
    draw.with_clip_rect(canvas_pos, canvas_max, || {
        let wire_layer = config.wire_layer;
        let draw_wires = config.show_wires;

        // ── Pre-pass: compute pin positions for ALL nodes ─────────────
        // Pin positions are needed for wire rendering.  Wires can connect
        // a visible node to an off-screen node, so we must compute
        // positions for every node — not just culled ones.  This is cheap
        // (pure arithmetic, no draw calls).
        nodes::precompute_all_pin_positions(graph, state, config, viewer);

        // Wires behind nodes (default).
        if draw_wires && wire_layer == WireLayer::BehindNodes {
            wires::render_wires(
                &draw,
                graph,
                state,
                config,
                viewer,
                ui.time() as f32,
                &aabb_buf,
            );
        }

        // ── Nodes ────────────────────────────────────────────────────────
        for &node_id in &state.scratch_visible {
            nodes::render_node_immutable(
                &draw,
                graph,
                state,
                config,
                viewer,
                node_id,
                font,
                base_font_size,
            );
        }

        // Wires above nodes
        if draw_wires && wire_layer == WireLayer::AboveNodes {
            wires::render_wires(
                &draw,
                graph,
                state,
                config,
                viewer,
                ui.time() as f32,
                &aabb_buf,
            );
        }

        // ── Dragging wire ────────────────────────────────────────────────
        if let Some(ref nw) = state.new_wire {
            wires::render_dragging_wire(&draw, state, config, ui, nw);
        }

        // ── Rectangle selection ──────────────────────────────────────────
        if let Some(ref rect_sel) = state.rect_select {
            let r = rect_sel.rect();
            draw.add_rect(
                [r[0], r[1]],
                [r[2], r[3]],
                c32(colors.selection_rect_fill, 30),
            )
            .filled(true)
            .build();
            draw.add_rect([r[0], r[1]], [r[2], r[3]], c32(colors.selection_rect, 180))
                .filled(false)
                .thickness(1.0)
                .build();
        }
    });

    // ── Stats overlay ────────────────────────────────────────────────────
    if config.show_stats_overlay {
        overlays::render_stats_overlay(&draw, graph, &mut *state, config, canvas_pos, canvas_size);
    }

    // ── Mini-map ─────────────────────────────────────────────────────────
    if config.show_minimap {
        overlays::render_minimap(&draw, graph, state, config, canvas_pos, canvas_size, viewer);
    }

    drop(draw);

    // ── Render mutable bodies (needs &mut graph) ─────────────────────────
    // Clip all body widgets to the canvas so text can't leak outside.
    if zoom >= config.lod_hide_body_zoom {
        // Cache values used by every node body (avoid per-node FFI / clone).
        let body_base_font_size = ui.current_font_size();
        let body_item_spacing = ui.clone_style().item_spacing();
        ui.with_clip_rect(canvas_pos, canvas_max, true, || {
            for &node_id in &state.scratch_visible {
                nodes::render_node_body(
                    graph,
                    state,
                    config,
                    viewer,
                    ui,
                    node_id,
                    body_base_font_size,
                    body_item_spacing,
                );
            }
        });
    }

    // ── Input handling (sets state.hovered via hit testing) ────────────
    input::handle_input(
        graph,
        state,
        config,
        viewer,
        ui,
        canvas_pos,
        canvas_size,
        canvas_hovered,
        &aabb_buf,
        &mut actions,
    );

    // ── Tooltip hover tracking (MUST run after handle_input) ─────────
    {
        let dt = ui.io().delta_time();
        if state.hovered == state.prev_hovered {
            state.hover_time += dt;
        } else {
            state.hover_time = 0.0;
            state.prev_hovered = state.hovered;
        }
    }

    actions
}
