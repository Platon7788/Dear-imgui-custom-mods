//! Main render pipeline for the knowledge-graph widget.
//!
//! Draws edges (lines) then nodes (filled circles + outlines) then labels,
//! using ImGui draw-list primitives. Per-frame input is also handled here —
//! pan, zoom, hover, click.

pub(crate) mod camera;
pub(crate) mod edge_bundle;
pub(crate) mod labels;
pub(crate) mod minimap;

use std::collections::HashSet;

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::pack_color_f32;

use super::config::{ColorMode, ForceConfig, LabelVisibility, SidebarKind, ViewerConfig};
use super::data::{GraphData, NodeId};
use super::event::GraphEvent;
use super::sim::Simulation;
use super::style::GraphColors;

use camera::Camera;

// ─── Color helpers ─────────────────────────────────────────────────────────────

/// Convert `[f32; 4]` RGBA to ImColor32 u32.
#[inline]
fn col(c: [f32; 4]) -> u32 {
    pack_color_f32(c)
}

/// Blend two colors by alpha `t` (0.0 = a, 1.0 = b).
#[inline]
fn blend_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Apply an alpha multiplier to a color.
#[inline]
fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
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
    let mut events: Vec<GraphEvent> = Vec::new();

    // 1. Resolve color palette.
    let colors: GraphColors = config
        .colors_override
        .as_deref()
        .cloned()
        .unwrap_or_else(GraphColors::default);

    // 2. Canvas geometry.
    let canvas_min = ui.cursor_screen_pos();
    let avail = ui.content_region_avail();
    // Reserve space for sidebar if needed.
    let sidebar_w = match sidebar_kind {
        SidebarKind::None => 0.0_f32,
        _ => 200.0_f32,
    };
    let canvas_size = [avail[0] - sidebar_w, avail[1].max(100.0)];
    let canvas_max = [canvas_min[0] + canvas_size[0], canvas_min[1] + canvas_size[1]];

    // 3. Tick physics simulation.
    let dt = ui.io().delta_time();
    ctx.sim.tick(graph, force_config, dt);

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

    // 6. Handle camera pan and zoom.
    let io = ui.io();
    let mouse = io.mouse_pos();
    let prev_cam = *ctx.camera;

    if canvas_hovered {
        // Zoom via scroll wheel.
        let wheel = io.mouse_wheel();
        if wheel != 0.0 {
            let factor = if wheel > 0.0 { 1.12_f32 } else { 1.0 / 1.12_f32 };
            ctx.camera.zoom_at(factor, mouse, canvas_min);
            events.push(GraphEvent::CameraChanged);
        }

        // Pan via left-drag on empty area (detected after hit-test below).
    }

    // 7. Hit-test: linear scan (Phase A). Phase B replaces with quadtree.
    let hovered_node = if canvas_hovered {
        hit_test_nearest(ui, graph, ctx.camera, canvas_min, mouse, force_config)
    } else {
        None
    };

    // 8. Handle click interaction.
    let lmb_clicked = canvas_hovered && ui.is_mouse_clicked(MouseButton::Left);
    let lmb_down = canvas_hovered && ui.is_mouse_down(MouseButton::Left);
    let lmb_double = canvas_hovered && ui.is_mouse_double_clicked(MouseButton::Left);
    let rmb_clicked = canvas_hovered && ui.is_mouse_clicked(MouseButton::Right);
    let ctrl = io.key_ctrl();

    // Pan when dragging on empty area (no node hovered).
    if lmb_down && hovered_node.is_none() {
        let delta = ui.mouse_drag_delta(MouseButton::Left);
        if delta[0] != 0.0 || delta[1] != 0.0 {
            ctx.camera.pan(delta);
            ui.reset_mouse_drag_delta(MouseButton::Left);
            events.push(GraphEvent::CameraChanged);
        }
    }

    // Emit hover event.
    if hovered_node != *ctx.hovered {
        *ctx.hovered = hovered_node;
        if let Some(id) = hovered_node {
            events.push(GraphEvent::NodeHovered(id));
        }
    }

    // Handle selection + click events.
    if lmb_clicked {
        if let Some(id) = hovered_node {
            if ctrl {
                // Ctrl+click toggles.
                if ctx.selection.contains(&id) {
                    ctx.selection.remove(&id);
                } else {
                    ctx.selection.insert(id);
                }
            } else {
                // Normal click: single-select.
                ctx.selection.clear();
                ctx.selection.insert(id);
            }
            events.push(GraphEvent::NodeClicked(id));
            events.push(GraphEvent::SelectionChanged(ctx.selection.clone()));
        } else {
            // Click on empty area clears selection.
            if !ctrl && !ctx.selection.is_empty() {
                ctx.selection.clear();
                events.push(GraphEvent::SelectionChanged(ctx.selection.clone()));
            }
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
        events.push(GraphEvent::NodeContextMenu(id, mouse));
    }

    // Camera-changed event when pan happened via inertia decay.
    let _ = prev_cam; // reserved for inertia comparison

    // 9. Determine node count for LOD decision.
    let node_count = graph.node_count();
    let use_lod = node_count > config.lod_threshold;

    // 10. Draw edges.
    for (_, edge) in graph.edges.iter() {
        let Some(node_a) = graph.nodes.get(edge.from) else { continue };
        let Some(node_b) = graph.nodes.get(edge.to) else { continue };

        let sa = ctx.camera.world_to_screen(node_a.pos, canvas_min);
        let sb = ctx.camera.world_to_screen(node_b.pos, canvas_min);

        // Frustum-cull edges that are fully outside the canvas.
        if !segment_visible(sa, sb, canvas_min, canvas_max) {
            continue;
        }

        let base_color = edge.style.color.unwrap_or(colors.edge_default);
        let thickness = (1.0 + edge.weight * 2.0) * ctx.camera.zoom.clamp(0.5, 2.0);
        let edge_col = if use_lod {
            // LOD: simplified, no AA
            with_alpha(base_color, 0.5)
        } else {
            base_color
        };

        draw.add_line(sa, sb, col(edge_col))
            .thickness(thickness)
            .build();
    }

    // 11. Draw nodes.
    for (node_id, node) in graph.nodes.iter() {
        let screen_pos = ctx.camera.world_to_screen(node.pos, canvas_min);

        // Frustum-cull nodes outside the canvas (with margin for radius).
        let margin = 50.0_f32;
        if screen_pos[0] < canvas_min[0] - margin
            || screen_pos[0] > canvas_max[0] + margin
            || screen_pos[1] < canvas_min[1] - margin
            || screen_pos[1] > canvas_max[1] + margin
        {
            continue;
        }

        let base_radius = node_radius(node_id, node, graph, force_config);
        let screen_radius = (base_radius * ctx.camera.zoom).max(2.0);

        // LOD: simplified circle for dense graphs.
        let num_segments = if use_lod || screen_radius < 4.0 { 4 } else { 0 };

        // Resolve fill color.
        let fill_color = resolve_node_color(
            node_id,
            &node.style,
            graph,
            &config.color_mode,
            &colors,
            ctx.hovered,
            ctx.selection,
        );

        // Fill.
        draw.add_circle(screen_pos, screen_radius, col(fill_color))
            .filled(true)
            .num_segments(num_segments)
            .build();

        // Outline.
        let outline_color = if ctx.selection.contains(&node_id) {
            colors.node_selected
        } else if *ctx.hovered == Some(node_id) {
            colors.node_hover
        } else {
            colors.node_outline
        };
        let outline_thickness = if ctx.selection.contains(&node_id) { 2.5 } else { 1.0 };
        draw.add_circle(screen_pos, screen_radius, col(outline_color))
            .thickness(outline_thickness)
            .num_segments(num_segments)
            .build();

        // Selection ring (extra bright ring for selected nodes).
        if ctx.selection.contains(&node_id) {
            draw.add_circle(screen_pos, screen_radius + 3.0, col(colors.node_selected))
                .thickness(1.5)
                .num_segments(num_segments)
                .build();
        }

        // Labels.
        let show_label = match config.show_labels {
            LabelVisibility::Always => true,
            LabelVisibility::HoverOnly => *ctx.hovered == Some(node_id),
            LabelVisibility::BySize => screen_radius >= 8.0,
            LabelVisibility::Never => false,
        };

        if show_label && ctx.camera.zoom >= config.min_label_zoom && !use_lod {
            labels::draw_label(
                &draw,
                &node.style.label,
                [screen_pos[0] - screen_radius, screen_pos[1] + screen_radius + 2.0],
                col(colors.label_text),
                ctx.camera.zoom,
                config.min_label_zoom,
            );
        }

        // Tooltip on hover.
        if *ctx.hovered == Some(node_id) && !node.style.label.is_empty() {
            ui.tooltip_text(&node.style.label);
        }
    }

    events
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
        return; // grid too dense at low zoom, skip
    }

    let origin_x = canvas_min[0] + camera.offset[0] % grid_step;
    let origin_y = canvas_min[1] + camera.offset[1] % grid_step;
    let canvas_max = [canvas_min[0] + canvas_size[0], canvas_min[1] + canvas_size[1]];

    let mut x = origin_x;
    while x < canvas_max[0] {
        draw.add_line([x, canvas_min[1]], [x, canvas_max[1]], color)
            .build();
        x += grid_step;
    }
    let mut y = origin_y;
    while y < canvas_max[1] {
        draw.add_line([canvas_min[0], y], [canvas_max[0], y], color)
            .build();
        y += grid_step;
    }
}

