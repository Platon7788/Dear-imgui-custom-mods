//! # StatusBar
//!
//! Composable bottom status bar with left/center/right sections.
//! Supports text items, clickable items, status indicators (colored dots),
//! and progress bars.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::status_bar::{StatusBar, StatusItem, Indicator};
//!
//! let mut bar = StatusBar::new("##status");
//! bar.left(StatusItem::indicator("Connected", Indicator::Success));
//! bar.left(StatusItem::text("Ln 42, Col 15"));
//! bar.right(StatusItem::text("UTF-8"));
//! bar.right(StatusItem::text("Rust"));
//! // In render loop: bar.render(ui);
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod config;
mod render;
mod tooltip;

pub use config::{Alignment, StatusBarConfig};

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::{calc_text_size, line_height};

use tooltip::paint_foreground_tooltip;

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

// ── Status indicator ────────────────────────────────────────────────────────

/// Visual status indicator (colored dot before text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indicator {
    None,
    Success,
    Warning,
    Error,
    Info,
}

impl Indicator {
    fn color(self, cfg: &StatusBarConfig) -> Option<[f32; 4]> {
        match self {
            Self::None => None,
            Self::Success => Some(cfg.colors.success),
            Self::Warning => Some(cfg.colors.warning),
            Self::Error => Some(cfg.colors.error),
            Self::Info => Some(cfg.colors.info),
        }
    }
}

// ── Status item ─────────────────────────────────────────────────────────────

/// A single item in the status bar.
#[derive(Debug, Clone)]
pub struct StatusItem {
    /// Display text.
    pub label: String,
    /// Unicode icon prefix (displayed before label).
    pub icon: String,
    /// Status indicator dot.
    pub indicator: Indicator,
    /// Whether this item is clickable (emits events).
    pub clickable: bool,
    /// Tooltip text (shown on hover).
    pub tooltip: Option<String>,
    /// Override text color.
    pub color: Option<[f32; 4]>,
    /// Progress value 0.0..=1.0 (draws a progress bar instead of text).
    pub progress: Option<f32>,
}

impl StatusItem {
    /// Plain text item.
    pub fn text(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: String::new(),
            indicator: Indicator::None,
            clickable: false,
            tooltip: None,
            color: None,
            progress: None,
        }
    }

    /// Text with a status indicator dot.
    pub fn indicator(label: impl Into<String>, ind: Indicator) -> Self {
        Self {
            indicator: ind,
            ..Self::text(label)
        }
    }

    /// Clickable text item.
    pub fn clickable(label: impl Into<String>) -> Self {
        Self {
            clickable: true,
            ..Self::text(label)
        }
    }

    /// Progress bar item (0.0..=1.0).
    pub fn progress(label: impl Into<String>, value: f32) -> Self {
        Self {
            progress: Some(value.clamp(0.0, 1.0)),
            ..Self::text(label)
        }
    }

    /// Builder: set tooltip.
    pub fn with_tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }

    /// Builder: set color override.
    pub fn with_color(mut self, c: [f32; 4]) -> Self {
        self.color = Some(c);
        self
    }

    /// Builder: set icon prefix.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

// ── Events ──────────────────────────────────────────────────────────────────

/// Which section produced a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSection {
    Left,
    Center,
    Right,
}

/// Event emitted when a clickable status item is activated.
#[derive(Debug, Clone)]
pub struct StatusBarEvent {
    /// The label of the clicked item.
    pub label: String,
    /// Section the clicked item belongs to.
    pub section: StatusSection,
    /// Position of the clicked item within its section (0-based).
    pub index: usize,
}

// ── StatusBar widget ────────────────────────────────────────────────────────

/// Bottom status bar widget.
pub struct StatusBar {
    id: String,
    left_items: Vec<StatusItem>,
    center_items: Vec<StatusItem>,
    right_items: Vec<StatusItem>,
    pub config: StatusBarConfig,
}

