//! Configuration types for the `notifications` module.

use super::theme::NotificationColors;
use crate::theme::Theme;

// ─── Severity ────────────────────────────────────────────────────────────────

/// Semantic importance of a notification — drives icon and accent color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// Neutral blue — general information.
    #[default]
    Info,
    /// Green — operation succeeded.
    Success,
    /// Amber — caution, non-fatal.
    Warning,
    /// Red — error / failure.
    Error,
    /// Gray — developer-only diagnostic.
    Debug,
}

impl Severity {
    /// Human-readable single-word label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Debug => "Debug",
        }
    }

    /// Pick the severity's accent color from a palette.
    pub fn accent(self, c: &NotificationColors) -> [f32; 4] {
        match self {
            Self::Info => c.info,
            Self::Success => c.success,
            Self::Warning => c.warning,
            Self::Error => c.error,
            Self::Debug => c.debug,
        }
    }
}

// ─── Placement ───────────────────────────────────────────────────────────────

/// Stack anchor position within the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    /// Top-right corner — stack grows downward.
    #[default]
    TopRight,
    /// Top-left corner — stack grows downward.
    TopLeft,
    /// Bottom-right corner — stack grows upward.
    BottomRight,
    /// Bottom-left corner — stack grows upward.
    BottomLeft,
    /// Top-center — stack grows downward, centered.
    TopCenter,
    /// Bottom-center — stack grows upward, centered.
    BottomCenter,
}

impl Placement {
    /// `true` if the stack grows upward (bottom anchors) vs downward.
    pub(crate) fn grows_up(self) -> bool {
        matches!(
            self,
            Self::BottomRight | Self::BottomLeft | Self::BottomCenter
        )
    }
    /// `true` if the slide-in direction is from the left edge.
    pub(crate) fn slides_from_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
    /// `true` if the slide-in direction is from the right edge.
    pub(crate) fn slides_from_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }
}

// ─── Duration ────────────────────────────────────────────────────────────────

/// How long a notification stays on screen before auto-dismissing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Duration {
    /// Auto-dismiss after N seconds.
    Timed(f32),
    /// Never auto-dismiss — user must close it manually.
    Sticky,
}

impl Default for Duration {
    fn default() -> Self {
        Self::Timed(4.0)
    }
}

// ─── Animation ───────────────────────────────────────────────────────────────

/// Entry / exit animation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationKind {
    /// Fade alpha 0 → 1 on enter, 1 → 0 on exit.
    #[default]
    Fade,
    /// Slide horizontally from the anchor edge + fade.
    SlideIn,
    /// Instant appear / disappear.
    None,
}

// ─── Action button ───────────────────────────────────────────────────────────

/// Action button shown inside a notification.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    /// Caller-defined identifier emitted in `NotificationEvent::ActionClicked`.
    pub id: u32,
    /// Button label.
    pub label: String,
}

// ─── Notification ────────────────────────────────────────────────────────────

/// A single notification. Use the severity-named constructors + builder methods.
#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub severity: Severity,
    pub duration: Duration,
    pub closable: bool,
    pub show_progress: bool,
    /// Show remaining seconds as text in the top-right of the toast ("4.2s").
    pub show_countdown: bool,
    pub show_icon: bool,
    pub actions: Vec<NotificationAction>,
    /// Overrides the severity accent color (icon, left strip, progress bar).
    pub custom_color: Option<[f32; 4]>,

    // ── runtime state (managed by NotificationCenter) ────────────────────────
    pub(crate) id: u64,
    pub(crate) elapsed: f32,
    pub(crate) enter_t: f32, // 0..=1 appear-animation progress
    pub(crate) exit_t: f32,  // 0..=1 dismiss-animation progress (0 = not dismissing)
    pub(crate) dismissing: bool,
}

impl Notification {
    fn base(severity: Severity, title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: String::new(),
            severity,
            duration: Duration::default(),
            closable: true,
            show_progress: false,
            show_countdown: false,
            show_icon: true,
            actions: Vec::new(),
            custom_color: None,
            id: 0,
            elapsed: 0.0,
            enter_t: 0.0,
            exit_t: 0.0,
            dismissing: false,
        }
    }

    /// Neutral info — default accent is blue.
    pub fn info(title: impl Into<String>) -> Self {
        Self::base(Severity::Info, title)
    }
    /// Success — default accent is green.
    pub fn success(title: impl Into<String>) -> Self {
        Self::base(Severity::Success, title)
    }
    /// Warning — default accent is amber.
    pub fn warning(title: impl Into<String>) -> Self {
        Self::base(Severity::Warning, title)
    }
    /// Error — default accent is red.
    pub fn error(title: impl Into<String>) -> Self {
        Self::base(Severity::Error, title)
    }
    /// Debug — default accent is gray.
    pub fn debug(title: impl Into<String>) -> Self {
        Self::base(Severity::Debug, title)
    }

    /// Set the body text (shown under the title, word-wrapped).
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }
    /// Auto-dismiss after `secs` seconds.
    pub fn with_duration_secs(mut self, secs: f32) -> Self {
        self.duration = Duration::Timed(secs);
        self
    }
    /// Never auto-dismiss — user must click `×` or an action.
    pub fn sticky(mut self) -> Self {
        self.duration = Duration::Sticky;
        self
    }
    /// Add an action button — caller receives `ActionClicked { id, action_id: id }` on click.
    pub fn with_action(mut self, id: u32, label: impl Into<String>) -> Self {
        self.actions.push(NotificationAction {
            id,
            label: label.into(),
        });
        self
    }
    /// Override the severity's default accent color.
    pub fn with_custom_color(mut self, color: [f32; 4]) -> Self {
        self.custom_color = Some(color);
        self
    }
    /// Disable the `×` close button (user can only dismiss via action / timeout).
    pub fn not_closable(mut self) -> Self {
        self.closable = false;
        self
    }
    /// Disable the leading severity icon.
    pub fn without_icon(mut self) -> Self {
        self.show_icon = false;
        self
    }
    /// Disable the bottom progress bar (still ticks toward auto-dismiss).
    pub fn without_progress(mut self) -> Self {
        self.show_progress = false;
        self
    }

    /// Show remaining seconds as compact text in the top-right of the toast.
    ///
    /// Displays `"10s"` for ≥ 10 s remaining, `"4.2s"` for < 10 s.
    /// Has no effect on sticky notifications. Can be combined with or without
    /// the bottom progress bar.
    pub fn with_countdown(mut self) -> Self {
        self.show_countdown = true;
        self
    }

    /// Effective accent color (custom override or severity default).
    pub(crate) fn resolved_accent(&self, c: &NotificationColors) -> [f32; 4] {
        self.custom_color.unwrap_or_else(|| self.severity.accent(c))
    }
}

