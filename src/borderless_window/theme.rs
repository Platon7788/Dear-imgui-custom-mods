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
    ///
    /// All built-in themes delegate to their dedicated module in
    /// `crate::theme::*` so the full theme stack (titlebar + nav + dialog +
    /// statusbar + ImGui style) lives in one file per theme.
    pub fn colors(&self) -> TitlebarColors {
        match self {
            Self::Dark      => crate::theme::dark::titlebar_colors(),
            Self::Light     => crate::theme::light::titlebar_colors(),
            Self::Midnight  => crate::theme::midnight::titlebar_colors(),
            Self::Solarized => crate::theme::solarized::titlebar_colors(),
            Self::Monokai   => crate::theme::monokai::titlebar_colors(),
            Self::Custom(c) => *c.clone(),
        }
    }

    // All built-in palettes live in `crate::theme::*` modules.
    // `TitlebarTheme::colors()` above dispatches to them.
}
