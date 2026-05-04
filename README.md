# Dear ImGui Custom Mod

Production-ready custom UI component library for `dear-imgui-rs`.

Zero per-frame allocations, modern Rust 2024 edition, fully themeable.

## Components

### Window Infrastructure

| Component | Description | Docs |
|-----------|-------------|------|
| **`app_window`** | Zero-boilerplate borderless application window — `AppWindow::run()` + `AppHandler` trait replaces ~300 lines of wgpu/winit/ImGui setup. Pure ImGui hit-detection titlebar (5 built-in themes via the unified `Theme` enum + per-instance `colors_override`, minimize/maximize/close, 8-direction edge resize, drag-to-move, close-confirmation, icon, extra buttons), event-driven render loop (`RenderMode`), DPI font rebuild, system clipboard backend, layout-independent shortcuts, cross-thread `AppProxy::wake()`, raw `on_window_event` hook, `WindowKind` presets (Splash / Tool / Dialog / Main) | [docs/app_window.md](docs/app_window.md) |
| **`nav_panel`** | Modern navigation panel (activity bar) — 3 docking positions (Left/Right/Top), flyout submenus, auto-hide with slide animation, toggle arrow, badges, button spacing/separators, per-button tooltip control, 5 unified themes, overlay variant (`render_nav_panel_overlay`) | [docs/nav_panel.md](docs/nav_panel.md) |
| **`confirm_dialog`** | Reusable modal confirmation dialog — 5 unified themes + `colors_override`, 4 draw-list icon types (Warning/Error/Info/Question), dim overlay, Esc/Enter keyboard shortcuts, green Cancel / red Confirm buttons, builder-pattern `DialogConfig` | [docs/confirm_dialog.md](docs/confirm_dialog.md) |
| **`notifications`** | Modern toast-notification center — 5 severity levels (Info/Success/Warning/Error/Debug) with draw-list icons, 6 stack placements (4 corners + top/bottom center), auto-dismiss timer with bottom progress bar, pause-on-hover, Fade/SlideIn/None animations, action buttons with caller-defined ids, manual `×` close, per-toast custom accent override, max-visible cap, 5 unified themes + `colors_override` | [docs/notifications.md](docs/notifications.md) |

### UI Widgets

