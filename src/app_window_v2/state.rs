//! Per-frame application state passed to [`AppHandlerV2`](super::AppHandlerV2).

use super::proxy::AppProxyV2;
use crate::theme::Theme;

// ── TitlebarStateV2 ─────────────────────────────────────────────────────────────

/// Live state of the titlebar — kept in sync with the OS window state.
#[derive(Debug, Clone)]
pub struct TitlebarStateV2 {
    pub maximized: bool,
    pub focused: bool,
}

impl Default for TitlebarStateV2 {
    fn default() -> Self {
        Self {
            maximized: false,
            focused: true,
        }
    }
}

impl TitlebarStateV2 {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn set_maximized(&mut self, v: bool) {
        self.maximized = v;
    }
    pub(super) fn set_focused(&mut self, v: bool) {
        self.focused = v;
    }
}

// ── AppStateV2 ──────────────────────────────────────────────────────────────────

/// Mutable state available inside `AppHandlerV2` callbacks.
///
/// Use the methods here to request window actions instead of holding an
/// OS handle yourself.
pub struct AppStateV2 {
    pub titlebar: TitlebarStateV2,

    pub(super) proxy: AppProxyV2,
    pub(super) should_exit: bool,
    pub(super) request_minimize: bool,
    pub(super) request_maximize: Option<bool>,
    pub(super) pending_theme: Option<Theme>,
    pub(super) request_visible: Option<bool>,
    pub(super) pending_title: Option<String>,
    pub(super) pending_opacity: Option<f32>,
}

impl AppStateV2 {
    pub(super) fn new(proxy: AppProxyV2) -> Self {
        Self {
            titlebar: TitlebarStateV2::new(),
            proxy,
            should_exit: false,
            request_minimize: false,
            request_maximize: None,
            pending_theme: None,
            request_visible: None,
            pending_title: None,
            pending_opacity: None,
        }
    }

    /// Get a clone of the cross-thread wake-up proxy.
    ///
    /// Hand the returned handle to background threads / async tasks so they
    /// can call [`AppProxyV2::wake`] when new work is ready. The proxy is
    /// `Send + Sync + Clone`.
    ///
    /// ```rust,ignore
    /// fn on_ready(&mut self, state: &mut AppStateV2) {
    ///     let proxy = state.proxy();
    ///     std::thread::spawn(move || {
    ///         // …background work…
    ///         let _ = proxy.wake();   // bring the UI back from idle
    ///     });
    /// }
    /// ```
    pub fn proxy(&self) -> AppProxyV2 {
        self.proxy.clone()
    }

    /// Exit the application on the next frame.
    pub fn exit(&mut self) {
        self.should_exit = true;
    }

    /// Minimise the window.
    pub fn minimize(&mut self) {
        self.request_minimize = true;
    }

    /// Set maximize state. Updates `titlebar.maximized` immediately so the
    /// button icon flips without waiting for the next OS event.
    pub fn set_maximized(&mut self, v: bool) {
        self.request_maximize = Some(v);
        self.titlebar.set_maximized(v);
    }

    /// Toggle maximize / restore.
    pub fn toggle_maximized(&mut self) {
        let v = !self.titlebar.maximized;
        self.set_maximized(v);
    }

    /// Apply a new theme at end-of-frame; `on_theme_changed` fires after.
    pub fn set_theme(&mut self, t: Theme) {
        self.pending_theme = Some(t);
    }

    /// Confirm a `CloseModeV2::Confirm` close. Triggers exit at end of the current frame.
    pub fn confirm_close(&mut self) {
        self.should_exit = true;
    }

    /// Make the window visible (useful after `.start_hidden()` or a previous `.hide()`).
    pub fn show(&mut self) {
        self.request_visible = Some(true);
    }

    /// Hide the window without closing it. Call `show()` to bring it back.
    pub fn hide(&mut self) {
        self.request_visible = Some(false);
    }

    /// Change the window title at runtime.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.pending_title = Some(title.into());
    }

    /// Change the window opacity at runtime (0.0 = transparent, 1.0 = opaque).
    pub fn set_opacity(&mut self, alpha: f32) {
        self.pending_opacity = Some(alpha.clamp(0.0, 1.0));
    }

    /// Request that the host render at least `frames` more frames after
    /// this one. Use from inside `render()` while you have an animation,
    /// timer, or follow-up state change in flight.
    ///
    /// In event-driven mode this is the difference between an animation
    /// playing and a frame freezing mid-tween. In continuous-render mode
    /// it's a no-op.
    ///
    /// This forwards to [`crate::frame_demand::request`] — built-in
    /// widgets (`notifications`, `confirm_dialog`, …) call it
    /// automatically while their animations are alive.
    pub fn keep_alive(&self, frames: u8) {
        crate::frame_demand::request(frames);
    }
}
