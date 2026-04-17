# VirtualTable

Virtualized table component for Dear ImGui, capable of rendering up to 10,000,000 rows at 60 FPS.

**Capacity**: Hard limit of `MAX_TABLE_ROWS` (10,000,000). `RingBuffer::new()` clamps capacity to this value. When full, oldest rows are automatically evicted (FIFO).

## Overview

`VirtualTable<T>` is a generic, trait-driven table widget. Implement the `VirtualTableRow` trait for your data type, define columns with `ColumnDef`, and the table handles rendering, sorting, editing, and selection.

## Features

- **Virtualization** via Dear ImGui ListClipper (only visible rows are rendered)
- **Column management**: resize, reorder, hide/show, freeze, alignment
- **Sorting**: single or multi-column, ascending/descending
- **Inline editing**: 10+ editor widgets (text, checkbox, combo, slider, spinner, color, progress bar, button, custom)
- **Selection**: None, Single, Multi (Ctrl+Click toggle, Shift+Click range) with vivid highlight and white text
- **Keyboard navigation**: Up/Down, Home/End, PageUp/PageDown with auto-scroll to selection
- **Scroll-to-row**: programmatic `scroll_to_row(idx)` and `select_row(idx)`
- **Row density**: Normal, Compact, Dense
- **Per-row and per-cell styling** (background color, text color, alignment)
- **Auto-scroll** (follow newest data, auto-disable on scroll-up)
- **Context menus** with row/column tracking
- **RingBuffer\<T\>** for fixed-capacity FIFO storage (O(1) push, automatic oldest-row eviction)
- **Capacity limit**: `MAX_TABLE_ROWS` (10,000,000) — clamped on `RingBuffer::new()`
- **Zero per-frame allocations** (scratch buffers reused, scoped borrows for ComboBox items and Button labels)

## Quick Start

```rust
use dear_imgui_custom_mod::virtual_table::*;

// 1. Define your row type
struct LogEntry {
    timestamp: String,
    level: &'static str,
    message: String,
}

// 2. Implement the trait
impl VirtualTableRow for LogEntry {
    fn cell_value(&self, col: usize) -> CellValue {
        match col {
            0 => CellValue::Text(self.timestamp.clone()),
            1 => CellValue::Text(self.level.to_string()),
            2 => CellValue::Text(self.message.clone()),
            _ => CellValue::Text(String::new()),
        }
    }

    fn set_cell_value(&mut self, col: usize, value: &CellValue) {
        if col == 2 {
            if let CellValue::Text(s) = value {
                self.message = s.clone();
            }
        }
    }
}

// 3. Create table
let columns = vec![
    ColumnDef::new("Time").fixed(120.0),
    ColumnDef::new("Level").fixed(80.0).align(CellAlignment::Center),
    ColumnDef::new("Message").stretch(1.0).editor(CellEditor::TextInput),
];

let config = TableConfig {
    sortable: true,
    selection_mode: SelectionMode::Single,
    edit_trigger: EditTrigger::DoubleClick,
    ..Default::default()
};

let mut table = VirtualTable::new("##logs", columns, 50_000, config);

// 4. Add data
table.push(LogEntry { /* ... */ });

// 5. Render each frame
table.render(&ui);
```

## Column Definition

Use the builder pattern:

```rust
ColumnDef::new("Name")
    .stretch(1.0)                              // fill remaining space
    .align(CellAlignment::Left)                // Left (default), Center, Right
    .editor(CellEditor::TextInput)             // inline editing widget
    .no_sort()                                 // disable sorting for this column
    .no_resize()                               // fixed width, no drag handle
```

### Sizing

| Method | Description |
|--------|-------------|
| `.fixed(px)` | Exact pixel width |
| `.stretch(weight)` | Proportional fill (weight relative to other stretch columns) |
| `.auto_fit(init_width)` | Auto-fit to content (requires initial width in px) |

### Clip Tooltip

When cell text is wider than the column, a tooltip is automatically shown on hover.
Enabled by default. Disable per-column with `.no_clip_tooltip()`.

```rust
ColumnDef::new("Description")
    .stretch(1.0)
    .clip_tooltip()        // enabled by default
    .no_clip_tooltip()     // disable for this column
```

### Default Sort Direction

Set the preferred initial sort direction when the user clicks a column header:

```rust
ColumnDef::new("Date")
    .fixed(120.0)
    .default_sort(false)   // default descending (newest first)

ColumnDef::new("Name")
    .stretch(1.0)
    .default_sort(true)    // default ascending (A-Z)
```

### Editors

| `CellEditor` | Widget |
|---------------|--------|
| `None` | Read-only (default) |
| `TextInput` | Single-line text field |
| `Checkbox` | Toggle for `CellValue::Bool` |
| `ComboBox { items }` | Dropdown for `CellValue::Choice(index)` |
| `SliderInt { min, max }` | Integer slider |
| `SliderFloat { min, max }` | Float slider |
| `SpinInt { step, step_fast }` | Integer stepper |
| `SpinFloat { step, step_fast }` | Float stepper |
| `ProgressBar` | Read-only progress (0.0-1.0) |
| `ColorEdit` | Color picker for `CellValue::Color` |
| `Button { label }` | Clickable button (check `table.button_clicked`) |
| `Custom` | User-rendered (override `render_cell`) |

## VirtualTableRow Trait

Required methods:

