//! # VirtualTable\<T\> v2
//!
//! Full-featured virtualized table component for Dear ImGui, inspired by
//! DevExpress VirtualTreeList and Delphi VirtualStringTree.
//! Built on Dear ImGui's native Table API (v1.92.6 docking branch).
//!
//! ## Key Features
//!
//! - **Column management**: resize, reorder, hide/show, freeze, per-column alignment
//! - **Sorting**: single and multi-column, ascending/descending, via `VirtualTableRow::compare()`
//! - **Inline editing**: TextInput, Checkbox, ComboBox, SliderInt/Float,
//!   SpinInt/Float, ColorEdit, ProgressBar, Button, Custom
//! - **Edit triggers**: DoubleClick, SingleClick, F2 key, or disabled
//! - **Styling**: per-row background/text color/height, per-cell bg/text/alignment
//! - **Selection**: None, Single, Multi (Ctrl+Click toggle, Shift+Click range)
//! - **Row density**: Normal (widget-friendly), Compact, Dense (text-only)
//! - **Virtualization**: Dear ImGui ListClipper — handles 100,000+ rows at 60 FPS
//! - **Data storage**: built-in `RingBuffer<T>` (fixed-capacity, O(1) push, FIFO eviction)
//! - **Context menus**: right-click with row + column tracking
//! - **Auto-scroll**: follow newest entries (disables on manual scroll-up)
//! - **Tooltips**: plain-text or custom ImGui-rendered per-row
//! - **Custom rendering**: `render_cell()` / `render_editor()` for arbitrary cell content
//!
//! ## Architecture
//!
//! ```text
//! virtual_table/
//! ├── mod.rs          VirtualTable<T> widget + render logic
//! ├── column.rs       ColumnDef, ColumnSizing, CellAlignment, CellEditor
//! ├── row.rs          VirtualTableRow trait, CellValue, CellStyle, RowStyle
//! ├── config.rs       TableConfig, SelectionMode, EditTrigger, RowDensity, etc.
//! ├── edit.rs         EditState (inline editing state machine)
//! ├── sort.rs         SortState / SortSpec (Dear ImGui sort specs wrapper)
//! └── ring_buffer.rs  RingBuffer<T> — fixed-capacity circular buffer with sort
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::virtual_table::*;
//! use std::cmp::Ordering;
//!
//! // 1. Define your row type
//! struct MyRow { name: String, score: f64 }
//!
//! impl VirtualTableRow for MyRow {
//!     fn cell_value(&self, col: usize) -> CellValue {
//!         match col {
//!             0 => CellValue::Text(self.name.clone()),
//!             1 => CellValue::Float(self.score),
//!             _ => CellValue::Text(String::new()),
//!         }
//!     }
//!     fn set_cell_value(&mut self, col: usize, value: &CellValue) {
//!         match col {
//!             0 => if let CellValue::Text(s) = value { self.name = s.clone(); }
//!             1 => if let CellValue::Float(v) = value { self.score = *v; }
//!             _ => {}
//!         }
//!     }
//!     fn compare(&self, other: &Self, col: usize) -> Ordering {
//!         match col {
//!             0 => self.name.cmp(&other.name),
//!             1 => self.score.partial_cmp(&other.score).unwrap_or(Ordering::Equal),
//!             _ => Ordering::Equal,
//!         }
//!     }
//! }
//!
//! // 2. Define columns
//! let columns = vec![
//!     ColumnDef::new("Name").stretch(1.0).editor(CellEditor::TextInput),
//!     ColumnDef::new("Score").fixed(100.0).align(CellAlignment::Right)
//!         .editor(CellEditor::SpinFloat { step: 0.1, step_fast: 1.0 }),
//! ];
//!
//! // 3. Create the table
//! let config = TableConfig::default();
//! let mut table = VirtualTable::new("my_table", columns, 10_000, config);
//!
//! // 4. Push data
//! table.push(MyRow { name: "Alice".into(), score: 95.5 });
//! table.push(MyRow { name: "Bob".into(), score: 87.3 });
//!
//! // 5. Render each frame
//! // table.render(&ui);
//! ```
//!
//! ## Cell Editors
//!
//! | Editor          | CellValue     | Widget                    | Notes                    |
//! |-----------------|---------------|---------------------------|--------------------------|
//! | `None`          | `Text`        | Plain text (read-only)    | Default                  |
//! | `TextInput`     | `Text`        | `input_text`              | Enter commits, Esc cancels |
//! | `Checkbox`      | `Bool`        | Checkbox                  | Always visible, instant  |
//! | `ComboBox`      | `Choice(idx)` | Dropdown                  | Always visible           |
//! | `SliderInt`     | `Int(i64)`    | Horizontal slider         | Commit on release        |
//! | `SliderFloat`   | `Float(f64)`  | Horizontal slider         | Commit on release        |
//! | `SpinInt`       | `Int(i64)`    | `input_int` with +/- step | Enter/focus-loss commits |
//! | `SpinFloat`     | `Float(f64)`  | `input_float` with step   | Enter/focus-loss commits |
//! | `ProgressBar`   | `Progress`    | Progress bar              | Read-only visualization  |
//! | `ColorEdit`     | `Color`       | Color picker swatch       | Always visible           |
//! | `Button{label}` | `Custom`      | Clickable button          | Check `button_clicked`   |
//! | `Custom`        | `Custom`      | User-defined              | `render_cell()`/`render_editor()` |
//!
//! ## Styling
//!
//! Override `row_style()` to set per-row background, text color, or custom height.
//! Override `cell_style()` to set per-cell background, text color, or alignment.
//! Cell style takes priority over row style.
//!
//! ## Configuration
//!
//! See [`TableConfig`] for all options. Key defaults:
//! - `resizable: true`, `sortable: true`, `hideable: true`
//! - `selection_mode: Single`, `edit_trigger: DoubleClick`
//! - `row_density: Normal`, `borders: Full`, `row_bg: true`
//! - `freeze_rows: 1` (frozen header), `scroll_y: true`
//!
//! ## Performance
//!
//! - **ListClipper**: only visible rows are rendered (O(visible), not O(total))
//! - **RingBuffer**: O(1) push, O(1) indexed access, zero allocation after init
//! - **No per-frame clones**: ComboBox items and Button labels use pointer borrows
//! - **Vertical centering**: computed once per row, not per cell
//! - **Sort**: in-place via `rotate_left` linearization (zero extra allocation)

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod column;
pub mod config;
mod edit;
pub mod ring_buffer;
pub mod row;
mod sort;

