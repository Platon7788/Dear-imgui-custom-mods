//! Theme module — per-theme bundles + shared color tokens.
//!
//! Each built-in theme lives in its own file and owns the full stack for
//! that theme: titlebar colors, nav colors, dialog colors, status-bar
//! config, and the Dear ImGui style palette. This keeps the "one theme =
//! one file" rule so a single change stays contained.
//!
//! Module layout — one theme per file, each owning a full stack:
//! - [`dark`]      — NxT native dark palette (default)
//! - [`light`]     — readable light palette with clearly visible borders
//! - [`midnight`]  — near-black OLED-friendly, Tokyo Night blue accent
//! - [`solarized`] — Solarized Dark (Ethan Schoonover), warm teal surfaces
//! - [`monokai`]   — Monokai Pro, warm charcoal + neon accents
//!
//! The color tokens below are legacy constants kept for callers that tint
//! their own widgets by the Dark palette (e.g. `code_editor`, `file_manager`).
//! Prefer the full theme modules above for new code.

pub mod dark;
pub mod light;
pub mod midnight;
pub mod monokai;
pub mod solarized;

// ─── Dark theme palette (NxT-inspired) ──────────────────────────────────────

// Base backgrounds
pub const BG_WINDOW: [f32; 4] = [0.12, 0.13, 0.16, 1.0];
pub const BG_CHILD: [f32; 4] = [0.14, 0.15, 0.19, 1.0];
pub const BG_FRAME: [f32; 4] = [0.16, 0.18, 0.22, 1.0];

// Accent
pub const ACCENT: [f32; 4] = [0.36, 0.61, 0.84, 1.0];
pub const ACCENT_HOVER: [f32; 4] = [0.42, 0.67, 0.90, 1.0];
pub const ACCENT_ACTIVE: [f32; 4] = [0.30, 0.55, 0.78, 1.0];

// Success (green)
pub const SUCCESS: [f32; 4] = [0.37, 0.72, 0.44, 1.0];
pub const SUCCESS_HOVER: [f32; 4] = [0.43, 0.78, 0.50, 1.0];
pub const SUCCESS_ACTIVE: [f32; 4] = [0.31, 0.66, 0.38, 1.0];

// Danger (red)
pub const DANGER: [f32; 4] = [0.82, 0.27, 0.27, 1.0];
pub const DANGER_HOVER: [f32; 4] = [0.88, 0.33, 0.33, 1.0];
pub const DANGER_ACTIVE: [f32; 4] = [0.76, 0.21, 0.21, 1.0];

// Warning (amber/orange)
pub const WARNING: [f32; 4] = [0.85, 0.65, 0.25, 1.0];

// Text
pub const TEXT_PRIMARY: [f32; 4] = [0.88, 0.90, 0.92, 1.0];
pub const TEXT_SECONDARY: [f32; 4] = [0.54, 0.57, 0.63, 1.0];
pub const TEXT_MUTED: [f32; 4] = [0.40, 0.42, 0.48, 1.0];
pub const TEXT_ERROR: [f32; 4] = [0.92, 0.38, 0.35, 1.0];

// Borders / Separators
pub const BORDER: [f32; 4] = [0.25, 0.28, 0.33, 1.0];
pub const SEPARATOR: [f32; 4] = [0.22, 0.25, 0.30, 1.0];

// Backgrounds — interactive
pub const BG_CHILD_HOVER: [f32; 4] = [0.18, 0.20, 0.25, 1.0];

// Selection
pub const SELECTION_BG: [f32; 4] = [0.35, 0.55, 0.80, 0.40];

// ─── Light theme palette ──────────────────────────────────────────────────────

// Base backgrounds
pub const LIGHT_BG_WINDOW: [f32; 4] = [0.96, 0.96, 0.98, 1.0];
pub const LIGHT_BG_CHILD: [f32; 4]  = [0.92, 0.92, 0.95, 1.0];
pub const LIGHT_BG_FRAME: [f32; 4]  = [0.87, 0.87, 0.91, 1.0];

// Accent
pub const LIGHT_ACCENT: [f32; 4]        = [0.18, 0.48, 0.76, 1.0];
pub const LIGHT_ACCENT_HOVER: [f32; 4]  = [0.24, 0.56, 0.86, 1.0];
pub const LIGHT_ACCENT_ACTIVE: [f32; 4] = [0.14, 0.40, 0.66, 1.0];

// Status
pub const LIGHT_SUCCESS: [f32; 4] = [0.20, 0.60, 0.28, 1.0];
pub const LIGHT_DANGER:  [f32; 4] = [0.80, 0.18, 0.18, 1.0];
pub const LIGHT_WARNING: [f32; 4] = [0.76, 0.52, 0.04, 1.0];

