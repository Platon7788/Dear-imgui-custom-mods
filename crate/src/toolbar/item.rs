//! Toolbar item model — [`ToolbarItem`], [`ToolbarItemKind`], and the
//! builder constructors. Pure data + small builders, no ImGui context
//! required.

/// Toolbar item variant.
#[derive(Debug, Clone)]
pub enum ToolbarItemKind {
    /// Clickable button.
    Button,
    /// Toggle button (on/off state).
    Toggle { on: bool },
    /// Visual separator line.
    Separator,
    /// Flexible spacer (pushes items to the right).
    Spacer,
    /// Dropdown (click → emits event, dropdown menu is handled externally).
    Dropdown {
        options: Vec<String>,
        selected: usize,
    },
}

/// A single toolbar item.
#[derive(Debug, Clone)]
pub struct ToolbarItem {
    /// Display label.
    pub label: String,
    /// Unicode icon text (empty = no icon).
    pub icon: String,
    /// Kind of item.
    pub kind: ToolbarItemKind,
    /// Tooltip text (shown on hover).
    pub tooltip: String,
    /// Whether this item is enabled.
    pub enabled: bool,
}

impl ToolbarItem {
    /// Create a button.
    pub fn button(label: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Button,
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Create a toggle button.
    pub fn toggle(label: impl Into<String>, on: bool, tooltip: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Toggle { on },
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Create a separator.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            kind: ToolbarItemKind::Separator,
            tooltip: String::new(),
            enabled: true,
        }
    }

    /// Create a spacer.
    pub fn spacer() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            kind: ToolbarItemKind::Spacer,
            tooltip: String::new(),
            enabled: true,
        }
    }

    /// Create a dropdown.
    ///
    /// `selected` is clamped into `0..options.len()` (or `0` when the
    /// option list is empty), so an out-of-range index from the caller
    /// can never reach the renderer.
    pub fn dropdown(
        label: impl Into<String>,
        options: Vec<String>,
        selected: usize,
        tooltip: impl Into<String>,
    ) -> Self {
        let clamped = if options.is_empty() {
            0
        } else {
            selected.min(options.len() - 1)
        };
        Self {
            label: label.into(),
            icon: String::new(),
            kind: ToolbarItemKind::Dropdown {
                options,
                selected: clamped,
            },
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    /// Builder: set enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder: set icon text (Unicode glyph).
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

// ── Display-text helpers ─────────────────────────────────────────────────────

/// Build the display string by combining icon and label.
///
/// Returns a `Cow<str>` to avoid allocation when only one part is present.
pub(super) fn display_text(item: &ToolbarItem) -> std::borrow::Cow<'_, str> {
    display_text_ref(&item.icon, &item.label)
}

/// Build the display string from icon and label references.
///
/// Zero-alloc when only icon or label is present.
pub(super) fn display_text_ref<'a>(icon: &'a str, label: &'a str) -> std::borrow::Cow<'a, str> {
    if icon.is_empty() {
        std::borrow::Cow::Borrowed(label)
    } else if label.is_empty() {
        std::borrow::Cow::Borrowed(icon)
    } else {
        std::borrow::Cow::Owned(format!("{} {}", icon, label))
    }
}
