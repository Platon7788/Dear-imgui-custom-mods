//! # dear-imgui-custom-mod
//!
//! Production-ready custom UI component library for `dear-imgui-rs`.
//!
//! Zero per-frame allocations, Rust 2024 edition, fully themeable via a
//! single crate-wide [`theme::Theme`] enum (5 variants: Dark, Light,
//! Midnight, Solarized, Monokai).
//!
//! ## Components
//!
//! - [`file_manager`] — Universal file/folder picker dialog (SelectFolder, OpenFile, SaveFile)
//!   with breadcrumb navigation, favorites sidebar, history, type-to-search, file filters
//! - [`virtual_table`] — Virtualized table for 100k+ rows with ListClipper, sortable columns,
//!   inline editing (text, checkbox, combo, slider, color, custom), multi-select
//! - [`virtual_tree`] — Virtualized tree-table for 500k+ nodes with drag-drop, filter,
//!   tree lines, keyboard navigation
//! - [`page_control`] — Generic tabbed container with Dashboard (tile grid) and Tabs
//!   (4 styles: Pill, Underline, Card, Square) views, close confirmation, badges, keyboard nav
//! - [`node_graph`] — Visual node graph editor with pan/zoom, bezier wires, pin shapes,
//!   multi-select, rectangle selection, mini-map, snap-to-grid
//! - [`code_editor`] — Full code editor with token-level syntax highlighting (Rust/TOML/RON),
//!   line numbers, cursor/selection, undo/redo, bracket matching, error markers
//! - [`hex_viewer`] — Binary hex dump viewer with offset/hex/ASCII columns, color regions,
//!   data inspector, goto, search, selection, diff highlighting
//! - [`disasm_view`] — Disassembly view with syntax highlighting
//! - [`timeline`] — Zoomable profiler timeline with nested spans, multi-track, flame graph,
//!   markers, tooltips, pan/zoom, adaptive time ruler
//! - [`status_bar`] — Composable bottom status bar (supports [`StatusBar::render_overlay`]
//!   for host-window-less rendering via the foreground draw list)
//! - [`toolbar`] — Configurable horizontal toolbar with buttons, toggles,
//!   separators, dropdowns, spacers, builder API
//! - [`diff_viewer`] — Side-by-side/unified diff viewer with Myers algorithm,
//!   synchronized scrolling, fold unchanged, hunk navigation
//! - [`property_inspector`] — Hierarchical property editor with 15+ value types,
//!   categories, search/filter, diff highlighting, nested objects
//! - [`nav_panel`] — Modern navigation panel (activity bar) with 3 dock positions,
//!   auto-hide/show, slide animation, submenu flyouts, badges. Has an overlay variant
//!   (`render_nav_panel_overlay`) for chrome-bar composition without host windows.
//!
//! ## Dialogs & Window chrome
//!
//! - [`confirm_dialog`] — Reusable modal confirmation dialog with 4 draw-list icon
//!   types (Warning/Error/Info/Question), dim overlay, keyboard shortcuts,
//!   destructive/normal button styles, builder-pattern [`confirm_dialog::DialogConfig`]
//! - [`borderless_window`] — Borderless window titlebar with resize zones, drag,
//!   minimize/maximize/close, extra buttons, `hwnd_of()` utility, DWM dark mode,
//!   Win11/Win10 rounded corners, cursor + resize-direction helpers. Has overlay
//!   variant ([`borderless_window::render_titlebar_overlay`]) for foreground-draw-list
//!   rendering without a host window.
//! - [`app_window`] — Zero-boilerplate window (wgpu+winit+ImGui); re-exports [`theme::Theme`]
//! - [`input`] — Keyboard / IME fixes for `dear-imgui-winit`: layout-independent
//!   `Ctrl+C` on Cyrillic/French/German layouts, numpad text injection, IME commit
//!
//! ## Resources
//!
//! - [`icons`] — Material Design Icons v7.4 constants (7,400+ icons)
//! - [`theme`] — Unified [`theme::Theme`] enum + per-theme palette modules
//! - [`fonts`] — Shared TTF blobs (Hack, JetBrains Mono, JetBrains Mono NL) and
//!   font installers (monospace / UI / MDI icon merge)
//! - [`utils`] — Color packing (RGB/RGBA to u32), text measurement
//!
//! ## Re-exports
//!
//! The three foundational GUI crates are re-exported so downstream users have a
//! single source of truth for version pinning — your `Cargo.toml` doesn't need
//! to track `dear-imgui-rs` / `wgpu` / `winit` separately.
//!
//! ```ignore
//! use dear_imgui_custom_mod::{dear_imgui_rs, wgpu, winit};
//! ```