| Component | Description | Docs |
|-----------|-------------|------|
| **`code_editor`** | Full-featured code editor — 10 languages (Rust, TOML, RON, Rhai, JSON, YAML, XML, ASM, Hex, Custom), 6 themes, 3 built-in fonts (Hack, JetBrains Mono), code folding, word wrap, find/replace, multi-cursor, undo/redo, breakpoints, error markers, smooth scrolling | [docs/code_editor.md](docs/code_editor.md) |
| **`file_manager`** | Universal file/folder picker dialog — SelectFolder, OpenFile, SaveFile modes. Breadcrumb navigation, favorites sidebar, back/forward history, type-to-search, file filters, overwrite confirmation | [docs/file_manager.md](docs/file_manager.md) |
| **`virtual_table`** | Virtualized table for up to 10M rows — ListClipper, sortable columns (single + multi), inline editing (text, checkbox, combo, slider, color, custom, button), selection with vivid highlight + white text, keyboard navigation (Up/Down/Home/End/PageUp/PageDown), scroll-to-row, clip tooltips, freeze cols/rows, `copy_to_clipboard`, `snap_last_row`, `RingBuffer<T>` FIFO eviction, `MAX_TABLE_ROWS` (10,000,000) capacity | [docs/virtual_table.md](docs/virtual_table.md) |
| **`virtual_tree`** | Virtualized tree-table for up to 10M nodes — slab/arena with generational `NodeId`, flat view cache, multi-column, inline editing, sibling-scoped sorting, drag-and-drop, filter/search, tree lines, striped rows, icons, badges, lazy children loading, configurable per-instance capacity with optional FIFO eviction | [docs/virtual_tree.md](docs/virtual_tree.md) |
| **`tab_control`** | Modern tab controller (DevExpress XtraTabControl-inspired) — 3 styles (Pill, Underline, Square), pinned tabs, drag-reorder, scroll, overflow dropdown, hover-preview thumbnail, badges, status indicators (Active/Inactive/Warning/Error/Dirty/None), per-tab dot color, keyboard shortcuts (Ctrl+T/W/Tab/1..9), single-pass hit-test, zero per-frame allocations | [docs/tab_control.md](docs/tab_control.md) |
| **`node_graph`** | Visual node graph editor — pan/zoom, bezier/straight/orthogonal wires, 4 pin shapes, multi-select, rectangle selection, mini-map, snap-to-grid, wire yanking, frustum culling, stats overlay, context menus, node shadow, wire flow animation, LOD, smooth zoom | [docs/node_graph.md](docs/node_graph.md) |
| **`force_graph`** | Obsidian-style force-directed knowledge graph — Barnes-Hut O(N log N) physics, pan/zoom/box-select, sidebar (search, tag filter, depth focus, time-travel slider), minimap overlay, 6 node shapes, color modes (static/tag/community/PageRank/betweenness), SVG/DOT/Mermaid export, Louvain community detection, drag/pin, context menus | [docs/force_graph.md](docs/force_graph.md) |
| **`hex_viewer`** | Binary hex dump viewer — offset/hex/ASCII columns, color regions, data inspector, goto address, pattern search, selection, diff highlighting, hover byte tooltips with binary/octal/decimal display, configurable bytes-per-row, endianness control | [docs/hex_viewer.md](docs/hex_viewer.md) |
| **`timeline`** | Zoomable profiler timeline — nested spans, multi-track with collapse, flame graph view, markers, tooltips, pan/zoom with Shift+scroll, adaptive time ruler, color-by-duration/category/name modes, configurable track height | [docs/timeline.md](docs/timeline.md) |
| **`diff_viewer`** | Side-by-side and unified diff viewer — Myers diff algorithm (O((N+M)D)), synchronized scrolling, fold unchanged regions, hunk navigation, hover row highlights, hunk accent bars, +/- prefixes in unified mode, context line control | [docs/diff_viewer.md](docs/diff_viewer.md) |
| **`disasm_view`** | Disassembly viewer — virtual-list rendering of arbitrary instruction streams, branch-arrow overlay (up to 16 levels), syntax-coloring (mnemonic by flow class, operand by kind), breakpoint + bookmark + watchpoint gutter markers, keyboard navigation (Up/Down/PgUp/PgDn/Home/End/Enter), goto-address (G), navigation history (Alt+←/→), multi-selection, colour-coded context menu, block tinting, `DisasmDataProvider` + `VecDisasmProvider`. Config serializable via serde+ron. | [docs/disasm_view.md](docs/disasm_view.md) |
| **`property_inspector`** | Hierarchical property editor — 15+ value types (bool, i32/i64, f32/f64, String, Color3/4, Vec2/3/4, Enum, Flags, Object, Array), categories with collapse, search/filter, diff highlighting, nested objects with expand/collapse, type badges, hover highlights | [docs/property_inspector.md](docs/property_inspector.md) |
| **`toolbar`** | Configurable horizontal toolbar — buttons, toggles, separators, dropdowns, spacers, builder API, icon support, hover underline accent, window-hovered guard, flexible spacer layout | [docs/toolbar.md](docs/toolbar.md) |
| **`status_bar`** | Composable bottom status bar — left/center/right sections, status indicators (Success/Warning/Error/Info), progress bars, clickable items with events, tooltips, icon support, hover highlights, overlay variant (`render_overlay`) | [docs/status_bar.md](docs/status_bar.md) |
| **`icons`** | Material Design Icons v7.4 codepoint constants (7400+ icons) | |
| **`theme`** | Unified `Theme` enum — 5 built-in palettes (Dark/Light/Midnight/Solarized/Monokai), each owning the full stack (titlebar/nav/dialog/statusbar/ImGui style); legacy semantic color tokens retained | [docs/theme.md](docs/theme.md) |
| **`utils`** | Color packing (RGB/RGBA to u32), `calc_text_size` wrapper, clipboard helpers (copy/paste), SVG/DOT/Mermaid export utilities (`force_graph`), glob pattern matching (file manager) | |

