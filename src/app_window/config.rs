//! Window startup configuration.

use crate::borderless_window::BorderlessConfig;
use crate::theme::Theme;

/// Where to place the window on startup.
#[derive(Debug, Clone, Default)]
pub enum StartPosition {
    /// Centred on the primary monitor. Default.
    #[default]
    CenterScreen,
    /// Top-left corner of the primary monitor.
    TopLeft,
    /// Explicit physical-pixel coordinates.
    Custom(i32, i32),
}

/// Full configuration for an [`AppWindow`](super::AppWindow).
///
/// # Example
/// ```rust,no_run
/// use dear_imgui_custom_mod::app_window::{AppConfig, StartPosition};
///
/// let cfg = AppConfig::new("My App", 1100.0, 680.0)
///     .with_min_size(640.0, 400.0)
///     .with_fps_limit(60)
///     .with_start_position(StartPosition::CenterScreen);
/// ```
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Window title (also used as the titlebar title by default).
    pub title: String,
    /// Initial window size in logical pixels. Default: `[1100.0, 680.0]`.
    pub size: [f64; 2],
    /// Minimum window size in logical pixels. Default: `[640.0, 400.0]`.
    pub min_size: [f64; 2],
    /// Where to place the window on startup. Default: `CenterScreen`.
    pub start_position: StartPosition,
    /// Target frames per second (0 = unlimited). Default: `60`.
    pub fps_limit: u32,
    /// Base font size in logical pixels. Default: `15.0`.
    pub font_size: f32,
    /// Rounded-corner radius in pixels (Win10 fallback path; Win11 DWM ignores it).
    /// Default: `8`.
    pub corner_radius: i32,
    /// Titlebar configuration.
    pub titlebar: BorderlessConfig,
    /// Merge Material Design Icons font into the default font atlas.
    /// Required for MDI icon codepoints (U+F0000–U+F1FFF) in nav panel buttons etc.
    /// Default: `false`.
    pub merge_mdi_icons: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Application".into(),
            size: [1100.0, 680.0],
            min_size: [640.0, 400.0],
            start_position: StartPosition::CenterScreen,
            fps_limit: 60,
            font_size: 15.0,
            corner_radius: 8,
            titlebar: BorderlessConfig::default(),
            merge_mdi_icons: false,
        }
    }
}

impl AppConfig {
    /// Create a config with the given title and window size.
    pub fn new(title: impl Into<String>, width: f64, height: f64) -> Self {
        let title = title.into();
        let titlebar = BorderlessConfig::new(title.clone());
        Self {
            title,
            size: [width, height],
            titlebar,
            ..Self::default()
        }
    }

    /// Set the minimum window size.
    pub fn with_min_size(mut self, w: f64, h: f64) -> Self {
        self.min_size = [w, h];
        self
    }

    /// Set the target frames per second (0 = unlimited).
    pub fn with_fps_limit(mut self, fps: u32) -> Self {
        self.fps_limit = fps;
        self
    }

    /// Set the base font size in logical pixels.
    pub fn with_font_size(mut self, sz: f32) -> Self {
        self.font_size = sz;
        self
    }

    /// Set the rounded-corner radius (Win10 fallback path; Win11 DWM ignores it).
    pub fn with_corner_radius(mut self, r: i32) -> Self {
        self.corner_radius = r;
        self
    }

    /// Set the window start position.
    pub fn with_start_position(mut self, p: StartPosition) -> Self {
        self.start_position = p;
        self
    }

    /// Replace the entire titlebar configuration.
    pub fn with_titlebar(mut self, t: BorderlessConfig) -> Self {
        self.titlebar = t;
        self
    }

    /// Apply a theme to the titlebar.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.titlebar = self.titlebar.with_theme(theme);
        self
    }

    /// Merge Material Design Icons into the font atlas.
    /// Enables MDI codepoints (U+F0000–U+F1FFF) for icons in nav panel buttons and UI widgets.
    pub fn with_mdi_icons(mut self) -> Self {
        self.merge_mdi_icons = true;
        self
    }
}
