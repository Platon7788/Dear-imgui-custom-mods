//! Color themes for the borderless titlebar.
//!
//! Six built-in presets: [`Dark`](TitlebarTheme::Dark), [`Light`](TitlebarTheme::Light),
//! [`Midnight`](TitlebarTheme::Midnight), [`Nord`](TitlebarTheme::Nord),
//! [`Solarized`](TitlebarTheme::Solarized), [`Monokai`](TitlebarTheme::Monokai),
//! plus fully [`Custom`](TitlebarTheme::Custom) colors.

/// A complete set of colors for the borderless titlebar.
#[derive(Debug, Clone)]
pub struct TitlebarColors {
    /// Titlebar background.
    pub bg: [f32; 4],
    /// 1-px separator line below the titlebar.
    pub separator: [f32; 4],
    /// Title text color.
    pub title: [f32; 4],
    /// Minimize button icon color.
    pub btn_minimize: [f32; 4],
    /// Maximize / restore button icon color.
    pub btn_maximize: [f32; 4],
    /// Close button icon color.
    pub btn_close: [f32; 4],
    /// Hover background for minimize and maximize buttons.
    pub btn_hover_bg: [f32; 4],
    /// Hover background for the close button.
    pub btn_close_hover_bg: [f32; 4],
    /// Window icon color (if [`BorderlessConfig::icon`](super::config::BorderlessConfig::icon) is set).
    pub icon: [f32; 4],
    /// Titlebar background color used to "erase" overlapping icon layers (restore icon).
    pub bg_erase: [f32; 4],
    /// Subtle hover tint over the drag-move zone.
    pub drag_hint: [f32; 4],
    /// Titlebar background when the window loses OS focus.
    pub bg_inactive: [f32; 4],
    /// Title text color when the window loses OS focus.
    pub title_inactive: [f32; 4],
}

/// Theme selector for the titlebar.
#[derive(Debug, Clone, Default)]
pub enum TitlebarTheme {
    /// NxT native dark palette — lifted into the library as the default Dark.
    /// See [`crate::theme::dark`] for the full stack.
    #[default]
    Dark,
    /// Readable light palette with clearly visible borders.
    /// See [`crate::theme::light`] for the full stack.
    Light,
    /// Near-black, high-contrast (VS Code dark+ inspired).
    Midnight,
    /// Solarized dark palette.
    Solarized,
    /// Monokai Pro palette.
    Monokai,
    /// Fully custom colors.
    Custom(Box<TitlebarColors>),
}

impl TitlebarTheme {
    /// Resolve to concrete [`TitlebarColors`].
    pub fn colors(&self) -> TitlebarColors {
        match self {
            Self::Dark      => crate::theme::dark::titlebar_colors(),
            Self::Light     => crate::theme::light::titlebar_colors(),
            Self::Midnight  => Self::midnight_colors(),
            Self::Solarized => Self::solarized_colors(),
            Self::Monokai   => Self::monokai_colors(),
            Self::Custom(c) => *c.clone(),
        }
    }

    // ── Built-in palettes ────────────────────────────────────────────────────
    //
    // The Dark and Light palettes now live in `crate::theme::dark` /
    // `crate::theme::light` along with their full stack (nav / dialog /
    // statusbar / ImGui style). The remaining three palettes are inline —
    // their nav / dialog / statusbar colors come from the
    // `From<&TitlebarTheme>` derivations in each component's theme file.

    pub fn midnight_colors() -> TitlebarColors {
        let bg = [0.07, 0.07, 0.09, 1.0];
        TitlebarColors {
            bg,
            separator:          [0.14, 0.16, 0.20, 1.0],
            title:              [0.75, 0.75, 0.78, 1.0],
            btn_minimize:       [1.00, 0.72, 0.00, 1.0],
            btn_maximize:       [0.28, 0.69, 1.00, 1.0],
            btn_close:          [1.00, 0.35, 0.35, 1.0],
            btn_hover_bg:       [0.17, 0.19, 0.26, 0.88],
            btn_close_hover_bg: [0.60, 0.10, 0.10, 0.92],
            icon:               [0.75, 0.75, 0.78, 1.0],
            bg_erase: bg,
            drag_hint:          [0.14, 0.16, 0.24, 0.35],
            bg_inactive:        [0.06, 0.06, 0.08, 1.0],
            title_inactive:     [0.40, 0.40, 0.44, 1.0],
        }
    }

    pub fn solarized_colors() -> TitlebarColors {
        // Solarized dark: base02 #073642, base0 #839496, blue #268BD2, red #DC322F
        let bg = [0.03, 0.21, 0.26, 1.0];
        TitlebarColors {
            bg,
            separator:          [0.35, 0.43, 0.46, 1.0],
            title:              [0.51, 0.58, 0.59, 1.0],
            btn_minimize:       [0.71, 0.54, 0.00, 1.0],
            btn_maximize:       [0.15, 0.55, 0.82, 1.0],
            btn_close:          [0.86, 0.20, 0.18, 1.0],
            btn_hover_bg:       [0.05, 0.27, 0.33, 0.88],
            btn_close_hover_bg: [0.55, 0.12, 0.10, 0.90],
            icon:               [0.51, 0.58, 0.59, 1.0],
            bg_erase: bg,
            drag_hint:          [0.05, 0.28, 0.35, 0.30],
            bg_inactive:        [0.02, 0.17, 0.21, 1.0],
            title_inactive:     [0.28, 0.35, 0.36, 1.0],
        }
    }

    pub fn monokai_colors() -> TitlebarColors {
        // Monokai Pro: #272822 bg, #F92672 pink, #A6E22E green, #E6DB74 yellow
        let bg = [0.15, 0.15, 0.15, 1.0];
        TitlebarColors {
            bg,
            separator:          [0.22, 0.22, 0.22, 1.0],
            title:              [0.95, 0.94, 0.93, 1.0],
            btn_minimize:       [0.90, 0.86, 0.45, 1.0],
            btn_maximize:       [0.65, 0.89, 0.18, 1.0],
            btn_close:          [0.98, 0.15, 0.45, 1.0],
            btn_hover_bg:       [0.26, 0.26, 0.26, 0.85],
            btn_close_hover_bg: [0.65, 0.08, 0.28, 0.92],
            icon:               [0.95, 0.94, 0.93, 1.0],
            bg_erase: bg,
            drag_hint:          [0.22, 0.22, 0.22, 0.35],
            bg_inactive:        [0.11, 0.11, 0.11, 1.0],
            title_inactive:     [0.48, 0.48, 0.46, 1.0],
        }
    }
}
