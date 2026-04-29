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
| **`property_inspector`** | Hierarchical property editor — 15+ value types (bool, i32/i64, f32/f64, String, Color3/4, Vec2/3/4, Enum, Flags, Object, Array), categories with collapse, search/filter, diff highlighting, nested objects with expand/collapse, type badges, hover highlights | [docs/property_inspector.md](docs/property_inspector.md) |
| **`proc_mon`** | Windows process monitor — `NtQuerySystemInformation` enumeration with reusable syscall buffer, WoW64 bitness cache, `foldhash` u32-keyed maps, status-based delta (direct `ProcStatus` equality — same mechanism as the `IMGUI_NXT` engine), row highlighting via `MonitorColors` (per-PID / per-name / self / suspended), canonical 4-column layout (`Name` stretch + fixed `PID`/`Bits`/`Status` pinned right), virtualized rendering (`virtual_table` integration), context-menu routing, case-insensitive search. Minimal 5-field `ProcessInfo`. Gated `proc_mon` feature (Windows only) | [docs/proc_mon.md](docs/proc_mon.md) |
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

```
src/
  lib.rs                            Crate root
  icons.rs                          MDI icon constants
  utils/
    color.rs                        RGBA packing helpers
    text.rs                         CalcTextSize wrapper
  app_window/
    mod.rs                          AppWindow + event loop, on_window_event, scale-factor font rebuild
    handler.rs                      AppHandler trait
    state.rs                        AppState (theme switch, keep_alive, proxy)
    proxy.rs                        AppProxy (Send + Sync) — cross-thread `wake()`
    win32.rs                        Self-contained Win32 glue (DWM, rounded corners, MinMax subclass, opacity)
    chrome/                         Pure-ImGui titlebar — buttons, drag, edge resize, glyphs
    config/                         AppConfig (Splash/Tool/Dialog/Main presets) + TitlebarConfig + RenderMode + FontStack
    gpu/                            wgpu+winit setup, ImGui IO wiring, surface management
    platform.rs                     hwnd_of(), set_titlebar_dark_mode() — OS helpers
  app_window/
    mod.rs                          AppWindow::run(), AppHandler trait, re-exports borderless types
    config.rs                       AppConfig builder, StartPosition
    state.rs                        AppState — set_theme(), exit(), toggle_maximized()
    gpu.rs                          wgpu + winit event loop, frame render, GPU init
    style.rs                        apply_imgui_style_for_theme() — full ImGui color palette
  confirm_dialog/
    mod.rs                          render_confirm_dialog() — themed modal dialog, DialogResult (#[must_use])
    config.rs                       DialogConfig (theme: Theme + colors_override), DialogIcon, ConfirmStyle
    theme.rs                        DialogColors (shared struct)
  notifications/
    mod.rs                          NotificationCenter — push/dismiss/render, 5-pass render pipeline, events
    config.rs                       Notification builder, Severity, Placement, Duration, AnimationKind, CenterConfig
    theme.rs                        NotificationColors (5 palettes: dark/light/midnight/solarized/monokai)
    icons.rs                        5 severity icons + × close glyph via DrawListMut (font-independent)
  nav_panel/
    mod.rs                          render_nav_panel() + render_nav_panel_overlay(), NavPanelResult (#[must_use])
    config.rs                       NavPanelConfig (theme: Theme + colors_override), NavButton, SubMenuItem, DockPosition
    state.rs                        NavPanelState — active, visible, animation, submenu
    theme.rs                        NavColors (shared struct)
  theme/
    mod.rs                          Theme enum, ALL, sub-palette resolvers, legacy color tokens
    dark.rs | light.rs | midnight.rs | solarized.rs | monokai.rs
                                    Per-theme full stacks (titlebar/nav/dialog/statusbar/ImGui style)
  code_editor/
    mod.rs                          CodeEditor widget — render, input, drawing
    buffer.rs                       TextBuffer — lines, cursor, selection, editing
    config.rs                       EditorConfig, SyntaxColors, Language, BuiltinFont
    token.rs                        Token and TokenKind types
    tokenizer.rs                    Legacy tokenizer (Rust/TOML/RON/Hex)
    undo.rs                         UndoStack with VecDeque and action grouping
    lang/                           Per-language tokenizer modules (9 languages)
  file_manager/
    mod.rs                          FileManager struct, public API
    config.rs                       DialogMode, FileFilter, FileManagerConfig
    render.rs                       ImGui rendering (drive bar, breadcrumb, table, footer)
    entry.rs                        FsEntry with pre-computed display strings
    favorites.rs                    Favorites sidebar
    history.rs                      Back/forward navigation stack
  virtual_table/
    mod.rs                          VirtualTable<T> struct, rendering, selection
    config.rs                       TableConfig, SelectionMode, EditTrigger
    column.rs                       ColumnDef builder, CellEditor variants, clip tooltip
    row.rs                          VirtualTableRow trait, CellValue, CellStyle
    edit.rs                         Inline editing state machine
    sort.rs                         Sort state (multi-column)
    ring_buffer.rs                  Fixed-capacity O(1) ring buffer
  virtual_tree/
    mod.rs                          VirtualTree<T> widget, render loop, public API
    arena.rs                        TreeArena<T> — slab storage, NodeId, parent/children
    node.rs                         VirtualTreeNode trait, NodeIcon
    config.rs                       TreeConfig (wraps TableConfig)
    flat_view.rs                    FlatView — cached linearization for ListClipper
    sort.rs                         Sibling-scoped sort state
    filter.rs                       FilterState — search with auto-expand
    drag.rs                         DragDropState for node reparenting
  tab_control/
    mod.rs                          TabControl<T>, TabItem trait, public API
    config.rs                       TabControlConfig, TabStyle, TabStatus, CloseGlyph, …
    layout.rs                       compute_tab_width, layout constants, pinned-prefix repair
    render.rs                       single-file renderer (strip, styles, events, popups)
    tests.rs                        32 unit tests
  node_graph/
    mod.rs                          NodeGraph<T> struct, public API
    graph.rs                        Graph<T> — slab storage + HashSet<Wire>
    viewer.rs                       NodeGraphViewer<T> trait
    config.rs                       NodeGraphConfig, NgColors
    state.rs                        InteractionState, Viewport, selection
    render/                         Rendering sub-modules (7 files, ~2400 lines total)
      mod.rs                        Main render entry point
      grid.rs                       Canvas grid
      nodes.rs                      Node frame, pin, body rendering
      wires.rs                      Wire routing and flow animation
      math.rs                       Geometry, obstacle avoidance, hit testing
      input.rs                      Mouse/keyboard input
      overlays.rs                   Stats overlay and minimap
    types.rs                        NodeId, PinInfo, PinShape, GraphAction
  force_graph/
    mod.rs                          GraphViewer widget — public API, event loop
    data.rs                         GraphData, NodeId/EdgeId (SlotMap backend)
    style.rs                        NodeStyle, EdgeStyle, NodeKind, GraphColors
    config.rs                       ViewerConfig, ForceConfig, ColorMode
    event.rs                        GraphEvent — typed event stream
    filter.rs                       FilterState — tag/search/depth/time-travel
    sim/                            Barnes-Hut O(N log N) physics simulation
    layout/                         Spiral + Louvain community layout
    metrics/                        PageRank, betweenness centrality
    render/                         Draw pipeline, camera, minimap, export
      mod.rs                        Main render entry — edges, nodes, LOD
      camera.rs                     Camera — pan/zoom/inertia/animation
      visibility.rs                 Visibility pass — filter + search-highlight
      minimap.rs                    160×100 minimap overlay with click-to-pan
      export.rs                     SVG / DOT (Graphviz) / Mermaid export
      groups.rs                     Color group resolution
      interaction.rs                Drag, box-select, keyboard, context menu
      labels.rs                     Label rendering with zoom-based fade
    sidebar.rs                      Sidebar — filters, color groups, display, export
  hex_viewer/
    mod.rs                          HexViewer widget — render, navigation, search
    config.rs                       HexViewerConfig
  timeline/
    mod.rs                          Timeline widget — tracks, spans, markers, ruler
    span.rs                         Span data type with validation
    config.rs                       TimelineConfig, TimelineColors
  diff_viewer/
    mod.rs                          DiffViewer widget — side-by-side/unified modes
    diff.rs                         Myers diff algorithm, hunk grouping
    config.rs                       DiffViewerConfig
  property_inspector/
    mod.rs                          PropertyInspector widget — categories, properties
    value.rs                        PropertyValue enum (15+ types)
    config.rs                       InspectorConfig
  toolbar/
    mod.rs                          Toolbar widget — buttons, toggles, dropdowns
    config.rs                       ToolbarConfig
  status_bar/
    mod.rs                          StatusBar widget — items, indicators, progress
    config.rs                       StatusBarConfig, Alignment
  proc_mon/
    mod.rs                          Feature-gate, Windows-only re-exports
    core.rs                         ProcessEnumerator — NT syscalls, WoW64 cache, status-based delta
    types.rs                        ProcessInfo (5 fields), ProcStatus, ProcessDelta, ColumnConfig (2 flags), MonitorColors, MonitorEvent
    config.rs                       MonitorConfig + default / minimal / all_columns presets
    ui.rs                           ProcessMonitor + ProcessRow — canonical 4-column layout (Name stretch + PID/Bits/Status fixed)
  demo/mod.rs                       Interactive showcase

examples/
  demo_code_editor.rs               CodeEditor demo (wgpu + winit)
  demo_tab_control.rs               TabControl demo (wgpu + winit)
  demo_file_manager.rs              FileManager demo
  demo_table.rs                     VirtualTable demo
  demo_node_graph.rs                NodeGraph demo
  demo_force_graph.rs               ForceGraph demo — Obsidian-style knowledge graph
  demo_knowledge_graph.rs           Knowledge graph demo via the `knowledge_graph` alias (`force_graph` re-export) — 50+ nodes, NodeKind shapes, sidebar, color modes, box-select
  demo_tree.rs                      VirtualTree demo
  demo_hex_viewer.rs                HexViewer demo — PE header, color regions
  demo_timeline.rs                  Timeline demo — 4 tracks, 50+ spans, markers
  demo_diff_viewer.rs               DiffViewer demo — 4 sample datasets, modes
  demo_property_inspector.rs        PropertyInspector demo — 5 categories, 20+ props
  demo_status_toolbar.rs            Toolbar + StatusBar combined demo with events
  demo_borderless.rs                BorderlessWindow standalone demo — all 5 themes, edge resize
  demo_nav_panel.rs                 NavPanel + StatusBar demo — full config panel, all positions
  demo_app_window.rs                AppWindow + Notifications demo — counter, theme picker, log panel, close confirm, all 5 toast severities, placement / animation combos, sticky / custom-color / action-button toasts
  demo_proc_mon.rs                  ProcessMonitor demo (Windows-only) — live NT-syscall enumeration, virtualized table, caller-drawn context menu (Copy PID / Details / Kill)
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
    AppWindow::new(AppConfig::new("My App", 1024.0, 768.0))
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
cargo run --example demo_app_window --release
cargo run --example demo_borderless --release
cargo run --example demo_nav_panel --release
cargo run --example demo_code_editor --release
cargo run --example demo_table --release
cargo run --example demo_tree --release
cargo run --example demo_tab_control --features "tab_control,app_window" --release
cargo run --example demo_file_manager --release
cargo run --example demo_node_graph --release
cargo run --example demo_force_graph --release
cargo run --example demo_knowledge_graph --features force_graph,app_window --release
cargo run --example demo_hex_viewer --release
cargo run --example demo_timeline --release
cargo run --example demo_diff_viewer --release
cargo run --example demo_property_inspector --release
cargo run --example demo_status_toolbar --release
cargo run --example demo_disasm_view --release
```

Some demos require `assets/materialdesignicons-webfont.ttf` for icons.

## Design Principles

- **10M-scale performance** — virtual_tree and virtual_table handle up to 10,000,000 nodes/rows at 60 FPS (`MAX_TREE_NODES` / `MAX_TABLE_ROWS` = 10,000,000) with configurable per-instance capacity limits and optional FIFO eviction
- **Zero per-frame allocations** — scratch buffers, `mem::take`, raw pointers for borrow avoidance, `mem::replace` for zero-copy commits
- **Index-based action processing** — avoids borrow conflicts between reads and writes
- **Two-phase rendering** — collect targets immutably, then apply mutations
- **Generic trait-based API** — `PageItem`, `VirtualTableRow`, `VirtualTreeNode`, `NodeGraphViewer` for user-defined types
- **Slab/HashMap data structures** — O(1) insert, remove, and lookup where it matters
- **Fully configurable** — colors, strings, sizes, capacity limits, behavior toggles via config structs
