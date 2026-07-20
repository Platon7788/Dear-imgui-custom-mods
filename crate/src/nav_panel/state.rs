//! Per-frame mutable state for the navigation panel.

use std::borrow::Cow;

/// Mutable runtime state for the navigation panel.
///
/// Create once, keep alive for the window's lifetime.
///
/// ID fields use `Cow<'static, str>` so both static literals and
/// runtime-built IDs (loaded from JSON, plugin-generated) work uniformly.
#[derive(Debug, Clone)]
pub struct NavPanelState {
    /// Currently active (selected) button ID.
    pub active: Option<Cow<'static, str>>,
    /// Whether the panel is visible (used with auto-hide).
    pub visible: bool,
    /// Animation progress: `0.0` = fully hidden, `1.0` = fully visible.
    pub animation_progress: f32,
    /// Currently open submenu button ID (if any).
    pub open_submenu: Option<Cow<'static, str>>,
    /// Screen-space anchor `[x, y]` of the open right-click reposition menu,
    /// or `None` when it's closed. Set to the cursor position on right-click
    /// (see `nav_panel::render`), cleared when the user picks an item or
    /// clicks away. `pub(super)` — only the render/context-menu modules touch
    /// it, so hosts never manage menu lifecycle by hand.
    pub(super) reposition_anchor: Option<[f32; 2]>,
    /// Whether the cursor was over the panel last frame. Tightened from
    /// `pub(crate)` to `pub(super)` — only the `nav_panel::render` module
    /// touches this flag.
    pub(super) was_hovered: bool,
}

impl Default for NavPanelState {
    fn default() -> Self {
        Self {
            active: None,
            visible: true,
            animation_progress: 1.0,
            open_submenu: None,
            reposition_anchor: None,
            was_hovered: false,
        }
    }
}

impl NavPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the active button by ID. Accepts both `&'static str` and `String`
    /// (runtime IDs loaded from a config file, plugin-generated, etc).
    pub fn set_active(&mut self, id: impl Into<Cow<'static, str>>) {
        self.active = Some(id.into());
    }

    /// Clear the active button.
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    /// Show the panel (useful with auto-hide).
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the panel (useful with auto-hide).
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggle panel visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Close any open submenu.
    pub fn close_submenu(&mut self) {
        self.open_submenu = None;
    }
}
