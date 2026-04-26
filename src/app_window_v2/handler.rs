//! User-implemented [`AppHandlerV2`] trait.

use dear_imgui_rs::Ui;

use super::state::AppStateV2;
use crate::theme::Theme;

/// Implement this trait to provide your application's render logic.
///
/// All methods except [`render`](Self::render) have default implementations.
pub trait AppHandlerV2 {
    /// Called once per frame, inside the full-screen content area below
    /// the titlebar. ImGui cursor is at the top-left of the content area;
    /// `ui.content_region_avail()` reports the available space.
    fn render(&mut self, ui: &Ui, state: &mut AppStateV2);

    /// Called when the user clicks the close button with
    /// [`CloseMode::Confirm`](super::config::CloseMode::Confirm), or when
    /// the OS sends a close request (Alt+F4 / right-click → Close).
    ///
    /// Default: confirms immediately (`state.exit()`).
    fn on_close_requested(&mut self, state: &mut AppStateV2) {
        state.exit();
    }

    /// Called when a custom extra-button in the titlebar is clicked.
    fn on_extra_button(&mut self, _id: &'static str, _state: &mut AppStateV2) {}

    /// Called after the theme changes (via `state.set_theme(...)`).
    /// Default: no-op. Override to apply your own ImGui style overrides.
    fn on_theme_changed(&mut self, _theme: &Theme, _state: &mut AppStateV2) {}
}