## Stack

- **Rust 1.95** — edition 2024, let-chains, `is_some_and`, `AtomicU32`
- **dear-imgui-rs 0.11.0** — Dear ImGui v1.92.6 (docking branch)
- **dear-imgui-wgpu 0.11.0** / **dear-imgui-winit 0.11.0** — wgpu + winit integration
- **wgpu 29.0.1** — GPU rendering backend
- **winit 0.30.13** — window and event loop
- **windows-sys 0.61.2** — drive enumeration (Windows)
- **MDI webfont** for icons (`assets/materialdesignicons-webfont.ttf`)

## Project Structure

The repo is a Cargo workspace with two packages: the publishable
**library** (`crate/`) and a **`publish = false` demo runner**
(`examples-app/`). Plain `cargo build` / `cargo test` / `cargo
clippy` only touch the library — examples are opt-in via
`-p examples-app` or `--workspace`.

```
.
├── Cargo.toml                       Workspace root + shared profiles
├── crate/                           Publishable `dear-imgui-custom-mod` library
│   ├── Cargo.toml                   [package], features, deps, dev-deps, benches
│   ├── src/                         Library code (see tree below)
│   ├── tests/                       Integration tests
│   ├── benches/                     Criterion microbenchmarks
│   └── assets/                      Embedded fonts (Hack, JetBrainsMono, MDI)
└── examples-app/                    Dev-only demo runner (publish = false)
    ├── Cargo.toml                   [package], path-dep on ../crate
    ├── src/lib.rs                   Empty stub (Cargo target placeholder)
    └── examples/                    15 demo_*.rs files
```

`crate/src/` layout:

