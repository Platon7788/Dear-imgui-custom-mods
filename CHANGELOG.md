# Changelog

## [0.5.0] — 2026-03-30

### Added
- **hex_viewer** — Binary hex dump viewer widget
  - Offset/hex/ASCII column layout with configurable bytes-per-row (8, 16, 32)
  - Color regions for visual data segmentation
  - Data inspector panel with multi-type decoding (u8–u64, i8–i64, f32, f64)
  - Goto address (hex `0x` or decimal), pattern search with match navigation
  - Selection (click + shift-click), diff highlighting for changed bytes
  - Hover byte tooltips: offset (hex+dec), hex/dec/octal/binary values, ASCII
  - Hover row highlight, zero-dimmed byte styling
  - Little/big endian toggle, configurable column widths
- **timeline** — Zoomable profiler timeline widget
  - Multi-track layout with per-track collapse/expand
  - Nested span rendering with depth-based vertical offset
  - Flame graph view mode
  - Named markers on the time ruler
  - Pan (drag) + zoom (scroll) with Shift+scroll for horizontal pan
  - Adaptive time ruler with auto-scaled tick intervals
  - Color modes: by duration, by category, by name hash
  - Span tooltips with label, duration, category, source info
  - Configurable track height, span padding, colors
  - `Span::new` validates start/end and rejects NaN/Infinity
  - Division-by-zero guard in `x_to_time`
- **diff_viewer** — Side-by-side and unified diff viewer
  - Myers diff algorithm with O((N+M)D) time, capped at max_d=50,000
  - Side-by-side (two-panel) and unified view modes
  - Synchronized scrolling between panels
  - Fold/unfold unchanged regions with configurable context lines
  - Hunk navigation (prev/next) with keyboard support
  - Hover row highlights in both panel and unified modes
  - Current hunk blue accent bar in unified mode
  - `+`/`-` prefix characters in unified mode
  - Diff stats: additions, deletions, unchanged count
  - Hunk context preservation across hunk boundaries
- **property_inspector** — Hierarchical property editor
  - 15+ value types: Bool, I32, I64, F32, F64, String, Color3, Color4, Vec2, Vec3, Vec4, Enum, Flags, Object, Array
  - Categories with collapsible headers (click to toggle)
  - Property nodes with expand/collapse for nested children
  - Recursive child rendering with `std::mem::take` pattern
  - Type badges (dimmed type name right-aligned on each row)
  - Hover highlight on all rows
  - Search/filter support, diff highlighting
  - Builder API for categories and properties
- **toolbar** — Configurable horizontal toolbar widget
  - Buttons, toggles, separators, dropdowns, spacers
  - Icon support via `with_icon()` builder (MDI Unicode glyphs)
  - Hover underline accent with configurable color and thickness
  - Window-hovered guard to prevent click-through
  - Flexible spacer layout (auto-distributes remaining width)
  - Dropdown cycles through options on click
  - Builder pattern API with `with_enabled()`, `with_icon()`
- **status_bar** — Composable bottom status bar widget
  - Left/center/right sections with independent item lists
  - Status indicators: Success, Warning, Error, Info (colored dots)
  - Progress bar items with label (0.0..=1.0)
  - Clickable items with event emission (`StatusBarEvent`)
  - Icon support via `with_icon()` builder
  - Hover highlight on all items (subtle for non-clickable, stronger for clickable)
  - Window-hovered guard, tooltips via `with_tooltip()`
  - Color override via `with_color()`
- **demo_hex_viewer** — Interactive HexViewer demo with PE header sample, color regions, config panel
- **demo_timeline** — Timeline demo with 4 tracks, 50+ spans, markers, color mode switching
- **demo_diff_viewer** — DiffViewer demo with 4 sample datasets, mode/fold/context config
- **demo_property_inspector** — PropertyInspector demo with 5 categories, 20+ properties
- **demo_status_toolbar** — Combined Toolbar + StatusBar demo with event log
- **icons** — Expanded to 7,400+ Material Design Icons v7.4 constants

