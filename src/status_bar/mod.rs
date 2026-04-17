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

pub use config::{Alignment, StatusBarConfig};

use dear_imgui_rs::{MouseButton, Ui};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

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
            Self::None    => None,
            Self::Success => Some(cfg.color_success),
            Self::Warning => Some(cfg.color_warning),
            Self::Error   => Some(cfg.color_error),
            Self::Info    => Some(cfg.color_info),
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
    /// Internal id for click tracking.
    id: u32,
}

static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

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
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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

/// Event emitted when a clickable status item is activated.
#[derive(Debug, Clone)]
pub struct StatusBarEvent {
    /// The label of the clicked item.
    pub label: String,
    /// Internal item id.
    pub item_id: u32,
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

    /// Render the status bar inside the current ImGui window using
    /// `cursor_screen_pos()` + `content_region_avail()` as geometry.
    /// Returns click events.
    pub fn render(&self, ui: &Ui) -> Vec<StatusBarEvent> {
        let _id_tok = ui.push_id(&self.id);
        let avail_w = ui.content_region_avail()[0];
        let bar_h = self.config.height;
        let cursor = ui.cursor_screen_pos();
        let draw = ui.get_window_draw_list();

        let events = self.render_impl(ui, cursor, [avail_w, bar_h], &draw, true);

        // Advance cursor past the bar (legacy in-window contract).
        ui.set_cursor_pos([ui.cursor_pos()[0], ui.cursor_pos()[1] + bar_h]);
        ui.dummy([0.0, 0.0]);

        events
    }

    /// Overlay variant: renders through `ui.get_foreground_draw_list()` at an
    /// explicit screen-space position, without requiring a host ImGui window.
    ///
    /// - `origin` — top-left of the bar in **screen** coordinates.
    /// - `size` — bar width / height in logical pixels (height overrides
    ///   `config.height` for this call).
    ///
    /// Hover detection uses position-only (no `is_window_hovered`), so the bar
    /// stays responsive even when no ImGui window covers the region.
    pub fn render_overlay(
        &self,
        ui: &Ui,
        origin: [f32; 2],
        size: [f32; 2],
    ) -> Vec<StatusBarEvent> {
        let _id_tok = ui.push_id(&self.id);
        let draw = ui.get_foreground_draw_list();
        self.render_impl(ui, origin, size, &draw, false)
    }

    fn render_impl(
        &self,
        ui: &Ui,
        origin: [f32; 2],
        size: [f32; 2],
        draw: &dear_imgui_rs::DrawListMut,
        use_window_hovered: bool,
    ) -> Vec<StatusBarEvent> {
        let mut events = Vec::new();
        let cfg = &self.config;
        let avail_w = size[0];
        let bar_h = size[1];
        let cursor = origin;

        // Background
        draw.add_rect(
            cursor,
            [cursor[0] + avail_w, cursor[1] + bar_h],
            col32(cfg.color_bg),
        ).filled(true).build();

        // Top border line
        draw.add_line(
            cursor,
            [cursor[0] + avail_w, cursor[1]],
            col32(cfg.color_separator),
        ).build();

        // Use "Mg" for representative glyph height (covers ascenders + descenders).
        let text_y = cursor[1] + (bar_h - calc_text_size("Mg")[1]) * 0.5;

        // ── Left items ──────────────────────────────────────────────
        let mut x = cursor[0] + cfg.item_padding;
        for item in &self.left_items {
            let w = self.render_item(draw, ui, item, x, text_y, cursor[1], bar_h, use_window_hovered, &mut events);
            x += w + cfg.item_padding;

            if cfg.show_separators {
                draw.add_line(
                    [x, cursor[1] + 3.0],
                    [x, cursor[1] + bar_h - 3.0],
                    col32(cfg.color_separator),
                ).build();
                x += cfg.separator_width + cfg.item_padding;
            }
        }

        // ── Right items (render right-to-left) ─────────────────────
        let mut rx = cursor[0] + avail_w - cfg.item_padding;
        for item in self.right_items.iter().rev() {
            let w = self.measure_item(item);
            rx -= w;
            self.render_item(draw, ui, item, rx, text_y, cursor[1], bar_h, use_window_hovered, &mut events);
            rx -= cfg.item_padding;

            if cfg.show_separators {
                draw.add_line(
                    [rx, cursor[1] + 3.0],
                    [rx, cursor[1] + bar_h - 3.0],
                    col32(cfg.color_separator),
                ).build();
                rx -= cfg.separator_width + cfg.item_padding;
            }
        }

        // ── Center items ────────────────────────────────────────────
        if !self.center_items.is_empty() {
            let total_w: f32 = self.center_items.iter()
                .map(|i| self.measure_item(i) + cfg.item_padding)
                .sum::<f32>() - cfg.item_padding;
            let mut cx = cursor[0] + (avail_w - total_w) * 0.5;
            for item in &self.center_items {
                let w = self.render_item(draw, ui, item, cx, text_y, cursor[1], bar_h, use_window_hovered, &mut events);
                cx += w + cfg.item_padding;
            }
        }

        events
    }

