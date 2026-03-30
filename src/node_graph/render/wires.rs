//! Wire rendering: drawing wires, flow animation, and dragging preview.
//!
//! Math, obstacle routing, and hit testing live in [`super::math`].

use dear_imgui_rs::Ui;

use crate::utils::color::rgb_arr as c32;

use super::super::config::NodeGraphConfig;
use super::super::graph::Graph;
use super::super::state::{InteractionState, NewWire};
use super::super::types::*;
use super::super::viewer::NodeGraphViewer;
use super::math::{
    bezier_control_points, cubic_bezier, find_obstacles_in_corridor,
    obstacle_aware_bezier_cps, ortho_wire_points, NodeAABB,
};

// ─── Wire rendering ──────────────────────────────────────────────────────────

pub(super) fn render_wires<T>(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    graph: &Graph<T>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    viewer: &dyn NodeGraphViewer<T>,
    time: f32,
    aabbs: &[NodeAABB],
) {
    let colors = &config.colors;

    for wire in graph.wires() {
        let Some(from_pos) = state.find_output_pos(wire.out_pin) else {
            continue;
        };
        let Some(to_pos) = state.find_input_pos(wire.in_pin) else {
            continue;
        };

        // Determine wire color and style from output pin info (single lookup)
        let (wire_color, wire_style) = if let Some(node) = graph.get_node(wire.out_pin.node) {
            let info = viewer.output_pin(&node.value, wire.out_pin.output);
            (info.effective_wire_color(), info.wire_style.unwrap_or(config.wire_style))
        } else {
            (colors.wire_default, config.wire_style)
        };

        let is_hovered =
            state.hovered == super::super::state::HoveredElement::Wire(wire.out_pin, wire.in_pin);
        let color = if is_hovered {
            colors.wire_hovered
        } else {
            wire_color
        };
        let thickness = if is_hovered {
            config.wire_thickness * 1.5
        } else {
            config.wire_thickness
        };

        draw_wire_smart(
            draw, from_pos, to_pos, color, thickness, wire_style, config,
            aabbs, wire.out_pin.node, wire.in_pin.node,
        );

        // ── Wire flow animation (dots moving along the wire) ─────────
        if config.wire_flow {
            render_wire_flow_dots(draw, from_pos, to_pos, wire_color, wire_style, config, time);
        }
    }
}

/// Draw a wire with obstacle-aware routing.
#[allow(clippy::too_many_arguments)]
fn draw_wire_smart(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    from: [f32; 2],
    to: [f32; 2],
    color: [u8; 3],
    thickness: f32,
    style: WireStyle,
    config: &NodeGraphConfig,
    obstacles: &[NodeAABB],
    src_node: NodeId,
    dst_node: NodeId,
) {
    let c = c32(color, 200);
    let margin = 10.0;

    // Check for obstacles in the direct wire corridor
    let (obs_y_min, obs_y_max, has_obstacle) =
        find_obstacles_in_corridor(obstacles, from, to, margin * 2.0, src_node, dst_node);

    match style {
        WireStyle::Line => {
            if !has_obstacle {
                draw.add_line(from, to, c).thickness(thickness).build();
            } else {
                // Route around: go above or below the obstacle cluster
                let go_above = (from[1] - obs_y_min).abs() < (from[1] - obs_y_max).abs();
                let detour_y = if go_above {
                    obs_y_min - margin
                } else {
                    obs_y_max + margin
                };
                let mid_x = (from[0] + to[0]) * 0.5;
                let p1 = [mid_x, detour_y];
                draw.add_line(from, p1, c).thickness(thickness).build();
                draw.add_line(p1, to, c).thickness(thickness).build();
            }
        }
        WireStyle::Bezier => {
            let (cp0, cp1) = obstacle_aware_bezier_cps(
                from, to, config.wire_curvature,
                has_obstacle, obs_y_min, obs_y_max, margin,
            );
            draw.add_bezier_curve(from, cp0, cp1, to, c)
                .thickness(thickness)
                .num_segments(0)
                .build();
        }
        WireStyle::Orthogonal => {
            let poly = ortho_wire_points(from, to, has_obstacle, obs_y_min, obs_y_max, margin);
            for i in 0..poly.len as usize - 1 {
                draw.add_line(poly.points[i], poly.points[i + 1], c)
                    .thickness(thickness)
                    .build();
            }
        }
    }
}

/// Draw a simple wire (for drag preview — no obstacle avoidance needed).
fn draw_wire_simple(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    from: [f32; 2],
    to: [f32; 2],
    color: [u8; 3],
    thickness: f32,
    style: WireStyle,
    config: &NodeGraphConfig,
) {
    let c = c32(color, 200);
    match style {
        WireStyle::Line => {
            draw.add_line(from, to, c).thickness(thickness).build();
        }
        WireStyle::Bezier => {
            let (cp0, cp1) = bezier_control_points(from, to, config.wire_curvature);
            draw.add_bezier_curve(from, cp0, cp1, to, c)
                .thickness(thickness)
                .num_segments(0)
                .build();
        }
        WireStyle::Orthogonal => {
            let mid_x = (from[0] + to[0]) * 0.5;
            let p1 = [mid_x, from[1]];
            let p2 = [mid_x, to[1]];
            draw.add_line(from, p1, c).thickness(thickness).build();
            draw.add_line(p1, p2, c).thickness(thickness).build();
            draw.add_line(p2, to, c).thickness(thickness).build();
        }
    }
}