impl StatusBar {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            left_items: Vec::new(),
            center_items: Vec::new(),
            right_items: Vec::new(),
            config: StatusBarConfig::default(),
        }
    }

    /// Add an item to the left section.
    pub fn left(&mut self, item: StatusItem) -> &mut Self {
        self.left_items.push(item);
        self
    }

    /// Add an item to the center section.
    pub fn center(&mut self, item: StatusItem) -> &mut Self {
        self.center_items.push(item);
        self
    }

    /// Add an item to the right section.
    pub fn right(&mut self, item: StatusItem) -> &mut Self {
        self.right_items.push(item);
        self
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.left_items.clear();
        self.center_items.clear();
        self.right_items.clear();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_text() {
        let item = StatusItem::text("hello");
        assert_eq!(item.label, "hello");
        assert!(!item.clickable);
        assert_eq!(item.indicator, Indicator::None);
    }

    #[test]
    fn item_indicator() {
        let item = StatusItem::indicator("OK", Indicator::Success);
        assert_eq!(item.indicator, Indicator::Success);
    }

    #[test]
    fn item_clickable() {
        let item = StatusItem::clickable("Click me");
        assert!(item.clickable);
    }

    #[test]
    fn item_progress() {
        let item = StatusItem::progress("Loading", 0.5);
        assert_eq!(item.progress, Some(0.5));
    }

    #[test]
    fn item_progress_clamped() {
        let item = StatusItem::progress("Over", 1.5);
        assert_eq!(item.progress, Some(1.0));
    }

    #[test]
    fn item_builders() {
        let item = StatusItem::text("test")
            .with_tooltip("tip")
            .with_color([1.0, 0.0, 0.0, 1.0]);
        assert_eq!(item.tooltip.as_deref(), Some("tip"));
        assert!(item.color.is_some());
    }

    #[test]
    fn bar_add_items() {
        let mut bar = StatusBar::new("##test");
        bar.left(StatusItem::text("a"));
        bar.center(StatusItem::text("b"));
        bar.right(StatusItem::text("c"));
        assert_eq!(bar.left_items.len(), 1);
        assert_eq!(bar.center_items.len(), 1);
        assert_eq!(bar.right_items.len(), 1);
    }

    #[test]
    fn bar_clear() {
        let mut bar = StatusBar::new("##test");
        bar.left(StatusItem::text("a"));
        bar.right(StatusItem::text("b"));
        bar.clear();
        assert!(bar.left_items.is_empty());
        assert!(bar.right_items.is_empty());
    }

    #[test]
    fn indicator_colors() {
        let cfg = StatusBarConfig::default();
        assert!(Indicator::None.color(&cfg).is_none());
        assert!(Indicator::Success.color(&cfg).is_some());
        assert!(Indicator::Warning.color(&cfg).is_some());
        assert!(Indicator::Error.color(&cfg).is_some());
        assert!(Indicator::Info.color(&cfg).is_some());
    }

    #[test]
    fn config_defaults() {
        let cfg = StatusBarConfig::default();
        assert_eq!(cfg.height, 22.0);
        assert!(cfg.show_separators);
        // Layout values come from config.ron (DDD: schema in .rs, values
        // in .ron). Pin them so a silent ron edit can't drift the default.
        assert_eq!(cfg.item_padding, 8.0);
        assert_eq!(cfg.separator_width, 1.0);
        assert!(cfg.show_top_border);
        assert_eq!(cfg.top_border_offset_left, 0.0);
        assert_eq!(cfg.top_border_offset_right, 0.0);
        assert_eq!(cfg.progress_width, 60.0);
        assert_eq!(cfg.progress_height, 8.0);
    }

    #[test]
    fn config_round_trips_through_ron() {
        // Serialising the default config and parsing it back must
        // reproduce the same layout values. `colors` is `#[serde(skip)]`
        // so it is re-derived from `StatusBarColors::default()` on parse
        // (not present in the serialised string) — verify it survives.
        let cfg = StatusBarConfig::default();
        let s = ron::to_string(&cfg).expect("serialise StatusBarConfig");
        let back: StatusBarConfig = ron::from_str(&s).expect("re-parse StatusBarConfig");
        assert_eq!(back.height, cfg.height);
        assert_eq!(back.item_padding, cfg.item_padding);
        assert_eq!(back.separator_width, cfg.separator_width);
        assert_eq!(back.show_separators, cfg.show_separators);
        assert_eq!(back.show_top_border, cfg.show_top_border);
        assert_eq!(back.progress_width, cfg.progress_width);
        assert_eq!(back.progress_height, cfg.progress_height);
        // skipped field falls back to its Default
        assert_eq!(back.colors.bg, cfg.colors.bg);
    }

    #[test]
    fn config_ron_is_valid() {
        // The bundled config.ron must always parse — `Default` unwraps it,
        // so a malformed ron would panic the whole widget at construction.
        let cfg: StatusBarConfig =
            ron::from_str(include_str!("config.ron")).expect("bundled config.ron parses");
        assert_eq!(cfg.height, 22.0);
    }

    #[test]
    fn alignment_default_is_left() {
        assert_eq!(Alignment::default(), Alignment::Left);
    }

    #[test]
    fn alignment_round_trips_through_ron() {
        for a in [Alignment::Left, Alignment::Center, Alignment::Right] {
            let s = ron::to_string(&a).expect("serialise Alignment");
            let back: Alignment = ron::from_str(&s).expect("re-parse Alignment");
            assert_eq!(back, a);
        }
    }

    #[test]
    fn item_with_icon_builder() {
        let item = StatusItem::text("file").with_icon("\u{F0214}");
        assert_eq!(item.icon, "\u{F0214}");
        assert_eq!(item.label, "file");
    }

    #[test]
    fn item_progress_clamps_negative() {
        // The clamp guards both ends — a negative progress would otherwise
        // paint a negative fill width.
        let item = StatusItem::progress("neg", -0.5);
        assert_eq!(item.progress, Some(0.0));
    }

    #[test]
    fn event_section_and_index() {
        // Smoke: the event payload now carries section + index instead of an
        // opaque `u32`. Logic-only — render path requires a live ImGui ctx.
        let ev = StatusBarEvent {
            label: "click".into(),
            section: StatusSection::Right,
            index: 2,
        };
        assert_eq!(ev.section, StatusSection::Right);
        assert_eq!(ev.index, 2);
    }
}