### Improved
- **node_graph** — Tooltip hover tracking moved after hit testing (was running before, so tooltips never triggered)
- **node_graph** — `collect_node_aabbs` now reuses a buffer instead of allocating Vec every frame
- **node_graph** — `NgColors` derives `Debug + Clone + Copy`, `NodeGraphConfig` derives `Debug + Clone`
- **node_graph** — Null-pointer guard on `igGetCurrentWindow()` unsafe calls with `debug_assert`
- **node_graph** — 42 new tests covering Graph slab, Viewport transforms, math functions (bezier, point-to-segment), InteractionState, config
- **toolbar** — Config derives `Copy` to avoid per-frame clones
- **status_bar** — Config derives `Copy`
- **property_inspector** — Config derives `Copy`, `PropertyValue` derives `PartialEq` and implements `Default`
- **diff_viewer** — Per-frame `clone()` of display lines eliminated via `render_panel_static` with raw slices

### Fixed
- **node_graph** — `node_to_top` comment corrected from "O(1)" to "O(n) find + O(1) swap_remove"
- **toolbar** — Dropdown panic on empty options list (added `!options.is_empty()` guard)
- **toolbar** — Dropdown selected index clamped at construction
- **toolbar** — ImGui `SetCursorPos` assertion crash (added `ui.dummy([0.0, 0.0])` after cursor advance)
- **status_bar** — ImGui `SetCursorPos` assertion crash (same fix)
- **timeline** — Division by zero in `x_to_time` when `pixels_per_second` is zero (clamped to `1e-9`)
- **timeline** — `Span::new` now validates and swaps start>end, rejects NaN/Infinity
- **timeline** — Shift+scroll conflict with zoom (now properly separated)
- **diff_viewer** — Myers algorithm capped at `max_d=50,000` to prevent excessive memory on large inputs
- **diff_viewer** — Hunk context loss: trailing context now preserved as leading context for next hunk
- **diff_viewer** — `render_unified` bounds check for mismatched line counts
- **code_editor/tokenizer** — Panic on multi-byte UTF-8 characters (Cyrillic, emoji) fixed

## [0.4.0] — 2026-03-30

### Added
- **code_editor** — Full-featured code editor widget built on ImGui DrawList API
  - `CodeEditor` widget with syntax highlighting, line numbers, cursor/selection, undo/redo
  - 10 built-in languages: Rust, TOML, RON, Rhai, JSON, YAML, XML, ASM (x86/ARM/RISC-V), Hex, None
  - Custom language support via `SyntaxDefinition` trait (`Language::Custom(Arc<dyn SyntaxDefinition>)`)
  - ASM tokenizer: AT&T + Intel + NASM syntax, registers, directives, labels, numeric literals
  - 6 built-in themes: DarkDefault, Monokai, OneDark, SolarizedDark, SolarizedLight, GithubLight
  - 3 embedded monospace fonts: Hack (default), JetBrains Mono NL, JetBrains Mono
  - MDI icons (Material Design Icons v7.4) merged into font atlas
  - `install_code_editor_font()` / `install_code_editor_font_ex()` — zero-config font setup
  - `BuiltinFont` enum with `Hack`, `JetBrainsMonoNL`, `JetBrainsMono` variants
  - Code folding with MDI chevron icons, hover highlight, and `"... N lines"` collapsed badge
  - `show_fold_indicators` config option — toggle fold UI and gutter column
  - Word wrap with smart word-boundary breaking
  - Find/replace bar with case-insensitive toggle, match navigation, replace-all
  - Multi-cursor support (Ctrl+D to select next occurrence)
  - Bracket matching and auto-close for `()`, `{}`, `[]`, quotes
  - Text transforms: UPPERCASE, lowercase, Title Case, trim whitespace
  - Line operations: duplicate, delete, move up/down
  - Toggle comment (Ctrl+/)
  - Font zoom (Ctrl+Scroll, Ctrl+Plus/Minus)
  - Hex editing mode: auto-space, auto-uppercase, value-based coloring
  - Color swatches next to hex color literals
  - Error/warning markers with underlines and gutter icons
  - Breakpoints with gutter indicators
  - Right-click context menu with 12 configurable sections (`ContextMenuConfig`)
  - `max_lines` and `max_line_length` config options (0 = unlimited)
  - Auto English keyboard layout on focus (Windows, opt-in)
  - `EditorConfig` with 20+ configurable options
- **demo_code_editor** — Interactive demo with font switcher, config panel, all features

