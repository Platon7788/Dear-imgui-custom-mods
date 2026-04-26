//! Unified configuration for [`AppWindowV2`](super::AppWindowV2).
//!
//! v2 keeps **all** window + titlebar configuration in a single struct.
//! There is no separate "borderless config" — the window is always borderless;
//! the chrome is just visual styling on top.

use crate::theme::Theme;

// ── FPS / power ──────────────────────────────────────────────────────────────

/// FPS limiting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FpsMode {
    /// Vsync via wgpu Fifo present mode (recommended).
    #[default]
    Auto,
    /// No frame cap — render as fast as possible.
    Unlimited,
    /// Hard cap at N frames per second.
    Fixed(u32),
}

/// GPU power preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerMode {
    /// Auto: prefer discrete on plugged-in laptops, integrated on battery.
    #[default]
    Auto,
    /// High-performance: always pick the discrete GPU.
    HighPerformance,
    /// Low-power: prefer integrated, refuse software fallback.
    LowPower,
}

/// Initial position of the window on the primary monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartPosition {
    /// Center of the primary monitor.
    #[default]
    CenterScreen,
    /// Top-left of the primary monitor.
    TopLeft,
    /// Custom physical pixel coordinates.
    Custom(i32, i32),
}

// ── Title alignment / close mode ──────────────────────────────────────────────

/// How the title text is aligned in the titlebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleAlign {
    /// Left-aligned (default).
    #[default]
    Left,
    /// Centered between the icon and the system buttons.
    Center,
}

/// What happens when the user clicks the close button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloseMode {
    /// Close immediately — fires `WindowEvent::CloseRequested` → exit.
    #[default]
    Immediate,
    /// Emit a `CloseRequested` callback so the user can show a confirmation
    /// dialog and call `state.confirm_close()` to actually close.
    Confirm,
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// Configuration for the system buttons (minimize, maximize, close).
#[derive(Debug, Clone)]
pub struct ButtonConfig {
    pub show_minimize: bool,
    pub show_maximize: bool,
    pub show_close: bool,
    /// Width of one button cell in logical pixels.
    pub width: f32,
    /// Half-extent of the icon glyph in logical pixels.
    pub icon_radius: f32,
    /// Extra (custom) buttons drawn to the left of minimize.
    pub extra: Vec<ExtraButton>,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self {
            show_minimize: true,
            show_maximize: true,
            show_close: true,
            width: 36.0,
            icon_radius: 6.0,
            extra: Vec::new(),
        }
    }
}

/// A custom button rendered in the titlebar.
#[derive(Debug, Clone)]
pub struct ExtraButton {
    pub id: &'static str,
    pub label: &'static str,
    pub color: [f32; 4],
    pub tooltip: Option<&'static str>,
}

impl ExtraButton {
    pub fn new(id: &'static str, label: &'static str, color: [f32; 4]) -> Self {
        Self {
            id,
            label,
            color,
            tooltip: None,
        }
    }
    pub fn with_tooltip(mut self, tip: &'static str) -> Self {
        self.tooltip = Some(tip);
        self
    }
}

// ── Titlebar config ──────────────────────────────────────────────────────────

/// Visual configuration for the titlebar.
///
/// Hit-testing is **not** configurable here — the OS handles it via the
/// WndProc subclass; the only knob is the visual layout (height, button
/// metrics, alignment, optional icon).
#[derive(Debug, Clone)]
pub struct TitlebarConfig {
    pub title: String,
    pub titlebar_height: f32,
    /// Resize-edge thickness in logical pixels (defines the corner zones
    /// in `WM_NCHITTEST`).
    pub resize_zone: f32,
    /// Bottom separator line height (0 = no line).
    pub separator_height: f32,
    pub theme: Theme,
    pub title_align: TitleAlign,
    /// Optional icon glyph (e.g. an MDI codepoint as `String`).
    pub icon: Option<String>,
    pub buttons: ButtonConfig,
    pub close_mode: CloseMode,
    /// Visible separator line at the bottom of the titlebar.
    pub separator_visible: bool,
    /// Inactive (unfocused) color mode — dim title/bg/icon when window
    /// loses OS focus. The `WM_ACTIVATE` lifecycle is handled by winit
    /// natively; we just use its `Focused` event.
    pub focus_dim: bool,
    /// Left padding before the icon / title text.
    pub title_padding_left: f32,
    /// Double-clicking the drag area toggles maximize/restore.
    pub double_click_maximize: bool,
}

impl Default for TitlebarConfig {
    fn default() -> Self {
        Self {
            title: String::from("App"),
            titlebar_height: 30.0,
            resize_zone: 6.0,
            separator_height: 1.0,
            theme: Theme::Dark,
            title_align: TitleAlign::Left,
            icon: None,
            buttons: ButtonConfig::default(),
            close_mode: CloseMode::Immediate,
            separator_visible: true,
            focus_dim: true,
            title_padding_left: 12.0,
            double_click_maximize: true,
        }
    }
}

// ── App config ───────────────────────────────────────────────────────────────

/// Complete configuration for an [`AppWindowV2`](super::AppWindowV2) instance.
#[derive(Debug, Clone)]
pub struct AppConfigV2 {
    pub size: [f32; 2],
    pub min_size: [f32; 2],
    pub start_position: StartPosition,
    pub fps_mode: FpsMode,
    pub power_mode: PowerMode,
    pub font_size: f32,
    /// Merge MDI icon font on top of the body font for inline icons.
    pub merge_mdi_icons: bool,
    pub titlebar: TitlebarConfig,
}

impl Default for AppConfigV2 {
    fn default() -> Self {
        Self {
            size: [1024.0, 720.0],
            min_size: [400.0, 300.0],
            start_position: StartPosition::CenterScreen,
            fps_mode: FpsMode::Auto,
            power_mode: PowerMode::Auto,
            font_size: 14.0,
            merge_mdi_icons: true,
            titlebar: TitlebarConfig::default(),
        }
    }
}

impl AppConfigV2 {
    /// Convenience constructor with a title and initial size.
    pub fn new(title: impl Into<String>, w: f32, h: f32) -> Self {
        let mut cfg = Self::default();
        cfg.titlebar.title = title.into();
        cfg.size = [w, h];
        cfg
    }
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.titlebar.theme = theme;
        self
    }
    pub fn with_min_size(mut self, w: f32, h: f32) -> Self {
        self.min_size = [w, h];
        self
    }
    pub fn with_fps_mode(mut self, m: FpsMode) -> Self {
        self.fps_mode = m;
        self
    }
    pub fn with_power_mode(mut self, m: PowerMode) -> Self {
        self.power_mode = m;
        self
    }
    pub fn with_start_position(mut self, p: StartPosition) -> Self {
        self.start_position = p;
        self
    }
    pub fn with_font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }
    pub fn with_close_mode(mut self, m: CloseMode) -> Self {
        self.titlebar.close_mode = m;
        self
    }
    pub fn with_title_align(mut self, a: TitleAlign) -> Self {
        self.titlebar.title_align = a;
        self
    }
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.titlebar.icon = Some(icon.into());
        self
    }
    pub fn with_extra_button(mut self, b: ExtraButton) -> Self {
        self.titlebar.buttons.extra.push(b);
        self
    }
}
