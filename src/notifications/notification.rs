//! Notification data type.

use super::enums::{Duration, Severity};
use super::theme::NotificationColors;

/// Action button shown inside a notification.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    /// Caller-defined identifier emitted in `NotificationEvent::ActionClicked`.
    pub id: u32,
    /// Button label.
    pub label: String,
}

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
    /// Pre-formatted ImGui window ID — built once at `push()` time so the
    /// hot render path doesn't `format!("##toast_{}", id)` every frame.
    pub(crate) win_id: String,
    /// Pre-formatted close-button ImGui ID (`##close_{id}`) — same
    /// rationale as `win_id`, eliminates a per-frame `format!` per
    /// visible toast.
    pub(crate) close_id: String,
    /// Pre-formatted action button ImGui IDs (`{label}##act_{id}_{n}`)
    /// — one per [`NotificationAction`] in `actions`. Built once at
    /// `push()` time. Same length as `actions`.
    pub(crate) action_ids: Vec<String>,
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
            // `NotificationCenter::push` overwrites these with the real
            // id-bearing strings; the placeholders here keep the struct
            // constructible from any of the severity-named factories.
            win_id: String::new(),
            close_id: String::new(),
            action_ids: Vec::new(),
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
