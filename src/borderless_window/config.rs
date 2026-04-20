//! Configuration for the borderless window titlebar.

use super::theme::TitlebarColors;
use crate::theme::Theme;

// ── Close mode ───────────────────────────────────────────────────────────────

/// How to handle the close button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloseMode {
    /// Close immediately — returns [`WindowAction::Close`](super::actions::WindowAction::Close).
    #[default]
    Immediate,
    /// Return [`WindowAction::CloseRequested`](super::actions::WindowAction::CloseRequested) first.
    /// Show your own dialog, then call [`TitlebarState::confirm_close`](super::state::TitlebarState::confirm_close).
    Confirm,
}

// ── Extra button ─────────────────────────────────────────────────────────────

/// A custom button added to the titlebar (left of standard buttons).
#[derive(Debug, Clone)]
pub struct ExtraButton {
    /// Unique id returned in [`WindowAction::Extra`](super::actions::WindowAction::Extra).
    pub id: &'static str,
    /// Icon character or short string (e.g. a Unicode glyph or ASCII shorthand).
    pub label: &'static str,
    /// Optional tooltip shown on hover.
    pub tooltip: Option<&'static str>,
    /// Icon color.
    pub color: [f32; 4],
}

impl ExtraButton {
    /// Create a new extra button.
    pub fn new(id: &'static str, label: &'static str, color: [f32; 4]) -> Self {
        Self {
            id,
            label,
            tooltip: None,
            color,
        }
    }
    /// Attach a tooltip.
    pub fn with_tooltip(mut self, tip: &'static str) -> Self {
        self.tooltip = Some(tip);
        self
    }
}

// ── Title alignment ──────────────────────────────────────────────────────────

/// Horizontal alignment of the title text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleAlign {
    /// Left-aligned (after icon, with left padding).
    #[default]
    Left,
    /// Centered between left edge and button area.
    Center,
}

// ── Button config ────────────────────────────────────────────────────────────

/// Configuration for the titlebar window-control buttons.
#[derive(Debug, Clone)]
pub struct ButtonConfig {
    /// Show minimize button. Default: `true`.
    pub show_minimize: bool,
    /// Show maximize / restore button. Default: `true`.
    pub show_maximize: bool,
    /// Show close button. Default: `true`.
    pub show_close: bool,
    /// Width (px) of each standard button cell. Default: `44.0`.
    pub width: f32,
    /// Radius (px) of the icon drawing canvas (icon is drawn in `[-r,+r]` square). Default: `6.0`.
    pub icon_radius: f32,
    /// Padding (px) around the icon for the hover highlight rectangle. Default: `4.0`.
    pub icon_hover_pad: f32,
    /// Custom buttons rendered left of the standard buttons.
    pub extra: Vec<ExtraButton>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            show_minimize: true,
            show_maximize: true,
            show_close: true,
            width: 44.0,
            icon_radius: 6.0,
            icon_hover_pad: 4.0,
            extra: Vec::new(),
        }
    }
}

impl ButtonConfig {
    /// Add a custom extra button.
    pub fn add_extra(mut self, btn: ExtraButton) -> Self {
        self.extra.push(btn);
        self
    }
    /// Hide the minimize button.
    pub fn no_minimize(mut self) -> Self {
        self.show_minimize = false;
        self
    }
    /// Hide the maximize / restore button.
    pub fn no_maximize(mut self) -> Self {
        self.show_maximize = false;
        self
    }
}

// ── Main config ──────────────────────────────────────────────────────────────

