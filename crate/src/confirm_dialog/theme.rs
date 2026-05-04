//! Confirm-dialog color palette.
//!
//! [`DialogColors`] is **defined in [`crate::theme::palettes`]** — the
//! crate-wide source of truth for theme tokens. This module re-exports
//! it so downstream code can keep using
//! `dear_imgui_custom_mod::confirm_dialog::DialogColors` without changes.
//!
//! The theme selector is the unified [`crate::theme::Theme`] at top
//! level — retrieve the matching palette with
//! [`Theme::dialog()`](crate::theme::Theme::dialog). For custom palettes,
//! build a [`DialogColors`] directly and pass it via
//! [`DialogConfig::with_colors`](super::config::DialogConfig::with_colors).

pub use crate::theme::DialogColors;
