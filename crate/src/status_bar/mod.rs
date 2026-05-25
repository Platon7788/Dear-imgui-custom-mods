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
use crate::utils::text::{calc_text_size, line_height};

fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// 2026-05-25 (vex0r session 130) — paint a tooltip body directly
/// into a foreground draw list, positioned ABOVE the cursor so the
/// status bar (which lives on the same foreground draw list) cannot
/// overlap it.
///
/// Used by `render_overlay_foreground` because a normal
/// `ui.tooltip(..)` body would land in a TopLayer ImGui window —
/// which paints BELOW the foreground draw list and gets sliced by
/// the bar strip the user is hovering. Painting into the SAME
/// foreground draw list with `cursor.y - box_h - 8 px gap` puts
/// the tooltip body above the cursor and outside the bar strip.
///
/// Geometry mirrors the crate-wide `themed_tooltip` look: 10×8 px
/// padding, 4 px corner rounding, 1 px border in the bar's
/// separator colour, background in the bar's bg colour but at
/// `0.95` alpha so the chrome under it bleeds through a touch and
/// the tooltip reads as floating rather than baked-in.
fn paint_foreground_tooltip(
    ui: &Ui,
    draw: &dear_imgui_rs::DrawListMut,
    cfg: &StatusBarConfig,
    text: &str,
) {
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 8.0;
    const ROUND: f32 = 4.0;
    const CURSOR_GAP_Y: f32 = 8.0; // gap between cursor and tooltip bottom

    let text_size = calc_text_size(text);
    let box_w = text_size[0] + 2.0 * PAD_X;
    let box_h = text_size[1] + 2.0 * PAD_Y;

    let mouse = ui.io().mouse_pos();
    let display = ui.io().display_size();

    // Anchor box above the cursor; clamp inside the viewport.
    let mut tip_y = mouse[1] - box_h - CURSOR_GAP_Y;
    if tip_y < 4.0 {
        // No room above — fall back to below the cursor. Bar's
        // foreground body lives at the very bottom strip, so anything
        // above the cursor is fine; below-cursor would only be needed
        // if cursor is near the TOP of the viewport, where there's no
        // bar to overlap anyway.
        tip_y = (mouse[1] + CURSOR_GAP_Y).min(display[1] - box_h - 4.0);
    }
    let tip_x = mouse[0]
        .min(display[0] - box_w - 4.0)
        .max(4.0);

    // Background — slight transparency so the tooltip floats above
    // the chrome rather than baking in as part of the bar.
    let mut bg = cfg.colors.bg;
    bg[3] = 0.95;
    draw.add_rect([tip_x, tip_y], [tip_x + box_w, tip_y + box_h], col32(bg))
        .filled(true)
        .rounding(ROUND)
        .build();
    draw.add_rect(
        [tip_x, tip_y],
        [tip_x + box_w, tip_y + box_h],
        col32(cfg.colors.separator),
    )
    .rounding(ROUND)
    .thickness(1.0)
    .build();
    draw.add_text(
        [tip_x + PAD_X, tip_y + PAD_Y],
        col32(cfg.colors.text),
        text,
    );
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

    /// Overlay variant: renders through `ui.get_background_draw_list()`
    /// at an explicit screen-space position, without requiring a host
    /// ImGui window.
    ///
    /// - `origin` — top-left of the bar in **screen** coordinates.
    /// - `size` — bar width / height in logical pixels (height overrides
    ///   `config.height` for this call).
    ///
    /// **Z-order note (2026-04-29):** the bar paints into the
    /// **background** draw list rather than the foreground one. This
    /// keeps it visually above the page surface but **below** every
    /// ImGui popup — tooltips, context menus, modal dialogs — so a
    /// tooltip raised by another widget can never get clipped by the
    /// status bar. If the host genuinely needs the bar to sit on top
    /// of popups (rare — usually a sign the tooltip should move
    /// instead), use [`Self::render_overlay_foreground`].
    ///
    /// **Host requirement:** the background draw list is drawn
    /// **before** all ImGui windows. If the host renders a top-level
    /// window over the same region, that window's `WindowBg` style
    /// fill will clobber the bar. [`crate::app_window::AppWindow`]
    /// hosts a full-window root behind every frame — when running
    /// under it, use [`Self::render_overlay_foreground`] instead so
    /// the bar paints into the foreground draw list (above all
    /// windows).
    ///
    /// Hover detection uses position-only (no `is_window_hovered`), so
    /// the bar stays responsive even when no ImGui window covers the
    /// region.
    pub fn render_overlay(&self, ui: &Ui, origin: [f32; 2], size: [f32; 2]) -> Vec<StatusBarEvent> {
        let _id_tok = ui.push_id(&self.id);
        let draw = ui.get_background_draw_list();
        self.render_impl(ui, origin, size, &draw, false)
    }

    /// Foreground-overlay variant of [`Self::render_overlay`] — paints
    /// into `ui.get_foreground_draw_list()`, which lives **above** every
    /// ImGui popup (tooltips, menus, modal dialogs).
    ///
    /// Use only when the bar genuinely must sit on top of popups —
    /// e.g. a kiosk-mode HUD that should always be readable. For
    /// standard chrome bars prefer [`Self::render_overlay`] so the
    /// host's tooltips don't get clipped.
    pub fn render_overlay_foreground(
        &self,
        ui: &Ui,
        origin: [f32; 2],
        size: [f32; 2],
    ) -> Vec<StatusBarEvent> {
        let _id_tok = ui.push_id(&self.id);
        let draw = ui.get_foreground_draw_list();
        // 2026-05-25 (vex0r session 130) — last arg flips tooltip
        // rendering into the "paint into the same foreground draw
        // list, above the cursor" path so per-icon tooltips don't get
        // clipped by the bar body that sits on the same draw list.
        self.render_impl_with_tooltip_mode(ui, origin, size, &draw, false, true)
    }

    fn render_impl(
        &self,
        ui: &Ui,
        origin: [f32; 2],
        size: [f32; 2],
        draw: &dear_imgui_rs::DrawListMut,
        use_window_hovered: bool,
    ) -> Vec<StatusBarEvent> {
        self.render_impl_with_tooltip_mode(
            ui, origin, size, draw, use_window_hovered, false,
        )
    }

    /// Shared body for both overlay variants. `tooltip_in_foreground`
    /// switches per-item tooltips from the default `ui.tooltip(..)`
    /// (TopLayer ImGui window) to a manual paint INTO the same
    /// `draw` list this body uses. Required when the bar itself lives
    /// in the foreground draw list — TopLayer windows render below
    /// foreground, so a stock `ui.tooltip` body would be sliced by
    /// the bar strip the user is hovering. The manual path:
    ///
    /// * positions the box ABOVE the cursor so the bar can never
    ///   overlap it (cursor.y - box_h - 8 px gap),
    /// * clamps to the viewport on both axes,
    /// * paints background + 1 px border + text into the foreground
    ///   draw list so it sits on top of everything (including the
    ///   bar's own background fill from a few `draw` calls earlier).
    fn render_impl_with_tooltip_mode(
        &self,
        ui: &Ui,
        origin: [f32; 2],
        size: [f32; 2],
        draw: &dear_imgui_rs::DrawListMut,
        use_window_hovered: bool,
        tooltip_in_foreground: bool,
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
            col32(cfg.colors.bg),
        )
        .filled(true)
        .build();

        // Top border line — optional, with per-side offsets so a
        // left/right-docked nav panel can claim its slice of the edge
        // and prevent a "phantom seam" running through the panel's
        // surface. Top-docked nav doesn't intersect this edge so no
        // offset is needed for that case.
        if cfg.show_top_border {
            let line_x0 = cursor[0] + cfg.top_border_offset_left.max(0.0);
            let line_x1 = (cursor[0] + avail_w - cfg.top_border_offset_right.max(0.0)).max(line_x0);
            if line_x1 > line_x0 {
                draw.add_line(
                    [line_x0, cursor[1]],
                    [line_x1, cursor[1]],
                    col32(cfg.colors.separator),
                )
                .build();
            }
        }

        // `line_height(ui)` is `igGetTextLineHeight()` — a direct read
        // from `ImGuiContext::FontSize`, no glyph walk per frame.
        let text_y = cursor[1] + (bar_h - line_height(ui)) * 0.5;

        // ── Left items ──────────────────────────────────────────────
        let mut x = cursor[0] + cfg.item_padding;
        for (idx, item) in self.left_items.iter().enumerate() {
            let w = self.render_item(
                draw,
                ui,
                item,
                StatusSection::Left,
                idx,
                x,
                text_y,
                cursor[1],
                bar_h,
                use_window_hovered,
                tooltip_in_foreground,
                &mut events,
            );
            x += w + cfg.item_padding;

            if cfg.show_separators {
                draw.add_line(
                    [x, cursor[1] + 3.0],
                    [x, cursor[1] + bar_h - 3.0],
                    col32(cfg.colors.separator),
                )
                .build();
                x += cfg.separator_width + cfg.item_padding;
            }
        }

        // ── Right items (render right-to-left) ─────────────────────
        // Cache widths once so `render_item` doesn't re-measure (was: 2× per item).
        let right_widths: Vec<f32> = self
            .right_items
            .iter()
            .map(|i| self.measure_item(i))
            .collect();
        let mut rx = cursor[0] + avail_w - cfg.item_padding;
        for (rev_idx, item) in self.right_items.iter().enumerate().rev() {
            let w = right_widths[rev_idx];
            rx -= w;
            self.render_item(
                draw,
                ui,
                item,
                StatusSection::Right,
                rev_idx,
                rx,
                text_y,
                cursor[1],
                bar_h,
                use_window_hovered,
                tooltip_in_foreground,
                &mut events,
            );
            rx -= cfg.item_padding;

            if cfg.show_separators {
                draw.add_line(
                    [rx, cursor[1] + 3.0],
                    [rx, cursor[1] + bar_h - 3.0],
                    col32(cfg.colors.separator),
                )
                .build();
                rx -= cfg.separator_width + cfg.item_padding;
            }
        }

        // ── Center items ────────────────────────────────────────────
        if !self.center_items.is_empty() {
            let total_w: f32 = self
                .center_items
                .iter()
                .map(|i| self.measure_item(i) + cfg.item_padding)
                .sum::<f32>()
                - cfg.item_padding;
            let mut cx = cursor[0] + (avail_w - total_w) * 0.5;
            for (idx, item) in self.center_items.iter().enumerate() {
                let w = self.render_item(
                    draw,
                    ui,
                    item,
                    StatusSection::Center,
                    idx,
                    cx,
                    text_y,
                    cursor[1],
                    bar_h,
                    use_window_hovered,
                    tooltip_in_foreground,
                    &mut events,
                );
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
        section: StatusSection,
        index: usize,
        x: f32,
        text_y: f32,
        bar_y: f32,
        bar_h: f32,
        use_window_hovered: bool,
        tooltip_in_foreground: bool,
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

        // No built-in hover paint — the bar stays fully static
        // visually. Clicks (on `clickable` items) and tooltips are
        // still dispatched on hover; the host application is free to
        // wrap individual items in its own visual hover treatment if
        // needed.
        if hovered && item.clickable && ui.is_mouse_clicked(MouseButton::Left) {
            events.push(StatusBarEvent {
                label: item.label.clone(),
                section,
                index,
            });
        }

        // Indicator dot
        if let Some(dot_color) = item.indicator.color(cfg) {
            let dot_r = 3.5;
            let dot_cx = cx + dot_r;
            let dot_cy = bar_y + bar_h * 0.5;
            draw.add_circle([dot_cx, dot_cy], dot_r, col32(dot_color))
                .filled(true)
                .build();
            cx += dot_r * 2.0 + 4.0;
        }

        // Icon prefix
        if !item.icon.is_empty() {
            draw.add_text(
                [cx, text_y],
                col32(item.color.unwrap_or(cfg.colors.text)),
                &item.icon,
            );
            cx += calc_text_size(&item.icon)[0] + 3.0;
        }

        // Progress bar or text
        if let Some(progress) = item.progress {
            let prog_w = cfg.progress_width;
            let prog_h = cfg.progress_height;
            let py = bar_y + (bar_h - prog_h) * 0.5;

            // Background
            draw.add_rect(
                [cx, py],
                [cx + prog_w, py + prog_h],
                col32([0.2, 0.2, 0.25, 1.0]),
            )
            .filled(true)
            .build();

            // Fill
            let fill_w = prog_w * progress;
            if fill_w > 0.0 {
                draw.add_rect([cx, py], [cx + fill_w, py + prog_h], col32(cfg.colors.info))
                    .filled(true)
                    .build();
            }

            cx += prog_w + 4.0;

            // Label after progress bar
            let text_color = item.color.unwrap_or(cfg.colors.text_dim);
            draw.add_text([cx, text_y], col32(text_color), &item.label);
        } else {
            let text_color = item.color.unwrap_or(cfg.colors.text);
            draw.add_text([cx, text_y], col32(text_color), &item.label);
        }

        // Tooltip — two paint paths.
        //
        // When the bar lives in a normal ImGui Window (`render(ui)`),
        // tooltips go through the crate-wide `themed_tooltip` helper
        // → `ui.tooltip(..)` → a TopLayer tooltip Window. Standard
        // ImGui z-order puts that Window above the bar's own Window,
        // so the tooltip body is always fully visible.
        //
        // When the bar is painted into the FOREGROUND draw list
        // (`render_overlay_foreground`), a TopLayer tooltip Window
        // would draw BELOW the foreground — the bar strip would
        // slice the bottom of the tooltip body that hangs next to a
        // bar-edge icon (vex0r session 130: "Kernel symbols ready
        // (1651/1723)" tooltip was visibly cut off). We paint the
        // tooltip into the SAME foreground draw list instead, with
        // the body anchored ABOVE the cursor so the bar's strip
        // never overlaps it.
        if hovered && let Some(ref tip) = item.tooltip {
            if tooltip_in_foreground {
                paint_foreground_tooltip(ui, draw, cfg, tip);
            } else {
                crate::utils::themed_tooltip(ui, || ui.text(tip));
            }
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
            w += self.config.progress_width + 4.0;
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
