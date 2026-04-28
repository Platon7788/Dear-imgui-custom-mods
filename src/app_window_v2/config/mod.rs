//! Configuration for [`AppWindowV2`](super::AppWindowV2).
//!
//! Modeled after RAD Studio's `TForm` component:
//! - [`WindowKindV2`]   — high-level preset (Splash / Tool / Dialog / Main).
//! - [`BorderStyleV2`]  — borderless / fixed / sizeable / dialog / tool.
//! - [`FormStyleV2`]    — stacking behaviour (Normal / StayOnTop).
//! - [`PositionV2`]     — start position on screen.
//! - [`ChromeV2`]       — titlebar configuration: `None` for splashes, `Custom`
//!   with full button + extras config for everything else.
//!
//! Use the four presets [`AppConfigV2::splash`], [`AppConfigV2::tool`],
//! [`AppConfigV2::dialog`], [`AppConfigV2::main`] to start, then customise
//! through the builder methods.

mod enums;
mod icon;
mod titlebar;

pub use enums::{
    BorderStyleV2, CloseModeV2, FormStyleV2, FpsModeV2, PositionV2, PowerModeV2, RenderModeV2,
    TitleAlignV2, WindowKindV2,
};
pub use icon::WindowIconV2;
pub use titlebar::{ButtonsV2, ChromeV2, ExtraButtonV2, TitlebarConfigV2};

use crate::theme::Theme;

// ── AppConfigV2 ────────────────────────────────────────────────────────────────

/// Top-level window configuration.
#[derive(Debug, Clone)]
pub struct AppConfigV2 {
    pub title: String,
    pub kind: WindowKindV2,
    pub border: BorderStyleV2,
    pub form_style: FormStyleV2,
    pub size: [f64; 2],
    pub min_size: Option<[f64; 2]>,
    pub max_size: Option<[f64; 2]>,
    pub position: PositionV2,
    pub chrome: ChromeV2,
    pub theme: Theme,
    /// Win10 fallback rounded-corner radius. Win11 DWM owns the corners.
    pub corner_radius: i32,
    /// Auto-close the window after this duration. Works for any window kind.
    pub auto_close_after: Option<std::time::Duration>,
    /// Render-loop scheduling strategy. See [`RenderModeV2`] for details
    /// — the default ([`RenderModeV2::EventDriven`] with `2 s` foreground
    /// pulse / `5 s` background pulse) is the right choice for almost
    /// every desktop app.
    pub render_mode: RenderModeV2,
    pub power_mode: PowerModeV2,
    pub font_size: f32,
    pub font: FontChoiceV2,
    pub merge_mdi_icons: bool,
    /// Show the window on creation. Set `false` to start hidden; call `state.show()` later.
    pub visible: bool,
    /// Initial window opacity (0.0 = fully transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Optional taskbar / Alt-Tab icon. Applied at window creation.
    pub window_icon: Option<WindowIconV2>,
    /// **Full-bleed content mode.** When `true`, the framework runs
    /// `handler.render(ui, state)` *directly* inside the root
    /// `##app_root` window — the default `WindowPadding=[8,8]` /
    /// `ItemSpacing=[6,4]` stack and the inner `##app_content`
    /// child-window wrapper are skipped. The handler owns the entire
    /// content rect (after the titlebar) and is responsible for any
    /// padding, spacing or scroll regions.
    ///
    /// Use for chart viewers, video players, 3D viewports, full-bleed
    /// editors, or any widget that needs the whole client area pixel
    /// for pixel. Default: `false`.
    pub raw_content: bool,
}

// ── FontChoiceV2 ───────────────────────────────────────────────────────────────

/// Font selection for the ImGui context. Default: built-in Hack.
#[derive(Debug, Clone)]
pub enum FontChoiceV2 {
    /// One of the fonts shipped with `code_editor`.
    Builtin(crate::fonts::BuiltinFont),
    /// A user-supplied TTF/OTF byte buffer (e.g. via `include_bytes!`).
    Bytes(std::sync::Arc<[u8]>),
    /// Multiple fonts merged into a single atlas. The first layer is the
    /// **base font** (its `merge` flag is ignored — bases are never merged);
    /// subsequent layers are added on top, typically with `merge = true` so
    /// their glyphs (icons, CJK, math) overlay the base.
    ///
    /// Example — Inter UI + Material Design Icons:
    /// ```ignore
    /// FontChoiceV2::Stack(vec![
    ///     FontLayerV2::base(include_bytes!("Inter.ttf"), 15.0),
    ///     FontLayerV2::merge(include_bytes!("MDI.ttf"), 13.0)
    ///         .with_glyph_ranges(GlyphRangesV2::Custom(vec![[0xF0001, 0xF1FFF]])),
    /// ])
    /// ```
    Stack(Vec<FontLayerV2>),
}

