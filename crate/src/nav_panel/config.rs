//! Configuration for the navigation panel.
//!
//! Text fields on [`NavButton`] / [`SubMenuItem`] use `Cow<'static, str>` so
//! configs assembled at compile time pay nothing (`&'static str` borrow)
//! while runtime-built configs (loaded from JSON, plugin-injected,
//! localised) work without `Box::leak` tricks. Both literal and `String`
//! callers are accepted via `impl Into<Cow<'static, str>>`.

use super::buttons::{NavButton, NavItem};
use super::enums::{ActiveStyle, ButtonStyle, DockPosition};
use super::theme::NavColors;
use crate::theme::Theme;

// ── Main config ──────────────────────────────────────────────────────────────

/// Full configuration for the navigation panel.
///
/// # Example
/// ```rust,no_run
/// # use dear_imgui_custom_mod::nav_panel::*;
/// # use dear_imgui_custom_mod::theme::Theme;
/// let cfg = NavPanelConfig::new(DockPosition::Left)
///     .with_theme(Theme::Dark)
///     .add_button(NavButton::action("home", "H", "Home")
///         .with_color([0.3, 0.6, 1.0, 1.0]))
///     .add_separator()
///     .add_button(NavButton::submenu("cfg", "*", "Settings")
///         .add_item(SubMenuItem::new("prefs", "Preferences")));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NavPanelConfig {
    /// Docking position.
    pub position: DockPosition,
    /// Color theme selector (built-in). Resolved at render time via
    /// [`Theme::nav`](crate::theme::Theme::nav), unless
    /// [`colors_override`](Self::colors_override) is set.
    pub theme: Theme,
    /// Optional custom palette that bypasses [`theme`](Self::theme).
    #[serde(skip, default)]
    pub colors_override: Option<Box<NavColors>>,

    // ── Dimensions ───────────────────────────────────────────────────────
    /// Panel width for Left/Right (px). Min: `16.0`.
    pub width: f32,
    /// Panel height for Top (px). Min: `16.0`.
    pub height: f32,
    /// Button cell size along main axis (px). Min: `14.0`.
    pub button_size: f32,
    /// Spacing between buttons (px). Default: `2.0`.
    pub button_spacing: f32,
    /// Button style for Top mode. Default: `IconOnly`.
    pub button_style: ButtonStyle,

    // ── Indicators ───────────────────────────────────────────────────────
    /// Active indicator thickness (px). Default: `3.0`.
    pub indicator_thickness: f32,
    /// Button hover/active rounding (px). Default: `6.0`.
    pub button_rounding: f32,

    // ── Hover style ──────────────────────────────────────────────────────
    /// Glyph scale factor on hover. `1.0` = no scaling, `1.20` = 20 %
    /// larger (default). Values above ~`1.5` start to look
    /// exaggerated; values below `1.05` are indistinguishable from
    /// the un-scaled glyph and probably not worth the bilinear-filter
    /// blur cost. Set to exactly `1.0` to disable the zoom entirely
    /// (icon stays the same size on hover, no other visual change).
    /// Pre-2026-04-29 there was a `HoverStyle` enum offering a flat
    /// tinted-rectangle alternative; the project owner removed it as
    /// the zoom proved sufficient for every use case.
    pub hover_zoom_scale: f32,

    // ── Active-button style ──────────────────────────────────────────────
    /// How the active button is highlighted ([`ActiveStyle::Ring`] or
    /// [`ActiveStyle::Bar`]). Default: `Ring` — a thin stroked circle
    /// around the icon, no background fill. Pre-2026-04-29 default was
    /// `Bar`; flip back via `with_active_style(ActiveStyle::Bar)` if
    /// you depended on that look.
    pub active_style: ActiveStyle,
    /// Stroke colour for [`ActiveStyle::Ring`]. `None` means
    /// "use the palette's `indicator`" — set to a dedicated colour
    /// when you want the ring to stand out against the brand accent
    /// (e.g. amber/orange focus on a blue-accent theme).
    ///
    /// Default: `Some([0.95, 0.62, 0.20, 1.0])` — warm amber that
    /// reads as orange on every built-in theme without being garish.
    pub active_ring_color: Option<[f32; 4]>,
    /// Stroke thickness for the active ring (px). Default: `1.5`.
    /// Independent of [`Self::indicator_thickness`] so the bar and
    /// ring styles can coexist with their own sensible defaults.
    pub active_ring_thickness: f32,
    /// Extra padding between the icon glyph and the active ring (px).
    /// Default: `4.0`.
    pub active_ring_padding: f32,

    // ── Separators ───────────────────────────────────────────────────────
    /// Padding around visual separators (px each side). Default: `4.0`.
    pub separator_padding: f32,
    /// Show visual separator lines between buttons. Default: `false`.
    /// When `true`, a thin line is drawn between every button (not just `NavItem::Separator`).
    pub show_button_separators: bool,

    // ── Toggle / auto-hide ───────────────────────────────────────────────
    /// Show toggle arrow button. Default: `false`.
    pub show_toggle: bool,
    /// Auto-hide when cursor leaves the panel. Default: `false`.
    pub auto_hide: bool,
    /// Auto-show when cursor enters the edge zone. Default: `true`.
    pub auto_show_on_hover: bool,
    /// Edge detection zone width for auto-show (px). Default: `6.0`.
    pub edge_zone: f32,

    // ── Animation ────────────────────────────────────────────────────────
    /// Enable slide animation. Default: `true`.
    pub animate: bool,
    /// Animation speed (progress per second). Default: `6.0`.
    pub animation_speed: f32,

    // ── Tooltips ─────────────────────────────────────────────────────────
    /// Show tooltips on hover globally. Default: `true`.
    pub show_tooltips: bool,

    // ── Submenu ──────────────────────────────────────────────────────────
    /// Submenu flyout min width (px). Default: `160.0`.
    pub submenu_min_width: f32,
    /// Submenu item height (px). Default: `26.0`.
    pub submenu_item_height: f32,

    // ── Edge offsets ─────────────────────────────────────────────────────
    /// Y offset for Top edge detection (e.g. titlebar height). Default: `0.0`.
    pub content_offset_y: f32,
    /// X offset for Left edge detection. Default: `0.0`.
    pub content_offset_x: f32,

    /// Panel items (buttons and separators).
    pub items: Vec<NavItem>,

    /// User-visible language for the panel-toggle tooltips
    /// ("Show panel" / "Toggle panel"). The nav buttons themselves
    /// keep host-supplied labels via [`NavItem::label`] — those stay
    /// host-driven (the host owns the user's mental model). Default
    /// [`crate::i18n::Locale::En`]; switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` into the active font atlas.
    /// `#[serde(default)]` so older `config.ron` files still parse.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

