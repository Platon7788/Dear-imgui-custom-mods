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
//! ├── mod.rs          VirtualTable<T> struct, new()/push(), free helpers re-export
//! ├── api.rs          Public data/column/selection/editing API + export/import
//! ├── render.rs       Render entry points (ring/slice/lookup), setup, read-only rows
//! ├── row_render.rs   Editable row rendering, header, sort, row-height resolution
//! ├── editor.rs       Inline cell-editor activation + rendering
//! ├── input.rs        Keyboard navigation, scroll, click selection
//! ├── helpers.rs      build_copy_text, row_height_to_stride, snap_outer_height
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

// Method impls + free helpers split across files
// (CLAUDE.md: keep files < 500 lines). Child modules extend the same
// `VirtualTable<T>` via `impl` blocks and see its parent-private fields.
mod api;
mod editor;
mod helpers;
mod input;
mod render;
mod row_render;

pub use column::{CellAlignment, CellEditor, ColumnDef, ColumnSizing};
pub use config::{BorderStyle, EditTrigger, RowDensity, SelectionMode, SizingPolicy, TableConfig};
pub use ring_buffer::{MAX_TABLE_ROWS, RingBuffer};
pub use row::{CellStyle, CellValue, RowStyle, VirtualTableRow};

use crate::utils::clipboard::set_clipboard;
use crate::utils::text::calc_text_size;
use column::{EditorKind, alignment_pad, editor_kind};
use dear_imgui_rs::{
    Key, ListClipper, MouseButton, SelectableFlags, TableBgTarget, TableRowFlags, Ui,
};
use edit::EditState;
use helpers::{build_copy_text, snap_outer_height};
use sort::{SortSpec, SortState};
// Re-export so `crate::virtual_table::row_height_to_stride` (used by
// `virtual_tree`) keeps resolving after the move into `helpers`.
pub(crate) use helpers::row_height_to_stride;

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
            pending_scroll_to: None,
            edit_state: EditState::default(),
            sort_state: SortState::default(),
            cell_buf: String::with_capacity(256),
        }
    }

    // ─── Data access ────────────────────────────────────────────────

    /// Append a row. O(1). If at capacity, the oldest row is evicted.
    ///
    /// On eviction every surviving row's logical index slides down by one;
    /// index-based selection / anchor / edit / pending-scroll are shifted in
    /// step so they stay pinned to the same data instead of silently jumping to
    /// the row that slid underneath them.
    #[inline]
    pub fn push(&mut self, item: T) {
        // Only the at-capacity push evicts. Skip all bookkeeping on the hot
        // streaming path when nothing index-based is currently tracked.
        if self.data.len() == self.data.capacity()
            && (!self.selected_rows.is_empty()
                || self.selection_anchor.is_some()
                || self.pending_scroll_to.is_some()
                || self.edit_state.active)
        {
            self.shift_indices_for_eviction();
        }
        self.data.push(item);
    }

    /// Slide index-based UI state down by one to follow a FIFO eviction of
    /// logical row 0. Anything pinned to row 0 is dropped (its data is gone).
    fn shift_indices_for_eviction(&mut self) {
        if !self.selected_rows.is_empty() {
            // Drain + reinsert reuses the set's capacity (no realloc).
            let indices: Vec<usize> = self.selected_rows.drain().collect();
            for r in indices {
                if r > 0 {
                    self.selected_rows.insert(r - 1);
                }
            }
        }
        self.selection_anchor = match self.selection_anchor {
            Some(0) | None => None,
            Some(a) => Some(a - 1),
        };
        self.pending_scroll_to = match self.pending_scroll_to {
            Some(0) | None => None,
            Some(a) => Some(a - 1),
        };
        if self.edit_state.active {
            if self.edit_state.row == 0 {
                self.edit_state.deactivate();
            } else {
                self.edit_state.row -= 1;
            }
        }
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    /// Minimal row carrying one integer payload.
    struct R(usize);
    impl VirtualTableRow for R {
        fn cell_value(&self, _col: usize) -> CellValue {
            CellValue::Int(self.0 as i64)
        }
        fn set_cell_value(&mut self, _col: usize, _value: &CellValue) {}
    }

    fn table(cap: usize) -> VirtualTable<R> {
        VirtualTable::new("t", vec![ColumnDef::new("c")], cap, TableConfig::default())
    }

    #[test]
    fn push_eviction_shifts_selection_to_track_data() {
        let mut t = table(3);
        for v in 0..3 {
            t.push(R(v)); // logical rows [0,1,2] hold values 0,1,2
        }
        t.select_row(2); // select the row whose payload is 2
        assert!(t.is_selected(2));

        t.push(R(3)); // full → evict logical row 0; window slides to [1,2,3]
        assert!(t.is_selected(1), "selection follows its data row down");
        assert!(!t.is_selected(2));
        assert_eq!(t.get(1).map(|r| r.0), Some(2));
    }

    #[test]
    fn push_eviction_drops_row_zero_selection() {
        let mut t = table(3);
        for v in 0..3 {
            t.push(R(v));
        }
        t.select_row(0); // select the row about to be evicted
        t.push(R(3));
        assert_eq!(t.selected_count(), 0, "evicted row's selection is dropped");
    }

    #[test]
    fn push_without_eviction_keeps_selection() {
        let mut t = table(5); // room to grow → no eviction
        for v in 0..3 {
            t.push(R(v));
        }
        t.select_row(2);
        t.push(R(3));
        assert!(t.is_selected(2), "no eviction → indices unchanged");
    }

    #[test]
    fn remove_adjusts_selection_indices() {
        let mut t = table(8);
        for v in 0..5 {
            t.push(R(v));
        }
        t.select_row(3);
        assert_eq!(t.remove(1).map(|r| r.0), Some(1)); // rows above 1 shift down
        assert!(
            t.is_selected(2),
            "selection at 3 follows to 2 after remove(1)"
        );
        assert_eq!(t.len(), 4);
    }
}