impl Default for FontChoiceV2 {
    fn default() -> Self {
        Self::Builtin(crate::fonts::BuiltinFont::Hack)
    }
}

// ── FontLayerV2 ────────────────────────────────────────────────────────────────

/// One font layer inside a [`FontChoiceV2::Stack`].
#[derive(Debug, Clone)]
pub struct FontLayerV2 {
    /// Raw TTF/OTF bytes. Owned via `Arc<[u8]>` so the same buffer can
    /// appear in multiple `FontLayerV2`s without allocation churn.
    pub bytes: std::sync::Arc<[u8]>,
    /// Pixel size in logical units. The framework multiplies by HiDPI
    /// scale automatically.
    pub size: f32,
    /// `true` ⇒ glyphs from this layer overlay the previous layer's atlas.
    /// Ignored on the first layer (always treated as base).
    pub merge: bool,
    /// Codepoint subset to bake. Use [`GlyphRangesV2::Default`] for
    /// Latin-only, presets for CJK/Cyrillic/Thai/Vietnamese, or
    /// [`GlyphRangesV2::Custom`] for arbitrary ranges (e.g. icon fonts).
    pub glyph_ranges: GlyphRangesV2,
}

impl FontLayerV2 {
    /// New base layer (`merge = false`). The first layer of a stack must
    /// be a base.
    pub fn base(bytes: impl Into<std::sync::Arc<[u8]>>, size: f32) -> Self {
        Self {
            bytes: bytes.into(),
            size,
            merge: false,
            glyph_ranges: GlyphRangesV2::Default,
        }
    }
    /// New merge layer (`merge = true`). Add after a base layer.
    pub fn merge(bytes: impl Into<std::sync::Arc<[u8]>>, size: f32) -> Self {
        Self {
            bytes: bytes.into(),
            size,
            merge: true,
            glyph_ranges: GlyphRangesV2::Default,
        }
    }
    /// Override the glyph-range subset baked from this layer.
    pub fn with_glyph_ranges(mut self, ranges: GlyphRangesV2) -> Self {
        self.glyph_ranges = ranges;
        self
    }
}

// ── GlyphRangesV2 ──────────────────────────────────────────────────────────────

/// Codepoint-range subset to bake into the font atlas. Maps to Dear ImGui's
/// `ImFontGlyphRanges*` constants.
///
/// **Default** is the right pick for any UI restricted to Latin text.
/// Pick a regional preset for non-Latin UI text. Use `Custom` for icon
/// fonts (Material Design Icons, Font Awesome, Phosphor, etc.) which
/// occupy private-use Unicode planes.
#[derive(Debug, Clone, Default)]
pub enum GlyphRangesV2 {
    /// Basic Latin + Latin-1 supplement (`0x0020..=0x00FF`). Default.
    #[default]
    Default,
    /// Basic Latin + Cyrillic (Russian, Ukrainian, Belarusian, …).
    Cyrillic,
    /// Basic Latin + Hiragana + Katakana + half-width (Japanese).
    Japanese,
    /// Basic Latin + CJK common ideograms (Chinese Simplified).
    ChineseSimplified,
    /// Basic Latin + CJK common ideograms (Chinese Traditional).
    ChineseTraditional,
    /// Basic Latin + Hangul (Korean).
    Korean,
    /// Basic Latin + Thai.
    Thai,
    /// Basic Latin + Vietnamese-specific accented glyphs.
    Vietnamese,
    /// Inclusive `[start, end]` ranges. Last entry ends the list — the
    /// framework appends the required `0` terminator. Useful for icon
    /// fonts: `Custom(vec![[0xF0001, 0xF1FFF]])` for MDI.
    Custom(Vec<[u32; 2]>),
}

// ── Default ──────────────────────────────────────────────────────────────────

