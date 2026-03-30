# VirtualTree

High-performance hierarchical tree-table component for Dear ImGui, capable of rendering up to 1,000,000 nodes at 60 FPS.

Inspired by DevExpress VirtualTreeList and Delphi VirtualStringTree.

**Capacity**: Configurable per-instance limit (default: `MAX_TREE_NODES` = 1,000,000). Insert methods return `Option<NodeId>` — `None` when at capacity. Optional FIFO eviction auto-removes the oldest root subtree on overflow.

## Overview

`VirtualTree<T>` is a generic, trait-driven tree widget. Implement the `VirtualTreeNode` trait for your data type, define columns with `ColumnDef`, and the tree handles rendering, expand/collapse, sorting, editing, selection, drag-and-drop, and filtering.

## Features

- **Slab/arena storage** with generational `NodeId` — O(1) insert, remove, and lookup
- **Flattened view cache** — rebuilt only on expand/collapse or structural changes, not every frame
- **ListClipper virtualization** — only visible rows rendered (100k+ nodes)
- **Multi-column** support reusing `ColumnDef`/`CellEditor` from `virtual_table`
- **Inline editing**: text, checkbox, combo, slider, color, button, custom
- **Selection**: None, Single, Multi (Ctrl+Click toggle, Shift+Click range on flat view)
- **Sibling-scoped sorting** via ImGui table headers
- **Drag-and-drop** node reparenting between any nodes
- **Filter/search** with auto-expand matching branches
- **Tree lines** — vertical/horizontal connector lines between parent and children
- **Striped rows** — alternating row backgrounds for readability
- **Scroll-to-node** — programmatically scroll any node into view
- **Per-node icons**: glyph, colored glyph, color swatch, or custom-rendered
- **Badges** — optional text after node label (e.g. children count, status)
- **Clip tooltips** — automatic tooltip when cell text is wider than column
- **Lazy children loading** — enable `config.lazy_load = true`; the tree calls `has_children()` to show the expand arrow, then loads children on first expand
- **Keyboard navigation**: Up/Down (flat), Left (collapse/parent), Right (expand/child)
- **Per-row and per-cell styling** (background color, text color)
- **Context menus** — right-click with node tracking
- **Configurable capacity** — per-instance `max_nodes` limit with runtime `set_capacity()`
- **Optional FIFO eviction** — auto-remove oldest root subtree when at capacity
- **Zero per-frame allocations** — scratch buffers reused, `mem::take` for arena ops

## Quick Start

```rust
use dear_imgui_custom_mod::virtual_tree::*;

// 1. Define your node type
struct FileNode {
    name: String,
    size: u64,
    is_folder: bool,
}

// 2. Implement the trait
impl VirtualTreeNode for FileNode {
    fn cell_value(&self, col: usize) -> CellValue {
        match col {
            0 => CellValue::Text(self.name.clone()),
            1 => CellValue::Text(format!("{} KB", self.size / 1024)),
            _ => CellValue::Text(String::new()),
        }
    }

    fn set_cell_value(&mut self, col: usize, value: &CellValue) {
        if col == 0 {
            if let CellValue::Text(s) = value {
                self.name = s.clone();
            }
        }
    }

    fn has_children(&self) -> bool {
        self.is_folder
    }

    fn icon(&self) -> NodeIcon {
        if self.is_folder {
            NodeIcon::Glyph('\u{F024B}') // folder icon
        } else {
            NodeIcon::Glyph('\u{F0214}') // file icon
        }
    }

    fn matches_filter(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }
}

// 3. Define columns
let columns = vec![
    ColumnDef::new("Name").stretch(1.0).editor(CellEditor::TextInput),
    ColumnDef::new("Size").fixed(100.0),
];

// 4. Create tree
let config = TreeConfig {
    show_tree_lines: true,
    drag_drop_enabled: true,
    striped: true,
    ..Default::default()
};
let mut tree = VirtualTree::new("##files", columns, config);

// 5. Add data
let root = tree.insert_root(FileNode {
    name: "src".into(), size: 0, is_folder: true,
}).unwrap();
tree.insert_child(root, FileNode {
    name: "main.rs".into(), size: 4096, is_folder: false,
});

// 6. Render each frame
tree.render(&ui);
```

## VirtualTreeNode Trait

### Required Methods

