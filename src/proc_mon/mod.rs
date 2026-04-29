//! # Process Monitor Widget
//!
//! Dear-ImGui virtualized table with NT-syscall-driven enumeration.
//!
//! ## Architecture
//!
//! Split into two layers since v0.10:
//!
//! ```text
//! useful-lib/proc_enum         (this crate's `proc_enum` dep)
//! ├── ProcessEnumerator        — NT syscall driver
//! ├── ProcessInfo              — minimal 5-field per-process snapshot
//! ├── ProcessDelta             — incremental upsert/remove update
//! ├── ProcStatus               — Running / Suspended
//! └── Error                    — SyscallFailed / BufferTooLarge / NotSupported
//!
//! dear_imgui_custom_mod::proc_mon  (this module)
//! ├── ProcessMonitor           — VirtualTable widget
//! ├── ProcessRow               — VirtualTable row adapter
//! ├── MonitorEvent             — RowSelected / DoubleClicked / ContextMenuRequested
//! ├── ColumnConfig             — which columns to show
//! ├── MonitorColors            — row-highlight palette
//! └── MonitorConfig            — bundle of the above + interval / window title
//! ```
//!
//! Headless consumers (CLI tools, daemons, alternative-UI back-ends) depend
//! directly on `proc_enum`. The widget consumes the same data primitives,
//! so swapping render layers stays a one-crate change.
//!
//! ## Re-exports
//!
//! For backward compatibility every public path that worked in v0.9 still
//! works in v0.10+: the headless types are re-exported from
//! `proc_enum`, so call-sites such as
//! `dear_imgui_custom_mod::proc_mon::ProcessInfo` continue to resolve.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::proc_mon::{
//!     MonitorConfig, ProcessEnumerator, ProcessMonitor,
//! };
//!
//! let mut enumerator = ProcessEnumerator::new();
//! let mut monitor = ProcessMonitor::new(MonitorConfig::default());
//!
//! // In your main loop:
//! let delta = enumerator.enumerate_delta()?;
//! monitor.apply_delta(&delta);
//!
//! // Render
//! if let Some(event) = monitor.render(&ui, &mut show_monitor) {
//!     match event {
//!         MonitorEvent::ContextMenuRequested(pid) => {
//!             ui.popup("##ctx", || {
//!                 if ui.button("Kill") { /* ... */ }
//!             });
//!         }
//!         _ => {}
//!     }
//! }
//! ```

#![allow(missing_docs)]
#![cfg(windows)] // Process monitoring is Windows-only

pub mod config;
pub mod types;
pub mod ui;

// ─── Re-exports — headless data layer ────────────────────────────────────────
//
// These come from `useful-lib/proc_enum`. Re-exporting them from this
// module is a backward-compat layer: pre-extraction consumers wrote
// `dear_imgui_custom_mod::proc_mon::ProcessInfo` and we don't want to break
// their imports just because the type's home crate moved.

pub use proc_enum::{Error, ProcStatus, ProcessDelta, ProcessEnumerator, ProcessInfo};

// ─── Re-exports — widget layer ───────────────────────────────────────────────

pub use config::MonitorConfig;
pub use types::{ColumnConfig, MonitorColors, MonitorEvent};
pub use ui::{ProcessMonitor, ProcessRow};
