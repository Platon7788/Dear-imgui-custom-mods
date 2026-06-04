//! Typed-character input for [`CodeEditor`] — the `read_input_chars`
//! loop with hex transforms, auto-skip closing pairs, auto-close brackets
//! / quotes, and per-line/length limits. Split out of input.rs.

use super::*;

impl CodeEditor {
    /// Handle characters typed this frame (called from `handle_keyboard`
    /// only when the editor is focused and writable).
    pub(super) fn handle_text_input(&mut self) {
        // ── Text input (typed characters) ───────────────────────
        let input_chars = read_input_chars();
        for raw_ch in input_chars {
            if raw_ch < ' ' || raw_ch == '\x7f' {
                continue;
            }

            // Enforce max_line_length limit
            if self.config.max_line_length > 0 {
                let cur = self.buffer.cursor();
                let line_len = self.buffer.line(cur.line).chars().count();
                if line_len >= self.config.max_line_length {
                    continue;
                }
            }

            // ── Hex input transforms ─────────────────────────────
            let ch = if self.config.hex_auto_uppercase && raw_ch.is_ascii_hexdigit() {
                raw_ch.to_ascii_uppercase()
            } else {
                raw_ch
            };

            // ── Auto-skip: check BEFORE inserting ────────────────
            // If the typed character is a closing bracket or quote and
            // the character at the cursor is the same, just skip past
            // it instead of inserting a duplicate.
            //
            // Guard with `selection().is_none_or(is_empty)`: when a
            // selection is active, typing ANY character must REPLACE the
            // selection (standard editor semantics). Without this guard,
            // typing a `)` that happened to match the char at the cursor
            // would silently move past it and DISCARD the selected text
            // instead of overwriting it.
            let is_closing = is_closing_bracket(ch) || is_closing_quote(ch);
            let has_active_sel = self.buffer.selection().is_some_and(|s| !s.is_empty());
            if is_closing && !has_active_sel {
                let line = self.buffer.line(self.buffer.cursor().line);
                let col = self.buffer.cursor().col;
                let next_ch = line.chars().nth(col);
                if next_ch == Some(ch) {
                    self.buffer.move_right();
                    self.reset_blink();
                    continue; // skip normal insert + auto-close
                }
            }

            // ── Normal insert ────────────────────────────────────
            self.snapshot_undo(false);
            if self.buffer.has_extra_cursors() {
                self.buffer.multi_insert_char(ch);
                self.invalidate_token_cache_all();
            } else {
                self.buffer.insert_char(ch);
                self.invalidate_token_cache_at(self.buffer.cursor().line);

                // Auto-space: after 2 consecutive hex digits insert a
                // separator space.
                //
                // The decision matrix is intentionally narrow so we neither
                // duplicate an existing separator nor silently merge two
                // hex runs:
                //
                //   next char | action     | why
                //   ----------+------------+------------------------------
                //   <EOL>     | insert ' ' | fresh byte at the end — common
                //   ' ' / \t  | skip       | separator already there
                //   hex (0-9  | skip       | merging would corrupt the byte
                //     a-f A-F)|            | boundary the user expects
                //   other     | insert ' ' | non-hex separators (`|` `;` `,`)
                //                           still warrant a visual break
                //
                // This fixes the "replace 2nd nibble → double space" bug
                // (next was ' ' and old code inserted another ' ').
                if self.config.hex_auto_space && ch.is_ascii_hexdigit() {
                    let line_idx = self.buffer.cursor().line;
                    let col = self.buffer.cursor().col;
                    // Compute everything on a borrowed `&str` so the
                    // hot path stays alloc-free. The two `String`s the
                    // old code allocated (`.to_string()` + `.collect()`)
                    // ran on every keypress while typing hex.
                    let (nibbles_before, needs_space) = {
                        let line = self.buffer.line(line_idx);
                        let byte_idx: usize = line.chars().take(col).map(|c| c.len_utf8()).sum();
                        let nibbles = line[..byte_idx]
                            .chars()
                            .rev()
                            .take_while(|c| c.is_ascii_hexdigit())
                            .count();
                        (nibbles, hex_auto_space_needed(line, col))
                    };
                    if nibbles_before == 2 && needs_space {
                        self.buffer.insert_char(' ');
                        self.invalidate_token_cache_at(line_idx);
                    }
                }
            }

            // ── Auto-close brackets ──────────────────────────────
            if self.config.auto_close_brackets
                && let Some(close) = closing_bracket(ch)
            {
                self.buffer.insert_char(close);
                self.buffer.move_left();
            }

            // ── Auto-close quotes ────────────────────────────────
            if self.config.auto_close_quotes
                && let Some(close) = closing_quote(ch)
            {
                let line = self.buffer.line(self.buffer.cursor().line);
                let col = self.buffer.cursor().col;
                // Don't auto-close if preceded by a backslash (escape)
                let is_escaped = col >= 2 && line.chars().nth(col - 2) == Some('\\');
                if !is_escaped {
                    self.buffer.insert_char(close);
                    self.buffer.move_left();
                }
            }

            self.reset_blink();
        }
    }
}
