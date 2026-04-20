# Dear ImGui Custom Mod

Production-ready custom UI component library for `dear-imgui-rs`.

Zero per-frame allocations, modern Rust 2024 edition, fully themeable.

## Components

### Window Infrastructure

| Component | Description | Docs |
|-----------|-------------|------|
| **`borderless_window`** | Reusable borderless-window titlebar — 5 built-in themes (Dark, Light, Midnight, Solarized, Monokai) via the unified `Theme` enum + per-instance `colors_override`, minimize/maximize/close buttons, 8-direction edge resize, drag-to-move, close-confirmation mode, optional focus-dim, drag-hint, separator, icon, extra buttons, `IconClick` action, overlay variant (`render_titlebar_overlay`) | [docs/borderless_window.md](docs/borderless_window.md) |
| **`app_window`** | Zero-boilerplate application window — `AppWindow::run()` + `AppHandler` trait replaces ~300 lines of wgpu/winit/ImGui setup. Auto GPU backend (DX12→Vulkan→GL), auto HiDPI, FPS cap, `StartPosition`, atomic theme switching via `AppState::set_theme(Theme)` | [docs/app_window.md](docs/app_window.md) |
| **`nav_panel`** | Modern navigation panel (activity bar) — 3 docking positions (Left/Right/Top), flyout submenus, auto-hide with slide animation, toggle arrow, badges, button spacing/separators, per-button tooltip control, 5 unified themes, overlay variant (`render_nav_panel_overlay`) | [docs/nav_panel.md](docs/nav_panel.md) |
| **`confirm_dialog`** | Reusable modal confirmation dialog — 5 unified themes + `colors_override`, 4 draw-list icon types (Warning/Error/Info/Question), dim overlay, Esc/Enter keyboard shortcuts, green Cancel / red Confirm buttons, builder-pattern `DialogConfig` | [docs/confirm_dialog.md](docs/confirm_dialog.md) |
| **`notifications`** | Modern toast-notification center — 5 severity levels (Info/Success/Warning/Error/Debug) with draw-list icons, 6 stack placements (4 corners + top/bottom center), auto-dismiss timer with bottom progress bar, pause-on-hover, Fade/SlideIn/None animations, action buttons with caller-defined ids, manual `×` close, per-toast custom accent override, max-visible cap, 5 unified themes + `colors_override` | [docs/notifications.md](docs/notifications.md) |

### UI Widgets

