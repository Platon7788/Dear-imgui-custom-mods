//! Titlebar color palette — the colours part of the theme system.
//!
//! [`TitlebarColors`] is **defined in [`crate::theme::palettes`]** — the
//! crate-wide source of truth for theme tokens. This module re-exports
//! it so downstream code can keep using
//! `dear_imgui_custom_mod::borderless_window::TitlebarColors` without
//! changes.
//!
//! The selector enum lives at top level as [`crate::theme::Theme`] —
//! every built-in theme exposes its titlebar palette through
//! [`Theme::titlebar()`](crate::theme::Theme::titlebar). Custom palettes
//! are built by constructing a [`TitlebarColors`] directly and handing it
//! to [`BorderlessConfig::with_colors`](super::config::BorderlessConfig::with_colors).

pub use crate::theme::TitlebarColors;