impl Default for AppConfigV2 {
    fn default() -> Self {
        Self {
            title: "Application".into(),
            kind: WindowKindV2::Main,
            border: BorderStyleV2::Sizeable,
            form_style: FormStyleV2::Normal,
            size: [1100.0, 680.0],
            min_size: Some([640.0, 400.0]),
            max_size: None,
            position: PositionV2::ScreenCenter,
            chrome: ChromeV2::default(),
            theme: Theme::Dark,
            corner_radius: 8,
            auto_close_after: None,
            render_mode: RenderModeV2::default(),
            power_mode: PowerModeV2::default(),
            font_size: 15.0,
            font: FontChoiceV2::default(),
            merge_mdi_icons: false,
            visible: true,
            opacity: 1.0,
            window_icon: None,
            raw_content: false,
        }
    }
}

// ── Presets ──────────────────────────────────────────────────────────────────

impl AppConfigV2 {
    /// Splash screen — borderless, centred, no buttons.
    /// Whole client area is yours; great for logos, videos, loading animations.
    pub fn splash(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKindV2::Splash,
            border: BorderStyleV2::None,
            chrome: ChromeV2::None,
            size: [w, h],
            min_size: None,
            position: PositionV2::ScreenCenter,
            form_style: FormStyleV2::StayOnTop,
            corner_radius: 12,
            ..Self::default()
        }
    }

    /// Tool / palette window — compact titlebar, close-only, smaller frame.
    pub fn tool(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKindV2::Tool,
            border: BorderStyleV2::SizeToolWin,
            chrome: ChromeV2::Custom(TitlebarConfigV2::tool()),
            size: [w, h],
            min_size: Some([200.0, 120.0]),
            position: PositionV2::Default,
            ..Self::default()
        }
    }

    /// Dialog — fixed size, close-only, screen-centred, stays on top.
    pub fn dialog(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKindV2::Dialog,
            border: BorderStyleV2::Dialog,
            chrome: ChromeV2::Custom(TitlebarConfigV2::dialog()),
            size: [w, h],
            min_size: None,
            position: PositionV2::ScreenCenter,
            form_style: FormStyleV2::StayOnTop,
            ..Self::default()
        }
    }

    /// Full main window with custom chrome.
    pub fn main(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            size: [w, h],
            ..Self::default()
        }
    }
}

// ── Builders (RAD-style fluent API) ─────────────────────────────────────────