| Method | Signature | Purpose |
|--------|-----------|---------|
| `cell_value` | `fn cell_value(&self, col: usize) -> CellValue` | Return typed cell value for column |
| `set_cell_value` | `fn set_cell_value(&mut self, col: usize, value: &CellValue)` | Accept edited value back |
| `has_children` | `fn has_children(&self) -> bool` | Whether to show expand arrow |

### Optional Methods

| Method | Default | Purpose |
|--------|---------|---------|
| `cell_display_text(col, buf)` | Formats `cell_value()` | Custom display text (avoids allocation) |
| `row_style()` | `None` | Per-row background/text color |
| `cell_style(col)` | `None` | Per-cell styling |
| `render_cell(ui, col, id)` | `false` | Custom cell rendering |
| `render_editor(ui, col, id)` | `false` | Custom editor rendering |
| `row_tooltip(buf)` | empty | Plain-text tooltip on row hover |
| `render_tooltip(ui)` | `false` | Rich tooltip via Dear ImGui |
| `compare(other, col)` | `Equal` | Custom sort ordering |
| `icon()` | `None` | Icon for tree column |
| `render_icon(ui)` | `false` | Custom icon rendering |
| `accepts_drop(dragged)` | `true` | Whether node accepts a drop |
| `is_draggable()` | `true` | Whether node can be dragged |
| `matches_filter(query)` | `true` | Filter match predicate |
| `badge()` | `""` | Badge text after node label |

## NodeIcon

```rust
enum NodeIcon {
    None,                           // no icon
    Glyph(char),                    // unicode codepoint (e.g. MDI icon)
    GlyphColored(char, [f32; 4]),   // glyph with RGBA tint
    ColorSwatch([f32; 4]),          // small colored square
    Custom,                         // user-rendered via render_icon()
}
```

## Configuration

```rust
TreeConfig {
    // Table settings (inherited from virtual_table)
    table: TableConfig {
        sortable: true,
        selection_mode: SelectionMode::Multi,
        edit_trigger: EditTrigger::DoubleClick,
        row_density: RowDensity::Normal,
        ..Default::default()
    },

    // Tree-specific settings
    tree_column: 0,               // which column shows hierarchy (default: 0)
    indent_width: 20.0,           // pixels per depth level
    show_tree_lines: false,       // vertical/horizontal connector lines
    tree_line_color: [0.35, 0.35, 0.35, 0.6],
    expand_on_double_click: true, // double-click expands/collapses
    auto_expand_on_filter: true,  // auto-expand matching branches
    lazy_load: false,             // lazy children loading
    drag_drop_enabled: false,     // drag-and-drop reparenting
    multi_select_flat: true,      // Shift+Click range on flat view
    striped: true,                // alternating row backgrounds

    // Expand button style
    expand_style: ExpandStyle::Arrow, // Arrow (default) or Glyph { collapsed, expanded, color }

    // Capacity
    max_nodes: MAX_TREE_NODES,    // per-instance limit (1..=1,000,000)
    evict_on_overflow: false,     // auto-remove oldest root subtree when full
}
```

## Public API

### Construction

```rust
VirtualTree::new(label: &str, columns: Vec<ColumnDef>, config: TreeConfig) -> Self
```

### Node Operations

| Method | Description |
|--------|-------------|
| `insert_root(data) -> Option<NodeId>` | Add a root node (`None` if at capacity) |
| `insert_root_at(index, data) -> Option<NodeId>` | Insert root at position |
| `insert_child(parent, data) -> Option<NodeId>` | Add child to parent |
| `insert_child_at(parent, index, data) -> Option<NodeId>` | Insert child at position |
| `remove(id) -> Option<T>` | Remove node and its subtree |
| `clear()` | Remove all nodes |
| `get(id) -> Option<&T>` | Read-only access to node data |
| `get_mut(id) -> Option<&mut T>` | Mutable access to node data |
| `node_count() -> usize` | Total node count |
| `parent(id) -> Option<NodeId>` | Parent of node |
| `children(id) -> &[NodeId]` | Direct children |
| `children_count(id) -> usize` | Number of direct children |
| `roots() -> &[NodeId]` | Root node list |
| `depth(id) -> Option<u16>` | Depth in tree (0 = root) |
| `arena() -> &TreeArena<T>` | Read-only access to arena |
| `capacity() -> usize` | Current capacity limit |
| `set_capacity(n)` | Change capacity at runtime (clamped to `1..=MAX_TREE_NODES`) |
| `evict_on_overflow() -> bool` | Whether auto-eviction is enabled |
| `set_evict_on_overflow(bool)` | Enable/disable oldest-root-subtree eviction on overflow |

