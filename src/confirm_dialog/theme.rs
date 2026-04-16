//! Color themes for the confirm dialog.

use crate::borderless_window::TitlebarTheme;

/// Complete color set for the confirm dialog.
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

/// Theme selector for the dialog.
#[derive(Debug, Clone, Default)]
pub enum DialogTheme {
    #[default]
    Dark,
    Light,
    Midnight,
    Nord,
    Solarized,
    Monokai,
    Custom(Box<DialogColors>),
}

impl DialogTheme {
    pub fn colors(&self) -> DialogColors {
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

    pub fn dark() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.55],
            bg:                 [0.14, 0.15, 0.19, 1.0],
            border:             [0.28, 0.30, 0.38, 0.80],
            title:              [0.92, 0.90, 0.96, 1.0],
            message:            [0.72, 0.70, 0.78, 1.0],
            separator:          [0.28, 0.30, 0.38, 0.60],

            icon_warning:       [0.95, 0.55, 0.13, 1.0],
            icon_error:         [0.94, 0.33, 0.31, 1.0],
            icon_info:          [0.31, 0.76, 0.97, 1.0],
            icon_question:      [0.70, 0.62, 0.86, 1.0],

            btn_confirm:        [0.70, 0.22, 0.22, 1.0],
            btn_confirm_hover:  [0.82, 0.30, 0.30, 1.0],
            btn_confirm_active: [0.60, 0.18, 0.18, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.18, 0.52, 0.35, 1.0],
            btn_cancel_hover:   [0.22, 0.62, 0.42, 1.0],
            btn_cancel_active:  [0.14, 0.44, 0.28, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn light() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.35],
            bg:                 [0.98, 0.98, 0.99, 1.0],
            border:             [0.75, 0.76, 0.80, 0.80],
            title:              [0.12, 0.12, 0.18, 1.0],
            message:            [0.30, 0.30, 0.38, 1.0],
            separator:          [0.78, 0.79, 0.84, 0.60],

            icon_warning:       [0.85, 0.60, 0.00, 1.0],
            icon_error:         [0.82, 0.16, 0.16, 1.0],
            icon_info:          [0.08, 0.46, 0.78, 1.0],
            icon_question:      [0.45, 0.35, 0.70, 1.0],

            btn_confirm:        [0.82, 0.16, 0.16, 1.0],
            btn_confirm_hover:  [0.90, 0.24, 0.24, 1.0],
            btn_confirm_active: [0.70, 0.12, 0.12, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.22, 0.62, 0.38, 1.0],
            btn_cancel_hover:   [0.28, 0.72, 0.46, 1.0],
            btn_cancel_active:  [0.18, 0.52, 0.32, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn midnight() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.65],
            bg:                 [0.09, 0.09, 0.12, 1.0],
            border:             [0.20, 0.22, 0.28, 0.80],
            title:              [0.90, 0.90, 0.92, 1.0],
            message:            [0.65, 0.65, 0.70, 1.0],
            separator:          [0.20, 0.22, 0.28, 0.60],

            icon_warning:       [0.96, 0.52, 0.10, 1.0],
            icon_error:         [1.00, 0.35, 0.35, 1.0],
            icon_info:          [0.28, 0.69, 1.00, 1.0],
            icon_question:      [0.70, 0.62, 0.86, 1.0],

            btn_confirm:        [0.75, 0.18, 0.18, 1.0],
            btn_confirm_hover:  [0.88, 0.26, 0.26, 1.0],
            btn_confirm_active: [0.62, 0.14, 0.14, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.15, 0.48, 0.30, 1.0],
            btn_cancel_hover:   [0.20, 0.58, 0.38, 1.0],
            btn_cancel_active:  [0.12, 0.40, 0.24, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn nord() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.50],
            bg:                 [0.20, 0.22, 0.28, 1.0],
            border:             [0.26, 0.30, 0.37, 0.80],
            title:              [0.88, 0.90, 0.94, 1.0],
            message:            [0.70, 0.73, 0.80, 1.0],
            separator:          [0.26, 0.30, 0.37, 0.60],

            icon_warning:       [0.92, 0.80, 0.55, 1.0],
            icon_error:         [0.75, 0.38, 0.42, 1.0],
            icon_info:          [0.53, 0.75, 0.82, 1.0],
            icon_question:      [0.70, 0.62, 0.86, 1.0],

            btn_confirm:        [0.75, 0.38, 0.42, 1.0],
            btn_confirm_hover:  [0.85, 0.45, 0.50, 1.0],
            btn_confirm_active: [0.62, 0.30, 0.34, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.36, 0.56, 0.52, 1.0],
            btn_cancel_hover:   [0.42, 0.66, 0.60, 1.0],
            btn_cancel_active:  [0.30, 0.48, 0.44, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn solarized() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.50],
            bg:                 [0.04, 0.24, 0.30, 1.0],
            border:             [0.35, 0.43, 0.46, 0.80],
            title:              [0.93, 0.91, 0.84, 1.0],
            message:            [0.51, 0.58, 0.59, 1.0],
            separator:          [0.35, 0.43, 0.46, 0.60],

