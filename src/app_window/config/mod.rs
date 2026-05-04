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
#[derive(Debug, Clone)]
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
    pub font: FontChoice,
    pub merge_mdi_icons: bool,
    /// Show the window on creation. Set `false` to start hidden; call `state.show()` later.
    pub visible: bool,
    /// Initial window opacity (0.0 = fully transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Optional taskbar / Alt-Tab icon. Applied at window creation.
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
}

// ── Default ──────────────────────────────────────────────────────────────────

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Application".into(),
            kind: WindowKind::Main,
            border: BorderStyle::Sizeable,
            form_style: FormStyle::Normal,
            size: [1100.0, 680.0],
            min_size: Some([640.0, 400.0]),
            max_size: None,
            position: Position::ScreenCenter,
            chrome: Chrome::default(),
            theme: Theme::Dark,
            corner_radius: 8,
            auto_close_after: None,
            render_mode: RenderMode::default(),
            power_mode: PowerMode::default(),
            font_size: 15.0,
            font: FontChoice::default(),
            merge_mdi_icons: false,
            visible: true,
            opacity: 1.0,
            window_icon: None,
            raw_content: false,
        }
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
}
