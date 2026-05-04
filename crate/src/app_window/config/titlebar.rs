//! Titlebar configuration: chrome, button bar, extra buttons.

use super::enums::{CloseMode, TitleAlign};

// ── Extra titlebar button ─────────────────────────────────────────────────────

/// A custom button rendered to the left of the standard buttons.
#[derive(Debug, Clone)]
pub struct ExtraButton {
    pub id: &'static str,
    pub label: &'static str,
    pub tooltip: Option<&'static str>,
    pub color: [f32; 4],
}

impl ExtraButton {
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

// ── Buttons ────────────────────────────────────────────────────────────────

/// Which standard buttons to draw in the custom titlebar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Buttons {
    pub minimize: bool,
    pub maximize: bool,
    pub close: bool,
    /// Width of each button cell (px). Default: `44.0`.
    pub width: f32,
    /// Icon canvas radius (px). Default: `6.0`.
    pub icon_radius: f32,
    /// Hover highlight padding (px). Default: `4.0`.
    pub icon_hover_pad: f32,
    /// Glyph scale factor on hover. `1.0` disables, `1.20` = 20 %
    /// larger (default). Same macOS-Dock-style micro-magnification
    /// the nav panel uses — gives the buttons a "lift" without
    /// painting a tinted rectangle behind them. Use
    /// [`Self::with_hover_zoom_scale`] to set a clamped value
    /// (`1.0..=2.0`); writing this field directly bypasses the clamp.
    pub hover_zoom_scale: f32,
    /// Whether to paint the rounded coloured rectangle behind a
    /// hovered button. Default: `false` — the project owner
    /// removed it because it competed with the zoom-on-hover signal
    /// (a red fill behind a magnified close icon read as "two
    /// hovers"). Flip back to `true` if you want the historic
    /// "Vex0r-style" red close hover.
    pub show_hover_bg: bool,
}

impl Default for Buttons {
    /// Loads `buttons.ron` — full chrome (minimize + maximize + close).
    /// `Buttons::close_only()` is the alternative constructor and stays
    /// in Rust because it's a value-derivation, not a free default.
    fn default() -> Self {
        ron::from_str(include_str!("buttons.ron"))
            .expect("built-in app_window/config/buttons.ron is valid")
    }
}

impl Buttons {
    /// Close-only set (used by tool windows and dialogs).
    pub fn close_only() -> Self {
        Self {
            minimize: false,
            maximize: false,
            close: true,
            ..Self::default()
        }
    }

    /// Set the hover-zoom scale with clamping to `1.0..=2.0`.
    /// `1.0` disables the zoom; values above `2.0` make the
    /// glyphs noticeably blurry (the default font atlas isn't
    /// re-rasterised, so larger sizes go through GPU bilinear).
    /// Out-of-range inputs silently snap to the bounds.
    pub fn with_hover_zoom_scale(mut self, scale: f32) -> Self {
        self.hover_zoom_scale = scale.clamp(1.0, 2.0);
        self
    }