### Improved
- **code_editor** — Adaptive smooth scrolling: faster catch-up when cursor moves rapidly (Enter spam)
- **code_editor** — Scroll dummy height includes bottom padding + 1px dummy for correct ImGui scroll extent
- **code_editor** — Wrap cache re-synced after input handling to prevent stale scroll targets on paste
- **code_editor** — `compute_wrap_points` rewritten: overflow checked BEFORE adding character width, re-evaluates current char after break (handles lines >2× max_width correctly)
- **code_editor** — Gutter layout: `| line numbers | fold icon | code |` with proper spacing

### Fixed
- **code_editor** — Scrollbar could not reach bottom of document with word wrap + large text
- **code_editor** — Rare HEX word-wrap overflow: last byte on a row could exceed the vertical boundary
- **code_editor/lang/asm** — NASM preprocessor directives (`%define`, `%macro`) were misclassified as AT&T registers
- **code_editor/lang/asm** — 12 clippy warnings about unused `line_start` variable

## [0.3.1] — 2026-03-26

### Added
- **virtual_tree** — `MAX_TREE_NODES` constant (1,000,000) — hard capacity limit with graceful `None` returns on insert
- **virtual_tree** — `TreeArena::with_capacity(n)` — pre-allocate arena with custom capacity limit (`1..=MAX_TREE_NODES`)
- **virtual_tree** — Configurable per-instance capacity: `set_capacity(n)` / `capacity()` on both `TreeArena` and `VirtualTree`
- **virtual_tree** — Optional FIFO eviction: `set_evict_on_overflow(true)` — auto-removes oldest root subtree when at capacity
- **virtual_tree** — `TreeConfig::max_nodes` and `TreeConfig::evict_on_overflow` — declarative capacity control
- **virtual_table** — `MAX_TABLE_ROWS` constant (1,000,000) — capacity clamped on `RingBuffer::new()`
- **virtual_tree** — `ExpandStyle::Glyph` — custom expand/collapse glyphs with optional color

### Improved

#### 500K-node optimization pass
- **virtual_tree/flat_view** — `index_of()` is now O(1) via `HashMap<NodeId, usize>` (was O(n) linear scan)
- **virtual_tree/flat_view** — Eliminated `visible_children.collect()` per expanded node — two-pass count+iterate without allocation (was 100K–300K temp Vec allocations per rebuild)
- **virtual_tree/filter** — Reusable `matching_buf` Vec across filter calls (no re-allocation)
- **virtual_tree/filter** — Safe early-break in ancestor walk when `auto_expand` is false (skip already-marked ancestors)
- **virtual_tree/arena** — `remove()` / `move_node()` use `position()` + `swap_remove()`/`remove()` instead of `retain()` — O(1) detach vs O(siblings)
- **virtual_tree/mod** — `deselect_descendants()` directly removes from HashSet instead of collecting into intermediate Vec
- **virtual_tree/mod** — Glyph expand button: zero-allocation rendering — button ID written into `cell_buf` tail, glyph text reused without clone
- **virtual_tree/mod** — `take_cell_value()` moves String out of edit buffer instead of cloning (zero-copy commit)
- **virtual_table/mod** — `handle_sort()` uses raw pointer to sort specs instead of `Vec::clone()`
- **virtual_table/mod** — `render_editor_inline()` uses raw pointer to `CellEditor` instead of `editor.clone()` (avoids cloning `Vec<String>` per frame)
- **virtual_table/mod** — `take_cell_value()` moves String out of edit buffer instead of cloning
- **virtual_table/mod** — All `unwrap()` calls in render_row replaced with safe `if let Some(row)` / `let Some(row) else continue` patterns — no panics at runtime
- **virtual_tree/flat_view** — Iterative DFS replaces recursive `walk()` — no stack overflow at any depth (tested at 10K levels)
- **virtual_tree/arena** — `remove()` and `update_subtree_depth()` converted from recursive to iterative — safe at any depth
- **virtual_tree** — `insert_root()` / `insert_root_at()` now return `Option<NodeId>` (capacity-aware)
- **virtual_tree/arena** — `depth` field uses `saturating_add(1)` — no u16 overflow at extreme depths
- **virtual_table/row** — Color formatting clamps `f32` to `0.0..=1.0` before `* 255 as u8` — no overflow
- **virtual_table/edit** + **virtual_tree/edit** — `i64→i32` and `f64→f32` casts clamped to prevent silent truncation
- **virtual_table/mod** — Shift+Click selection range clamped to `data.len()` — no out-of-bounds indices
- **virtual_table/mod** + **virtual_tree/mod** — `unreachable!()` in ComboBox/Button editor paths replaced with safe `deactivate() + return`
- **virtual_tree/mod** — `tree_column` clamped to `col_count - 1` — no silent skip on misconfigured index
- **bench** — Runtime stress tests for 500K and 1M nodes: insert, expand, flat_view rebuild, filter, remove, deep chain, memory estimate

