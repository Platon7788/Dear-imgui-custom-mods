//! Edge + node draw passes for the force-graph renderer.
//!
//! Split out of `render/mod.rs` to keep that file under the 500-line limit.
//! Low-level shape geometry lives in [`super::shapes`]; stateless geometry /
//! colour helpers live in [`super::helpers`]. These two passes are the only
//! draw-list-heavy work per frame.

use std::collections::HashSet;

use dear_imgui_rs::Ui;

use crate::utils::color::{blend_color, pack_color_f32, with_alpha};

use super::super::config::{ForceConfig, LabelVisibility, ViewerConfig};
use super::super::data::{GraphData, NodeId};
use super::super::style::GraphColors;
use super::camera::Camera;
use super::helpers::{
    contains_ignore_ascii_case, node_radius, resolve_node_color, segment_visible,
};
use super::shapes::{draw_node_outline, draw_node_shape};
use super::visibility::VisibleSet;
use super::{groups, labels};

// Re-export so the rest of `render` keeps importing these from `draw`.
pub(crate) use super::helpers::graph_bounds;

/// Convert `[f32; 4]` RGBA to ImColor32 u32.
#[inline]
pub(crate) fn col(c: [f32; 4]) -> u32 {
    pack_color_f32(c)
}

/// Per-frame state shared by the edge and node draw passes.
pub(super) struct DrawPass<'a> {
    pub camera: &'a Camera,
    pub canvas_min: [f32; 2],
    pub canvas_max: [f32; 2],
    pub colors: &'a GraphColors,
    pub hover_active: bool,
    pub hovered: Option<NodeId>,
    pub use_lod: bool,
}

// ─── Edge pass ─────────────────────────────────────────────────────────────────

/// Draw all visible edges (lines), with hover-fade, LOD thinning, and frustum
/// culling. `zoom_thickness` is hoisted by the caller out of the loop.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_edges(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &GraphData,
    visible: &VisibleSet,
    pass: &DrawPass<'_>,
    hover_neighbors: &HashSet<NodeId>,
    time_threshold: f32,
    hover_fade_opacity: f32,
    zoom_thickness: f32,
) {
    for (_, edge) in graph.edges.iter() {
        let Some(node_a) = graph.nodes.get(edge.from) else {
            continue;
        };
        let Some(node_b) = graph.nodes.get(edge.to) else {
            continue;
        };

        // Skip if either endpoint is invisible.
        if !visible.contains(edge.from) || !visible.contains(edge.to) {
            continue;
        }

        // Time-travel: hide edges created after the threshold.
        if time_threshold.is_finite() && edge.style.created_at > time_threshold {
            continue;
        }

        let sa = pass.camera.world_to_screen(node_a.pos, pass.canvas_min);
        let sb = pass.camera.world_to_screen(node_b.pos, pass.canvas_min);

        if !segment_visible(sa, sb, pass.canvas_min, pass.canvas_max) {
            continue;
        }

        let base_color = edge.style.color.unwrap_or(pass.colors.edge_default);

        // Hover fade: dim edges not connected to hovered node.
        let alpha = if pass.hover_active {
            if hover_neighbors.contains(&edge.from) && hover_neighbors.contains(&edge.to) {
                1.0
            } else {
                hover_fade_opacity
            }
        } else {
            1.0
        };

        let thickness = (1.0 + edge.weight * 2.0) * zoom_thickness;
        let edge_col = if pass.use_lod {
            with_alpha(base_color, 0.5 * alpha)
        } else {
            with_alpha(base_color, alpha)
        };

        draw.add_line(sa, sb, col(edge_col))
            .thickness(thickness)
            .build();
    }
}

// ─── Node pass ─────────────────────────────────────────────────────────────────

