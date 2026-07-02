//! Mouse input handling for [`CodeEditor`] (click, drag-select, gutter
//! fold toggle, wheel scroll, Ctrl+wheel zoom, context-menu open).
//! Split out of input.rs (500-line rule).

use super::*;

impl CodeEditor {
    pub(super) fn handle_mouse(&mut self, ui: &Ui, gutter_width: f32, inner_size: [f32; 2]) {
        // Clear the drag latch as soon as the button is up, even if the pointer
        // has left the window — otherwise releasing a drag outside the window
        // leaves `mouse_selecting` stuck on and a later hover resumes selecting.
        if self.mouse_selecting && ui.is_mouse_released(MouseButton::Left) {
            self.mouse_selecting = false;
        }
        if !ui.is_window_hovered() {
            return;
        }

        let io = ui.io();
        let [mx, my] = io.mouse_pos();
        // cursor_screen_pos includes −scroll, so it's the scroll-adjusted
        // origin.  origin_* compensates back to the fixed window position.
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_x = ui.scroll_x();
        let scroll_y = ui.scroll_y();
        let origin_x = win_x + scroll_x;
        let origin_y = win_y + scroll_y;
        let text_x = win_x + gutter_width;

        // ── Ctrl+Scroll zoom ──────────────────────────────────────────────
        if io.key_ctrl() && io.mouse_wheel() != 0.0 {
            self.config.font_size_scale =
                (self.config.font_size_scale + io.mouse_wheel() * 0.1).clamp(0.4, 4.0);
        }

        // ── I-beam cursor ONLY inside the text content area ───────────────
        let content_max_x = origin_x + inner_size[0];
        let content_max_y = origin_y + inner_size[1];
        if mx >= origin_x + gutter_width
            && mx < content_max_x
            && my >= origin_y
            && my < content_max_y
        {
            // SAFETY: igSetMouseCursor is a standard ImGui call.
            unsafe {
                dear_imgui_rs::sys::igSetMouseCursor(
                    dear_imgui_rs::sys::ImGuiMouseCursor_TextInput,
                );
            }
        }

        // Convert mouse position to text position.
        // win_y already includes −scroll_y, so (my − win_y) / h gives
        // the visual row directly.
        let vrow = ((my - win_y) / self.line_height).max(0.0) as usize;
        let (line, sub_row) = self.visual_row_to_line(vrow);
        let line = line.min(self.buffer.line_count().saturating_sub(1));
        let line_content = self.buffer.line(line).to_string();

        // For wrapped sub-rows, map into the column range.
        let (col_start, col_end) = self.sub_row_col_range(line, sub_row);
        // saturating_sub: defensive against stale wrap caches (see
        // sub_row_col_range) — we never want to trigger usize underflow here.
        let sub_str: String = line_content
            .chars()
            .skip(col_start)
            .take(col_end.saturating_sub(col_start))
            .collect();

        let raw_col = col_start
            + x_to_col(
                &sub_str,
                (mx - text_x).max(0.0),
                self.char_advance,
                self.config.tab_size,
            );
        let col = raw_col.min(line_content.chars().count());
        let click_pos = CursorPos::new(line, col);

        let time = ui.time();

        // Click in gutter area → toggle fold
        if self.config.show_fold_indicators && ui.is_mouse_clicked(MouseButton::Left) && mx < text_x
        {
            let has_fold = self.fold_regions.iter().any(|r| r.start_line == line);
            if has_fold {
                self.toggle_fold(line);
                return;
            }
        }

        if ui.is_mouse_clicked(MouseButton::Left) {
            // Alt+Click: add extra cursor at click position
            if ui.io().key_alt() && !self.config.read_only {
                self.buffer.add_cursor(click_pos);
                self.reset_blink();
                return;
            }

            // Any non-Alt click clears extra cursors
            if self.buffer.has_extra_cursors() {
                self.buffer.clear_extra_cursors();
            }

            // Detect double/triple click
            if time - self.last_click_time < 0.4 && self.last_click_pos == click_pos {
                self.click_count = (self.click_count + 1).min(3);
            } else {
                self.click_count = 1;
            }
            self.last_click_time = time;
            self.last_click_pos = click_pos;

            match self.click_count {
                1 => {
                    if ui.io().key_shift() {
                        let anchor = self
                            .buffer
                            .selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor);
                        self.buffer.set_selection(anchor, click_pos);
                    } else {
                        self.buffer.set_cursor_clear_sel(click_pos);
                    }
                }
                2 => {
                    self.buffer.set_cursor(click_pos);
                    self.buffer.select_word_at_cursor();
                }
                3 => {
                    self.buffer.set_cursor(click_pos);
                    self.buffer.select_line();
                }
                _ => {}
            }
            // Latch drag-select for every click kind, so a drag after a
            // double/triple click keeps extending the selection (from the
            // word/line anchor) instead of doing nothing.
            self.mouse_selecting = true;
            self.reset_blink();
        }

        if ui.is_mouse_dragging(MouseButton::Left) && self.mouse_selecting {
            let anchor = self
                .buffer
                .selection()
                .map_or(self.buffer.cursor(), |s| s.anchor);
            self.buffer.set_selection(anchor, click_pos);
            // Edge auto-scroll: dragging near the top/bottom edge scrolls so
            // the selection can extend past the visible viewport.
            let edge = self.line_height * 1.5;
            if my < origin_y + edge {
                self.scroll_y = (self.scroll_y - self.line_height).max(0.0);
                self.target_scroll_y = self.scroll_y;
            } else if my > content_max_y - edge {
                let max_scroll = (self.total_visual_rows() as f32 * self.line_height).max(0.0);
                self.scroll_y = (self.scroll_y + self.line_height).min(max_scroll);
                self.target_scroll_y = self.scroll_y;
            }
        }

        if ui.is_mouse_released(MouseButton::Left) {
            self.mouse_selecting = false;
        }

        // ── Right-click → context menu ────────────────────────────────────
        if ui.is_mouse_clicked(MouseButton::Right) {
            // Move cursor to click position if nothing is selected
            if self.buffer.selection().is_none() {
                self.buffer.set_cursor_clear_sel(click_pos);
                self.reset_blink();
            }
            if self.config.context_menu.enabled {
                ui.open_popup("##editor_ctx");
            }
        }

        // Scroll with mouse wheel (smooth) — suppressed when Ctrl is held (zoom mode)
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 && !io.key_ctrl() {
            let delta = -wheel * self.config.scroll_speed * self.line_height;
            // Use total VISUAL rows (word-wrap aware) — not buffer.line_count().
            // With wrap on, one text line can produce N visual rows, so clamping
            // to line_count instead of total_visual_rows() caps wheel scroll at
            // a tiny fraction of the actual document height and the scrollbar
            // appears stuck. Keep a one-row bottom margin for consistency with
            // the total_height used by the dummy element (see render()).
            let max_scroll = (self.total_visual_rows() as f32 * self.line_height).max(0.0);
            if self.config.smooth_scrolling {
                self.target_scroll_y = (self.target_scroll_y + delta).clamp(0.0, max_scroll);
            } else {
                self.scroll_y = (self.scroll_y + delta).clamp(0.0, max_scroll);
                self.target_scroll_y = self.scroll_y;
                ui.set_scroll_y(self.scroll_y);
            }
        }
    }
}
