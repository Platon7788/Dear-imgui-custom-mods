//! Chrome configuration — schema only. Default values live in
//! [`config.ron`](../../../crate/src/chrome/config.ron).
//!
//! Schema (this file) ↔ values (ron) is the project-wide config pattern
//! (see ADR-026): the schema declares the public type surface and any
//! derivation logic; the ron file owns the canonical default values so
//! changing a default doesn't recompile every consumer.

// ── TitleAlign ────────────────────────────────────────────────────────────────

/// Title text horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TitleAlign {
    /// Left-aligned after the optional icon. Default.
    #[default]
    Left,
    /// Centred between the icon area and the button row.
    Center,
}

// ── CloseMode ─────────────────────────────────────────────────────────────────

/// How the close button (and Alt-F4) behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CloseMode {
    /// Close immediately. The host is still expected to honour the
    /// close request (e.g. `event_loop.exit()` or `process::exit(0)`)
    /// — chrome only signals intent, never terminates the process
    /// itself. Default.
    #[default]
    Immediate,
    /// Surface the close request without acting on it. The host
    /// receives `Some(CloseMode::Confirm)` from
    /// [`Chrome::take_close_request`](super::Chrome::take_close_request)
    /// and can show a confirmation dialog before exiting.
    Confirm,
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// Which standard buttons to draw in the custom titlebar, plus shared
/// per-button geometry / hover behaviour.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Buttons {
    /// Show the minimise button.
    pub minimize: bool,
    /// Show the maximise / restore button.
    pub maximize: bool,
    /// Show the close button.
    pub close: bool,
    /// Width of each button cell in logical pixels.
    pub width: f32,
    /// Icon canvas radius in logical pixels — controls the size of the
    /// vector glyph (bar / square / cross) drawn inside each button.
    pub icon_radius: f32,
    /// Glyph scale factor on hover. `1.0` disables zoom; `1.20` (default)
    /// gives a macOS-Dock-style "lift" without painting a tinted rect
    /// behind the button.
    pub hover_zoom_scale: f32,
}

impl Default for Buttons {
    fn default() -> Self {
        // Derive Buttons defaults from the inlined block inside the
        // titlebar config.ron — single source of truth for every shared
        // field (width / icon_radius / hover_zoom_scale).
        TitlebarConfig::default().buttons
    }
}

impl Buttons {
    /// Close-only set — used by tool windows and dialogs.
    pub fn close_only() -> Self {
        Self {
            minimize: false,
            maximize: false,
            close: true,
            ..Self::default()
        }
    }

    /// Set the hover-zoom scale, clamped to `1.0..=2.0`. Values outside
    /// the range silently snap to the bounds — `< 1.0` would shrink the
    /// glyph (visually wrong) and `> 2.0` makes the bilinear-scaled glyph
    /// too blurry to read.
    pub fn with_hover_zoom_scale(mut self, scale: f32) -> Self {
        self.hover_zoom_scale = scale.clamp(1.0, 2.0);
        self
    }
}

// ── TitlebarConfig ──────────────────────────────────────────────────────────

/// Pixel-level titlebar configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TitlebarConfig {
    /// Total titlebar height in logical pixels.
    pub height: f32,
    /// Show the window title text.
    pub title_visible: bool,
    /// Title text horizontal alignment.
    pub title_align: TitleAlign,
    /// Left padding before the icon / title text.
    pub title_padding_left: f32,
    /// Optional glyph drawn before the title. Useful for a brand mark
    /// (Material Design Icon, emoji, single character).
    pub icon: Option<String>,
    /// Draw a 1-px separator strip along the bottom edge of the titlebar.
    pub separator_visible: bool,
    /// Height of the separator strip (when `separator_visible`).
    pub separator_height: f32,
    /// Double-click on the empty title area toggles maximise / restore.
    pub double_click_maximize: bool,
    /// Standard button row configuration.
    pub buttons: Buttons,
    /// How the close button behaves — see [`CloseMode`].
    pub close_mode: CloseMode,
}

