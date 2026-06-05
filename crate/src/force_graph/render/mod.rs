//! Main render pipeline for the force-graph widget.
//!
//! Draws edges (lines) then nodes (filled circles + outlines) then labels,
//! using ImGui draw-list primitives. Per-frame input is also handled here —
//! pan, zoom, hover, click, drag, box-select, keyboard, context menu.
//!
//! The low-level draw passes (edges, nodes, shapes, colour resolution, culling)
//! live in the [`draw`] sibling module to keep this file under the 500-line
//! limit; this module owns the per-frame orchestration only.

pub(crate) mod camera;
pub(crate) mod draw;
pub(crate) mod edge_bundle;
pub(crate) mod export;
pub(crate) mod groups;
pub(crate) mod helpers;
pub(crate) mod interaction;
pub(crate) mod labels;
pub(crate) mod minimap;
pub(crate) mod shapes;
pub(crate) mod visibility;

use std::collections::HashSet;

use dear_imgui_rs::{MouseButton, Ui};

use super::config::{ForceConfig, SidebarKind, ViewerConfig};
use super::data::{GraphData, NodeId};
use super::event::GraphEvent;
use super::filter::FilterState;
use super::sim::Simulation;
use super::style::GraphColors;

use camera::Camera;
use draw::{DrawPass, col, graph_bounds};

// ─── Main render function ──────────────────────────────────────────────────────

/// Per-frame render state passed through the pipeline.
pub(crate) struct RenderCtx<'a> {
    pub camera: &'a mut Camera,
    pub sim: &'a mut Simulation,
    pub selection: &'a mut HashSet<NodeId>,
    pub hovered: &'a mut Option<NodeId>,
    pub filter: &'a mut FilterState,
    pub dragging_node: &'a mut Option<NodeId>,
    pub drag_world_offset: &'a mut [f32; 2],
    pub box_select_start: &'a mut Option<[f32; 2]>,
    pub ctx_menu_node: &'a mut Option<NodeId>,
    /// Cached set of hovered node + its neighbours — rebuilt only when hovered changes.
    pub hover_neighbors: &'a mut HashSet<NodeId>,
    pub last_hovered: &'a mut Option<NodeId>,
}