## [0.3.0] — 2026-03-18

### Added
- **virtual_tree** — Hierarchical tree-table component for 100k+ nodes
  - `VirtualTree<T>` widget with `VirtualTreeNode` trait
  - `TreeArena<T>` — generational slab storage with `NodeId`, parent/children links, O(1) insert/remove/lookup
  - `FlatView` — cached linearization rebuilt only on structural changes (not every frame)
  - ListClipper virtualization for visible rows only
  - Multi-column support reusing `ColumnDef`/`CellEditor` from `virtual_table`
  - Inline editing: text, checkbox, combo, slider, color, button, custom
  - Selection: None, Single, Multi (Ctrl+Click toggle, Shift+Click range on flat view)
  - Sibling-scoped sorting via ImGui table headers
  - Drag-and-drop node reparenting with `accepts_drop()` / `is_draggable()` control
  - Filter/search with auto-expand matching branches
  - Tree lines — vertical/horizontal connector lines via `continuation_mask: u64` bitmask
  - Striped rows (alternating backgrounds) via `config.striped`
  - Scroll-to-node — `scroll_to_node(id)` expands ancestors + scrolls into view
  - `NodeIcon` variants: `Glyph`, `GlyphColored`, `ColorSwatch`, `Custom`
  - `badge()` trait method — optional text after node label
  - Clip tooltips — automatic hover tooltip when cell text exceeds column width
  - Lazy children loading via callback
  - Keyboard navigation: Up/Down (flat), Left (collapse/parent), Right (expand/child)
  - `TreeConfig` wrapping `TableConfig` with tree-specific settings
  - `children_count(id)`, `ensure_visible(id)`, `flat_row_count()`, `flat_index_of(id)` API
- **demo_tree** — Full interactive VirtualTree example
  - TaskNode with 6 kinds (Folder, RustFile, Config, Document, Test, Asset) and 4 priority levels
  - 6 columns: Name (TextInput), Done (Checkbox), Progress (SliderFloat), Priority (ComboBox), Size, Action (Button)
  - Colored icons per node type, per-row styling (dimmed done items), per-cell colored priority text
  - Toolbar: filter, expand/collapse all, stress test 10K nodes, add root, tree lines/striped/drag-drop toggles
  - Context menu: Add Child File, Add Subfolder, Toggle Done, Set Priority submenu, Delete
- **virtual_table** — New `ColumnDef` features
  - `ColumnSizing::AutoFit(f32)` — auto-fit column to content width
  - `clip_tooltip: bool` — automatic tooltip when cell text is wider than column (default: `true`)
  - `default_sort: Option<bool>` — default sort direction (ascending/descending) for column header
  - Builder methods: `.auto_fit()`, `.clip_tooltip()`, `.no_clip_tooltip()`, `.default_sort(ascending)`
  - Clip tooltip rendering in both read-only and editable row paths
- **docs/virtual_tree.md** — Full component documentation

### Improved
- **virtual_tree** — Zero per-frame allocations: `write!` into scratch buffer instead of `format!()`, `mem::take` for arena children ops, unsafe pointer for CellEditor access during render
- **virtual_tree** — `mem::forget` on TreeNodeToken with `NO_TREE_PUSH_ON_OPEN` to prevent ID stack corruption
- **virtual_tree** — Filter ancestor walk always reaches root (removed unsafe early-break optimization)
- **virtual_tree** — `.map().flatten()` → `.and_then()` cleanup
- **README.md** — Updated with virtual_tree component, docs links, demo command, project structure

