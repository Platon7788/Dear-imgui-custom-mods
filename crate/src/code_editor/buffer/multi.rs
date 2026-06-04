//! Multi-cursor editing (Ctrl+D / Alt+Click).

use super::{CursorPos, Selection, TextBuffer, byte_to_char, char_to_byte};

impl TextBuffer {
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