// ─── Wire flow dots ─────────────────────────────────────────────────────────

/// Draw animated dots along a wire to visualize data flow direction.
fn render_wire_flow_dots(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    from: [f32; 2],
    to: [f32; 2],
    color: [u8; 3],
    style: WireStyle,
    config: &NodeGraphConfig,
    time: f32,
) {
    let spacing = config.wire_flow_spacing;
    let speed = config.wire_flow_speed;
    let dot_r = 2.0;

    let samples = 40;
    let offset = (time * speed) % spacing;

    match style {
        WireStyle::Bezier => {
            let (cp0, cp1) = bezier_control_points(from, to, config.wire_curvature);
            let mut prev = from;
            let mut arc = 0.0_f32;
            let mut arcs: [f32; 41] = [0.0; 41];
            #[allow(clippy::needless_range_loop)]
            for i in 1..=samples {
                let t = i as f32 / samples as f32;
                let pt = cubic_bezier(from, cp0, cp1, to, t);
                let dx = pt[0] - prev[0];
                let dy = pt[1] - prev[1];
                arc += (dx * dx + dy * dy).sqrt();
                arcs[i] = arc;
                prev = pt;
            }
            let mut next_dot = offset;
            for i in 1..=samples {
                while next_dot <= arcs[i] && next_dot > arcs[i - 1] {
                    let frac = (next_dot - arcs[i - 1]) / (arcs[i] - arcs[i - 1]).max(0.001);
                    let t0 = (i - 1) as f32 / samples as f32;
                    let t1 = i as f32 / samples as f32;
                    let t = t0 + frac * (t1 - t0);
                    let pt = cubic_bezier(from, cp0, cp1, to, t);
                    draw.add_circle(pt, dot_r, c32(color, 180))
                        .num_segments(6)
                        .filled(true)
                        .build();
                    next_dot += spacing;
                }
            }
        }
        WireStyle::Line => {
            let dx = to[0] - from[0];
            let dy = to[1] - from[1];
            let length = (dx * dx + dy * dy).sqrt();
            if length < 1.0 { return; }
            let nx = dx / length;
            let ny = dy / length;
            let mut d = offset;
            while d < length {
                let pt = [from[0] + nx * d, from[1] + ny * d];
                draw.add_circle(pt, dot_r, c32(color, 180))
                    .num_segments(6)
                    .filled(true)
                    .build();
                d += spacing;
            }
        }
        WireStyle::Orthogonal => {
            let mid_x = (from[0] + to[0]) * 0.5;
            let segs: [[f32; 2]; 4] = [from, [mid_x, from[1]], [mid_x, to[1]], to];
            let mut total = 0.0_f32;
            for i in 0..3 {
                let dx = segs[i + 1][0] - segs[i][0];
                let dy = segs[i + 1][1] - segs[i][1];
                total += (dx * dx + dy * dy).sqrt();
            }
            let mut d = offset;
            while d < total {
                let mut accum = 0.0_f32;
                for i in 0..3 {
                    let dx = segs[i + 1][0] - segs[i][0];
                    let dy = segs[i + 1][1] - segs[i][1];
                    let seg_len = (dx * dx + dy * dy).sqrt();
                    if accum + seg_len >= d {
                        let frac = (d - accum) / seg_len.max(0.001);
                        let pt = [
                            segs[i][0] + dx * frac,
                            segs[i][1] + dy * frac,
                        ];
                        draw.add_circle(pt, dot_r, c32(color, 180))
                            .num_segments(6)
                            .filled(true)
                            .build();
                        break;
                    }
                    accum += seg_len;
                }
                d += spacing;
            }
        }
    }
}

// ─── Dragging wire ───────────────────────────────────────────────────────────

pub(super) fn render_dragging_wire(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    state: &InteractionState,
    config: &NodeGraphConfig,
    ui: &Ui,
    new_wire: &NewWire,
) {
    let mouse = ui.io().mouse_pos();
    let colors = &config.colors;

    match new_wire {
        NewWire::FromOutput(pin) => {
            if let Some(from_pos) = state.find_output_pos(*pin) {
                draw_wire_simple(
                    draw, from_pos, mouse,
                    colors.wire_dragging, config.wire_thickness, config.wire_style, config,
                );
            }
        }
        NewWire::FromInput(pin) => {
            if let Some(to_pos) = state.find_input_pos(*pin) {
                draw_wire_simple(
                    draw, mouse, to_pos,
                    colors.wire_dragging, config.wire_thickness, config.wire_style, config,
                );
            }
        }
    }
}
