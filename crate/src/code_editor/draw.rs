//! Low-level draw helpers for [`CodeEditor`] — token batching, selection
//! / find-match rectangles, fold indicators, hex colour swatches, and the
//! token-kind -> colour mapping. Split out of mod.rs.

use super::*;

impl CodeEditor {
    // ── Drawing helpers ─────────────────────────────────────────────

    /// Draw tokens using batched draw calls — consecutive tokens of the same
    /// color are merged into a single `AddText()` call.
    pub(super) fn draw_tokens_batched(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        tokens: &[Token],
        line_str: &str,
        text_start_x: f32,
        y: f32,
    ) {
        if tokens.is_empty() {
            return;
        }

        let mut batch_start_x = text_start_x;
        let mut batch_color = self.token_color(tokens[0].kind);
        let mut batch_text = String::with_capacity(64);
        let mut x = text_start_x;

        for tok in tokens {
            let byte_end = (tok.start + tok.len).min(line_str.len());
            // Guard: skip any token whose byte range doesn't sit on UTF-8 char
            // boundaries (can happen with multi-byte chars in the fallback path).
            if !line_str.is_char_boundary(tok.start) || !line_str.is_char_boundary(byte_end) {
                // Advance x approximately so subsequent tokens stay aligned.
                x += tok.len as f32 * self.char_advance;
                continue;
            }
            let text = &line_str[tok.start..byte_end];
            let color = self.token_color(tok.kind);

            if tok.kind == TokenKind::Whitespace {
                // Flush current batch before whitespace
                if !batch_text.is_empty() {
                    draw_list.add_text(
                        [batch_start_x, y + self.text_baseline_dy],
                        col32(batch_color),
                        &batch_text,
                    );
                    batch_text.clear();
                }

                if self.config.show_whitespace {
                    for ch in text.chars() {
                        let ch_w = if ch == '\t' {
                            self.char_advance * self.config.tab_size as f32
                        } else {
                            self.char_advance
                        };
                        if ch == ' ' {
                            let cx = x + ch_w * 0.5;
                            let cy = y + self.text_baseline_dy + self.text_line_height * 0.5;
                            draw_list
                                .add_circle(
                                    [cx, cy],
                                    1.0,
                                    col32(self.config.colors.whitespace_marker),
                                )
                                .filled(true)
                                .build();
                        } else if ch == '\t' {
                            let arrow_y = y + self.text_baseline_dy + self.text_line_height * 0.5;
                            draw_list
                                .add_line(
                                    [x + 2.0, arrow_y],
                                    [x + ch_w - 2.0, arrow_y],
                                    col32(self.config.colors.whitespace_marker),
                                )
                                .build();
                        }
                        x += ch_w;
                    }
                } else {
                    // Account for tab character width even when not drawing
                    // whitespace markers (tabs are wider than regular chars).
                    for ch in text.chars() {
                        if ch == '\t' {
                            x += self.char_advance * self.config.tab_size as f32;
                        } else {
                            x += self.char_advance;
                        }
                    }
                }

                batch_start_x = x;
                batch_color = color;
                continue;
            }

            // Same color → extend batch
            if color == batch_color && !batch_text.is_empty() {
                batch_text.push_str(text);
                x += text.chars().count() as f32 * self.char_advance;
                continue;
            }

            // Different color → flush and start new batch
            if !batch_text.is_empty() {
                draw_list.add_text(
                    [batch_start_x, y + self.text_baseline_dy],
                    col32(batch_color),
                    &batch_text,
                );
            }

            batch_text.clear();
            batch_text.push_str(text);
            batch_start_x = x;
            batch_color = color;
            x += text.chars().count() as f32 * self.char_advance;
        }

        // Flush final batch
        if !batch_text.is_empty() {
            draw_list.add_text(
                [batch_start_x, y + self.text_baseline_dy],
                col32(batch_color),
                &batch_text,
            );
        }
    }

    /// Draw tokens for a sub-range of columns (used by word wrap).
    ///
    /// Only characters in `col_start..col_end` are drawn, positioned
    /// starting at `text_start_x`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_tokens_batched_range(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        tokens: &[Token],
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        if tokens.is_empty() {
            return;
        }

        // Build a char→byte mapping for the range.
        let chars: Vec<(usize, char)> = line_str.char_indices().collect();
        let byte_start = chars.get(col_start).map_or(line_str.len(), |&(b, _)| b);
        let byte_end = chars.get(col_end).map_or(line_str.len(), |&(b, _)| b);

