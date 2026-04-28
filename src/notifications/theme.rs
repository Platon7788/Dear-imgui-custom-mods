//! Notification-center color palette.
//!
//! [`NotificationColors`] is **defined in [`crate::theme::palettes`]** —
//! the crate-wide source of truth for theme tokens. This module
//! re-exports it (along with its `dark()`/`light()`/`midnight()`/
//! `solarized()`/`monokai()` preset constructors) so downstream code can
//! keep using `dear_imgui_custom_mod::notifications::NotificationColors`
//! without changes.
//!
//! The full palette is picked via
//! [`Theme::notifications`](crate::theme::Theme::notifications) — for
//! custom looks, build a [`NotificationColors`] directly and pass it
//! via [`CenterConfig::with_colors`](super::config::CenterConfig::with_colors).

pub use crate::theme::NotificationColors;
