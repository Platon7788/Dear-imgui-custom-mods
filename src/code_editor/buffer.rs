//! Text buffer with cursor, selection, and modification tracking.

use std::ops::Range;

/// Position in the text buffer (line + column in chars, not bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPos {
    pub line: usize,
    pub col: usize,
}

impl CursorPos {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl PartialOrd for CursorPos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CursorPos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

/// A text selection defined by anchor (where selection started) and cursor (where it ends).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: CursorPos,
    pub cursor: CursorPos,
}

impl Selection {
    /// Returns (start, end) in document order.
    pub fn ordered(&self) -> (CursorPos, CursorPos) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// Whether this selection covers zero characters.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

/// The text buffer — stores lines, cursor, selection, and dirty state.
pub struct TextBuffer {
    lines: Vec<String>,
    cursor: CursorPos,
    selection: Option<Selection>,
    /// Preferred column when moving up/down (sticky column).
    sticky_col: Option<usize>,
    /// Whether content has been modified since last `clear_modified()`.
    modified: bool,
    /// Total number of edits (used for undo grouping).
    edit_version: u64,
    /// Extra cursors for multi-cursor editing (Ctrl+D / Alt+Click).
    /// The primary cursor is always `self.cursor`; these are additional ones.
    extra_cursors: Vec<CursorPos>,
    /// Selections for each extra cursor (parallel to `extra_cursors`).
    extra_selections: Vec<Option<Selection>>,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: CursorPos::default(),
            selection: None,
            sticky_col: None,
            modified: false,
            edit_version: 0,
            extra_cursors: Vec::new(),
            extra_selections: Vec::new(),
        }
    }
}

impl TextBuffer {
    // ── Getters ──────────────────────────────────────────────────────────

    /// Total number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get line content by index.
    pub fn line(&self, idx: usize) -> &str {
        self.lines.get(idx).map_or("", |s| s.as_str())
    }

    /// Get all lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Current cursor position.
    pub fn cursor(&self) -> CursorPos {
        self.cursor
    }

    /// Current selection (None if no selection active).
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Whether the buffer has been modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Edit version (incremented on each edit).
    pub fn edit_version(&self) -> u64 {
        self.edit_version
    }

