//! Fluent builder methods for [`AppConfig`].

use std::sync::Arc;
use std::time::Duration;
use super::{AppConfig, Chrome, CloseMode, ExtraButton, FontChoice, FontLayer, FormStyle, FpsMode, PowerMode, RenderMode, TitlebarConfig, WindowIcon};

// ── Builders (RAD-style fluent API) ─────────────────────────────────────────

impl AppConfig {
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
    pub fn with_position(mut self, p: super::Position) -> Self {
        self.position = p;
        self
    }
    pub fn with_border(mut self, b: super::BorderStyle) -> Self {
        self.border = b;
        self
    }
    pub fn with_form_style(mut self, fs: FormStyle) -> Self {
        self.form_style = fs;
        self
    }
    pub fn with_theme(mut self, t: crate::theme::Theme) -> Self {
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
        self.form_style = FormStyle::StayOnTop;
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
    pub fn with_render_mode(mut self, mode: RenderMode) -> Self {
        self.render_mode = mode;
        self
    }

    // ─── Continuous-mode shortcuts ───────────────────────────────────────

    /// Switch to [`RenderMode::Continuous`] with the given FPS cap.
    /// Implies foreground vsync via the `Fixed(n)` timer.
    pub fn with_fps_limit(mut self, fps: u32) -> Self {
        let unfocused_fps = match &self.render_mode {
            RenderMode::Continuous { unfocused_fps, .. } => *unfocused_fps,
            RenderMode::EventDriven { .. } => 30,
        };
        self.render_mode = RenderMode::Continuous {
            fps_mode: FpsMode::Fixed(fps),
            unfocused_fps,
        };
        self
    }

    /// Switch to [`RenderMode::Continuous`] with default vsync.
    /// Use this for game-style apps that always have moving content.
    pub fn continuous_render(mut self) -> Self {
        let unfocused_fps = match &self.render_mode {
            RenderMode::Continuous { unfocused_fps, .. } => *unfocused_fps,
            RenderMode::EventDriven { .. } => 30,
        };
        self.render_mode = RenderMode::Continuous {
            fps_mode: FpsMode::Auto,
            unfocused_fps,
        };
        self
    }

    /// FPS cap applied when the window is unfocused. Only meaningful in
    /// [`RenderMode::Continuous`] — switches to it if currently
    /// [`RenderMode::EventDriven`]. Use `0` to disable the throttle.
    pub fn with_unfocused_fps(mut self, fps: u32) -> Self {
        let fps_mode = match &self.render_mode {
            RenderMode::Continuous { fps_mode, .. } => fps_mode.clone(),
            RenderMode::EventDriven { .. } => FpsMode::Auto,
        };
        self.render_mode = RenderMode::Continuous {
            fps_mode,
            unfocused_fps: fps,
        };
        self
    }

    // ─── Event-driven shortcuts ──────────────────────────────────────────

    /// Foreground idle pulse for [`RenderMode::EventDriven`]. Switches
    /// to event-driven if currently continuous.
    pub fn with_idle_pulse(mut self, every: Duration) -> Self {
        let unfocused = match &self.render_mode {
            RenderMode::EventDriven {
                unfocused_idle_pulse,
                ..
            } => *unfocused_idle_pulse,
            RenderMode::Continuous { .. } => Some(Duration::from_secs(5)),
        };
        self.render_mode = RenderMode::EventDriven {
            idle_pulse: Some(every),
            unfocused_idle_pulse: unfocused,
        };
        self
    }

    /// Background idle pulse for [`RenderMode::EventDriven`]. Switches
    /// to event-driven if currently continuous.
    pub fn with_unfocused_idle_pulse(mut self, every: Duration) -> Self {
        let foreground = match &self.render_mode {
            RenderMode::EventDriven { idle_pulse, .. } => *idle_pulse,
            RenderMode::Continuous { .. } => Some(Duration::from_secs(2)),
        };
        self.render_mode = RenderMode::EventDriven {
            idle_pulse: foreground,
            unfocused_idle_pulse: Some(every),
        };
        self
    }

    /// Disable the foreground idle pulse — repaint only on input or
    /// explicit [`crate::frame_demand::request`] calls.
    pub fn without_idle_pulse(mut self) -> Self {
        let unfocused = match &self.render_mode {
            RenderMode::EventDriven {
                unfocused_idle_pulse,
                ..
            } => *unfocused_idle_pulse,
            RenderMode::Continuous { .. } => None,
        };
        self.render_mode = RenderMode::EventDriven {
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
        self.render_mode = RenderMode::EventDriven {
            idle_pulse: None,
            unfocused_idle_pulse: None,
        };
        self
    }
    pub fn with_power_mode(mut self, m: PowerMode) -> Self {
        self.power_mode = m;
        self
    }
    pub fn with_auto_close(mut self, d: Duration) -> Self {
        self.auto_close_after = Some(d);
        self
    }

    /// Use one of the fonts shipped with `code_editor`.
    pub fn with_builtin_font(mut self, font: crate::fonts::BuiltinFont) -> Self {
        self.font = FontChoice::Builtin(font);
        self
    }

    /// Use a user-supplied TTF/OTF byte buffer (e.g. `include_bytes!("Inter.ttf")`).
    pub fn with_font_bytes(mut self, bytes: impl Into<Arc<[u8]>>) -> Self {
        self.font = FontChoice::Bytes(bytes.into());
        self
    }

    /// Use a stack of fonts merged into a single ImGui atlas — typical for
    /// UI font + icon overlay (Inter + MDI), UI font + code font (UI +
    /// JetBrains Mono), or Latin + CJK (Noto Sans + Noto CJK).
    ///
    /// First layer is the **base** (always non-merged regardless of its
    /// `merge` flag); subsequent layers should set `merge = true` so their
    /// glyphs overlay the base.
    pub fn with_font_stack(mut self, layers: Vec<FontLayer>) -> Self {
        self.font = FontChoice::Stack(layers);
        self
    }

    /// Replace chrome with a no-titlebar (splash) configuration.
    pub fn without_chrome(mut self) -> Self {
        self.chrome = Chrome::None;
        self
    }

    /// Replace chrome with the given titlebar config.
    pub fn with_chrome(mut self, t: TitlebarConfig) -> Self {
        self.chrome = Chrome::Custom(t);
        self
    }

    /// Add an extra titlebar button (only meaningful when chrome is `Custom`).
    pub fn with_extra_button(mut self, b: ExtraButton) -> Self {
        debug_assert!(
            matches!(self.chrome, Chrome::Custom(_)),
            "with_extra_button requires Chrome::Custom (current chrome is None)"
        );
        if let Chrome::Custom(ref mut tb) = self.chrome {
            tb.extras.push(b);
        }
        self
    }

    /// Set the titlebar icon glyph.
    pub fn with_icon(mut self, glyph: impl Into<String>) -> Self {
        debug_assert!(
            matches!(self.chrome, Chrome::Custom(_)),
            "with_icon requires Chrome::Custom (current chrome is None)"
        );
        if let Chrome::Custom(ref mut tb) = self.chrome {
            tb.icon = Some(glyph.into());
        }
        self
    }

    /// Hide the minimize button.
    pub fn without_minimize(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, Chrome::Custom(_)),
            "without_minimize requires Chrome::Custom (current chrome is None)"
        );
        if let Chrome::Custom(ref mut tb) = self.chrome {
            tb.buttons.minimize = false;
        }
        self
    }

