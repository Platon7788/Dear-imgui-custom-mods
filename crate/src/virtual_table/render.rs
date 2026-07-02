//! Render entry points (ring / slice / lookup), table setup, read-only rows.
//!
//! Split out of `mod.rs` to keep files under 500 lines; extends
//! [`VirtualTable`](super::VirtualTable) via an `impl` block.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    // ─── Render (ring buffer) ───────────────────────────────────────
    //
    // Row-height note: virtualization (`ListClipper`) assumes a **uniform** row
    // stride derived from `TableConfig::default_row_height` (or the density
    // preset). Per-row `RowStyle::height` overrides still render, but mixing
    // heights desyncs the clipper's offset math (rows may overlap or the last
    // rows become unreachable by scroll). Keep row heights uniform for
    // virtualized tables; use `RowStyle::height` only for short, non-scrolling
    // tables.

    /// Render the table. Call once per frame inside an ImGui window.
    ///
    /// After this call, check [`button_clicked`](Self::button_clicked),
    /// [`double_clicked_row`](Self::double_clicked_row),
    /// [`open_context_menu`](Self::open_context_menu), etc.
    ///
    /// See the row-height note above on uniform row heights and virtualization.
    pub fn render(&mut self, ui: &Ui) {
        if self.config.card_border {
            // Card wrapper: bordered child_window with rounded corners
            // (inherits global ChildRounding from the host theme).
            // `WindowPadding [0, 0]` keeps the border flush against the
            // outer table cells — no double inner gap.
            let _wp = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([0.0, 0.0]));
            let card_id = format!("{}##card", self.id);
            ui.child_window(&card_id)
                .size([0.0, 0.0])
                .border(true)
                .build(ui, || self.render_inner(ui));
            return;
        }
        self.render_inner(ui);
    }

    pub(super) fn render_inner(&mut self, ui: &Ui) {
        self.double_clicked_row = None;
        self.button_clicked = None;
        self.copied_text = None;

        let col_count = self.columns.len();
        if col_count == 0 {
            return;
        }

        let options = self.config.to_table_options();

        let _table = match ui.begin_table_with_flags(&self.id, col_count, options) {
            Some(t) => t,
            None => return,
        };

        // Column setup
        for i in 0..col_count {
            let col = &self.columns[i];
            // dear-imgui-rs 0.14 asserts non-zero user_id when
            // `Some(_)` is passed. Default `col.user_id == 0` + first
            // column `i == 0` collapses to `Id::from(0)` → panic.
            // Bumping the fallback to `i as u32 + 1` keeps id stable
            // per column slot while staying strictly positive.
            let user_id = dear_imgui_rs::Id::from(col.user_id.max(i as u32 + 1));
            ui.table_setup_column(
                &col.name,
                col.imgui_flags(),
                Some(col.column_width()),
                Some(user_id),
            );
            if !col.visible {
                ui.table_set_column_enabled(i, false);
            }
        }

        // When the host suppressed the header row entirely, also clamp
        // freeze_rows to 0 — `table_setup_scroll_freeze` would otherwise
        // reserve dead pixels at the top of the body for a header that
        // never renders.
        let freeze_rows = if self.config.show_headers {
            self.config.freeze_rows
        } else {
            0
        };
        ui.table_setup_scroll_freeze(
            self.config.freeze_cols.max(0) as usize,
            freeze_rows.max(0) as usize,
        );

        // Header — opt-out via `TableConfig::show_headers = false` for
        // register-dump / status panes where the column captions are
        // self-explanatory and the strip just steals vertical space.
        if self.config.show_headers {
            self.render_header(ui);
        }

        // Sort
        self.handle_sort(ui);

        // Rows — explicit row stride for accurate ListClipper virtualization.
        // We pass `row_stride = row_h + 2*CellPadding.y`, not bare `row_h`, because
        // that is the physical pixel height of each row inside an ImGui table
        // (see `row_height_to_stride` for the derivation). Using bare `row_h`
        // causes ListClipper's final `SeekCursorForItem(ItemsCount)` to
        // understate the scroll range by `row_count * 2*CellPadding.y`, which
        // makes the last rows unreachable via manual scroll.
        let row_count = self.data.len();
        let row_h = self.effective_row_height(&None);
        // Read CellPadding.y from the live style instead of cloning the whole
        // `Style` struct each frame.
        let cell_padding_y = unsafe { (*dear_imgui_rs::sys::igGetStyle()).CellPadding.y };
        let row_stride = row_height_to_stride(row_h, cell_padding_y);
        let clip = ListClipper::new(row_count).items_height(row_stride);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            self.render_row(ui, row_idx);
        }

        self.handle_keyboard_nav(ui, row_count);
        self.handle_scroll(ui, row_count);

        // Ctrl+C — copy selected rows. Layout-independence is provided
        // by `crate::input::keyboard::try_inject_ctrl_alt_shortcut` at
        // the host level (see app_window / app_window), so plain
        // ImGui `is_key_pressed(Key::C)` is enough — no per-widget VK
        // probe needed.
        if self.config.copy_to_clipboard
            && !self.selected_rows.is_empty()
            && ui.is_window_focused()
            && ui.io().key_ctrl()
            && ui.is_key_pressed(Key::C)
        {
            let text = build_copy_text(&self.selected_rows, self.columns.len(), |ri, ci, buf| {
                if let Some(row) = self.data.get(ri) {
                    row.cell_display_text(ci, buf);
                }
            });
            set_clipboard(&text);
            self.copied_text = Some(text);
        }
    }

    // ─── Render (external slice) ───────────────────────────────────

    /// Render from an external slice instead of the internal `RingBuffer`.
    ///
    /// Sorting and inline editing are disabled (data is borrowed immutably).
    /// Selection, context menus, tooltips, and styling work normally.
    pub fn render_slice(&mut self, ui: &Ui, rows: &[T]) {
        self.render_external(ui, rows.len(), |idx| rows.get(idx));
    }

    // ─── Render (lookup closure) ────────────────────────────────────

    /// Render using a lookup closure instead of the internal `RingBuffer`.
    ///
    /// Avoids copying rows — the caller provides `row_count` and a closure
    /// that returns `Option<&T>` for each logical index. Ideal for HashMap
    /// lookups, merged multi-buffer indices, or any non-contiguous data.
    ///
    /// Sorting and inline editing are disabled (data is externally managed).
    /// Selection, context menus, tooltips, cell styles, and auto-scroll
    /// work identically to [`render()`](Self::render).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let sorted_keys = &monitor.sorted_pids;
    /// let map = &monitor.processes;
    /// table.render_lookup(ui, sorted_keys.len(), |idx| {
    ///     sorted_keys.get(idx).and_then(|pid| map.get(pid))
    /// });
    /// ```
    pub fn render_lookup<'a, F>(&mut self, ui: &Ui, row_count: usize, get_row: F)
    where
        F: Fn(usize) -> Option<&'a T>,
        T: 'a,
    {
        self.render_external(ui, row_count, get_row);
    }

    // ─── Internal: shared external-data render ─────────────────────

    /// Shared implementation for `render_slice` and `render_lookup`.
    /// Read-only: no sorting, no inline editing.
    pub(super) fn render_external<'a, F>(&mut self, ui: &Ui, row_count: usize, get_row: F)
    where
        F: Fn(usize) -> Option<&'a T>,
        T: 'a,
    {
        if self.config.card_border {
            // Same card wrapper as `render()` — see comment there.
            let _wp = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([0.0, 0.0]));
            let card_id = format!("{}##card_ext", self.id);
            ui.child_window(&card_id)
                .size([0.0, 0.0])
                .border(true)
                .build(ui, || self.render_external_inner(ui, row_count, get_row));
            return;
        }
        self.render_external_inner(ui, row_count, get_row);
    }

    pub(super) fn render_external_inner<'a, F>(&mut self, ui: &Ui, row_count: usize, get_row: F)
    where
        F: Fn(usize) -> Option<&'a T>,
        T: 'a,
    {
        self.double_clicked_row = None;
        self.button_clicked = None;
        self.copied_text = None;

        let col_count = self.columns.len();
        if col_count == 0 {
            return;
        }

        let options = self.config.to_table_options();

        // Quantize outer height so the last visible row is never clipped
        // mid-pixel (opt-in via `TableConfig::snap_last_row`).
        // Header row height = `FontSize + 2*CellPadding.y` (matches
        // ImGui's internal `TableGetHeaderRowHeight`, imgui_tables.cpp:3084).
        // Data row physical stride = `row_h + 2*CellPadding.y` (see
        // `row_height_to_stride`), so we compute the biggest
        // `N * row_stride + header_h` that fits in the available height.
        let row_h = self.effective_row_height(&None);
        // Read CellPadding.y from the live style instead of cloning the whole
        // `Style` struct each frame.
        let cell_padding_y = unsafe { (*dear_imgui_rs::sys::igGetStyle()).CellPadding.y };
        let row_stride = row_height_to_stride(row_h, cell_padding_y);
        let outer_size = if self.config.snap_last_row && self.config.scroll_y {
            let avail_h = ui.content_region_avail()[1];
            let header_h =
                unsafe { dear_imgui_rs::sys::igGetTextLineHeight() } + cell_padding_y * 2.0;
            [0.0, snap_outer_height(avail_h, header_h, row_stride)]
        } else {
            [0.0, 0.0]
        };

        let _table = match ui.begin_table_with_sizing(&self.id, col_count, options, outer_size, 0.0)
        {
            Some(t) => t,
            None => return,
        };

        // Column setup
        for i in 0..col_count {
            let col = &self.columns[i];
            // dear-imgui-rs 0.14 asserts non-zero user_id (see comment
            // in the body-render branch above for context).
            let user_id = dear_imgui_rs::Id::from(col.user_id.max(i as u32 + 1));
            ui.table_setup_column(
                &col.name,
                col.imgui_flags(),
                Some(col.column_width()),
                Some(user_id),
            );
            if !col.visible {
                ui.table_set_column_enabled(i, false);
            }
        }

        // Same header-suppression opt-out as in `render()` above.
        let freeze_rows = if self.config.show_headers {
            self.config.freeze_rows
        } else {
            0
        };
        ui.table_setup_scroll_freeze(
            self.config.freeze_cols.max(0) as usize,
            freeze_rows.max(0) as usize,
        );

        // Header — opt-out via `TableConfig::show_headers = false`.
        if self.config.show_headers {
            self.render_header(ui);
        }

        // No sorting — data order is caller-managed.

        // Rows (read-only path: no inline editing).
        // Use `row_stride` (= row_h + 2*CellPadding.y), not bare `row_h` — see
        // the comment in `render()` above and `row_height_to_stride` below.
        let clip = ListClipper::new(row_count).items_height(row_stride);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            let idx = row_idx;
            let row = match get_row(idx) {
                Some(r) => r,
                None => continue,
            };

            self.render_row_readonly(ui, idx, row, row_count);
        }

        self.handle_keyboard_nav(ui, row_count);
        self.handle_scroll(ui, row_count);

        // Ctrl+C — same shortcut as `render`. See the note there for why
        // a plain `is_key_pressed` is sufficient on non-Latin layouts.
        if self.config.copy_to_clipboard
            && !self.selected_rows.is_empty()
            && ui.is_window_focused()
            && ui.io().key_ctrl()
            && ui.is_key_pressed(Key::C)
        {
            let text = build_copy_text(&self.selected_rows, self.columns.len(), |ri, ci, buf| {
                if let Some(row) = get_row(ri) {
                    row.cell_display_text(ci, buf);
                }
            });
            set_clipboard(&text);
            self.copied_text = Some(text);
        }
    }

    // ─── Internal: read-only row rendering ─────────────────────────

    /// Render a single row from an external `&T` reference.
    /// Handles selection, tooltips, context menu, cell styling — everything
    /// except inline editing and always-visible editors (Checkbox, ComboBox,
    /// ColorEdit, ProgressBar, Button), which degrade to text display.
    pub(super) fn render_row_readonly(&mut self, ui: &Ui, idx: usize, row: &T, row_count: usize) {
        let row_style = row.row_style();

        let row_height = self.effective_row_height(&row_style);

        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);

        // Row background
        if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_row_bg1_color(bg);
        }

        // Selection state — O(1) via foldhash-backed HashSet
        let is_selected = self.selected_rows.contains(&idx);

        if is_selected
            && let Some(sel_bg) =
                row::resolve_selection_bg(row_style.as_ref(), self.config.selection_color)
        {
            ui.table_set_row_bg1_color(sel_bg);
        }

        let _row_id = ui.push_id(idx);

        // Selectable spanning all columns for click/selection/highlight
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
            self.handle_selection(ui, idx, row_count);

            if ui.is_mouse_double_clicked(MouseButton::Left) {
                self.double_clicked_row = Some(idx);
            }
        }

        // Tooltip
        if ui.is_item_hovered() && !row.render_tooltip(ui) {
            self.cell_buf.clear();
            row.row_tooltip(&mut self.cell_buf);
            if !self.cell_buf.is_empty() {
                crate::utils::themed_tooltip(ui, || ui.text(&self.cell_buf));
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, idx, row_count);
            self.context_row = Some(idx);
            self.context_col = ui.table_get_hovered_column().column().map(usize::from);
            self.open_context_menu = true;
        }

        // ── Render cells (read-only: text + custom only) ───────────
        // Priority when selected:
        //   per-row selection_text_color
        //   → config-wide selection_text_color
        //   → per-row text_color (legacy fallback)
        // Priority when not selected: per-row text_color only.
        let row_text_color = if is_selected {
            row::resolve_selection_text_color(row_style.as_ref(), self.config.selection_text_color)
        } else {
            row_style.as_ref().and_then(|s| s.text_color)
        };
        let col_count = self.columns.len();

        // Vertical centering
        let widget_h = unsafe { dear_imgui_rs::sys::igGetFrameHeight() };
        let vert_offset = ((row_height - widget_h) * 0.5).max(0.0);

        for col_idx in 0..col_count {
            if col_idx == 0 {
                ui.same_line_with_spacing(0.0, 0.0);
            } else {
                ui.table_next_column();
            }

            if vert_offset > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0], cursor[1] + vert_offset]);
            }

            let _cell_id = ui.push_id(col_idx);

            // Custom cell rendering (CellEditor::Custom)
            if matches!(
                editor_kind(&self.columns[col_idx].editor),
                EditorKind::Custom
            ) && row.render_cell(ui, col_idx)
            {
                continue;
            }

            // Text rendering with styling
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
            // Column can override (Some); falls back to table-level default.
            let show_clip_tooltip = self.columns[col_idx]
                .clip_tooltip
                .unwrap_or(self.config.default_clip_tooltip);
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