/// Find the nearest node under the cursor (linear scan, Phase A).
fn hit_test_nearest(
    ui: &Ui,
    graph: &GraphData,
    camera: &Camera,
    canvas_min: [f32; 2],
    mouse: [f32; 2],
    force_config: &ForceConfig,
) -> Option<NodeId> {
    let _ = ui; // reserved for Phase B quadtree index
    let world_mouse = camera.screen_to_world(mouse, canvas_min);
    let mut best: Option<(NodeId, f32)> = None;

    for (node_id, node) in graph.nodes.iter() {
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
        ColorMode::ByCommunity => {
            // Phase C stub: fall back to Static.
            style.color.unwrap_or(colors.node_default)
        }
        ColorMode::ByPageRank => {
            let pr = graph.pagerank();
            // Phase C: pr is empty. Fall back to default.
            if pr.is_empty() {
                colors.node_default
            } else {
                blend_color(colors.node_default, colors.node_selected, pr[0])
            }
        }
        ColorMode::ByBetweenness => {
            let bc = graph.betweenness_centrality();
            if bc.is_empty() {
                colors.node_default
            } else {
                blend_color(colors.node_default, colors.node_selected, bc[0])
            }
        }
        ColorMode::Custom(f) => f(style, graph),
    };

    // Hover + selection tinting.
    if selection.contains(&id) {
        blend_color(base, colors.node_selected, 0.25)
    } else if *hovered == Some(id) {
        blend_color(base, colors.node_hover, 0.35)
    } else {
        base
    }
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
fn segment_visible(
    a: [f32; 2],
    b: [f32; 2],
    min: [f32; 2],
    max: [f32; 2],
) -> bool {
    // AABB of the segment vs canvas rect.
    let seg_min = [a[0].min(b[0]), a[1].min(b[1])];
    let seg_max = [a[0].max(b[0]), a[1].max(b[1])];
    seg_max[0] >= min[0]
        && seg_min[0] <= max[0]
        && seg_max[1] >= min[1]
        && seg_min[1] <= max[1]
}