// ─── Crate-level lints ───────────────────────────────────────────────────────
//
// `missing_docs` is ON crate-wide so any NEW public item surfaces a warning.
// Existing modules that haven't been documented yet carry a module-level
// `#![allow(missing_docs)]` — turn that into `#![warn(missing_docs)]` to
// drive a per-module doc-coverage pass. This way the lint actively helps
// without drowning the build in 8000 pre-existing warnings.
//
// `unreachable_pub` catches accidental public re-exports — the most common
// source of silent API leaks from internal helpers. Kept at warn, not deny,
// so intentional re-exports from `lib.rs` compile without per-item allow.
//
// We deliberately do NOT `forbid(unsafe_code)` — `borderless_window::platform`
// uses `unsafe` for documented Win32 calls (DWM dark mode, SetCursor bypass,
// SetWindowRgn fallback on Win10). Every such block carries a `// SAFETY:`
// comment.
#![warn(missing_docs)]
#![warn(unreachable_pub)]
// Pre-existing rustdoc issues across several modules (broken intra-doc
// links, redundant link targets, private-item links, one bare URL) are
// demoted from error to warn so `RUSTDOCFLAGS=-D warnings` does not block
// releases. Scheduled fix is the module-level doc-coverage pass — see
// CONTRIBUTING.md.
#![allow(rustdoc::broken_intra_doc_links)]
#![allow(rustdoc::private_intra_doc_links)]
#![allow(rustdoc::redundant_explicit_links)]
#![allow(rustdoc::bare_urls)]

// ─── Re-exports of foundational GUI crates ───────────────────────────────────
//
// Downstream consumers get these types "for free" with their
// `dear-imgui-custom-mod` dep, and the versions are always in lock-step with
// what this crate was compiled against — no more two-Cargo-file pin drift.

/// Re-export of [`dear_imgui_rs`] — the Dear ImGui Rust binding this crate is built on.
pub use dear_imgui_rs;
/// Re-export of [`dear_imgui_wgpu`] — the wgpu renderer backend for Dear ImGui.
pub use dear_imgui_wgpu;
/// Re-export of [`dear_imgui_winit`] — the winit platform backend for Dear ImGui.
pub use dear_imgui_winit;
/// Re-export of [`wgpu`] — the GPU abstraction backing [`dear_imgui_wgpu`].
pub use wgpu;
/// Re-export of [`winit`] — the window / event loop backing [`dear_imgui_winit`].
pub use winit;

// ─── Infrastructure modules (always compiled) ────────────────────────────────
//
// These are used by most components and carry little weight on their own.
// Gating them behind features would force every leaf-component flag to
// depend on them, adding noise to `Cargo.toml` for no payoff.

pub mod fonts;
pub mod icons;
pub mod input;
pub mod theme;
pub mod utils;

// ─── Component modules (gated behind features) ───────────────────────────────

#[cfg(feature = "app_window")]
pub mod app_window;
#[cfg(feature = "borderless_window")]
pub mod borderless_window;
#[cfg(feature = "code_editor")]
pub mod code_editor;
#[cfg(feature = "confirm_dialog")]
pub mod confirm_dialog;
#[cfg(feature = "diff_viewer")]
pub mod diff_viewer;
#[cfg(feature = "disasm_view")]
pub mod disasm_view;
#[cfg(feature = "file_manager")]
pub mod file_manager;
#[cfg(feature = "force_graph")]
pub mod force_graph;
#[cfg(feature = "hex_viewer")]
pub mod hex_viewer;
#[cfg(feature = "nav_panel")]
pub mod nav_panel;
#[cfg(feature = "node_graph")]
pub mod node_graph;
#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "page_control")]
pub mod page_control;
#[cfg(feature = "proc_mon")]
pub mod proc_mon;
#[cfg(feature = "property_inspector")]
pub mod property_inspector;
#[cfg(feature = "status_bar")]
pub mod status_bar;
#[cfg(feature = "timeline")]
pub mod timeline;
#[cfg(feature = "toolbar")]
pub mod toolbar;
#[cfg(feature = "virtual_table")]
pub mod virtual_table;
#[cfg(feature = "virtual_tree")]
pub mod virtual_tree;
#[cfg(feature = "force_graph")]
pub use force_graph as knowledge_graph; // backwards-compat alias

// ─── Demo helpers (internal; only compiled when `full` is on) ────────────────
//
// `demo/` references many components, so it needs every feature they bring.
// Keep it gated behind `full` — it's just internal test scaffolding.

#[cfg(feature = "full")]
pub mod demo;