```
src/
  lib.rs                            Crate root + feature flags
  icons.rs                          MDI icon constants (7400+ codepoints)
  utils/
    color.rs                        RGBA packing helpers
    text.rs                         CalcTextSize wrapper
  app_window/
    mod.rs                          AppWindow + ApplicationHandler thin delegates
    startup.rs                      GPU + ImGui init (resumed body)
    dispatch.rs                     Per-frame event dispatch (window_event, about_to_wait)
    handler.rs                      AppHandler trait
    state.rs                        AppState (set_theme, keep_alive, exit, proxy)
    proxy.rs                        AppProxy (Send + Sync) — cross-thread wake()
    win32.rs                        Win32 glue (DWM, rounded corners, MinMax subclass, opacity)
    chrome/                         Pure-ImGui titlebar — buttons, drag, edge resize, glyphs
    config/
      mod.rs                        AppConfig struct + Default + preset constructors
      builders.rs                   with_* fluent builder methods
      fonts.rs                      FontChoice, FontLayer, GlyphRanges
      enums.rs                      WindowKind, BorderStyle, FormStyle, Position, RenderMode, …
      titlebar.rs                   Buttons, Chrome, ExtraButton, TitlebarConfig
      icon.rs                       WindowIcon
    gpu/                            wgpu + winit setup, ImGui IO wiring, surface management
    platform.rs                     hwnd_of(), set_titlebar_dark_mode()
  confirm_dialog/
    mod.rs                          render_confirm_dialog() — themed modal, DialogResult (#[must_use])
    config.rs                       DialogConfig, DialogIcon, ConfirmStyle (+ config.ron)
    theme.rs                        DialogColors
  notifications/
    mod.rs                          NotificationCenter — push/dismiss/render, 5-pass pipeline
    config.rs                       NotificationCenterConfig (+ config.ron)
    enums.rs                        Severity, Placement, Duration, AnimationKind
    notification.rs                 Notification builder, NotificationAction
    theme.rs                        NotificationColors (5 palettes)
    icons.rs                        Severity icons + × close glyph (font-independent draw-list)
  nav_panel/
    mod.rs                          render_nav_panel() + overlay variant, NavPanelResult (#[must_use])
    config.rs                       NavPanelConfig (+ config.ron)
    buttons.rs                      SubMenuItem, NavButton, NavItem
    enums.rs                        DockPosition, ButtonStyle, ActiveStyle
    state.rs                        NavPanelState — active, visible, animation, submenu
    theme.rs                        NavColors
  theme/
    mod.rs                          Theme enum, ALL, sub-palette resolvers, legacy tokens
    palettes.rs                     Palette structs for each theme
    dark.rs | light.rs | midnight.rs | solarized.rs | monokai.rs
  code_editor/
    mod.rs                          CodeEditor widget — render, input, drawing
    config.rs                       EditorConfig, ContextMenuConfig (+ config.ron)
    syntax_colors.rs                EditorTheme, SyntaxColors (8 colour palettes)
    font_setup.rs                   CODE_EDITOR_FONT_PTR, font install helpers
    buffer.rs                       TextBuffer — lines, cursor, selection, editing
    token.rs                        Token + TokenKind types
    tokenizer.rs                    Legacy tokenizer (Rust/TOML/RON/Hex)
    undo.rs                         UndoStack
    lang/                           Per-language tokenizer modules (9 languages)
  disasm_view/
    mod.rs                          DisasmView widget — virtual list, keyboard nav, bookmarks
    config.rs                       DisasmViewConfig, ColumnWidths (serde+ron, + config.ron)
    provider.rs                     DisasmDataProvider trait, InstructionEntry, VecDisasmProvider
    arrows.rs                       BranchArrow, compute_arrows, compute_arrows_clipped
    draw.rs                         Rendering pipeline (gutter, mnemonic, operand, arrows)
    input.rs                        Keyboard + mouse input, navigation history
    popup.rs                        Colour-coded context menu (nav/follow/bp/watchpoint/bookmark)
    tokens.rs                       Operand tokenizer for syntax colouring
  hex_viewer/
    mod.rs                          HexViewer widget — render, navigation, search, editing
    config.rs                       HexViewerConfig (serde+ron, + config.ron)
    provider.rs                     HexDataProvider, VecDataProvider, ColorRegion, ByteCategory
    nav_history.rs                  NavHistory — back/forward navigation
    undo.rs                         UndoEntry, UndoStack
    draw.rs                         Rendering pipeline (offset, hex, ASCII columns)
    input.rs                        Keyboard + mouse input
    search.rs                       Hex search engine (wildcard `??` support)
    popup.rs                        Context menu
  file_manager/
    mod.rs                          FileManager — public API, dialog modes
    config.rs                       FileManagerConfig, FileFilter, DialogMode (+ config.ron)
    render.rs                       ImGui rendering (breadcrumb, table, footer)
    entry.rs                        FsEntry with pre-computed display strings
    favorites.rs                    Favorites sidebar
    history.rs                      Back/forward navigation stack
  virtual_table/
    mod.rs                          VirtualTable<T> struct, rendering, selection
    config.rs                       TableConfig, SelectionMode, EditTrigger (+ config.ron)
    column.rs                       ColumnDef, CellEditor variants, clip tooltip
    row.rs                          VirtualTableRow trait, CellValue, CellStyle
    edit.rs                         Inline editing state machine
    sort.rs                         Sort state (multi-column)
    ring_buffer.rs                  Fixed-capacity O(1) ring buffer
  virtual_tree/
    mod.rs                          VirtualTree<T> widget, render loop, public API
    config.rs                       TreeConfig (+ config.ron)
    arena.rs                        TreeArena<T> — slab storage, NodeId, parent/children
    node.rs                         VirtualTreeNode trait, NodeIcon
    flat_view.rs                    FlatView — cached linearization for ListClipper
    sort.rs                         Sibling-scoped sort state
    filter.rs                       FilterState — search with auto-expand
    drag.rs                         DragDropState for node reparenting
  tab_control/
    mod.rs                          TabControl<T>, TabItem trait, public API
    config.rs                       TabControlConfig (serde+ron, + config.ron)
    types.rs                        TabId, TabStatus, Badge, CloseGlyph, TabStyle, TabAction
    colors.rs                       TabColors
    strings.rs                      TabStrings (String fields for RON round-trip)
    layout.rs                       compute_tab_width, layout constants, pinned-prefix repair
    render.rs                       Strip renderer, styles, events, popups
    tests.rs                        32 unit tests
  node_graph/
    mod.rs                          NodeGraph<T> struct, public API
    config.rs                       NodeGraphConfig, NgColors (+ config.ron)
    graph.rs                        Graph<T> — slab storage + HashSet<Wire>
    viewer.rs                       NodeGraphViewer<T> trait
    state.rs                        InteractionState, Viewport, selection
    types.rs                        NodeId, PinInfo, PinShape, GraphAction
    render/                         Rendering sub-modules (7 files)
  force_graph/
    mod.rs                          GraphViewer widget — public API, event loop
    config.rs                       ViewerConfig, ForceConfig, ColorMode (+ config.ron)
    data.rs                         GraphData, NodeId/EdgeId (SlotMap)
    style.rs                        NodeStyle, EdgeStyle, NodeKind, GraphColors
    event.rs                        GraphEvent — typed event stream
    filter.rs                       FilterState — tag/search/depth/time-travel
    sim/                            Barnes-Hut O(N log N) physics
    layout/                         Spiral + Louvain community layout
    metrics/                        PageRank, betweenness centrality
    render/                         Draw pipeline, camera, minimap, export
    sidebar.rs                      Sidebar — filters, color groups, display, export
  timeline/
    mod.rs                          Timeline widget — tracks, spans, markers, ruler
    config.rs                       TimelineConfig, TimelineColors (+ config.ron)
    span.rs                         Span data type
  diff_viewer/
    mod.rs                          DiffViewer widget — side-by-side/unified modes
    config.rs                       DiffViewerConfig (+ config.ron)
    diff.rs                         Myers diff algorithm, hunk grouping
  property_inspector/
    mod.rs                          PropertyInspector widget — categories, properties
    config.rs                       InspectorConfig (+ config.ron)
    value.rs                        PropertyValue enum (15+ types)
  toolbar/
    mod.rs                          Toolbar widget — buttons, toggles, dropdowns
    config.rs                       ToolbarConfig (+ config.ron)
  status_bar/
    mod.rs                          StatusBar widget — items, indicators, progress
    config.rs                       StatusBarConfig, Alignment (+ config.ron)

examples/
  demo_app_window.rs                AppWindow — all 4 window kinds, themes, nav+status, dialogs
  demo_nav_panel.rs                 NavPanel + StatusBar — full config panel, all dock positions
  demo_code_editor.rs               CodeEditor — syntax highlighting, themes, MDI fonts
  demo_tab_control.rs               TabControl — styles, pinned tabs, badges, nested tabs
  demo_disasm_view.rs               DisasmView — branch arrows, breakpoints, bookmarks, RON export
  demo_hex_viewer.rs                HexViewer — PE header overlay, wildcard search, undo/redo
  demo_file_manager.rs              FileManager — open/save/folder dialogs, favorites
  demo_table.rs                     VirtualTable — 10K rows, inline editing, multi-sort
  demo_tree.rs                      VirtualTree — nested nodes, drag-drop, filter/search
  demo_node_graph.rs                NodeGraph — bezier wires, minimap, snap-to-grid
  demo_force_graph.rs               ForceGraph — Obsidian-style knowledge graph, physics
  demo_timeline.rs                  Timeline — 4 tracks, 50+ spans, markers
  demo_diff_viewer.rs               DiffViewer — 4 sample datasets, side-by-side + unified
  demo_property_inspector.rs        PropertyInspector — 5 categories, 20+ typed properties
  demo_status_toolbar.rs            Toolbar + StatusBar — combined demo with events
```