impl Default for NavPanelConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron")).expect("built-in nav_panel/config.ron is valid")
    }
}

impl NavPanelConfig {
    /// Create a config with position-aware defaults.
    ///
    /// **Left/Right** (vertical): width=28, button_size=24.
    /// **Top** (horizontal): height=20, button_size=18.
    pub fn new(position: DockPosition) -> Self {
        let mut cfg = Self {
            position,
            ..Self::default()
        };
        match position {
            DockPosition::Left | DockPosition::Right => {
                cfg.width = 28.0;
                cfg.button_size = 24.0;
            }
            DockPosition::Top => {
                cfg.height = 20.0;
                cfg.button_size = 18.0;
            }
        }
        cfg
    }

    // ── Builders ─────────────────────────────────────────────────────────────

    pub fn with_theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self.colors_override = None;
        self
    }
    /// Use a custom [`NavColors`] palette instead of the built-in theme.
    pub fn with_colors(mut self, c: NavColors) -> Self {
        self.colors_override = Some(Box::new(c));
        self
    }
    /// Resolved palette for rendering.
    pub(crate) fn resolved_colors(&self) -> NavColors {
        if let Some(c) = &self.colors_override {
            (**c).clone()
        } else {
            self.theme.nav()
        }
    }
    pub fn with_width(mut self, w: f32) -> Self {
        self.width = w.max(16.0);
        self
    }
    pub fn with_height(mut self, h: f32) -> Self {
        self.height = h.max(16.0);
        self
    }
    pub fn with_button_size(mut self, s: f32) -> Self {
        self.button_size = s.max(14.0);
        self
    }
    /// Set spacing between buttons (px).
    pub fn with_button_spacing(mut self, s: f32) -> Self {
        self.button_spacing = s.max(0.0);
        self
    }
    pub fn with_button_style(mut self, s: ButtonStyle) -> Self {
        self.button_style = s;
        self
    }
    pub fn with_indicator_thickness(mut self, t: f32) -> Self {
        self.indicator_thickness = t;
        self
    }
    pub fn with_button_rounding(mut self, r: f32) -> Self {
        self.button_rounding = r;
        self
    }

    /// Set the glyph scale on hover. Clamped to `[1.0, 3.0]` —
    /// `1.0` disables the zoom (icon stays the same size on hover),
    /// going above `3.0` makes the glyph overflow the button cell.
    /// Sweet spot: `1.15` – `1.30`.
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::nav_panel::*;
    /// let cfg = NavPanelConfig::new(DockPosition::Left)
    ///     .with_hover_zoom_scale(1.30); // 30 % larger on hover
    /// ```
    pub fn with_hover_zoom_scale(mut self, s: f32) -> Self {
        self.hover_zoom_scale = s.clamp(1.0, 3.0);
        self
    }

    /// Pick the active-button visual style ([`ActiveStyle::Bar`] or
    /// [`ActiveStyle::Ring`]).
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::nav_panel::*;
    /// let cfg = NavPanelConfig::new(DockPosition::Left)
    ///     .with_active_style(ActiveStyle::Ring)
    ///     .with_active_ring_color([1.0, 0.65, 0.20, 1.0]); // orange ring
    /// ```
    pub fn with_active_style(mut self, s: ActiveStyle) -> Self {
        self.active_style = s;
        self
    }

    /// Override the active-ring stroke colour. Only takes effect when
    /// [`Self::active_style`] is [`ActiveStyle::Ring`].
    pub fn with_active_ring_color(mut self, c: [f32; 4]) -> Self {
        self.active_ring_color = Some(c);
        self
    }

    /// Reset the active-ring colour to "use palette indicator".
    pub fn without_active_ring_color(mut self) -> Self {
        self.active_ring_color = None;
        self
    }

    /// Set the active-ring stroke thickness (px). Clamped to `>= 0.5`
    /// so the ring is always at least visible.
    pub fn with_active_ring_thickness(mut self, t: f32) -> Self {
        self.active_ring_thickness = t.max(0.5);
        self
    }

    /// Set the padding between the icon glyph and the active ring (px).
    pub fn with_active_ring_padding(mut self, p: f32) -> Self {
        self.active_ring_padding = p.max(0.0);
        self
    }
    pub fn with_separator_padding(mut self, p: f32) -> Self {
        self.separator_padding = p;
        self
    }
    /// Show thin separator lines between every button (not just NavItem::Separator).
    pub fn with_button_separators(mut self, v: bool) -> Self {
        self.show_button_separators = v;
        self
    }
    pub fn with_toggle_button(mut self, v: bool) -> Self {
        self.show_toggle = v;
        self
    }
    pub fn with_auto_hide(mut self, v: bool) -> Self {
        self.auto_hide = v;
        self
    }
    pub fn with_auto_show_on_hover(mut self, v: bool) -> Self {
        self.auto_show_on_hover = v;
        self
    }
    pub fn with_animate(mut self, v: bool) -> Self {
        self.animate = v;
        self
    }
    pub fn with_animation_speed(mut self, s: f32) -> Self {
        self.animation_speed = s;
        self
    }
    pub fn without_tooltips(mut self) -> Self {
        self.show_tooltips = false;
        self
    }
    pub fn with_content_offset_y(mut self, y: f32) -> Self {
        self.content_offset_y = y;
        self
    }
    pub fn with_content_offset_x(mut self, x: f32) -> Self {
        self.content_offset_x = x;
        self
    }

    pub fn add_button(mut self, btn: NavButton) -> Self {
        self.items.push(NavItem::Button(btn));
        self
    }
    pub fn add_separator(mut self) -> Self {
        self.items.push(NavItem::Separator);
        self
    }
}