    #[allow(clippy::too_many_arguments)]
    fn render_item(
        &self,
        draw: &dear_imgui_rs::DrawListMut,
        ui: &Ui,
        item: &StatusItem,
        x: f32,
        text_y: f32,
        bar_y: f32,
        bar_h: f32,
        use_window_hovered: bool,
        events: &mut Vec<StatusBarEvent>,
    ) -> f32 {
        let cfg = &self.config;
        let mut cx = x;
        let w = self.measure_item(item);

        let mouse_pos = ui.io().mouse_pos();
        let in_bounds = mouse_pos[0] >= x
            && mouse_pos[0] < x + w
            && mouse_pos[1] >= bar_y
            && mouse_pos[1] < bar_y + bar_h;
        let hovered = in_bounds && (!use_window_hovered || ui.is_window_hovered());

        // Hover paint — opt-in via `config.highlight_hover` (default: off).
        // Clicks are dispatched regardless of the flag so minimalist buttons
        // stay functional without any visual feedback.
        if hovered {
            if cfg.highlight_hover {
                let hover_bg = if item.clickable {
                    if ui.is_mouse_down(MouseButton::Left) { cfg.color_active } else { cfg.color_hover }
                } else {
                    [1.0, 1.0, 1.0, 0.04] // subtle highlight for non-clickable
                };
                draw.add_rect(
                    [x - 2.0, bar_y],
                    [x + w + 2.0, bar_y + bar_h],
                    col32(hover_bg),
                ).filled(true).build();
            }

            if item.clickable && ui.is_mouse_clicked(MouseButton::Left) {
                events.push(StatusBarEvent {
                    label: item.label.clone(),
                    item_id: item.id,
                });
            }
        }

        // Indicator dot
        if let Some(dot_color) = item.indicator.color(cfg) {
            let dot_r = 3.5;
            let dot_cx = cx + dot_r;
            let dot_cy = bar_y + bar_h * 0.5;
            draw.add_circle(
                [dot_cx, dot_cy],
                dot_r,
                col32(dot_color),
            ).filled(true).build();
            cx += dot_r * 2.0 + 4.0;
        }

        // Icon prefix
        if !item.icon.is_empty() {
            draw.add_text([cx, text_y], col32(item.color.unwrap_or(cfg.color_text)), &item.icon);
            cx += calc_text_size(&item.icon)[0] + 3.0;
        }

        // Progress bar or text
        if let Some(progress) = item.progress {
            let prog_w = 60.0;
            let prog_h = 8.0;
            let py = bar_y + (bar_h - prog_h) * 0.5;

            // Background
            draw.add_rect(
                [cx, py],
                [cx + prog_w, py + prog_h],
                col32([0.2, 0.2, 0.25, 1.0]),
            ).filled(true).build();

            // Fill
            let fill_w = prog_w * progress;
            if fill_w > 0.0 {
                draw.add_rect(
                    [cx, py],
                    [cx + fill_w, py + prog_h],
                    col32(cfg.color_info),
                ).filled(true).build();
            }

            cx += prog_w + 4.0;

            // Label after progress bar
            let text_color = item.color.unwrap_or(cfg.color_text_dim);
            draw.add_text([cx, text_y], col32(text_color), &item.label);
        } else {
            let text_color = item.color.unwrap_or(cfg.color_text);
            draw.add_text([cx, text_y], col32(text_color), &item.label);
        }

        // Tooltip
        if hovered
            && let Some(ref tip) = item.tooltip {
                ui.tooltip_text(tip);
            }

        w
    }

    fn measure_item(&self, item: &StatusItem) -> f32 {
        let mut w = 0.0_f32;

        // Icon prefix
        if !item.icon.is_empty() {
            w += calc_text_size(&item.icon)[0] + 3.0;
        }

        // Indicator dot
        if item.indicator != Indicator::None {
            w += 3.5 * 2.0 + 4.0;
        }

        // Progress bar
        if item.progress.is_some() {
            w += 60.0 + 4.0;
        }

        // Text
        w += calc_text_size(&item.label)[0];

        w
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
        // Minimal/static-looking bar by default — hover feedback is opt-in.
        assert!(!cfg.highlight_hover);
    }

    #[test]
    fn theme_presets_keep_hover_off_by_default() {
        // Every theme that ships a bundled StatusBarConfig must follow the
        // same default: no hover paint unless the caller explicitly enables it.
        #[cfg(feature = "status_bar")]
        {
            use crate::theme::Theme;
            for theme in [
                Theme::Dark,
                Theme::Light,
                Theme::Midnight,
                Theme::Solarized,
                Theme::Monokai,
            ] {
                assert!(
                    !theme.statusbar().highlight_hover,
                    "theme {theme:?} must default to highlight_hover=false",
                );
            }
        }
    }

    #[test]
    fn item_ids_unique() {
        let a = StatusItem::text("a");
        let b = StatusItem::text("b");
        assert_ne!(a.id, b.id);
    }
}
