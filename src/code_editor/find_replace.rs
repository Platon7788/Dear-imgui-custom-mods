//! Find / Replace state + search logic.
//!
//! Extracted from `mod.rs` to keep the top-level file focused on the
//! editor's state machine + render orchestration. The search algorithm
//! (case-insensitive lowercase cache, whole-word boundary detection,
//! find-in-selection scope) lives here end-to-end.

use super::buffer::{self, CursorPos};

/// Where to search — matches VSCode's find-in-selection semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindScope {
    /// Search the entire document (default).
    #[default]
    All,
    /// Search only within the current selection. If the user has no active
    /// selection when this is set, the mode falls back to `All` silently.
    Selection,
}

/// Find/Replace panel state.
#[derive(Debug, Default)]
pub struct FindReplaceState {
    /// Whether the find panel is open.
    pub open: bool,
    /// Set to true the frame the panel is opened — used to auto-focus the input.
    pub just_opened: bool,
    /// Search query.
    pub query: String,
    /// Replacement text.
    pub replacement: String,
    /// Case-sensitive search.
    pub case_sensitive: bool,
    /// Whole-word search.
    pub whole_word: bool,
    /// Whether the replace field is visible.
    pub show_replace: bool,
    /// Restrict search scope to a selection (VSCode "Find in selection").
    pub scope: FindScope,
    /// All match positions: (line, col_start, col_end) in chars.
    pub matches: Vec<(usize, usize, usize)>,
    /// Current match index (for cycling through matches).
    pub current_match: usize,
    /// Per-line lowercase cache for case-insensitive search. Rebuilt only
    /// when the text `edit_version` changes — eliminates the
    /// `line.to_lowercase()` alloc per line per keystroke of the query,
    /// which on a 10 000-line file dominated Find perf.
    lowercase_cache: Vec<String>,
    /// Edit version the lowercase cache was built against. `u64::MAX` means
    /// invalid / not yet built.
    lowercase_version: u64,
}

impl FindReplaceState {
    /// Invalidate the lowercase cache. Called when text or case-sensitivity
    /// flag changes so the next `update_matches` rebuilds.
    pub(super) fn invalidate_lowercase_cache(&mut self) {
        self.lowercase_version = u64::MAX;
        self.lowercase_cache.clear();
    }

    /// Retained for tests and downstream callers that want whole-document
    /// search without wiring up the scoped variant. Thin wrapper — prefer
    /// [`update_matches_scoped`](Self::update_matches_scoped) for new code.
    #[allow(dead_code)]
    pub(super) fn update_matches(&mut self, lines: &[String], edit_version: u64) {
        self.update_matches_scoped(lines, edit_version, None);
    }

    /// Internal variant that restricts matching to `(start, end)` — inclusive
    /// `(line, col)` bounds in char coordinates. Used for find-in-selection.
    pub(super) fn update_matches_scoped(
        &mut self,
        lines: &[String],
        edit_version: u64,
        bounds: Option<(CursorPos, CursorPos)>,
    ) {
        self.matches.clear();
        if self.query.is_empty() { return; }

        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        // Build (or reuse) the per-line lowercase cache for case-insensitive
        // searches. This is the single biggest source of allocations during
        // a Find operation on large files: without cache, `line.to_lowercase()`
        // runs N_lines × N_keystrokes times.
        if !self.case_sensitive && self.lowercase_version != edit_version {
            self.lowercase_cache.clear();
            self.lowercase_cache.reserve(lines.len());
            for line in lines {
                self.lowercase_cache.push(line.to_lowercase());
            }
            self.lowercase_version = edit_version;
        }

        // Clamp iteration to `bounds` (for find-in-selection). Outside the
        // bounds we skip the line entirely; on boundary lines we filter per-
        // match after the find() returns a byte offset.
        let (first_line, last_line) = match bounds {
            Some((s, e)) => (s.line, e.line),
            None => (0, lines.len().saturating_sub(1)),
        };

        for (line_idx, line) in lines.iter().enumerate() {
            if line_idx < first_line || line_idx > last_line { continue; }
            let search_line: &str = if self.case_sensitive {
                line.as_str()
            } else {
                // Defensive: cache should be in sync, but fall back if the
                // caller forgot to pass a fresh edit_version.
                self.lowercase_cache
                    .get(line_idx)
                    .map(|s| s.as_str())
                    .unwrap_or(line.as_str())
            };

            let mut start = 0;
            while let Some(pos) = search_line[start..].find(&query) {
                let byte_start = start + pos;
                let byte_end = byte_start + query.len();
                let col_start = buffer::byte_to_char(line, byte_start);
                let col_end = buffer::byte_to_char(line, byte_end);

                // Per-match bounds filter for start/end lines of the selection.
                if let Some((s, e)) = bounds {
                    if line_idx == s.line && col_start < s.col {
                        start = byte_start + query.len().max(1);
                        continue;
                    }
                    if line_idx == e.line && col_end > e.col {
                        start = byte_start + query.len().max(1);
                        continue;
                    }
                }

                if self.whole_word {
                    // Word-boundary check with full Unicode awareness —
                    // ASCII-only `is_ascii_alphanumeric` treated é/ж/你 as
                    // non-word chars, so "ana" inside "mañana" or "рад"
                    // inside "радуга" leaked through the whole-word filter.
                    let before_ok = match line[..byte_start].chars().next_back() {
                        Some(c) => !buffer::is_word_char(c),
                        None => true,
                    };
                    let after_ok = match line[byte_end..].chars().next() {
                        Some(c) => !buffer::is_word_char(c),
                        None => true,
                    };
                    if before_ok && after_ok {
                        self.matches.push((line_idx, col_start, col_end));
                    }
                } else {
                    self.matches.push((line_idx, col_start, col_end));
                }

                start = byte_start + query.len().max(1);
            }
        }

        if self.current_match >= self.matches.len() {
            self.current_match = 0;
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches() {
        let mut state = FindReplaceState::default();
        state.query = "hello".to_string();
        let lines = vec![
            "hello world".to_string(),
            "say hello".to_string(),
            "nothing here".to_string(),
        ];
        state.update_matches(&lines, 1);
        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.matches[0], (0, 0, 5));
        assert_eq!(state.matches[1], (1, 4, 9));
    }

    #[test]
    fn test_find_case_insensitive() {
        let mut state = FindReplaceState::default();
        state.query = "hello".to_string();
        state.case_sensitive = false;
        let lines = vec!["Hello HELLO hello".to_string()];
        state.update_matches(&lines, 1);
        assert_eq!(state.matches.len(), 3);
    }
}
