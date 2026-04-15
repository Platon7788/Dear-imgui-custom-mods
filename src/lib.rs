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
//!
//! ## Utilities
//!
//! - [`borderless_window`] — Borderless window titlebar with dark/light themes,
//!   resize zones, drag, minimize/maximize/close, and custom extra buttons
//! - [`icons`] — Material Design Icons v7.4 constants (7,400+ icons)
//! - [`theme`] — Dark and Light color palettes with semantic tokens
//! - [`utils`] — Color packing (RGB/RGBA to u32), text measurement

pub mod app_window;
pub mod borderless_window;
pub mod code_editor;
pub mod diff_viewer;
pub mod disasm_view;
pub mod file_manager;
pub mod hex_viewer;
pub mod icons;
pub mod timeline;
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