### Expand/Collapse

| Method | Description |
|--------|-------------|
| `expand(id)` | Expand a node |
| `collapse(id)` | Collapse a node |
| `toggle(id)` | Toggle expand/collapse |
| `expand_all()` | Expand all nodes |
| `collapse_all()` | Collapse all nodes |
| `is_expanded(id) -> bool` | Check expand state |
| `ensure_visible(id)` | Expand all ancestors |
| `scroll_to_node(id)` | Expand ancestors + scroll into view |

### Selection

| Method | Description |
|--------|-------------|
| `selected_nodes() -> impl Iterator<Item = NodeId>` | Iterate selected nodes |
| `selected_node() -> Option<NodeId>` | Single selection convenience |
| `selected_count() -> usize` | Number of selected nodes |
| `is_selected(id) -> bool` | Check if node is selected |
| `select(id)` | Select a node |
| `deselect(id)` | Deselect a node |
| `clear_selection()` | Deselect all |

### Sorting

```rust
tree.sort_children(parent: Option<NodeId>, col: usize, ascending: bool);
```

Sorts the children of `parent` (or roots if `None`) by column. Sorting is also triggered automatically by clicking column headers.

### Filter/Search

| Method | Description |
|--------|-------------|
| `set_filter(query)` | Apply filter — only matching nodes and their ancestors are shown |
| `clear_filter()` | Remove filter, show all nodes |
| `is_filtered() -> bool` | Whether a filter is active |

When a filter is active and `auto_expand_on_filter` is `true`, all ancestors of matching nodes are automatically expanded.

### Drag-and-Drop

Enable with `config.drag_drop_enabled = true`. Nodes can be dragged and dropped onto other nodes. Override `accepts_drop()` and `is_draggable()` on `VirtualTreeNode` to control which nodes participate.

```rust
tree.move_node(id, new_parent: Option<NodeId>, position: usize) -> bool;
```

After a successful reparent, `tree.last_reparent` is set to `Some((node_id, new_parent, position))` (type: `Option<(NodeId, Option<NodeId>, usize)>`). Check and clear it each frame to react to drag-and-drop events.

### Editing

| Method | Description |
|--------|-------------|
| `is_editing() -> bool` | Whether an editor is active |
| `cancel_edit()` | Cancel the current edit |

### View Info

| Method | Description |
|--------|-------------|
| `flat_row_count() -> usize` | Number of visible rows |
| `flat_index_of(id) -> Option<usize>` | Index in flat view |
| `columns() -> &[ColumnDef]` | Column definitions |
| `columns_mut() -> &mut [ColumnDef]` | Mutable column access |

### Rendering

```rust
tree.render(&mut self, ui: &Ui);
```

Call once per frame inside a Dear ImGui window.

## Column Definition

Reuses `ColumnDef` from `virtual_table`:

