//! Nav panel configuration enums.

// ── Position ─────────────────────────────────────────────────────────────────

/// Where the panel is docked.
///
/// Three positions are supported: Left, Right, Top.
/// Bottom is intentionally omitted — that slot is reserved for
/// [`StatusBar`](crate::status_bar::StatusBar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DockPosition {
    #[default]
    Left,
    Right,
    Top,
}

// ── Button style (for Top) ──────────────────────────────────────────────────

/// How buttons are rendered in horizontal (Top) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ButtonStyle {
    /// Icon only — small square cells.
    #[default]
    IconOnly,
    /// Icon + label side by side.
    IconWithLabel,
    /// Text label only, no icon.
    LabelOnly,
}

// ── Active-button visual style ──────────────────────────────────────────────

/// How the **active** button is highlighted.
///
/// As of 2026-04-29 the default is [`Self::Ring`] — a thin stroked
/// circle around the icon, no background fill — because that's the
/// look the project owner wants out of the box (matches the
/// VS-Code-with-orange-focus-ring aesthetic). [`Self::Bar`] is still
/// available for callers who prefer the older "filled background +
/// indicator strip" treatment.
///
/// Pick the style that matches the surrounding chrome density —
/// `Bar` reads better on busy workspaces (wins the visual contest
/// against tooltips, hover states, etc.), `Ring` keeps the icon
/// silhouette intact and works well on calmer layouts where the
/// active button is the visual centre of attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ActiveStyle {
    /// Thin stroked circle around the icon, no background fill, no
    /// indicator strip. Colour = [`NavPanelConfig::active_ring_color`]
    /// if set, otherwise falls back to the palette's
    /// [`NavColors::indicator`](super::theme::NavColors::indicator).
    /// Default.
    ///
    /// [`NavPanelConfig::active_ring_color`]: super::config::NavPanelConfig::active_ring_color
    #[default]
    Ring,
    /// Filled background tint + indicator strip on the docked edge
    /// (Left/Right/Top side). The classic Vex0r/Code-style sidebar
    /// active state.
    Bar,
}
