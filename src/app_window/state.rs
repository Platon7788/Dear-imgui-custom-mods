//! Per-frame application state passed to [`AppHandler::render`](super::AppHandler::render).

use crate::borderless_window::TitlebarState;
use crate::theme::Theme;

/// Mutable application state available every render frame.
///
/// Use this to request window actions (exit, maximize, theme switch) without
/// needing direct access to the OS window handle.
pub struct AppState {
    /// Titlebar state (maximized, focused, confirmed_close).
    pub titlebar: TitlebarState,
    pub(super) should_exit:      bool,
    pub(super) maximize_toggle:  Option<bool>,
    pub(super) pending_theme:    Option<Theme>,
}

impl AppState {
    pub(super) fn new() -> Self {
        Self {
            titlebar:        TitlebarState::new(),
            should_exit:     false,
            maximize_toggle: None,
            pending_theme:   None,
        }
    }

    /// Request a theme change.
    ///
    /// `AppWindow` will apply the full ImGui style and call
    /// [`AppHandler::on_theme_changed`](super::AppHandler::on_theme_changed)
    /// at the end of the current frame.
    pub fn set_theme(&mut self, theme: Theme) {
        self.pending_theme = Some(theme);
    }

    /// Request the window to close on the next frame.
    pub fn exit(&mut self) {
        self.should_exit = true;
    }

    /// Maximise or restore the window.
    pub fn set_maximized(&mut self, v: bool) {
        self.maximize_toggle = Some(v);
        self.titlebar.set_maximized(v);
    }

    /// Toggle maximised state.
    pub fn toggle_maximized(&mut self) {
        let next = !self.titlebar.maximized;
        self.set_maximized(next);
    }
}