// ─── Center configuration ────────────────────────────────────────────────────

/// Configuration of the [`NotificationCenter`](super::NotificationCenter).
///
/// Defaults match the sensible "top-right, fade, 4 visible" toast pattern.
#[derive(Debug, Clone)]
pub struct CenterConfig {
    /// Stack anchor. Default: `TopRight`.
    pub placement: Placement,
    /// Maximum number of notifications shown at once. Default: `5`.
    pub max_visible: usize,
    /// Vertical gap between stacked toasts (px). Default: `8.0`.
    pub spacing: f32,
    /// `[x, y]` offset from the anchor edge (px). Default: `[16.0, 16.0]`.
    pub margin: [f32; 2],
    /// Toast width (px). Default: `340.0`.
    pub width: f32,
    /// Internal padding inside each toast (px). Default: `[12.0, 10.0]`.
    pub padding: [f32; 2],
    /// Corner rounding radius (px). Default: `6.0`.
    pub rounding: f32,
    /// Width of the accent strip on the leading edge (px). Default: `4.0`.
    pub accent_strip: f32,
    /// Height of the bottom progress bar (px). Default: `3.0`.
    pub progress_height: f32,
    /// Entry / exit animation kind. Default: `Fade`.
    pub animation: AnimationKind,
    /// Animation duration (seconds). Default: `0.25`.
    pub animation_duration: f32,
    /// Pause the auto-dismiss timer while the cursor is over the toast. Default: `true`.
    pub pause_on_hover: bool,
    /// Color theme. Default: `Dark`.
    pub theme: Theme,
    /// Optional custom palette (overrides [`theme`](Self::theme)).
    pub colors_override: Option<Box<NotificationColors>>,
}

impl Default for CenterConfig {
    fn default() -> Self {
        Self {
            placement: Placement::TopRight,
            max_visible: 5,
            spacing: 8.0,
            margin: [16.0, 16.0],
            width: 340.0,
            padding: [12.0, 10.0],
            rounding: 6.0,
            accent_strip: 4.0,
            progress_height: 3.0,
            animation: AnimationKind::Fade,
            animation_duration: 0.25,
            pause_on_hover: true,
            theme: Theme::Dark,
            colors_override: None,
        }
    }
}

impl CenterConfig {
    /// Start from defaults and mutate via builder methods.
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }
    pub fn with_max_visible(mut self, n: usize) -> Self {
        self.max_visible = n.max(1);
        self
    }
    pub fn with_spacing(mut self, s: f32) -> Self {
        self.spacing = s;
        self
    }
    pub fn with_margin(mut self, mx: f32, my: f32) -> Self {
        self.margin = [mx, my];
        self
    }
    pub fn with_width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn with_padding(mut self, px: f32, py: f32) -> Self {
        self.padding = [px, py];
        self
    }
    pub fn with_rounding(mut self, r: f32) -> Self {
        self.rounding = r;
        self
    }
    pub fn with_animation(mut self, a: AnimationKind) -> Self {
        self.animation = a;
        self
    }
    pub fn with_animation_duration(mut self, secs: f32) -> Self {
        self.animation_duration = secs.max(0.0);
        self
    }
    pub fn with_pause_on_hover(mut self, on: bool) -> Self {
        self.pause_on_hover = on;
        self
    }
    pub fn with_theme(mut self, t: Theme) -> Self {
        self.theme = t;
        self.colors_override = None;
        self
    }
    /// Use a custom [`NotificationColors`] palette instead of the built-in theme.
    pub fn with_colors(mut self, c: NotificationColors) -> Self {
        self.colors_override = Some(Box::new(c));
        self
    }
    /// Resolved palette for rendering.
    pub(crate) fn resolved_colors(&self) -> NotificationColors {
        if let Some(c) = &self.colors_override {
            (**c).clone()
        } else {
            self.theme.notifications()
        }
    }
}
