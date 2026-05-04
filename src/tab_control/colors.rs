//! Color palette for tab control.

#![allow(missing_docs)] // numeric layout fields are self-explanatory

use super::types::TabStatus;

/// Color palette — all colors are `[R, G, B]` in 0..=255 range (alpha per use).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TabColors {
    /// Background of an inactive tab.
    pub tab_bg: [u8; 3],
    /// Background of a hovered (but not active) tab.
    pub tab_hover: [u8; 3],
    /// Background of the active tab.
    pub tab_active: [u8; 3],
    /// Generic accent color (focus ring, drag indicator).
    pub accent: [u8; 3],
    /// Primary text color (active tab title).
    pub text: [u8; 3],
    /// Muted text color (inactive tab title).
    pub text_muted: [u8; 3],
    /// Background tint of the close-button hover area.
    pub close_hover: [u8; 3],
    /// Background of the entire tab strip (behind tabs and side buttons).
    pub strip_bg: [u8; 3],
    /// Background of the **outer frame** drawn below the strip when
    /// [`TabControlConfig::body_inset_enabled`] is `true`. This is the
    /// surface filling the `body_inset`-wide gap around the inner
    /// body rectangle (the visible "frame around content" effect).
    /// Default mirrors [`Self::strip_bg`] so the strip and frame
    /// read as one continuous chrome surface, but hosts can colour
    /// the frame independently — changing this field DOES NOT
    /// affect the tab strip itself.
    pub frame_bg: [u8; 3],
    /// Outline colour of the active-tab body frame, drawn around the
    /// outer rectangle when
    /// [`TabControlConfig::body_inset_border`] is `true`. Default
    /// mirrors [`Self::accent`] so the border reads as the same
    /// "this is selected" cue the strip uses for the active tab.
    pub frame_border: [u8; 3],
    /// Background of the active tab's body — the borderless
    /// child-window the strip's `render_content()` runs inside when
    /// [`TabControlConfig::body_inset_enabled`] is `true`.
    /// Default is **slightly lighter** than [`Self::strip_bg`] so
    /// the body reads as a distinct surface against the chrome and
    /// the body-inset gap registers as a visible frame around it
    /// (an earlier "mirror strip_bg" default made the inset
    /// invisible — pinned by the
    /// `body_bg_default_differs_from_strip_bg_for_visible_frame`
    /// test). Set to a contrasting hue when the host wants stronger
    /// separation (e.g. white-on-dark editor on a dark chrome).
    pub body_bg: [u8; 3],
    /// Color of the bottom-of-strip separator line and other thin dividers.
    pub separator: [u8; 3],
    pub status_active: [u8; 3],
    pub status_inactive: [u8; 3],
    pub status_warning: [u8; 3],
    pub status_error: [u8; 3],
    /// Color of the dirty-state indicator (replaces close icon).
    pub status_dirty: [u8; 3],
}

impl Default for TabColors {
    fn default() -> Self {
        Self {
            tab_bg: [0x35, 0x3a, 0x44],
            tab_hover: [0x3f, 0x45, 0x52],
            tab_active: [0x4a, 0x52, 0x60],
            accent: [0x5b, 0x9b, 0xd5],
            text: [0xe8, 0xec, 0xf2],
            text_muted: [0x90, 0x98, 0xa6],
            close_hover: [0xe0, 0x60, 0x60],
            strip_bg: [0x2a, 0x2e, 0x37],
            // Default mirrors strip_bg so the strip + frame read as
            // one chrome surface; hosts override `frame_bg` to
            // distinguish them.
            frame_bg: [0x2a, 0x2e, 0x37],
            // Default mirrors `accent` so the active-tab border
            // (when enabled) reads as the same "selected" hue the
            // strip uses to highlight the active tab.
            frame_border: [0x5b, 0x9b, 0xd5],
            // Slightly *lighter* than strip_bg so the body reads as
            // a "raised inset surface" rather than a dark hole.
            // User feedback 2026-04-30: prior darker defaults read
            // as "чёрное". Hosts that want a different look can
            // override `colors.body_bg` directly or via the demo's
            // colour picker.
            body_bg: [0x32, 0x36, 0x40],
            separator: [0x3f, 0x46, 0x54],
            status_active: [0x5f, 0xb8, 0x70],
            status_inactive: [0x8a, 0x92, 0xa1],
            status_warning: [0xd0, 0x7a, 0x30],
            status_error: [0xd0, 0x45, 0x45],
            status_dirty: [0x4f, 0xc3, 0xf7],
        }
    }
}

