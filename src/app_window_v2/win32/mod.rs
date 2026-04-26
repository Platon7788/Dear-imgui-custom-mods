//! Win32-specific helpers for the v2 borderless window.
//!
//! All public items here are `#[cfg(windows)]` — the entire module is
//! only compiled on Windows. The parent module provides cross-platform
//! shims that no-op away on other targets.

#![cfg(windows)]
#![allow(missing_docs)]

pub mod dwm;
pub mod monitor;
pub mod subclass;