pub use column::{CellAlignment, CellEditor, ColumnDef, ColumnSizing};
pub use config::{BorderStyle, EditTrigger, RowDensity, SelectionMode, SizingPolicy, TableConfig};
pub use ring_buffer::{MAX_TABLE_ROWS, RingBuffer};
pub use row::{CellStyle, CellValue, RowStyle, VirtualTableRow};

use crate::utils::clipboard::{c_key_down_physical, set_clipboard};
use crate::utils::text::calc_text_size;
use column::{EditorKind, alignment_pad, editor_kind};
use dear_imgui_rs::{
    Key, ListClipper, MouseButton, SelectableFlags, TableBgTarget, TableRowFlags, Ui,
};
use edit::EditState;
use sort::{SortSpec, SortState};

use std::collections::HashSet;

/// Fast hash set for row indices. Uses `foldhash` — a modern, high-quality
/// hash optimized for integer keys. O(1) `contains()` vs O(n) for `Vec`.
type IndexSet = HashSet<usize, foldhash::fast::FixedState>;

// ─── VirtualTable ───────────────────────────────────────────────────────────

/// Virtualized table widget with inline editing, sorting, selection, and styling.
///
/// Generic over `T: VirtualTableRow` — your row data type.
/// Data is stored in a [`RingBuffer<T>`] with configurable capacity.
///
/// # Per-frame output fields
///
/// After each `render()` call, check these public fields:
/// - `double_clicked_row` — row index if double-clicked this frame
/// - `button_clicked` — `(row, col)` if a `CellEditor::Button` was clicked
/// - `context_row` / `context_col` — row/column of the right-click
/// - `open_context_menu` — `true` when user right-clicked (reset it after handling)
pub struct VirtualTable<T: VirtualTableRow> {
    id: String,
    columns: Vec<ColumnDef>,
    /// Table configuration. All fields are `pub` — modify freely between frames.
    pub config: TableConfig,
    data: RingBuffer<T>,

    // Selection
    selected_rows: IndexSet,
    /// Anchor row for Shift+Click range selection (last explicitly clicked row).
    selection_anchor: Option<usize>,
    /// Set to `Some(idx)` when a row is double-clicked. Reset each frame.
    pub double_clicked_row: Option<usize>,
    /// Row index of the last right-click (for context menu logic).
    pub context_row: Option<usize>,
    /// Column index of the last right-click (for per-column context menus).
    pub context_col: Option<usize>,
    /// `true` when the user right-clicked a row. Set to `false` after handling.
    pub open_context_menu: bool,

    /// Set to `Some((row, col))` when a `CellEditor::Button` is clicked. Reset each frame.
    pub button_clicked: Option<(usize, usize)>,

    /// Set to `Some(text)` when **Ctrl+C** copies selected rows this frame
    /// (requires [`TableConfig::copy_to_clipboard`] = `true`). Reset each frame.
    pub copied_text: Option<String>,

    /// Tracks whether the physical C key was held last frame (for edge detection).
    /// Needed for layout-independent Ctrl+C: we read the Windows scancode directly.
    c_key_prev: bool,

    /// Row index to scroll to on the next frame. Set via `scroll_to_row()`.
    pending_scroll_to: Option<usize>,

    edit_state: EditState,
    sort_state: SortState,
    cell_buf: String,
}

