//! Shared utility helpers for custom ImGui components.

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub(crate) mod clipboard;
pub mod color;
pub mod export;
pub mod glob;
pub mod hex;
pub mod popup;
pub mod text;
pub mod tooltip;

pub use color::pack_color_f32;
pub use popup::{
    action_row, button_with_color, compact_popup_body, danger_button, selected_button,
    success_button, themed_popup_style,
};
pub use tooltip::themed_tooltip;