| Component | Description | Docs |
|-----------|-------------|------|
| **`code_editor`** | Full-featured code editor — 10 languages (Rust, TOML, RON, Rhai, JSON, YAML, XML, ASM, Hex, Custom), 6 themes, 3 built-in fonts (Hack, JetBrains Mono), code folding, word wrap, find/replace, multi-cursor, undo/redo, breakpoints, error markers, smooth scrolling | [docs/code_editor.md](docs/code_editor.md) |
| **`file_manager`** | Universal file/folder picker dialog — SelectFolder, OpenFile, SaveFile modes. Breadcrumb navigation, favorites sidebar, back/forward history, type-to-search, file filters, overwrite confirmation | [docs/file_manager.md](docs/file_manager.md) |
| **`virtual_table`** | Virtualized table for up to 1M rows — ListClipper, sortable columns, inline editing (text, checkbox, combo, slider, color, custom), selection with vivid highlight + white text, keyboard navigation (Up/Down/Home/End/PageUp/PageDown), scroll-to-row, clip tooltips, auto-fit columns, `RingBuffer<T>` FIFO eviction, `MAX_TABLE_ROWS` capacity | [docs/virtual_table.md](docs/virtual_table.md) |
| **`virtual_tree`** | Virtualized tree-table for up to 1M nodes — slab/arena with generational `NodeId`, flat view cache, multi-column, inline editing, sibling-scoped sorting, drag-and-drop, filter/search, tree lines, striped rows, icons, badges, configurable capacity with optional FIFO eviction | [docs/virtual_tree.md](docs/virtual_tree.md) |
| **`page_control`** | Generic tabbed container — Dashboard (tile grid) and Tabs (4 styles: Pill, Underline, Card, Square) views. Close confirmation, badges, status indicators, keyboard navigation | [docs/page_control.md](docs/page_control.md) |
| **`node_graph`** | Visual node graph editor — pan/zoom, bezier/straight/orthogonal wires, 4 pin shapes, multi-select, rectangle selection, mini-map, snap-to-grid, wire yanking, frustum culling, stats overlay, context menus, node shadow, wire flow animation, LOD, smooth zoom | [docs/node_graph.md](docs/node_graph.md) |
| **`hex_viewer`** | Binary hex dump viewer — offset/hex/ASCII columns, color regions, data inspector, goto address, pattern search, selection, diff highlighting, hover byte tooltips with binary/octal/decimal display, configurable bytes-per-row, endianness control | [docs/hex_viewer.md](docs/hex_viewer.md) |
| **`timeline`** | Zoomable profiler timeline — nested spans, multi-track with collapse, flame graph view, markers, tooltips, pan/zoom with Shift+scroll, adaptive time ruler, color-by-duration/category/name modes, configurable track height | [docs/timeline.md](docs/timeline.md) |
| **`diff_viewer`** | Side-by-side and unified diff viewer — Myers diff algorithm (O((N+M)D)), synchronized scrolling, fold unchanged regions, hunk navigation, hover row highlights, hunk accent bars, +/- prefixes in unified mode, context line control | [docs/diff_viewer.md](docs/diff_viewer.md) |
| **`property_inspector`** | Hierarchical property editor — 15+ value types (bool, i32/i64, f32/f64, String, Color3/4, Vec2/3/4, Enum, Flags, Object, Array), categories with collapse, search/filter, diff highlighting, nested objects with expand/collapse, type badges, hover highlights | [docs/property_inspector.md](docs/property_inspector.md) |
| **`toolbar`** | Configurable horizontal toolbar — buttons, toggles, separators, dropdowns, spacers, builder API, icon support, hover underline accent, window-hovered guard, flexible spacer layout | [docs/toolbar.md](docs/toolbar.md) |
| **`status_bar`** | Composable bottom status bar — left/center/right sections, status indicators (Success/Warning/Error/Info), progress bars, clickable items with events, tooltips, icon support, hover highlights, overlay variant (`render_overlay`) | [docs/status_bar.md](docs/status_bar.md) |
| **`icons`** | Material Design Icons v7.4 codepoint constants (7400+ icons) | |
| **`theme`** | Unified `Theme` enum — 5 built-in palettes (Dark/Light/Midnight/Solarized/Monokai), each owning the full stack (titlebar/nav/dialog/statusbar/ImGui style); legacy semantic color tokens retained | [docs/theme.md](docs/theme.md) |
| **`utils`** | Color packing (RGB/RGBA to u32), `calc_text_size` wrapper | |

## Stack