    /// Hide the maximize button.
    pub fn without_maximize(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, Chrome::Custom(_)),
            "without_maximize requires Chrome::Custom (current chrome is None)"
        );
        if let Chrome::Custom(ref mut tb) = self.chrome {
            tb.buttons.maximize = false;
        }
        self
    }

    /// Switch close button to `Confirm` mode (fire callback first).
    pub fn with_close_confirm(mut self) -> Self {
        debug_assert!(
            matches!(self.chrome, Chrome::Custom(_)),
            "with_close_confirm requires Chrome::Custom (current chrome is None)"
        );
        if let Chrome::Custom(ref mut tb) = self.chrome {
            tb.close_mode = CloseMode::Confirm;
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

    /// Set the taskbar / Alt-Tab icon directly from a [`WindowIcon`].
    pub fn with_window_icon(mut self, icon: WindowIcon) -> Self {
        self.window_icon = Some(icon);
        self
    }

    /// Set the taskbar / Alt-Tab icon from raw RGBA pixels. Errors are
    /// logged and the icon is silently dropped.
    pub fn with_window_icon_rgba(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        match WindowIcon::from_rgba(rgba, width, height) {
            Ok(icon) => self.window_icon = Some(icon),
            Err(err) => eprintln!("app_window: with_window_icon_rgba: {err}"),
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
    ///
    /// **Z-order side-effect:** in `raw_content` mode the framework
    /// also adds `WindowFlags::NO_BACKGROUND` to the root window so
    /// the background draw list is visible through it. That is what
    /// lets [`crate::status_bar::StatusBar::render_overlay`] and
    /// [`crate::nav_panel::render_nav_panel_overlay`] (both of
    /// which paint into the background draw list) actually appear
    /// on screen. The visible page surface stays opaque thanks to
    /// the GPU clear pass which fills the swap chain with
    /// `Theme::window_bg()` before any ImGui rendering.
    pub fn raw_content(mut self) -> Self {
        self.raw_content = true;
        self
    }

    /// Set the edge-resize hit zone width in logical pixels (default `6.0`).
    ///
    /// Increase for touch-friendly or high-DPI deployments where a finger
    /// needs a wider grab area. The value is already in DPI-independent
    /// logical pixels, so it scales automatically with monitor scale factor.
    pub fn with_resize_zone(mut self, px: f32) -> Self {
        self.resize_zone = px.max(1.0);
        self
    }
}
