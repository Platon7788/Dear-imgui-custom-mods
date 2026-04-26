//! Mutable per-frame state passed to [`AppHandlerV2::render`](super::handler::AppHandlerV2::render).
//!
//! Unlike v1, the `maximized` and `focused` flags here are **read-only**
//! views of the OS state — the OS owns the window lifecycle and we mirror
//! its events. Use the methods on this struct to *request* state changes
//! (exit, theme switch, programmatic maximize); they're applied at the end
//! of the current frame.

use crate::theme::Theme;

/// Per-frame state passed to your render handler.
pub struct AppStateV2 {
    /// Title bar mirror state (focused / maximized) — read-only for the
    /// handler; mutated by the event loop from native OS events.
    pub titlebar: TitlebarStateV2,
    pub(super) should_exit: bool,
    pub(super) request_minimize: bool,
    pub(super) request_maximize: Option<bool>,
    pub(super) pending_theme: Option<Theme>,
    pub(super) confirmed_close: bool,
}

impl AppStateV2 {
    pub(super) fn new() -> Self {
        Self {
            titlebar: TitlebarStateV2::default(),
            should_exit: false,
            request_minimize: false,
            request_maximize: None,
            pending_theme: None,
            confirmed_close: false,
        }
    }

    /// Request a theme change. Applied at end of frame:
    /// - Updates the ImGui style
    /// - Updates the titlebar palette
    /// - Calls [`AppHandlerV2::on_theme_changed`](super::handler::AppHandlerV2::on_theme_changed)
    pub fn set_theme(&mut self, t: Theme) {
        self.pending_theme = Some(t);
    }

    /// Request the window to close.
    pub fn exit(&mut self) {
        self.should_exit = true;
    }

    /// Request the window to be minimized to the taskbar.
    pub fn minimize(&mut self) {
        self.request_minimize = true;
    }

    /// Request a maximize toggle. Pass `true` to maximize, `false` to restore.
    pub fn set_maximized(&mut self, v: bool) {
        self.request_maximize = Some(v);
    }

    /// Toggle maximized state.
    pub fn toggle_maximized(&mut self) {
        self.request_maximize = Some(!self.titlebar.maximized);
    }

    /// Confirm a pending close (used with [`CloseMode::Confirm`](super::config::CloseMode::Confirm)).
    pub fn confirm_close(&mut self) {
        self.confirmed_close = true;
    }
}

/// Read-only view of the titlebar's OS-derived state.
#[derive(Debug, Clone, Default)]
pub struct TitlebarStateV2 {
    /// Currently maximized — mirrored from `WindowEvent::Resized` →
    /// `window.is_maximized()`. Determines which icon (maximize vs.
    /// restore) the titlebar draws.
    pub maximized: bool,
    /// Window has OS focus — mirrored from `WindowEvent::Focused`.
    pub focused: bool,
}