impl AppConfigV2 {
    pub fn with_title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn with_size(mut self, w: f64, h: f64) -> Self {
        self.size = [w, h];
        self
    }
    pub fn with_min_size(mut self, w: f64, h: f64) -> Self {
        self.min_size = Some([w, h]);
        self
    }
    pub fn with_max_size(mut self, w: f64, h: f64) -> Self {
        self.max_size = Some([w, h]);
        self
    }
    pub fn with_position(mut self, p: PositionV2) -> Self {
        self.position = p;
        self
    }
    pub fn with_border(mut self, b: BorderStyleV2) -> Self {
        self.border = b;
        self
    }
    pub fn with_form_style(mut self, fs: FormStyleV2) -> Self {
        self.form_style = fs;
        self
    }
    pub fn with_theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self
    }
    pub fn with_corner_radius(mut self, r: i32) -> Self {
        self.corner_radius = r;
        self
    }
    pub fn with_font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }
    pub fn stay_on_top(mut self) -> Self {
        self.form_style = FormStyleV2::StayOnTop;
        self
    }
    pub fn with_mdi_icons(mut self) -> Self {
        self.merge_mdi_icons = true;
        self
    }
    /// Replace the entire render strategy. Most callers want one of the
    /// targeted helpers below ([`continuous_render`](Self::continuous_render),
    /// [`with_fps_limit`](Self::with_fps_limit),
    /// [`with_idle_pulse`](Self::with_idle_pulse),
    /// [`without_idle_pulse`](Self::without_idle_pulse),
    /// [`event_driven_minimal`](Self::event_driven_minimal)).
    pub fn with_render_mode(mut self, mode: RenderModeV2) -> Self {
        self.render_mode = mode;
        self
    }

    // ─── Continuous-mode shortcuts ───────────────────────────────────────

    /// Switch to [`RenderModeV2::Continuous`] with the given FPS cap.
    /// Implies foreground vsync via the `Fixed(n)` timer.
    pub fn with_fps_limit(mut self, fps: u32) -> Self {
        let unfocused_fps = match &self.render_mode {
            RenderModeV2::Continuous { unfocused_fps, .. } => *unfocused_fps,
            RenderModeV2::EventDriven { .. } => 30,
        };
        self.render_mode = RenderModeV2::Continuous {
            fps_mode: FpsModeV2::Fixed(fps),
            unfocused_fps,
        };
        self
    }

    /// Switch to [`RenderModeV2::Continuous`] with default vsync.
    /// Use this for game-style apps that always have moving content.
    pub fn continuous_render(mut self) -> Self {
        let unfocused_fps = match &self.render_mode {
            RenderModeV2::Continuous { unfocused_fps, .. } => *unfocused_fps,
            RenderModeV2::EventDriven { .. } => 30,
        };
        self.render_mode = RenderModeV2::Continuous {
            fps_mode: FpsModeV2::Auto,
            unfocused_fps,
        };
        self
    }

    /// FPS cap applied when the window is unfocused. Only meaningful in
    /// [`RenderModeV2::Continuous`] — switches to it if currently
    /// [`RenderModeV2::EventDriven`]. Use `0` to disable the throttle.
    pub fn with_unfocused_fps(mut self, fps: u32) -> Self {
        let fps_mode = match &self.render_mode {
            RenderModeV2::Continuous { fps_mode, .. } => fps_mode.clone(),
            RenderModeV2::EventDriven { .. } => FpsModeV2::Auto,
        };
        self.render_mode = RenderModeV2::Continuous {
            fps_mode,
            unfocused_fps: fps,
        };
        self
    }

    // ─── Event-driven shortcuts ──────────────────────────────────────────

    /// Foreground idle pulse for [`RenderModeV2::EventDriven`]. Switches
    /// to event-driven if currently continuous.
    pub fn with_idle_pulse(mut self, every: std::time::Duration) -> Self {
        let unfocused = match &self.render_mode {
            RenderModeV2::EventDriven {
                unfocused_idle_pulse,
                ..
            } => *unfocused_idle_pulse,
            RenderModeV2::Continuous { .. } => Some(std::time::Duration::from_secs(5)),
        };
        self.render_mode = RenderModeV2::EventDriven {
            idle_pulse: Some(every),
            unfocused_idle_pulse: unfocused,
        };
        self
    }

    /// Background idle pulse for [`RenderModeV2::EventDriven`]. Switches
    /// to event-driven if currently continuous.
    pub fn with_unfocused_idle_pulse(mut self, every: std::time::Duration) -> Self {
        let foreground = match &self.render_mode {
            RenderModeV2::EventDriven { idle_pulse, .. } => *idle_pulse,
            RenderModeV2::Continuous { .. } => Some(std::time::Duration::from_secs(2)),
        };
        self.render_mode = RenderModeV2::EventDriven {
            idle_pulse: foreground,
            unfocused_idle_pulse: Some(every),
        };
        self
    }

    /// Disable the foreground idle pulse — repaint only on input or
    /// explicit [`crate::frame_demand::request`] calls.
    pub fn without_idle_pulse(mut self) -> Self {
        let unfocused = match &self.render_mode {
            RenderModeV2::EventDriven {
                unfocused_idle_pulse,
                ..
            } => *unfocused_idle_pulse,
            RenderModeV2::Continuous { .. } => None,
        };
        self.render_mode = RenderModeV2::EventDriven {
            idle_pulse: None,
            unfocused_idle_pulse: unfocused,
        };
        self
    }

    /// Strictest event-driven setting: zero idle pulses, repaint **only**
    /// on input or explicit [`crate::frame_demand::request`] calls.
    /// CPU/GPU usage drops to absolute zero while idle.
    /// Suitable when nothing in your UI changes without input.
    pub fn event_driven_minimal(mut self) -> Self {
        self.render_mode = RenderModeV2::EventDriven {
            idle_pulse: None,
            unfocused_idle_pulse: None,
        };
        self
    }
    pub fn with_power_mode(mut self, m: PowerModeV2) -> Self {
        self.power_mode = m;
        self
    }
    pub fn with_auto_close(mut self, d: std::time::Duration) -> Self {
        self.auto_close_after = Some(d);
        self
    }

    /// Use one of the fonts shipped with `code_editor`.
    pub fn with_builtin_font(mut self, font: crate::fonts::BuiltinFont) -> Self {
        self.font = FontChoiceV2::Builtin(font);
        self
    }

    /// Use a user-supplied TTF/OTF byte buffer (e.g. `include_bytes!("Inter.ttf")`).
    pub fn with_font_bytes(mut self, bytes: impl Into<std::sync::Arc<[u8]>>) -> Self {
        self.font = FontChoiceV2::Bytes(bytes.into());
        self
    }

    /// Use a stack of fonts merged into a single ImGui atlas — typical for
    /// UI font + icon overlay (Inter + MDI), UI font + code font (UI +
    /// JetBrains Mono), or Latin + CJK (Noto Sans + Noto CJK).
    ///
    /// First layer is the **base** (always non-merged regardless of its
    /// `merge` flag); subsequent layers should set `merge = true` so their
    /// glyphs overlay the base.
    pub fn with_font_stack(mut self, layers: Vec<FontLayerV2>) -> Self {
        self.font = FontChoiceV2::Stack(layers);
        self
    }

    /// Replace chrome with a no-titlebar (splash) configuration.
    pub fn without_chrome(mut self) -> Self {
        self.chrome = ChromeV2::None;
        self
    }

    /// Replace chrome with the given titlebar config.
    pub fn with_chrome(mut self, t: TitlebarConfigV2) -> Self {
        self.chrome = ChromeV2::Custom(t);
        self
    }

    /// Add an extra titlebar button (only meaningful when chrome is `Custom`).
    pub fn with_extra_button(mut self, b: ExtraButtonV2) -> Self {
        debug_assert!(
            matches!(self.chrome, ChromeV2::Custom(_)),
            "with_extra_button requires ChromeV2::Custom (current chrome is None)"
        );
        if let ChromeV2::Custom(ref mut tb) = self.chrome {
            tb.extras.push(b);
        }
        self
    }

    /// Set the titlebar icon glyph.
    pub fn with_icon(mut self, glyph: impl Into<String>) -> Self {
        debug_assert!(
            matches!(self.chrome, ChromeV2::Custom(_)),
            "with_icon requires ChromeV2::Custom (current chrome is None)"
        );
        if let ChromeV2::Custom(ref mut tb) = self.chrome {
            tb.icon = Some(glyph.into());
        }
        self
    }

    /// Hide the minimize button.
    pub fn without_minimize(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, ChromeV2::Custom(_)),
            "without_minimize requires ChromeV2::Custom (current chrome is None)"
        );
        if let ChromeV2::Custom(ref mut tb) = self.chrome {
            tb.buttons.minimize = false;
        }
        self
    }

    /// Hide the maximize button.
    pub fn without_maximize(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, ChromeV2::Custom(_)),
            "without_maximize requires ChromeV2::Custom (current chrome is None)"
        );
        if let ChromeV2::Custom(ref mut tb) = self.chrome {
            tb.buttons.maximize = false;
        }
        self
    }

    /// Switch close button to `Confirm` mode (fire callback first).
    pub fn with_close_confirm(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, ChromeV2::Custom(_)),
            "with_close_confirm requires ChromeV2::Custom (current chrome is None)"
        );
        if let ChromeV2::Custom(ref mut tb) = self.chrome {
            tb.close_mode = CloseModeV2::Confirm;
        }
        self
    }

    /// Start hidden — the window will not appear until `state.show()` is called.
    pub fn start_hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Initial opacity (0.0 = fully transparent, 1.0 = opaque).
    pub fn with_opacity(mut self, alpha: f32) -> Self {
        self.opacity = alpha.clamp(0.0, 1.0);
        self
    }

    /// Set the taskbar / Alt-Tab icon directly from a [`WindowIconV2`].
    pub fn with_window_icon(mut self, icon: WindowIconV2) -> Self {
        self.window_icon = Some(icon);
        self
    }

    /// Set the taskbar / Alt-Tab icon from raw RGBA pixels. Errors are
    /// logged and the icon is silently dropped.
    pub fn with_window_icon_rgba(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        match WindowIconV2::from_rgba(rgba, width, height) {
            Ok(icon) => self.window_icon = Some(icon),
            Err(err) => eprintln!("app_window_v2: with_window_icon_rgba: {err}"),
        }
        self
    }

    /// Enable **full-bleed content mode** — `handler.render(ui, state)`
    /// runs directly inside the root window without the framework's
    /// default child-window wrapper, padding, or item-spacing. The
    /// handler owns the entire content rect (after the titlebar).
    ///
    /// Use for chart viewers, video players, 3D viewports, full-bleed
    /// code editors, or anything that needs pixel-perfect control over
    /// the client area. The titlebar (when configured) still renders
    /// above the content; only the content wrapper is skipped.
    pub fn raw_content(mut self) -> Self {
        self.raw_content = true;
        self
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

impl AppConfigV2 {
    /// Whether the OS should drive resize on this window.
    pub(super) fn os_resizable(&self) -> bool {
        self.border.is_resizable()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_layer_base_is_not_merge() {
        let layer = FontLayerV2::base(vec![0u8; 200], 15.0);
        assert!(!layer.merge);
        assert_eq!(layer.size, 15.0);
        assert!(matches!(layer.glyph_ranges, GlyphRangesV2::Default));
    }

    #[test]
    fn font_layer_merge_is_merge() {
        let layer = FontLayerV2::merge(vec![0u8; 200], 13.0);
        assert!(layer.merge);
        assert_eq!(layer.size, 13.0);
    }

    #[test]
    fn font_layer_with_glyph_ranges_overrides_default() {
        let layer =
            FontLayerV2::base(vec![0u8; 200], 15.0).with_glyph_ranges(GlyphRangesV2::Cyrillic);
        assert!(matches!(layer.glyph_ranges, GlyphRangesV2::Cyrillic));
    }

    #[test]
    fn glyph_ranges_default_is_default_variant() {
        assert!(matches!(GlyphRangesV2::default(), GlyphRangesV2::Default));
    }

    #[test]
    fn render_mode_event_driven_default() {
        let m = RenderModeV2::default();
        assert!(m.is_event_driven());
        match m {
            RenderModeV2::EventDriven {
                idle_pulse,
                unfocused_idle_pulse,
            } => {
                assert_eq!(idle_pulse, Some(std::time::Duration::from_secs(2)));
                assert_eq!(
                    unfocused_idle_pulse,
                    Some(std::time::Duration::from_secs(5))
                );
            }
            _ => panic!("expected EventDriven default"),
        }
    }

    #[test]
    fn render_mode_continuous_reports_fps_mode() {
        let m = RenderModeV2::Continuous {
            fps_mode: FpsModeV2::Fixed(60),
            unfocused_fps: 30,
        };
        assert!(!m.is_event_driven());
        assert!(matches!(m.fps_mode(), FpsModeV2::Fixed(60)));
    }

    #[test]
    fn render_mode_event_driven_fps_mode_is_auto() {
        // Event-driven mode always returns Auto so wgpu picks vsync.
        let m = RenderModeV2::EventDriven {
            idle_pulse: None,
            unfocused_idle_pulse: None,
        };
        assert!(matches!(m.fps_mode(), FpsModeV2::Auto));
    }

    #[test]
    fn with_fps_limit_switches_to_continuous() {
        let cfg = AppConfigV2::default().with_fps_limit(60);
        assert!(matches!(cfg.render_mode, RenderModeV2::Continuous { .. }));
    }

    #[test]
    fn with_idle_pulse_keeps_event_driven() {
        let cfg = AppConfigV2::default().with_idle_pulse(std::time::Duration::from_millis(500));
        assert!(cfg.render_mode.is_event_driven());
    }

    #[test]
    fn event_driven_minimal_disables_pulses() {
        let cfg = AppConfigV2::default().event_driven_minimal();
        match cfg.render_mode {
            RenderModeV2::EventDriven {
                idle_pulse,
                unfocused_idle_pulse,
            } => {
                assert_eq!(idle_pulse, None);
                assert_eq!(unfocused_idle_pulse, None);
            }
            _ => panic!("event_driven_minimal must yield EventDriven"),
        }
    }

    #[test]
    fn continuous_render_default_unfocused_30() {
        let cfg = AppConfigV2::default().continuous_render();
        match cfg.render_mode {
            RenderModeV2::Continuous {
                fps_mode,
                unfocused_fps,
            } => {
                assert!(matches!(fps_mode, FpsModeV2::Auto));
                assert_eq!(unfocused_fps, 30);
            }
            _ => panic!("continuous_render must yield Continuous"),
        }
    }

    #[test]
    fn raw_content_default_is_false() {
        let cfg = AppConfigV2::default();
        assert!(!cfg.raw_content);
    }

    #[test]
    fn raw_content_builder_flips_flag() {
        let cfg = AppConfigV2::default().raw_content();
        assert!(cfg.raw_content);
    }

    #[test]
    fn raw_content_preserves_other_fields() {
        // Builder must be additive — other config must survive.
        let cfg = AppConfigV2::main("My App", 800.0, 600.0)
            .with_theme(crate::theme::Theme::Light)
            .raw_content();
        assert!(cfg.raw_content);
        assert_eq!(cfg.title, "My App");
        assert_eq!(cfg.size, [800.0, 600.0]);
        assert!(matches!(cfg.theme, crate::theme::Theme::Light));
    }
}
