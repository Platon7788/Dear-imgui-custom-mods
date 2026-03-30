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

pub mod column;
pub mod config;
mod edit;
pub mod ring_buffer;
pub mod row;
mod sort;

pub use column::{CellAlignment, CellEditor, ColumnDef, ColumnSizing};
pub use config::{BorderStyle, EditTrigger, RowDensity, SelectionMode, SizingPolicy, TableConfig};
pub use ring_buffer::{RingBuffer, MAX_TABLE_ROWS};
pub use row::{CellStyle, CellValue, RowStyle, VirtualTableRow};

use crate::utils::text::calc_text_size;
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
        let mut shifted = IndexSet::default();
        for &r in &self.selected_rows {
            shifted.insert(if r > index { r - 1 } else { r });
        }
        self.selected_rows = shifted;
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

    // ─── Editing ────────────────────────────────────────────────────

    pub fn is_editing(&self) -> bool {
        self.edit_state.active
    }

    pub fn cancel_edit(&mut self) {
        self.edit_state.deactivate();
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

        // Rows
        let row_count = self.data.len();
        let clip = ListClipper::new(row_count as i32);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            self.render_row(ui, row_idx as usize);
        }

        // Auto-scroll
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

        // No sorting — data order is caller-managed.

        // Rows (read-only path: no inline editing)
        let clip = ListClipper::new(row_count as i32);
        let tok = clip.begin(ui);

        for row_idx in tok.iter() {
            let idx = row_idx as usize;
            let row = match get_row(idx) {
                Some(r) => r,
                None => continue,
            };

            self.render_row_readonly(ui, idx, row);
        }

        // Auto-scroll
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
        if ui.is_item_hovered()
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
            self.context_col = if hovered >= 0 { Some(hovered as usize) } else { None };
            self.open_context_menu = true;
        }

        // ── Render cells (read-only: text + custom only) ───────────
        let row_text_color = row_style.as_ref().and_then(|s| s.text_color);
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
            if matches!(editor_kind(&self.columns[col_idx].editor), EditorKind::Custom)
                && row.render_cell(ui, col_idx)
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

            // Clip tooltip: show full text when hovered and clipped
            if self.columns[col_idx].clip_tooltip
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
            ui.table_header(&col.name);
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
                    ascending: s.sort_direction
                        == dear_imgui_rs::SortDirection::Ascending,
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

        // Row background
        if let Some(ref style) = row_style
            && let Some(bg) = style.bg_color
        {
            ui.table_set_bg_color(TableBgTarget::RowBg1, bg, -1);
        }

        // Selection state — O(1) via foldhash-backed HashSet
        let is_selected = self.selected_rows.contains(&idx);

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
                && !row.render_tooltip(ui) {
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
            self.context_col = if hovered >= 0 { Some(hovered as usize) } else { None };
            self.open_context_menu = true;
        }

        // ── Render cells ────────────────────────────────────────────
        let row_text_color = row_style.as_ref().and_then(|s| s.text_color);
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
                            && let Some(row) = self.data.get_mut(idx) {
                                row.set_cell_value(col_idx, &CellValue::Bool(b));
                            }
                }
                EditorKind::ComboBox => {
                    let items = match &self.columns[col_idx].editor {
                        CellEditor::ComboBox { items } => items.clone(),
                        _ => { self.edit_state.deactivate(); return; }
                    };
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Choice(mut choice) = val {
                            ui.set_next_item_width(-1.0);
                            if ui.combo_simple_string("##combo", &mut choice, &items)
                                && let Some(row) = self.data.get_mut(idx) {
                                    row.set_cell_value(col_idx, &CellValue::Choice(choice));
                                }
                        }
                }
                EditorKind::ColorEdit => {
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Color(mut c) = val {
                            ui.set_next_item_width(-1.0);
                            if ui.color_edit4_config("##color", &mut c)
                                .flags(dear_imgui_rs::ColorEditFlags::NO_INPUTS)
                                .build()
                                && let Some(row) = self.data.get_mut(idx) {
                                    row.set_cell_value(col_idx, &CellValue::Color(c));
                                }
                        }
                }
                EditorKind::Button => {
                    let label = match &self.columns[col_idx].editor {
                        CellEditor::Button { label } => label.clone(),
                        _ => { self.edit_state.deactivate(); return; }
                    };
                    if ui.button(&label) {
                        self.button_clicked = Some((idx, col_idx));
                    }
                }
                EditorKind::ProgressBar => {
                    if let Some(val) = self.data.get(idx).map(|r| r.cell_value(col_idx))
                        && let CellValue::Progress(p) = val {
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
                    let Some(row) = self.data.get(idx) else { continue };
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

                    // Clip tooltip: show full text when hovered and clipped
                    if self.columns[col_idx].clip_tooltip
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

// ─── Helpers (free functions to avoid borrow issues) ────────────────────────

fn alignment_pad(alignment: CellAlignment, col_w: f32, text_w: f32) -> f32 {
    match alignment {
        CellAlignment::Left => 0.0,
        CellAlignment::Center => ((col_w - text_w) * 0.5).max(0.0),
        CellAlignment::Right => (col_w - text_w - 4.0).max(0.0),
    }
}

/// Quick categorization of editor types to avoid matching the full enum in hot paths.
#[derive(Clone, Copy, PartialEq)]
enum EditorKind {
    None,
    Checkbox,
    ComboBox,
    Button,
    ProgressBar,
    ColorEdit,
    Custom,
    Other,
}

fn editor_kind(e: &CellEditor) -> EditorKind {
    match e {
        CellEditor::None => EditorKind::None,
        CellEditor::Checkbox => EditorKind::Checkbox,
        CellEditor::ComboBox { .. } => EditorKind::ComboBox,
        CellEditor::Button { .. } => EditorKind::Button,
        CellEditor::ProgressBar => EditorKind::ProgressBar,
        CellEditor::ColorEdit => EditorKind::ColorEdit,
        CellEditor::Custom => EditorKind::Custom,
        _ => EditorKind::Other,
    }
}
