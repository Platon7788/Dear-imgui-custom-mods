//! Per-frame mutable state for the navigation panel.

/// Mutable runtime state for the navigation panel.
///
/// Create once, keep alive for the window's lifetime.
#[derive(Debug, Clone)]
pub struct NavPanelState {
    /// Currently active (selected) button ID.
    pub active: Option<&'static str>,
    /// Whether the panel is visible (used with auto-hide).
    pub visible: bool,
    /// Animation progress: `0.0` = fully hidden, `1.0` = fully visible.
    pub animation_progress: f32,
    /// Currently open submenu button ID (if any).
    /// Currently open submenu button ID (if any).
    pub open_submenu: Option<&'static str>,
    /// Whether the cursor was over the panel last frame.
    pub(crate) was_hovered: bool,
}

impl Default for NavPanelState {
    fn default() -> Self {
        Self {
            active: None,
            visible: true,
            animation_progress: 1.0,
            open_submenu: None,
            was_hovered: false,
        }
    }
}

impl NavPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the active button by ID.
    pub fn set_active(&mut self, id: &'static str) {
        self.active = Some(id);
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