## Quick Start

### AppWindow

```rust
use dear_imgui_custom_mod::app_window::{AppConfig, AppHandler, AppState, AppWindow};
use dear_imgui_rs::Ui;

struct MyApp;

impl AppHandler for MyApp {
    fn render(&mut self, ui: &Ui, _state: &mut AppState) {
        ui.window("Hello").build(|| {
            ui.text("Hello from AppWindow!");
        });
    }
}

fn main() {
    AppWindow::new(AppConfig::main("My App", 1024.0, 768.0))
        .run(MyApp)
        .expect("event loop error");
}
```

### Borderless Window — use the bundled `AppWindow` host

The custom titlebar (drag, edge resize, minimize / maximize / close,
icon, extra buttons, all themes, close-confirm mode) lives **inside**
`app_window` — there is no separate `borderless_window` crate path
anymore. See [docs/app_window.md](docs/app_window.md) for the full
config builder reference and the four window-kind presets
(`splash` / `tool` / `dialog` / `main`).

`nav_panel` and `status_bar` keep matching `render_nav_panel_overlay`
and `StatusBar::render_overlay` entry points for foreground-draw-list
composition over a host window.

### Node Graph

```rust
use dear_imgui_custom_mod::node_graph::*;

let mut ng: NodeGraph<MyNode> = NodeGraph::new("my_graph");
ng.add_node(MyNode::Add, [100.0, 100.0]);

for action in ng.render(&ui, &MyViewer) {
    match action {
        GraphAction::Connected(wire) => { ng.graph.connect(wire.out_pin, wire.in_pin); }
        GraphAction::Disconnected(wire) => { ng.graph.disconnect(wire.out_pin, wire.in_pin); }
        GraphAction::DeleteSelected => {
            for id in ng.selected() { ng.remove_node(id); }
        }
        _ => {}
    }
}
```

