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
/// [`crate::app_window`]'s chrome layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
}

// ── DialogColors ────────────────────────────────────────────────────────────

/// Complete colour set for the confirm dialog. Consumed by
/// [`crate::confirm_dialog`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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
    /// NxT-Dark default — pinned to `Theme::Dark.statusbar_colors()` so
    /// callers that construct a `StatusBarConfig::default()` without
    /// going through the theme system still get the **same** surface
    /// shade as `Theme::Dark.nav().bg` and the rest of the chrome
    /// stack. (Pre-2026-04-29 this default was stale-hardcoded and
    /// rendered noticeably darker than the matching nav/titlebar
    /// surfaces — see `theme::tests::default_matches_dark_theme`.)
    fn default() -> Self {
        Self {
            // Dark theme tokens, expressed as `[u8 / 255]`:
            //   bg        = STATUSBAR_BG (0x2B2F38)
            //   text      = FG           (0xE0E4EA)
            //   text_dim  = FG_MUTED     (0x8A92A1)
            //   separator = BORDER       (0x3F4654)
            //   hover     = SECONDARY_HOVER (0x48505E)
            //   active    = SECONDARY_HOVER
            //   success   = SUCCESS      (0x5FB870)
            //   warning   = WARNING      (0xD9A643)
            //   error     = DANGER       (0xE06060)
            //   info      = ACCENT       (0x5B9BD5)
            bg: [
                0x2B as f32 / 255.0,
                0x2F as f32 / 255.0,
                0x38 as f32 / 255.0,
                1.0,
            ],
            text: [
                0xE0 as f32 / 255.0,
                0xE4 as f32 / 255.0,
                0xEA as f32 / 255.0,
                1.0,
            ],
            text_dim: [
                0x8A as f32 / 255.0,
                0x92 as f32 / 255.0,
                0xA1 as f32 / 255.0,
                1.0,
            ],
            separator: [
                0x3F as f32 / 255.0,
                0x46 as f32 / 255.0,
                0x54 as f32 / 255.0,
                1.0,
            ],
            hover: [
                0x48 as f32 / 255.0,
                0x50 as f32 / 255.0,
                0x5E as f32 / 255.0,
                1.0,
            ],
            active: [
                0x48 as f32 / 255.0,
                0x50 as f32 / 255.0,
                0x5E as f32 / 255.0,
                1.0,
            ],

            success: [
                0x5F as f32 / 255.0,
                0xB8 as f32 / 255.0,
                0x70 as f32 / 255.0,
                1.0,
            ],
            warning: [
                0xD9 as f32 / 255.0,
                0xA6 as f32 / 255.0,
                0x43 as f32 / 255.0,
                1.0,
            ],
            error: [
                0xE0 as f32 / 255.0,
                0x60 as f32 / 255.0,
                0x60 as f32 / 255.0,
                1.0,
            ],
            info: [
                0x5B as f32 / 255.0,
                0x9B as f32 / 255.0,
                0xD5 as f32 / 255.0,
                1.0,
            ],
        }
    }
}

// ── HexViewerColors / DisasmViewColors ──────────────────────────────────────
//
// These two palettes are the largest in the crate (18 and 26 colour tokens
// plus their `from_tokens` factories), so each lives in its own sibling file
// to keep every palette source under the size limit. They are re-exported
// here so the standard `crate::theme::{HexViewerColors, DisasmViewColors,
// DisasmFlowKind}` import paths stay unchanged.
mod disasm;
mod hex;

pub use disasm::{DisasmFlowKind, DisasmViewColors, DisasmViewTokens};
pub use hex::{HexViewerColors, HexViewerTokens};

// ── NotificationColors ──────────────────────────────────────────────────────

/// Complete colour set for the notification center. Consumed by
/// [`crate::notifications`].
///
/// Ships with two named-preset constructors (`dark()`, `light()`). The
/// built-in [`crate::theme::Theme`] variants delegate to these
/// constructors so adding a new theme that wants the default
/// notification look is one method call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
}
