//! Rendering implementation for [`StatusBar`].
//!
//! Holds the public render entry points (`render`, `render_overlay`,
//! `render_overlay_foreground`), the shared layout body, and the
//! per-item paint/measure helpers. Split out of `mod.rs` (was 761
//! lines) so every file in the module stays under the 500-line cap.

use super::*;

impl StatusBar {
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
    /// fill will clobber the bar. [`crate::chrome::Chrome`]
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
        self.render_impl_with_tooltip_mode(ui, origin, size, draw, use_window_hovered, false)
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
            let w = self.measure_item(item);
            self.render_item(
                draw,
                ui,
                item,
                w,
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
                w,
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
            // Cache widths once: used for both the centering offset and
            // each item's paint, so `render_item` never re-measures.
            let center_widths: Vec<f32> = self
                .center_items
                .iter()
                .map(|i| self.measure_item(i))
                .collect();
            let total_w: f32 = center_widths
                .iter()
                .map(|w| w + cfg.item_padding)
                .sum::<f32>()
                - cfg.item_padding;
            let mut cx = cursor[0] + (avail_w - total_w) * 0.5;
            for (idx, item) in self.center_items.iter().enumerate() {
                let w = center_widths[idx];
                self.render_item(
                    draw,
                    ui,
                    item,
                    w,
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

    /// Paint one item at `x`. `w` is the item's pre-measured width
    /// (from [`Self::measure_item`]) — passed in so the caller's layout
    /// loop and this paint share a single measurement instead of
    /// measuring twice per frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_item(
        &self,
        draw: &dear_imgui_rs::DrawListMut,
        ui: &Ui,
        item: &StatusItem,
        w: f32,
        section: StatusSection,
        index: usize,
        x: f32,
        text_y: f32,
        bar_y: f32,
        bar_h: f32,
        use_window_hovered: bool,
        tooltip_in_foreground: bool,
        events: &mut Vec<StatusBarEvent>,
    ) {
        let cfg = &self.config;
        let mut cx = x;

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
    }

    /// Measure the rendered width of `item` in logical pixels. Pure
    /// layout math (icon + indicator dot + progress bar + label),
    /// reachable without an ImGui context only via the text-measuring
    /// path it shares with paint — kept here so paint and layout never
    /// disagree.
    pub(super) fn measure_item(&self, item: &StatusItem) -> f32 {
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