- **Rust 1.94** — edition 2024, let-chains, `is_some_and`, `AtomicU32`
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
  borderless_window/
    mod.rs                          render_titlebar() + render_titlebar_overlay() — draw-list titlebar, edge resize, buttons
    config.rs                       BorderlessConfig (theme: Theme + colors_override), ButtonConfig, ExtraButton, CloseMode, TitleAlign
    theme.rs                        TitlebarColors (shared struct)
    actions.rs                      WindowAction, ResizeEdge, TitlebarResult (#[must_use])
    state.rs                        TitlebarState — focused, maximized, confirm_close()
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
  page_control/
    mod.rs                          PageControl<T>, PageItem trait
    config.rs                       PageControlConfig, TabStyle, PageAction
    render.rs                       Dashboard tiles, tab strip (4 styles)
    types.rs                        PageId, PageStatus, Badge, ContentView
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
  demo/mod.rs                       Interactive showcase

examples/
  demo_code_editor.rs               CodeEditor demo (wgpu + winit)
  demo_page_control.rs              PageControl demo (wgpu + winit)
  demo_file_manager.rs              FileManager demo
  demo_table.rs                     VirtualTable demo
  demo_node_graph.rs                NodeGraph demo
  demo_tree.rs                      VirtualTree demo
  demo_hex_viewer.rs                HexViewer demo — PE header, color regions
  demo_timeline.rs                  Timeline demo — 4 tracks, 50+ spans, markers
  demo_diff_viewer.rs               DiffViewer demo — 4 sample datasets, modes
  demo_property_inspector.rs        PropertyInspector demo — 5 categories, 20+ props
  demo_status_toolbar.rs            Toolbar + StatusBar combined demo with events
  demo_borderless.rs                BorderlessWindow standalone demo — all 5 themes, edge resize
  demo_nav_panel.rs                 NavPanel + StatusBar demo — full config panel, all positions
  demo_app_window.rs                AppWindow + Notifications demo — counter, theme picker, log panel, close confirm, all 5 toast severities, placement / animation combos, sticky / custom-color / action-button toasts
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

### Borderless Window (manual)

```rust
use dear_imgui_custom_mod::borderless_window::{
    BorderlessConfig, CloseMode, TitlebarState, WindowAction, render_titlebar,
};
use dear_imgui_custom_mod::theme::Theme;

let cfg = BorderlessConfig::new("My App")
    .with_theme(Theme::Solarized)
    .with_close_mode(CloseMode::Confirm);
let mut state = TitlebarState::new();

// Inside a full-screen zero-padding Dear ImGui window each frame:
let res = render_titlebar(ui, &cfg, &mut state);

if let Some(edge) = res.hover_edge {
    window.set_cursor(cursor_for_edge(edge));
}
match res.action {
    WindowAction::Close          => event_loop.exit(),
    WindowAction::CloseRequested => { /* show confirm dialog */ }
    WindowAction::Minimize       => window.set_minimized(true),
    WindowAction::Maximize       => window.set_maximized(!state.maximized),
    WindowAction::DragStart      => { window.drag_window().ok(); }
    WindowAction::ResizeStart(e) => { window.drag_resize_window(to_winit(e)).ok(); }
    _ => {}
}
```

Need a foreground-draw-list titlebar over your own windows instead of
inside a host ImGui window? Use `render_titlebar_overlay(ui, &cfg, &mut
state, origin, full_window_size)` — see [docs/borderless_window.md](docs/borderless_window.md).
`nav_panel` and `status_bar` have matching `render_nav_panel_overlay` and
`StatusBar::render_overlay` entry points.

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

### Page Control

```rust
use dear_imgui_custom_mod::page_control::{PageControl, PageItem};

let mut pc = PageControl::new();
pc.add(my_page);

if let Some(action) = pc.render(&ui) {
    match action {
        PageAction::Activated(id) => { /* tab clicked */ }
        PageAction::Closed(id) => { pc.remove(id); }
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
cargo run --example demo_nav_panel --release
cargo run --example demo_app_window --release
cargo run --example demo_borderless --release
cargo run --example demo_code_editor --release
cargo run --example demo_node_graph --release
cargo run --example demo_table --release
cargo run --example demo_tree --release
cargo run --example demo_page_control --release
cargo run --example demo_file_manager --release
cargo run --example demo_hex_viewer --release
cargo run --example demo_timeline --release
cargo run --example demo_diff_viewer --release
cargo run --example demo_property_inspector --release
cargo run --example demo_status_toolbar --release
```

Some demos require `assets/materialdesignicons-webfont.ttf` for icons.

## Design Principles

- **1M-scale performance** — virtual_tree and virtual_table handle up to 1,000,000 nodes/rows at 60 FPS with configurable capacity limits and optional FIFO eviction
- **Zero per-frame allocations** — scratch buffers, `mem::take`, raw pointers for borrow avoidance, `mem::replace` for zero-copy commits
- **Index-based action processing** — avoids borrow conflicts between reads and writes
- **Two-phase rendering** — collect targets immutably, then apply mutations
- **Generic trait-based API** — `PageItem`, `VirtualTableRow`, `VirtualTreeNode`, `NodeGraphViewer` for user-defined types
- **Slab/HashMap data structures** — O(1) insert, remove, and lookup where it matters
- **Fully configurable** — colors, strings, sizes, capacity limits, behavior toggles via config structs
