//! Crate-wide palette types — the colour tokens consumed by every
//! component that participates in the unified theme system.
//!
//! ## Why these live here
//!
//! Colour palettes are part of the **theme contract**, not implementation
//! detail of the consuming widget. Earlier they were defined inside each
//! component (`borderless_window::TitlebarColors`, `nav_panel::NavColors`,
//! `confirm_dialog::DialogColors`, `notifications::NotificationColors`),
//! which forced [`crate::theme`] — an *infrastructure* module — to import
//! from feature-gated *consumer* modules to construct its built-in
//! palettes. That is a textbook inverse dependency: the foundation
//! reaches up into the higher layers.
//!
//! After session 022 every palette **type** lives here. Consumer
//! modules `pub use` them back, and the theme modules build palettes
//! from a single import:
//!
//! ```ignore
//! use super::{TitlebarColors, NavColors, DialogColors, NotificationColors};
//! ```
//!
//! No `#[cfg(feature = …)]` walls, no upward references.
//!
//! ## What is *not* here yet
//!
//! [`crate::status_bar::StatusBarConfig`] still combines layout fields
//! (height, padding, separator widths) with colour fields. Splitting
//! those would touch the entire `status_bar` rendering path; the
//! refactor is scheduled separately. Until then,
//! [`crate::theme::Theme::statusbar`] remains the one cross-module
//! palette accessor that depends on a consumer module.

// ── TitlebarColors ──────────────────────────────────────────────────────────

/// A complete set of colours for the borderless titlebar, consumed by
/// [`crate::app_window_v2`]'s chrome layer.
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
    /// Window icon color (if `BorderlessConfig::icon` is set).
    pub icon: [f32; 4],
    /// Titlebar background colour used to "erase" overlapping icon layers (restore icon).
    pub bg_erase: [f32; 4],
    /// Subtle hover tint over the drag-move zone.
    pub drag_hint: [f32; 4],
    /// Titlebar background when the window loses OS focus.
    pub bg_inactive: [f32; 4],
    /// Title text color when the window loses OS focus.
    pub title_inactive: [f32; 4],
}

// ── DialogColors ────────────────────────────────────────────────────────────

/// Complete colour set for the confirm dialog. Consumed by
/// [`crate::confirm_dialog`].
#[derive(Debug, Clone)]
pub struct DialogColors {
    /// Fullscreen dim overlay behind the dialog.
    pub overlay: [f32; 4],
    /// Dialog window background.
    pub bg: [f32; 4],
    /// Dialog border color.
    pub border: [f32; 4],
    /// Title / header text color.
    pub title: [f32; 4],
    /// Body message text color.
    pub message: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],

    /// Icon color for Warning type.
    pub icon_warning: [f32; 4],
    /// Icon color for Error type.
    pub icon_error: [f32; 4],
    /// Icon color for Info type.
    pub icon_info: [f32; 4],
    /// Icon color for Question type.
    pub icon_question: [f32; 4],

    /// Confirm (destructive) button background — red.
    pub btn_confirm: [f32; 4],
    /// Confirm button hover.
    pub btn_confirm_hover: [f32; 4],
    /// Confirm button active/press.
    pub btn_confirm_active: [f32; 4],
    /// Confirm button text.
    pub btn_confirm_text: [f32; 4],

    /// Cancel (safe) button background — green.
    pub btn_cancel: [f32; 4],
    /// Cancel button hover.
    pub btn_cancel_hover: [f32; 4],
    /// Cancel button active/press.
    pub btn_cancel_active: [f32; 4],
    /// Cancel button text.
    pub btn_cancel_text: [f32; 4],
}

// ── NavColors ───────────────────────────────────────────────────────────────

/// Complete colour set for the navigation panel. Consumed by
/// [`crate::nav_panel`].
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

// ── StatusBarColors ─────────────────────────────────────────────────────────

/// Colour subset for the status bar — the *theme* part of
/// [`crate::status_bar::StatusBarConfig`]. The layout fields (height,
/// padding, separator widths, progress dimensions, hover-feedback flag)
/// stay on `StatusBarConfig` because they describe how `status_bar`
/// renders rather than what colours it uses.
///
/// Built-in themes expose this both directly via
/// [`crate::theme::Theme::statusbar_colors`] (always available, no
/// feature gate) and as part of the full
/// [`crate::theme::Theme::statusbar`] config (gated on the
/// `status_bar` feature because that returns `StatusBarConfig`).
#[derive(Debug, Clone, Copy)]
pub struct StatusBarColors {
    /// Bar background color.
    pub bg: [f32; 4],
    /// Default text color.
    pub text: [f32; 4],
    /// Dimmed/secondary text color.
    pub text_dim: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],
    /// Hovered item background.
    pub hover: [f32; 4],
    /// Clicked item background.
    pub active: [f32; 4],

    /// Success indicator color (green dot).
    pub success: [f32; 4],
    /// Warning indicator color (yellow dot).
    pub warning: [f32; 4],
    /// Error indicator color (red dot).
    pub error: [f32; 4],
    /// Info indicator color (blue dot).
    pub info: [f32; 4],
}

