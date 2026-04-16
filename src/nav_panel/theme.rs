//! Color themes for the navigation panel.

use crate::borderless_window::TitlebarTheme;

/// Complete color set for the navigation panel.
#[derive(Debug, Clone)]
pub struct NavColors {
    /// Panel background.
    pub bg: [f32; 4],
    /// Button hover background.
    pub btn_hover: [f32; 4],
    /// Active button background.
    pub btn_active: [f32; 4],
    /// Active indicator bar color (accent).
    pub indicator: [f32; 4],
    /// Default icon tint (monochrome fallback).
    pub icon_default: [f32; 4],
    /// Icon color when active.
    pub icon_active: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],
    /// Badge circle background.
    pub badge_bg: [f32; 4],
    /// Badge text color.
    pub badge_text: [f32; 4],
    /// Submenu flyout background.
    pub submenu_bg: [f32; 4],
    /// Submenu item hover.
    pub submenu_hover: [f32; 4],
    /// Submenu item text.
    pub submenu_text: [f32; 4],
    /// Submenu border.
    pub submenu_border: [f32; 4],
    /// Submenu separator.
    pub submenu_separator: [f32; 4],
    /// Toggle button icon color.
    pub toggle_icon: [f32; 4],
}

/// Theme selector for the nav panel.
#[derive(Debug, Clone, Default)]
pub enum NavTheme {
    #[default]
    Dark,
    Light,
    Midnight,
    Solarized,
    Monokai,
    Custom(Box<NavColors>),
}

impl NavTheme {
    pub fn colors(&self) -> NavColors {
        match self {
            Self::Dark      => crate::theme::dark::nav_colors(),
            Self::Light     => crate::theme::light::nav_colors(),
            Self::Midnight  => Self::midnight(),
            Self::Solarized => Self::solarized(),
            Self::Monokai   => Self::monokai(),
            Self::Custom(c) => *c.clone(),
        }
    }

    // Dark and Light palettes live in `crate::theme::{dark,light}` — see
    // `NavTheme::colors()` above for the dispatch.

    pub fn midnight() -> NavColors {
        NavColors {
            bg:               [0.06, 0.06, 0.08, 1.0],
            btn_hover:        [0.14, 0.16, 0.22, 1.0],
            btn_active:       [0.10, 0.12, 0.16, 1.0],
            indicator:        [0.30, 0.70, 1.00, 1.0],
            icon_default:     [0.48, 0.48, 0.54, 1.0],
            icon_active:      [0.90, 0.90, 0.92, 1.0],
            separator:        [0.16, 0.18, 0.24, 0.60],
            badge_bg:         [0.90, 0.25, 0.25, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.10, 0.10, 0.14, 1.0],
            submenu_hover:    [0.18, 0.20, 0.28, 1.0],
            submenu_text:     [0.85, 0.85, 0.88, 1.0],
            submenu_border:   [0.22, 0.24, 0.32, 0.80],
            submenu_separator:[0.18, 0.20, 0.28, 0.60],
            toggle_icon:      [0.44, 0.44, 0.50, 1.0],
        }
    }

    pub fn solarized() -> NavColors {
        NavColors {
            bg:               [0.02, 0.17, 0.22, 1.0],
            btn_hover:        [0.04, 0.24, 0.30, 1.0],
            btn_active:       [0.03, 0.20, 0.26, 1.0],
            indicator:        [0.15, 0.55, 0.82, 1.0],
            icon_default:     [0.40, 0.48, 0.50, 1.0],
            icon_active:      [0.93, 0.91, 0.84, 1.0],
            separator:        [0.30, 0.38, 0.42, 0.60],
            badge_bg:         [0.86, 0.20, 0.18, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.04, 0.22, 0.28, 1.0],
            submenu_hover:    [0.06, 0.28, 0.35, 1.0],
            submenu_text:     [0.51, 0.58, 0.59, 1.0],
            submenu_border:   [0.35, 0.43, 0.46, 0.80],
            submenu_separator:[0.30, 0.38, 0.42, 0.60],
            toggle_icon:      [0.38, 0.46, 0.48, 1.0],
        }
    }

    pub fn monokai() -> NavColors {
        NavColors {
            bg:               [0.12, 0.12, 0.12, 1.0],
            btn_hover:        [0.22, 0.22, 0.22, 1.0],
            btn_active:       [0.17, 0.17, 0.17, 1.0],
            indicator:        [0.65, 0.89, 0.18, 1.0],
            icon_default:     [0.52, 0.52, 0.50, 1.0],
            icon_active:      [0.95, 0.94, 0.93, 1.0],
            separator:        [0.24, 0.24, 0.24, 0.60],
            badge_bg:         [0.98, 0.15, 0.45, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.18, 0.18, 0.18, 1.0],
            submenu_hover:    [0.26, 0.26, 0.26, 1.0],
            submenu_text:     [0.90, 0.90, 0.88, 1.0],
            submenu_border:   [0.30, 0.30, 0.30, 0.80],
            submenu_separator:[0.26, 0.26, 0.26, 0.60],
            toggle_icon:      [0.48, 0.48, 0.46, 1.0],
        }
    }
}

// ─── Conversion from a borderless-window titlebar theme ──────────────────────

/// Derive a matching [`NavColors`] from a [`TitlebarTheme`].
///
/// This lets applications drive every visual component from a single theme
/// selection: if the titlebar is in `Midnight`, the nav panel, status bar and
/// confirm dialog can all pick up the same palette automatically.
///
/// The mapping is opinionated but consistent across all seven built-in
/// titlebar themes:
/// - `bg`, `btn_hover`, `btn_active`    ← titlebar `bg` / `btn_hover_bg`
/// - `indicator`                        ← titlebar `btn_maximize` (accent)
/// - `icon_default` / `icon_active`     ← titlebar `title_inactive` / `title`
/// - `separator`                        ← titlebar `separator`
/// - `badge_bg` / `badge_text`          ← close-button red / white (stable across themes)
/// - `submenu_*`                        ← derived from the matching surface colors
impl From<&TitlebarTheme> for NavColors {
    fn from(theme: &TitlebarTheme) -> Self {
        let tb = theme.colors();
        let badge_bg = tb.btn_close;
        Self {
            bg: tb.bg,
            btn_hover: tb.btn_hover_bg,
            btn_active: tb.btn_hover_bg,
            indicator: tb.btn_maximize,
            icon_default: tb.title_inactive,
            icon_active: tb.title,
            separator: tb.separator,
            badge_bg,
            badge_text: [1.0, 1.0, 1.0, 1.0],
            submenu_bg: tb.bg,
            submenu_hover: tb.btn_hover_bg,
            submenu_text: tb.title,
            submenu_border: tb.separator,
            submenu_separator: tb.separator,
            toggle_icon: tb.title_inactive,
        }
    }
}

/// Convenience: `NavTheme::from(&TitlebarTheme::Midnight)` → `NavTheme::Custom(Box<NavColors>)`.
impl From<&TitlebarTheme> for NavTheme {
    fn from(theme: &TitlebarTheme) -> Self {
        NavTheme::Custom(Box::new(NavColors::from(theme)))
    }
}