/// Full configuration for the borderless window titlebar.
///
/// # Example
/// ```rust,no_run
/// # use dear_imgui_custom_mod::borderless_window::*;
/// # use dear_imgui_custom_mod::theme::Theme;
/// let cfg = BorderlessConfig::new("My App")
///     .with_theme(Theme::Solarized)
///     .with_title_align(TitleAlign::Center)
///     .with_close_mode(CloseMode::Confirm)
///     .with_icon("\u{2302}")
///     .with_buttons(
///         ButtonConfig::default()
///             .add_extra(ExtraButton::new("theme", "\u{263D}", [0.8, 0.8, 0.5, 1.0])
///                 .with_tooltip("Toggle theme"))
///     );
/// ```
#[derive(Debug, Clone)]
pub struct BorderlessConfig {
    /// Window title text.
    pub title: String,
    /// Titlebar height in pixels. Default: `28.0`.
    pub titlebar_height: f32,
    /// Edge / corner resize detection zone width (px). Default: `6.0`.
    pub resize_zone: f32,
    /// Height of the separator line below the titlebar (px). Default: `1.0`.
    pub separator_height: f32,
    /// Color theme selector. Concrete palette is resolved at render time via
    /// [`Theme::titlebar`](crate::theme::Theme::titlebar), unless
    /// [`colors_override`](Self::colors_override) is set.
    pub theme: Theme,
    /// Optional custom palette that bypasses [`theme`](Self::theme). Set via
    /// [`with_colors`](Self::with_colors); useful for third-party themes or
    /// tint overrides that do not fit the built-in palette set.
    pub colors_override: Option<Box<TitlebarColors>>,
    /// Title text alignment.
    pub title_align: TitleAlign,
    /// Optional icon character shown before the title.
    pub icon: Option<String>,
    /// Button configuration.
    pub buttons: ButtonConfig,
    /// Maximize window on titlebar double-click. Default: `true`.
    pub double_click_maximize: bool,
    /// Left padding before icon / title (px). Default: `10.0`.
    pub title_padding_left: f32,
    /// Close button behavior.
    pub close_mode: CloseMode,
    /// Show the 1-px separator line below the titlebar. Default: `true`.
    pub separator_visible: bool,
    /// Highlight the drag-move zone on hover. Default: `true`.
    pub show_drag_hint: bool,
    /// Dim titlebar colors when the window loses OS focus. Default: `false`.
    pub focus_dim: bool,
}

impl Default for BorderlessConfig {
    fn default() -> Self {
        Self {
            title: String::from("Application"),
            titlebar_height: 28.0,
            resize_zone: 6.0,
            separator_height: 1.0,
            theme: Theme::Dark,
            colors_override: None,
            title_align: TitleAlign::Left,
            icon: None,
            buttons: ButtonConfig::default(),
            double_click_maximize: true,
            title_padding_left: 10.0,
            close_mode: CloseMode::Immediate,
            separator_visible: true,
            show_drag_hint: true,
            focus_dim: false,
        }
    }
}

impl BorderlessConfig {
    /// Create a config with the given window title and all other fields at their defaults.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
    /// Set the window title text.
    pub fn with_title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    /// Set the color theme.
    pub fn with_theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self.colors_override = None;
        self
    }
    /// Use a custom [`TitlebarColors`] palette instead of the built-in theme.
    pub fn with_colors(mut self, c: TitlebarColors) -> Self {
        self.colors_override = Some(Box::new(c));
        self
    }
    /// Resolve the palette for rendering: override if set, otherwise theme.
    pub(crate) fn resolved_colors(&self) -> TitlebarColors {
        if let Some(c) = &self.colors_override {
            (**c).clone()
        } else {
            self.theme.titlebar()
        }
    }
    /// Set the titlebar height in pixels.
    pub fn with_titlebar_height(mut self, h: f32) -> Self {
        self.titlebar_height = h;
        self
    }
    /// Set the edge/corner resize detection zone width in pixels.
    pub fn with_resize_zone(mut self, z: f32) -> Self {
        self.resize_zone = z;
        self
    }
    /// Set the title text alignment.
    pub fn with_title_align(mut self, a: TitleAlign) -> Self {
        self.title_align = a;
        self
    }
    /// Set the icon character/glyph shown before the title text.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    /// Replace the entire button configuration.
    pub fn with_buttons(mut self, b: ButtonConfig) -> Self {
        self.buttons = b;
        self
    }
    /// Set the close button behavior (immediate or confirm dialog).
    pub fn with_close_mode(mut self, m: CloseMode) -> Self {
        self.close_mode = m;
        self
    }
    /// Hide the maximize / restore button.
    pub fn without_maximize(mut self) -> Self {
        self.buttons.show_maximize = false;
        self
    }
    /// Hide the minimize button.
    pub fn without_minimize(mut self) -> Self {
        self.buttons.show_minimize = false;
        self
    }
    /// Hide the 1-px separator line below the titlebar.
    pub fn without_separator(mut self) -> Self {
        self.separator_visible = false;
        self
    }
    /// Disable the drag-zone hover highlight.
    pub fn without_drag_hint(mut self) -> Self {
        self.show_drag_hint = false;
        self
    }
    /// Enable titlebar dimming when the window loses OS focus.
    pub fn with_focus_dim(mut self) -> Self {
        self.focus_dim = true;
        self
    }
    /// Disable titlebar dimming when the window loses OS focus (already the default).
    pub fn without_focus_dim(mut self) -> Self {
        self.focus_dim = false;
        self
    }
}