impl Default for StatusBarColors {
    /// NxT-Dark default — matches `Theme::Dark.statusbar_colors()`. Used
    /// when a user constructs `StatusBarConfig::default()` without
    /// going through `Theme`.
    fn default() -> Self {
        Self {
            bg: [0.12, 0.12, 0.15, 1.0],
            text: [0.85, 0.87, 0.90, 1.0],
            text_dim: [0.50, 0.52, 0.58, 1.0],
            separator: [0.25, 0.27, 0.32, 0.6],
            hover: [0.20, 0.22, 0.28, 1.0],
            active: [0.25, 0.28, 0.35, 1.0],

            success: [0.30, 0.80, 0.40, 1.0],
            warning: [0.90, 0.75, 0.20, 1.0],
            error: [0.90, 0.30, 0.30, 1.0],
            info: [0.40, 0.65, 0.90, 1.0],
        }
    }
}

// ── NotificationColors ──────────────────────────────────────────────────────

/// Complete colour set for the notification center. Consumed by
/// [`crate::notifications`].
///
/// Unlike the other palettes, this one ships with five named-preset
/// constructors (`dark()`, `light()`, `midnight()`, `solarized()`,
/// `monokai()`). The built-in [`crate::theme::Theme`] variants delegate
/// to these constructors so adding a new theme that wants the default
/// notification look is one method call.
#[derive(Debug, Clone)]
pub struct NotificationColors {
    /// Toast window background.
    pub bg: [f32; 4],
    /// Toast border.
    pub border: [f32; 4],
    /// Title text.
    pub title: [f32; 4],
    /// Body text (dimmer than title).
    pub body: [f32; 4],
    /// `×` close-button glyph — default.
    pub close: [f32; 4],
    /// `×` close-button glyph — hover.
    pub close_hover: [f32; 4],
    /// Progress-bar track (background).
    pub progress_bg: [f32; 4],

    // Severity accents — used for icon color + left accent strip + progress fill.
    /// Info severity — default blue.
    pub info: [f32; 4],
    /// Success severity — default green.
    pub success: [f32; 4],
    /// Warning severity — default amber.
    pub warning: [f32; 4],
    /// Error severity — default red.
    pub error: [f32; 4],
    /// Debug severity — default gray.
    pub debug: [f32; 4],

    /// Action-button background — default.
    pub btn_action: [f32; 4],
    /// Action-button background — hover.
    pub btn_action_hover: [f32; 4],
    /// Action-button background — active / pressed.
    pub btn_action_active: [f32; 4],
    /// Action-button text.
    pub btn_action_text: [f32; 4],
}

impl NotificationColors {
    /// NxT dark palette — matches `Theme::Dark`.
    pub fn dark() -> Self {
        Self {
            bg: [0.18, 0.20, 0.24, 0.96],
            border: [0.28, 0.31, 0.37, 1.0],
            title: [0.88, 0.90, 0.92, 1.0],
            body: [0.68, 0.71, 0.77, 1.0],
            close: [0.54, 0.57, 0.63, 1.0],
            close_hover: [0.95, 0.95, 0.95, 1.0],
            progress_bg: [0.25, 0.27, 0.32, 0.8],

            info: [0.36, 0.61, 0.84, 1.0],
            success: [0.37, 0.72, 0.44, 1.0],
            warning: [0.85, 0.65, 0.25, 1.0],
            error: [0.88, 0.37, 0.37, 1.0],
            debug: [0.55, 0.58, 0.64, 1.0],

            btn_action: [0.28, 0.31, 0.38, 1.0],
            btn_action_hover: [0.35, 0.40, 0.48, 1.0],
            btn_action_active: [0.22, 0.25, 0.31, 1.0],
            btn_action_text: [0.92, 0.94, 0.96, 1.0],
        }
    }

