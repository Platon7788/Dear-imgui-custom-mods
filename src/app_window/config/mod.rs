//! Configuration for [`AppWindow`](super::AppWindow).
//!
//! Modeled after RAD Studio's `TForm` component:
//! - [`WindowKind`]   — high-level preset (Splash / Tool / Dialog / Main).
//! - [`BorderStyle`]  — borderless / fixed / sizeable / dialog / tool.
//! - [`FormStyle`]    — stacking behaviour (Normal / StayOnTop).
//! - [`Position`]     — start position on screen.
//! - [`Chrome`]       — titlebar configuration: `None` for splashes, `Custom`
//!   with full button + extras config for everything else.
//!
//! Use the four presets [`AppConfig::splash`], [`AppConfig::tool`],
//! [`AppConfig::dialog`], [`AppConfig::main`] to start, then customise
//! through the builder methods.

mod builders;
mod enums;
mod fonts;
mod icon;
mod titlebar;

pub use enums::{
    BorderStyle, CloseMode, FormStyle, FpsMode, Position, PowerMode, RenderMode, TitleAlign,
    WindowKind,
};
pub use fonts::{FontChoice, FontLayer, GlyphRanges};
pub use icon::WindowIcon;
pub use titlebar::{Buttons, Chrome, ExtraButton, TitlebarConfig};

use crate::theme::Theme;

// ── AppConfig ────────────────────────────────────────────────────────────────

/// Top-level window configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub title: String,
    pub kind: WindowKind,
    pub border: BorderStyle,
    pub form_style: FormStyle,
    pub size: [f64; 2],
    pub min_size: Option<[f64; 2]>,
    pub max_size: Option<[f64; 2]>,
    pub position: Position,
    pub chrome: Chrome,
    pub theme: Theme,
    /// Win10 fallback rounded-corner radius. Win11 DWM owns the corners.
    pub corner_radius: i32,
    /// Auto-close the window after this duration. Works for any window kind.
    pub auto_close_after: Option<std::time::Duration>,
    /// Render-loop scheduling strategy. See [`RenderMode`] for details
    /// — the default ([`RenderMode::EventDriven`] with `2 s` foreground
    /// pulse / `5 s` background pulse) is the right choice for almost
    /// every desktop app.
    pub render_mode: RenderMode,
    pub power_mode: PowerMode,
    pub font_size: f32,
    /// Font selection. `FontChoice::Bytes` / `FontChoice::Stack` carry
    /// `Arc<[u8]>` TTF buffers — runtime data, not config — so the field
    /// is skipped from ron. Hosts supply fonts via the builder API.
    #[serde(skip, default)]
    pub font: FontChoice,
    pub merge_mdi_icons: bool,
    /// Show the window on creation. Set `false` to start hidden; call `state.show()` later.
    pub visible: bool,
    /// Initial window opacity (0.0 = fully transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Optional taskbar / Alt-Tab icon. Applied at window creation.
    /// Holds raw RGBA pixel data — runtime payload, not config — so the
    /// field is skipped from ron. Hosts supply icons via the builder API.
    #[serde(skip, default)]
    pub window_icon: Option<WindowIcon>,
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
    /// Width of the invisible edge-resize hit zone in **logical pixels**.
    ///
    /// Dear ImGui works in logical pixels (already DPI-scaled by
    /// `HiDpiMode::Default`), so this value automatically adapts:
    /// `6.0` logical = 12 physical px at 200 % DPI.
    ///
    /// Increase for touch-friendly / high-DPI deployments (e.g. `10.0`).
    /// Default: `6.0`.
    pub resize_zone: f32,
}

// ── Default ──────────────────────────────────────────────────────────────────

impl Default for AppConfig {
    /// Loads `default.ron` — the schema's value-side. `font` and
    /// `window_icon` are populated from their type-side defaults
    /// (skipped from ron because they hold runtime byte buffers).
    fn default() -> Self {
        ron::from_str(include_str!("default.ron"))
            .expect("built-in app_window/config/default.ron is valid")
    }
}

// ── Presets ──────────────────────────────────────────────────────────────────