### File Manager

```rust
use dear_imgui_custom_mod::file_manager::{FileManager, FileFilter};

let mut fm = FileManager::new();
fm.open_file(None, vec![
    FileFilter::new("Rust Files (*.rs)", &["rs"]),
    FileFilter::all(),
]);

if fm.render(&ui) {
    if let Some(path) = &fm.selected_path {
        println!("Selected: {}", path.display());
    }
}
```

### Virtual Tree

```rust
use dear_imgui_custom_mod::virtual_tree::*;

let mut tree = VirtualTree::new("##tree", columns, TreeConfig::default());
let root = tree.insert_root(MyNode { name: "Root".into(), .. }).unwrap();
tree.insert_child(root, MyNode { name: "Child".into(), .. });

tree.render(&ui);
```

### Tab Control

```rust
use dear_imgui_custom_mod::tab_control::{TabControl, TabItem, TabAction};

struct MyTab { name: String }
impl TabItem for MyTab {
    fn title(&self) -> &str { &self.name }
    fn render_content(&mut self, ui: &Ui) { ui.text(&self.name); }
}

let mut tc: TabControl<MyTab> = TabControl::new("##my_tabs");
tc.add(MyTab { name: "First".into() });

if let Some(action) = tc.render(ui) {
    match action {
        TabAction::Activated(id)     => { /* tab clicked */ }
        TabAction::Closed(id)        => { /* tab closed */ }
        TabAction::AddRequested      => { tc.add(MyTab { name: "New".into() }); }
        _ => {}
    }
}
```

### Status Bar

```rust
use dear_imgui_custom_mod::status_bar::{StatusBar, StatusItem, Indicator};

let mut bar = StatusBar::new("##status");
bar.left(StatusItem::indicator("Connected", Indicator::Success));
bar.left(StatusItem::text("Ln 42, Col 15"));
bar.right(StatusItem::text("UTF-8"));
bar.right(StatusItem::text("Rust"));
// In render loop: bar.render(ui);
```

