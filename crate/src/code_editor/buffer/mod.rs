//! Text buffer with cursor, selection, and modification tracking.
//!
//! The buffer is split into cohesive sibling modules to keep each file
//! under the 500-line ceiling (CLAUDE.md). This file owns the type
//! definitions, getters/setters, the free utility functions, and the
//! test suite (so tests can reach private fields); the `impl TextBuffer`
//! method groups live in:
//!
//! - [`nav`]    — cursor movement (arrows, word, home/end, doc, page).
//! - [`edit`]   — insert / delete / indent / line operations.
//! - [`select`] — selection helpers + bracket matching.
//! - [`multi`]  — multi-cursor editing.

mod edit;
mod multi;
mod nav;
mod select;

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

/// Line-ending style detected at load time and preserved on save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// `\n` (Unix, macOS). Default.
    #[default]
    Lf,
    /// `\r\n` (Windows).
    Crlf,
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
    /// Line-ending style — detected on `set_text` from the first `\r\n`
    /// occurrence, preserved on `text()` so round-trip through the editor
    /// doesn't mangle CRLF Windows files into LF.
    line_ending: LineEnding,
    /// Whether the loaded text appeared to use tab indentation (at least
    /// one non-empty line starts with `\t`). Populated on `set_text`.
    /// Downstream editors can consult this to pick the right indent style.
    detected_uses_tabs: bool,
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
            line_ending: LineEnding::default(),
            detected_uses_tabs: false,
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

    /// Line-ending style detected at load time. Preserved by [`text`] and
    /// [`text_into`] so Windows files round-trip cleanly.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Override the detected line ending (e.g. after conversion).
    pub fn set_line_ending(&mut self, ending: LineEnding) {
        self.line_ending = ending;
    }

    /// Whether the loaded text appeared to use tab indentation. Populated
    /// by [`set_text`]. Downstream callers (editor config) consult this
    /// to pick the right indent style.
    pub fn detected_uses_tabs(&self) -> bool {
        self.detected_uses_tabs
    }

    /// Get entire text as a single string.
    ///
    /// Preserves the original line-ending style ([`LineEnding`]). Allocates
    /// a fresh `String` on every call — for large buffers (> 1 MB)
    /// consider [`text_into`](Self::text_into) to reuse an existing capacity.
    pub fn text(&self) -> String {
        let sep = match self.line_ending {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        };
        self.lines.join(sep)
    }

    /// Append the entire text into `buf`, reusing existing capacity.
    ///
    /// Preserves the original line-ending style ([`LineEnding`]). Callers
    /// that poll text every frame (save-on-change watchers, export dialogs)
    /// avoid the per-call heap allocation by keeping a persistent `String`.
    /// `buf` is cleared first.
    pub fn text_into(&self, buf: &mut String) {
        buf.clear();
        let sep_len = match self.line_ending {
            LineEnding::Lf => 1,
            LineEnding::Crlf => 2,
        };
        let needed = self.lines.iter().map(|l| l.len()).sum::<usize>()
            + self.lines.len().saturating_sub(1) * sep_len;
        if buf.capacity() < needed {
            buf.reserve(needed - buf.capacity());
        }
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                match self.line_ending {
                    LineEnding::Lf => buf.push('\n'),
                    LineEnding::Crlf => buf.push_str("\r\n"),
                }
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
    ///
    /// Detects line-ending style (`\r\n` vs `\n`) and whether the file
    /// appears to use tab indentation — consult [`line_ending`] and
    /// [`detected_uses_tabs`] after load.
    pub fn set_text(&mut self, text: &str) {
        // Line-ending detection: first `\r\n` occurrence wins. Cap the
        // scan to the first 64 KB — CRLF files are homogeneous, a single
        // `\r\n` within the first few KB is virtually certain. Without
        // the cap, loading a 100 MB LF-only log file walked the entire
        // buffer.
        const CRLF_SCAN_CAP: usize = 64 * 1024;
        let scan_region = if text.len() > CRLF_SCAN_CAP {
            // Find a safe UTF-8 boundary at or below the cap.
            let mut cut = CRLF_SCAN_CAP;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            &text[..cut]
        } else {
            text
        };
        self.line_ending = if scan_region.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };

        // Tab vs spaces detection: scan first N indented lines. If any
        // non-empty line begins with `\t`, mark as tabs. This is the same
        // heuristic VSCode / Sublime use before falling back to config.
        self.detected_uses_tabs = text.lines().take(256).any(|l| l.starts_with('\t'));

        // `str::lines()` strips both `\n` and `\r\n` — no need to do it
        // ourselves. The line ending is preserved separately in
        // `self.line_ending` and re-applied by `text()`.
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

    /// Restore text from an undo/redo snapshot — preserves `modified` as
    /// **true** and **bumps** `edit_version` instead of resetting them.
    ///
    /// The plain `set_text` resets both fields because it's meant for
    /// "load a fresh document". Undo/redo is a different semantic — the
    /// buffer still differs from disk, and caches keyed by `edit_version`
    /// (wrap / find-lowercase / fold / token) must invalidate, not see
    /// a lower version number than they were built against.
    pub fn restore_from_undo(&mut self, text: &str, cursor: CursorPos) {
        // Detect line-ending consistency (should match what was stored,
        // but stay defensive).
        self.line_ending = if text.contains("\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        self.lines = text.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if text.ends_with('\n') {
            self.lines.push(String::new());
        }
        self.cursor = self.clamp_pos(cursor);
        self.selection = None;
        self.sticky_col = None;
        self.extra_cursors.clear();
        self.extra_selections.clear();
        // Dirty state: undo/redo leaves the buffer differing from the
        // last-saved state, so `modified = true` regardless of the
        // undo direction. Host code reads `is_modified()` for the
        // save-prompt gate — it must not silently go false.
        self.modified = true;
        // Bump version so caches rebuild at their next access.
        self.edit_version = self.edit_version.saturating_add(1);
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

    // ── Helpers (shared across sibling impl modules) ─────────────────────

    pub(super) fn line_char_count(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, |s| s.chars().count())
    }

    pub(super) fn clamp_pos(&self, pos: CursorPos) -> CursorPos {
        let line = pos.line.min(self.lines.len().saturating_sub(1));
        let col = pos.col.min(self.line_char_count(line));
        CursorPos::new(line, col)
    }
}

// ── Utility functions ────────────────────────────────────────────────────────

/// Convert char column to byte offset in a string.
pub fn char_to_byte(s: &str, char_col: usize) -> usize {
    s.char_indices()
        .nth(char_col)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

/// Convert byte offset to char column. An offset that lands inside a
/// multi-byte char snaps down to that char's start rather than panicking,
/// so this stays total for any `byte_offset` (a `pub fn` — callers must
/// not be able to crash it with an unaligned offset).
pub fn byte_to_char(s: &str, byte_offset: usize) -> usize {
    let mut off = byte_offset.min(s.len());
    while off > 0 && !s.is_char_boundary(off) {
        off -= 1;
    }
    s[..off].chars().count()
}

/// Unicode-aware word-char test — matches `\w` semantics: alphanumeric
/// (including non-ASCII letters like é / ж / 你) plus underscore.
/// Exported for use in editor-level helpers (whole-word find, etc.).
pub(crate) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