    /// Get entire text as a single string.
    ///
    /// Allocates a fresh `String` on every call — for large buffers (> 1 MB)
    /// consider [`text_into`](Self::text_into) to reuse an existing capacity.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Append the entire text into `buf`, reusing existing capacity.
    ///
    /// Callers that poll text every frame (save-on-change watchers, export
    /// dialogs) avoid the per-call heap allocation by keeping a persistent
    /// `String` buffer. `buf` is cleared first.
    pub fn text_into(&self, buf: &mut String) {
        buf.clear();
        let needed = self.lines.iter().map(|l| l.len()).sum::<usize>()
            + self.lines.len().saturating_sub(1);
        if buf.capacity() < needed {
            buf.reserve(needed - buf.capacity());
        }
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                buf.push('\n');
            }
            buf.push_str(line);
        }
    }

    /// Get selected text, or empty string if no selection.
    pub fn selected_text(&self) -> String {
        let sel = match self.selection {
            Some(s) if !s.is_empty() => s,
            _ => return String::new(),
        };
        let (start, end) = sel.ordered();
        if start.line == end.line {
            let line = self.line(start.line);
            let s = char_to_byte(line, start.col);
            let e = char_to_byte(line, end.col);
            return line[s..e].to_string();
        }
        let mut result = String::new();
        // First line
        let first = self.line(start.line);
        let s = char_to_byte(first, start.col);
        result.push_str(&first[s..]);
        // Middle lines
        for i in (start.line + 1)..end.line {
            result.push('\n');
            result.push_str(self.line(i));
        }
        // Last line
        result.push('\n');
        let last = self.line(end.line);
        let e = char_to_byte(last, end.col);
        result.push_str(&last[..e]);
        result
    }

    // ── Setters ──────────────────────────────────────────────────────────

    /// Replace all text (resets cursor, selection, modified flag).
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        // If text ends with newline, add trailing empty line
        if text.ends_with('\n') {
            self.lines.push(String::new());
        }
        self.cursor = CursorPos::default();
        self.selection = None;
        self.sticky_col = None;
        self.modified = false;
        self.edit_version = 0;
        self.extra_cursors.clear();
        self.extra_selections.clear();
    }

    /// Mark buffer as clean.
    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    /// Set cursor position (clamped to valid range).
    pub fn set_cursor(&mut self, pos: CursorPos) {
        self.cursor = self.clamp_pos(pos);
        self.sticky_col = None;
    }

    /// Set cursor and clear selection.
    pub fn set_cursor_clear_sel(&mut self, pos: CursorPos) {
        self.set_cursor(pos);
        self.selection = None;
    }

    /// Start or extend selection.
    pub fn set_selection(&mut self, anchor: CursorPos, cursor: CursorPos) {
        let anchor = self.clamp_pos(anchor);
        let cursor = self.clamp_pos(cursor);
        self.selection = Some(Selection { anchor, cursor });
        self.cursor = cursor;
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    // ── Navigation ───────────────────────────────────────────────────────

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
        self.cursor.col = if self.cursor.col == first_non_ws { 0 } else { first_non_ws };
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
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor.col;

        if col >= chars.len() {
            // Move to next line
            if self.cursor.line + 1 < self.lines.len() {
                self.cursor.line += 1;
                self.cursor.col = 0;
            }
            self.sticky_col = None;
            return;
        }

        // Skip current word
        while col < chars.len() && is_word_char(chars[col]) { col += 1; }
        // Skip whitespace
        while col < chars.len() && !is_word_char(chars[col]) { col += 1; }

        self.cursor.col = col;
        self.sticky_col = None;
    }

    /// Move to previous word boundary.
    pub fn move_word_left(&mut self) {
        let line = self.line(self.cursor.line);
        let chars: Vec<char> = line.chars().collect();
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

        // Back up over whitespace/non-word
        while col > 0 && !is_word_char(chars[col - 1]) { col -= 1; }
        // Back up over word
        while col > 0 && is_word_char(chars[col - 1]) { col -= 1; }

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
            self.cursor.line = (self.cursor.line + lines as usize)
                .min(self.lines.len().saturating_sub(1));
        }
        self.cursor.col = target_col.min(self.line_char_count(self.cursor.line));
        self.sticky_col = Some(target_col);
    }

    // ── Editing ──────────────────────────────────────────────────────────

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
    pub fn insert_text(&mut self, text: &str) {
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
            self.lines.insert(pos.line + insert_lines.len() - 1, last.clone());
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
    fn delete_selection_impl(&mut self) -> bool {
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

    /// Select all text.
    pub fn select_all(&mut self) {
        let start = CursorPos::default();
        let end_line = self.lines.len().saturating_sub(1);
        let end_col = self.line_char_count(end_line);
        let end = CursorPos::new(end_line, end_col);
        self.selection = Some(Selection { anchor: start, cursor: end });
        self.cursor = end;
    }

    /// Select the entire line at cursor.
    pub fn select_line(&mut self) {
        let line = self.cursor.line;
        let start = CursorPos::new(line, 0);
        let end_col = self.line_char_count(line);
        let end = CursorPos::new(line, end_col);
        self.selection = Some(Selection { anchor: start, cursor: end });
        self.cursor = end;
    }

    /// Select word at cursor position.
    pub fn select_word_at_cursor(&mut self) {
        let line = self.line(self.cursor.line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.col;

        if col >= chars.len() { return; }

        let mut start = col;
        let mut end = col;

        if is_word_char(chars[col]) {
            while start > 0 && is_word_char(chars[start - 1]) { start -= 1; }
            while end < chars.len() && is_word_char(chars[end]) { end += 1; }
        }

        self.selection = Some(Selection {
            anchor: CursorPos::new(self.cursor.line, start),
            cursor: CursorPos::new(self.cursor.line, end),
        });
        self.cursor.col = end;
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
            if i >= self.lines.len() { continue; }
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
        if line == 0 { return; }
        self.lines.swap(line, line - 1);
        self.cursor.line -= 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Move the current line down (Alt+Down).
    pub fn move_line_down(&mut self) {
        let line = self.cursor.line;
        if line + 1 >= self.lines.len() { return; }
        self.lines.swap(line, line + 1);
        self.cursor.line += 1;
        self.modified = true;
        self.edit_version += 1;
    }

    /// Toggle line comment for a range of lines (Ctrl+/).
    pub fn toggle_line_comment(&mut self, range: Range<usize>) {
        // Check if ALL lines in range are commented
        let all_commented = range.clone().all(|i| {
            if i >= self.lines.len() { return false; }
            self.lines[i].trim_start().starts_with("//")
        });

        for i in range {
            if i >= self.lines.len() { continue; }
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

    // ── Bracket matching ─────────────────────────────────────────────────

    /// Find the matching bracket for the character at cursor.
    /// Returns `Some((line, col))` if found.
    pub fn find_matching_bracket(&self) -> Option<CursorPos> {
        let line = self.line(self.cursor.line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.col;
        if col >= chars.len() { return None; }

        let ch = chars[col];
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
                let lchars: Vec<char> = self.line(l).chars().collect();
                while c < lchars.len() {
                    if lchars[c] == open { depth += 1; }
                    if lchars[c] == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some(CursorPos::new(l, c));
                        }
                    }
                    c += 1;
                }
                l += 1;
                c = 0;
            }
        } else {
            let mut l = self.cursor.line;
            let mut c = col as isize;
            loop {
                let lchars: Vec<char> = self.line(l).chars().collect();
                while c >= 0 {
                    let cu = c as usize;
                    if lchars[cu] == close { depth += 1; }
                    if lchars[cu] == open {
                        depth -= 1;
                        if depth == 0 {
                            return Some(CursorPos::new(l, cu));
                        }
                    }
                    c -= 1;
                }
                if l == 0 { break; }
                l -= 1;
                c = self.line_char_count(l) as isize - 1;
            }
        }
        None
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn line_char_count(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, |s| s.chars().count())
    }

    fn clamp_pos(&self, pos: CursorPos) -> CursorPos {
        let line = pos.line.min(self.lines.len().saturating_sub(1));
        let col = pos.col.min(self.line_char_count(line));
        CursorPos::new(line, col)
    }

    // ── Multi-cursor ────────────────────────────────────────────────────

    /// Get all extra cursor positions (excluding the primary cursor).
    pub fn extra_cursors(&self) -> &[CursorPos] {
        &self.extra_cursors
    }

    /// Get all extra selections (parallel to `extra_cursors`).
    pub fn extra_selections(&self) -> &[Option<Selection>] {
        &self.extra_selections
    }

    /// Whether multi-cursor mode is active.
    pub fn has_extra_cursors(&self) -> bool {
        !self.extra_cursors.is_empty()
    }

    /// Add an extra cursor at the given position.
    /// Deduplicates if cursor already exists at that position.
    pub fn add_cursor(&mut self, pos: CursorPos) {
        let pos = self.clamp_pos(pos);
        // Don't add if it matches the primary cursor
        if pos == self.cursor {
            return;
        }
        // Don't add duplicates
        if self.extra_cursors.contains(&pos) {
            return;
        }
        self.extra_cursors.push(pos);
        self.extra_selections.push(None);
    }

    /// Add an extra cursor with a selection.
    pub fn add_cursor_with_selection(&mut self, cursor: CursorPos, sel: Selection) {
        let cursor = self.clamp_pos(cursor);
        if cursor == self.cursor {
            return;
        }
        if self.extra_cursors.contains(&cursor) {
            return;
        }
        self.extra_cursors.push(cursor);
        self.extra_selections.push(Some(sel));
    }

    /// Clear all extra cursors, returning to single-cursor mode.
    pub fn clear_extra_cursors(&mut self) {
        self.extra_cursors.clear();
        self.extra_selections.clear();
    }

    /// Get all cursor positions (primary + extras), sorted in document order.
    pub fn all_cursors_sorted(&self) -> Vec<CursorPos> {
        let mut all = vec![self.cursor];
        all.extend_from_slice(&self.extra_cursors);
        all.sort();
        all.dedup();
        all
    }

    /// Find the next occurrence of `needle` after `after_pos` and return its range.
    /// Used for Ctrl+D (select next occurrence).
    pub fn find_next_occurrence(
        &self,
        needle: &str,
        after_pos: CursorPos,
    ) -> Option<(CursorPos, CursorPos)> {
        if needle.is_empty() {
            return None;
        }
        // Search from after_pos to end, then wrap around
        for line_idx in after_pos.line..self.lines.len() {
            let line = &self.lines[line_idx];
            let start_col = if line_idx == after_pos.line {
                char_to_byte(line, after_pos.col)
            } else {
                0
            };
            if let Some(byte_offset) = line[start_col..].find(needle) {
                let match_start = byte_to_char(line, start_col + byte_offset);
                let match_end = match_start + needle.chars().count();
                return Some((
                    CursorPos::new(line_idx, match_start),
                    CursorPos::new(line_idx, match_end),
                ));
            }
        }
        // Wrap around from the beginning
        for line_idx in 0..=after_pos.line.min(self.lines.len().saturating_sub(1)) {
            let line = &self.lines[line_idx];
            let end_byte = if line_idx == after_pos.line {
                char_to_byte(line, after_pos.col)
            } else {
                line.len()
            };
            if let Some(byte_offset) = line[..end_byte].find(needle) {
                let match_start = byte_to_char(line, byte_offset);
                let match_end = match_start + needle.chars().count();
                return Some((
                    CursorPos::new(line_idx, match_start),
                    CursorPos::new(line_idx, match_end),
                ));
            }
        }
        None
    }

    /// Insert a character at all cursors (primary + extras).
    /// Edits in reverse document order to preserve positions.
    pub fn multi_insert_char(&mut self, ch: char) {
        let mut cursors = self.all_cursors_sorted();
        // Reverse to edit from bottom-up
        cursors.reverse();

        // Save primary cursor index
        let primary_idx = cursors.iter().position(|c| *c == self.cursor);

        let mut new_cursors: Vec<CursorPos> = Vec::with_capacity(cursors.len());

        for cursor_pos in &cursors {
            // Temporarily set cursor to this position
            self.cursor = *cursor_pos;
            self.selection = None; // per-cursor selection cleared on insert
            self.insert_char(ch);
            new_cursors.push(self.cursor);
        }

        // Reverse back to document order
        new_cursors.reverse();

        // Restore primary and extra cursors
        if let Some(pi) = primary_idx {
            let doc_idx = cursors.len() - 1 - pi;
            self.cursor = new_cursors[doc_idx];
            self.extra_cursors.clear();
            self.extra_selections.clear();
            for (i, &c) in new_cursors.iter().enumerate() {
                if i != doc_idx {
                    self.extra_cursors.push(c);
                    self.extra_selections.push(None);
                }
            }
        }
    }

    /// Delete (backspace) at all cursors.
    pub fn multi_backspace(&mut self) {
        let mut cursors = self.all_cursors_sorted();
        cursors.reverse();

        let primary_idx = cursors.iter().position(|c| *c == self.cursor);
        let mut new_cursors: Vec<CursorPos> = Vec::with_capacity(cursors.len());

        for cursor_pos in &cursors {
            self.cursor = *cursor_pos;
            self.selection = None;
            self.backspace();
            new_cursors.push(self.cursor);
        }

        new_cursors.reverse();

        if let Some(pi) = primary_idx {
            let doc_idx = cursors.len() - 1 - pi;
            self.cursor = new_cursors[doc_idx];
            self.extra_cursors.clear();
            self.extra_selections.clear();
            for (i, &c) in new_cursors.iter().enumerate() {
                if i != doc_idx {
                    self.extra_cursors.push(c);
                    self.extra_selections.push(None);
                }
            }
        }
    }

    /// Delete at all cursors.
    pub fn multi_delete(&mut self) {
        let mut cursors = self.all_cursors_sorted();
        cursors.reverse();

        let primary_idx = cursors.iter().position(|c| *c == self.cursor);
        let mut new_cursors: Vec<CursorPos> = Vec::with_capacity(cursors.len());

        for cursor_pos in &cursors {
            self.cursor = *cursor_pos;
            self.selection = None;
            self.delete();
            new_cursors.push(self.cursor);
        }

        new_cursors.reverse();

        if let Some(pi) = primary_idx {
            let doc_idx = cursors.len() - 1 - pi;
            self.cursor = new_cursors[doc_idx];
            self.extra_cursors.clear();
            self.extra_selections.clear();
            for (i, &c) in new_cursors.iter().enumerate() {
                if i != doc_idx {
                    self.extra_cursors.push(c);
                    self.extra_selections.push(None);
                }
            }
        }
    }

}

// ── Utility functions ────────────────────────────────────────────────────────

/// Convert char column to byte offset in a string.
pub fn char_to_byte(s: &str, char_col: usize) -> usize {
    s.char_indices()
        .nth(char_col)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

/// Convert byte offset to char column.
pub fn byte_to_char(s: &str, byte_offset: usize) -> usize {
    s[..byte_offset.min(s.len())].chars().count()
}

/// Unicode-aware word-char test — matches `\w` semantics: alphanumeric
/// (including non-ASCII letters like é / ж / 你) plus underscore.
/// Exported for use in editor-level helpers (whole-word find, etc.).
pub(crate) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> TextBuffer {
        let mut b = TextBuffer::default();
        b.set_text(text);
        b
    }

    #[test]
    fn test_set_text() {
        let b = buf("hello\nworld");
        assert_eq!(b.line_count(), 2);
        assert_eq!(b.line(0), "hello");
        assert_eq!(b.line(1), "world");
    }

    #[test]
    fn test_insert_char() {
        let mut b = buf("ab");
        b.set_cursor(CursorPos::new(0, 1));
        b.insert_char('X');
        assert_eq!(b.line(0), "aXb");
        assert_eq!(b.cursor().col, 2);
        assert!(b.is_modified());
    }

    #[test]
    fn test_newline() {
        let mut b = buf("hello world");
        b.set_cursor(CursorPos::new(0, 5));
        b.insert_newline(false, 4);
        assert_eq!(b.line_count(), 2);
        assert_eq!(b.line(0), "hello");
        assert_eq!(b.line(1), " world");
    }

    #[test]
    fn test_auto_indent_after_brace() {
        let mut b = buf("fn main() {");
        b.set_cursor(CursorPos::new(0, 11));
        b.insert_newline(true, 4);
        assert_eq!(b.line(1), "    ");
        assert_eq!(b.cursor().col, 4);
    }

    #[test]
    fn test_backspace() {
        let mut b = buf("abc");
        b.set_cursor(CursorPos::new(0, 2));
        b.backspace();
        assert_eq!(b.line(0), "ac");
    }

    #[test]
    fn test_backspace_merge_lines() {
        let mut b = buf("ab\ncd");
        b.set_cursor(CursorPos::new(1, 0));
        b.backspace();
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.line(0), "abcd");
        assert_eq!(b.cursor(), CursorPos::new(0, 2));
    }

    #[test]
    fn test_delete_forward() {
        let mut b = buf("abc");
        b.set_cursor(CursorPos::new(0, 1));
        b.delete();
        assert_eq!(b.line(0), "ac");
    }

    #[test]
    fn test_selection_and_delete() {
        let mut b = buf("hello world");
        b.set_selection(CursorPos::new(0, 0), CursorPos::new(0, 5));
        b.backspace();
        assert_eq!(b.line(0), " world");
    }

    #[test]
    fn test_selected_text() {
        let b = {
            let mut b = buf("hello\nworld\nfoo");
            b.set_selection(CursorPos::new(0, 3), CursorPos::new(1, 3));
            b
        };
        assert_eq!(b.selected_text(), "lo\nwor");
    }

    #[test]
    fn test_bracket_matching() {
        let b = {
            let mut b = buf("fn foo(bar(baz))");
            b.set_cursor(CursorPos::new(0, 6));
            b
        };
        let m = b.find_matching_bracket();
        assert_eq!(m, Some(CursorPos::new(0, 15)));
    }

    #[test]
    fn test_move_word() {
        let mut b = buf("hello world_foo bar");
        b.set_cursor(CursorPos::new(0, 0));
        b.move_word_right();
        assert_eq!(b.cursor().col, 6); // after "hello "
        b.move_word_right();
        assert_eq!(b.cursor().col, 16); // after "world_foo "
    }

    #[test]
    fn test_smart_home() {
        let mut b = buf("    hello");
        b.set_cursor(CursorPos::new(0, 7));
        b.move_home();
        assert_eq!(b.cursor().col, 4); // first non-whitespace
        b.move_home();
        assert_eq!(b.cursor().col, 0); // absolute start
    }

    #[test]
    fn test_select_all() {
        let mut b = buf("hello\nworld");
        b.select_all();
        assert_eq!(b.selected_text(), "hello\nworld");
    }

    #[test]
    fn test_insert_multiline_text() {
        let mut b = buf("ab");
        b.set_cursor(CursorPos::new(0, 1));
        b.insert_text("X\nY\nZ");
        assert_eq!(b.line_count(), 3);
        assert_eq!(b.line(0), "aX");
        assert_eq!(b.line(1), "Y");
        assert_eq!(b.line(2), "Zb");
    }

    #[test]
    fn test_char_to_byte() {
        assert_eq!(char_to_byte("hello", 2), 2);
        assert_eq!(char_to_byte("hello", 5), 5);
        assert_eq!(char_to_byte("hello", 10), 5); // clamped
    }
}