            icon_warning:       [0.71, 0.54, 0.00, 1.0],
            icon_error:         [0.86, 0.20, 0.18, 1.0],
            icon_info:          [0.15, 0.55, 0.82, 1.0],
            icon_question:      [0.42, 0.44, 0.77, 1.0],

            btn_confirm:        [0.86, 0.20, 0.18, 1.0],
            btn_confirm_hover:  [0.94, 0.28, 0.26, 1.0],
            btn_confirm_active: [0.72, 0.14, 0.12, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.20, 0.52, 0.40, 1.0],
            btn_cancel_hover:   [0.26, 0.62, 0.48, 1.0],
            btn_cancel_active:  [0.16, 0.44, 0.34, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn monokai() -> DialogColors {
        DialogColors {
            overlay:            [0.0, 0.0, 0.0, 0.60],
            bg:                 [0.18, 0.18, 0.18, 1.0],
            border:             [0.28, 0.28, 0.28, 0.80],
            title:              [0.95, 0.94, 0.93, 1.0],
            message:            [0.72, 0.72, 0.70, 1.0],
            separator:          [0.28, 0.28, 0.28, 0.60],

            icon_warning:       [0.90, 0.86, 0.45, 1.0],
            icon_error:         [0.98, 0.15, 0.45, 1.0],
            icon_info:          [0.40, 0.85, 0.94, 1.0],
            icon_question:      [0.68, 0.51, 0.90, 1.0],

            btn_confirm:        [0.80, 0.12, 0.36, 1.0],
            btn_confirm_hover:  [0.90, 0.20, 0.44, 1.0],
            btn_confirm_active: [0.68, 0.08, 0.28, 1.0],
            btn_confirm_text:   [1.0, 1.0, 1.0, 1.0],

            btn_cancel:         [0.40, 0.65, 0.12, 1.0],
            btn_cancel_hover:   [0.48, 0.75, 0.18, 1.0],
            btn_cancel_active:  [0.34, 0.56, 0.08, 1.0],
            btn_cancel_text:    [1.0, 1.0, 1.0, 1.0],
        }
    }
}

// ─── Conversion from a borderless-window titlebar theme ──────────────────────

#[inline]
fn shift(c: [f32; 4], delta: f32) -> [f32; 4] {
    [
        (c[0] + delta).clamp(0.0, 1.0),
        (c[1] + delta).clamp(0.0, 1.0),
        (c[2] + delta).clamp(0.0, 1.0),
        c[3],
    ]
}

/// Derive matching [`DialogColors`] from a [`TitlebarTheme`].
///
/// Keeps the confirm dialog visually coherent with the titlebar:
/// - `bg` / `title` / `message` / `separator` / `border` mirror the titlebar palette.
/// - Destructive (confirm) button uses the titlebar close-button color.
/// - Cancel button uses a derived green accent. Icon palette stays semantic
///   (warning orange / error red / info blue / question purple) so the icon
///   meaning is independent of the chrome color.
impl From<&TitlebarTheme> for DialogColors {
    fn from(theme: &TitlebarTheme) -> Self {
        let tb = theme.colors();
        let confirm = tb.btn_close;
        DialogColors {
            overlay: [0.0, 0.0, 0.0, 0.55],
            bg: tb.bg,
            border: tb.separator,
            title: tb.title,
            message: tb.title_inactive,
            separator: tb.separator,
            // Semantic icon palette — stable across themes.
            icon_warning: [0.95, 0.55, 0.13, 1.0],
            icon_error: [0.94, 0.33, 0.31, 1.0],
            icon_info: tb.btn_maximize,
            icon_question: [0.70, 0.62, 0.86, 1.0],
            btn_confirm: confirm,
            btn_confirm_hover: shift(confirm, 0.08),
            btn_confirm_active: shift(confirm, -0.08),
            btn_confirm_text: [1.0, 1.0, 1.0, 1.0],
            btn_cancel: [0.22, 0.56, 0.34, 1.0],
            btn_cancel_hover: [0.28, 0.64, 0.40, 1.0],
            btn_cancel_active: [0.16, 0.46, 0.28, 1.0],
            btn_cancel_text: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Convenience: `DialogTheme::from(&TitlebarTheme::Nord)` → `DialogTheme::Custom(Box<DialogColors>)`.
impl From<&TitlebarTheme> for DialogTheme {
    fn from(theme: &TitlebarTheme) -> Self {
        DialogTheme::Custom(Box::new(DialogColors::from(theme)))
    }
}
