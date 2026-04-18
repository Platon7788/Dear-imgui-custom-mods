//! Main render pipeline for the force-graph widget.
//!
//! Draws edges (lines) then nodes (filled circles + outlines) then labels,
//! using ImGui draw-list primitives. Per-frame input is also handled here —
//! pan, zoom, hover, click, drag, box-select, keyboard, context menu.

pub(crate) mod camera;
pub(crate) mod edge_bundle;
pub(crate) mod export;
pub(crate) mod groups;
pub(crate) mod interaction;
pub(crate) mod labels;
pub(crate) mod minimap;
pub(crate) mod visibility;

use std::collections::HashSet;

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::{blend_color, pack_color_f32, with_alpha};

use super::config::{ColorMode, ForceConfig, LabelVisibility, SidebarKind, ViewerConfig};
use super::data::{GraphData, NodeId};
use super::event::GraphEvent;
use super::filter::FilterState;
use super::sim::Simulation;
use super::style::{GraphColors, NodeKind};

use camera::Camera;

// ─── Color helpers ─────────────────────────────────────────────────────────────

/// Convert `[f32; 4]` RGBA to ImColor32 u32.
#[inline]
fn col(c: [f32; 4]) -> u32 {
    pack_color_f32(c)
}

// ─── Okabe-Ito palette (8 colors, colorblind-safe) ────────────────────────────