/// Draw the full knowledge-graph for one ImGui frame.
///
/// Returns events to propagate to the caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    ui: &Ui,
    graph: &mut GraphData,
    config: &ViewerConfig,
    force_config: &ForceConfig,
    ctx: &mut RenderCtx<'_>,
    widget_id: &str,
    sidebar_kind: &SidebarKind,
) -> Vec<GraphEvent> {
    let mut events: Vec<GraphEvent> = Vec::with_capacity(8);

    // 1. Resolve color palette.
    //
    // `colors_override` wins, then the active theme drives a synthesized
    // `GraphColors::from_theme(...)` palette. The historic
    // `GraphColors::default()` fallback now lives only inside
    // `from_theme(Theme::Dark)` — calling `default()` directly would lock
    // the graph to the dark palette regardless of the host theme.
    let colors: GraphColors = config
        .colors_override
        .as_deref()
        .cloned()
        .unwrap_or_else(|| GraphColors::from_theme(config.theme));

    // 2. Canvas geometry.
    let canvas_min = ui.cursor_screen_pos();
    let avail = ui.content_region_avail();
    let sidebar_w = match sidebar_kind {
        SidebarKind::None => 0.0_f32,
        _ => 220.0_f32,
    };
    let canvas_size = [avail[0] - sidebar_w, avail[1].max(100.0)];
    let canvas_max = [
        canvas_min[0] + canvas_size[0],
        canvas_min[1] + canvas_size[1],
    ];

    // 3. Tick physics simulation + advance camera animation.
    let dt = ui.io().delta_time();
    ctx.sim.tick(graph, force_config, dt);
    let was_animating = ctx.camera.is_animating();
    ctx.camera.update_inertia(dt, 5.0);
    ctx.camera.update_animation(dt);
    if was_animating {
        events.push(GraphEvent::CameraChanged);
    }

    // 4. Draw background + optional grid.
    let draw = ui.get_window_draw_list();
    draw.add_rect(canvas_min, canvas_max, col(colors.background))
        .filled(true)
        .build();

    if config.background_grid {
        helpers::draw_grid(
            &draw,
            ctx.camera,
            canvas_min,
            canvas_size,
            col(colors.grid_line),
        );
    }

    // 5. Invisible button over the canvas captures mouse input.
    ui.invisible_button(widget_id, canvas_size);
    let canvas_hovered = ui.is_item_hovered();

    // 6. Visibility pass — which nodes to draw.
    let visible = visibility::compute(graph, ctx.filter, config.search_highlight_mode);

    // 7. Handle camera pan and zoom.
    let io = ui.io();
    let mouse = io.mouse_pos();

    if canvas_hovered {
        let wheel = io.mouse_wheel();
        if wheel != 0.0 {
            let factor = if wheel > 0.0 {
                1.12_f32
            } else {
                1.0 / 1.12_f32
            };
            ctx.camera.zoom_at(factor, mouse, canvas_min);
            events.push(GraphEvent::CameraChanged);
        }
    }

    // 8. Hit-test: linear scan (Phase A; Phase B replaces with quadtree).
    let hovered_node = if canvas_hovered && ctx.dragging_node.is_none() {
        helpers::hit_test_nearest(graph, ctx.camera, canvas_min, mouse, force_config, &visible)
    } else {
        *ctx.dragging_node
    };

    // 9. Handle drag (returns true while dragging — suppresses pan).
    let dragging = interaction::handle_drag(
        ui,
        graph,
        ctx.camera,
        canvas_min,
        hovered_node,
        ctx.dragging_node,
        ctx.drag_world_offset,
        config,
        &mut events,
    );

    // 10. Pan when dragging on empty area.  Shift is reserved for box-select,
    //     so plain LMB drag on empty space = pan (matches Obsidian/Figma UX).
    let lmb_down = canvas_hovered && ui.is_mouse_down(MouseButton::Left);
    let shift = io.key_shift();
    if lmb_down && !dragging && hovered_node.is_none() && !shift {
        let delta = ui.mouse_drag_delta(MouseButton::Left);
        if delta[0] != 0.0 || delta[1] != 0.0 {
            ctx.camera.pan(delta);
            ui.reset_mouse_drag_delta(MouseButton::Left);
            events.push(GraphEvent::CameraChanged);
        }
    }

    // 11. Box-select.
    let box_active = interaction::handle_box_select(
        ui,
        ctx.camera,
        canvas_min,
        graph,
        force_config,
        hovered_node,
        ctx.selection,
        ctx.box_select_start,
        config,
        &mut events,
    );

    // 12. Keyboard shortcuts.
    interaction::handle_keyboard(
        ui,
        ctx.camera,
        canvas_hovered,
        ctx.selection,
        graph,
        ctx.sim,
        canvas_size,
        &mut events,
    );

    // 13. Click interaction (single/double/right) — skipped when dragging.
    if !dragging {
        let lmb_clicked = canvas_hovered && ui.is_mouse_clicked(MouseButton::Left);
        let lmb_double = canvas_hovered && ui.is_mouse_double_clicked(MouseButton::Left);
        let rmb_clicked = canvas_hovered && ui.is_mouse_clicked(MouseButton::Right);
        let ctrl = io.key_ctrl();

        if lmb_clicked {
            if let Some(id) = hovered_node {
                if ctrl {
                    if ctx.selection.contains(&id) {
                        ctx.selection.remove(&id);
                    } else {
                        ctx.selection.insert(id);
                    }
                } else {
                    ctx.selection.clear();
                    ctx.selection.insert(id);
                }
                events.push(GraphEvent::NodeClicked(id));
                events.push(GraphEvent::SelectionChanged(ctx.selection.clone()));
            } else if !ctrl && !box_active && !ctx.selection.is_empty() {
                ctx.selection.clear();
                events.push(GraphEvent::SelectionChanged(ctx.selection.clone()));
            }
        }

        if lmb_double && let Some(id) = hovered_node {
            events.push(GraphEvent::NodeDoubleClicked(id));
        }

        if rmb_clicked && let Some(id) = hovered_node {
            *ctx.ctx_menu_node = Some(id);
            events.push(GraphEvent::NodeContextMenu(id, mouse));
        }
    }

    // 14. Emit hover change event.
    if hovered_node != *ctx.hovered {
        *ctx.hovered = hovered_node;
        if let Some(id) = hovered_node {
            events.push(GraphEvent::NodeHovered(id));
        }
    }

    // 15. Context menu.
    interaction::handle_context_menu(
        ui,
        graph,
        ctx.ctx_menu_node,
        ctx.selection,
        ctx.filter,
        ctx.sim,
        &mut events,
        config,
    );

    // 16. FitToScreen event → trigger camera animation.
    if events.iter().any(|e| matches!(e, GraphEvent::FitToScreen))
        && let Some(b) = graph_bounds(graph)
    {
        ctx.camera
            .fit_to_bounds(b[0], b[1], canvas_size, config.fit_padding);
        events.push(GraphEvent::CameraChanged);
    }

    // 17. Update hover-neighbor cache — rebuilt only when hovered node changes.
    if *ctx.hovered != *ctx.last_hovered {
        *ctx.last_hovered = *ctx.hovered;
        ctx.hover_neighbors.clear();
        if let Some(hov) = *ctx.hovered {
            ctx.hover_neighbors.insert(hov);
            for nb in graph.neighbors(hov) {
                ctx.hover_neighbors.insert(nb);
            }
        }
    }
    let hover_active = ctx.hovered.is_some();

    // 18. Determine LOD.
    let node_count = graph.node_count();
    let use_lod = node_count > config.lod_threshold;

    // 19-20. Draw edges then nodes via the `draw` sub-module.
    let pass = DrawPass {
        camera: ctx.camera,
        canvas_min,
        canvas_max,
        colors: &colors,
        hover_active,
        hovered: *ctx.hovered,
        use_lod,
    };

    // Hoist zoom-dependent constant out of the per-edge loop.
    let zoom_thickness = ctx.camera.zoom.clamp(0.5, 2.0) * config.edge_thickness_multiplier;
    draw::draw_edges(
        &draw,
        graph,
        &visible,
        &pass,
        ctx.hover_neighbors,
        ctx.filter.time_threshold,
        config.hover_fade_opacity,
        zoom_thickness,
    );

    // Pre-lowercase the search query ONCE per frame instead of re-lowercasing
    // it inside the per-node loop (was up to N allocations per frame — measured
    // as one of the hottest paths for graphs >5k nodes during the 2026-04-30
    // audit). Empty string when search is inactive — `String::new` does not
    // allocate.
    let search_query_lc: String = if config.search_highlight_mode {
        ctx.filter.search_query.to_ascii_lowercase()
    } else {
        String::new()
    };

    draw::draw_nodes(
        ui,
        &draw,
        graph,
        &visible,
        config,
        force_config,
        &pass,
        ctx.hover_neighbors,
        ctx.selection,
        &search_query_lc,
        ctx.filter.search_match_tags,
    );

    // 21. Draw box-select rectangle overlay.
    interaction::draw_box_rect(
        &draw,
        ctx.box_select_start,
        ctx.camera,
        canvas_min,
        mouse,
        &colors,
    );

    // 22. Minimap overlay (bottom-right corner).
    if config.minimap {
        minimap::render_minimap(ui, graph, ctx.camera, canvas_min, canvas_size, &draw);
    }

    events
}