## [0.2.1] — 2026-03-17

### Added
- **node_graph** — Stats overlay drawn on the canvas corner (node count, wire count, zoom level, selection count)
  - Configurable corner (`stats_overlay_corner: u8`, 0–3) and margin (`stats_overlay_margin: f32`)
  - Toggle via `show_stats_overlay: bool` in config
- **node_graph** — Orthogonal wire style (`WireStyle::Orthogonal`): 3-segment forward routing, 5-segment backward routing with obstacle avoidance
- **node_graph** — `body_height()` method on `NodeGraphViewer` trait — per-node body height override for nodes with multiple widget rows
- **node_graph** — Frustum culling: only visible nodes are rendered each frame, enabling graphs with up to 100,000 nodes

### Improved
- **node_graph** — `selected` field changed from `Vec<NodeId>` to `HashSet<NodeId>` — all selection operations are now O(1)
- **node_graph** — `selected()` now returns `Vec<NodeId>` (collected from HashSet) instead of `&[NodeId]`
- **node_graph** — Node body rendered inside `with_clip_rect()` — widgets can no longer overflow node boundaries
- **node_graph** — Bezier tangent length uses adaptive extent-based scaling instead of a fixed 50px value — curves look correct at all zoom levels and node distances
- **node_graph** — Minimap navigation: removed confusing viewport rectangle; click or drag on minimap navigates directly to that position
- **node_graph** — Minimap drag remains active when cursor leaves minimap bounds (coordinates clamped to valid range)
- **node_graph** — Removed scrollbar rendering and config fields (`show_scrollbar_h`, `show_scrollbar_v`, `scrollbar_thickness`)
- **node_graph** — Stats display moved from external toolbar to built-in canvas overlay
- **demo_node_graph** — `body_height()` implemented for Vec2 (54.0) and Color (42.0) nodes to fit their widget content

### Fixed
- **node_graph** — Wire drag-and-drop was broken: `is_mouse_dragging()` returns `false` on mouse-release frame; replaced with `mouse_drag_delta()` threshold check
- **node_graph** — ImGui assertion `SetCursorScreenPos() requires subsequent item` — added `ui.dummy()` after `set_cursor_screen_pos()` for body height reservation
- **node_graph** — Orthogonal wire hit-test now matches rendering exactly (removed erroneous `abs < 2.0` fallback condition)

## [0.2.0] — 2026-03-17

### Added
- **node_graph** — Visual node graph editor component
  - `NodeGraph<T>` widget with `NodeGraphViewer<T>` trait
  - Slab-based `Graph<T>` storage (O(1) insert/remove) + `HashSet<Wire>`
  - Pan/zoom canvas with scroll-to-cursor zoom
  - Bezier and straight-line wire rendering via native `ImDrawList`
  - 4 pin shapes: Circle, Triangle, Square, Diamond
  - Per-pin color, stroke, and wire style overrides (`PinInfo` builder)
  - Custom node headers with color tinting
  - Node body rendering with `&mut T` (sliders, combos, color pickers, etc.)
  - Multi-select (Ctrl+Click) and rectangle selection
  - Node collapse/expand with chevron button
  - Snap-to-grid with configurable grid size
  - Interactive mini-map (click/drag to navigate)
  - Wire yanking (Ctrl+Click wire to detach and redirect)
  - Dropped wire on canvas fires `DroppedWireOut`/`DroppedWireIn` actions for auto-connect menus
  - Context menus: right-click on canvas (`CanvasMenu`) or node (`NodeMenu`)
  - Keyboard: Delete (remove selected), Ctrl+A (select all), Escape (cancel wire/rect)
  - LOD culling: labels, pins, and bodies hidden at low zoom levels
  - Wire layer control: behind or above nodes
  - Tooltips on nodes and individual pins
  - `HashMap<PinId, [f32; 2]>` for O(1) pin position lookup
  - `HashSet<NodeId>` for O(1) draw order membership check
  - Fixed-size array for diamond pin geometry (zero per-frame allocations)
  - Multiple actions per frame via `Vec<GraphAction>` return type
  - `NodeToggled` and `SelectAll` handled internally
  - Multi-select snap drift fix (delta computed from snapped position)
  - Viewer trait lifetime fix: `&str` methods can return data from `&T` or `&self`
