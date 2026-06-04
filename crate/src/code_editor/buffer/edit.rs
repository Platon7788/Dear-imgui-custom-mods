//! Text editing: insert / delete / indent / line operations.

use std::ops::Range;

use super::{TextBuffer, char_to_byte};

impl TextBuffer {
    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection_impl();
        let line = &self.lines[self.cursor.line];
        let byte_idx = char_to_byte(line, self.cursor.col);
        let mut new_line = line[..byte_idx].to_string();
        new_line.push(ch);
        new_line.push_str(&line[byte_idx..]);
        self.lines[self.cursor.line] = new_line;
        self.cursor.col += 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Insert a string at cursor position (handles newlines).
    ///
    /// Normalises CRLF → LF on entry: clipboard pastes from Windows
    /// hosts arrive as `\r\n`, but the buffer stores lines without
    /// terminator (the line-ending style is tracked separately in
    /// [`line_ending`](Self::line_ending) and re-applied by
    /// [`text`](Self::text)). Without normalisation a stray `\r` ends
    /// up at the end of every pasted line.
    pub fn insert_text(&mut self, text: &str) {
        let normalised = if text.contains("\r\n") {
            std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
        } else {
            std::borrow::Cow::Borrowed(text)
        };
        let text = normalised.as_ref();

        self.delete_selection_impl();
        let pos = self.cursor;

        let line = &self.lines[pos.line];
        let byte_idx = char_to_byte(line, pos.col);
        let before = line[..byte_idx].to_string();
        let after = line[byte_idx..].to_string();

        let insert_lines: Vec<&str> = text.split('\n').collect();
        if insert_lines.len() == 1 {
            // Single line insert
            let mut new_line = before;
            new_line.push_str(insert_lines[0]);
            let new_col = new_line.chars().count();
            new_line.push_str(&after);
            self.lines[pos.line] = new_line;
            self.cursor.col = new_col;
        } else {
            // Multi-line insert
            let first = format!("{}{}", before, insert_lines[0]);
            let last = format!("{}{}", insert_lines[insert_lines.len() - 1], after);

            self.lines[pos.line] = first;
            for (j, &mid) in insert_lines[1..insert_lines.len() - 1].iter().enumerate() {
                self.lines.insert(pos.line + 1 + j, mid.to_string());
            }
            self.lines
                .insert(pos.line + insert_lines.len() - 1, last.clone());
            self.cursor.line = pos.line + insert_lines.len() - 1;
            self.cursor.col = insert_lines[insert_lines.len() - 1].chars().count();
        }

        self.modified = true;
        self.edit_version += 1;
        self.sticky_col = None;
    }

    /// Insert a newline at cursor (Enter key). Handles auto-indent.
    pub fn insert_newline(&mut self, auto_indent: bool, tab_size: u8) {
        self.delete_selection_impl();
        let pos = self.cursor;
        let line = &self.lines[pos.line];
        let byte_idx = char_to_byte(line, pos.col);
        let before = line[..byte_idx].to_string();
        let after = line[byte_idx..].to_string();

        // Compute indent
        let mut indent = String::new();
        if auto_indent {
            // Copy leading whitespace from current line
            for ch in self.lines[pos.line].chars() {
                if ch == ' ' || ch == '\t' {
                    indent.push(ch);
                } else {
                    break;
                }
            }
            // Extra indent if line ends with `{`
            if before.trim_end().ends_with('{') {
                for _ in 0..tab_size {
                    indent.push(' ');
                }
            }
        }

        let new_line = format!("{indent}{after}");
        let new_col = indent.chars().count();
        self.lines[pos.line] = before;
        self.lines.insert(pos.line + 1, new_line);
        self.cursor.line = pos.line + 1;
        self.cursor.col = new_col;
        self.modified = true;
        self.edit_version += 1;
        self.sticky_col = None;
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.delete_selection_impl() {
            return;
        }
        let pos = self.cursor;
        if pos.col > 0 {
            let line = &self.lines[pos.line];
            let byte_idx = char_to_byte(line, pos.col);
            let prev_byte = char_to_byte(line, pos.col - 1);
            let mut new_line = line[..prev_byte].to_string();
            new_line.push_str(&line[byte_idx..]);
            self.lines[pos.line] = new_line;
            self.cursor.col -= 1;
            self.modified = true;
            self.edit_version += 1;
        } else if pos.line > 0 {
            // Merge with previous line
            let current = self.lines.remove(pos.line);
            let prev_len = self.line_char_count(pos.line - 1);
            self.lines[pos.line - 1].push_str(&current);
            self.cursor.line -= 1;
            self.cursor.col = prev_len;
            self.modified = true;
            self.edit_version += 1;
        }
    }

    /// Delete character at cursor (Delete key).
    pub fn delete(&mut self) {
        if self.delete_selection_impl() {
            return;
        }
        let pos = self.cursor;
        let max_col = self.line_char_count(pos.line);
        if pos.col < max_col {
            let line = &self.lines[pos.line];
            let byte_idx = char_to_byte(line, pos.col);
            let next_byte = char_to_byte(line, pos.col + 1);
            let mut new_line = line[..byte_idx].to_string();
            new_line.push_str(&line[next_byte..]);
            self.lines[pos.line] = new_line;
            self.modified = true;
            self.edit_version += 1;
        } else if pos.line + 1 < self.lines.len() {
            // Merge next line into current
            let next = self.lines.remove(pos.line + 1);
            self.lines[pos.line].push_str(&next);
            self.modified = true;
            self.edit_version += 1;
        }
    }