        let mut x = text_start_x;
        let mut batch_start_x = x;
        let mut batch_color = [0.0f32; 4];
        let mut batch_text = String::with_capacity(64);
        let mut first_batch = true;

        for tok in tokens {
            let tok_byte_end = (tok.start + tok.len).min(line_str.len());
            // Skip tokens entirely outside our column range.
            if tok_byte_end <= byte_start || tok.start >= byte_end {
                continue;
            }
            // Clip token to our range.
            let clip_start = tok.start.max(byte_start);
            let clip_end = tok_byte_end.min(byte_end);
            if !line_str.is_char_boundary(clip_start) || !line_str.is_char_boundary(clip_end) {
                continue;
            }
            let text = &line_str[clip_start..clip_end];
            let color = self.token_color(tok.kind);

            if tok.kind == TokenKind::Whitespace {
                if !batch_text.is_empty() {
                    draw_list.add_text(
                        [batch_start_x, y + self.text_baseline_dy],
                        col32(batch_color),
                        &batch_text,
                    );
                    batch_text.clear();
                }
                for ch in text.chars() {
                    x += if ch == '\t' {
                        self.char_advance * self.config.tab_size as f32
                    } else {
                        self.char_advance
                    };
                }
                batch_start_x = x;
                first_batch = true;
                continue;
            }

            if !first_batch && color == batch_color {
                batch_text.push_str(text);
                x += text.chars().count() as f32 * self.char_advance;
                continue;
            }

            if !batch_text.is_empty() {
                draw_list.add_text(
                    [batch_start_x, y + self.text_baseline_dy],
                    col32(batch_color),
                    &batch_text,
                );
            }
            batch_text.clear();
            batch_text.push_str(text);
            batch_start_x = x;
            batch_color = color;
            first_batch = false;
            x += text.chars().count() as f32 * self.char_advance;
        }
        if !batch_text.is_empty() {
            draw_list.add_text(
                [batch_start_x, y + self.text_baseline_dy],
                col32(batch_color),
                &batch_text,
            );
        }
    }

    /// Draw small colored swatches next to hex color literals on a single line.
    ///
    /// Recognises `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `0xRRGGBB`, `0xAARRGGBB`.
    pub(super) fn draw_hex_color_swatches(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_str: &str,
        text_start_x: f32,
        y: f32,
    ) {
        let swatch = (self.text_line_height - 4.0).max(6.0);
        // Centre against the text portion, not the full row, so an annotation
        // strip doesn't push the swatch off-baseline from the token it refers to.
        let sy_off = self.text_baseline_dy + (self.text_line_height - swatch) / 2.0;
        let bytes = line_str.as_bytes();
        let len = bytes.len();
        let mut i = 0usize;

        while i < len {
            // Find start of a potential hex token
            let (tok_start, tok_end) =
                if bytes[i] == b'#' && i + 1 < len && bytes[i + 1].is_ascii_hexdigit() {
                    let s = i;
                    i += 1;
                    while i < len && bytes[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                    (s, i)
                } else if i + 1 < len
                    && bytes[i] == b'0'
                    && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
                {
                    let s = i;
                    i += 2;
                    while i < len && bytes[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                    (s, i)
                } else {
                    // Advance safely over one Unicode scalar
                    i += line_str[i..].chars().next().map_or(1, |c| c.len_utf8());
                    continue;
                };

            // Safety: tok_start/tok_end are on ASCII boundaries
            if !line_str.is_char_boundary(tok_start) || !line_str.is_char_boundary(tok_end) {
                continue;
            }
            let token_text = &line_str[tok_start..tok_end];
            if let Some(color) = parse_hex_color(token_text) {
                // x position: chars up to end of token × char_advance + gap
                let char_end = line_str[..tok_end].chars().count();
                let sx = text_start_x
                    + col_to_x(line_str, char_end, self.char_advance, self.config.tab_size)
                    + 2.0;
                let sy = y + sy_off;

                // Filled swatch
                draw_list
                    .add_rect([sx, sy], [sx + swatch, sy + swatch], col32(color))
                    .filled(true)
                    .build();
                // Dark border
                draw_list
                    .add_rect(
                        [sx, sy],
                        [sx + swatch, sy + swatch],
                        col32([0.0, 0.0, 0.0, 0.55]),
                    )
                    .filled(false)
                    .build();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_selection(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        sel: buffer::Selection,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        let (start, end) = sel.ordered();
        if line_idx < start.line || line_idx > end.line {
            return;
        }

        let line_chars = line_str.chars().count();
        let sel_start = if line_idx == start.line { start.col } else { 0 };
        let sel_end = if line_idx == end.line {
            end.col
        } else {
            line_chars
        };

        // Clip to the sub-row column range
        let vis_start = sel_start.max(col_start);
        let vis_end = sel_end.min(col_end);

        // A line fully inside the selection (not the end line) includes the
        // trailing newline; extend one glyph past the real EOL so blank/short
        // middle lines still show the highlight instead of a gap. Only on the
        // sub-row that actually ends the logical line (wrap-aware).
        let eol_pad = if line_idx < end.line && vis_end >= line_chars {
            self.char_advance
        } else {
            0.0
        };

        if vis_start > vis_end || (vis_start == vis_end && eol_pad <= 0.0) {
            return;
        }

        // X positions are relative to col_start (the sub-row starts at text_start_x)
        let base_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        let x1 = text_start_x
            + col_to_x(line_str, vis_start, self.char_advance, self.config.tab_size)
            - base_x;
        let x2 = text_start_x
            + col_to_x(line_str, vis_end, self.char_advance, self.config.tab_size)
            - base_x
            + eol_pad;
        let bg = self.config.colors.selection_bg;
        draw_list
            .add_rect([x1, y], [x2, y + self.line_height], col32(bg))
            .filled(true)
            .build();
        // Thin border for extra visibility
        let border_color = [bg[0], bg[1], bg[2], (bg[3] + 0.25).min(1.0)];
        draw_list
            .add_rect([x1, y], [x2, y + self.line_height], col32(border_color))
            .build();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_find_matches(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        if !self.find_replace.open {
            return;
        }
        // Matches are stored in document order — binary-search the range
        // belonging to `line_idx` instead of scanning all M matches per
        // visible line. Previous implementation was O(V × M); on a 10 000-
        // match find with 50 visible rows this drops from 500K iterations
        // to O(log M + k) where k = matches on this line (typically 0-3).
        let matches = &self.find_replace.matches;
        let lo = matches.partition_point(|(ml, _, _)| *ml < line_idx);
        let hi = matches.partition_point(|(ml, _, _)| *ml <= line_idx);
        if lo == hi {
            return;
        }

        let col_start_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        for (i, &(_, cs, ce)) in matches.iter().enumerate().take(hi).skip(lo) {
            // Clip match to sub-row range
            let vis_start = cs.max(col_start);
            let vis_end = ce.min(col_end);
            if vis_start >= vis_end {
                continue;
            }

            let x1 = text_start_x
                + col_to_x(line_str, vis_start, self.char_advance, self.config.tab_size)
                - col_start_x;
            let x2 = text_start_x
                + col_to_x(line_str, vis_end, self.char_advance, self.config.tab_size)
                - col_start_x;
            let color = if i == self.find_replace.current_match {
                self.config.colors.search_current_bg
            } else {
                self.config.colors.search_match_bg
            };
            draw_list
                .add_rect([x1, y], [x2, y + self.line_height], col32(color))
                .filled(true)
                .build();
        }
    }

    pub(super) fn draw_fold_indicator(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        win_x: f32,
        gutter_width: f32,
        y: f32,
    ) {
        let region = self.fold_regions.iter().find(|r| r.start_line == line_idx);
        if let Some(region) = region {
            // Position the icon in the gutter, right before line numbers.
            // Place fold icon at right edge of gutter, between line numbers and code
            let icon_x = win_x + gutter_width - self.char_advance * 1.8;
            let icon_y = y;
            // Theme-derived so the chevron stays legible on light themes
            // (the old hardcoded grey vanished on white backgrounds).
            let color = col32(self.config.colors.line_number);
            let color_hover = col32(self.config.colors.line_number_active);

            // Use MDI chevron icons for crisp rendering at any size.
            let icon = if region.folded {
                icons::CHEVRON_RIGHT // ▸ collapsed
            } else {
                icons::CHEVRON_DOWN // ▾ expanded
            };

            // Highlight on hover (mouse in the fold icon area).
            let mouse_pos = unsafe { dear_imgui_rs::sys::igGetMousePos() };
            let in_fold_area = mouse_pos.x >= icon_x
                && mouse_pos.x < icon_x + self.char_advance * 1.5
                && mouse_pos.y >= y
                && mouse_pos.y < y + self.line_height;
            let c = if in_fold_area { color_hover } else { color };

            draw_list.add_text([icon_x, icon_y], c, icon);

            // Draw "... N lines" badge after the line text when folded.
            if region.folded {
                let hidden = region.end_line.saturating_sub(region.start_line);
                if hidden > 0 {
                    let badge = format!(" ... {hidden} lines ");
                    let line_str = self.buffer.line(line_idx);
                    let text_x = win_x + gutter_width + 4.0;
                    // CRITICAL: use `chars().count()` not `len()` so
                    // the badge anchors at the last visible glyph,
                    // not at the byte length. UTF-8 multibyte chars
                    // (Cyrillic, CJK, emoji) made the badge drift
                    // by N bytes for any non-ASCII source line.
                    // Same fix for `badge_w` below.
                    // Tab-aware anchor so the badge doesn't overlap code on
                    // tab-indented folded lines.
                    let badge_x = text_x
                        + col_to_x(
                            line_str,
                            line_str.chars().count(),
                            self.char_advance,
                            self.config.tab_size,
                        );
                    let badge_y = y;
                    let badge_w = badge.chars().count() as f32 * self.char_advance;

                    // Badge colours from the theme, chosen for CONTRAST so the
                    // "... N lines" pill is clearly visible (a folded block must
                    // be obvious). current_line_bg is lighter than gutter_bg on
                    // dark themes and darker on light ones — readable on both.
                    let bg_c = self.config.colors.current_line_bg;
                    let bg = col32([bg_c[0], bg_c[1], bg_c[2], 0.95]);
                    let border = col32(self.config.colors.gutter_separator);
                    draw_list
                        .add_rect(
                            [badge_x, badge_y + 1.0],
                            [badge_x + badge_w, badge_y + self.line_height - 1.0],
                            bg,
                        )
                        .filled(true)
                        .rounding(3.0)
                        .build();
                    draw_list
                        .add_rect(
                            [badge_x, badge_y + 1.0],
                            [badge_x + badge_w, badge_y + self.line_height - 1.0],
                            border,
                        )
                        .rounding(3.0)
                        .build();

                    // Badge text — bright active-line-number colour for legibility.
                    let text_col = col32(self.config.colors.line_number_active);
                    draw_list.add_text([badge_x, badge_y], text_col, &badge);
                }
            }
        }
    }

    pub(super) fn token_color(&self, kind: TokenKind) -> [f32; 4] {
        match kind {
            TokenKind::Keyword => self.config.colors.keyword,
            TokenKind::TypeName => self.config.colors.type_name,
            TokenKind::Lifetime => self.config.colors.lifetime,
            TokenKind::String => self.config.colors.string,
            TokenKind::CharLit => self.config.colors.char_lit,
            TokenKind::Number => self.config.colors.number,
            TokenKind::Comment => self.config.colors.comment,
            // Doc comments: tint the comment colour toward the keyword accent so
            // they read as "special" — theme-aware, no per-theme palette field.
            TokenKind::DocComment => {
                let c = self.config.colors.comment;
                let k = self.config.colors.keyword;
                [
                    c[0] * 0.6 + k[0] * 0.4,
                    c[1] * 0.6 + k[1] * 0.4,
                    c[2] * 0.6 + k[2] * 0.4,
                    c[3],
                ]
            }
            TokenKind::Attribute => self.config.colors.attribute,
            TokenKind::MacroCall => self.config.colors.macro_call,
            TokenKind::Operator => self.config.colors.operator,
            TokenKind::Punctuation => self.config.colors.punctuation,
            TokenKind::Identifier => self.config.colors.identifier,
            TokenKind::Whitespace => self.config.colors.identifier,
            TokenKind::UserCodeMarker => self.config.colors.user_code_marker,
            TokenKind::HexNull => self.config.colors.hex_null,
            TokenKind::HexFF => self.config.colors.hex_ff,
            TokenKind::HexDefault => self.config.colors.hex_default,
            TokenKind::HexPrintable => self.config.colors.hex_printable,
        }
    }
}