- **demo_node_graph** — Full interactive example
  - 8 node types: Float, Vec2, Color, Add/Sub/Mul/Div, Clamp, Mix, Output
  - Typed pins with different shapes and colors
  - Context menu to add nodes, auto-connect on dropped wires
  - Toolbar: Fit, Reset, Grid, Snap, Minimap, Wire Layer toggles
- **docs/** — Per-component documentation
  - `docs/file_manager.md` — FileManager guide with API reference
  - `docs/virtual_table.md` — VirtualTable guide with trait reference
  - `docs/page_control.md` — PageControl guide with tab styles
  - `docs/node_graph.md` — NodeGraph guide with full configuration reference

### Improved
- **node_graph** viewer trait: unified lifetime `'a` on `title()`, `input_label()`, `output_label()`, tooltip methods — returned `&str` can now come from node data, not just the viewer
- **node_graph** `select_node()` and `deselect_all()` now public API
- **node_graph** `fit_to_content()` uses actual node dimensions via `config.node_height()` + `viewer.node_width()` instead of hardcoded values
- **node_graph** `screen_to_graph()` guards against division-by-zero on `zoom <= 0`
- **node_graph** single `draw_order` clone per frame (was cloned twice)
- **node_graph** removed unused `_viewer` parameter from `is_collapse_button_hit()`
- **README.md** — Updated with node_graph component, docs links, all 4 examples

## [0.1.1] — 2026-03-15

### Improved
- **page_control** — 4 tab styles (Pill, Underline, Card, Square) with runtime switching
- **page_control** — Close button on dashboard tiles with confirmation dialog
- **page_control** — `Hash` derive on all public enums (`PageStatus`, `ContentView`, `TabStyle`, `PageAction`)
- **page_control** — Modern Rust patterns: `.is_some_and()`, let-chains, `AtomicU32` for static counters
- **page_control** — `Box<NestedPage>` to reduce enum variant size disparity in demo
- **file_manager** — Footer layout: filename input + buttons on a single row (SaveFile mode)
- **file_manager** — Filter dropdown + buttons on a single row (OpenFile mode)
- **file_manager** — Disabled confirm button rendered as dimmed button (preserves layout)
- **file_manager** — Content area height correctly reserves space for footer (no scroll needed)
- **file_manager** — All collapsible_if warnings fixed with let-chains
- **virtual_table** — All collapsible_if warnings fixed with let-chains
- **demo** — MDI font loading via `FontSource::TtfData` with merge mode (dynamic glyph loading)
- **demo** — Tab style switcher button in toolbar

### Fixed
- Unnecessary `as i32` cast on `ImGuiCond_Appearing` (already `i32`)
- `#[allow(clippy::too_many_arguments)]` on render functions that genuinely need many params
- Zero clippy warnings across the entire project

## [0.1.0] — 2026-03-06

### Added
- **file_manager** — Universal file/folder picker dialog
  - Modes: SelectFolder, OpenFile, SaveFile
  - Drive selector, breadcrumb navigation, file filters
  - Favorites sidebar, back/forward history, type-to-search
  - Rename, delete, new folder/file creation
  - Overwrite confirmation modal
  - Multi-select (Ctrl+Click), keyboard navigation
  - Zero per-frame allocations
- **virtual_table** — Virtualized table component
  - `VirtualTableRow` trait for custom row types
  - `RingBuffer<T>` — fixed-capacity O(1) ring buffer
  - `ColumnDef` — fixed, stretch, centered columns with builder pattern
  - ListClipper integration for 100k+ row rendering
  - Inline editing: text, checkbox, combo, slider, spinner, color, progress, custom
  - Selection modes: None, Single, Multi
  - Sortable columns (multi-column)
- **page_control** — Generic tabbed container
  - Dashboard view (interactive tile grid with status indicators)
  - Tabs view (pill-shaped tab strip with scroll buttons)
  - Close confirmation popups, badges, context menu
  - Keyboard navigation (arrow keys, Ctrl+W)
- **icons** — Material Design Icons v7.4 constants (160+ icons)
- **theme** — Dark color palette with semantic tokens
- **utils** — Color packing (RGBA to u32), text measurement wrapper
- **demo** — Interactive showcase with tabs for all components