impl AppConfig {
    /// Splash screen — borderless, centred, no buttons.
    /// Whole client area is yours; great for logos, videos, loading animations.
    pub fn splash(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKind::Splash,
            border: BorderStyle::None,
            chrome: Chrome::None,
            size: [w, h],
            min_size: None,
            position: Position::ScreenCenter,
            form_style: FormStyle::StayOnTop,
            corner_radius: 12,
            ..Self::default()
        }
    }

    /// Tool / palette window — compact titlebar, close-only, smaller frame.
    pub fn tool(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKind::Tool,
            border: BorderStyle::SizeToolWin,
            chrome: Chrome::Custom(TitlebarConfig::tool()),
            size: [w, h],
            min_size: Some([200.0, 120.0]),
            position: Position::Default,
            ..Self::default()
        }
    }

    /// Dialog — fixed size, close-only, screen-centred, stays on top.
    pub fn dialog(title: impl Into<String>, w: f64, h: f64) -> Self {
        Self {
            title: title.into(),
            kind: WindowKind::Dialog,
            border: BorderStyle::Dialog,
            chrome: Chrome::Custom(TitlebarConfig::dialog()),
            size: [w, h],
            min_size: None,
            position: Position::ScreenCenter,
            form_style: FormStyle::StayOnTop,
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

// ── Internal helpers ──────────────────────────────────────────────────────────

impl AppConfig {
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
        let layer = FontLayer::base(vec![0u8; 200], 15.0);
        assert!(!layer.merge);
        assert_eq!(layer.size, 15.0);
        assert!(matches!(layer.glyph_ranges, GlyphRanges::Default));
    }

    #[test]
    fn font_layer_merge_is_merge() {
        let layer = FontLayer::merge(vec![0u8; 200], 13.0);
        assert!(layer.merge);
        assert_eq!(layer.size, 13.0);
    }

    #[test]
    fn font_layer_with_glyph_ranges_overrides_default() {
        let layer = FontLayer::base(vec![0u8; 200], 15.0).with_glyph_ranges(GlyphRanges::Cyrillic);
        assert!(matches!(layer.glyph_ranges, GlyphRanges::Cyrillic));
    }

    #[test]
    fn glyph_ranges_default_is_default_variant() {
        assert!(matches!(GlyphRanges::default(), GlyphRanges::Default));
    }

    #[test]
    fn render_mode_event_driven_default() {
        let m = RenderMode::default();
        assert!(m.is_event_driven());
        match m {
            RenderMode::EventDriven {
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
        let m = RenderMode::Continuous {
            fps_mode: FpsMode::Fixed(60),
            unfocused_fps: 30,
        };
        assert!(!m.is_event_driven());
        assert!(matches!(m.fps_mode(), FpsMode::Fixed(60)));
    }

    #[test]
    fn render_mode_event_driven_fps_mode_is_auto() {
        // Event-driven mode always returns Auto so wgpu picks vsync.
        let m = RenderMode::EventDriven {
            idle_pulse: None,
            unfocused_idle_pulse: None,
        };
        assert!(matches!(m.fps_mode(), FpsMode::Auto));
    }

    #[test]
    fn with_fps_limit_switches_to_continuous() {
        let cfg = AppConfig::default().with_fps_limit(60);
        assert!(matches!(cfg.render_mode, RenderMode::Continuous { .. }));
    }

    #[test]
    fn with_idle_pulse_keeps_event_driven() {
        let cfg = AppConfig::default().with_idle_pulse(std::time::Duration::from_millis(500));
        assert!(cfg.render_mode.is_event_driven());
    }

    #[test]
    fn event_driven_minimal_disables_pulses() {
        let cfg = AppConfig::default().event_driven_minimal();
        match cfg.render_mode {
            RenderMode::EventDriven {
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
        let cfg = AppConfig::default().continuous_render();
        match cfg.render_mode {
            RenderMode::Continuous {
                fps_mode,
                unfocused_fps,
            } => {
                assert!(matches!(fps_mode, FpsMode::Auto));
                assert_eq!(unfocused_fps, 30);
            }
            _ => panic!("continuous_render must yield Continuous"),
        }
    }

    #[test]
    fn raw_content_default_is_false() {
        let cfg = AppConfig::default();
        assert!(!cfg.raw_content);
    }

    #[test]
    fn raw_content_builder_flips_flag() {
        let cfg = AppConfig::default().raw_content();
        assert!(cfg.raw_content);
    }

    #[test]
    fn raw_content_preserves_other_fields() {
        // Builder must be additive — other config must survive.
        let cfg = AppConfig::main("My App", 800.0, 600.0)
            .with_theme(crate::theme::Theme::Light)
            .raw_content();
        assert!(cfg.raw_content);
        assert_eq!(cfg.title, "My App");
        assert_eq!(cfg.size, [800.0, 600.0]);
        assert!(matches!(cfg.theme, crate::theme::Theme::Light));
    }

    // ── config.ron round-trip & schema-vs-values guard ───────────────────
    //
    // Locks the new convention (`config.rs` = schema, `*.ron` = values)
    // for `app_window`. If a non-skip field is added to `AppConfig` /
    // `TitlebarConfig` / `Buttons` and ron is not updated, these tests
    // fail at compile/parse time.

    #[test]
    fn default_ron_parses() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.title, "Application");
        assert!(matches!(cfg.kind, WindowKind::Main));
        assert_eq!(cfg.size, [1100.0, 680.0]);
        assert_eq!(cfg.font_size, 15.0);
        assert_eq!(cfg.resize_zone, 6.0);
        // skip-fields — ron leaves them at their type-default.
        assert!(cfg.window_icon.is_none());
        assert!(matches!(cfg.font, FontChoice::Builtin(_)));
    }

    #[test]
    fn titlebar_main_matches_default() {
        let tb = TitlebarConfig::default();
        assert_eq!(tb.height, 28.0);
        assert!(tb.buttons.minimize && tb.buttons.maximize && tb.buttons.close);
        assert!(tb.double_click_maximize);
    }

    #[test]
    fn titlebar_tool_is_compact_close_only() {
        let tb = TitlebarConfig::tool();
        assert_eq!(tb.height, 22.0);
        assert!(!tb.buttons.minimize && !tb.buttons.maximize);
        assert!(tb.buttons.close);
        assert!(!tb.double_click_maximize);
    }

    #[test]
    fn titlebar_dialog_is_close_only_no_dblclick() {
        let tb = TitlebarConfig::dialog();
        assert_eq!(tb.height, 28.0);
        assert!(!tb.buttons.minimize && !tb.buttons.maximize);
        assert!(tb.buttons.close);
        assert!(!tb.double_click_maximize);
    }

    #[test]
    fn default_chrome_inline_matches_titlebar_main_ron() {
        // Drift guard: `default.ron` inlines the main-titlebar block
        // because ron 0.8 has no `include`. If `titlebar_main.ron`
        // changes, `default.ron`'s chrome must be updated too.
        let from_default = AppConfig::default();
        let from_titlebar = TitlebarConfig::default();
        let Chrome::Custom(ref tb) = from_default.chrome else {
            panic!("AppConfig::default().chrome must be Chrome::Custom");
        };
        assert_eq!(tb.height, from_titlebar.height);
        assert_eq!(tb.title_align, from_titlebar.title_align);
        assert_eq!(tb.title_padding_left, from_titlebar.title_padding_left);
        assert_eq!(tb.separator_visible, from_titlebar.separator_visible);
        assert_eq!(tb.separator_height, from_titlebar.separator_height);
        assert_eq!(tb.double_click_maximize, from_titlebar.double_click_maximize);
        assert_eq!(tb.close_mode, from_titlebar.close_mode);
        assert_eq!(tb.buttons.width, from_titlebar.buttons.width);
        assert_eq!(tb.buttons.icon_radius, from_titlebar.buttons.icon_radius);
        assert_eq!(tb.buttons.icon_hover_pad, from_titlebar.buttons.icon_hover_pad);
        assert_eq!(tb.buttons.hover_zoom_scale, from_titlebar.buttons.hover_zoom_scale);
        assert_eq!(tb.buttons.show_hover_bg, from_titlebar.buttons.show_hover_bg);
    }

    #[test]
    fn default_round_trips_through_ron_for_serde_fields() {
        // Skip-fields (`font`, `window_icon`) are excluded by serde, so a
        // ron→struct→ron→struct cycle must preserve everything else.
        let original = AppConfig::default();
        let text =
            ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        let restored: AppConfig = ron::from_str(&text).unwrap();
        assert_eq!(original.title, restored.title);
        assert_eq!(original.size, restored.size);
        assert_eq!(original.min_size, restored.min_size);
        assert_eq!(original.font_size, restored.font_size);
        assert_eq!(original.opacity, restored.opacity);
        assert_eq!(original.corner_radius, restored.corner_radius);
        assert_eq!(original.resize_zone, restored.resize_zone);
        assert_eq!(original.raw_content, restored.raw_content);
        assert!(matches!(restored.kind, WindowKind::Main));
        assert!(matches!(restored.border, BorderStyle::Sizeable));
        assert!(matches!(restored.theme, Theme::Dark));
    }

    #[test]
    fn presets_compose_over_default_correctly() {
        let splash = AppConfig::splash("S", 600.0, 400.0);
        assert!(matches!(splash.kind, WindowKind::Splash));
        assert!(matches!(splash.border, BorderStyle::None));
        assert!(matches!(splash.chrome, Chrome::None));

        let tool = AppConfig::tool("T", 320.0, 480.0);
        assert!(matches!(tool.kind, WindowKind::Tool));
        let Chrome::Custom(ref tb) = tool.chrome else {
            panic!("tool preset must use Chrome::Custom");
        };
        assert_eq!(tb.height, 22.0);

        let dialog = AppConfig::dialog("D", 400.0, 150.0);
        assert!(matches!(dialog.kind, WindowKind::Dialog));
        assert!(matches!(dialog.form_style, FormStyle::StayOnTop));
    }
}