    /// Toggle the historic coloured hover rectangle behind a hovered
    /// button. Default is `false` (Vex0r-style red-fill removed
    /// 2026-04-29 because it competed with the new hover-zoom
    /// signal). Flip back to `true` for the legacy look.
    pub fn with_hover_bg(mut self, show: bool) -> Self {
        self.show_hover_bg = show;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_default_has_macos_dock_zoom() {
        let b = Buttons::default();
        assert!(b.hover_zoom_scale > 1.0 && b.hover_zoom_scale <= 1.5);
        assert!(!b.show_hover_bg, "hover bg defaults off post-2026-04-29");
        assert!(b.minimize && b.maximize && b.close);
    }

    #[test]
    fn close_only_preset_keeps_zoom_default() {
        let b = Buttons::close_only();
        assert!(!b.minimize && !b.maximize && b.close);
        assert!(b.hover_zoom_scale > 1.0);
    }

    #[test]
    fn with_hover_zoom_scale_clamps() {
        // Floor at 1.0 — anything below would shrink the icon on
        // hover (visually wrong).
        let b = Buttons::default().with_hover_zoom_scale(0.5);
        assert!((b.hover_zoom_scale - 1.0).abs() < 1e-6);
        // Ceiling at 2.0 — beyond that the bilinear-scaled glyph
        // gets too blurry to read.
        let b = Buttons::default().with_hover_zoom_scale(5.0);
        assert!((b.hover_zoom_scale - 2.0).abs() < 1e-6);
        // In-range values pass through.
        let b = Buttons::default().with_hover_zoom_scale(1.35);
        assert!((b.hover_zoom_scale - 1.35).abs() < 1e-6);
    }

    #[test]
    fn with_hover_bg_toggle() {
        let b = Buttons::default().with_hover_bg(true);
        assert!(b.show_hover_bg);
        let b = b.with_hover_bg(false);
        assert!(!b.show_hover_bg);
    }

    // ── Drift guards: `buttons.ron` is the single source of truth for
    // every shared field of `Buttons` (width, icon_radius, icon_hover_pad,
    // hover_zoom_scale, show_hover_bg). The three titlebar presets each
    // inline a copy because ron 0.8 has no `include` mechanism — these
    // tests fail loudly if anyone updates one file without updating the
    // others. `minimize` / `maximize` are deliberately preset-specific
    // (full chrome / close-only) and not compared.

    fn assert_buttons_inline_matches_canonical(actual: &Buttons) {
        let canonical = Buttons::default();
        assert_eq!(actual.width, canonical.width);
        assert_eq!(actual.icon_radius, canonical.icon_radius);
        assert_eq!(actual.icon_hover_pad, canonical.icon_hover_pad);
        assert_eq!(actual.hover_zoom_scale, canonical.hover_zoom_scale);
        assert_eq!(actual.show_hover_bg, canonical.show_hover_bg);
    }

    #[test]
    fn buttons_inline_in_titlebar_main_matches_canonical() {
        // `titlebar_main.ron` uses the full set; `minimize/maximize` must
        // both be `true` for that preset.
        let tb = TitlebarConfig::default();
        assert!(tb.buttons.minimize && tb.buttons.maximize && tb.buttons.close);
        assert_buttons_inline_matches_canonical(&tb.buttons);
    }

    #[test]
    fn buttons_inline_in_titlebar_tool_matches_canonical() {
        let tb = TitlebarConfig::tool();
        assert!(!tb.buttons.minimize && !tb.buttons.maximize && tb.buttons.close);
        assert_buttons_inline_matches_canonical(&tb.buttons);
    }

    #[test]
    fn buttons_inline_in_titlebar_dialog_matches_canonical() {
        let tb = TitlebarConfig::dialog();
        assert!(!tb.buttons.minimize && !tb.buttons.maximize && tb.buttons.close);
        assert_buttons_inline_matches_canonical(&tb.buttons);
    }
}

// ── TitlebarConfig ──────────────────────────────────────────────────────────

/// Pixel-level titlebar configuration. Used inside [`Chrome::Custom`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TitlebarConfig {
    pub height: f32,
    pub title_visible: bool,
    pub title_align: TitleAlign,
    pub title_padding_left: f32,
    pub icon: Option<String>,
    pub separator_visible: bool,
    pub separator_height: f32,
    pub double_click_maximize: bool,
    pub buttons: Buttons,
    /// Extra buttons rendered next to the standard ones. `ExtraButton`
    /// holds `&'static str` fields (id/label/tooltip) for zero-allocation
    /// dispatch, so it cannot be deserialised — extras are populated by
    /// the host via the builder API instead. Skipped from ron.
    #[serde(skip, default)]
    pub extras: Vec<ExtraButton>,
    pub close_mode: CloseMode,
}

impl Default for TitlebarConfig {
    /// Loads `titlebar_main.ron` — the main-window chrome preset.
    fn default() -> Self {
        ron::from_str(include_str!("titlebar_main.ron"))
            .expect("built-in app_window/config/titlebar_main.ron is valid")
    }
}

impl TitlebarConfig {
    /// Compact preset for tool windows (smaller height, close-only).
    /// Loaded from `titlebar_tool.ron`.
    pub fn tool() -> Self {
        ron::from_str(include_str!("titlebar_tool.ron"))
            .expect("built-in app_window/config/titlebar_tool.ron is valid")
    }
    /// Dialog preset (fixed-size feel — no maximize).
    /// Loaded from `titlebar_dialog.ron`.
    pub fn dialog() -> Self {
        ron::from_str(include_str!("titlebar_dialog.ron"))
            .expect("built-in app_window/config/titlebar_dialog.ron is valid")
    }
}

// ── Chrome ──────────────────────────────────────────────────────────────────

/// Titlebar mode selector. `None` for borderless splash; `Custom` for everything else.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Chrome {
    /// No titlebar at all. The whole client area is yours.
    None,
    /// Custom titlebar drawn by the framework.
    Custom(TitlebarConfig),
}

impl Default for Chrome {
    fn default() -> Self {
        Self::Custom(TitlebarConfig::default())
    }
}