impl<T: VirtualTableRow> VirtualTable<T> {
    /// Create a new table with the given columns and ring buffer capacity.
    ///
    /// - `id` — unique ImGui identifier (e.g. `"##my_table"`)
    /// - `columns` — column definitions (use [`ColumnDef::new()`] builder)
    /// - `capacity` — maximum rows in the ring buffer (oldest evicted when full)
    /// - `config` — table behavior settings (see [`TableConfig`])
    pub fn new(
        id: impl Into<String>,
        columns: Vec<ColumnDef>,
        capacity: usize,
        config: TableConfig,
    ) -> Self {
        Self {
            id: id.into(),
            columns,
            config,
            data: RingBuffer::new(capacity),
            selected_rows: IndexSet::default(),
            selection_anchor: None,
            double_clicked_row: None,
            context_row: None,
            context_col: None,
            open_context_menu: false,
            button_clicked: None,
            copied_text: None,
            c_key_prev: false,
            pending_scroll_to: None,
            edit_state: EditState::default(),
            sort_state: SortState::default(),
            cell_buf: String::with_capacity(256),
        }
    }

    // ─── Data access ────────────────────────────────────────────────

    /// Append a row. O(1). If at capacity, the oldest row is evicted.
    #[inline]
    pub fn push(&mut self, item: T) {
        self.data.push(item);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }

    /// Remove all rows and reset selection/editing state.
    pub fn clear(&mut self) {
        self.data.clear();
        self.selected_rows.clear();
        self.selection_anchor = None;
        self.edit_state.deactivate();
    }

