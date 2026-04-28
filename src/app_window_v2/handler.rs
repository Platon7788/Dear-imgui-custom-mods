//! [`AppHandlerV2`] trait — implement this to drive your application.

use dear_imgui_rs::Ui;
use winit::event::WindowEvent;

use super::state::AppStateV2;
use crate::theme::Theme;

/// Application logic interface for [`AppWindowV2`](super::AppWindowV2).
///
/// All methods have default (no-op) implementations — override only what you need.
///
/// The minimum useful impl just overrides [`render`](Self::render):
///
/// ```rust,no_run
/// use dear_imgui_custom_mod::app_window_v2::{AppHandlerV2, AppStateV2};
/// use dear_imgui_rs::Ui;
///
/// struct MyApp { count: u32 }
///
/// impl AppHandlerV2 for MyApp {
///     fn render(&mut self, ui: &Ui, _state: &mut AppStateV2) {
///         ui.text(format!("count = {}", self.count));
///         if ui.button("+") { self.count += 1; }
///     }
/// }
/// ```
pub trait AppHandlerV2 {
    /// Called every frame inside the content area below the titlebar
    /// (or the whole window if `ChromeV2::None`).
    ///
    /// `ui.content_region_avail()` returns the remaining space.
    fn render(&mut self, ui: &Ui, state: &mut AppStateV2);

    /// Close requested (close button, Alt-F4, OS close).
    /// Default: exit. Override + use `state.confirm_close()` for confirm dialogs.
    fn on_close_requested(&mut self, state: &mut AppStateV2) {
        state.exit();
    }

    /// A custom titlebar [`ExtraButtonV2`](super::config::ExtraButtonV2) was clicked.
    fn on_extra_button(&mut self, _id: &'static str, _state: &mut AppStateV2) {}

    /// The titlebar icon (if set) was clicked.
    fn on_icon_click(&mut self, _state: &mut AppStateV2) {}

    /// Theme was changed via [`AppStateV2::set_theme`].
    fn on_theme_changed(&mut self, _theme: &Theme, _state: &mut AppStateV2) {}

    /// Called once when the window is fully created and ready.
    ///
    /// Use [`AppStateV2::proxy`](super::AppStateV2::proxy) here to grab a
    /// cross-thread wake-up handle for any background work you spawn.
    fn on_ready(&mut self, _state: &mut AppStateV2) {}

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
    fn on_window_event(&mut self, _event: &WindowEvent, _state: &mut AppStateV2) -> bool {
        false
    }
}
