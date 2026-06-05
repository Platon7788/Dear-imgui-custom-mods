//! L-shaped branch-arrow rendering for [`super::super::DisasmView`].
//!
//! Each cached [`BranchArrow`] is drawn as a source stub, a vertical
//! lane line, a target stub, and a target arrowhead — stubs and head
//! are suppressed at any clipped (offscreen) endpoint so the arrow
//! reads as "continues offscreen". The depth → lane-X mapping is
//! factored into the pure [`lane_x`] helper for unit testing.

use super::*;
use crate::disasm_view::arrows::MAX_ARROW_DEPTH;

/// Spacing (px) between successive arrow lanes inside the arrow
/// column. `MAX_ARROW_DEPTH + 1` slots so even the outermost lane
/// keeps a margin from the column's left edge.
pub(super) fn lane_spacing(arrows_col_w: f32) -> f32 {
    arrows_col_w / (MAX_ARROW_DEPTH as f32 + 1.0)
}

/// X position of an arrow's vertical lane: deeper arrows sit further
/// left of the text-side base. `depth` 0 is closest to the text.
/// Pure → unit-tested.
pub(super) fn lane_x(arrow_base_x: f32, depth: usize, spacing: f32) -> f32 {
    arrow_base_x - (depth as f32 + 1.0) * spacing
}

impl DisasmView {
    /// Draw branch arrows between instructions.
    pub(in crate::disasm_view) fn draw_arrows(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        origin_x: f32,
        origin_y: f32,
    ) {
        let cols = &self.config.columns;
        let colors = &self.config.colors;
        let lh = self.line_height;

        // Arrow area starts after margin.
        let arrow_base_x = origin_x
            + if self.config.show_breakpoints {
                cols.margin
            } else {
                0.0
            }
            + cols.arrows;
        let spacing = lane_spacing(cols.arrows);

        for arrow in &self.cached_arrows {
            let from_y = origin_y + arrow.from_idx as f32 * lh + lh * 0.5;
            let to_y = origin_y + arrow.to_idx as f32 * lh + lh * 0.5;
            let x = lane_x(arrow_base_x, arrow.depth, spacing);

            let color = col32(colors.arrow_color(arrow.flow_kind));
            let thickness = if arrow.depth == 0 { 1.5 } else { 1.0 };

            // Horizontal stub at the source end — suppressed when
            // the source is offscreen (clipped) so the arrow visually
            // reads as "vertical line entering from above/below"
            // rather than "wraps into the gutter at this row".
            if !arrow.clipped_from {
                draw_list
                    .add_line([arrow_base_x, from_y], [x, from_y], color)
                    .thickness(thickness)
                    .build();
            }
            // Vertical line.
            draw_list
                .add_line([x, from_y], [x, to_y], color)
                .thickness(thickness)
                .build();
            // Horizontal stub at the target end — same suppression
            // for clipped target as for clipped source.
            if !arrow.clipped_to {
                draw_list
                    .add_line([x, to_y], [arrow_base_x, to_y], color)
                    .thickness(thickness)
                    .build();
            }

            // Arrowhead at target — only when the target is in
            // window; suppressing the head for clipped targets is
            // the visual cue "continues offscreen".
            if !arrow.clipped_to {
                let dir = if to_y > from_y { 1.0 } else { -1.0 };
                let head_size = 4.0;
                draw_list
                    .add_triangle(
                        [arrow_base_x, to_y],
                        [arrow_base_x - head_size, to_y - head_size * dir],
                        [arrow_base_x - head_size, to_y + head_size * dir],
                        color,
                    )
                    .filled(true)
                    .build();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_spacing_divides_column_into_slots() {
        // MAX_ARROW_DEPTH + 1 slots across the column width.
        let w = (MAX_ARROW_DEPTH as f32 + 1.0) * 5.0;
        assert_eq!(lane_spacing(w), 5.0);
    }

    #[test]
    fn lane_x_depth0_closest_to_base() {
        // depth 0 sits exactly one spacing left of the base.
        assert_eq!(lane_x(100.0, 0, 10.0), 90.0);
    }

    #[test]
    fn lane_x_deeper_arrows_further_left() {
        let base = 100.0;
        let s = 10.0;
        let d0 = lane_x(base, 0, s);
        let d1 = lane_x(base, 1, s);
        let d2 = lane_x(base, 2, s);
        assert!(d1 < d0, "deeper arrows must be further left");
        assert!(d2 < d1);
        assert_eq!(d2, 70.0);
    }

    #[test]
    fn lane_x_outermost_stays_in_column() {
        // The outermost lane (MAX_ARROW_DEPTH - 1) must still be right
        // of the column's left edge given the +1 slot in lane_spacing.
        let col_w = 60.0;
        let s = lane_spacing(col_w);
        let base = 100.0;
        let left_edge = base - col_w;
        let x = lane_x(base, MAX_ARROW_DEPTH - 1, s);
        assert!(x > left_edge, "outermost lane must stay inside the column");
    }
}
