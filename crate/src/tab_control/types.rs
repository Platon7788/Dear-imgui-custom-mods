//! Tab control type definitions.

#![allow(missing_docs)] // numeric layout fields are self-explanatory

// ─── Tab identifier ─────────────────────────────────────────────────────────

/// Opaque, auto-incrementing tab identifier.
///
/// Assigned internally by [`TabControl::add`](super::TabControl::add).
/// Stable across removals — never reused within a single `TabControl` instance.
pub type TabId = u64;

// ─── Tab status ─────────────────────────────────────────────────────────────

/// Visual status of a tab — controls the small indicator dot drawn on the tab.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum TabStatus {
    /// Green dot (default — normal/healthy).
    #[default]
    Active,
    /// Muted dot (idle/disconnected).
    Inactive,
    /// Amber dot, slowly pulsing.
    Warning,
    /// Red dot, slowly pulsing.
    Error,
    /// Cyan filled circle — "unsaved changes" indicator (editor-style).
    /// Replaces the close button visually until the tab is saved.
    Dirty,
    /// No status dot at all. The dot slot is *also removed* from layout —
    /// title shifts left by the dot's reserve. Per-tab opt-out, complementary
    /// to the global [`TabControlConfig::show_status_dot`] flag.
    ///
    /// **Layout-jump caveat:** `Active ↔ Dirty` keeps a stable layout
    /// (the dot slot is reserved in both states). However `None ↔ Dirty`
    /// shifts the tab content by ≈11 px on toggle, because `None` removes
    /// the slot entirely. If your status field swings between those two
    /// values frequently and you want stable layout, prefer
    /// `Inactive ↔ Dirty` over `None ↔ Dirty`.
    None,
}

// ─── Badge ──────────────────────────────────────────────────────────────────

/// Small badge pill drawn after the title (notification count, status label, …).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Badge {
    /// Text shown inside the pill.
    pub text: String,
    /// Background color `[R, G, B]` (alpha is applied automatically).
    pub color: [u8; 3],
}

impl Badge {
    /// Numeric badge (e.g. unread count).
    pub fn count(n: u32, color: [u8; 3]) -> Self {
        Self {
            text: n.to_string(),
            color,
        }
    }

    /// Text label badge.
    pub fn label(text: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

// ─── Close-button glyph style ───────────────────────────────────────────────

/// Visual style of the per-tab close button. All variants render through
/// the draw list (`add_line` / `add_rect`) and therefore work even when
/// [`TabControlConfig::icons_available`] is `false`.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum CloseGlyph {
    /// Plain diagonal cross. Lightweight, default.
    #[default]
    Cross,
    /// Bolder diagonal cross — more visible on busy backgrounds.
    CrossBold,
    /// Cross inside a thin rounded square — most prominent.
    SquareX,
    /// Cross inside a circle — softest visual.
    CircleX,
}

// ─── Tab style ──────────────────────────────────────────────────────────────

/// Visual style for the tabs.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TabStyle {
    /// Fully rounded pill (default).
    #[default]
    Pill,
    /// Flat with a thick accent bar at the bottom (Material-style).
    Underline,
    /// Rectangular with small top rounding.
    Square,
}

// ─── Action returned by render() ────────────────────────────────────────────

/// User interaction reported by [`TabControl::render`](super::TabControl::render).
///
/// At most one action is returned per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TabAction {
    /// A tab became active (clicked, keyboard, or programmatic focus).
    Activated(TabId),
    /// A tab was closed (post-confirmation if `confirm_close` is enabled).
    Closed(TabId),
    /// The "+" add button was clicked. Only fires when `show_add_button = true`.
    AddRequested,
    /// A tab was double-clicked (e.g. for rename or detach).
    DoubleClicked(TabId),
    /// Tabs were reordered via drag-and-drop. Payload is the moved tab ID.
    Reordered(TabId),
}
