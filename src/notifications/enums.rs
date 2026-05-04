//! Notification configuration enums.

use super::theme::NotificationColors;

// ─── Severity ────────────────────────────────────────────────────────────────

/// Semantic importance of a notification — drives icon and accent color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    /// Neutral blue — general information.
    #[default]
    Info,
    /// Green — operation succeeded.
    Success,
    /// Amber — caution, non-fatal.
    Warning,
    /// Red — error / failure.
    Error,
    /// Gray — developer-only diagnostic.
    Debug,
}

impl Severity {
    /// Human-readable single-word label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Debug => "Debug",
        }
    }

    /// Pick the severity's accent color from a palette.
    pub fn accent(self, c: &NotificationColors) -> [f32; 4] {
        match self {
            Self::Info => c.info,
            Self::Success => c.success,
            Self::Warning => c.warning,
            Self::Error => c.error,
            Self::Debug => c.debug,
        }
    }
}

// ─── Placement ───────────────────────────────────────────────────────────────

/// Stack anchor position within the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Placement {
    /// Top-right corner — stack grows downward.
    #[default]
    TopRight,
    /// Top-left corner — stack grows downward.
    TopLeft,
    /// Bottom-right corner — stack grows upward.
    BottomRight,
    /// Bottom-left corner — stack grows upward.
    BottomLeft,
    /// Top-center — stack grows downward, centered.
    TopCenter,
    /// Bottom-center — stack grows upward, centered.
    BottomCenter,
}

impl Placement {
    /// `true` if the stack grows upward (bottom anchors) vs downward.
    pub(crate) fn grows_up(self) -> bool {
        matches!(
            self,
            Self::BottomRight | Self::BottomLeft | Self::BottomCenter
        )
    }
    /// `true` if the slide-in direction is from the left edge.
    pub(crate) fn slides_from_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
    /// `true` if the slide-in direction is from the right edge.
    pub(crate) fn slides_from_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }
}

// ─── Duration ────────────────────────────────────────────────────────────────

/// How long a notification stays on screen before auto-dismissing.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Duration {
    /// Auto-dismiss after N seconds.
    Timed(f32),
    /// Never auto-dismiss — user must close it manually.
    Sticky,
}

impl Default for Duration {
    fn default() -> Self {
        Self::Timed(4.0)
    }
}

// ─── Animation ───────────────────────────────────────────────────────────────

/// Entry / exit animation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AnimationKind {
    /// Fade alpha 0 → 1 on enter, 1 → 0 on exit.
    #[default]
    Fade,
    /// Slide horizontally from the anchor edge + fade.
    SlideIn,
    /// Instant appear / disappear.
    None,
}
