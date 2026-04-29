//! [`AppHandler`] trait — implement this to drive your application.

use dear_imgui_rs::Ui;
use winit::event::WindowEvent;

use super::state::AppState;
use crate::theme::Theme;

/// Application logic interface for [`AppWindow`](super::AppWindow).
///
/// All methods have default (no-op) implementations — override only what you need.
///
/// The minimum useful impl just overrides [`render`](Self::render):
///
/// ```rust,no_run
/// use dear_imgui_custom_mod::app_window::{AppHandler, AppState};
/// use dear_imgui_rs::Ui;
///
/// struct MyApp { count: u32 }
///
/// impl AppHandler for MyApp {
///     fn render(&mut self, ui: &Ui, _state: &mut AppState) {
///         ui.text(format!("count = {}", self.count));
///         if ui.button("+") { self.count += 1; }
///     }
/// }
/// ```
pub trait AppHandler {
    /// Called every frame inside the content area below the titlebar
    /// (or the whole window if `Chrome::None`).
    ///
    /// `ui.content_region_avail()` returns the remaining space.
    fn render(&mut self, ui: &Ui, state: &mut AppState);

    /// Close requested (close button, Alt-F4, OS close).
    /// Default: exit. Override + use `state.confirm_close()` for confirm dialogs.
    fn on_close_requested(&mut self, state: &mut AppState) {
        state.exit();
    }

    /// A custom titlebar [`ExtraButton`](super::config::ExtraButton) was clicked.
    fn on_extra_button(&mut self, _id: &'static str, _state: &mut AppState) {}

    /// The titlebar icon (if set) was clicked.
    fn on_icon_click(&mut self, _state: &mut AppState) {}

    /// Theme was changed via [`AppState::set_theme`].
    fn on_theme_changed(&mut self, _theme: &Theme, _state: &mut AppState) {}

    /// Called once when the window is fully created and ready.
    ///
    /// Use [`AppState::proxy`](super::AppState::proxy) here to grab a
    /// cross-thread wake-up handle for any background work you spawn.
    fn on_ready(&mut self, _state: &mut AppState) {}

    /// Raw winit [`WindowEvent`] hook, called **before** the event reaches
    /// Dear ImGui's platform layer.
    ///
    /// Return `true` to **consume** the event so Dear ImGui does not see
    /// it. Consuming **only suppresses the ImGui platform handler** — the
    /// framework's own routing (`Resized` reconfigures the wgpu surface,
    /// `CloseRequested` exits, `Focused` updates titlebar tint,
    /// `RedrawRequested` runs `render_frame`) still executes. Consuming a
    /// structural event is normally pointless; the previous "consume =
    /// skip everything" contract has been replaced after producing
    /// black-rect / stuck-window bugs.
    ///
    /// Use this hook for:
    ///
    /// - **Drag-and-drop file paths** — read [`WindowEvent::DroppedFile`]
    ///   and [`WindowEvent::HoveredFile`] which would otherwise only
    ///   trigger a redraw without delivering the path.
    /// - **Layout-independent hotkeys** — match physical keys before the
    ///   ImGui keyboard layer translates them.
    /// - **Custom IME / touchpad gestures** — winit delivers richer events
    ///   than ImGui ever consumes.
    /// - **Application-level shortcuts** that should bypass focused
    ///   widgets (e.g. global "F12 to toggle dev console").
    ///
    /// Default: returns `false` (do not consume).
    fn on_window_event(&mut self, _event: &WindowEvent, _state: &mut AppState) -> bool {
        false
    }
}
