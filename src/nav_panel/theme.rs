//! Nav-panel color palette.
//!
//! [`NavColors`] is **defined in [`crate::theme::palettes`]** — the
//! crate-wide source of truth for theme tokens. This module re-exports
//! it so downstream code can keep using
//! `dear_imgui_custom_mod::nav_panel::NavColors` without changes.
//!
//! The theme selector is the unified [`crate::theme::Theme`] at top
//! level — retrieve the matching palette with
//! [`Theme::nav()`](crate::theme::Theme::nav). For custom palettes,
//! build a [`NavColors`] directly and pass it via
//! [`NavPanelConfig::with_colors`](super::config::NavPanelConfig::with_colors).

pub use crate::theme::NavColors;
