//! Undo/redo stack with action grouping.
//!
//! Groups consecutive character inserts/deletes into single undo entries,
//! so that typing "hello" produces one undo step, not five.

use std::collections::VecDeque;

use super::buffer::CursorPos;

/// A snapshot of the buffer state for undo/redo.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// Full text content before the edit.
    pub text: String,
    /// Cursor position before the edit.
    pub cursor: CursorPos,
    /// Edit version at the time of snapshot.
    pub version: u64,
}

/// Undo/redo manager with grouping.
pub struct UndoStack {
    undo: VecDeque<UndoEntry>,
    redo: Vec<UndoEntry>,
    max_entries: usize,
    /// Last edit version that was pushed — used for grouping.
    last_push_version: u64,
    /// Minimum edits between undo snapshots for char-by-char typing.
    group_threshold: u64,
}

impl UndoStack {
    pub fn new(max_entries: usize) -> Self {
        Self {
            undo: VecDeque::with_capacity(64),
            redo: Vec::with_capacity(32),
            max_entries,
            last_push_version: 0,
            group_threshold: 1,
        }
    }

    /// Whether `push(entry {version}, force)` would actually store the entry.
    ///
    /// Callers use this to skip the expensive snapshot-build (full buffer
    /// text allocation) when consecutive char edits will just be grouped
    /// into the existing snapshot anyway. On a 1 MB buffer, `buffer.text()`
    /// is O(n) per keystroke — this check turns N keystrokes into a single
    /// allocation.
    pub fn should_push(&self, version: u64, force: bool) -> bool {
        force || version == 0 || version - self.last_push_version > self.group_threshold
    }

    /// Push a snapshot before an edit. Consecutive char edits are grouped.
    ///
    /// `force` = true means always push (used for newline, delete selection, paste).
    pub fn push(&mut self, entry: UndoEntry, force: bool) {
        let version = entry.version;
        // Group consecutive single-char edits
        if !force && version > 0 && version - self.last_push_version <= self.group_threshold {
            // Don't push — the previous snapshot still covers this edit
            self.last_push_version = version;
            // Still clear redo on new edits
            self.redo.clear();
            return;
        }

        self.undo.push_back(entry);
        self.last_push_version = version;
        self.redo.clear();

        // Evict oldest if over limit — O(1) with VecDeque
        if self.undo.len() > self.max_entries {
            self.undo.pop_front();
        }
    }

    /// Force a snapshot (breaks any current grouping).
    pub fn force_snapshot(&mut self, entry: UndoEntry) {
        self.push(entry, true);
    }

    /// Pop and return the last undo entry, pushing current state to redo.
    pub fn undo(&mut self, current: UndoEntry) -> Option<UndoEntry> {
        let entry = self.undo.pop_back()?;
        self.redo.push(current);
        self.last_push_version = entry.version;
        Some(entry)
    }

    /// Pop and return the last redo entry, pushing current state to undo.
    pub fn redo(&mut self, current: UndoEntry) -> Option<UndoEntry> {
        let entry = self.redo.pop()?;
        self.undo.push_back(current);
        self.last_push_version = entry.version;
        Some(entry)
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_push_version = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, version: u64) -> UndoEntry {
        UndoEntry {
            text: text.to_string(),
            cursor: CursorPos::default(),
            version,
        }
    }

    #[test]
    fn test_basic_undo_redo() {
        let mut stack = UndoStack::new(100);
        stack.force_snapshot(entry("a", 0));
        stack.force_snapshot(entry("ab", 1));
        assert!(stack.can_undo());

        let restored = stack.undo(entry("abc", 2)).unwrap();
        assert_eq!(restored.text, "ab");
        assert!(stack.can_redo());

        let redone = stack.redo(entry("ab", 1)).unwrap();
        assert_eq!(redone.text, "abc");
    }

    #[test]
    fn test_grouping() {
        let mut stack = UndoStack::new(100);
        stack.force_snapshot(entry("", 0));
        // Consecutive char edits should be grouped
        stack.push(entry("a", 1), false);
        stack.push(entry("ab", 2), false);
        stack.push(entry("abc", 3), false);
        // Only initial snapshot should exist — consecutive edits are grouped
        assert_eq!(stack.undo.len(), 1); // ""
    }

    #[test]
    fn test_redo_cleared_on_edit() {
        let mut stack = UndoStack::new(100);
        stack.force_snapshot(entry("a", 0));
        stack.force_snapshot(entry("ab", 1));
        stack.undo(entry("abc", 2));
        assert!(stack.can_redo());

        // New edit clears redo
        stack.force_snapshot(entry("ax", 3));
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_max_entries() {
        let mut stack = UndoStack::new(3);
        for i in 0..10 {
            stack.force_snapshot(entry(&format!("v{i}"), i));
        }
        assert!(stack.undo.len() <= 3);
    }
}
