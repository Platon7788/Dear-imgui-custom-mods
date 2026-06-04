//! Events emitted by toolbar interaction. Returned per-frame from
//! [`Toolbar::render`](super::Toolbar::render).

/// Event emitted by toolbar interaction.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    /// A button was clicked.
    ButtonClicked { index: usize, label: String },
    /// A toggle was toggled (new state).
    Toggled {
        index: usize,
        label: String,
        on: bool,
    },
    /// A dropdown selection changed.
    DropdownChanged {
        index: usize,
        label: String,
        selected: usize,
    },
}