```rust
ColumnDef::new("Name")
    .stretch(1.0)                          // proportional fill
    .fixed(120.0)                          // exact pixel width
    .auto_fit(100.0)                       // auto-fit to content (init_width required)
    .align(CellAlignment::Left)           // Left, Center, Right
    .editor(CellEditor::TextInput)         // inline editor
    .clip_tooltip()                        // tooltip when text clipped (default: on)
    .no_clip_tooltip()                     // disable clip tooltip
    .default_sort(true)                    // default ascending sort
    .no_sort()                             // disable sorting
    .no_resize()                           // disable drag resize
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
| `ProgressBar` | Read-only progress (0.0–1.0) |
| `ColorEdit` | Color picker for `CellValue::Color` |
| `Button { label }` | Clickable button |
| `Custom` | User-rendered via `render_cell()` / `render_editor()` |

## Performance (1M nodes)

VirtualTree is optimized to handle up to 1,000,000 nodes at 60 FPS. Key techniques:

### Per-frame rendering: O(visible rows)

- **ListClipper virtualization** — Dear ImGui renders only visible rows (~50–100), regardless of total node count. Even with 1M nodes, per-frame work is constant.
- **Flat view cache** — the DFS linearization is rebuilt only on expand/collapse or structural changes, not every frame.

### Flat view rebuild: O(visible nodes), zero allocations per node

- **Two-pass children iteration** — counts visible children first, then iterates. Eliminates `Vec::collect()` that previously caused 100K–300K temporary allocations per rebuild.
- **HashMap index** — `flat_index_of(id)` is O(1) via `HashMap<NodeId, usize>` (was O(n) linear scan). Uses `foldhash` for fast hashing of `NodeId`.

### Arena operations: O(1)

- **Generational slab** — insert, remove, and lookup are all O(1).
- **`position()` + `swap_remove()`** — child detach in `remove()` is O(1) instead of O(siblings) with `retain()`.
- **`position()` + `remove()`** — child detach in `move_node()` preserves sibling order, still avoids full `retain()` scan.

### Filter/search: O(nodes) with early-break

- **Reusable buffer** — `matching_buf: Vec<NodeId>` is reused across filter calls (no re-allocation).
- **Safe early-break** — when `auto_expand` is false, ancestor walk stops at already-visited nodes, avoiding redundant work on deep trees.

### Zero per-frame allocations

- **Scratch buffer** — `write!` into reusable `cell_buf` instead of `format!()`.
- **Glyph expand button** — button ID written into `cell_buf` tail, glyph text reused without clone.
- **`mem::take` for arena children** — eliminates Vec clone during `remove()` and `update_subtree_depth()`.
- **Raw pointer for CellEditor** — avoids cloning `Vec<String>` (ComboBox items) per frame.
- **`take_cell_value()`** — moves String out of edit buffer via `mem::replace` instead of cloning (zero-copy commit).

### Capacity limits

| Constant | Value | Enforced at |
|----------|-------|-------------|
| `MAX_TREE_NODES` | 1,000,000 | Absolute upper bound — `TreeArena::alloc()` |

Capacity is configurable per instance via `TreeConfig::max_nodes` or `set_capacity(n)` at runtime. Both are clamped to `1..=MAX_TREE_NODES`.

**Overflow behavior** (configurable via `TreeConfig::evict_on_overflow` or `set_evict_on_overflow()`):

| `evict_on_overflow` | Behavior at capacity |
|---------------------|---------------------|
| `false` (default) | Insert returns `None` — caller decides what to do |
| `true` | Oldest root subtree (first root + all descendants) is auto-removed, then insert proceeds |

**Usage examples:**

```rust
// 1. Default: 1M limit, no eviction
let config = TreeConfig::default();

// 2. Custom limit, no eviction
let config = TreeConfig { max_nodes: 10_000, ..Default::default() };

// 3. Custom limit + auto-eviction (rolling log/monitor tree)
let config = TreeConfig {
    max_nodes: 50_000,
    evict_on_overflow: true,
    ..Default::default()
};

// 4. Change at runtime
tree.set_capacity(20_000);
tree.set_evict_on_overflow(true);
```

**Memory estimate at 1M nodes** (approximate):

| Component | Size |
|-----------|------|
| Arena slots (80 bytes × 1M) | ~76 MB |
| Arena generations (4 bytes × 1M) | ~4 MB |
| Flat view rows (24 bytes × 1M) | ~23 MB |
| Flat view index_map | ~15 MB |
| Selection HashSet (worst case) | ~16 MB |
| **Total** | **~118 MB** |

Pre-allocate with `TreeConfig::max_nodes` to size the arena upfront and avoid reallocation during bulk inserts.

## Architecture

```
virtual_tree/
  mod.rs          VirtualTree<T> — widget struct, render loop, public API
  arena.rs        TreeArena<T> — generational slab storage, parent/children links
  node.rs         VirtualTreeNode trait, NodeIcon enum
  config.rs       TreeConfig (wraps TableConfig from virtual_table)
  flat_view.rs    FlatView — cached linearization with continuation_mask for tree lines
  sort.rs         SortState — sibling-scoped sorting via ImGui table headers
  filter.rs       FilterState — search with auto-expand ancestors of matches
  drag.rs         Drag-and-drop payload type identifier for node reparenting
```

### Key Design Decisions

- **Generational NodeId** — prevents use-after-remove bugs (stale IDs return `None`)
- **Flat view cache** — avoids re-walking the tree every frame; only rebuilt on structure changes
- **continuation_mask: u64** — bitmask per row enables O(1) tree line rendering (supports up to 64 depth levels)
- **ManuallyDrop on TreeNodeToken** — `NO_TREE_PUSH_ON_OPEN` flag means no `TreePush` happens, so `Drop` must be skipped to avoid ID stack corruption
- **mem::take for arena children** — eliminates Vec clone during `remove()` and `update_subtree_depth()`
- **Unsafe pointer for CellEditor** — avoids cloning `Vec<String>` (ComboBox items) per frame; safe because columns aren't mutated during rendering
