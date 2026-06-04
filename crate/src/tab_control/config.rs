//! Configuration types for [`TabControl`](super::TabControl).
//!
//! Pure value types — no logic. Separated from `mod.rs` to keep the public
//! surface focused.

#![allow(missing_docs)] // numeric layout fields are self-explanatory

use super::colors::TabColors;
use super::strings::TabStrings;
use super::types::*;

// ─── Configuration ──────────────────────────────────────────────────────────

/// Full configuration for [`TabControl`](super::TabControl).
///
/// All fields have sensible defaults via [`Default`].
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TabControlConfig {
    // ── Behavior ──
    /// Allow closing tabs (global override; per-tab can still opt out via
    /// [`TabItem::is_closable`](super::TabItem::is_closable)).
    pub closable: bool,
    /// Show a confirmation popup before closing a tab.
    pub confirm_close: bool,
    /// Middle-click on a tab closes it (browser-style).
    pub middle_click_close: bool,
    /// Scroll wheel on the tab strip scrolls tabs horizontally.
    pub scroll_with_wheel: bool,
    /// Left/Right arrow keys cycle tabs, Ctrl+W closes the active tab.
    /// Gated by window focus, not hover.
    pub keyboard_nav: bool,
    /// Show a "+" button at the end of the tab strip.
    /// Returns [`TabAction::AddRequested`] when clicked.
    pub show_add_button: bool,
    /// Right-click on a tab populates `context_tab` and sets `open_context_menu`.
    pub context_menu: bool,
    /// When `true`, the tab strip is rendered but `render_content()` is NOT
    /// called on the active tab. The caller renders content after
    /// [`TabControl::render`](super::TabControl::render) returns.
    pub external_content: bool,
    /// When `true` (default), the active tab's `render_content()` runs
    /// inside a borderless child-window inset by [`Self::body_inset`]
    /// pixels from the outer window — a visible gap sits between the
    /// outer edges and the child rectangle so user widgets never touch
    /// the chrome. Set `false` for a full-bleed body (legacy
    /// behaviour) — useful for charts, hex dumps, or any widget that
    /// wants to use every available pixel.
    pub body_inset_enabled: bool,
    /// Outer inset in pixels — `[horizontal, vertical]` — applied to
    /// the active tab's body child-window when
    /// [`Self::body_inset_enabled`] is `true`. The renderer
    /// shifts the cursor inwards by this amount and shrinks the child
    /// by `2 ×` the same on both axes, producing a visible gap around
    /// the child rectangle. Default `[2.0, 2.0]`.
    pub body_inset: [f32; 2],
    /// When `true`, draw an outlined rectangle around the active
    /// tab's body frame in [`TabColors::frame_border`]. Off by
    /// default — opt-in cue for IDE-style hosts that want a
    /// "selected pane" highlight matching the active tab. Has no
    /// effect when [`Self::body_inset_enabled`] is `false`.
    pub body_inset_border: bool,
    /// Stroke thickness of the [`Self::body_inset_border`] outline,
    /// in pixels. Default `1.5`.
    pub body_inset_border_thickness: f32,
    /// Allow drag-and-drop reordering of tabs.
    pub draggable: bool,
    /// Show overflow `…` dropdown when tabs don't fit.
    pub show_overflow_dropdown: bool,
    /// Whether the active ImGui font contains the glyph range used by
    /// [`crate::icons`] (Material Design Icons, U+F0000–U+FFFFF).
    ///
    /// Default `false` — glyphs would render as `?` boxes otherwise. Set this
    /// to `true` *after* you've registered the MDI font with ImGui. When
    /// `false`, [`TabItem::icon`](super::TabItem::icon) is ignored — neither
    /// reserved in the layout nor drawn — so tabs look clean even when icons
    /// can't be displayed.
    pub icons_available: bool,
    /// If `Some(ms)`, hovering over an inactive tab for `ms` milliseconds
    /// activates it automatically (Edge / Win11-style). `None` disables.
    pub hover_activate_ms: Option<u32>,
    /// If `Some(ms)`, hovering over an *inactive* tab for `ms` milliseconds
    /// shows a Windows-taskbar-peek-style preview popup with a live
    /// re-render of the tab's content. `None` disables.
    ///
    /// The active tab never shows a preview (no point peeking content the
    /// user is already looking at).
    pub preview_hover_ms: Option<u32>,
    /// Preview popup base size `[width, height]` in pixels.
    /// `width` is enforced as the tooltip's width; `height` acts as a hint
    /// for the upper bound (the popup auto-grows up to ~8× this height to
    /// fit content without ever showing a scrollbar). Default `[370, 250]`.
    pub preview_size: [f32; 2],
    /// Font scale applied inside the preview popup. Smaller values fit more
    /// content in a compact box, mimicking a thumbnail. Default `0.85`.
    pub preview_font_scale: f32,
    /// Visual style of the close button glyph. See [`CloseGlyph`].
    pub close_glyph: CloseGlyph,
    /// Width of a pinned tab. Pinned tabs are compact (icon-only when
    /// available, otherwise a 1-letter fallback) and live in a non-scrolling
    /// strip on the left.
    pub pinned_tab_width: f32,

    // ── Tab strip layout ──
    /// Visual style of the tabs themselves.
    pub tab_style: TabStyle,
    /// Show the small accent underline on the active tab (Card / Square).
    pub show_tab_underline: bool,
    pub tab_height: f32,
    pub tab_rounding: f32,
    pub tab_padding_h: f32,
    pub tab_gap: f32,
    pub tab_min_width: f32,
    pub tab_max_width: f32,
    pub close_btn_size: f32,
    pub close_btn_gap: f32,
    pub strip_padding_v: f32,
    pub scroll_btn_width: f32,
    pub scroll_speed: f32,
    /// Smooth scroll animation toward `scroll_target`.
    pub smooth_scroll: bool,
    /// Animate newly added tabs (grow from 0 to full width).
    pub animate_open: bool,
    /// Animate closing tabs (shrink to 0 then remove).
    pub animate_close: bool,
    /// Show a centered placeholder when there are no tabs.
    pub show_empty_placeholder: bool,
    /// Show the small per-tab status indicator dot. When `false`, the dot
    /// slot is removed from layout entirely (title shifts left). Per-tab
    /// override is also available via [`TabStatus::None`]. Default `true`.
    pub show_status_dot: bool,

    // ── Appearance ──
    pub colors: TabColors,
    pub strings: TabStrings,

    /// User-visible language for the popup confirmations and the
    /// empty-state placeholder. Default [`crate::i18n::Locale::En`].
    /// Switching to [`crate::i18n::Locale::Ru`] requires the host to
    /// bake `GlyphRanges::Cyrillic` (or a superset) into the active
    /// font atlas — without that, non-ASCII characters render as `?`.
    ///
    /// `strings` is automatically refreshed when `locale` changes
    /// through [`crate::tab_control::TabControl::set_locale`]. Hosts
    /// that want a custom (third-language) catalogue can still
    /// assign `strings` directly — that bypasses the locale match.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

impl Default for TabControlConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron")).expect("built-in tab_control/config.ron is valid")
    }
}

