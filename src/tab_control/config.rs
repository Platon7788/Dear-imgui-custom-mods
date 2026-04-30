//! Configuration types for [`TabControl`](super::TabControl).
//!
//! Pure value types — no logic. Separated from `mod.rs` to keep the public
//! surface focused.

#![allow(missing_docs)] // numeric layout fields are self-explanatory

// ─── Tab identifier ─────────────────────────────────────────────────────────

/// Opaque, auto-incrementing tab identifier.
///
/// Assigned internally by [`TabControl::add`](super::TabControl::add).
/// Stable across removals — never reused within a single `TabControl` instance.
pub type TabId = u64;

// ─── Tab status ─────────────────────────────────────────────────────────────

/// Visual status of a tab — controls the small indicator dot drawn on the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
#[derive(Debug, Clone)]
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
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

// ─── Localization strings ───────────────────────────────────────────────────

/// User-facing strings — override for localization.
pub struct TabStrings {
    pub cancel: &'static str,
    pub close: &'static str,
    pub close_confirm: &'static str,
    /// Confirmation text shown when the tab being closed has unsaved changes
    /// (status `Dirty`). More urgent phrasing than [`Self::close_confirm`].
    pub close_confirm_dirty: &'static str,
    pub no_tabs: &'static str,
    pub empty_hint: &'static str,
    pub overflow_tooltip: &'static str,
    pub add_tab: &'static str,
}

impl Default for TabStrings {
    fn default() -> Self {
        Self {
            cancel: "Cancel",
            close: "Close",
            close_confirm: "Close this tab?",
            close_confirm_dirty: "This tab has unsaved changes. Discard and close?",
            no_tabs: "No tabs",
            empty_hint: "Add a tab to begin\u{2026}",
            overflow_tooltip: "All tabs",
            add_tab: "New tab",
        }
    }
}

// ─── Color palette ──────────────────────────────────────────────────────────

/// Color palette — all colors are `[R, G, B]` in 0..=255 range (alpha per use).
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
    /// Background of the active tab's content area — the borderless
    /// child-window the strip's `render_content()` runs inside when
    /// [`TabControlConfig::body_inset_enabled`] is `true`.
    /// Default mirrors [`Self::strip_bg`] so the content surface sits
    /// in the same plane as the strip and the 1-px padding inset
    /// reads as invisible breathing space. Set to a contrasting hue
    /// when the host wants the content area to read as a distinct
    /// surface (e.g. white-on-dark editor on a dark chrome).
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
            // Visibly darker than strip_bg so the framed-content
            // visual works on every theme out of the box (gap =
            // strip_bg, inner = body_bg). Earlier defaults of
            // ~`-0.04` lift were too subtle on dark themes — the
            // frame got lost in the surrounding chrome.
            body_bg: [0x18, 0x1c, 0x24],
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
            // Visibly darker than strip_bg so the framed-content
            // visual reads as a genuine "tab body" inset on every
            // theme out of the box. `-0.10` lift gives a clear ~25
            // u8 step on dark themes; light themes still get a
            // visible tonal step because the clamp keeps it
            // monotonic.
            body_bg: {
                let lift = -0.10_f32;
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

// ─── Configuration ──────────────────────────────────────────────────────────

/// Full configuration for [`TabControl`](super::TabControl).
///
/// All fields have sensible defaults via [`Default`].
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
    /// the chrome. Set `false` for full-bleed content (legacy
    /// behaviour) — useful for charts, hex dumps, or any widget that
    /// wants to use every available pixel.
    pub body_inset_enabled: bool,
    /// Outer inset in pixels — `[horizontal, vertical]` — applied to
    /// the active tab's content child-window when
    /// [`Self::body_inset_enabled`] is `true`. The renderer
    /// shifts the cursor inwards by this amount and shrinks the child
    /// by `2 ×` the same on both axes, producing a visible gap around
    /// the child rectangle. Default `[2.0, 2.0]`.
    pub body_inset: [f32; 2],
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
}

impl Default for TabControlConfig {
    fn default() -> Self {
        Self {
            closable: true,
            confirm_close: true,
            middle_click_close: true,
            scroll_with_wheel: true,
            keyboard_nav: true,
            show_add_button: false,
            context_menu: true,
            external_content: false,
            body_inset_enabled: true,
            body_inset: [4.0, 4.0],
            draggable: true,
            show_overflow_dropdown: true,
            icons_available: false,
            hover_activate_ms: None,
            preview_hover_ms: None,
            preview_size: [370.0, 250.0],
            preview_font_scale: 0.85,
            close_glyph: CloseGlyph::default(),
            pinned_tab_width: 36.0,

            tab_style: TabStyle::default(),
            show_tab_underline: true,
            tab_height: 26.0,
            tab_rounding: 6.0,
            tab_padding_h: 10.0,
            tab_gap: 2.0,
            tab_min_width: 80.0,
            tab_max_width: 320.0,
            close_btn_size: 12.0,
            close_btn_gap: 6.0,
            strip_padding_v: 4.0,
            scroll_btn_width: 24.0,
            scroll_speed: 220.0,
            smooth_scroll: true,
            animate_open: true,
            animate_close: true,
            show_empty_placeholder: true,
            show_status_dot: true,

            colors: TabColors::default(),
            strings: TabStrings::default(),
        }
    }
}

impl TabControlConfig {
    /// Total tab strip height (tab + vertical padding × 2).
    #[inline]
    pub fn strip_height(&self) -> f32 {
        self.tab_height + self.strip_padding_v * 2.0
    }
}
