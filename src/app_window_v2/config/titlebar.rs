//! Titlebar configuration: chrome, button bar, extra buttons.

use super::enums::{CloseModeV2, TitleAlignV2};

// ── Extra titlebar button ─────────────────────────────────────────────────────

/// A custom button rendered to the left of the standard buttons.
#[derive(Debug, Clone)]
pub struct ExtraButtonV2 {
    pub id: &'static str,
    pub label: &'static str,
    pub tooltip: Option<&'static str>,
    pub color: [f32; 4],
}

impl ExtraButtonV2 {
    pub fn new(id: &'static str, label: &'static str, color: [f32; 4]) -> Self {
        Self {
            id,
            label,
            tooltip: None,
            color,
        }
    }
    pub fn with_tooltip(mut self, t: &'static str) -> Self {
        self.tooltip = Some(t);
        self
    }
}

// ── ButtonsV2 ────────────────────────────────────────────────────────────────

/// Which standard buttons to draw in the custom titlebar.
#[derive(Debug, Clone)]
pub struct ButtonsV2 {
    pub minimize: bool,
    pub maximize: bool,
    pub close: bool,
    /// Width of each button cell (px). Default: `44.0`.
    pub width: f32,
    /// Icon canvas radius (px). Default: `6.0`.
    pub icon_radius: f32,
    /// Hover highlight padding (px). Default: `4.0`.
    pub icon_hover_pad: f32,
}

impl Default for ButtonsV2 {
    fn default() -> Self {
        Self {
            minimize: true,
            maximize: true,
            close: true,
            width: 44.0,
            icon_radius: 6.0,
            icon_hover_pad: 4.0,
        }
    }
}

impl ButtonsV2 {
    /// Close-only set (used by tool windows and dialogs).
    pub fn close_only() -> Self {
        Self {
            minimize: false,
            maximize: false,
            close: true,
            ..Self::default()
        }
    }
}

// ── TitlebarConfigV2 ──────────────────────────────────────────────────────────

/// Pixel-level titlebar configuration. Used inside [`ChromeV2::Custom`].
#[derive(Debug, Clone)]
pub struct TitlebarConfigV2 {
    pub height: f32,
    pub title_visible: bool,
    pub title_align: TitleAlignV2,
    pub title_padding_left: f32,
    pub icon: Option<String>,
    pub separator_visible: bool,
    pub separator_height: f32,
    pub double_click_maximize: bool,
    pub buttons: ButtonsV2,
    pub extras: Vec<ExtraButtonV2>,
    pub close_mode: CloseModeV2,
}

impl Default for TitlebarConfigV2 {
    fn default() -> Self {
        Self {
            height: 28.0,
            title_visible: true,
            title_align: TitleAlignV2::Left,
            title_padding_left: 10.0,
            icon: None,
            separator_visible: true,
            separator_height: 1.0,
            double_click_maximize: true,
            buttons: ButtonsV2::default(),
            extras: Vec::new(),
            close_mode: CloseModeV2::Immediate,
        }
    }
}

impl TitlebarConfigV2 {
    /// Compact preset for tool windows (smaller height, close-only).
    pub fn tool() -> Self {
        Self {
            height: 22.0,
            buttons: ButtonsV2::close_only(),
            double_click_maximize: false,
            ..Self::default()
        }
    }
    /// Dialog preset (fixed-size feel — no maximize).
    pub fn dialog() -> Self {
        Self {
            buttons: ButtonsV2::close_only(),
            double_click_maximize: false,
            ..Self::default()
        }
    }
}

// ── ChromeV2 ──────────────────────────────────────────────────────────────────

/// Titlebar mode selector. `None` for borderless splash; `Custom` for everything else.
#[derive(Debug, Clone)]
pub enum ChromeV2 {
    /// No titlebar at all. The whole client area is yours.
    None,
    /// Custom titlebar drawn by the framework.
    Custom(TitlebarConfigV2),
}

impl Default for ChromeV2 {
    fn default() -> Self {
        Self::Custom(TitlebarConfigV2::default())
    }
}
