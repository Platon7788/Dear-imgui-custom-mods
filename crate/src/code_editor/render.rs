//! `CodeEditor::render` — the per-frame draw + input-dispatch entry point.
//!
//! Split out of mod.rs (CLAUDE.md 500-line rule). Methods operate on the
//! `CodeEditor` struct defined in the parent module; private methods that
//! cross module boundaries are `pub(super)`.

use super::*;

impl CodeEditor {
    // ── Render ───────────────────────────────────────────────────────

    /// Render the editor. Call this every frame.
    ///
    /// The editor fills the available content region.
    pub fn render(&mut self, ui: &Ui) {
        // ── Font scale ───────────────────────────────────────────────
        // Push a scaled version of the current font so that char_advance,
        // line_height, and all text rendering use the correct size.
        // SAFETY: igPushFont / igPopFont are paired and always balanced.
        let base_font_size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
        let scaled_font_size = base_font_size * self.config.font_size_scale;
        unsafe {
            dear_imgui_rs::sys::igPushFont(code_editor_font_ptr(), scaled_font_size);
        }

        // Measure char advance using ImFont::CalcTextSizeA directly — the same
        // API that AddText uses internally.  The high-level igCalcTextSize adds
        // ceil(+0.99999) rounding which inflates char_advance and causes cursor
        // / selection positions to drift away from rendered glyph positions.
        // Using CalcTextSizeA gives the raw floating-point advance.
        self.char_advance = calc_char_advance(scaled_font_size);
        self.line_height = unsafe { dear_imgui_rs::sys::igGetTextLineHeight() } + 2.0;

        // Recompute caches if text changed. Fold regions are only needed
        // when the gutter actually draws the fold markers — skip the full
        // document scan otherwise. On a 10 000-line file this was running
        // on every keystroke even when folds weren't visible.
        // A language switch via config_mut() (rather than set_language) leaves
        // a stale token cache — every line keeps the old language's colours
        // until edited. Detect the change by discriminant and invalidate.
        let lang_discriminant = std::mem::discriminant(&self.config.language);
        if self.last_language != Some(lang_discriminant) {
            self.last_language = Some(lang_discriminant);
            self.bc_version = u64::MAX;
            self.bc_dirty_from = Some(0);
            self.token_cache.clear();
        }

        self.update_block_comment_states();
        if self.config.show_fold_indicators {
            self.update_fold_regions();
        }
        self.ensure_token_cache_size();
        // Backstop: single-cursor structural edits don't reconcile extra
        // cursors, so keep them in-bounds before anything indexes their lines.
        self.buffer.clamp_extra_cursors();

        let fold_extra = if self.config.show_fold_indicators {
            2.0
        } else {
            0.0
        };
        let gutter_width = if self.config.show_line_numbers {
            let digits = digit_count(self.buffer.line_count());
            // Layout: | padding | line_numbers | [fold_icon] | gap | code
            (digits as f32 + 1.3 + fold_extra) * self.char_advance
        } else if self.config.show_fold_indicators {
            self.char_advance * 2.0 // minimal gutter for fold arrows only
        } else {
            self.char_advance * 0.5 // tiny left margin, no gutter content
        };

        // ── Find/Replace bar at the TOP (before the editor child window) ──
        if self.find_replace.open {
            self.render_find_replace_bar(ui);
        }

        let avail = ui.content_region_avail();
        // `Arc::clone` is a single atomic refcount bump — way cheaper
        // than the historic `format!("##ce_{}", self.id)` alloc, and it
        // lets us drop the immutable borrow on `self` before the
        // `&mut self` closure below.
        let child_id = std::sync::Arc::clone(&self.child_id);

        // Push style for the editor region (uses theme's editor_bg)
        let _bg_token = ui.push_style_color(StyleColor::ChildBg, self.config.colors.editor_bg);

        // When word-wrap is active there is no horizontal content overflow,
        // so suppress the horizontal scrollbar entirely.  Keeping it visible
        // when wrapping would (a) waste vertical space and (b) shrink
        // inner_size[1], causing the last visible line to be clipped.
        let child_flags = if self.config.word_wrap {
            WindowFlags::NO_MOVE | WindowFlags::NO_SCROLL_WITH_MOUSE
        } else {
            WindowFlags::HORIZONTAL_SCROLLBAR
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SCROLL_WITH_MOUSE
        };

        ui.child_window(child_id.as_ref())
            .size(avail)
            .flags(child_flags)
            .build(ui, || {
                self.focused = ui.is_window_focused();

                // ── Keyboard layout switching on focus change ────────────
                self.handle_input_locale_switch();

                // Inner window size — the actual visible area of the child
                // window (accounts for scrollbar, border, padding).
                let inner_size = ui.window_size();
                self.visible_height = inner_size[1];

                // ── Word wrap cache ───────────────────────────────────────
                // Reserve the vertical scrollbar width so wrapped text never
                // sneaks under the scrollbar on the right edge. ImGui's
                // `content_region_avail()` already subtracts the scrollbar
                // width when one is visible — but we want a STABLE wrap
                // width regardless of scrollbar visibility, otherwise the
                // wrap toggles on/off as content grows over the threshold
                // (chicken-and-egg: wrap width depends on scrollbar, which
                // depends on content height, which depends on wrap width).
                // Always reserve `style.scrollbar_size` so the wrap is stable.
                // Read the field directly from the live ImGuiStyle instead of
                // `ui.clone_style()`, which deep-copies the entire ~700-byte
                // ImGuiStyle struct every frame just to read one f32.
                // SAFETY: igGetStyle returns the current frame's valid style ptr.
                let scrollbar_reserve =
                    unsafe { (*dear_imgui_rs::sys::igGetStyle()).ScrollbarSize };
                let text_area_w = (inner_size[0] - gutter_width - scrollbar_reserve).max(1.0);
                self.update_wrap_cache(text_area_w);

                // ── Read ImGui scroll state first ───────────────────────
                // This is the source of truth — user may have dragged the
                // scrollbar or ImGui processed wheel events.
                let imgui_scroll_y = ui.scroll_y();
                self.scroll_x = ui.scroll_x();

                // Detect external scroll change (scrollbar drag) — if ImGui's
                // scroll differs from what we wrote last frame, the user moved
                // the scrollbar directly.  Adopt that position as the new target
                // so smooth-scroll doesn't fight the scrollbar.
                let external_scroll = (imgui_scroll_y - self.last_set_scroll_y).abs() > 0.5;
                self.scroll_y = imgui_scroll_y;
                if external_scroll {
                    self.target_scroll_y = imgui_scroll_y;
                }

                // Update cursor blink
                let dt = ui.io().delta_time();
                self.update_blink(dt);

                // Smooth scrolling (modifies self.scroll_y toward target)
                self.update_smooth_scroll(dt);

                // Snapshot cursor position BEFORE input so we can tell whether
                // the cursor was actually moved this frame. `ensure_cursor_visible`
                // should fire only when the cursor moves (typing, arrow keys,
                // click) — NOT on every frame, otherwise any wheel-scroll that
                // takes the cursor off-screen gets immediately reverted back
                // because we force the cursor into the viewport.
                let cursor_before = self.buffer.cursor();

                // Handle input (may call ensure_cursor_visible → self.scroll_y)
                if self.focused {
                    self.handle_keyboard(ui);
                }
                self.handle_mouse(ui, gutter_width, inner_size);

                // Re-sync wrap cache after input — paste/Enter may have added
                // lines, so the pre-input cache is stale.
                self.update_wrap_cache(text_area_w);

                // Only pull scroll toward cursor when cursor itself moved.
                // Wheel / scrollbar-drag scrolling is free to take the cursor
                // out of view — the cursor stays clipped by the child window
                // naturally until the user types or arrows back to it.
                if self.buffer.cursor() != cursor_before {
                    self.ensure_cursor_visible();
                }

                // ── Sync scroll back to ImGui ───────────────────────────
                // Input handling may have updated self.scroll_y (e.g.
                // ensure_cursor_visible, smooth scroll, mouse wheel).
                // Apply it so ImGui's scrollbar and cursor_screen_pos
                // reflect the new state.
                ui.set_scroll_y(self.scroll_y);
                self.last_set_scroll_y = self.scroll_y;

                let draw_list = ui.get_window_draw_list();
                // cursor_screen_pos() includes the scroll offset (ImGui's
                // DC.CursorPos = Pos + Pad − Scroll).  So `win_x`/`win_y`
                // already have −scroll baked in — content drawn at
                // `win_y + line*h` lands at the correct screen position and
                // is automatically clipped by the child window.
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let scroll_x = ui.scroll_x();
                self.scroll_x = scroll_x;
                self.scroll_y = scroll_y;

                // Scroll-independent origin: the fixed top-left of the
                // content area in screen space.  Used for UI elements that
                // must NOT scroll (gutter X position).
                let origin_x = win_x + scroll_x;
                let origin_y = win_y + scroll_y;

                // first/last visible: in VISUAL ROW space when wrapping.
                let first_vrow = (scroll_y / self.line_height) as usize;
                let visible_count = (self.visible_height / self.line_height) as usize + 2;
                let last_vrow = first_vrow + visible_count;

                // Map visual rows back to buffer lines for the rendering loop.
                let (first_visible, _) = self.visual_row_to_line(first_vrow);
                let (last_vis_line, _) = self.visual_row_to_line(last_vrow);
                let last_visible = (last_vis_line + 1).min(self.buffer.line_count());

                // text_start_x: scrolls horizontally with content.
                // win_x already contains −scroll_x, so no extra subtraction.
                let text_start_x = win_x + gutter_width;
                let cursor_pos = self.buffer.cursor();
                let selection = self.buffer.selection();
                let matching_bracket = if self.config.bracket_matching {
                    self.buffer.find_matching_bracket()
                } else {
                    None
                };

                // ── Build visible line list (respecting folds) ──────────
                let visible_lines = self.build_visible_lines(first_visible, last_visible);
                let wrapping = self.config.word_wrap;

                // Pre-populate token cache for all visible lines so the draw
                // loop doesn't need &mut self (avoids per-line to_string() alloc).
                for &(line_idx, _) in &visible_lines {
                    self.get_cached_tokens(line_idx);
                }

                // ── Draw lines (batched) ────────────────────────────────
                self.draw_visible_lines(
                    &draw_list,
                    &visible_lines,
                    wrapping,
                    win_y,
                    origin_x,
                    inner_size,
                    text_start_x,
                    gutter_width,
                    cursor_pos,
                    selection,
                    matching_bracket,
                );

                // ── Cursor (primary + extras) ──────────────────────────
                self.draw_cursors(
                    &draw_list,
                    wrapping,
                    win_y,
                    origin_y,
                    inner_size,
                    text_start_x,
                    cursor_pos,
                );

                // ── Error marker tooltips ───────────────────────────────
                if ui.is_window_hovered() {
                    let [_mx, my] = ui.io().mouse_pos();
                    let hover_vrow = ((my - win_y) / self.line_height).max(0.0) as usize;
                    let (hover_line, _) = self.visual_row_to_line(hover_vrow);
                    for marker in &self.error_markers {
                        if marker.line == hover_line {
                            crate::utils::themed_tooltip(ui, || ui.text(&marker.message));
                            break;
                        }
                    }
                }

                // Set dummy size for scrolling.
                // The dummy must extend the content region so ImGui's scrollbar
                // covers the full document.  Height = all visual rows + a small
                // bottom margin so the last line is never clipped.
                let total_height =
                    self.total_visual_rows() as f32 * self.line_height + self.line_height; // extra row of padding at bottom
                let total_width = if wrapping {
                    inner_size[0]
                } else {
                    let max_line_len = (first_visible..last_visible)
                        .map(|i| self.buffer.line(i).chars().count())
                        .max()
                        .unwrap_or(80);
                    gutter_width + (max_line_len as f32 + 10.0) * self.char_advance
                };
                // Place cursor at the very end of the content area and emit
                // a 1px-tall dummy so ImGui registers the full scroll extent.
                ui.set_cursor_pos([0.0, total_height]);
                ui.dummy([total_width, 1.0]);

                // ── Right-click context menu ─────────────────────────────
                self.render_context_menu(ui);
            });

        // ── Pop the font pushed at the start of render() ─────────────
        // SAFETY: balances the igPushFont call at the top of this function.
        unsafe {
            dear_imgui_rs::sys::igPopFont();
        }
    }
}
