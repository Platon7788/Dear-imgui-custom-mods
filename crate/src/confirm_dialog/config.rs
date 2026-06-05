//! Configuration for the confirm dialog.

use super::theme::DialogColors;
use crate::theme::Theme;

/// Icon type displayed in the dialog header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
/// # use dear_imgui_custom_mod::theme::Theme;
/// let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
///     .with_icon(DialogIcon::Warning)
///     .with_confirm_label("Close")
///     .with_cancel_label("Cancel")
///     .with_theme(Theme::Dark);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DialogConfig {
    /// Dialog title / header text. An **empty** string means "no host
    /// override" — the render path falls back to the localized default
    /// for [`locale`](Self::locale) (see [`resolved_title`](Self::resolved_title)).
    /// [`new`](Self::new) populates this, so a host-built config always
    /// carries an explicit title.
    pub title: String,
    /// Body message shown below the title. Empty ⇒ use the localized
    /// default (see [`resolved_message`](Self::resolved_message)).
    pub message: String,
    /// Confirm button label. Empty ⇒ use the localized default for
    /// [`locale`](Self::locale) (see [`resolved_confirm_label`](Self::resolved_confirm_label)).
    /// Set explicitly via [`with_confirm_label`](Self::with_confirm_label) to override.
    pub confirm_label: String,
    /// Cancel button label. Empty ⇒ use the localized default for
    /// [`locale`](Self::locale) (see [`resolved_cancel_label`](Self::resolved_cancel_label)).
    /// Set explicitly via [`with_cancel_label`](Self::with_cancel_label) to override.
    pub cancel_label: String,
    /// Icon displayed in the header area. Default: `Warning`.
    pub icon: DialogIcon,
    /// Confirm button visual style. Default: `Destructive`.
    pub confirm_style: ConfirmStyle,
    /// Color theme. Default: `Dark`.
    pub theme: Theme,
    /// Optional custom palette that bypasses [`theme`](Self::theme).
    #[serde(skip, default)]
    pub colors_override: Option<DialogColors>,
    /// Dialog width (px). Default: `340.0`.
    pub width: f32,
    /// Dialog height (px). Default: `160.0`.
    pub height: f32,
    /// Window padding inside the dialog (px). Default: `16.0`.
    pub padding: f32,
    /// Button height (px). Default: `27.0`.
    pub button_height: f32,
    /// Fixed button width (px). `0.0` auto-sizes each button to its content
    /// (label + icon + [`button_padding_x`](Self::button_padding_x)); any
    /// value `> 0.0` forces both buttons to exactly that width.
    /// Default: `75.0`.
    #[serde(default)]
    pub button_width: f32,
    /// Gap between buttons (px). Default: `60.0`.
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
    /// Header icon canvas radius / size (px). Default: `16.0`.
    pub header_icon_size: f32,
    /// Horizontal padding inside each cancel / confirm button (px each side).
    /// Adds to the label + glyph width to compute the cell size.
    /// Default: `22.0`.
    pub button_padding_x: f32,
    /// Distance from the bottom edge of the dialog to the bottom of the
    /// button cells, expressed as a fraction of [`padding`](Self::padding).
    /// Default: `0.35`.
    pub button_bottom_factor: f32,
    /// In-button icon radius as a fraction of the button height (the ✕ /
    /// power / check glyph). Smaller values draw more compact glyphs.
    /// Default: `0.16`.
    #[serde(default = "default_button_icon_scale")]
    pub button_icon_scale: f32,

    /// User-visible language for the dialog's **default** title,
    /// message and button labels. Default [`crate::i18n::Locale::En`].
    ///
    /// The localized strings only apply where the host left the
    /// corresponding field empty — an explicit
    /// [`title`](Self::title) / [`message`](Self::message) /
    /// [`confirm_label`](Self::confirm_label) /
    /// [`cancel_label`](Self::cancel_label) always wins (see the
    /// `resolved_*` accessors). Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` into the active font atlas — without
    /// that, Cyrillic characters render as `?` placeholders.
    ///
    /// `#[serde(default)]` so older `config.ron` files (which predate
    /// this field) still parse, defaulting to English.
    #[serde(default)]
    pub locale: crate::i18n::Locale,
}

/// Serde fallback for [`DialogConfig::button_icon_scale`] when an older saved
/// ron predates the field — keeps glyphs visible instead of collapsing to a
/// zero-radius (invisible) icon.
fn default_button_icon_scale() -> f32 {
    0.16
}

