//! Cursor navigation: arrows, word movement, home/end, document, page.

use super::{CursorPos, TextBuffer, char_to_byte, is_word_char};

impl TextBuffer {
    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.line_char_count(self.cursor.line);
        }
        self.sticky_col = None;
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        let max_col = self.line_char_count(self.cursor.line);
        if self.cursor.col < max_col {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
        self.sticky_col = None;
    }

    /// Move cursor up by one line.
    pub fn move_up(&mut self) {
        if self.cursor.line > 0 {
            let target_col = self.sticky_col.unwrap_or(self.cursor.col);
            self.cursor.line -= 1;
            self.cursor.col = target_col.min(self.line_char_count(self.cursor.line));
            self.sticky_col = Some(target_col);
        }
    }

    /// Move cursor down by one line.
    pub fn move_down(&mut self) {
        if self.cursor.line + 1 < self.lines.len() {
            let target_col = self.sticky_col.unwrap_or(self.cursor.col);
            self.cursor.line += 1;
            self.cursor.col = target_col.min(self.line_char_count(self.cursor.line));
            self.sticky_col = Some(target_col);
        }
    }

    /// Move to start of line.
    pub fn move_home(&mut self) {
        // Smart home: first press → first non-whitespace, second → col 0
        let line = self.line(self.cursor.line);
        let first_non_ws = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
        self.cursor.col = if self.cursor.col == first_non_ws {
            0
        } else {
            first_non_ws
        };
        self.sticky_col = None;
    }

    /// Move to end of line.
    pub fn move_end(&mut self) {
        self.cursor.col = self.line_char_count(self.cursor.line);
        self.sticky_col = None;
    }

    /// Move to next word boundary.
    pub fn move_word_right(&mut self) {
        let line = self.line(self.cursor.line);
        let line_len = line.chars().count();
        let mut col = self.cursor.col;

        if col >= line_len {
            // Move to next line
            if self.cursor.line + 1 < self.lines.len() {
                self.cursor.line += 1;
                self.cursor.col = 0;
            }
            self.sticky_col = None;
            return;
        }

        // Forward scan from cursor position — `chars().skip(col)` iterates
        // without allocating (no Vec<char> collect).
        let mut iter = line.chars().skip(col).peekable();
        // Skip current word
        while let Some(&c) = iter.peek() {
            if !is_word_char(c) {
                break;
            }
            iter.next();
            col += 1;
        }
        // Skip whitespace / non-word
        while let Some(&c) = iter.peek() {
            if is_word_char(c) {
                break;
            }
            iter.next();
            col += 1;
        }

        self.cursor.col = col;
        self.sticky_col = None;
    }

    /// Move to previous word boundary.
    pub fn move_word_left(&mut self) {
        let line = self.line(self.cursor.line);
        let mut col = self.cursor.col;

        if col == 0 {
            // Move to end of previous line
            if self.cursor.line > 0 {
                self.cursor.line -= 1;
                self.cursor.col = self.line_char_count(self.cursor.line);
            }
            self.sticky_col = None;
            return;
        }

        // Reverse scan over the prefix `line[..byte_offset_of_col]`.
        // `Chars` is DoubleEndedIterator — no Vec<char> allocation required.
        // We slice by byte offset first to bound the rev() iterator.
        let byte_col = char_to_byte(line, col);
        let mut iter = line[..byte_col].chars().rev().peekable();
        // Back up over whitespace / non-word
        while let Some(&c) = iter.peek() {
            if is_word_char(c) {
                break;
            }
            iter.next();
            col -= 1;
        }
        // Back up over word
        while let Some(&c) = iter.peek() {
            if !is_word_char(c) {
                break;
            }
            iter.next();
            col -= 1;
        }
        self.cursor.col = col;
        self.sticky_col = None;
    }

    /// Move cursor to start of document.
    pub fn move_doc_start(&mut self) {
        self.cursor = CursorPos::default();
        self.sticky_col = None;
    }

    /// Move cursor to end of document.
    pub fn move_doc_end(&mut self) {
        self.cursor.line = self.lines.len().saturating_sub(1);
        self.cursor.col = self.line_char_count(self.cursor.line);
        self.sticky_col = None;
    }

    /// Move cursor to a specific line (0-based), column 0.
    pub fn goto_line(&mut self, line: usize) {
        self.cursor.line = line.min(self.lines.len().saturating_sub(1));
        self.cursor.col = 0;
        self.selection = None;
        self.sticky_col = None;
    }

    /// Page up/down movement.
    pub fn move_page(&mut self, lines: isize) {
        let target_col = self.sticky_col.unwrap_or(self.cursor.col);
        if lines < 0 {
            self.cursor.line = self.cursor.line.saturating_sub(lines.unsigned_abs());
        } else {
            self.cursor.line =
                (self.cursor.line + lines as usize).min(self.lines.len().saturating_sub(1));
        }
        self.cursor.col = target_col.min(self.line_char_count(self.cursor.line));
        self.sticky_col = Some(target_col);
    }
}
