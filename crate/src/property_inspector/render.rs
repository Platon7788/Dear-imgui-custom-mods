//! Draw-list rendering for [`PropertyInspector`](super::PropertyInspector).
//!
//! Split out of `mod.rs` to keep files under 500 lines. The whole
//! widget is painted via the window draw list (no per-cell ImGui
//! widgets) so a frame is a single linear walk over categories →
//! properties → expanded children.

use dear_imgui_rs::{DrawListMut, MouseButton, Ui};

use super::{
    Category, InspectorConfig, PropertyChangedEvent, PropertyInspector, PropertyNode,
    PropertyValue, col32,
};
use crate::utils::text::{calc_text_size, line_height};

/// Hit-test a `[x0, x1) × [y0, y1)` row band against a mouse position.
#[inline]
fn point_in_row(mouse: [f32; 2], x0: f32, x1: f32, y0: f32, y1: f32) -> bool {
    mouse[0] >= x0 && mouse[0] < x1 && mouse[1] >= y0 && mouse[1] < y1
}

/// Per-frame state shared by every row, gathered once so the recursive
/// row walk does not re-query ImGui input on every property.
struct FrameCtx {
    win_pos: [f32; 2],
    win_w: f32,
    key_w: f32,
    line_h: f32,
    mouse_pos: [f32; 2],
    is_clicked: bool,
    window_hovered: bool,
}

impl PropertyInspector {
    /// Render the inspector. Returns change events.
    ///
    /// **Note (2026-04-30 audit):** the inspector currently renders
    /// properties **read-only** — all values display via the draw list,
    /// no edit widgets are wired in. Therefore the returned
    /// `Vec<PropertyChangedEvent>` is **always empty** at the moment.
    /// The signature is preserved so when inline-edit support lands
    /// (planned: text input for `String`, drag widgets for numerics,
    /// checkbox for `Bool`, color picker for `Color`) callers won't need
    /// to migrate. The [`PropertyValue::parse_like`](super::PropertyValue::parse_like)
    /// and [`clamp_in_place`](super::PropertyValue::clamp_in_place)
    /// helpers already exist to back those editors. Tracked as
    /// "implement value-edit widgets" in the deferred-fixes list.
    pub fn render(&mut self, ui: &Ui) -> Vec<PropertyChangedEvent> {
        let events = Vec::new();
        let cfg = self.config; // `Copy`, not clone

        let _id_tok = ui.push_id(&self.id);

        // Filter bar
        if cfg.show_filter {
            ui.set_next_item_width(-1.0);
            ui.input_text("##filter", &mut self.filter).build();
        }

        let avail = ui.content_region_avail();
        let key_w = avail[0].max(1.0) * cfg.key_width_ratio.clamp(0.1, 0.9);

        ui.child_window("##inspector_scroll")
            .size(avail)
            .build(ui, || {
                let draw = ui.get_window_draw_list();
                let win_pos = ui.cursor_screen_pos();
                let ctx = FrameCtx {
                    win_pos,
                    win_w: ui.content_region_avail()[0],
                    key_w,
                    line_h: line_height(ui),
                    mouse_pos: ui.io().mouse_pos(),
                    is_clicked: ui.is_mouse_clicked(MouseButton::Left),
                    window_hovered: ui.is_window_hovered(),
                };

                let mut y = win_pos[1];
                let mut row_idx = 0usize;
                let filter_lower = self.filter.to_lowercase();

                for cat_idx in 0..self.categories.len() {
                    Self::render_category_header(
                        &draw,
                        &ctx,
                        &cfg,
                        &mut self.categories[cat_idx],
                        &mut y,
                    );

                    if self.categories[cat_idx].collapsed {
                        continue;
                    }

                    for prop_idx in 0..self.categories[cat_idx].properties.len() {
                        if !self.categories[cat_idx].properties[prop_idx]
                            .matches_filter(&filter_lower)
                        {
                            continue;
                        }
                        Self::render_property(
                            &draw,
                            &ctx,
                            &cfg,
                            &mut self.categories[cat_idx].properties[prop_idx],
                            &mut y,
                            &mut row_idx,
                        );
                    }
                }

                // Dummy for scroll extent.
                ui.set_cursor_pos([0.0, y - win_pos[1]]);
                ui.dummy([1.0, 1.0]);
            });

        events
    }

    /// Draw a collapsible category header strip and toggle its state on
    /// click. No-op (and no `y` advance) for the unnamed root category
    /// or when category headers are disabled.
    fn render_category_header(
        draw: &DrawListMut<'_>,
        ctx: &FrameCtx,
        cfg: &InspectorConfig,
        cat: &mut Category,
        y: &mut f32,
    ) {
        if !cfg.show_categories || cat.name.is_empty() {
            return;
        }

        let [wx, _] = ctx.win_pos;
        let x1 = wx + ctx.win_w;
        let row_top = *y;
        let row_bot = *y + cfg.row_height;

        // Header background.
        draw.add_rect([wx, row_top], [x1, row_bot], col32(cfg.color_category_bg))
            .filled(true)
            .build();

        let hovered = point_in_row(ctx.mouse_pos, wx, x1, row_top, row_bot);
        if hovered {
            draw.add_rect([wx, row_top], [x1, row_bot], col32([1.0, 1.0, 1.0, 0.04]))
                .filled(true)
                .build();
        }

        let arrow = if cat.collapsed {
            "\u{25B8}"
        } else {
            "\u{25BE}"
        };
        // Two `add_text` calls — the arrow glyph is fixed width, so we
        // know the offset for the name without measuring a heap-allocated
        // `format!` slice every frame.
        let ty = *y + (cfg.row_height - ctx.line_h) * 0.5;
        let arrow_x = wx + 4.0;
        let text_color = col32(cfg.color_category_text);
        draw.add_text([arrow_x, ty], text_color, arrow);
        let arrow_w = calc_text_size(arrow)[0] + 4.0;
        draw.add_text([arrow_x + arrow_w, ty], text_color, &cat.name);

        // Click toggles collapse (guarded by window-hover so a click in
        // the filter input above does not fall through).
        if ctx.window_hovered && ctx.is_clicked && hovered {
            cat.collapsed = !cat.collapsed;
        }

        *y += cfg.row_height;
    }

