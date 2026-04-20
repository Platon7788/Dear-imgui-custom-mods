//! Notification-center color palette.
//!
//! The full palette is picked via [`crate::theme::Theme::notifications`] —
//! for custom looks, build a [`NotificationColors`] directly and pass it via
//! [`CenterConfig::with_colors`](super::config::CenterConfig::with_colors).

/// Complete color set for the notification center.
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
            bg:            [0.18, 0.20, 0.24, 0.96],
            border:        [0.28, 0.31, 0.37, 1.0],
            title:         [0.88, 0.90, 0.92, 1.0],
            body:          [0.68, 0.71, 0.77, 1.0],
            close:         [0.54, 0.57, 0.63, 1.0],
            close_hover:   [0.95, 0.95, 0.95, 1.0],
            progress_bg:   [0.25, 0.27, 0.32, 0.8],

            info:          [0.36, 0.61, 0.84, 1.0],
            success:       [0.37, 0.72, 0.44, 1.0],
            warning:       [0.85, 0.65, 0.25, 1.0],
            error:         [0.88, 0.37, 0.37, 1.0],
            debug:         [0.55, 0.58, 0.64, 1.0],

            btn_action:        [0.28, 0.31, 0.38, 1.0],
            btn_action_hover:  [0.35, 0.40, 0.48, 1.0],
            btn_action_active: [0.22, 0.25, 0.31, 1.0],
            btn_action_text:   [0.92, 0.94, 0.96, 1.0],
        }
    }

    /// Light palette — matches `Theme::Light`.
    pub fn light() -> Self {
        Self {
            bg:            [0.98, 0.98, 0.99, 0.98],
            border:        [0.78, 0.80, 0.84, 1.0],
            title:         [0.12, 0.14, 0.18, 1.0],
            body:          [0.36, 0.39, 0.44, 1.0],
            close:         [0.50, 0.54, 0.60, 1.0],
            close_hover:   [0.10, 0.12, 0.16, 1.0],
            progress_bg:   [0.88, 0.89, 0.92, 0.8],

            info:          [0.18, 0.48, 0.76, 1.0],
            success:       [0.18, 0.60, 0.32, 1.0],
            warning:       [0.82, 0.55, 0.16, 1.0],
            error:         [0.80, 0.22, 0.22, 1.0],
            debug:         [0.46, 0.49, 0.55, 1.0],

            btn_action:        [0.86, 0.88, 0.92, 1.0],
            btn_action_hover:  [0.78, 0.82, 0.88, 1.0],
            btn_action_active: [0.70, 0.74, 0.82, 1.0],
            btn_action_text:   [0.14, 0.16, 0.20, 1.0],
        }
    }

    /// Midnight palette — Tokyo Night accent, OLED-friendly.
    pub fn midnight() -> Self {
        Self {
            bg:            [0.06, 0.07, 0.10, 0.97],
            border:        [0.18, 0.20, 0.28, 1.0],
            title:         [0.86, 0.88, 0.94, 1.0],
            body:          [0.58, 0.62, 0.72, 1.0],
            close:         [0.46, 0.49, 0.58, 1.0],
            close_hover:   [0.92, 0.94, 0.98, 1.0],
            progress_bg:   [0.12, 0.14, 0.20, 0.8],

            info:          [0.50, 0.72, 0.96, 1.0],
            success:       [0.58, 0.82, 0.62, 1.0],
            warning:       [0.95, 0.78, 0.42, 1.0],
            error:         [0.94, 0.46, 0.52, 1.0],
            debug:         [0.48, 0.52, 0.62, 1.0],

            btn_action:        [0.14, 0.17, 0.24, 1.0],
            btn_action_hover:  [0.20, 0.24, 0.32, 1.0],
            btn_action_active: [0.10, 0.12, 0.18, 1.0],
            btn_action_text:   [0.88, 0.90, 0.96, 1.0],
        }
    }

    /// Solarized-dark palette.
    pub fn solarized() -> Self {
        Self {
            bg:            [0.0,  0.17, 0.21, 0.97],
            border:        [0.03, 0.21, 0.26, 1.0],
            title:         [0.93, 0.91, 0.84, 1.0],
            body:          [0.51, 0.58, 0.59, 1.0],
            close:         [0.40, 0.48, 0.51, 1.0],
            close_hover:   [0.93, 0.91, 0.84, 1.0],
            progress_bg:   [0.03, 0.21, 0.26, 0.8],

            info:          [0.15, 0.55, 0.82, 1.0],   // blue
            success:       [0.52, 0.60, 0.0,  1.0],   // green
            warning:       [0.71, 0.54, 0.0,  1.0],   // yellow
            error:         [0.86, 0.20, 0.18, 1.0],   // red
            debug:         [0.40, 0.48, 0.51, 1.0],   // base01

            btn_action:        [0.03, 0.21, 0.26, 1.0],
            btn_action_hover:  [0.06, 0.28, 0.33, 1.0],
            btn_action_active: [0.0,  0.17, 0.21, 1.0],
            btn_action_text:   [0.93, 0.91, 0.84, 1.0],
        }
    }

    /// Monokai-Pro palette — warm charcoal with neon accents.
    pub fn monokai() -> Self {
        Self {
            bg:            [0.16, 0.16, 0.16, 0.97],
            border:        [0.26, 0.24, 0.23, 1.0],
            title:         [0.98, 0.96, 0.90, 1.0],
            body:          [0.64, 0.62, 0.58, 1.0],
            close:         [0.50, 0.48, 0.44, 1.0],
            close_hover:   [1.0,  0.98, 0.92, 1.0],
            progress_bg:   [0.22, 0.20, 0.19, 0.8],

            info:          [0.47, 0.78, 0.91, 1.0],   // cyan
            success:       [0.67, 0.82, 0.40, 1.0],   // green
            warning:       [1.0,  0.76, 0.31, 1.0],   // yellow/orange
            error:         [1.0,  0.40, 0.44, 1.0],   // red
            debug:         [0.68, 0.50, 0.80, 1.0],   // purple

            btn_action:        [0.26, 0.24, 0.23, 1.0],
            btn_action_hover:  [0.34, 0.32, 0.30, 1.0],
            btn_action_active: [0.20, 0.18, 0.17, 1.0],
            btn_action_text:   [0.98, 0.96, 0.90, 1.0],
        }
    }
}