impl Default for DialogConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in confirm_dialog/config.ron is valid")
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

    pub fn with_theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self.colors_override = None;
        self
    }
    /// Use a custom [`DialogColors`] palette instead of the built-in theme.
    pub fn with_colors(mut self, c: DialogColors) -> Self {
        self.colors_override = Some(c);
        self
    }
    /// Resolved palette for rendering.
    pub(crate) fn resolved_colors(&self) -> DialogColors {
        match &self.colors_override {
            Some(c) => c.clone(),
            None => self.theme.dialog(),
        }
    }

    /// The localized string catalogue for the active [`locale`](Self::locale).
    #[inline]
    fn strings(&self) -> &'static crate::i18n::confirm_dialog::Strings {
        crate::i18n::confirm_dialog::strings(self.locale)
    }

    /// Title to render: the host-set [`title`](Self::title) when
    /// non-empty, otherwise the localized default. Host text always
    /// wins over the localized default.
    #[must_use]
    pub fn resolved_title(&self) -> &str {
        if self.title.is_empty() {
            self.strings().title
        } else {
            &self.title
        }
    }

    /// Message to render: host-set [`message`](Self::message) when
    /// non-empty, otherwise the localized default.
    #[must_use]
    pub fn resolved_message(&self) -> &str {
        if self.message.is_empty() {
            self.strings().message
        } else {
            &self.message
        }
    }

    /// Confirm-button label to render: host-set
    /// [`confirm_label`](Self::confirm_label) when non-empty, otherwise
    /// the localized default.
    #[must_use]
    pub fn resolved_confirm_label(&self) -> &str {
        if self.confirm_label.is_empty() {
            self.strings().confirm
        } else {
            &self.confirm_label
        }
    }

    /// Cancel-button label to render: host-set
    /// [`cancel_label`](Self::cancel_label) when non-empty, otherwise
    /// the localized default.
    #[must_use]
    pub fn resolved_cancel_label(&self) -> &str {
        if self.cancel_label.is_empty() {
            self.strings().cancel
        } else {
            &self.cancel_label
        }
    }
    pub fn with_icon(mut self, icon: DialogIcon) -> Self {
        self.icon = icon;
        self
    }
    pub fn with_confirm_label(mut self, l: impl Into<String>) -> Self {
        self.confirm_label = l.into();
        self
    }
    pub fn with_cancel_label(mut self, l: impl Into<String>) -> Self {
        self.cancel_label = l.into();
        self
    }

    /// Set the user-visible language for the dialog's **default** title,
    /// message and button labels.
    ///
    /// This only affects fields the host left empty — an explicit
    /// [`title`](Self::title) / [`with_confirm_label`](Self::with_confirm_label)
    /// / [`with_cancel_label`](Self::with_cancel_label) value is never
    /// overwritten. Switching to [`crate::i18n::Locale::Ru`] requires
    /// the host to bake `GlyphRanges::Cyrillic` into the active font
    /// atlas, otherwise Cyrillic renders as `?`.
    ///
    /// ```rust,no_run
    /// # use dear_imgui_custom_mod::confirm_dialog::DialogConfig;
    /// # use dear_imgui_custom_mod::i18n::Locale;
    /// // Empty title/message ⇒ the Russian defaults are used.
    /// let cfg = DialogConfig::default().with_locale(Locale::Ru);
    /// assert_eq!(cfg.resolved_confirm_label(), "Подтвердить");
    /// ```
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.locale = locale;
        self
    }

    /// Mid-flight language switch — mirror of [`with_locale`](Self::with_locale)
    /// taking `&mut self`.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.locale = locale;
    }

    /// Currently-active locale.
    #[must_use]
    pub fn locale(&self) -> crate::i18n::Locale {
        self.locale
    }
    pub fn with_confirm_style(mut self, s: ConfirmStyle) -> Self {
        self.confirm_style = s;
        self
    }
    pub fn with_width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn with_height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    pub fn with_padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }
    pub fn with_button_height(mut self, h: f32) -> Self {
        self.button_height = h;
        self
    }
    pub fn with_button_gap(mut self, g: f32) -> Self {
        self.button_gap = g;
        self
    }
    /// Fixed button width (px). `0.0` restores content auto-sizing.
    pub fn with_button_width(mut self, w: f32) -> Self {
        self.button_width = w;
        self
    }
    /// In-button icon radius as a fraction of button height. Smaller = more
    /// compact glyph.
    pub fn with_button_icon_scale(mut self, s: f32) -> Self {
        self.button_icon_scale = s;
        self
    }
    /// Horizontal padding inside each button cell (px each side).
    pub fn with_button_padding_x(mut self, p: f32) -> Self {
        self.button_padding_x = p;
        self
    }
    /// Header icon size (px). `0.0` is allowed if `icon == DialogIcon::None`
    /// is also true, but in practice changing only this is rarely useful.
    pub fn with_header_icon_size(mut self, s: f32) -> Self {
        self.header_icon_size = s;
        self
    }
    /// Bottom-margin factor for the button row, expressed as a fraction
    /// of `padding` (default `0.35`).
    pub fn with_button_bottom_factor(mut self, f: f32) -> Self {
        self.button_bottom_factor = f;
        self
    }
    pub fn with_rounding(mut self, r: f32) -> Self {
        self.rounding = r;
        self
    }
    pub fn with_border_thickness(mut self, t: f32) -> Self {
        self.border_thickness = t;
        self
    }
    pub fn with_accent_border(mut self, on: bool) -> Self {
        self.accent_border = on;
        self
    }
    pub fn with_separator(mut self, on: bool) -> Self {
        self.show_separator = on;
        self
    }
    pub fn with_button_icons(mut self, on: bool) -> Self {
        self.show_button_icons = on;
        self
    }
    pub fn without_dim(mut self) -> Self {
        self.dim_background = false;
        self
    }
    pub fn without_keyboard(mut self) -> Self {
        self.keyboard_shortcuts = false;
        self
    }
}
