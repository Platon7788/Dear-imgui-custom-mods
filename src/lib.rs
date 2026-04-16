//! # dear-imgui-custom-mod
//!
//! Production-ready custom UI component library for `dear-imgui-rs`.
//!
//! Zero per-frame allocations, modern Rust 2024 edition, fully themeable.
//!
//! ## Components
//!
//! - [`file_manager`] — Universal file/folder picker dialog (SelectFolder, OpenFile, SaveFile)
//!   with breadcrumb navigation, favorites sidebar, history, type-to-search, file filters
//! - [`virtual_table`] — Virtualized table for 100k+ rows with ListClipper, sortable columns,
//!   inline editing (text, checkbox, combo, slider, color, custom), multi-select
//! - [`page_control`] — Generic tabbed container with Dashboard (tile grid) and Tabs
//!   (4 styles: Pill, Underline, Card, Square) views, close confirmation, badges, keyboard nav
//! - [`node_graph`] — Visual node graph editor with pan/zoom, bezier wires, pin shapes,
//!   multi-select, rectangle selection, mini-map, snap-to-grid
//! - [`code_editor`] — Full code editor with token-level syntax highlighting (Rust/TOML/RON),
//!   line numbers, cursor/selection, undo/redo, bracket matching, error markers
//! - [`hex_viewer`] — Binary hex dump viewer with offset/hex/ASCII columns, color regions,
//!   data inspector, goto, search, selection, diff highlighting
//! - [`timeline`] — Zoomable profiler timeline with nested spans, multi-track, flame graph,
//!   markers, tooltips, pan/zoom, adaptive time ruler
//! - [`status_bar`] — Composable bottom status bar with left/center/right sections,
//!   indicators, progress bars, clickable items, tooltips
//! - [`toolbar`] — Configurable horizontal toolbar with buttons, toggles,
//!   separators, dropdowns, spacers, builder API
//! - [`diff_viewer`] — Side-by-side/unified diff viewer with Myers algorithm,
//!   synchronized scrolling, fold unchanged, hunk navigation
//! - [`property_inspector`] — Hierarchical property editor with 15+ value types,
//!   categories, search/filter, diff highlighting, nested objects
//! - [`nav_panel`] — Modern navigation panel (activity bar) with 4 dock positions,
//!   auto-hide/show, slide animation, submenu flyouts, badges, 6 themes
//!
//! ## Dialogs & Utilities
//!
//! - [`confirm_dialog`] — Reusable modal confirmation dialog with 6 themes,
//!   4 draw-list icon types (Warning/Error/Info/Question), dim overlay, keyboard shortcuts,
//!   green Cancel / red Confirm buttons, builder-pattern `DialogConfig`
//! - [`borderless_window`] — Borderless window titlebar with 6 themes,
//!   resize zones, drag, minimize/maximize/close, extra buttons, `hwnd_of()` utility,
//!   DWM dark mode, Win11/Win10 rounded corners, cursor + resize-direction helpers
//! - [`app_window`] — Zero-boilerplate window (wgpu+winit+ImGui); re-exports borderless types
//! - [`input`] — Keyboard / IME fixes for `dear-imgui-winit`: layout-independent
//!   `Ctrl+C` on Cyrillic/French/German layouts, numpad text injection, IME commit
//! - [`icons`] — Material Design Icons v7.4 constants (7,400+ icons)
//! - [`theme`] — Dark and Light color palettes with semantic tokens
//! - [`utils`] — Color packing (RGB/RGBA to u32), text measurement

pub mod app_window;
pub mod borderless_window;
pub mod code_editor;
pub mod confirm_dialog;
pub mod diff_viewer;
pub mod disasm_view;
pub mod file_manager;
pub mod hex_viewer;
pub mod icons;
pub mod input;
pub mod timeline;
pub mod nav_panel;
pub mod node_graph;
pub mod page_control;
pub mod property_inspector;
pub mod status_bar;
pub mod theme;
pub mod toolbar;
pub mod utils;
pub mod virtual_table;
pub mod virtual_tree;

pub mod demo;
