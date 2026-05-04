//! Undo/redo stack for hex_viewer edits.

// ── Undo Entry ──────────────────────────────────────────────────────────────

/// A single undo-able edit operation.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// Byte offset where the edit happened.
    pub offset: u64,
    /// Old byte values before the edit.
    pub old_bytes: Vec<u8>,
    /// New byte values after the edit.
    pub new_bytes: Vec<u8>,
}

/// Undo/redo stack with configurable depth.
#[derive(Debug, Clone)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
    /// Points to the next undo position (entries[pos-1] is last applied).
    pos: usize,
    /// Maximum stack depth (0 = unlimited).
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            entries: Vec::new(),
            pos: 0,
            max_depth,
        }
    }

    /// Record an edit. Truncates any redo history.
    pub fn push(&mut self, entry: UndoEntry) {
        self.entries.truncate(self.pos);
        self.entries.push(entry);
        self.pos = self.entries.len();
        // Trim oldest if over capacity.
        if self.max_depth > 0 && self.entries.len() > self.max_depth {
            let remove = self.entries.len() - self.max_depth;
            self.entries.drain(..remove);
            self.pos = self.entries.len();
        }
    }

    /// Undo the last edit. Returns the entry to reverse, or `None`.
    pub fn undo(&mut self) -> Option<&UndoEntry> {
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        Some(&self.entries[self.pos])
    }

    /// Redo the next edit. Returns the entry to re-apply, or `None`.
    pub fn redo(&mut self) -> Option<&UndoEntry> {
        if self.pos >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[self.pos];
        self.pos += 1;
        Some(entry)
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.pos > 0
    }
    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.pos < self.entries.len()
    }
    /// Number of undo steps available.
    pub fn undo_count(&self) -> usize {
        self.pos
    }
    /// Number of redo steps available.
    pub fn redo_count(&self) -> usize {
        self.entries.len() - self.pos
    }
    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pos = 0;
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(256)
    }
}
