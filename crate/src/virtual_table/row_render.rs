//! Editable row rendering, header, sort handling, row-height resolution.
//!
//! Split out of `mod.rs` to keep files under 500 lines; extends
//! [`VirtualTable`](super::VirtualTable) via an `impl` block.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    // ─── Internal: header ───────────────────────────────────────────

    pub(super) fn render_header(&self, ui: &Ui) {
        ui.table_next_row_with_flags(TableRowFlags::HEADERS, 0.0);
        for i in 0..self.columns.len() {
            if !ui.table_set_column_index(i) {
                continue;
            }
            let col = &self.columns[i];
            let col_w = ui.content_region_avail_width();
            let text_w = calc_text_size(&col.name)[0];
            let pad = alignment_pad(col.header_alignment, col_w, text_w);
            if pad > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + pad, cursor[1]]);
            }
            // When `header_popup = false` the caller doesn't want the
            // native ImGui "Size column to fit / Size all columns to
            // default" right-click popup — it would normally register
            // automatically inside `ui.table_header(...)`. Render the
            // caption with raw `ui.text()` instead so no header popup
            // is ever attached. Sortable indicators / drag-reorder
            // gestures are lost as a side effect; callers that need
            // those keep `header_popup = true` (the default).
            if !self.config.header_popup {
                ui.text(&col.name);
                continue;
            }
            // Tightly scope the header-flatten style so it can't bleed
            // into the selection highlight on row bodies below (which
            // reuse the same `HeaderHovered`/`HeaderActive` colors).
            // Tokens drop at the close-brace, before the next column
            // or row renders.
            if self.config.flat_headers {
                let _hdr_hover = ui.push_style_color(
                    dear_imgui_rs::StyleColor::HeaderHovered,
                    [0.0, 0.0, 0.0, 0.0],
                );
                let _hdr_active = ui.push_style_color(
                    dear_imgui_rs::StyleColor::HeaderActive,
                    [0.0, 0.0, 0.0, 0.0],
                );
                ui.table_header(&col.name);
            } else {
                ui.table_header(&col.name);
            }
        }
    }

    // ─── Internal: sort ─────────────────────────────────────────────

    pub(super) fn handle_sort(&mut self, ui: &Ui) {
        if !self.config.sortable {
            return;
        }
        if let Some(mut specs) = ui.table_get_sort_specs()
            && specs.is_dirty()
        {
            self.sort_state.specs.clear();
            for s in specs.iter() {
                self.sort_state.specs.push(SortSpec {
                    column_index: usize::from(s.column_index),
                    ascending: s.sort_direction == dear_imgui_rs::SortDirection::Ascending,
                });
            }
            specs.clear_dirty(ui);

            // Move specs out temporarily to avoid borrow conflict with self.data.
            let specs = std::mem::take(&mut self.sort_state.specs);
            self.data.sort_by(|a, b| {
                for spec in &specs {
                    let ord = a.compare(b, spec.column_index);
                    let ord = if spec.ascending { ord } else { ord.reverse() };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
            self.sort_state.specs = specs;

            self.edit_state.deactivate();
            self.selected_rows.clear();
            self.selection_anchor = None;
        }
    }

    // ─── Internal: row rendering ────────────────────────────────────

    pub(super) fn render_row(&mut self, ui: &Ui, idx: usize) {
        // Extract row-level data upfront via scoped borrow (no raw pointers held across mut).
        let row_style = match self.data.get(idx) {
            Some(r) => r.row_style(),
            None => return,
        };

        let row_height = self.effective_row_height(&row_style);

        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);

        // Row background (custom row_style has lower priority than selection color)
        if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_row_bg1_color(bg);
        }

        // Selection state — O(1) via foldhash-backed HashSet
        let is_selected = self.selected_rows.contains(&idx);

        // Paint the whole row with the selection color so it is clearly visible
        // even when many rows are selected. Applied after row_style so selection
        // always wins over custom row backgrounds. Per-row override takes
        // precedence over the table-wide default.
        if is_selected
            && let Some(sel_bg) =
                row::resolve_selection_bg(row_style.as_ref(), self.config.selection_color)
        {
            ui.table_set_row_bg1_color(sel_bg);
        }

        // Push row-level ID scope (covers selectable + ALL cells)
        let _row_id = ui.push_id(idx);

        // First column: selectable spanning all columns for click handling + highlight
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
            self.handle_selection(ui, idx, self.data.len());

            // Double-click always tracked (user may need it for custom logic)
            if ui.is_mouse_double_clicked(MouseButton::Left) {
                self.double_clicked_row = Some(idx);
            }

            // Edit trigger: activate editor on the hovered column
            let activate_edit = match self.config.edit_trigger {
                EditTrigger::DoubleClick => ui.is_mouse_double_clicked(MouseButton::Left),
                EditTrigger::SingleClick => true, // selectable was clicked
                _ => false,
            };
            if activate_edit && let Some(hovered_col) = ui.table_get_hovered_column().column() {
                self.try_activate_edit(idx, hovered_col.get());
            }
        }

        // F2 key triggers editor on selected row's first editable column
        if is_selected
            && self.config.edit_trigger == EditTrigger::F2Key
            && ui.is_key_pressed(Key::F2)
        {
            for c in 0..self.columns.len() {
                if !matches!(
                    editor_kind(&self.columns[c].editor),
                    EditorKind::None
                        | EditorKind::Checkbox
                        | EditorKind::ComboBox
                        | EditorKind::Button
                        | EditorKind::ProgressBar
                        | EditorKind::ColorEdit
                        | EditorKind::Custom
                ) {
                    self.try_activate_edit(idx, c);
                    break;
                }
            }
        }

        // Tooltip
        if ui.is_item_hovered()
            && let Some(row) = self.data.get(idx)
            && !row.render_tooltip(ui)
        {
            self.cell_buf.clear();
            row.row_tooltip(&mut self.cell_buf);
            if !self.cell_buf.is_empty() {
                crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, idx, self.data.len());
            self.context_row = Some(idx);
            self.context_col = ui.table_get_hovered_column().column().map(usize::from);
            self.open_context_menu = true;
        }

        // ── Render cells ────────────────────────────────────────────
        // Selected priority:
        //   per-row selection_text_color
        //   → config-wide selection_text_color
        //   → per-row text_color (legacy fallback)
        // Not selected: per-row text_color only.
        let row_text_color = if is_selected {
            row::resolve_selection_text_color(row_style.as_ref(), self.config.selection_text_color)
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
        let col_count = self.columns.len();

        // Vertical centering offset: (row_height - widget_height) / 2
        let widget_h = unsafe { dear_imgui_rs::sys::igGetFrameHeight() };
        let vert_offset = ((row_height - widget_h) * 0.5).max(0.0);

        for col_idx in 0..col_count {
            if col_idx == 0 {
                ui.same_line_with_spacing(0.0, 0.0);
            } else {
                ui.table_next_column();
            }

            // Apply vertical centering
            if vert_offset > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0], cursor[1] + vert_offset]);
            }

            let _cell_id = ui.push_id(col_idx);

            // Editing this cell?
            if self.edit_state.is_editing(idx, col_idx) {
                self.render_editor_inline(ui, idx, col_idx);
                continue;
            }

            // Determine what to render based on editor type
            let editor_kind = editor_kind(&self.columns[col_idx].editor);

            match editor_kind {
                EditorKind::Checkbox => {
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Bool(mut b) = val
                        && ui.checkbox("##cb", &mut b)
                        && let Some(row) = self.data.get_mut(idx)
                    {
                        row.set_cell_value(col_idx, &CellValue::Bool(b));
                    }
                }
                EditorKind::ComboBox => {
                    let val = self.data.get(idx).map(|r| r.cell_value(col_idx));
                    if let Some(CellValue::Choice(mut choice)) = val {
                        let changed = {
                            let items = match &self.columns[col_idx].editor {
                                CellEditor::ComboBox { items } => items,
                                // Unreachable: editor_kind already classified this
                                // column as ComboBox. Skip the cell rather than
                                // aborting the whole row if that ever changes.
                                _ => continue,
                            };
                            ui.set_next_item_width(-1.0);
                            ui.combo_simple_string("##combo", &mut choice, items)
                        };
                        if changed && let Some(row) = self.data.get_mut(idx) {
                            row.set_cell_value(col_idx, &CellValue::Choice(choice));
                        }
                    }
                }
                EditorKind::ColorEdit => {
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Color(mut c) = val
                    {
                        ui.set_next_item_width(-1.0);
                        if ui
                            .color_edit4_config("##color", &mut c)
                            .flags(dear_imgui_rs::ColorEditFlags::NO_INPUTS)
                            .build()
                            && let Some(row) = self.data.get_mut(idx)
                        {
                            row.set_cell_value(col_idx, &CellValue::Color(c));
                        }
                    }
                }
                EditorKind::Button => {
                    let clicked = {
                        let label = match &self.columns[col_idx].editor {
                            CellEditor::Button { label } => label.as_str(),
                            _ => continue, // unreachable; skip cell, don't abort row
                        };
                        ui.button(label)
                    };
                    if clicked {
                        self.button_clicked = Some((idx, col_idx));
                    }
                }
                EditorKind::ProgressBar => {
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Progress(p) = val
                    {
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
                EditorKind::Custom => {
                    if let Some(row) = self.data.get(idx) {
                        row.render_cell(ui, col_idx);
                    }
                }
                EditorKind::Other | EditorKind::None => {
                    let Some(row) = self.data.get(idx) else {
                        continue;
                    };
                    self.cell_buf.clear();
                    row.cell_display_text(col_idx, &mut self.cell_buf);

                    let cell_style = row.cell_style(col_idx);
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
                        let col_w = ui.content_region_avail_width();
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
                        .unwrap_or(self.config.default_clip_tooltip);
                    if show_clip_tooltip && !self.cell_buf.is_empty() && ui.is_item_hovered() {
                        let col_w = ui.content_region_avail_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        if text_w > col_w {
                            crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
                        }
                    }
                }
            }
        }
        // _row_id is dropped here, covering all cells
    }

    /// Compute the effective row height: custom style > config default > auto (by density).
    pub(super) fn effective_row_height(&self, row_style: &Option<row::RowStyle>) -> f32 {
        let auto_h = unsafe {
            match self.config.row_density {
                config::RowDensity::Normal => dear_imgui_rs::sys::igGetFrameHeightWithSpacing(),
                config::RowDensity::Compact => dear_imgui_rs::sys::igGetFrameHeight() + 2.0,
                config::RowDensity::Dense => dear_imgui_rs::sys::igGetFontSize() + 2.0,
            }
        };
        row_style
            .as_ref()
            .and_then(|s| s.height)
            .or(self.config.default_row_height)
            .unwrap_or(auto_h)
    }
}
