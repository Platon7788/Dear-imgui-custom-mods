//! Tree-column cell: indentation, tree lines, expand glyph/arrow, icon, text.
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
    // ─── Internal: tree cell ────────────────────────────────────────

    pub(super) fn render_tree_cell(
        &mut self,
        ui: &Ui,
        flat_idx: usize,
        flat_row: &flat_view::FlatRow,
        node_id: NodeId,
        row_height: f32,
        row_text_color: Option<[f32; 4]>,
    ) {
        let indent = flat_row.depth as f32 * self.config.indent_width;
        let tree_col = self
            .config
            .tree_column
            .min(self.columns.len().saturating_sub(1));
        let indent_w = self.config.indent_width;

        // ── Tree lines (vertical + horizontal connectors) ────────────
        if self.config.show_tree_lines && flat_row.depth > 0 {
            let draw_list = ui.get_window_draw_list();
            let cursor_screen = ui.cursor_screen_pos();
            // Use the actual row height (density / per-row override aware) so
            // connectors line up with row boundaries — not a hardcoded
            // FrameHeightWithSpacing, which is wrong for Dense/Compact rows.
            let row_h = row_height;
            let line_color = crate::utils::color::rgba_f32(
                self.config.tree_line_color[0],
                self.config.tree_line_color[1],
                self.config.tree_line_color[2],
                self.config.tree_line_color[3],
            );

            // Vertical continuation lines at ancestor depths
            for d in 1..flat_row.depth {
                if flat_row.continuation_mask & (1u64 << d) != 0 {
                    let x = cursor_screen[0] + (d as f32) * indent_w + indent_w * 0.5;
                    draw_list
                        .add_line(
                            [x, cursor_screen[1]],
                            [x, cursor_screen[1] + row_h],
                            line_color,
                        )
                        .build();
                }
            }

            // This node's connector: vertical stub + horizontal branch
            let x = cursor_screen[0] + (flat_row.depth as f32) * indent_w + indent_w * 0.5;
            let mid_y = cursor_screen[1] + row_h * 0.5;

            // Vertical stub: from top of row to mid-y (or full if not last child)
            let vert_end = if flat_row.is_last_child {
                mid_y
            } else {
                cursor_screen[1] + row_h
            };
            draw_list
                .add_line([x, cursor_screen[1]], [x, vert_end], line_color)
                .build();

            // Horizontal branch: from vertical line to arrow/icon
            let arrow_space = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
            let h_end = cursor_screen[0]
                + indent
                + if flat_row.is_leaf {
                    arrow_space * 0.5
                } else {
                    0.0
                };
            draw_list
                .add_line([x, mid_y], [h_end, mid_y], line_color)
                .build();
        }

        // ── Editing the tree column? ────────────────────────────────
        if self.edit_state.is_editing(node_id, tree_col) {
            if indent > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
            }
            self.render_editor_inline(ui, tree_col, node_id);
            return;
        }

        if flat_row.is_leaf {
            // Leaf: indent + (arrow space) + icon + text
            let arrow_width = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
            let total_indent = indent + arrow_width;
            if total_indent > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + total_indent, cursor[1]]);
            }
        } else {
            match &self.config.expand_style {
                config::ExpandStyle::Glyph {
                    collapsed,
                    expanded,
                    color,
                } => {
                    // Custom glyph expand/collapse indicator
                    let glyph = if flat_row.is_expanded {
                        *expanded
                    } else {
                        *collapsed
                    };
                    let glyph_color = *color;

                    // Indent
                    if indent > 0.0 {
                        let cursor = ui.cursor_pos();
                        ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
                    }

                    // Render glyph as a clickable invisible button, zero heap
                    // allocation. `cell_buf` holds "<glyph>##xp<idx>": the whole
                    // string is the button's ImGui ID (the leading glyph is
                    // harmless for an invisible button), while only the
                    // `[..glyph_len]` prefix is drawn. This avoids the previous
                    // raw-pointer `unsafe` slice — the `Arrow` arm does the same.
                    self.cell_buf.clear();
                    self.cell_buf.push(glyph);
                    let font_size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
                    let glyph_len = self.cell_buf.len();
                    let glyph_sz = crate::utils::text::calc_text_size(&self.cell_buf[..glyph_len]);
                    let btn_w = glyph_sz[0].max(font_size);

                    let _ = std::fmt::Write::write_fmt(
                        &mut self.cell_buf,
                        format_args!("##xp{}", flat_idx),
                    );

                    if ui.invisible_button(&self.cell_buf, [btn_w, font_size]) {
                        self.pending_toggle = Some(node_id);
                    }
                    // Draw glyph over the invisible button (use only the glyph portion)
                    let btn_min = ui.item_rect_min();
                    let draw_list = ui.get_window_draw_list();
                    let glyph_x = btn_min[0] + (btn_w - glyph_sz[0]) * 0.5;
                    let glyph_y = btn_min[1];
                    let c =
                        glyph_color.unwrap_or(row_text_color.unwrap_or([0.85, 0.88, 0.92, 1.0]));
                    let color_u32 = crate::utils::color::rgba_f32(c[0], c[1], c[2], c[3]);
                    draw_list.add_text([glyph_x, glyph_y], color_u32, &self.cell_buf[..glyph_len]);

                    ui.same_line_with_spacing(0.0, 4.0);
                }
                config::ExpandStyle::Arrow => {
                    // Custom arrow via invisible_button + draw_list triangle.
                    // Using ImGui TreeNode here would create a second hover-highlight
                    // inside our Selectable, causing a "double focus" artifact.
                    if indent > 0.0 {
                        let cursor = ui.cursor_pos();
                        ui.set_cursor_pos([cursor[0] + indent, cursor[1]]);
                    }

                    let font_size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
                    let btn_sz = font_size;

                    self.cell_buf.clear();
                    let _ = std::fmt::Write::write_fmt(
                        &mut self.cell_buf,
                        format_args!("##ar{}", flat_idx),
                    );

                    if ui.invisible_button(&self.cell_buf, [btn_sz, btn_sz]) {
                        self.pending_toggle = Some(node_id);
                    }

                    // Draw triangle arrow over the invisible button
                    let btn_min = ui.item_rect_min();
                    let draw_list = ui.get_window_draw_list();
                    let arrow_color = crate::utils::color::rgba_f32(0.65, 0.68, 0.72, 1.0);
                    let cx = btn_min[0] + btn_sz * 0.5;
                    let cy = btn_min[1] + btn_sz * 0.5;
                    let r = btn_sz * 0.25;

                    if flat_row.is_expanded {
                        // ▾ Down-pointing triangle
                        draw_list
                            .add_triangle(
                                [cx - r, cy - r * 0.5],
                                [cx + r, cy - r * 0.5],
                                [cx, cy + r],
                                arrow_color,
                            )
                            .filled(true)
                            .build();
                    } else {
                        // ▸ Right-pointing triangle
                        draw_list
                            .add_triangle(
                                [cx - r * 0.5, cy - r],
                                [cx + r, cy],
                                [cx - r * 0.5, cy + r],
                                arrow_color,
                            )
                            .filled(true)
                            .build();
                    }

                    ui.same_line_with_spacing(0.0, 2.0);
                }
            }
        }

        // ── Render icon ─────────────────────────────────────────────
        if let Some(data) = self.arena.get_data(node_id) {
            match data.icon() {
                NodeIcon::None => {}
                NodeIcon::Glyph(ch) => {
                    // Stack-encode the codepoint into a 4-byte buffer — no
                    // String alloc, no heap traffic. Repeated for thousands
                    // of rows it shaves ~1µs/row off the icon column.
                    let mut buf = [0u8; 4];
                    let s = ch.encode_utf8(&mut buf);
                    ui.text(s);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::GlyphColored(ch, color) => {
                    let mut buf = [0u8; 4];
                    let s = ch.encode_utf8(&mut buf);
                    ui.text_colored(color, s);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::ColorSwatch(c) => {
                    let size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
                    let cursor_screen = ui.cursor_screen_pos();
                    let draw_list = ui.get_window_draw_list();
                    let color = crate::utils::color::rgba_f32(c[0], c[1], c[2], c[3]);
                    draw_list
                        .add_rect(
                            cursor_screen,
                            [cursor_screen[0] + size, cursor_screen[1] + size],
                            color,
                        )
                        .filled(true)
                        .build();
                    ui.dummy([size, size]);
                    ui.same_line_with_spacing(0.0, 4.0);
                }
                NodeIcon::Custom => {
                    if data.render_icon(ui) {
                        ui.same_line_with_spacing(0.0, 4.0);
                    }
                }
            }
        }

        // ── Render text + badge ─────────────────────────────────────
        // Use the clamped `tree_col` (not the raw `config.tree_column`): an
        // out-of-range `tree_column` would otherwise panic on `self.columns[…]`.
        if let Some(data) = self.arena.get_data(node_id) {
            self.cell_buf.clear();
            data.cell_display_text(tree_col, &mut self.cell_buf);

            let color = data
                .cell_style(tree_col)
                .and_then(|s| s.text_color)
                .or(row_text_color);

            if let Some(c) = color {
                ui.text_colored(c, &self.cell_buf);
            } else {
                ui.text(&self.cell_buf);
            }

            // Clip tooltip for tree cell text.
            let show_clip_tooltip = self.columns[tree_col]
                .clip_tooltip
                .unwrap_or(self.config.table.default_clip_tooltip);
            if show_clip_tooltip && !self.cell_buf.is_empty() && ui.is_item_hovered() {
                let col_w = ui.current_column_width();
                let text_w = calc_text_size(&self.cell_buf)[0];
                // Account for indent + arrow + icon width
                let arrow_width = unsafe { dear_imgui_rs::sys::igGetTreeNodeToLabelSpacing() };
                let used_w = indent + arrow_width + 20.0; // approximate icon + spacing
                if text_w + used_w > col_w {
                    crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
                }
            }

            // Badge (e.g. children count)
            let badge = data.badge();
            if !badge.is_empty() {
                ui.same_line_with_spacing(0.0, 6.0);
                ui.text_colored([0.50, 0.55, 0.62, 1.0], badge);
            }
        }
    }
}
