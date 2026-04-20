//! Action events and result type returned by [`render_titlebar`](super::render_titlebar).

/// Eight window edges / corners for resize drag operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// Action produced by the titlebar each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowAction {
    /// Nothing actionable this frame.
    None,
    /// Minimize button pressed.
    Minimize,
    /// Maximize / restore toggled (button **or** double-click on drag area).
    Maximize,
    /// Close confirmed (immediate mode **or** after `state.confirm_close()`).
    Close,
    /// Close button pressed with [`CloseMode::Confirm`](super::config::CloseMode).
    /// Show your own dialog, then call [`TitlebarState::confirm_close`](super::state::TitlebarState::confirm_close).
    CloseRequested,
    /// User pressed mouse on the drag area → call `window.drag_window().ok()`.
    DragStart,
    /// The window icon (if any) was clicked — show a custom menu or handle freely.
    IconClick,
    /// User pressed mouse on a resize edge → call `window.drag_resize_window(to_winit(edge)).ok()`.
    ResizeStart(ResizeEdge),
    /// A custom extra button was clicked — value is the button `id`.
    Extra(&'static str),
}

/// Value returned by [`render_titlebar`](super::render_titlebar) each frame.
///
/// Ignoring this result drops the user's window action (drag / resize / close)
/// on the floor — `#[must_use]` makes that a compile warning.
#[derive(Debug, Clone, Copy)]
#[must_use = "window actions are produced each frame — dropping the result means dropping user input"]
pub struct TitlebarResult {
    /// Primary action to handle (drag, resize, button clicks, …).
    pub action: WindowAction,
    /// Edge / corner the cursor is currently **hovering** (regardless of click).
    ///
    /// Use this to update the OS cursor icon every frame:
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::borderless_window::ResizeEdge;
    /// # let edge: Option<ResizeEdge> = None;
    /// # let window: () = ();
    /// // if let Some(e) = result.hover_edge { window.set_cursor(cursor_for_edge(e)); }
    /// ```
    pub hover_edge: Option<ResizeEdge>,
}

impl TitlebarResult {
    pub(crate) fn none() -> Self {
        Self {
            action: WindowAction::None,
            hover_edge: None,
        }
    }
}

impl Default for TitlebarResult {
    fn default() -> Self {
        Self::none()
    }
}