const OKABE_ITO: [[f32; 4]; 8] = [
    [0.902, 0.624, 0.000, 1.0], // orange
    [0.337, 0.706, 0.914, 1.0], // sky blue
    [0.000, 0.620, 0.451, 1.0], // blue-green
    [0.941, 0.894, 0.259, 1.0], // yellow
    [0.000, 0.447, 0.698, 1.0], // blue
    [0.835, 0.369, 0.000, 1.0], // vermillion
    [0.800, 0.475, 0.655, 1.0], // reddish purple
    [0.600, 0.600, 0.600, 1.0], // gray (fallback)
];

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
    let colors: GraphColors = config
        .colors_override
        .as_deref()
        .cloned()
        .unwrap_or_else(GraphColors::default);

    // 2. Canvas geometry.
    let canvas_min = ui.cursor_screen_pos();
    let avail = ui.content_region_avail();
    let sidebar_w = match sidebar_kind {
        SidebarKind::None => 0.0_f32,
        _ => 220.0_f32,
    };
    let canvas_size = [avail[0] - sidebar_w, avail[1].max(100.0)];
    let canvas_max = [canvas_min[0] + canvas_size[0], canvas_min[1] + canvas_size[1]];

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
        draw_grid(&draw, ctx.camera, canvas_min, canvas_size, col(colors.grid_line));
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
            let factor = if wheel > 0.0 { 1.12_f32 } else { 1.0 / 1.12_f32 };
            ctx.camera.zoom_at(factor, mouse, canvas_min);
            events.push(GraphEvent::CameraChanged);
        }
    }

    // 8. Hit-test: linear scan (Phase A; Phase B replaces with quadtree).
    let hovered_node = if canvas_hovered && ctx.dragging_node.is_none() {
        hit_test_nearest(graph, ctx.camera, canvas_min, mouse, force_config, &visible)
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

        if lmb_double
            && let Some(id) = hovered_node
        {
            events.push(GraphEvent::NodeDoubleClicked(id));
        }

        if rmb_clicked
            && let Some(id) = hovered_node
        {
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
    if events.iter().any(|e| matches!(e, GraphEvent::FitToScreen)) {
        let bounds = graph_bounds(graph);
        if let Some(b) = bounds {
            ctx.camera.fit_to_bounds(b[0], b[1], canvas_size, config.fit_padding);
            events.push(GraphEvent::CameraChanged);
        }
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

    // 19. Draw edges.
    // Hoist zoom-dependent constant out of the per-edge loop.
    let zoom_thickness = ctx.camera.zoom.clamp(0.5, 2.0) * config.edge_thickness_multiplier;
    for (_, edge) in graph.edges.iter() {
        let Some(node_a) = graph.nodes.get(edge.from) else { continue };
        let Some(node_b) = graph.nodes.get(edge.to) else { continue };

        // Skip if either endpoint is invisible.
        if !visible.contains(edge.from) || !visible.contains(edge.to) {
            continue;
        }

        // Time-travel: hide edges created after the threshold.
        if ctx.filter.time_threshold.is_finite()
            && edge.style.created_at > ctx.filter.time_threshold
        {
            continue;
        }

        let sa = ctx.camera.world_to_screen(node_a.pos, canvas_min);
        let sb = ctx.camera.world_to_screen(node_b.pos, canvas_min);

        if !segment_visible(sa, sb, canvas_min, canvas_max) {
            continue;
        }

        let base_color = edge.style.color.unwrap_or(colors.edge_default);

        // Hover fade: dim edges not connected to hovered node.
        let alpha = if hover_active {
            if ctx.hover_neighbors.contains(&edge.from) && ctx.hover_neighbors.contains(&edge.to) {
                1.0
            } else {
                config.hover_fade_opacity
            }
        } else {
            1.0
        };

        let thickness = (1.0 + edge.weight * 2.0) * zoom_thickness;
        let edge_col = if use_lod {
            with_alpha(base_color, 0.5 * alpha)
        } else {
            with_alpha(base_color, alpha)
        };

        draw.add_line(sa, sb, col(edge_col))
            .thickness(thickness)
            .build();
    }

    // 20. Draw nodes.
    for (node_id, node) in graph.nodes.iter() {
        if !visible.contains(node_id) {
            continue;
        }

        let screen_pos = ctx.camera.world_to_screen(node.pos, canvas_min);

        let base_radius = node_radius(node_id, node, graph, force_config);
        let screen_radius = (base_radius * ctx.camera.zoom * config.node_size_multiplier).max(2.0);

        let margin = screen_radius + 4.0;
        if screen_pos[0] < canvas_min[0] - margin
            || screen_pos[0] > canvas_max[0] + margin
            || screen_pos[1] < canvas_min[1] - margin
            || screen_pos[1] > canvas_max[1] + margin
        {
            continue;
        }

        let num_segments: i32 = if use_lod || screen_radius < 4.0 { 4 } else { 0 };

        // Hover fade: dim non-neighbor nodes.
        let mut node_alpha = if hover_active {
            if ctx.hover_neighbors.contains(&node_id) { 1.0 } else { config.hover_fade_opacity }
        } else {
            1.0
        };

        // Search-highlight: dim nodes that don't match the active query.
        if config.search_highlight_mode && !ctx.filter.search_query.is_empty() {
            let q = ctx.filter.search_query.to_ascii_lowercase();
            let label_match = node.style.label.to_ascii_lowercase().contains(&q);
            let tag_match = ctx.filter.search_match_tags
                && node.style.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q));
            if !label_match && !tag_match {
                node_alpha *= 0.15;
            }
        }

        // Resolve fill color (color groups take priority).
        let base_fill = groups::resolve_group_color(&node.style, &config.color_groups)
            .unwrap_or_else(|| {
                resolve_node_color(
                    node_id,
                    &node.style,
                    graph,
                    &config.color_mode,
                    &colors,
                    ctx.hovered,
                    ctx.selection,
                )
            });

        // Selection / hover tint.
        let fill_color = if ctx.selection.contains(&node_id) {
            blend_color(base_fill, colors.node_selected, 0.25)
        } else if *ctx.hovered == Some(node_id) {
            blend_color(base_fill, colors.node_hover, 0.35)
        } else {
            base_fill
        };
        let fill_color = with_alpha(fill_color, node_alpha);

        // Soft glow halo drawn BEFORE fill so it sits underneath.
        if config.glow_on_hover
            && (*ctx.hovered == Some(node_id) || ctx.selection.contains(&node_id))
        {
            for i in (1u32..=3).rev() {
                let glow_r = screen_radius + i as f32 * 5.0;
                let glow_alpha = (0.10 / i as f32) * node_alpha;
                draw.add_circle(screen_pos, glow_r, col(with_alpha(fill_color, glow_alpha)))
                    .filled(true)
                    .num_segments(0)
                    .build();
            }
        }

        // Draw node shape based on NodeKind.
        draw_node_shape(
            &draw,
            node.style.kind,
            screen_pos,
            screen_radius,
            col(fill_color),
            num_segments,
        );

        // Outline — matches node shape so squares/diamonds look correct.
        let outline_color = if ctx.selection.contains(&node_id) {
            with_alpha(colors.node_selected, node_alpha)
        } else if *ctx.hovered == Some(node_id) {
            with_alpha(colors.node_hover, node_alpha)
        } else {
            with_alpha(colors.node_outline, node_alpha)
        };
        let outline_thickness = if ctx.selection.contains(&node_id) { 2.5 } else { 1.0 };
        draw_node_outline(
            &draw,
            node.style.kind,
            screen_pos,
            screen_radius,
            col(outline_color),
            outline_thickness,
            num_segments,
        );

        // Selection ring (3 px outside fill, shape-matched).
        if ctx.selection.contains(&node_id) {
            draw_node_outline(
                &draw,
                node.style.kind,
                screen_pos,
                screen_radius + 3.0,
                col(with_alpha(colors.node_selected, node_alpha)),
                1.5,
                num_segments,
            );
        }

        // Icon centered inside the node (only when radius ≥ 8 screen-px).
        if let Some(icon_char) = node.style.icon
            && screen_radius >= 8.0
        {
            let mut icon_buf = [0u8; 4];
            let icon_str: &str = icon_char.encode_utf8(&mut icon_buf);
            let text_size = crate::utils::text::calc_text_size(icon_str);
            let icon_pos = [
                screen_pos[0] - text_size[0] * 0.5,
                screen_pos[1] - text_size[1] * 0.5,
            ];
            draw.add_text(icon_pos, col(with_alpha([1.0, 1.0, 1.0, 0.9], node_alpha)), icon_str);
        }

        // Pinned indicator: small diamond at top-right.
        if node.style.pinned && screen_radius >= 6.0 {
            let pin_x = screen_pos[0] + screen_radius * 0.7;
            let pin_y = screen_pos[1] - screen_radius * 0.7;
            let s = 3.0_f32;
            draw.add_rect(
                [pin_x - s, pin_y - s],
                [pin_x + s, pin_y + s],
                col([1.0, 0.85, 0.0, node_alpha]),
            )
            .filled(true)
            .build();
        }

        // Labels.
        let show_label = match config.show_labels {
            LabelVisibility::Always => true,
            LabelVisibility::HoverOnly => *ctx.hovered == Some(node_id),
            LabelVisibility::BySize => screen_radius >= 8.0,
            LabelVisibility::Never => false,
        };

        // Text fade: opacity based on zoom relative to threshold.
        let label_alpha = if config.text_fade_threshold != 0.0 {
            let fade = (ctx.camera.zoom - config.min_label_zoom) / config.text_fade_threshold.abs();
            fade.clamp(0.0, 1.0) * node_alpha
        } else {
            node_alpha
        };

        if show_label
            && ctx.camera.zoom >= config.min_label_zoom
            && !use_lod
            && label_alpha > 0.02
        {
            labels::draw_label(
                &draw,
                &node.style.label,
                [screen_pos[0] - screen_radius, screen_pos[1] + screen_radius + 2.0],
                col(with_alpha(colors.label_text, label_alpha)),
                ctx.camera.zoom,
                config.min_label_zoom,
            );
        }

        // Tooltip on hover.
        if *ctx.hovered == Some(node_id) {
            let tip = node
                .style
                .tooltip
                .as_deref()
                .unwrap_or(&node.style.label);
            if !tip.is_empty() {
                ui.tooltip_text(tip);
            }
        }
    }

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

// ─── Node shape drawing ────────────────────────────────────────────────────────

/// Draw the filled shape for a node based on its `NodeKind`.
fn draw_node_shape(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    kind: NodeKind,
    pos: [f32; 2],
    r: f32,
    fill: u32,
    num_segments: i32,
) {
    match kind {
        // Regular + Custom → filled circle.
        NodeKind::Regular | NodeKind::Custom(_) => {
            draw.add_circle(pos, r, fill)
                .filled(true)
                .num_segments(num_segments)
                .build();
        }
        // Tag → filled square.
        NodeKind::Tag => {
            draw.add_rect(
                [pos[0] - r, pos[1] - r],
                [pos[0] + r, pos[1] + r],
                fill,
            )
            .filled(true)
            .build();
        }
        // Attachment → small filled circle (0.7× radius).
        NodeKind::Attachment => {
            draw.add_circle(pos, r * 0.7, fill)
                .filled(true)
                .num_segments(num_segments)
                .build();
        }
        // Unresolved → diamond (two filled triangles).
        NodeKind::Unresolved => {
            let top   = [pos[0],       pos[1] - r];
            let right = [pos[0] + r,   pos[1]    ];
            let bot   = [pos[0],       pos[1] + r];
            let left  = [pos[0] - r,   pos[1]    ];
            draw.add_triangle(top, right, bot, fill).filled(true).build();
            draw.add_triangle(top, bot, left, fill).filled(true).build();
        }
        // Cluster → large circle with octagon approximation.
        NodeKind::Cluster => {
            draw.add_circle(pos, r, fill)
                .filled(true)
                .num_segments(8)
                .build();
        }
    }
}

/// Draw the outline/stroke for a node, shape-matched to its `NodeKind`.
fn draw_node_outline(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    kind: NodeKind,
    pos: [f32; 2],
    r: f32,
    color: u32,
    thickness: f32,
    num_segments: i32,
) {
    match kind {
        NodeKind::Tag => {
            draw.add_rect([pos[0] - r, pos[1] - r], [pos[0] + r, pos[1] + r], color)
                .thickness(thickness)
                .build();
        }
        NodeKind::Unresolved => {
            let top   = [pos[0],     pos[1] - r];
            let right = [pos[0] + r, pos[1]    ];
            let bot   = [pos[0],     pos[1] + r];
            let left  = [pos[0] - r, pos[1]    ];
            draw.add_line(top,   right, color).thickness(thickness).build();
            draw.add_line(right, bot,   color).thickness(thickness).build();
            draw.add_line(bot,   left,  color).thickness(thickness).build();
            draw.add_line(left,  top,   color).thickness(thickness).build();
        }
        NodeKind::Attachment => {
            draw.add_circle(pos, r * 0.7, color)
                .thickness(thickness)
                .num_segments(num_segments)
                .build();
        }
        NodeKind::Cluster => {
            draw.add_circle(pos, r, color)
                .thickness(thickness)
                .num_segments(8)
                .build();
        }
        _ => {
            draw.add_circle(pos, r, color)
                .thickness(thickness)
                .num_segments(num_segments)
                .build();
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

/// Draw a dotted background grid.
fn draw_grid(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    camera: &Camera,
    canvas_min: [f32; 2],
    canvas_size: [f32; 2],
    color: u32,
) {
    let grid_step = 50.0_f32 * camera.zoom;
    if grid_step < 8.0 {
        return;
    }

    let origin_x = canvas_min[0] + camera.offset[0] % grid_step;
    let origin_y = canvas_min[1] + camera.offset[1] % grid_step;
    let canvas_max = [canvas_min[0] + canvas_size[0], canvas_min[1] + canvas_size[1]];

    let mut x = origin_x;
    while x < canvas_max[0] {
        draw.add_line([x, canvas_min[1]], [x, canvas_max[1]], color).build();
        x += grid_step;
    }
    let mut y = origin_y;
    while y < canvas_max[1] {
        draw.add_line([canvas_min[0], y], [canvas_max[0], y], color).build();
        y += grid_step;
    }
}

/// Find the nearest visible node under the cursor (linear scan, Phase A).
fn hit_test_nearest(
    graph: &GraphData,
    camera: &Camera,
    canvas_min: [f32; 2],
    mouse: [f32; 2],
    force_config: &ForceConfig,
    visible: &visibility::VisibleSet,
) -> Option<NodeId> {
    let world_mouse = camera.screen_to_world(mouse, canvas_min);
    let mut best: Option<(NodeId, f32)> = None;

    for (node_id, node) in graph.nodes.iter() {
        if !visible.contains(node_id) {
            continue;
        }
        let r = if force_config.radius_by_degree {
            let deg = graph.adjacency.get(&node_id).map_or(0, |v| v.len());
            force_config.radius_base + force_config.radius_per_degree * deg as f32
        } else {
            node.style.radius.unwrap_or(force_config.radius_base)
        };

        let dx = world_mouse[0] - node.pos[0];
        let dy = world_mouse[1] - node.pos[1];
        let dist_sq = dx * dx + dy * dy;

        if dist_sq <= r * r && (best.is_none() || dist_sq < best.unwrap().1) {
            best = Some((node_id, dist_sq));
        }
    }

    best.map(|(id, _)| id)
}

/// Compute a node's base radius in world space.
fn node_radius(
    id: NodeId,
    node: &super::data::Node,
    graph: &GraphData,
    fc: &ForceConfig,
) -> f32 {
    if let Some(r) = node.style.radius {
        return r;
    }
    if fc.radius_by_degree {
        let deg = graph.adjacency.get(&id).map_or(0, |v| v.len());
        fc.radius_base + fc.radius_per_degree * deg as f32
    } else {
        fc.radius_base
    }
}

/// Resolve the fill color for a node based on the current ColorMode.
fn resolve_node_color(
    id: NodeId,
    style: &super::style::NodeStyle,
    graph: &GraphData,
    mode: &ColorMode,
    colors: &GraphColors,
    hovered: &Option<NodeId>,
    selection: &HashSet<NodeId>,
) -> [f32; 4] {
    let base = match mode {
        ColorMode::Static => style.color.unwrap_or(colors.node_default),
        ColorMode::ByTag => {
            if let Some(tag) = style.tags.first() {
                let hash = fnv1a_hash(tag.as_bytes()) as usize;
                OKABE_ITO[hash % OKABE_ITO.len()]
            } else {
                colors.node_default
            }
        }
        ColorMode::ByCommunity => style.color.unwrap_or(colors.node_default),
        ColorMode::ByPageRank => {
            let score = graph.pagerank_for(id);
            blend_color(colors.node_default, colors.node_selected, score.clamp(0.0, 1.0))
        }
        ColorMode::ByBetweenness => {
            let score = graph.betweenness_for(id);
            blend_color(colors.node_default, colors.node_selected, score.clamp(0.0, 1.0))
        }
        ColorMode::Custom(f) => f(style, graph),
    };

    // Hover + selection tinting (applied by caller after group resolution).
    let _ = (id, hovered, selection); // tinting done in caller
    base
}

/// Compute the AABB of all node positions. Returns `None` when graph is empty.
pub(crate) fn graph_bounds(graph: &GraphData) -> Option<[[f32; 2]; 2]> {
    let mut it = graph.nodes.iter();
    let (_, first) = it.next()?;
    let mut min = first.pos;
    let mut max = first.pos;
    for (_, node) in it {
        min[0] = min[0].min(node.pos[0]);
        min[1] = min[1].min(node.pos[1]);
        max[0] = max[0].max(node.pos[0]);
        max[1] = max[1].max(node.pos[1]);
    }
    Some([min, max])
}

/// Simple FNV-1a hash for stable tag→color mapping.
fn fnv1a_hash(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Returns true if the line segment [a, b] is at least partially inside
/// the rect [min, max]. Used for edge frustum culling.
fn segment_visible(a: [f32; 2], b: [f32; 2], min: [f32; 2], max: [f32; 2]) -> bool {
    let seg_min = [a[0].min(b[0]), a[1].min(b[1])];
    let seg_max = [a[0].max(b[0]), a[1].max(b[1])];
    seg_max[0] >= min[0]
        && seg_min[0] <= max[0]
        && seg_max[1] >= min[1]
        && seg_min[1] <= max[1]
}