    /// Delete the current selection (if any). Returns true if something was deleted.
    pub(super) fn delete_selection_impl(&mut self) -> bool {
        let sel = match self.selection {
            Some(s) if !s.is_empty() => s,
            _ => return false,
        };
        let (start, end) = sel.ordered();

        if start.line == end.line {
            let line = &self.lines[start.line];
            let s = char_to_byte(line, start.col);
            let e = char_to_byte(line, end.col);
            let mut new_line = line[..s].to_string();
            new_line.push_str(&line[e..]);
            self.lines[start.line] = new_line;
        } else {
            let first = &self.lines[start.line];
            let last = &self.lines[end.line];
            let s = char_to_byte(first, start.col);
            let e = char_to_byte(last, end.col);
            let merged = format!("{}{}", &first[..s], &last[e..]);
            self.lines[start.line] = merged;
            // Remove lines between start+1..=end
            for _ in 0..(end.line - start.line) {
                self.lines.remove(start.line + 1);
            }
        }

        self.cursor = start;
        self.selection = None;
        self.modified = true;
        self.edit_version += 1;
        self.sticky_col = None;
        true
    }

    /// Delete word before cursor (Ctrl+Backspace).
    pub fn delete_word_left(&mut self) {
        if self.selection.is_some() {
            self.backspace();
            return;
        }
        let start = self.cursor;
        self.move_word_left();
        let end = self.cursor;
        if start != end {
            self.set_selection(start, end);
            self.delete_selection_impl();
        }
    }

    /// Delete word after cursor (Ctrl+Delete).
    pub fn delete_word_right(&mut self) {
        if self.selection.is_some() {
            self.delete();
            return;
        }
        let start = self.cursor;
        self.move_word_right();
        let end = self.cursor;
        if start != end {
            self.set_selection(start, end);
            self.delete_selection_impl();
        }
    }

    /// Indent selected lines (Tab).
    pub fn indent_lines(&mut self, range: Range<usize>, tab_size: u8, use_spaces: bool) {
        let indent: String = if use_spaces {
            " ".repeat(tab_size as usize)
        } else {
            "\t".to_string()
        };
        for i in range {
            if i < self.lines.len() {
                self.lines[i] = format!("{indent}{}", self.lines[i]);
            }
        }
        self.modified = true;
        self.edit_version += 1;
    }

    /// Unindent selected lines (Shift+Tab).
    pub fn unindent_lines(&mut self, range: Range<usize>, tab_size: u8) {
        for i in range {
            if i >= self.lines.len() {
                continue;
            }
            let line = &self.lines[i];
            let mut remove = 0usize;
            for ch in line.chars() {
                if ch == '\t' && remove == 0 {
                    remove = 1;
                    break;
                } else if ch == ' ' && remove < tab_size as usize {
                    remove += 1;
                } else {
                    break;
                }
            }
            if remove > 0 {
                self.lines[i] = self.lines[i][remove..].to_string();
            }
        }
        self.modified = true;
        self.edit_version += 1;
    }

    // ── Line operations ──────────────────────────────────────────────────

    /// Duplicate the current line (Ctrl+Shift+D).
    pub fn duplicate_line(&mut self) {
        let line = self.cursor.line;
        let content = self.lines[line].clone();
        self.lines.insert(line + 1, content);
        self.cursor.line += 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Move the current line up (Alt+Up).
    pub fn move_line_up(&mut self) {
        let line = self.cursor.line;
        if line == 0 {
            return;
        }
        self.lines.swap(line, line - 1);
        self.cursor.line -= 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Move the current line down (Alt+Down).
    pub fn move_line_down(&mut self) {
        let line = self.cursor.line;
        if line + 1 >= self.lines.len() {
            return;
        }
        self.lines.swap(line, line + 1);
        self.cursor.line += 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Toggle line comment for a range of lines (Ctrl+/).
    pub fn toggle_line_comment(&mut self, range: Range<usize>) {
        // Check if ALL lines in range are commented
        let all_commented = range.clone().all(|i| {
            if i >= self.lines.len() {
                return false;
            }
            self.lines[i].trim_start().starts_with("//")
        });

        for i in range {
            if i >= self.lines.len() {
                continue;
            }
            if all_commented {
                // Remove comment prefix — but only if the `//` we remove is
                // the one that STARTS the line's non-whitespace content.
                // `line.find("//")` without this guard strips `//` from
                // inside strings (`let s = "a//b";` would lose the inner
                // marker) or from inside other `//` comments.
                let line = &self.lines[i];
                let indent_len = line.bytes().take_while(|b| b.is_ascii_whitespace()).count();
                if line[indent_len..].starts_with("//") {
                    let mut new_line = line[..indent_len].to_string();
                    let after = &line[indent_len + 2..];
                    // Remove one space after // if present (matches the
                    // insert path below which writes "// rest").
                    if let Some(stripped) = after.strip_prefix(' ') {
                        new_line.push_str(stripped);
                    } else {
                        new_line.push_str(after);
                    }
                    self.lines[i] = new_line;
                }
            } else {
                // Add comment prefix
                let indent_len = self.lines[i]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .count();
                let indent: String = self.lines[i].chars().take(indent_len).collect();
                let rest: String = self.lines[i].chars().skip(indent_len).collect();
                self.lines[i] = format!("{indent}// {rest}");
            }
        }
        self.modified = true;
        self.edit_version += 1;
    }

    /// Delete the entire current line (Ctrl+Shift+K).
    pub fn delete_line(&mut self) {
        let line = self.cursor.line;
        if self.lines.len() > 1 {
            self.lines.remove(line);
            if self.cursor.line >= self.lines.len() {
                self.cursor.line = self.lines.len() - 1;
            }
            self.cursor.col = self.cursor.col.min(self.line_char_count(self.cursor.line));
        } else {
            self.lines[0] = String::new();
            self.cursor.col = 0;
        }
        self.modified = true;
        self.edit_version += 1;
    }
}
