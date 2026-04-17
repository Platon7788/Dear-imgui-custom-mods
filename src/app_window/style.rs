//! Deprecated compatibility re-export for the Dear ImGui style applier.
//!
//! The entry point now lives as a method on [`crate::theme::Theme`]:
//!
//! ```rust,ignore
//! use dear_imgui_custom_mod::theme::Theme;
//! Theme::Dark.apply_imgui_style(&mut context.style_mut());
//! ```

use crate::theme::Theme;

/// Apply the Dear ImGui style for the given theme.
///
/// Thin wrapper kept for source compatibility with the previous
/// `dear_imgui_custom_mod::app_window::apply_imgui_style_for_theme(theme, style)`
/// call site. Prefer [`Theme::apply_imgui_style`] directly.
pub fn apply_imgui_style_for_theme(theme: Theme, style: &mut dear_imgui_rs::Style) {
    theme.apply_imgui_style(style);
}