    /// Remove the row at logical index. O(n). Returns the removed item.
    /// Automatically adjusts selection indices and deactivates any active editor.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.edit_state.deactivate();
        // Remove the deleted row and shift indices above it down by 1.
        // In-place: collect indices that need decrement, then rebuild.
        // This avoids allocating a second IndexSet.
        self.selected_rows.remove(&index);
        // Drain + reinsert reuses the HashSet's allocated capacity (no reallocation).
        let indices: Vec<usize> = self.selected_rows.drain().collect();
        for r in indices {
            self.selected_rows.insert(if r > index { r - 1 } else { r });
        }
        // Adjust anchor
        if let Some(a) = self.selection_anchor {
            if a == index {
                self.selection_anchor = None;
            } else if a > index {
                self.selection_anchor = Some(a - 1);
            }
        }
        self.data.remove(index)
    }

    pub fn data(&self) -> &RingBuffer<T> {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut RingBuffer<T> {
        &mut self.data
    }

    // ─── Column access ──────────────────────────────────────────────

    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    pub fn columns_mut(&mut self) -> &mut [ColumnDef] {
        &mut self.columns
    }

    // ─── Selection ──────────────────────────────────────────────────

    /// Returns an iterator over selected row indices.
    pub fn selected_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.selected_rows.iter().copied()
    }

    /// Number of selected rows.
    pub fn selected_count(&self) -> usize {
        self.selected_rows.len()
    }

    /// Returns `true` if the given row index is selected.
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selected_rows.contains(&idx)
    }

    /// Returns the anchor (last explicitly clicked) row, or any selected row.
    /// For `Single` mode, returns the one selected row.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection_anchor
            .filter(|a| self.selected_rows.contains(a))
            .or_else(|| self.selected_rows.iter().next().copied())
    }

    pub fn clear_selection(&mut self) {
        self.selected_rows.clear();
        self.selection_anchor = None;
    }

    /// Programmatically select a single row (clears previous selection) and
    /// scroll to it on the next frame.
    pub fn select_row(&mut self, idx: usize) {
        self.selected_rows.clear();
        self.selected_rows.insert(idx);
        self.selection_anchor = Some(idx);
        self.pending_scroll_to = Some(idx);
    }

    /// Request scroll to the given row index on the next frame.
    pub fn scroll_to_row(&mut self, idx: usize) {
        self.pending_scroll_to = Some(idx);
    }

    // ─── Editing ────────────────────────────────────────────────────

    pub fn is_editing(&self) -> bool {
        self.edit_state.active
    }

    pub fn cancel_edit(&mut self) {
        self.edit_state.deactivate();
    }

    // ─── Export / Import ────────────────────────────────────────────

    /// Export selected rows (or all if none selected) to a `FlatExportData`.
    ///
    /// Requires `T: Exportable`. Only available when export is conceptually enabled.
    pub fn export_data(
        &self,
        scope: crate::utils::export::ExportScope,
    ) -> crate::utils::export::FlatExportData
    where
        T: crate::utils::export::Exportable,
    {
        let names = T::field_names();
        let columns: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let mut data = crate::utils::export::FlatExportData::new(columns);

        match scope {
            crate::utils::export::ExportScope::Selected => {
                for idx in self.selected_rows() {
                    if let Some(row) = self.data.get(idx) {
                        let vals: Vec<crate::utils::export::FieldValue> =
                            (0..T::field_count()).map(|c| row.field_value(c)).collect();
                        data.add_row(vals);
                    }
                }
            }
            crate::utils::export::ExportScope::All => {
                for row in self.data.iter() {
                    let vals: Vec<crate::utils::export::FieldValue> =
                        (0..T::field_count()).map(|c| row.field_value(c)).collect();
                    data.add_row(vals);
                }
            }
        }

        // If scope was Selected but nothing was selected, export all.
        if data.rows.is_empty() && scope == crate::utils::export::ExportScope::Selected {
            return self.export_data(crate::utils::export::ExportScope::All);
        }

        data
    }

    /// Export to string in the given format.
    pub fn export_string(
        &self,
        scope: crate::utils::export::ExportScope,
        format: crate::utils::export::ExportFormat,
    ) -> String
    where
        T: crate::utils::export::Exportable,
    {
        let data = self.export_data(scope);
        crate::utils::export::format_flat(&data, format)
    }

    /// Export to file. Format auto-detected from extension.
    pub fn export_to_file(
        &self,
        scope: crate::utils::export::ExportScope,
        path: &std::path::Path,
    ) -> std::io::Result<()>
    where
        T: crate::utils::export::Exportable,
    {
        let data = self.export_data(scope);
        crate::utils::export::export_flat_to_file(&data, path, None)
    }

    /// Import rows from file, appending to the table.
    pub fn import_from_file(&mut self, path: &std::path::Path) -> Option<usize>
    where
        T: crate::utils::export::Importable,
    {
        let data = crate::utils::export::import_flat_from_file(path)?;
        let mut count = 0;
        for row_vals in &data.rows {
            let fields: Vec<(&str, crate::utils::export::FieldValue)> = data
                .columns
                .iter()
                .zip(row_vals.iter())
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            if let Some(item) = T::from_fields(&fields) {
                self.push(item);
                count += 1;
            }
        }
        Some(count)
    }

    // ─── Render (ring buffer) ───────────────────────────────────────

    /// Render the table. Call once per frame inside an ImGui window.
    ///
    /// After this call, check [`button_clicked`](Self::button_clicked),
    /// [`double_clicked_row`](Self::double_clicked_row),
    /// [`open_context_menu`](Self::open_context_menu), etc.
    pub fn render(&mut self, ui: &Ui) {
        self.double_clicked_row = None;
        self.button_clicked = None;
        self.copied_text = None;

        let col_count = self.columns.len();
        if col_count == 0 {
            return;
        }

        let flags = self.config.to_table_flags();

        let _table = match ui.begin_table_with_flags(&self.id, col_count, flags) {
            Some(t) => t,
            None => return,
        };

        // Column setup
        for i in 0..col_count {
            let col = &self.columns[i];
            ui.table_setup_column(
                &col.name,
                col.imgui_flags(),
                col.init_width_or_weight(),
                col.user_id.max(i as u32),
            );
            if !col.visible {
                ui.table_set_column_enabled(i as i32, false);
            }
        }

        ui.table_setup_scroll_freeze(self.config.freeze_cols, self.config.freeze_rows);

        // Header
        self.render_header(ui);

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
        let cell_padding_y = ui.clone_style().cell_padding()[1];
        let row_stride = row_height_to_stride(row_h, cell_padding_y);
        let clip = ListClipper::new(row_count as i32).items_height(row_stride);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            self.render_row(ui, row_idx as usize);
        }

        self.handle_keyboard_nav(ui, row_count);
        self.handle_scroll(ui, row_count);

        // Ctrl+C — copy selected rows.
        // Uses physical key detection (layout-independent): on Windows we read
        // VK_C (0x43) directly via GetAsyncKeyState so Russian/any layout works.
        let c_now = c_key_down_physical();
        let c_just = c_now && !self.c_key_prev;
        self.c_key_prev = c_now;
        if self.config.copy_to_clipboard
            && !self.selected_rows.is_empty()
            && ui.is_window_hovered()
            && ui.io().key_ctrl()
            && (c_just || (!c_now && ui.is_key_pressed(Key::C)))
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
    fn render_external<'a, F>(&mut self, ui: &Ui, row_count: usize, get_row: F)
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

        let flags = self.config.to_table_flags();

        // Quantize outer height so the last visible row is never clipped
        // mid-pixel (opt-in via `TableConfig::snap_last_row`).
        // Header row height = `FontSize + 2*CellPadding.y` (matches
        // ImGui's internal `TableGetHeaderRowHeight`, imgui_tables.cpp:3084).
        // Data row physical stride = `row_h + 2*CellPadding.y` (see
        // `row_height_to_stride`), so we compute the biggest
        // `N * row_stride + header_h` that fits in the available height.
        let row_h = self.effective_row_height(&None);
        let cell_padding_y = ui.clone_style().cell_padding()[1];
        let row_stride = row_height_to_stride(row_h, cell_padding_y);
        let outer_size = if self.config.snap_last_row && self.config.scroll_y {
            let avail_h = ui.content_region_avail()[1];
            let header_h =
                unsafe { dear_imgui_rs::sys::igGetTextLineHeight() } + cell_padding_y * 2.0;
            [0.0, snap_outer_height(avail_h, header_h, row_stride)]
        } else {
            [0.0, 0.0]
        };

        let _table = match ui.begin_table_with_sizing(&self.id, col_count, flags, outer_size, 0.0) {
            Some(t) => t,
            None => return,
        };

        // Column setup
        for i in 0..col_count {
            let col = &self.columns[i];
            ui.table_setup_column(
                &col.name,
                col.imgui_flags(),
                col.init_width_or_weight(),
                col.user_id.max(i as u32),
            );
            if !col.visible {
                ui.table_set_column_enabled(i as i32, false);
            }
        }

        ui.table_setup_scroll_freeze(self.config.freeze_cols, self.config.freeze_rows);

        // Header
        self.render_header(ui);

        // No sorting — data order is caller-managed.

        // Rows (read-only path: no inline editing).
        // Use `row_stride` (= row_h + 2*CellPadding.y), not bare `row_h` — see
        // the comment in `render()` above and `row_height_to_stride` below.
        let clip = ListClipper::new(row_count as i32).items_height(row_stride);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            let idx = row_idx as usize;
            let row = match get_row(idx) {
                Some(r) => r,
                None => continue,
            };

            self.render_row_readonly(ui, idx, row);
        }

        self.handle_keyboard_nav(ui, row_count);
        self.handle_scroll(ui, row_count);

        // Ctrl+C — copy selected rows (layout-independent: physical key position)
        if self.config.copy_to_clipboard
            && !self.selected_rows.is_empty()
            && ui.is_window_hovered()
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
    fn render_row_readonly(&mut self, ui: &Ui, idx: usize, row: &T) {
        let row_style = row.row_style();

        let row_height = self.effective_row_height(&row_style);

        ui.table_next_row_with_flags(TableRowFlags::NONE, row_height);

        // Row background
        if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_bg_color(TableBgTarget::RowBg1, bg, -1);
        }

        // Selection state — O(1) via foldhash-backed HashSet
        let is_selected = self.selected_rows.contains(&idx);

        if is_selected {
            // Per-row override wins; otherwise fall back to the table-wide
            // default from `TableConfig::selection_color`.
            let sel_bg = row_style
                .as_ref()
                .and_then(|s| s.selection_color)
                .unwrap_or(self.config.selection_color);
            if sel_bg[3] > 0.0 {
                ui.table_set_bg_color(TableBgTarget::RowBg1, sel_bg, -1);
            }
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
            self.handle_selection(ui, idx);

            if ui.is_mouse_double_clicked(MouseButton::Left) {
                self.double_clicked_row = Some(idx);
            }
        }

        // Tooltip
        if ui.is_item_hovered() && !row.render_tooltip(ui) {
            self.cell_buf.clear();
            row.row_tooltip(&mut self.cell_buf);
            if !self.cell_buf.is_empty() {
                ui.tooltip_text(&self.cell_buf);
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, idx);
            self.context_row = Some(idx);
            let hovered = ui.table_get_hovered_column();
            self.context_col = if hovered >= 0 {
                Some(hovered as usize)
            } else {
                None
            };
            self.open_context_menu = true;
        }

        // ── Render cells (read-only: text + custom only) ───────────
        // Priority when selected:
        //   per-row selection_text_color
        //   → config-wide selection_text_color
        //   → per-row text_color (legacy fallback)
        // Priority when not selected: per-row text_color only.
        let row_text_color = if is_selected {
            row_style
                .as_ref()
                .and_then(|s| s.selection_text_color)
                .or(self.config.selection_text_color)
                .or_else(|| row_style.as_ref().and_then(|s| s.text_color))
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
                ui.table_set_bg_color(TableBgTarget::CellBg, bg, -1);
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
            if show_clip_tooltip
                && !self.cell_buf.is_empty()
                && ui.is_item_hovered()
            {
                let col_w = ui.current_column_width();
                let text_w = calc_text_size(&self.cell_buf)[0];
                if text_w > col_w {
                    ui.tooltip_text(&self.cell_buf);
                }
            }
        }
    }

    // ─── Internal: header ───────────────────────────────────────────

    fn render_header(&self, ui: &Ui) {
        ui.table_next_row_with_flags(TableRowFlags::HEADERS, 0.0);
        for i in 0..self.columns.len() {
            if !ui.table_set_column_index(i as i32) {
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

    fn handle_sort(&mut self, ui: &Ui) {
        if !self.config.sortable {
            return;
        }
        if let Some(mut specs) = ui.table_get_sort_specs()
            && specs.is_dirty()
        {
            self.sort_state.specs.clear();
            for s in specs.iter() {
                self.sort_state.specs.push(SortSpec {
                    column_index: s.column_index as usize,
                    ascending: s.sort_direction == dear_imgui_rs::SortDirection::Ascending,
                });
            }
            specs.clear_dirty();

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

    fn render_row(&mut self, ui: &Ui, idx: usize) {
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
            ui.table_set_bg_color(TableBgTarget::RowBg1, bg, -1);
        }

        // Selection state — O(1) via foldhash-backed HashSet
        let is_selected = self.selected_rows.contains(&idx);

        // Paint the whole row with the selection color so it is clearly visible
        // even when many rows are selected. Applied after row_style so selection
        // always wins over custom row backgrounds. Per-row override takes
        // precedence over the table-wide default.
        if is_selected {
            let sel_bg = row_style
                .as_ref()
                .and_then(|s| s.selection_color)
                .unwrap_or(self.config.selection_color);
            if sel_bg[3] > 0.0 {
                ui.table_set_bg_color(TableBgTarget::RowBg1, sel_bg, -1);
            }
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
            self.handle_selection(ui, idx);

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
            if activate_edit {
                let hovered_col = ui.table_get_hovered_column();
                if hovered_col >= 0 {
                    self.try_activate_edit(idx, hovered_col as usize);
                }
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
                ui.tooltip_text(&self.cell_buf);
            }
        }

        // Context menu
        if ui.is_item_hovered() && ui.is_mouse_clicked(MouseButton::Right) {
            self.handle_selection(ui, idx);
            self.context_row = Some(idx);
            let hovered = ui.table_get_hovered_column();
            self.context_col = if hovered >= 0 {
                Some(hovered as usize)
            } else {
                None
            };
            self.open_context_menu = true;
        }

        // ── Render cells ────────────────────────────────────────────
        // Selected priority:
        //   per-row selection_text_color
        //   → config-wide selection_text_color
        //   → per-row text_color (legacy fallback)
        // Not selected: per-row text_color only.
        let row_text_color = if is_selected {
            row_style
                .as_ref()
                .and_then(|s| s.selection_text_color)
                .or(self.config.selection_text_color)
                .or_else(|| row_style.as_ref().and_then(|s| s.text_color))
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
                                _ => {
                                    self.edit_state.deactivate();
                                    return;
                                }
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
                            _ => {
                                self.edit_state.deactivate();
                                return;
                            }
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
                        ui.table_set_bg_color(TableBgTarget::CellBg, bg, -1);
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
                        .unwrap_or(self.config.default_clip_tooltip);
                    if show_clip_tooltip
                        && !self.cell_buf.is_empty()
                        && ui.is_item_hovered()
                    {
                        let col_w = ui.current_column_width();
                        let text_w = calc_text_size(&self.cell_buf)[0];
                        if text_w > col_w {
                            ui.tooltip_text(&self.cell_buf);
                        }
                    }
                }
            }
        }
        // _row_id is dropped here, covering all cells
    }

    // ─── Internal: inline editor ────────────────────────────────────

    fn try_activate_edit(&mut self, row_idx: usize, col_idx: usize) {
        if col_idx >= self.columns.len() {
            return;
        }
        if matches!(
            editor_kind(&self.columns[col_idx].editor),
            EditorKind::None
                | EditorKind::Checkbox
                | EditorKind::ComboBox
                | EditorKind::Button
                | EditorKind::ProgressBar
                | EditorKind::ColorEdit
        ) {
            return;
        }

        if let Some(row) = self.data.get(row_idx) {
            let value = row.cell_value(col_idx);
            self.edit_state.activate(row_idx, col_idx, &value);
        }
    }

    fn render_editor_inline(&mut self, ui: &Ui, idx: usize, col_idx: usize) {
        let mut commit = false;
        let mut cancel = false;

        ui.set_next_item_width(-1.0);

        // Clone the editor config to avoid borrow conflict with self.edit_state/self.data.
        let editor_snapshot = self.columns[col_idx].editor.clone();
        let first_frame = self.edit_state.just_activated;
        if first_frame {
            self.edit_state.just_activated = false;
        }
        self.edit_state.frames_active += 1;

        match &editor_snapshot {
            CellEditor::TextInput => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                let entered = ui
                    .input_text("##edit", &mut self.edit_state.text_buf)
                    .enter_returns_true(true)
                    .build();
                if entered {
                    commit = true;
                }
                // ImGui handles focus natively: deactivation = user clicked away / Tab / etc.
                if !first_frame && !entered {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SliderInt { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.int_val);
                // Commit only when user releases the slider (not every drag frame)
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SliderFloat { min, max } => {
                ui.slider_config("##edit", *min, *max)
                    .build(&mut self.edit_state.float_val);
                if !first_frame && ui.is_item_deactivated_after_edit() {
                    commit = true;
                }
            }
            CellEditor::SpinInt { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputInt(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.int_val,
                        *step,
                        *step_fast,
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::SpinFloat { step, step_fast } => {
                if first_frame {
                    unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0) };
                }
                unsafe {
                    dear_imgui_rs::sys::igInputFloat(
                        c"##edit".as_ptr(),
                        &mut self.edit_state.float_val,
                        *step,
                        *step_fast,
                        c"%.2f".as_ptr(),
                        0,
                    );
                }
                if !first_frame {
                    if ui.is_item_deactivated_after_edit() {
                        if self.config.commit_on_focus_loss {
                            commit = true;
                        } else {
                            cancel = true;
                        }
                    } else if ui.is_item_deactivated() {
                        cancel = true;
                    }
                }
            }
            CellEditor::Custom => {
                if let Some(row) = self.data.get_mut(idx)
                    && row.render_editor(ui, col_idx)
                {
                    commit = true;
                }
            }
            _ => {
                self.edit_state.deactivate();
                return;
            }
        }

        // Escape always cancels
        if ui.is_key_pressed(Key::Escape) {
            cancel = true;
        }

        if cancel {
            self.edit_state.deactivate();
        } else if commit {
            let value = self.edit_state.take_cell_value(&editor_snapshot);
            if let Some(row) = self.data.get_mut(idx) {
                row.set_cell_value(col_idx, &value);
            }
            self.edit_state.deactivate();
        }
    }

    /// Compute the effective row height: custom style > config default > auto (by density).
    fn effective_row_height(&self, row_style: &Option<row::RowStyle>) -> f32 {
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

    // ─── Internal: keyboard navigation ─────────────────────────────

    fn handle_keyboard_nav(&mut self, ui: &Ui, row_count: usize) {
        if !ui.is_window_focused()
            || self.edit_state.active
            || self.config.selection_mode == SelectionMode::None
            || row_count == 0
        {
            return;
        }
        let current = self.selection_anchor.unwrap_or(0);
        let new_idx = if ui.is_key_pressed(Key::UpArrow) {
            Some(current.saturating_sub(1))
        } else if ui.is_key_pressed(Key::DownArrow) {
            Some((current + 1).min(row_count - 1))
        } else if ui.is_key_pressed(Key::Home) {
            Some(0)
        } else if ui.is_key_pressed(Key::End) {
            Some(row_count - 1)
        } else if ui.is_key_pressed(Key::PageUp) {
            Some(current.saturating_sub(20))
        } else if ui.is_key_pressed(Key::PageDown) {
            Some((current + 20).min(row_count - 1))
        } else {
            None
        };

        if let Some(idx) = new_idx
            && (idx != current || !self.selected_rows.contains(&idx))
        {
            self.selected_rows.clear();
            self.selected_rows.insert(idx);
            self.selection_anchor = Some(idx);
            self.pending_scroll_to = Some(idx);
        }
    }

    // ─── Internal: scroll ──────────────────────────────────────────

    fn handle_scroll(&mut self, ui: &Ui, row_count: usize) {
        if let Some(target) = self.pending_scroll_to.take()
            && row_count > 0
        {
            let frac = target as f32 / (row_count - 1).max(1) as f32;
            ui.set_scroll_y(frac * ui.scroll_max_y());
        }
        if self.config.auto_scroll {
            let wheel = ui.io().mouse_wheel();
            if wheel > 0.0 && ui.is_window_hovered() {
                self.config.auto_scroll = false;
            }
            if self.config.auto_scroll && row_count > 0 {
                ui.set_scroll_here_y(1.0);
            }
        }
    }

    // ─── Internal: selection ────────────────────────────────────────

    fn handle_selection(&mut self, ui: &Ui, idx: usize) {
        match self.config.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.insert(idx);
                self.selection_anchor = Some(idx);
            }
            SelectionMode::Multi => {
                let io = ui.io();
                let ctrl = io.key_ctrl();
                let shift = io.key_shift();

                if ctrl {
                    // Toggle: O(1) insert/remove via HashSet
                    if !self.selected_rows.remove(&idx) {
                        self.selected_rows.insert(idx);
                    }
                    self.selection_anchor = Some(idx);
                } else if shift {
                    let anchor = self.selection_anchor.unwrap_or(idx);
                    let max_idx = self.data.len().saturating_sub(1);
                    let (start, end) = if idx < anchor {
                        (idx, anchor.min(max_idx))
                    } else {
                        (anchor, idx.min(max_idx))
                    };
                    self.selected_rows.clear();
                    for r in start..=end {
                        self.selected_rows.insert(r);
                    }
                    // Keep anchor unchanged for consecutive shift-clicks
                } else {
                    self.selected_rows.clear();
                    self.selected_rows.insert(idx);
                    self.selection_anchor = Some(idx);
                }
            }
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Build tab-separated text from the given selection.
/// `fill(row, col, buf)` writes display text for each cell into `buf`.
fn build_copy_text<F>(selected: &IndexSet, col_count: usize, fill: F) -> String
where
    F: Fn(usize, usize, &mut String),
{
    let mut sorted: Vec<usize> = selected.iter().copied().collect();
    sorted.sort_unstable();
    let mut out = String::new();
    let mut cell_buf = String::new();
    for row_idx in sorted {
        for col_idx in 0..col_count {
            if col_idx > 0 {
                out.push('\t');
            }
            cell_buf.clear();
            fill(row_idx, col_idx, &mut cell_buf);
            out.push_str(&cell_buf);
        }
        out.push('\n');
    }
    out
}

/// Physical pixel height of a row inside a Dear ImGui table.
///
/// The value passed to `Selectable::size([_, row_height])` (and to
/// `TableNextRow`'s `min_row_height`) is NOT what ImGui actually lays out.
/// Every row is augmented by `2 * CellPadding.y`:
///
/// * `TableBeginCell` offsets the cursor by `+CellPadding.y` from `RowPosY1`
///   (imgui_tables.cpp:2188 — `window->DC.CursorPos.y = table->RowPosY1 +
///   table->RowCellPaddingY;`).
/// * `TableEndCell` extends `RowPosY2` to `CursorMaxPos.y + CellPadding.y`
///   (imgui_tables.cpp:2247).
///
/// Therefore `RowPosY2 - RowPosY1 == row_height + 2*CellPadding.y` for any
/// row whose content (here: the SPAN_ALL_COLUMNS Selectable) equals
/// `row_height`.
///
/// `ListClipper::items_height` must be set to this stride: ImGui's
/// `ImGuiListClipper::End` seeks the cursor to
/// `StartPosY + ItemsCount * items_height` (imgui.cpp:3401, 3406), which
/// fixes the inner scroll-window's content size. Using the bare `row_height`
/// there understates the content size by `row_count * 2*CellPadding.y` and
/// ImGui clamps `scroll_y <= scroll_max_y`, making the final rows
/// unreachable via manual scroll. This matches the upstream hint at
/// imgui.cpp:3319 ("If your clipper item height is != from actual table
/// row height, consider using ImGuiListClipperFlags_NoSetTableRowCounters").
#[inline]
pub(crate) fn row_height_to_stride(row_height: f32, cell_padding_y: f32) -> f32 {
    row_height + 2.0 * cell_padding_y.max(0.0)
}

/// Quantize a Dear ImGui table's outer height so it holds a whole number of
/// rows plus the header — used by `TableConfig::snap_last_row`.
///
/// Ensures at least one row fits even when the available area is very small.
/// `row_stride` must already include the `2*CellPadding.y` surcharge (see
/// `row_height_to_stride`).
#[inline]
pub(crate) fn snap_outer_height(avail_h: f32, header_h: f32, row_stride: f32) -> f32 {
    let content_h = (avail_h - header_h).max(0.0);
    let row_count_fit = if row_stride > 0.0 {
        (content_h / row_stride).floor().max(0.0)
    } else {
        0.0
    };
    let quantized = row_count_fit * row_stride + header_h;
    quantized.max(row_stride + header_h)
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn row_stride_adds_two_cell_paddings() {
        // Normal density with default ImGui CellPadding (2 px).
        assert_eq!(row_height_to_stride(25.0, 2.0), 29.0);
        // Generous padding used in some themes.
        assert_eq!(row_height_to_stride(25.0, 4.0), 33.0);
        // Dense density, zero padding.
        assert_eq!(row_height_to_stride(17.0, 0.0), 17.0);
    }

    #[test]
    fn row_stride_clamps_negative_padding() {
        // Bogus negative padding from corrupted style must not shrink the stride.
        assert_eq!(row_height_to_stride(20.0, -5.0), 20.0);
    }

    #[test]
    fn snap_fits_nine_rows() {
        // avail=300, header=20, stride=29 → floor(280/29)=9 → 9*29+20=281.
        assert!((snap_outer_height(300.0, 20.0, 29.0) - 281.0).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_guarantees_at_least_one_row() {
        // avail too small to fit even header+row → still returns header+stride.
        let out = snap_outer_height(10.0, 20.0, 29.0);
        assert!((out - (29.0 + 20.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_with_exact_fit() {
        // avail exactly fits 10 rows + header → quantized unchanged.
        let stride = 30.0;
        let header = 25.0;
        let avail = 10.0 * stride + header;
        let out = snap_outer_height(avail, header, stride);
        assert!((out - avail).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_zero_stride_does_not_panic() {
        // Pathological input must not divide by zero.
        let out = snap_outer_height(200.0, 20.0, 0.0);
        assert!((out - 20.0).abs() < f32::EPSILON);
    }

    /// Regression test for the scroll-unreachability bug: with 500 rows of
    /// `row_h=25` and `cell_padding_y=2`, the total content height the clipper
    /// reports must match the rendered height (500 * 29 = 14500), not the
    /// bogus 500 * 25 = 12500 that the pre-fix code produced.
    #[test]
    fn clipper_content_matches_rendered_height_large_row_count() {
        let row_count = 500usize;
        let row_h = 25.0;
        let cp_y = 2.0;

        // Pre-fix (bogus): items_height == row_h
        let bogus_total = row_count as f32 * row_h;
        // Post-fix: items_height == row_h + 2*CellPadding.y
        let stride = row_height_to_stride(row_h, cp_y);
        let correct_total = row_count as f32 * stride;

        assert_eq!(correct_total, 14500.0);
        assert_eq!(bogus_total, 12500.0);
        // The gap equals exactly `row_count * 2 * CellPadding.y`.
        assert_eq!(correct_total - bogus_total, row_count as f32 * 2.0 * cp_y);
    }
}
