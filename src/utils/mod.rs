//! Shared utility helpers for custom ImGui components.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub(crate) mod clipboard;
pub mod color;
pub mod export;
pub mod glob;
pub mod text;

pub use color::pack_color_f32;
