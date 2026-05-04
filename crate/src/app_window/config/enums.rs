//! Small standalone enums used by [`AppConfig`](super::AppConfig).

// ── WindowKind ────────────────────────────────────────────────────────────────

/// High-level window preset. Picking one sets sensible defaults for everything
/// else; you can still override any of them with the builder methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WindowKind {
    /// Borderless splash — no titlebar, no buttons, no resize.
    /// Whole client area is yours for a logo, video, or loading animation.
    Splash,
    /// Tool / palette window — compact titlebar, close-only, smaller frame.
    Tool,
    /// Dialog — fixed size, close-only, centred over parent or screen.
    Dialog,
    /// Main application window — full custom chrome, all buttons, resizable.
    #[default]
    Main,
}

// ── BorderStyle ───────────────────────────────────────────────────────────────

/// Border behaviour. Equivalent to RAD Studio `BorderStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum BorderStyle {
    /// No border. Splash. Use [`WindowKind::Splash`].
    None,
    /// Single thin border, fixed size.
    Single,
    /// Resizable border. Default for [`WindowKind::Main`].
    #[default]
    Sizeable,
    /// Dialog frame, fixed size.
    Dialog,
    /// Tool-window frame, fixed size.
    ToolWindow,
    /// Tool-window frame, resizable.
    SizeToolWin,
}

impl BorderStyle {
    /// Whether the OS should let the user resize the window by dragging edges.
    pub fn is_resizable(self) -> bool {
        matches!(self, Self::Sizeable | Self::SizeToolWin)
    }
    /// Whether this style uses the compact tool-window titlebar height.
    pub fn is_tool(self) -> bool {
        matches!(self, Self::ToolWindow | Self::SizeToolWin)
    }
}

// ── FormStyle ─────────────────────────────────────────────────────────────────

/// Window stacking behaviour. Equivalent to RAD Studio `FormStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FormStyle {
    /// Normal window.
    #[default]
    Normal,
    /// Always on top of other windows (`WS_EX_TOPMOST`).
    StayOnTop,
}

// ── Position ──────────────────────────────────────────────────────────────────

/// Where the window opens. Equivalent to RAD Studio `Position`.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Position {
    /// OS picks. Default.
    #[default]
    Default,
    /// Centre of the primary monitor.
    ScreenCenter,
    /// Top-left of the primary monitor.
    TopLeft,
    /// Specific physical-pixel coordinates.
    Custom(i32, i32),
}

// ── CloseMode ─────────────────────────────────────────────────────────────────

/// How the close button (and Alt-F4) behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CloseMode {
    /// Close immediately. Default.
    #[default]
    Immediate,
    /// Fire `AppHandler::on_close_requested` first; the close completes only
    /// when `AppState::confirm_close` is called from there or from your
    /// confirmation UI.
    Confirm,
}

// ── FpsMode ───────────────────────────────────────────────────────────────────

/// Frame rate control. Only consulted in
/// [`RenderMode::Continuous`](RenderMode) — event-driven mode always
/// uses adaptive vsync (`AutoVsync`).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum FpsMode {
    /// Adaptive vsync — wgpu picks FifoRelaxed (smooth on slow frames) or Fifo. Default.
    #[default]
    Auto,
    /// Cap to N frames per second (vsync, WaitUntil timer).
    Fixed(u32),
    /// No vsync cap — wgpu picks Immediate or Mailbox (AutoNoVsync). High CPU/GPU load.
    Unlimited,
}

// ── RenderMode ────────────────────────────────────────────────────────────────

/// Top-level render strategy. Picks one of two scheduling models —
/// **event-driven** (the default; idle CPU/GPU ≈ 0%) or **continuous**
/// (game-style — every loop iteration repaints).
///
/// Event-driven is the right choice for desktop tools, editors, dialogs,
/// chat apps, dashboards — anything that mostly waits on user input.
/// Continuous is for games, simulations, live previews, anything where
/// content changes without input.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RenderMode {
    /// Default. Repaint only on input events, animation requests
    /// ([`crate::frame_demand`]), or the optional periodic *idle pulses*
    /// for time-based widgets (clocks, uptime counters, status metrics).
    EventDriven {
        /// Foreground refresh cadence. `None` ⇒ pure event-driven (paint
        /// only on input or [`crate::frame_demand::request`]).
        /// Recommended: `Some(2 s)` for typical apps with a clock.
        idle_pulse: Option<std::time::Duration>,
        /// Background refresh cadence — applied while the window is not
        /// the foreground window. Should be ≥ `idle_pulse`. `None` ⇒
        /// no pulse at all when unfocused (wakes on input only).
        /// Recommended: `Some(5 s)`.
        unfocused_idle_pulse: Option<std::time::Duration>,
    },
    /// Continuous render — every iteration of the event loop calls
    /// `request_redraw`, gated by vsync (`fps_mode = Auto`) or an explicit
    /// frame cap (`fps_mode = Fixed(n)`). Use for game-style apps.
    Continuous {
        /// Foreground frame mode (vsync / fixed cap / unlimited).
        fps_mode: FpsMode,
        /// Background FPS cap when unfocused. `0` ⇒ no extra throttle.
        unfocused_fps: u32,
    },
}

impl Default for RenderMode {
    fn default() -> Self {
        Self::EventDriven {
            idle_pulse: Some(std::time::Duration::from_secs(2)),
            unfocused_idle_pulse: Some(std::time::Duration::from_secs(5)),
        }
    }
}

impl RenderMode {
    /// `true` iff this is the [`EventDriven`](Self::EventDriven) variant.
    pub fn is_event_driven(&self) -> bool {
        matches!(self, Self::EventDriven { .. })
    }
    /// FPS mode used for the wgpu surface. Event-driven mode always picks
    /// [`FpsMode::Auto`] so vsync gates the frame timing.
    pub fn fps_mode(&self) -> FpsMode {
        match self {
            Self::Continuous { fps_mode, .. } => *fps_mode,
            Self::EventDriven { .. } => FpsMode::Auto,
        }
    }
}

// ── PowerMode ─────────────────────────────────────────────────────────────────

/// GPU adapter selection preference.
///
/// The default ([`HighPerformance`](Self::HighPerformance)) is the
/// [`IMGUI_NXT`](https://github.com/Platon7788/IMGUI_NXT)-proven strategy:
/// ask the OS GPU manager for the highest-performance adapter, then fall
/// back to the software renderer if the primary path fails. This works
/// on integrated-only laptops (returns the iGPU because it is the only
/// adapter), on hybrid Optimus / switchable-graphics laptops (the OS
/// routes correctly through the display-attached GPU), and on desktops
/// (returns the dedicated card). The fallback to a WARP / llvmpipe
/// software adapter ensures the application *always* launches, even on
/// machines without a working hardware-accelerated path.
///
/// The previous `Auto` variant was a duplicate alias and has been
/// removed — there was only ever one strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PowerMode {
    /// Ask the OS GPU manager for the highest-performance adapter.
    /// Default — works on every tested setup (integrated-only, hybrid,
    /// desktop, software fallback).
    #[default]
    HighPerformance,
    /// Prefer the integrated GPU (battery saving on dual-GPU laptops).
    /// On integrated-only machines this is identical to
    /// [`HighPerformance`](Self::HighPerformance) — there is only one
    /// adapter to pick.
    LowPower,
}

// ── TitleAlign ────────────────────────────────────────────────────────────────

/// Title text horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TitleAlign {
    /// Left-aligned after icon. Default.
    #[default]
    Left,
    /// Centred between left edge and button area.
    Center,
}
