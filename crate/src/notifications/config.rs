//! Configuration types for the `notifications` module.

use super::enums::{AnimationKind, Placement};
use super::theme::NotificationColors;
use crate::theme::Theme;

// ─── Center configuration ────────────────────────────────────────────────────

/// Configuration of the [`NotificationCenter`](super::NotificationCenter).
///
/// Defaults match the sensible "top-right, fade, 4 visible" toast pattern.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    #[serde(skip, default)]
    pub colors_override: Option<NotificationColors>,
}

impl Default for CenterConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in notifications/config.ron is valid")
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
        self.colors_override = Some(c);
        self
    }
    /// Resolved palette for rendering.
    pub(crate) fn resolved_colors(&self) -> NotificationColors {
        match &self.colors_override {
            Some(c) => c.clone(),
            None => self.theme.notifications(),
        }
    }
}
