//! Confirm-dialog color palette.
//!
//! The theme selector is the unified [`crate::theme::Theme`] at top level —
//! retrieve the matching palette with [`Theme::dialog()`](crate::theme::Theme::dialog).
//! For custom palettes, build a [`DialogColors`] directly and pass it via
//! [`DialogConfig::with_colors`](super::config::DialogConfig::with_colors).

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
