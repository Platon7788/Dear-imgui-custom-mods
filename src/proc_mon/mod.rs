//! # Process Monitor Module
//!
//! Production-ready process monitoring component with NT syscall enumeration
//! and Dear ImGui virtualized table display.
//!
//! ## Features
//!
//! - **Zero-allocation enumeration**: Reusable syscall buffer, cached bitness
//! - **Delta updates**: Only new/changed/removed processes
//! - **Full metrics**: Memory, threads, handles, I/O
//! - **Suspended detection**: Thread state analysis
//! - **Virtualized rendering**: Handles 10,000+ processes at 60 FPS
//! - **Search filter**: Zero-allocation case-insensitive search
//! - **Fixed sort**: By create_time (newest first)
//!
//! ## Architecture
//!
//! ```
//! proc_mon/
//! ├── mod.rs        # Public API re-exports
//! ├── types.rs      # ProcessInfo, ProcStatus, ProcessDelta, ColumnConfig
//! ├── core.rs       # Syscall enumerator
//! ├── config.rs     # MonitorConfig
//! └── ui.rs         # VirtualTable monitor window
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::proc_mon::{
//!     ProcessEnumerator, ProcessMonitor, MonitorConfig,
//! };
//!
//! // Create enumerator and UI monitor
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
//!             // Render your own context menu
//!             ui.popup("##ctx", || {
//!                 if ui.button("Kill") { /* ... */ }
//!             });
//!         }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Column Configuration
//!
//! ```rust,ignore
//! let config = MonitorConfig {
//!     columns: ColumnConfig {
//!         memory: true,
//!         threads: true,
//!         handles: true,
//!         ..ColumnConfig::default()
//!     },
//!     ..Default::default()
//! };
//! ```

#![allow(missing_docs)]
#![cfg(windows)] // Process monitoring is Windows-only

pub mod config;
pub mod core;
pub mod types;
pub mod ui;

// ─── Re-exports ──────────────────────────────────────────────────────────────

pub use config::MonitorConfig;
pub use core::{Error, ProcessEnumerator};
pub use types::{
    format_bytes, format_cpu_percent, format_cpu_time, format_create_time, ColumnConfig,
    MonitorEvent, ProcStatus, ProcessDelta, ProcessInfo,
};
pub use ui::{ProcessMonitor, ProcessRow};
