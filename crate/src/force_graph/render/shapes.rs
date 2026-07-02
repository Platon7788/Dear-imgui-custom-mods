//! Node shape primitives for the force-graph renderer.
//!
//! Split out of `render/draw.rs` to keep that file under the 500-line limit.
//! Each function maps a [`NodeKind`] onto draw-list calls for its fill and
//! outline geometry.

use super::super::style::NodeKind;

/// Draw the filled shape for a node based on its `NodeKind`.
pub(super) fn draw_node_shape(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    kind: NodeKind,
    pos: [f32; 2],
    r: f32,
    fill: u32,
    num_segments: dear_imgui_rs::DrawSegmentCount,
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
            draw.add_rect([pos[0] - r, pos[1] - r], [pos[0] + r, pos[1] + r], fill)
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
            let top = [pos[0], pos[1] - r];
            let right = [pos[0] + r, pos[1]];
            let bot = [pos[0], pos[1] + r];
            let left = [pos[0] - r, pos[1]];
            draw.add_triangle(top, right, bot, fill)
                .filled(true)
                .build();
            draw.add_triangle(top, bot, left, fill).filled(true).build();
        }
        // Cluster → large circle with octagon approximation.
        NodeKind::Cluster => {
            draw.add_circle(pos, r, fill)
                .filled(true)
                .num_segments(dear_imgui_rs::DrawSegmentCount::count(8))
                .build();
        }
    }
}

/// Draw the outline/stroke for a node, shape-matched to its `NodeKind`.
pub(super) fn draw_node_outline(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    kind: NodeKind,
    pos: [f32; 2],
    r: f32,
    color: u32,
    thickness: f32,
    num_segments: dear_imgui_rs::DrawSegmentCount,
) {
    match kind {
        NodeKind::Tag => {
            draw.add_rect([pos[0] - r, pos[1] - r], [pos[0] + r, pos[1] + r], color)
                .thickness(thickness)
                .build();
        }
        NodeKind::Unresolved => {
            let top = [pos[0], pos[1] - r];
            let right = [pos[0] + r, pos[1]];
            let bot = [pos[0], pos[1] + r];
            let left = [pos[0] - r, pos[1]];
            draw.add_line(top, right, color)
                .thickness(thickness)
                .build();
            draw.add_line(right, bot, color)
                .thickness(thickness)
                .build();
            draw.add_line(bot, left, color).thickness(thickness).build();
            draw.add_line(left, top, color).thickness(thickness).build();
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
                .num_segments(dear_imgui_rs::DrawSegmentCount::count(8))
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