    /// Render a single property row and its children recursively.
    #[allow(clippy::only_used_in_recursion)]
    fn render_property(
        draw: &DrawListMut<'_>,
        ctx: &FrameCtx,
        cfg: &InspectorConfig,
        prop: &mut PropertyNode,
        y: &mut f32,
        row_idx: &mut usize,
    ) {
        let [wx, _] = ctx.win_pos;
        let x1 = wx + ctx.win_w;
        let row_top = *y;
        let row_bot = *y + cfg.row_height;

        // Alternate row background.
        if *row_idx % 2 == 1 {
            draw.add_rect([wx, row_top], [x1, row_bot], col32(cfg.color_bg_alt))
                .filled(true)
                .build();
        }

        // Changed (diff) highlight.
        if cfg.highlight_changes && prop.changed {
            draw.add_rect([wx, row_top], [x1, row_bot], col32(cfg.color_changed))
                .filled(true)
                .build();
        }

        // Hover highlight.
        let row_hovered = point_in_row(ctx.mouse_pos, wx, x1, row_top, row_bot);
        if row_hovered {
            draw.add_rect([wx, row_top], [x1, row_bot], col32([1.0, 1.0, 1.0, 0.04]))
                .filled(true)
                .build();
        }

        let indent = prop.depth as f32 * cfg.indent;
        let ty = *y + (cfg.row_height - ctx.line_h) * 0.5;

        // Expand arrow + click-to-toggle for nodes with children.
        let has_children = prop.has_children();
        if has_children {
            let arrow = if prop.expanded {
                "\u{25BE}"
            } else {
                "\u{25B8}"
            };
            draw.add_text([wx + indent + 2.0, ty], col32(cfg.color_key), arrow);
            if ctx.window_hovered && ctx.is_clicked && row_hovered {
                prop.expanded = !prop.expanded;
            }
        }

        // Key.
        let key_x = wx + indent + if has_children { 16.0 } else { 4.0 };
        draw.add_text([key_x, ty], col32(cfg.color_key), &prop.key);

        // Key/value separator line.
        let sep_x = wx + ctx.key_w;
        draw.add_line(
            [sep_x, row_top],
            [sep_x, row_bot],
            col32(cfg.color_separator),
        )
        .build();

        // Value (with an inline swatch for color variants).
        let val_x = sep_x + 4.0;
        let val_text = prop.value.display();
        let val_color = if prop.read_only {
            cfg.color_readonly
        } else {
            cfg.color_value
        };
        let swatch = match &prop.value {
            PropertyValue::Color3([r, g, b]) => Some([*r, *g, *b, 1.0]),
            PropertyValue::Color4(c) => Some(*c),
            _ => None,
        };
        if let Some(rgba) = swatch {
            draw.add_rect(
                [val_x, row_top + 2.0],
                [val_x + 14.0, row_bot - 2.0],
                col32(rgba),
            )
            .filled(true)
            .build();
            draw.add_text([val_x + 18.0, ty], col32(val_color), &val_text);
        } else {
            draw.add_text([val_x, ty], col32(val_color), &val_text);
        }

        // Type badge (dimmed, right-aligned).
        let type_badge = prop.value.type_name();
        let badge_x = x1 - calc_text_size(type_badge)[0] - 6.0;
        draw.add_text([badge_x, ty], col32([0.35, 0.38, 0.45, 1.0]), type_badge);

        *y += cfg.row_height;
        *row_idx += 1;

        // Recurse into expanded explicit children.
        if prop.expanded && !prop.children.is_empty() {
            let child_depth = prop.depth + 1;
            for child_idx in 0..prop.children.len() {
                // Take the child out temporarily to satisfy the borrow
                // checker with mutable recursion, then put it back.
                let mut child = std::mem::take(&mut prop.children[child_idx]);
                child.depth = child_depth;
                Self::render_property(draw, ctx, cfg, &mut child, y, row_idx);
                prop.children[child_idx] = child;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_row_is_half_open() {
        // Inclusive on the top-left, exclusive on the bottom-right.
        assert!(point_in_row([0.0, 0.0], 0.0, 10.0, 0.0, 10.0));
        assert!(point_in_row([9.99, 9.99], 0.0, 10.0, 0.0, 10.0));
        assert!(!point_in_row([10.0, 5.0], 0.0, 10.0, 0.0, 10.0));
        assert!(!point_in_row([5.0, 10.0], 0.0, 10.0, 0.0, 10.0));
        assert!(!point_in_row([-0.1, 5.0], 0.0, 10.0, 0.0, 10.0));
    }
}