    /// Light palette — matches `Theme::Light`.
    pub fn light() -> Self {
        Self {
            bg: [0.98, 0.98, 0.99, 0.98],
            border: [0.78, 0.80, 0.84, 1.0],
            title: [0.12, 0.14, 0.18, 1.0],
            body: [0.36, 0.39, 0.44, 1.0],
            close: [0.50, 0.54, 0.60, 1.0],
            close_hover: [0.10, 0.12, 0.16, 1.0],
            progress_bg: [0.88, 0.89, 0.92, 0.8],

            info: [0.18, 0.48, 0.76, 1.0],
            success: [0.18, 0.60, 0.32, 1.0],
            warning: [0.82, 0.55, 0.16, 1.0],
            error: [0.80, 0.22, 0.22, 1.0],
            debug: [0.46, 0.49, 0.55, 1.0],

            btn_action: [0.86, 0.88, 0.92, 1.0],
            btn_action_hover: [0.78, 0.82, 0.88, 1.0],
            btn_action_active: [0.70, 0.74, 0.82, 1.0],
            btn_action_text: [0.14, 0.16, 0.20, 1.0],
        }
    }

    /// Midnight palette — Tokyo Night accent, OLED-friendly.
    pub fn midnight() -> Self {
        Self {
            bg: [0.06, 0.07, 0.10, 0.97],
            border: [0.18, 0.20, 0.28, 1.0],
            title: [0.86, 0.88, 0.94, 1.0],
            body: [0.58, 0.62, 0.72, 1.0],
            close: [0.46, 0.49, 0.58, 1.0],
            close_hover: [0.92, 0.94, 0.98, 1.0],
            progress_bg: [0.12, 0.14, 0.20, 0.8],

            info: [0.50, 0.72, 0.96, 1.0],
            success: [0.58, 0.82, 0.62, 1.0],
            warning: [0.95, 0.78, 0.42, 1.0],
            error: [0.94, 0.46, 0.52, 1.0],
            debug: [0.48, 0.52, 0.62, 1.0],

            btn_action: [0.14, 0.17, 0.24, 1.0],
            btn_action_hover: [0.20, 0.24, 0.32, 1.0],
            btn_action_active: [0.10, 0.12, 0.18, 1.0],
            btn_action_text: [0.88, 0.90, 0.96, 1.0],
        }
    }

    /// Solarized-dark palette.
    pub fn solarized() -> Self {
        Self {
            bg: [0.0, 0.17, 0.21, 0.97],
            border: [0.03, 0.21, 0.26, 1.0],
            title: [0.93, 0.91, 0.84, 1.0],
            body: [0.51, 0.58, 0.59, 1.0],
            close: [0.40, 0.48, 0.51, 1.0],
            close_hover: [0.93, 0.91, 0.84, 1.0],
            progress_bg: [0.03, 0.21, 0.26, 0.8],

            info: [0.15, 0.55, 0.82, 1.0],   // blue
            success: [0.52, 0.60, 0.0, 1.0], // green
            warning: [0.71, 0.54, 0.0, 1.0], // yellow
            error: [0.86, 0.20, 0.18, 1.0],  // red
            debug: [0.40, 0.48, 0.51, 1.0],  // base01

            btn_action: [0.03, 0.21, 0.26, 1.0],
            btn_action_hover: [0.06, 0.28, 0.33, 1.0],
            btn_action_active: [0.0, 0.17, 0.21, 1.0],
            btn_action_text: [0.93, 0.91, 0.84, 1.0],
        }
    }

    /// Monokai-Pro palette — warm charcoal with neon accents.
    pub fn monokai() -> Self {
        Self {
            bg: [0.16, 0.16, 0.16, 0.97],
            border: [0.26, 0.24, 0.23, 1.0],
            title: [0.98, 0.96, 0.90, 1.0],
            body: [0.64, 0.62, 0.58, 1.0],
            close: [0.50, 0.48, 0.44, 1.0],
            close_hover: [1.0, 0.98, 0.92, 1.0],
            progress_bg: [0.22, 0.20, 0.19, 0.8],

            info: [0.47, 0.78, 0.91, 1.0],    // cyan
            success: [0.67, 0.82, 0.40, 1.0], // green
            warning: [1.0, 0.76, 0.31, 1.0],  // yellow/orange
            error: [1.0, 0.40, 0.44, 1.0],    // red
            // Debug = neutral grey across every theme so the docs claim
            // ("Gray — developer-only diagnostic") holds true universally.
            // Earlier Monokai used purple here, which surprised callers
            // expecting a low-saturation tone.
            debug: [0.62, 0.60, 0.56, 1.0], // warm grey (matches FG_MUTED tone)

            btn_action: [0.26, 0.24, 0.23, 1.0],
            btn_action_hover: [0.34, 0.32, 0.30, 1.0],
            btn_action_active: [0.20, 0.18, 0.17, 1.0],
            btn_action_text: [0.98, 0.96, 0.90, 1.0],
        }
    }
}