impl TabColors {
    /// Return the `[u8; 3]` color associated with a [`TabStatus`].
    /// `TabStatus::None` returns `status_inactive` as a neutral fallback —
    /// callers should normally check for `None` and skip drawing entirely.
    pub fn status_color(&self, status: TabStatus) -> [u8; 3] {
        match status {
            TabStatus::Active => self.status_active,
            TabStatus::Inactive | TabStatus::None => self.status_inactive,
            TabStatus::Warning => self.status_warning,
            TabStatus::Error => self.status_error,
            TabStatus::Dirty => self.status_dirty,
        }
    }

    /// Build a `TabColors` from the nav-panel and status-bar palettes
    /// of an active theme. This is what `Theme::tab_colors()` calls
    /// internally, so the tab strip stays visually coherent with the
    /// rest of the chrome stack — same `bg` / `separator` / `text`
    /// surfaces, same status-indicator hues.
    ///
    /// `tab_bg` deliberately mirrors `strip_bg` (= `nav.bg`); inactive
    /// tabs blend with the strip the way VS Code / Sublime Merge does,
    /// while hover/active surfaces lift through `nav.btn_hover` /
    /// `nav.btn_active`.
    pub fn from_palettes(
        nav: &crate::theme::NavColors,
        sb: &crate::theme::StatusBarColors,
    ) -> Self {
        let to_u8 = |c: [f32; 4]| {
            let r = (c[0] * 255.0).round().clamp(0.0, 255.0) as u8;
            let g = (c[1] * 255.0).round().clamp(0.0, 255.0) as u8;
            let b = (c[2] * 255.0).round().clamp(0.0, 255.0) as u8;
            [r, g, b]
        };
        Self {
            tab_bg: to_u8(nav.bg),
            tab_hover: to_u8(nav.btn_hover),
            tab_active: to_u8(nav.btn_active),
            accent: to_u8(nav.indicator),
            text: to_u8(nav.icon_active),
            text_muted: to_u8(nav.icon_default),
            close_hover: to_u8(sb.error),
            strip_bg: to_u8(nav.bg),
            // Mirrors strip_bg by default — same chrome surface;
            // hosts override for independent colouring.
            frame_bg: to_u8(nav.bg),
            // Borrow the nav indicator hue for the active-tab
            // frame border, so it reads as the same "selected"
            // accent the strip uses internally.
            frame_border: to_u8(nav.indicator),
            // Slightly *lighter* than strip_bg (= nav.bg) so the
            // body reads as a raised inset surface. Earlier
            // attempts went darker which read as a "dark hole" on
            // already-dark chrome. `+0.03` keeps the step subtle
            // (light themes still get a tonal lift, dark themes
            // get a soft pop without fluorescing).
            body_bg: {
                let lift = 0.03_f32;
                to_u8([
                    (nav.bg[0] + lift).clamp(0.0, 1.0),
                    (nav.bg[1] + lift).clamp(0.0, 1.0),
                    (nav.bg[2] + lift).clamp(0.0, 1.0),
                    nav.bg[3],
                ])
            },
            separator: to_u8(nav.separator),
            status_active: to_u8(sb.success),
            status_inactive: to_u8(sb.text_dim),
            status_warning: to_u8(sb.warning),
            status_error: to_u8(sb.error),
            status_dirty: to_u8(sb.info),
        }
    }
}