```rust
fn cell_value(&self, col: usize) -> CellValue;
fn set_cell_value(&mut self, col: usize, value: &CellValue);
```

Optional overrides:

| Method | Description |
|--------|-------------|
| `cell_display_text(&self, col, buf)` | Custom display text (avoids CellValue allocation) |
| `row_style(&self)` | Per-row background/text color |
| `cell_style(&self, col)` | Per-cell styling |
| `compare(&self, other, col)` | Custom sort ordering |
| `render_cell(&self, ui, col)` | Full custom cell rendering |
| `row_tooltip(&self, buf)` | Plain-text tooltip on row hover |
| `render_tooltip(&self, ui)` | Rich tooltip via Dear ImGui |

## Configuration

```rust
TableConfig {
    resizable: true,           // column drag-resize
    reorderable: false,        // column drag-reorder
    hideable: true,            // right-click header to hide columns
    sortable: true,            // click header to sort
    selection_mode: SelectionMode::Multi,
    selection_color: [0.20, 0.45, 0.85, 0.75],       // vivid blue at 75% opacity
    selection_text_color: Some([1.0, 1.0, 1.0, 1.0]), // white text on selection
    edit_trigger: EditTrigger::DoubleClick,
    row_density: RowDensity::Normal,
    show_row_lines: true,
    show_column_lines: true,
    auto_scroll: false,        // follow newest rows
    ..Default::default()
}
```

### Selection Highlight

Selected rows are painted with `selection_color` (default: vivid blue, 75% opacity) and text is overridden with `selection_text_color` (default: white). Both are configurable:

```rust
config.selection_color = [0.20, 0.45, 0.85, 0.75];        // RGBA background
config.selection_text_color = Some([1.0, 1.0, 1.0, 1.0]); // white text
config.selection_text_color = None;                         // keep default text color
```

### Keyboard Navigation

When the table is focused and no editor is active:

| Key | Action |
|-----|--------|
| **Up/Down** | Move selection by one row |
| **Home/End** | Jump to first/last row |
| **PageUp/PageDown** | Jump 20 rows up/down |

All keyboard actions auto-scroll the view to keep the selected row visible.

### Programmatic Scroll

```rust
table.scroll_to_row(42);    // scroll row 42 into view
table.select_row(42);       // select row 42 + scroll to it
```

## Performance (10M rows)

VirtualTable is optimized to handle up to 10,000,000 rows at 60 FPS.

### Per-frame rendering: O(visible rows)

- **ListClipper virtualization** — Dear ImGui renders only visible rows (~50–100), regardless of total row count.
- **RingBuffer\<T\>** — O(1) push, O(1) indexed access, no allocations after creation. When full, oldest entry is overwritten (FIFO).

### Capacity

| Constant | Value | Enforced at |
|----------|-------|-------------|
| `MAX_TABLE_ROWS` | 10,000,000 | `RingBuffer::new()` — capacity is clamped |

The RingBuffer always evicts the oldest row when full — this is inherent to the ring buffer design and always active.

### Zero per-frame allocations

- **Scoped borrows for ComboBox/Button** — `items` and `label` references are scoped so the borrow ends before mutable data access, eliminating `Vec<String>` clone per frame.
- **`take_cell_value()`** — moves String out of edit buffer via `mem::replace` instead of cloning (zero-copy commit).
- **ListClipper with explicit `items_height`** — accurate row height avoids per-frame auto-measurement and empty gaps.
- **Padding-aware clipper stride** — `items_height` is set to `row_h + 2*CellPadding.y` because ImGui's table adds cell padding around every row (`TableBeginCell` / `TableEndCell`, see `imgui_tables.cpp:1915,2188,2247`). Using the bare `row_h` understates the content size by `row_count * 2*CellPadding.y` and makes the final rows unreachable via manual scroll inside tightly-sized containers (e.g. nested `child_window`). The crate exposes the helper as `virtual_table::row_height_to_stride(row_h, cell_padding_y)` and covers it with unit tests.
- **Safe error handling** — zero `unwrap()` calls in render paths; all use `if let Some` / `let Some else continue` patterns.
- **Shared utilities** — `EditorKind`, `alignment_pad`, clipboard helpers extracted to avoid duplication between virtual_table and virtual_tree.

### RingBuffer Capacity

`RingBuffer::new(capacity)` clamps capacity to `MAX_TABLE_ROWS` (10,000,000). When the buffer is full, `push()` overwrites the oldest entry (FIFO) — this is always active by design. There is no way to disable eviction on the ring buffer; use a `Vec<T>` if you need unbounded storage.

## Architecture

```
virtual_table/
  mod.rs          VirtualTable<T> struct, rendering, selection, scrolling
  config.rs       TableConfig, SelectionMode, EditTrigger, BorderStyle, RowDensity
  column.rs       ColumnDef, CellEditor, EditorKind, alignment_pad (shared with virtual_tree)
  row.rs          VirtualTableRow trait, CellValue, CellStyle, RowStyle
  edit.rs         Inline editing state machine (16 unit tests)
  sort.rs         Sort state (multi-column)
  ring_buffer.rs  Fixed-capacity O(1) ring buffer with iterators
```

## Unit Tests

Run with `cargo test --lib`:
- `edit.rs` — 16 tests: activate/deactivate, value buffers, take_cell_value for all editor types
- `ring_buffer` — tested via `cargo test --example demo_table` (push, wrap, sort, iter, stress)
