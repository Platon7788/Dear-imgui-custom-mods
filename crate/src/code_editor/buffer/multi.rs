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

    /// Clamp every extra cursor to a valid in-bounds position.
    ///
    /// Defensive backstop: single-cursor structural edits (Enter, delete-line,
    /// move-line, duplicate-line) don't reconcile the extra cursors, so a
    /// stale extra could otherwise point past the end of the buffer and panic
    /// the renderer when it indexes that line. Cheap — extras are few.
    pub fn clamp_extra_cursors(&mut self) {
        for i in 0..self.extra_cursors.len() {
            self.extra_cursors[i] = self.clamp_pos(self.extra_cursors[i]);
        }
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

    /// Insert a character at every cursor (primary + extras).
    pub fn multi_insert_char(&mut self, ch: char) {
        // A typed char is always +1 in global-offset space — a newline never
        // reaches here (Enter routes through the single-cursor path).
        self.multi_edit(|b| {
            b.insert_char(ch);
            1
        });
    }

    /// Backspace at every cursor.
    pub fn multi_backspace(&mut self) {
        self.multi_edit(|b| {
            // Backspace at the very start of the document is a no-op.
            let no_op = b.cursor.line == 0 && b.cursor.col == 0;
            b.backspace();
            if no_op { 0 } else { -1 }
        });
    }

    /// Delete-forward at every cursor.
    pub fn multi_delete(&mut self) {
        self.multi_edit(|b| {
            // Delete at the very end of the document is a no-op.
            let at_doc_end = b.cursor.col >= b.line_char_count(b.cursor.line)
                && b.cursor.line + 1 >= b.lines.len();
            b.delete();
            if at_doc_end { 0 } else { -1 }
        });
    }

    /// Apply a single-cursor `edit` at every cursor (primary + extras),
    /// keeping all positions correct across structural changes.
    ///
    /// Positions are tracked as **global character offsets** — the document
    /// viewed as its lines joined by `\n` — so a line merge or split is a
    /// uniform ±1-char change instead of a special line-index reshuffle (the
    /// source of the previous out-of-bounds / same-line-drift bugs). Edits run
    /// bottom-up so an edit never disturbs a not-yet-processed (lower) cursor;
    /// each already-processed (higher) offset is then shifted by the deltas of
    /// the edits applied below it. `edit` returns its net char-count delta.
    fn multi_edit(&mut self, mut edit: impl FnMut(&mut Self) -> isize) {
        let sorted = self.all_cursors_sorted();
        if sorted.len() <= 1 {
            self.selection = None;
            edit(self);
            return;
        }
        let primary_idx = sorted.iter().position(|c| *c == self.cursor).unwrap_or(0);
        let offsets: Vec<usize> = sorted.iter().map(|&p| self.pos_to_offset(p)).collect();
        let n = offsets.len();

        let mut result_raw = vec![0usize; n];
        let mut deltas = vec![0isize; n];
        for i in (0..n).rev() {
            self.cursor = self.offset_to_pos(offsets[i]);
            self.selection = None;
            deltas[i] = edit(self);
            result_raw[i] = self.pos_to_offset(self.cursor);
        }

        // Final offset = raw result + sum of the deltas from edits applied
        // *after* it (those at lower offsets → indices `< i`).
        let mut final_positions: Vec<CursorPos> = Vec::with_capacity(n);
        let mut prefix = 0isize;
        for i in 0..n {
            let off = (result_raw[i] as isize + prefix).max(0) as usize;
            final_positions.push(self.offset_to_pos(off));
            prefix += deltas[i];
        }

        self.cursor = final_positions[primary_idx];
        self.extra_cursors.clear();
        self.extra_selections.clear();
        let mut seen = vec![self.cursor];
        for (i, &p) in final_positions.iter().enumerate() {
            if i == primary_idx || seen.contains(&p) {
                continue;
            }
            seen.push(p);
            self.extra_cursors.push(p);
            self.extra_selections.push(None);
        }
    }

    /// Global character offset of `pos` (document = lines joined by `\n`).
    /// Clamps out-of-range input so it can never panic on a stale cursor.
    fn pos_to_offset(&self, pos: CursorPos) -> usize {
        let line = pos.line.min(self.lines.len().saturating_sub(1));
        let mut off = 0usize;
        for l in 0..line {
            off += self.line_char_count(l) + 1; // +1 for the newline
        }
        off + pos.col.min(self.line_char_count(line))
    }

    /// Inverse of [`pos_to_offset`](Self::pos_to_offset) against the current
    /// buffer contents.
    fn offset_to_pos(&self, mut off: usize) -> CursorPos {
        let last = self.lines.len().saturating_sub(1);
        for i in 0..self.lines.len() {
            let len = self.line_char_count(i);
            if off <= len || i == last {
                return CursorPos::new(i, off.min(len));
            }
            off -= len + 1;
        }
        CursorPos::new(0, 0)
    }
}