impl Default for TitlebarConfig {
    /// Loads `config.ron` — the canonical default. Update field defaults
    /// in the ron file, not here; the schema-vs-values split is the
    /// project-wide convention (ADR-026).
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in chrome/config.ron is valid")
    }
}

impl TitlebarConfig {
    /// Compact preset for tool / palette windows: smaller height,
    /// close-only buttons, no double-click maximise.
    pub fn tool() -> Self {
        Self {
            height: 22.0,
            double_click_maximize: false,
            buttons: Buttons::close_only(),
            ..Self::default()
        }
    }

    /// Builder: replace the title alignment.
    pub fn with_title_align(mut self, align: TitleAlign) -> Self {
        self.title_align = align;
        self
    }
    /// Builder: set the titlebar icon glyph.
    pub fn with_icon(mut self, glyph: impl Into<String>) -> Self {
        self.icon = Some(glyph.into());
        self
    }
    /// Builder: switch the close button to `Confirm` mode — the host gets
    /// a chance to cancel the close before the window exits. See
    /// [`Chrome::take_close_request`](super::Chrome::take_close_request)
    /// for the dispatch flow.
    pub fn with_close_confirm(mut self) -> Self {
        self.close_mode = CloseMode::Confirm;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_default_full_set() {
        let b = Buttons::default();
        assert!(b.minimize && b.maximize && b.close);
        assert_eq!(b.width, 44.0);
        assert_eq!(b.icon_radius, 6.0);
        assert!(b.hover_zoom_scale > 1.0 && b.hover_zoom_scale <= 1.5);
    }

    #[test]
    fn buttons_close_only() {
        let b = Buttons::close_only();
        assert!(!b.minimize && !b.maximize && b.close);
        // Geometry inherits from default — close-only is a behavioural
        // change, not a layout one.
        assert_eq!(b.width, Buttons::default().width);
    }

    #[test]
    fn with_hover_zoom_scale_clamps() {
        assert_eq!(Buttons::default().with_hover_zoom_scale(0.5).hover_zoom_scale, 1.0);
        assert_eq!(Buttons::default().with_hover_zoom_scale(5.0).hover_zoom_scale, 2.0);
        assert!(
            (Buttons::default().with_hover_zoom_scale(1.35).hover_zoom_scale - 1.35).abs() < 1e-6
        );
    }

    #[test]
    fn titlebar_default_main() {
        let tb = TitlebarConfig::default();
        assert_eq!(tb.height, 28.0);
        assert!(tb.buttons.minimize && tb.buttons.maximize && tb.buttons.close);
        assert!(tb.double_click_maximize);
    }

    #[test]
    fn titlebar_tool_compact_close_only() {
        let tb = TitlebarConfig::tool();
        assert_eq!(tb.height, 22.0);
        assert!(!tb.buttons.minimize && !tb.buttons.maximize && tb.buttons.close);
        assert!(!tb.double_click_maximize);
    }

    #[test]
    fn ron_round_trip_preserves_all_fields() {
        // Schema-vs-values guard: if a non-skip field is added to the
        // schema and ron isn't updated (or vice versa), round-trip
        // will surface a mismatch.
        let original = TitlebarConfig::default();
        let text =
            ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        let restored: TitlebarConfig = ron::from_str(&text).unwrap();
        assert_eq!(original.height, restored.height);
        assert_eq!(original.title_align, restored.title_align);
        assert_eq!(original.title_padding_left, restored.title_padding_left);
        assert_eq!(original.separator_height, restored.separator_height);
        assert_eq!(original.double_click_maximize, restored.double_click_maximize);
        assert_eq!(original.close_mode, restored.close_mode);
        assert_eq!(original.buttons.width, restored.buttons.width);
        assert_eq!(original.buttons.icon_radius, restored.buttons.icon_radius);
        assert_eq!(original.buttons.hover_zoom_scale, restored.buttons.hover_zoom_scale);
    }
}
