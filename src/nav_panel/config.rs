//! Configuration for the navigation panel.

use super::theme::NavTheme;

// ── Position ─────────────────────────────────────────────────────────────────

/// Where the panel is docked.
///
/// Three positions are supported: Left, Right, Top.
/// Bottom is intentionally omitted — that slot is reserved for
/// [`StatusBar`](crate::status_bar::StatusBar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockPosition {
    #[default]
    Left,
    Right,
    Top,
}

// ── Button style (for Top) ──────────────────────────────────────────────────

/// How buttons are rendered in horizontal (Top) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonStyle {
    /// Icon only — small square cells.
    #[default]
    IconOnly,
    /// Icon + label side by side.
    IconWithLabel,
    /// Text label only, no icon.
    LabelOnly,
}

// ── Submenu item ─────────────────────────────────────────────────────────────

/// An item in a button's flyout submenu.
#[derive(Debug, Clone)]
pub enum SubMenuItem {
    /// Clickable item.
    Item {
        id: &'static str,
        label: &'static str,
        icon: Option<&'static str>,
        shortcut: Option<&'static str>,
    },
    /// Visual separator line between items.
    Separator,
}

impl SubMenuItem {
    pub fn new(id: &'static str, label: &'static str) -> Self {
        Self::Item { id, label, icon: None, shortcut: None }
    }
    pub fn with_icon(self, icon: &'static str) -> Self {
        match self {
            Self::Item { id, label, shortcut, .. } =>
                Self::Item { id, label, icon: Some(icon), shortcut },
            s => s,
        }
    }
    pub fn with_shortcut(self, sc: &'static str) -> Self {
        match self {
            Self::Item { id, label, icon, .. } =>
                Self::Item { id, label, icon, shortcut: Some(sc) },
            s => s,
        }
    }
    pub fn separator() -> Self { Self::Separator }
}

// ── Nav button ───────────────────────────────────────────────────────────────

/// A button in the navigation panel.
#[derive(Debug, Clone)]
pub struct NavButton {
    pub id: &'static str,
    pub icon: &'static str,
    pub tooltip: &'static str,
    pub color: Option<[f32; 4]>,
    pub submenu: Vec<SubMenuItem>,
    pub badge: Option<String>,
    /// Show tooltip on hover. Default: `true`.
    pub show_tooltip: bool,
}

impl NavButton {
    pub fn action(id: &'static str, icon: &'static str, tooltip: &'static str) -> Self {
        Self { id, icon, tooltip, color: None, submenu: Vec::new(), badge: None, show_tooltip: true }
    }
    pub fn submenu(id: &'static str, icon: &'static str, tooltip: &'static str) -> Self {
        Self { id, icon, tooltip, color: None, submenu: Vec::new(), badge: None, show_tooltip: true }
    }
    pub fn with_color(mut self, c: [f32; 4]) -> Self { self.color = Some(c); self }
    pub fn add_item(mut self, item: SubMenuItem) -> Self { self.submenu.push(item); self }
    pub fn add_separator(mut self) -> Self { self.submenu.push(SubMenuItem::Separator); self }
    pub fn without_tooltip(mut self) -> Self { self.show_tooltip = false; self }
    pub fn with_badge(mut self, text: impl Into<String>) -> Self {
        self.badge = Some(text.into()); self
    }
}

// ── Nav panel item ───────────────────────────────────────────────────────────

/// An element in the navigation panel layout.
#[derive(Debug, Clone)]
pub enum NavItem {
    Button(NavButton),
    Separator,
}

// ── Main config ──────────────────────────────────────────────────────────────

/// Full configuration for the navigation panel.
///
/// # Example
/// ```rust,no_run
/// # use dear_imgui_custom_mod::nav_panel::*;
/// let cfg = NavPanelConfig::new(DockPosition::Left)
///     .with_theme(NavTheme::Dark)
///     .add_button(NavButton::action("home", "H", "Home")
///         .with_color([0.3, 0.6, 1.0, 1.0]))
///     .add_separator()
///     .add_button(NavButton::submenu("cfg", "*", "Settings")
///         .add_item(SubMenuItem::new("prefs", "Preferences")));
/// ```
#[derive(Debug, Clone)]
pub struct NavPanelConfig {
    /// Docking position.
    pub position: DockPosition,
    /// Color theme.
    pub theme: NavTheme,

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
}

impl Default for NavPanelConfig {
    fn default() -> Self {
        Self {
            position: DockPosition::Left,
            theme: NavTheme::Dark,
            width: 28.0,
            height: 24.0,
            button_size: 24.0,
            button_spacing: 4.0,
            button_style: ButtonStyle::IconOnly,
            indicator_thickness: 3.0,
            button_rounding: 6.0,
            separator_padding: 4.0,
            show_button_separators: true,
            show_toggle: false,
            auto_hide: false,
            auto_show_on_hover: true,
            edge_zone: 6.0,
            animate: true,
            animation_speed: 6.0,
            show_tooltips: true,
            submenu_min_width: 160.0,
            submenu_item_height: 26.0,
            content_offset_y: 0.0,
            content_offset_x: 0.0,
            items: Vec::new(),
        }
    }
}

impl NavPanelConfig {
    /// Create a config with position-aware defaults.
    ///
    /// **Left/Right** (vertical): width=28, button_size=24.
    /// **Top** (horizontal): height=20, button_size=18.
    pub fn new(position: DockPosition) -> Self {
        let mut cfg = Self { position, ..Self::default() };
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

    pub fn with_theme(mut self, t: NavTheme) -> Self { self.theme = t; self }
    pub fn with_width(mut self, w: f32) -> Self { self.width = w.max(16.0); self }
    pub fn with_height(mut self, h: f32) -> Self { self.height = h.max(16.0); self }
    pub fn with_button_size(mut self, s: f32) -> Self { self.button_size = s.max(14.0); self }
    /// Set spacing between buttons (px).
    pub fn with_button_spacing(mut self, s: f32) -> Self { self.button_spacing = s.max(0.0); self }
    pub fn with_button_style(mut self, s: ButtonStyle) -> Self { self.button_style = s; self }
    pub fn with_indicator_thickness(mut self, t: f32) -> Self { self.indicator_thickness = t; self }
    pub fn with_button_rounding(mut self, r: f32) -> Self { self.button_rounding = r; self }
    pub fn with_separator_padding(mut self, p: f32) -> Self { self.separator_padding = p; self }
    /// Show thin separator lines between every button (not just NavItem::Separator).
    pub fn with_button_separators(mut self, v: bool) -> Self { self.show_button_separators = v; self }
    pub fn with_toggle_button(mut self, v: bool) -> Self { self.show_toggle = v; self }
    pub fn with_auto_hide(mut self, v: bool) -> Self { self.auto_hide = v; self }
    pub fn with_auto_show_on_hover(mut self, v: bool) -> Self { self.auto_show_on_hover = v; self }
    pub fn with_animate(mut self, v: bool) -> Self { self.animate = v; self }
    pub fn with_animation_speed(mut self, s: f32) -> Self { self.animation_speed = s; self }
    pub fn without_tooltips(mut self) -> Self { self.show_tooltips = false; self }
    pub fn with_content_offset_y(mut self, y: f32) -> Self { self.content_offset_y = y; self }
    pub fn with_content_offset_x(mut self, x: f32) -> Self { self.content_offset_x = x; self }

    pub fn add_button(mut self, btn: NavButton) -> Self {
        self.items.push(NavItem::Button(btn)); self
    }
    pub fn add_separator(mut self) -> Self {
        self.items.push(NavItem::Separator); self
    }
}