// Text
pub const LIGHT_TEXT_PRIMARY:   [f32; 4] = [0.10, 0.10, 0.14, 1.0];
pub const LIGHT_TEXT_SECONDARY: [f32; 4] = [0.36, 0.38, 0.44, 1.0];
pub const LIGHT_TEXT_MUTED:     [f32; 4] = [0.55, 0.56, 0.62, 1.0];
pub const LIGHT_TEXT_ERROR:     [f32; 4] = [0.82, 0.18, 0.16, 1.0];

// Borders
pub const LIGHT_BORDER:    [f32; 4] = [0.70, 0.71, 0.76, 1.0];
pub const LIGHT_SEPARATOR: [f32; 4] = [0.76, 0.77, 0.82, 1.0];

// Selection
pub const LIGHT_SELECTION_BG: [f32; 4] = [0.25, 0.50, 0.82, 0.30];

// ─── Unified Theme selector ──────────────────────────────────────────────────

use crate::borderless_window::TitlebarColors;
use crate::confirm_dialog::DialogColors;
use crate::nav_panel::NavColors;
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::Style;

/// Single application-wide theme selector.
///
/// Every built-in theme owns the full stack (titlebar / nav / dialog /
/// statusbar / Dear ImGui style) through its per-theme module
/// (`crate::theme::{dark, light, midnight, solarized, monokai}`). Components
/// take this value by reference and pull the matching sub-palette via the
/// methods below — there is no per-component theme enum any more.
///
/// ```rust,no_run
/// use dear_imgui_custom_mod::theme::Theme;
/// let t = Theme::Dark.next();    // compile-time variants
/// let tb = Theme::Dark.titlebar();
/// let cols = Theme::default().nav();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// NxT native dark palette (warm grey + blue accent).
    #[default]
    Dark,
    /// Readable light palette with visible borders.
    Light,
    /// Near-black OLED-friendly (Tokyo Night accent).
    Midnight,
    /// Solarized Precision Colors — dark variant.
    Solarized,
    /// Monokai Pro — warm charcoal + neon accents.
    Monokai,
}

impl Theme {
    /// All built-in themes, ordered as they appear in Settings UIs.
    pub const ALL: &'static [Theme] = &[
        Theme::Dark,
        Theme::Light,
        Theme::Midnight,
        Theme::Solarized,
        Theme::Monokai,
    ];

    /// Human-readable English name — used in menus / combo boxes.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Midnight => "Midnight",
            Self::Solarized => "Solarized",
            Self::Monokai => "Monokai",
        }
    }

    /// Titlebar colours for this theme.
    pub fn titlebar(self) -> TitlebarColors {
        match self {
            Self::Dark => dark::titlebar_colors(),
            Self::Light => light::titlebar_colors(),
            Self::Midnight => midnight::titlebar_colors(),
            Self::Solarized => solarized::titlebar_colors(),
            Self::Monokai => monokai::titlebar_colors(),
        }
    }

    /// Nav-panel colours for this theme.
    pub fn nav(self) -> NavColors {
        match self {
            Self::Dark => dark::nav_colors(),
            Self::Light => light::nav_colors(),
            Self::Midnight => midnight::nav_colors(),
            Self::Solarized => solarized::nav_colors(),
            Self::Monokai => monokai::nav_colors(),
        }
    }

    /// Confirm-dialog colours for this theme.
    pub fn dialog(self) -> DialogColors {
        match self {
            Self::Dark => dark::dialog_colors(),
            Self::Light => light::dialog_colors(),
            Self::Midnight => midnight::dialog_colors(),
            Self::Solarized => solarized::dialog_colors(),
            Self::Monokai => monokai::dialog_colors(),
        }
    }

    /// Status-bar config (colours + default geometry) for this theme.
    pub fn statusbar(self) -> StatusBarConfig {
        match self {
            Self::Dark => dark::statusbar_config(),
            Self::Light => light::statusbar_config(),
            Self::Midnight => midnight::statusbar_config(),
            Self::Solarized => solarized::statusbar_config(),
            Self::Monokai => monokai::statusbar_config(),
        }
    }

    /// Apply this theme's Dear ImGui style (rounding + sizing + colours)
    /// to the supplied style object. Call once at startup and any time
    /// after a theme change.
    pub fn apply_imgui_style(self, style: &mut Style) {
        match self {
            Self::Dark => dark::apply_imgui_style(style),
            Self::Light => light::apply_imgui_style(style),
            Self::Midnight => midnight::apply_imgui_style(style),
            Self::Solarized => solarized::apply_imgui_style(style),
            Self::Monokai => monokai::apply_imgui_style(style),
        }
    }

    /// Cycle to the next theme in `Theme::ALL` (wraps around) — handy for
    /// a "theme" extra button in the titlebar.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}