impl TabControlConfig {
    /// Total tab strip height (tab + vertical padding × 2).
    #[inline]
    pub fn strip_height(&self) -> f32 {
        self.tab_height + self.strip_padding_v * 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    #[test]
    fn config_default_locale_is_english() {
        let cfg = TabControlConfig::default();
        assert_eq!(cfg.locale, Locale::En);
    }

    #[test]
    fn config_locale_round_trips_through_ron() {
        let cfg = TabControlConfig {
            locale: Locale::Ru,
            ..TabControlConfig::default()
        };
        let text = ron::ser::to_string(&cfg).unwrap();
        let back: TabControlConfig = ron::from_str(&text).unwrap();
        assert_eq!(back.locale, Locale::Ru);
    }

    #[test]
    fn config_locale_field_optional_in_ron() {
        // Older configs without `locale:` must fall back to English.
        // Use the canonical default but strip the locale line so
        // forward-compat behaviour is exercised.
        let canonical = ron::ser::to_string(&TabControlConfig::default()).unwrap();
        // Crude but sufficient: strip the trailing `,locale:En`.
        let stripped = canonical.replace(",locale:En", "");
        let cfg: TabControlConfig = ron::from_str(&stripped)
            .expect("tab_control config without `locale:` field must still parse");
        assert_eq!(cfg.locale, Locale::En);
    }
}
