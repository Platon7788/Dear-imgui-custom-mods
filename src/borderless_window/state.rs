//! Per-frame mutable state for the borderless titlebar.

/// Mutable runtime state for the borderless titlebar.
///
/// Create once, keep alive for the window's lifetime.
/// Update [`maximized`](Self::maximized) after toggling the OS window state.
#[derive(Debug, Clone)]
pub struct TitlebarState {
    /// Whether the window is currently maximized.
    ///
    /// Determines which icon to show (maximize vs. restore).
    /// Call [`set_maximized`](Self::set_maximized) after `window.set_maximized(...)`.
    pub maximized: bool,

    /// Pending close confirmation — set by [`confirm_close`](Self::confirm_close).
    pub(crate) confirmed_close: bool,
    /// Whether the window currently has OS focus.
    pub focused: bool,
}

impl Default for TitlebarState {
    fn default() -> Self {
        Self {
            maximized: false,
            confirmed_close: false,
            focused: true,
        }
    }
}

impl TitlebarState {
    pub fn new() -> Self { Self::default() }

    /// Sync the maximized flag after toggling the OS window state.
    pub fn set_maximized(&mut self, v: bool) { self.maximized = v; }

    /// Sync the focused flag from the OS `WindowEvent::Focused` event.
    pub fn set_focused(&mut self, v: bool) { self.focused = v; }

    /// Signal that the user confirmed the close action.
    ///
    /// On the next [`render_titlebar`](super::render_titlebar) call,
    /// [`WindowAction::Close`](super::actions::WindowAction::Close) will be returned.
    ///
    /// Use this with [`CloseMode::Confirm`](super::config::CloseMode::Confirm):
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::borderless_window::{TitlebarState, WindowAction};
    /// # let mut state = TitlebarState::new();
    /// # let action = WindowAction::None;
    /// if matches!(action, WindowAction::CloseRequested) {
    ///     // show your dialog, then:
    ///     state.confirm_close();
    /// }
    /// ```
    pub fn confirm_close(&mut self) { self.confirmed_close = true; }

    /// Cancel a pending close (reset after user dismisses the confirmation dialog).
    pub fn cancel_close(&mut self) { self.confirmed_close = false; }
}