### Toolbar

```rust
use dear_imgui_custom_mod::toolbar::{Toolbar, ToolbarItem};

let mut toolbar = Toolbar::new("##toolbar");
toolbar.add(ToolbarItem::button("New", "Create new file"));
toolbar.add(ToolbarItem::separator());
toolbar.add(ToolbarItem::toggle("Bold", false, "Toggle bold"));
toolbar.add(ToolbarItem::spacer());
toolbar.add(ToolbarItem::button("Settings", "Open settings"));
// In render loop: let events = toolbar.render(ui);
```

### Diff Viewer

```rust
use dear_imgui_custom_mod::diff_viewer::DiffViewer;

let mut diff = DiffViewer::new("##diff");
diff.set_texts("old text...", "new text...");
// In render loop: diff.render(ui);
```

## Running the Demos

```bash
cargo run -p examples-app --example demo_app_window --release
cargo run -p examples-app --example demo_nav_panel --release
cargo run -p examples-app --example demo_code_editor --release
cargo run -p examples-app --example demo_tab_control --release
cargo run -p examples-app --example demo_disasm_view --release
cargo run -p examples-app --example demo_hex_viewer --release
cargo run -p examples-app --example demo_file_manager --release
cargo run -p examples-app --example demo_table --release
cargo run -p examples-app --example demo_tree --release
cargo run -p examples-app --example demo_node_graph --release
cargo run -p examples-app --example demo_force_graph --release
cargo run -p examples-app --example demo_timeline --release
cargo run -p examples-app --example demo_diff_viewer --release
cargo run -p examples-app --example demo_property_inspector --release
cargo run -p examples-app --example demo_status_toolbar --release
```

All demos load their fonts (Hack, JetBrains Mono, MDI icons) from
`crate/assets/` via `include_bytes!`-baked constants — no external
asset files at runtime.

## Design Principles

- **10M-scale performance** — virtual_tree and virtual_table handle up to 10,000,000 nodes/rows at 60 FPS (`MAX_TREE_NODES` / `MAX_TABLE_ROWS` = 10,000,000) with configurable per-instance capacity limits and optional FIFO eviction
- **Zero per-frame allocations** — scratch buffers, `mem::take`, raw pointers for borrow avoidance, `mem::replace` for zero-copy commits
- **Index-based action processing** — avoids borrow conflicts between reads and writes
- **Two-phase rendering** — collect targets immutably, then apply mutations
- **Generic trait-based API** — `PageItem`, `VirtualTableRow`, `VirtualTreeNode`, `NodeGraphViewer` for user-defined types
- **Slab/HashMap data structures** — O(1) insert, remove, and lookup where it matters
- **Fully configurable** — colors, strings, sizes, capacity limits, behavior toggles via config structs
- **Serializable configs** — every config struct derives `serde::Serialize + serde::Deserialize`; defaults embedded via `include_str!("config.ron")`; round-trip with `ron::to_string(&cfg)` / `ron::from_str(&s)`
- **DDD config split** — schema lives in `config.rs` (struct + serde derives), default values live in a sibling `config.ron` loaded by `Default::default()`. Composite sub-structs that hold a value-set get their own `.ron` (e.g. `buttons.ron`, `column_widths.ron`); parent ron inlines the same fields, drift-tests pin them in sync. See [docs/config_pattern.md](docs/config_pattern.md).
- **English + Russian localisation** — every standalone widget with user-visible UI (hex_viewer, disasm_view, code_editor, file_manager, tab_control, timeline, diff_viewer, nav_panel, force_graph — 9 of 9) ships an EN+RU catalogue in `crate::i18n`. Switch per-widget with `.with_locale(Locale::Ru)`; the choice round-trips through ron alongside every other display setting. See [docs/i18n.md](docs/i18n.md).
