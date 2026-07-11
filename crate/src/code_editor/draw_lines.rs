//! `draw_visible_lines` + `draw_cursors` — the per-frame line-drawing
//! loop and caret rendering for [`CodeEditor`]. Split out of draw.rs to
//! keep both files under the 500-line ceiling.

use super::*;

impl CodeEditor {
    /// Draw every visible line: per-line decorations (current-line bg,
    /// error/warning marker, breakpoint, fold indicator, line number),
    /// selection + find highlights, gutter separator, tokenized text,
    /// hex colour swatches, and bracket-match highlight. Word-wrap aware.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_visible_lines(
        &mut self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        visible_lines: &[(usize, usize)],
        wrapping: bool,
        win_y: f32,
        origin_x: f32,
        inner_size: [f32; 2],
        text_start_x: f32,
        gutter_width: f32,
        cursor_pos: CursorPos,
        selection: Option<buffer::Selection>,
        matching_bracket: Option<CursorPos>,
    ) {
        for &(line_idx, _screen_row) in visible_lines {
            let line_str = self.buffer.line(line_idx);

            // How many visual sub-rows does this line occupy?
            let sub_row_count = if wrapping && line_idx < self.wrap_cols.len() {
                self.wrap_cols[line_idx].len() + 1
            } else {
                1
            };

            for sub_row in 0..sub_row_count {
                // Fold-aware display row (also collapses the vertical gap left
                // by folded lines — visual_row_of accounts for both wrap and
                // folds). Non-wrapped lines used to hardcode `line_idx` here,
                // which is why folding left blank rows.
                let col_at_start = if wrapping && sub_row > 0 {
                    self.wrap_cols[line_idx][sub_row - 1]
                } else {
                    0
                };
                let vrow = self.visual_row_of(line_idx, col_at_start);
                let y = win_y + (vrow as f32) * self.line_height;

                // Column range for this sub-row
                let (col_start, col_end) = if wrapping {
                    self.sub_row_col_range(line_idx, sub_row)
                } else {
                    (0, line_str.chars().count())
                };

                // Opaque gutter background (up to the separator). Painted
                // under the line numbers / fold markers so the horizontally
                // scrolled code column can never show through the gutter.
                draw_list
                    .add_rect(
                        [origin_x, y],
                        [
                            origin_x + gutter_width - self.char_advance * 0.5,
                            y + self.line_height,
                        ],
                        col32(self.config.colors.gutter_bg),
                    )
                    .filled(true)
                    .build();

                // ── Per-line decorations (only on first sub-row) ──
                if sub_row == 0 {
                    // Current line highlight — drawn BEFORE selection so
                    // the selection overlay is visible on top.  Also skip
                    // when there is an active selection touching this line
                    // so the selection color isn't washed out.
                    let sel_on_line = selection.is_some_and(|s| {
                        let (a, b) = s.ordered();
                        line_idx >= a.line
                            && line_idx <= b.line
                            && !(a.line == b.line && a.col == b.col)
                    });
                    if self.config.highlight_current_line
                        && line_idx == cursor_pos.line
                        && self.focused
                        && !sel_on_line
                    {
                        let num_rows = sub_row_count as f32;
                        draw_list
                            .add_rect(
                                [origin_x, y],
                                [origin_x + inner_size[0], y + self.line_height * num_rows],
                                col32(self.config.colors.current_line_bg),
                            )
                            .filled(true)
                            .build();
                    }

                    // Error / warning marker background. Colour comes
                    // from the active theme (`error_underline` /
                    // `warning_underline`) instead of a hardcoded red,
                    // and honours `LineMarker::is_error` so warnings
                    // render amber rather than red. Both config knobs
                    // were previously dead — defined per-theme but
                    // never read in the draw path.
                    if self.error_lines.contains(&line_idx) {
                        // A line is a "warning" only if it has markers
                        // and none of them are errors.
                        let is_warning = self
                            .error_markers
                            .iter()
                            .filter(|m| m.line == line_idx)
                            .all(|m| !m.is_error)
                            && self.error_markers.iter().any(|m| m.line == line_idx);
                        let base = if is_warning {
                            self.config.colors.warning_underline
                        } else {
                            self.config.colors.error_underline
                        };
                        // Dim to a translucent row highlight (the raw
                        // theme colour is full-opacity for underlines).
                        let marker_bg = [base[0], base[1], base[2], 0.15];
                        draw_list
                            .add_rect(
                                [origin_x, y],
                                [origin_x + inner_size[0], y + self.line_height],
                                col32(marker_bg),
                            )
                            .filled(true)
                            .build();
                    }

                    // Breakpoint marker in gutter
                    if self.breakpoint_lines.contains(&line_idx) {
                        let center = [origin_x + gutter_width * 0.2, y + self.line_height * 0.5];
                        let radius = self.line_height * 0.3;
                        draw_list
                            .add_circle(center, radius, col32(self.config.colors.breakpoint))
                            .filled(true)
                            .build();
                    }

                    // Fold indicator in gutter
                    if self.config.show_fold_indicators {
                        self.draw_fold_indicator(draw_list, line_idx, origin_x, gutter_width, y);
                    }

                    // Line number — scratch buffer keeps its
                    // capacity across frames (10 digits max),
                    // so this branch is zero-alloc after the
                    // first repaint.
                    if self.config.show_line_numbers {
                        use std::fmt::Write as _;
                        self.gutter_buf.clear();
                        let _ = write!(self.gutter_buf, "{}", line_idx + 1);
                        let num_color = if line_idx == cursor_pos.line {
                            self.config.colors.line_number_active
                        } else {
                            self.config.colors.line_number
                        };
                        let right_pad = if self.config.show_fold_indicators {
                            2.5
                        } else {
                            0.5
                        };
                        let num_x = origin_x + gutter_width
                            - (self.gutter_buf.len() as f32 + right_pad) * self.char_advance;
                        // Nudge line-number down by the annotation-strip offset
                        // so it aligns with its line's tokens, not floats up in
                        // the caption band when Above() is active.
                        draw_list.add_text(
                            [num_x, y + self.text_baseline_dy],
                            col32(num_color),
                            self.gutter_buf.as_str(),
                        );
                    }
                }

                // Clip the horizontally-scrolling code column to the right of
                // the gutter so long lines / selections / find highlights can
                // never paint over the fixed line-number gutter. Tighten only
                // the LEFT edge and keep the child window's existing vertical
                // clip for top/right/bottom — anchoring Y at `win_y` (= line 0's
                // scrolled screen position) clipped away every row past the
                // first viewport-height, blanking the text below ~line 50.
                let clip_min = draw_list.clip_rect_min();
                let clip_max = draw_list.clip_rect_max();
                let _code_clip = draw_list.push_clip_rect(
                    [
                        origin_x + gutter_width - self.char_advance * 0.5,
                        clip_min[1],
                    ],
                    [clip_max[0], clip_max[1]],
                    true,
                );

                // ── Selection & find highlights (every sub-row) ──
                // Drawn AFTER current-line-bg so the selection is on top.
                if let Some(sel) = selection {
                    self.draw_selection(
                        draw_list,
                        sel,
                        line_idx,
                        line_str,
                        text_start_x,
                        y,
                        col_start,
                        col_end,
                    );
                }
                for sel in self
                    .buffer
                    .extra_selections()
                    .iter()
                    .filter_map(|s| s.as_ref())
                {
                    self.draw_selection(
                        draw_list,
                        *sel,
                        line_idx,
                        line_str,
                        text_start_x,
                        y,
                        col_start,
                        col_end,
                    );
                }
                self.draw_find_matches(
                    draw_list,
                    line_idx,
                    line_str,
                    text_start_x,
                    y,
                    col_start,
                    col_end,
                );

                // ── Wash pass (behind tokens) ─────────────────────────
                // Line-decoration Wash overlays go BEHIND the token text
                // so the glyphs stay readable on top of the tinted rect.
                self.draw_wash_pass(
                    draw_list,
                    line_idx,
                    line_str,
                    text_start_x,
                    y,
                    col_start,
                    col_end,
                );

                // Gutter separator line (every sub-row)
                draw_list
                    .add_line(
                        [origin_x + gutter_width - self.char_advance * 0.5, y],
                        [
                            origin_x + gutter_width - self.char_advance * 0.5,
                            y + self.line_height,
                        ],
                        col32(self.config.colors.gutter_separator),
                    )
                    .build();

                // ── Tokenized text ───────────────────────────────
                if !wrapping || sub_row_count == 1 {
                    // No wrapping — draw full line as before.
                    let tokens = self.cached_tokens(line_idx);
                    self.draw_tokens_batched(draw_list, &tokens, line_str, text_start_x, y);
                } else {
                    // Word wrap: draw only the columns for this sub-row.
                    let tokens = self.cached_tokens(line_idx);
                    self.draw_tokens_batched_range(
                        draw_list,
                        &tokens,
                        line_str,
                        text_start_x,
                        y,
                        col_start,
                        col_end,
                    );
                }

                if sub_row == 0 && self.config.show_color_swatches {
                    self.draw_hex_color_swatches(draw_list, line_str, text_start_x, y);
                }

                // ── Rule / Ghost / EndPill / Captions ─────────────────
                // Painted ON TOP of tokens so the semantic hints stay
                // visible over highlighted syntax. EndPill only on the
                // last sub-row of the logical line. Captions only when
                // AnnotationStrip is enabled — early-out inside.
                self.draw_rule_pass(
                    draw_list,
                    line_idx,
                    line_str,
                    text_start_x,
                    y,
                    col_start,
                    col_end,
                );
                self.draw_ghost_pass(
                    draw_list,
                    line_idx,
                    line_str,
                    text_start_x,
                    y,
                    col_start,
                    col_end,
                );
                if sub_row == sub_row_count - 1 {
                    self.draw_end_pill_pass(
                        draw_list,
                        line_idx,
                        line_str,
                        text_start_x,
                        y,
                        col_start,
                    );
                }
                self.draw_captions_pass(
                    draw_list,
                    line_idx,
                    line_str,
                    text_start_x,
                    y,
                    col_start,
                    col_end,
                );

                // Bracket match highlight (check all sub-rows)
                if let Some(match_pos) = matching_bracket {
                    let col_start_x =
                        col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
                    // Highlight the matched bracket
                    if match_pos.line == line_idx
                        && match_pos.col >= col_start
                        && match_pos.col < col_end
                    {
                        let bx = text_start_x
                            + col_to_x(
                                line_str,
                                match_pos.col,
                                self.char_advance,
                                self.config.tab_size,
                            )
                            - col_start_x;
                        draw_list
                            .add_rect(
                                [bx, y + self.text_baseline_dy],
                                [bx + self.char_advance, y + self.line_height],
                                col32(self.config.colors.bracket_match_bg),
                            )
                            .filled(true)
                            .build();
                    }
                    // Highlight the cursor bracket
                    if cursor_pos.line == line_idx
                        && cursor_pos.col >= col_start
                        && cursor_pos.col < col_end
                    {
                        let bx = text_start_x
                            + col_to_x(
                                line_str,
                                cursor_pos.col,
                                self.char_advance,
                                self.config.tab_size,
                            )
                            - col_start_x;
                        draw_list
                            .add_rect(
                                [bx, y + self.text_baseline_dy],
                                [bx + self.char_advance, y + self.line_height],
                                col32(self.config.colors.bracket_match_bg),
                            )
                            .filled(true)
                            .build();
                    }
                }
            }
        }
    }

    /// Draw the primary text cursor plus any extra (multi-cursor) carets.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_cursors(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        wrapping: bool,
        win_y: f32,
        origin_y: f32,
        inner_size: [f32; 2],
        text_start_x: f32,
        cursor_pos: CursorPos,
    ) {
        if self.focused && self.cursor_visible && !self.config.read_only {
            let cursor_vrow = self.visual_row_of(cursor_pos.line, cursor_pos.col);
            let (col_start, _) = if wrapping {
                let (_, sub) = self.visual_row_to_line(cursor_vrow);
                self.sub_row_col_range(cursor_pos.line, sub)
            } else {
                (0usize, 0usize)
            };
            let cursor_line_str = self.buffer.line(cursor_pos.line);
            let cx = text_start_x
                + col_to_x(
                    cursor_line_str,
                    cursor_pos.col,
                    self.char_advance,
                    self.config.tab_size,
                )
                - col_to_x(
                    cursor_line_str,
                    col_start,
                    self.char_advance,
                    self.config.tab_size,
                )
                - 1.0;
            let cy = win_y + cursor_vrow as f32 * self.line_height;
            // Cursor spans the text portion of the row — starts below the
            // annotation strip when Above() is active so it doesn't reach
            // into the caption band.
            draw_list
                .add_line(
                    [cx, cy + self.text_baseline_dy],
                    [cx, cy + self.line_height],
                    col32(self.config.colors.cursor),
                )
                .thickness(1.5)
                .build();

            // Draw extra cursors — themed from the caret colour (slightly
            // translucent to read as secondary) instead of a hardcoded blue
            // that clashed on light themes.
            let extra_cursor_col = {
                let c = self.config.colors.cursor;
                col32([c[0], c[1], c[2], 0.85])
            };
            for extra in self.buffer.extra_cursors() {
                let ev = self.visual_row_of(extra.line, extra.col);
                let extra_line_str = self.buffer.line(extra.line);
                let extra_col_start = if wrapping {
                    let (_, esub) = self.visual_row_to_line(ev);
                    self.sub_row_col_range(extra.line, esub).0
                } else {
                    0
                };
                let ex = text_start_x
                    + col_to_x(
                        extra_line_str,
                        extra.col,
                        self.char_advance,
                        self.config.tab_size,
                    )
                    - col_to_x(
                        extra_line_str,
                        extra_col_start,
                        self.char_advance,
                        self.config.tab_size,
                    )
                    - 1.0;
                let ey = win_y + ev as f32 * self.line_height;
                if ey >= origin_y - self.line_height && ey <= origin_y + inner_size[1] {
                    draw_list
                        .add_line(
                            [ex, ey + self.text_baseline_dy],
                            [ex, ey + self.line_height],
                            extra_cursor_col,
                        )
                        .thickness(1.5)
                        .build();
                }
            }
        }
    }
}
