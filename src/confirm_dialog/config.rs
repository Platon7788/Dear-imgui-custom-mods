//! Configuration for the confirm dialog.

use super::theme::DialogTheme;

/// Icon type displayed in the dialog header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogIcon {
    /// No icon.
    None,
    /// Yellow/orange warning triangle with "!".
    #[default]
    Warning,
    /// Red circle with "×".
    Error,
    /// Blue circle with "i".
    Info,
    /// Purple circle with "?".
    Question,
}

/// Confirm button visual style preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfirmStyle {
    /// Red / destructive (for close, delete, discard).
    #[default]
    Destructive,
    /// Uses the theme's default accent — not red.
    Normal,
}

/// Full configuration for the confirm dialog.
///
/// # Example
/// ```rust,no_run
/// # use dear_imgui_custom_mod::confirm_dialog::*;
/// let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
///     .with_icon(DialogIcon::Warning)
///     .with_confirm_label("Close")
///     .with_cancel_label("Cancel")
///     .with_theme(DialogTheme::Dark);
/// ```
#[derive(Debug, Clone)]
pub struct DialogConfig {
    /// Dialog title / header text.
    pub title: String,
    /// Body message shown below the title.
    pub message: String,
    /// Confirm button label. Default: `"Confirm"`.
    pub confirm_label: String,
    /// Cancel button label. Default: `"Cancel"`.
    pub cancel_label: String,
    /// Icon displayed in the header area. Default: `Warning`.
    pub icon: DialogIcon,
    /// Confirm button visual style. Default: `Destructive`.
    pub confirm_style: ConfirmStyle,
    /// Color theme. Default: `Dark`.
    pub theme: DialogTheme,
    /// Dialog width (px). Default: `340.0`.
    pub width: f32,
    /// Dialog height (px). Default: `160.0`.
    pub height: f32,
    /// Window padding inside the dialog (px). Default: `16.0`.
    pub padding: f32,
    /// Button height (px). Default: `30.0`.
    pub button_height: f32,
    /// Gap between buttons (px). Default: `12.0`.
    pub button_gap: f32,
    /// Draw a dim overlay behind the dialog. Default: `true`.
    pub dim_background: bool,
    /// Handle Escape (cancel) and Enter (confirm) keys. Default: `true`.
    pub keyboard_shortcuts: bool,
    /// Border rounding radius (px). Default: `6.0`.
    pub rounding: f32,
    /// Border thickness (px). Default: `1.5`.
    pub border_thickness: f32,
    /// Use the icon color as the dialog border color (orange for Warning, red for
    /// Error, etc.). When `false`, the theme's neutral border color is used.
    /// Default: `true`.
    pub accent_border: bool,
    /// Draw a horizontal separator line between the message and the buttons.
    /// Default: `false` (cleaner modern look).
    pub show_separator: bool,
    /// Draw small icons inside the cancel and confirm buttons (X for cancel,
    /// power glyph for destructive confirm, check for normal confirm).
    /// Default: `true`.
    pub show_button_icons: bool,
}

impl Default for DialogConfig {
    fn default() -> Self {
        Self {
            title: String::from("Confirm"),
            message: String::from("Are you sure?"),
            confirm_label: String::from("Confirm"),
            cancel_label: String::from("Cancel"),
            icon: DialogIcon::Warning,
            confirm_style: ConfirmStyle::Destructive,
            theme: DialogTheme::Dark,
            width: 340.0,
            height: 160.0,
            padding: 16.0,
            button_height: 30.0,
            button_gap: 20.0,
            dim_background: true,
            keyboard_shortcuts: true,
            rounding: 6.0,
            border_thickness: 1.5,
            accent_border: true,
            show_separator: false,
            show_button_icons: true,
        }
    }
}

impl DialogConfig {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            ..Self::default()
        }
    }

    pub fn with_theme(mut self, t: DialogTheme) -> Self { self.theme = t; self }
    pub fn with_icon(mut self, icon: DialogIcon) -> Self { self.icon = icon; self }
    pub fn with_confirm_label(mut self, l: impl Into<String>) -> Self { self.confirm_label = l.into(); self }
    pub fn with_cancel_label(mut self, l: impl Into<String>) -> Self { self.cancel_label = l.into(); self }
    pub fn with_confirm_style(mut self, s: ConfirmStyle) -> Self { self.confirm_style = s; self }
    pub fn with_width(mut self, w: f32) -> Self { self.width = w; self }
    pub fn with_height(mut self, h: f32) -> Self { self.height = h; self }
    pub fn with_padding(mut self, p: f32) -> Self { self.padding = p; self }
    pub fn with_button_height(mut self, h: f32) -> Self { self.button_height = h; self }
    pub fn with_rounding(mut self, r: f32) -> Self { self.rounding = r; self }
    pub fn with_border_thickness(mut self, t: f32) -> Self { self.border_thickness = t; self }
    pub fn with_accent_border(mut self, on: bool) -> Self { self.accent_border = on; self }
    pub fn with_separator(mut self, on: bool) -> Self { self.show_separator = on; self }
    pub fn with_button_icons(mut self, on: bool) -> Self { self.show_button_icons = on; self }
    pub fn without_dim(mut self) -> Self { self.dim_background = false; self }
    pub fn without_keyboard(mut self) -> Self { self.keyboard_shortcuts = false; self }
}
