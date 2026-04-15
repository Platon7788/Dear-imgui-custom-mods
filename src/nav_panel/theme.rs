//! Color themes for the navigation panel.

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
    Nord,
    Solarized,
    Monokai,
    Custom(Box<NavColors>),
}

impl NavTheme {
    pub fn colors(&self) -> NavColors {
        match self {
            Self::Dark      => Self::dark(),
            Self::Light     => Self::light(),
            Self::Midnight  => Self::midnight(),
            Self::Nord      => Self::nord(),
            Self::Solarized => Self::solarized(),
            Self::Monokai   => Self::monokai(),
            Self::Custom(c) => *c.clone(),
        }
    }

    pub fn dark() -> NavColors {
        NavColors {
            bg:               [0.10, 0.11, 0.14, 1.0],
            btn_hover:        [0.18, 0.20, 0.26, 1.0],
            btn_active:       [0.15, 0.17, 0.22, 1.0],
            indicator:        [0.35, 0.65, 0.95, 1.0],
            icon_default:     [0.55, 0.55, 0.62, 1.0],
            icon_active:      [0.92, 0.92, 0.96, 1.0],
            separator:        [0.22, 0.24, 0.30, 0.60],
            badge_bg:         [0.85, 0.28, 0.25, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.16, 0.17, 0.22, 1.0],
            submenu_hover:    [0.22, 0.24, 0.32, 1.0],
            submenu_text:     [0.88, 0.88, 0.92, 1.0],
            submenu_border:   [0.28, 0.30, 0.38, 0.80],
            submenu_separator:[0.24, 0.26, 0.34, 0.60],
            toggle_icon:      [0.50, 0.50, 0.58, 1.0],
        }
    }

    pub fn light() -> NavColors {
        NavColors {
            bg:               [0.92, 0.93, 0.95, 1.0],
            btn_hover:        [0.84, 0.85, 0.88, 1.0],
            btn_active:       [0.88, 0.89, 0.92, 1.0],
            indicator:        [0.15, 0.50, 0.85, 1.0],
            icon_default:     [0.42, 0.42, 0.50, 1.0],
            icon_active:      [0.10, 0.10, 0.18, 1.0],
            separator:        [0.78, 0.79, 0.84, 0.60],
            badge_bg:         [0.85, 0.22, 0.20, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.96, 0.96, 0.97, 1.0],
            submenu_hover:    [0.88, 0.89, 0.92, 1.0],
            submenu_text:     [0.15, 0.15, 0.22, 1.0],
            submenu_border:   [0.78, 0.79, 0.84, 0.80],
            submenu_separator:[0.82, 0.83, 0.88, 0.60],
            toggle_icon:      [0.50, 0.50, 0.58, 1.0],
        }
    }

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

    pub fn nord() -> NavColors {
        NavColors {
            bg:               [0.16, 0.18, 0.22, 1.0],
            btn_hover:        [0.22, 0.25, 0.32, 1.0],
            btn_active:       [0.19, 0.22, 0.28, 1.0],
            indicator:        [0.53, 0.75, 0.82, 1.0],
            icon_default:     [0.55, 0.58, 0.66, 1.0],
            icon_active:      [0.88, 0.90, 0.94, 1.0],
            separator:        [0.24, 0.27, 0.34, 0.60],
            badge_bg:         [0.75, 0.38, 0.42, 1.0],
            badge_text:       [1.0, 1.0, 1.0, 1.0],
            submenu_bg:       [0.20, 0.22, 0.28, 1.0],
            submenu_hover:    [0.26, 0.30, 0.38, 1.0],
            submenu_text:     [0.85, 0.87, 0.91, 1.0],
            submenu_border:   [0.28, 0.32, 0.40, 0.80],
            submenu_separator:[0.26, 0.29, 0.36, 0.60],
            toggle_icon:      [0.52, 0.56, 0.64, 1.0],
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