/// Draw all visible nodes: shape, outline, selection ring, glow, icon, pin
/// marker, label, and hover tooltip. Mutates the draw list.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_nodes(
    ui: &Ui,
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &GraphData,
    visible: &VisibleSet,
    config: &ViewerConfig,
    force_config: &ForceConfig,
    pass: &DrawPass<'_>,
    hover_neighbors: &HashSet<NodeId>,
    selection: &HashSet<NodeId>,
    search_query_lc: &str,
    search_match_tags: bool,
) {
    for (node_id, node) in graph.nodes.iter() {
        if !visible.contains(node_id) {
            continue;
        }

        let screen_pos = pass.camera.world_to_screen(node.pos, pass.canvas_min);

        let base_radius = node_radius(node_id, node, graph, force_config);
        let screen_radius = (base_radius * pass.camera.zoom * config.node_size_multiplier).max(2.0);

        let margin = screen_radius + 4.0;
        if screen_pos[0] < pass.canvas_min[0] - margin
            || screen_pos[0] > pass.canvas_max[0] + margin
            || screen_pos[1] < pass.canvas_min[1] - margin
            || screen_pos[1] > pass.canvas_max[1] + margin
        {
            continue;
        }

        let num_segments = if pass.use_lod || screen_radius < 4.0 {
            dear_imgui_rs::DrawSegmentCount::count(4)
        } else {
            dear_imgui_rs::DrawSegmentCount::AUTO
        };

        // Hover fade: dim non-neighbor nodes.
        let mut node_alpha = if pass.hover_active {
            if hover_neighbors.contains(&node_id) {
                1.0
            } else {
                config.hover_fade_opacity
            }
        } else {
            1.0
        };

        // Search-highlight: dim nodes that don't match the active query.
        if !search_query_lc.is_empty() {
            let label_match = contains_ignore_ascii_case(&node.style.label, search_query_lc);
            let tag_match = search_match_tags
                && node
                    .style
                    .tags
                    .iter()
                    .any(|t| contains_ignore_ascii_case(t, search_query_lc));
            if !label_match && !tag_match {
                node_alpha *= 0.15;
            }
        }

        // Resolve fill color (color groups take priority).
        let base_fill = groups::resolve_group_color(&node.style, &config.color_groups)
            .unwrap_or_else(|| {
                resolve_node_color(node_id, &node.style, graph, &config.color_mode, pass.colors)
            });

        // Selection / hover tint.
        let fill_color = if selection.contains(&node_id) {
            blend_color(base_fill, pass.colors.node_selected, 0.25)
        } else if pass.hovered == Some(node_id) {
            blend_color(base_fill, pass.colors.node_hover, 0.35)
        } else {
            base_fill
        };
        let fill_color = with_alpha(fill_color, node_alpha);

        // Soft glow halo drawn BEFORE fill so it sits underneath.
        if config.glow_on_hover && (pass.hovered == Some(node_id) || selection.contains(&node_id)) {
            for i in (1u32..=3).rev() {
                let glow_r = screen_radius + i as f32 * 5.0;
                let glow_alpha = (0.10 / i as f32) * node_alpha;
                draw.add_circle(screen_pos, glow_r, col(with_alpha(fill_color, glow_alpha)))
                    .filled(true)
                    .num_segments(dear_imgui_rs::DrawSegmentCount::AUTO)
                    .build();
            }
        }

        // Draw node shape based on NodeKind.
        draw_node_shape(
            draw,
            node.style.kind,
            screen_pos,
            screen_radius,
            col(fill_color),
            num_segments,
        );

        // Outline — matches node shape so squares/diamonds look correct.
        let outline_color = if selection.contains(&node_id) {
            with_alpha(pass.colors.node_selected, node_alpha)
        } else if pass.hovered == Some(node_id) {
            with_alpha(pass.colors.node_hover, node_alpha)
        } else {
            with_alpha(pass.colors.node_outline, node_alpha)
        };
        let outline_thickness = if selection.contains(&node_id) {
            2.5
        } else {
            1.0
        };
        draw_node_outline(
            draw,
            node.style.kind,
            screen_pos,
            screen_radius,
            col(outline_color),
            outline_thickness,
            num_segments,
        );

        // Selection ring (3 px outside fill, shape-matched).
        if selection.contains(&node_id) {
            draw_node_outline(
                draw,
                node.style.kind,
                screen_pos,
                screen_radius + 3.0,
                col(with_alpha(pass.colors.node_selected, node_alpha)),
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
            draw.add_text(
                icon_pos,
                col(with_alpha([1.0, 1.0, 1.0, 0.9], node_alpha)),
                icon_str,
            );
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
            LabelVisibility::HoverOnly => pass.hovered == Some(node_id),
            LabelVisibility::BySize => screen_radius >= 8.0,
            LabelVisibility::Never => false,
        };

        // Text fade: opacity based on zoom relative to threshold.
        let label_alpha = if config.text_fade_threshold != 0.0 {
            let fade =
                (pass.camera.zoom - config.min_label_zoom) / config.text_fade_threshold.abs();
            fade.clamp(0.0, 1.0) * node_alpha
        } else {
            node_alpha
        };

        if show_label
            && pass.camera.zoom >= config.min_label_zoom
            && !pass.use_lod
            && label_alpha > 0.02
        {
            labels::draw_label(
                draw,
                &node.style.label,
                [
                    screen_pos[0] - screen_radius,
                    screen_pos[1] + screen_radius + 2.0,
                ],
                col(with_alpha(pass.colors.label_text, label_alpha)),
                pass.camera.zoom,
                config.min_label_zoom,
            );
        }

        // Tooltip on hover.
        if pass.hovered == Some(node_id) {
            let tip = node.style.tooltip.as_deref().unwrap_or(&node.style.label);
            if !tip.is_empty() {
                crate::utils::themed_tooltip(ui, || ui.text(tip));
            }
        }
    }
}
