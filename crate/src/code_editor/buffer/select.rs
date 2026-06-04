//! Selection helpers (select all / line / word) and bracket matching.

use super::{CursorPos, Selection, TextBuffer, char_to_byte, is_word_char};

impl TextBuffer {
    /// Select all text.
    pub fn select_all(&mut self) {
        let start = CursorPos::default();
        let end_line = self.lines.len().saturating_sub(1);
        let end_col = self.line_char_count(end_line);
        let end = CursorPos::new(end_line, end_col);
        self.selection = Some(Selection {
            anchor: start,
            cursor: end,
        });
        self.cursor = end;
    }

    /// Select the entire line at cursor.
    pub fn select_line(&mut self) {
        let line = self.cursor.line;
        let start = CursorPos::new(line, 0);
        let end_col = self.line_char_count(line);
        let end = CursorPos::new(line, end_col);
        self.selection = Some(Selection {
            anchor: start,
            cursor: end,
        });
        self.cursor = end;
    }

    /// Select word at cursor position.
    pub fn select_word_at_cursor(&mut self) {
        let line = self.line(self.cursor.line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.col;

        if col >= chars.len() {
            return;
        }

        let mut start = col;
        let mut end = col;

        if is_word_char(chars[col]) {
            while start > 0 && is_word_char(chars[start - 1]) {
                start -= 1;
            }
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        }

        self.selection = Some(Selection {
            anchor: CursorPos::new(self.cursor.line, start),
            cursor: CursorPos::new(self.cursor.line, end),
        });
        self.cursor.col = end;
    }

    // ── Bracket matching ─────────────────────────────────────────────────

    /// Find the matching bracket for the character at cursor.
    /// Returns `Some((line, col))` if found.
    ///
    /// Walks forward or backward across lines using iterator adapters —
    /// no per-line `Vec<char>` allocation. Brackets are all ASCII so
    /// `chars()` decoding is single-byte; `rev()` works via `Chars`'s
    /// DoubleEndedIterator impl.
    pub fn find_matching_bracket(&self) -> Option<CursorPos> {
        let line = self.line(self.cursor.line);
        let col = self.cursor.col;
        let line_len = line.chars().count();
        if col >= line_len {
            return None;
        }

        // Get the bracket char at col via one chars().nth — cheaper than
        // a full Vec<char> allocation when the char turns out not to be a
        // bracket (common case on cursor moves).
        let ch = line.chars().nth(col)?;
        let (open, close, forward) = match ch {
            '(' => ('(', ')', true),
            ')' => ('(', ')', false),
            '{' => ('{', '}', true),
            '}' => ('{', '}', false),
            '[' => ('[', ']', true),
            ']' => ('[', ']', false),
            _ => return None,
        };

        let mut depth = 0i32;
        if forward {
            let mut l = self.cursor.line;
            let mut c = col;
            while l < self.lines.len() {
                let line = self.line(l);
                for (i, ch) in line.chars().enumerate().skip(c) {
                    if ch == open {
                        depth += 1;
                    }
                    if ch == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some(CursorPos::new(l, i));
                        }
                    }
                }
                l += 1;
                c = 0;
            }
        } else {
            let mut l = self.cursor.line;
            let mut c = col;
            loop {
                let line = self.line(l);
                let line_len = line.chars().count();
                // Backward walk: iterate chars in reverse starting from
                // byte offset of `c`, inclusive.
                let byte_end = char_to_byte(line, (c + 1).min(line_len));
                let prefix = &line[..byte_end];
                // We need the char position relative to the full line —
                // count down from `c`.
                let mut pos = c;
                for ch in prefix.chars().rev() {
                    if ch == close {
                        depth += 1;
                    }
                    if ch == open {
                        depth -= 1;
                        if depth == 0 {
                            return Some(CursorPos::new(l, pos));
                        }
                    }
                    if pos == 0 {
                        break;
                    }
                    pos -= 1;
                }
                if l == 0 {
                    break;
                }
                l -= 1;
                c = self.line_char_count(l).saturating_sub(1);
            }
        }
        None
    }
}
