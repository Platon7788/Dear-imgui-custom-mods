//! Per-row rendering (selectable, drag-drop, data cells).
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
    // ─── Internal: row rendering ────────────────────────────────────

    pub(super) fn render_row(&mut self, ui: &Ui, flat_idx: usize) {
        let flat_row = match self.flat_view.rows.get(flat_idx) {
            Some(r) => *r,
            None => return,
        };
        let node_id = flat_row.node_id;

        // Extract row-level data
        let row_style = self.arena.get_data(node_id).and_then(|d| d.row_style());

        // Row height
        let auto_h = self.density_row_height();
        let row_height = row_style
            .as_ref()
            .and_then(|s| s.height)
            .or(self.config.table.default_row_height)
            .unwrap_or(auto_h);

        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);

        let is_selected = self.selected_nodes.contains(&node_id);

        // Row background:
        // - Unselected: `row_style.bg_color` overrides the striped zebra.
        // - Selected:   `row_style.selection_color` override (per-row); if
        //   absent, leave the bg to `Selectable`'s built-in `Header` tint
        //   so the default selection highlight stays intact.
        if is_selected {
            if let Some(ref style) = row_style
                && let Some(sel_bg) = style.selection_color
                && sel_bg[3] > 0.0
            {
                ui.table_set_row_bg1_color(sel_bg);
            }
        } else if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_row_bg1_color(bg);
        } else if self.config.striped && flat_idx % 2 == 1 {
            ui.table_set_row_bg1_color([1.0, 1.0, 1.0, 0.02]);
        }

        let _row_id = ui.push_id(flat_idx);

        // Selectable spanning all columns
        ui.table_next_column();
        if ui
            .selectable_config("##sel")
            .flags(
                SelectableFlags::ALLOW_DOUBLE_CLICK
                    | SelectableFlags::SPAN_ALL_COLUMNS
                    | SelectableFlags::ALLOW_OVERLAP,
            )
            .selected(is_selected)
            .size([0.0, row_height])
            .build()
        {
            self.handle_selection(ui, flat_idx);

            if ui.is_mouse_double_clicked(MouseButton::Left) {
                self.double_clicked_node = Some(node_id);
                if self.config.expand_on_double_click && !flat_row.is_leaf {
                    self.pending_toggle = Some(node_id);
                }
            }

            // Edit trigger
            let activate_edit = match self.config.table.edit_trigger {
                EditTrigger::DoubleClick => ui.is_mouse_double_clicked(MouseButton::Left),
                EditTrigger::SingleClick => true,
                _ => false,
            };
            if activate_edit && let Some(hovered_col) = ui.table_get_hovered_column().column() {
                self.try_activate_edit(flat_idx, hovered_col.get());
            }
        }

        // Tooltip
        if ui.is_item_hovered()
            && let Some(data) = self.arena.get_data(node_id)
            && !data.render_tooltip(ui)
        {
            self.cell_buf.clear();
            data.row_tooltip(&mut self.cell_buf);
            if !self.cell_buf.is_empty() {
                crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, flat_idx);
            self.context_node = Some(node_id);
            self.context_col = ui.table_get_hovered_column().column().map(usize::from);
            self.open_context_menu = true;
        }

        // ── Drag-and-drop ───────────────────────────────────────────
        if self.config.drag_drop_enabled {
            // Drag source
            let is_draggable = self
                .arena
                .get_data(node_id)
                .is_some_and(|d| d.is_draggable());
            if is_draggable
                && let Some(tooltip) = ui
                    .drag_drop_source_config(drag::DRAG_DROP_TYPE)
                    .begin_payload(node_id)
            {
                // Show node name as drag tooltip
                if let Some(data) = self.arena.get_data(node_id) {
                    self.cell_buf.clear();
                    data.cell_display_text(self.config.tree_column, &mut self.cell_buf);
                    ui.text(&self.cell_buf);
                }
                tooltip.end();
            }

            // Drop target
            if let Some(target) = ui.drag_drop_target() {
                if let Some(Ok(payload)) = target.accept_payload::<NodeId, _>(
                    drag::DRAG_DROP_TYPE,
                    dear_imgui_rs::DragDropTargetFlags::NONE,
                ) && payload.delivery
                {
                    let dragged_id = payload.data;
                    // Check if target accepts this drop
                    let accepted = self
                        .arena
                        .get_data(node_id)
                        .and_then(|target_data| {
                            self.arena
                                .get_data(dragged_id)
                                .map(|dragged_data| target_data.accepts_drop(dragged_data))
                        })
                        .unwrap_or(false);

                    if accepted {
                        // Move dragged node as child of target
                        let pos = self.arena.children(node_id).len();
                        self.arena.move_node(dragged_id, Some(node_id), pos);
                        self.arena.expand(node_id);
                        self.flat_view.mark_dirty();
                        // Record event for consumers
                        self.last_reparent = Some((dragged_id, Some(node_id), pos));
                    }
                }
                target.pop();
            }
        }

        // ── Render cells ────────────────────────────────────────────
        // Priority for selected rows: per-row selection_text_color
        // → per-row text_color (fallback). Unselected: per-row text_color only.
        let row_text_color = if is_selected {
            row_style
                .as_ref()
                .and_then(|s| s.selection_text_color)
                .or_else(|| row_style.as_ref().and_then(|s| s.text_color))
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
        let col_count = self.columns.len();
        let tree_col = self.config.tree_column.min(col_count.saturating_sub(1));

        let widget_h = unsafe { dear_imgui_rs::sys::igGetFrameHeight() };
        let vert_offset = ((row_height - widget_h) * 0.5).max(0.0);

        for col_idx in 0..col_count {
            if col_idx == 0 {
                ui.same_line_with_spacing(0.0, 0.0);
                // Apply vertical centering offset once (first column only).
                if vert_offset > 0.0 {
                    let cursor = ui.cursor_pos();
                    ui.set_cursor_pos([cursor[0], cursor[1] + vert_offset]);
                }
            } else {
                ui.table_next_column();
            }

            let _cell_id = ui.push_id(col_idx);

            // Tree column: indent + expand arrow + icon + text
            if col_idx == tree_col {
                self.render_tree_cell(ui, flat_idx, &flat_row, node_id, row_height, row_text_color);
                continue;
            }

            // Non-tree column: same as VirtualTable
            if self.edit_state.is_editing(node_id, col_idx) {
                self.render_editor_inline(ui, col_idx, node_id);
                continue;
            }

            self.render_data_cell(ui, node_id, col_idx, row_text_color);
        }
    }

    // ─── Internal: data cell (non-tree) ─────────────────────────────

    pub(super) fn render_data_cell(
        &mut self,
        ui: &Ui,
        node_id: NodeId,
        col_idx: usize,
        row_text_color: Option<[f32; 4]>,
    ) {
        let ek = editor_kind(&self.columns[col_idx].editor);

        match ek {
            EditorKind::Checkbox => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Bool(mut b) = val
                        && ui.checkbox("##cb", &mut b)
                        && let Some(data) = self.arena.get_data_mut(node_id)
                    {
                        data.set_cell_value(col_idx, &CellValue::Bool(b));
                    }
                }
            }
            EditorKind::ComboBox => {
                let val = self.arena.get_data(node_id).map(|d| d.cell_value(col_idx));
                if let Some(CellValue::Choice(mut choice)) = val {
                    let changed = {
                        let items = match &self.columns[col_idx].editor {
                            CellEditor::ComboBox { items } => items,
                            _ => {
                                self.edit_state.deactivate();
                                return;
                            }
                        };
                        ui.set_next_item_width(-1.0);
                        ui.combo_simple_string("##combo", &mut choice, items)
                    };
                    if changed && let Some(data) = self.arena.get_data_mut(node_id) {
                        data.set_cell_value(col_idx, &CellValue::Choice(choice));
                    }
                }
            }
            EditorKind::ColorEdit => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Color(mut c) = val {
                        ui.set_next_item_width(-1.0);
                        if ui
                            .color_edit4_config("##color", &mut c)
                            .flags(dear_imgui_rs::ColorEditFlags::NO_INPUTS)
                            .build()
                            && let Some(data) = self.arena.get_data_mut(node_id)
                        {
                            data.set_cell_value(col_idx, &CellValue::Color(c));
                        }
                    }
                }
            }
            EditorKind::Button => {
                let clicked = {
                    let label = match &self.columns[col_idx].editor {
                        CellEditor::Button { label } => label.as_str(),
                        _ => {
                            self.edit_state.deactivate();
                            return;
                        }
                    };
                    ui.button(label)
                };
                if clicked {
                    self.button_clicked = Some((node_id, col_idx));
                }
            }
            EditorKind::ProgressBar => {
                if let Some(data) = self.arena.get_data(node_id) {
                    let val = data.cell_value(col_idx);
                    if let CellValue::Progress(p) = val {
                        self.cell_buf.clear();
                        let _ = std::fmt::Write::write_fmt(
                            &mut self.cell_buf,
                            format_args!("{:.0}%", p * 100.0),
                        );
                        ui.progress_bar(p)
                            .size([-1.0, 0.0])
                            .overlay_text(&self.cell_buf)
                            .build();
                    }
                }
            }
            EditorKind::Custom => {
                if let Some(data) = self.arena.get_data(node_id) {
                    data.render_cell(ui, col_idx, node_id);
                }
            }
            EditorKind::Other | EditorKind::None => {
                if let Some(data) = self.arena.get_data(node_id) {
                    self.cell_buf.clear();
                    data.cell_display_text(col_idx, &mut self.cell_buf);

                    let cell_style = data.cell_style(col_idx);
                    let col_alignment = self.columns[col_idx].alignment;
                    let cell_alignment = cell_style
                        .as_ref()
                        .and_then(|s| s.alignment)
                        .unwrap_or(col_alignment);

                    if let Some(ref style) = cell_style
                        && let Some(bg) = style.bg_color
                    {
                        ui.table_set_cell_bg_color(bg, dear_imgui_rs::TableColumnRef::Current);
                    }

                    if !self.cell_buf.is_empty() {
                        let col_w = ui.current_column_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        let pad = alignment_pad(cell_alignment, col_w, text_w);
                        if pad > 0.0 {
                            let cursor = ui.cursor_pos();
                            ui.set_cursor_pos([cursor[0] + pad, cursor[1]]);
                        }
                    }

                    let color = cell_style
                        .as_ref()
                        .and_then(|s| s.text_color)
                        .or(row_text_color);

                    if let Some(c) = color {
                        ui.text_colored(c, &self.cell_buf);
                    } else {
                        ui.text(&self.cell_buf);
                    }

                    // Clip tooltip: show full text when hovered and clipped.
                    let show_clip_tooltip = self.columns[col_idx]
                        .clip_tooltip
                        .unwrap_or(self.config.table.default_clip_tooltip);
                    if show_clip_tooltip && !self.cell_buf.is_empty() && ui.is_item_hovered() {
                        let col_w = ui.current_column_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        if text_w > col_w {
                            crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
                        }
                    }
                }
            }
        }
    }
}
