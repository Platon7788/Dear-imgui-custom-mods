//! Render entry points, table setup, header, sort handling, row-stride.
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
    // ─── Render ─────────────────────────────────────────────────────
    //
    // Row-height note: virtualization (`ListClipper`) assumes a **uniform** row
    // stride derived from `TableConfig::default_row_height` (or the density
    // preset). Per-row `RowStyle::height` overrides still render, but mixing
    // heights desynchronizes the clipper's offset math (rows may overlap or the
    // last rows may be unreachable by scroll). For virtualized trees, keep row
    // heights uniform; use `RowStyle::height` only for non-scrolling trees.

    /// Render the tree, stretching to fill available height.
    /// Use this instead of `render()` when the tree is inside a fixed-size window
    /// and you want it to use all remaining vertical space (scrollable).
    ///
    /// See the row-height note above on uniform row heights and virtualization.
    pub fn render_fill(&mut self, ui: &Ui) {
        self.render_inner(ui, true);
    }

    /// Render the tree. Call once per frame inside an ImGui window.
    ///
    /// See the row-height note above on uniform row heights and virtualization.
    pub fn render(&mut self, ui: &Ui) {
        self.render_inner(ui, false);
    }

    pub(super) fn render_inner(&mut self, ui: &Ui, fill_height: bool) {
        if self.config.card_border {
            // Card wrapper: bordered child_window with rounded corners
            // (inherits global ChildRounding from the host theme).
            // `WindowPadding [0, 0]` keeps the border flush against the
            // outer table cells — no double inner gap. `VirtualTree`
            // calls `begin_table` directly (not `VirtualTable::render`),
            // so there's no risk of a double border from the nested
            // `TreeConfig::table.card_border`.
            let _wp = ui.push_style_var(dear_imgui_rs::StyleVar::WindowPadding([0.0, 0.0]));
            let card_id = format!("{}##card", self.id);
            ui.child_window(&card_id)
                .size([0.0, 0.0])
                .border(true)
                .build(ui, || self.render_inner_body(ui, fill_height));
            return;
        }
        self.render_inner_body(ui, fill_height);
    }

    pub(super) fn render_inner_body(&mut self, ui: &Ui, fill_height: bool) {
        // Apply pending toggle from previous frame.
        // After expanding a node, scroll to it so children are immediately visible.
        if let Some(id) = self.pending_toggle.take() {
            let was_expanded = self.arena.get(id).is_some_and(|s| s.expanded);
            self.toggle(id);
            if !was_expanded {
                // Node was collapsed and is now expanded → scroll to show it
                self.scroll_to_node = Some(id);
            }
        }

        // Reset per-frame outputs
        self.double_clicked_node = None;
        self.button_clicked = None;
        self.last_reparent = None;

        // Rebuild flat view if dirty
        if self.flat_view.dirty {
            self.flat_view.rebuild(&self.arena, &self.filter_state);
        }

        let col_count = self.columns.len();
        if col_count == 0 {
            return;
        }

        let mut options = self.config.table.to_table_options();
        // `TreeConfig::flat_headers` overrides `TableConfig::highlight_hovered`:
        // the column-wide tint painted by `HIGHLIGHT_HOVERED_COLUMN` would
        // defeat the per-header transparent push in `render_header` below.
        // Callers only need to flip `flat_headers` — nothing else.
        if self.config.flat_headers {
            options.flags &= !dear_imgui_rs::TableFlags::HIGHLIGHT_HOVERED_COLUMN;
        }
        // `TreeConfig::striped` paints its own zebra (`render_row` overrides
        // `RowBg1` on odd rows). Drop ImGui's automatic `ROW_BG` alternation so
        // the two striping systems don't fight — exactly one is authoritative.
        if self.config.striped {
            options.flags &= !dear_imgui_rs::TableFlags::ROW_BG;
        }
        // Always enable ScrollY for fill_height — required for outer_size to work.
        if fill_height {
            options.flags |= dear_imgui_rs::TableFlags::SCROLL_Y;
        }
        let _table = if fill_height {
            // Stretch table to fill remaining window height.
            // outer_size.y > 0 = fixed height; ImGui creates an internal child window
            // with scrollbar when content exceeds this height.
            let avail_h = ui.content_region_avail()[1].max(100.0);
            match ui.begin_table_with_sizing(&self.id, col_count, options, [0.0, avail_h], 0.0) {
                Some(t) => t,
                None => return,
            }
        } else {
            match ui.begin_table_with_flags(&self.id, col_count, options) {
                Some(t) => t,
                None => return,
            }
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

        ui.table_setup_scroll_freeze(
            self.config.table.freeze_cols.max(0) as usize,
            self.config.table.freeze_rows.max(0) as usize,
        );

        // Header
        self.render_header(ui);

        // Sort
        self.handle_sort(ui);

        // Rows via ListClipper — explicit row stride for accurate virtualization.
        // Without this, ListClipper auto-measures the first row which can be wrong
        // (header padding, variable density) → renders too few rows → empty gap.
        //
        // We pass `row_stride = row_h + 2*CellPadding.y`, not bare `row_h`: the
        // physical row height inside an ImGui table is always row_h + 2*CellPadding.y
        // (see `crate::virtual_table::row_height_to_stride` for the derivation).
        // Using bare `row_h` understates the virtual content size by
        // `row_count * 2*CellPadding.y` and makes the last rows unreachable via
        // manual scroll in tightly-sized containers (e.g. nested child_window).
        let row_count = self.flat_view.len();
        let row_h = self
            .config
            .table
            .default_row_height
            .unwrap_or_else(|| self.density_row_height());
        // Read CellPadding.y straight from the live style instead of cloning the
        // whole `Style` struct each frame.
        let cell_padding_y = unsafe { (*dear_imgui_rs::sys::igGetStyle()).CellPadding.y };
        let row_stride = crate::virtual_table::row_height_to_stride(row_h, cell_padding_y);
        let clip = ListClipper::new(row_count).items_height(row_stride);
        let tok = clip.begin(ui);

        let scroll_target = self
            .scroll_to_node
            .take()
            .and_then(|id| self.flat_view.index_of(id));

        for flat_idx in tok.iter() {
            let idx = flat_idx;
            self.render_row(ui, idx);

            // Scroll to target node
            if scroll_target == Some(idx) {
                unsafe { dear_imgui_rs::sys::igSetScrollHereY(0.5) };
            }
        }

        // Keyboard navigation
        self.handle_keyboard(ui);

        // Ctrl+C — copy selected nodes. Layout-independence is provided
        // by `crate::input::keyboard::try_inject_ctrl_alt_shortcut` at
        // the host level, so plain ImGui `is_key_pressed(Key::C)` is
        // enough — no per-widget physical-key probe needed.
        //
        // Gated on `is_window_focused` (not `is_window_hovered`) to match the
        // keyboard-navigation gate above — copy and arrow keys now react to the
        // same focus condition.
        self.copied_text = None;
        if self.config.table.copy_to_clipboard
            && !self.selected_nodes.is_empty()
            && ui.is_window_focused()
            && ui.io().key_ctrl()
            && ui.is_key_pressed(Key::C)
        {
            let text = self.build_copy_text();
            set_clipboard(&text);
            self.copied_text = Some(text);
        }
    }

    /// Density-derived row height (used when no explicit `default_row_height`
    /// or per-row override applies). Single source of truth for the row-stride
    /// estimate and per-row layout.
    pub(super) fn density_row_height(&self) -> f32 {
        unsafe {
            match self.config.table.row_density {
                RowDensity::Normal => dear_imgui_rs::sys::igGetFrameHeightWithSpacing(),
                RowDensity::Compact => dear_imgui_rs::sys::igGetFrameHeight() + 2.0,
                RowDensity::Dense => dear_imgui_rs::sys::igGetFontSize() + 2.0,
            }
        }
    }

    // ─── Internal: header ───────────────────────────────────────────

    pub(super) fn render_header(&self, ui: &Ui) {
        ui.table_next_row_with_flags(TableRowFlags::HEADERS, 0.0);
        for i in 0..self.columns.len() {
            if !ui.table_set_column_index(i) {
                continue;
            }
            let col = &self.columns[i];
            let col_w = ui.current_column_width();
            let text_w = calc_text_size(&col.name)[0];
            let pad = alignment_pad(col.header_alignment, col_w, text_w);
            if pad > 0.0 {
                let cursor = ui.cursor_pos();
                ui.set_cursor_pos([cursor[0] + pad, cursor[1]]);
            }
            // When `header_popup = false` skip the native ImGui
            // `TableHeader` call entirely — that's what attaches the
            // "Size column to fit / Size all columns to default"
            // right-click popup. Raw `ui.text()` keeps the caption
            // visible without registering a popup. (Sortable + drag-
            // reorder gestures are lost — see VirtualTable's mirror
            // comment for the same trade-off.)
            if !self.config.header_popup {
                ui.text(&col.name);
                continue;
            }
            // Tightly scope the header-flatten style so it can't bleed
            // into `Selectable` rows below (those share
            // `HeaderHovered`/`HeaderActive` for the selection
            // highlight). Tokens drop at the close-brace, before the
            // next column or row is rendered.
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
        if !self.config.table.sortable {
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
            specs.clear_dirty();

            self.sort_state.sort_all(&mut self.arena);
            self.flat_view.mark_dirty();
            self.edit_state.deactivate();
        }
    }
}
