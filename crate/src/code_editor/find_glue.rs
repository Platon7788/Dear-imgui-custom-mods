//! Find / replace glue + the find bar and right-click context menu UI.
//!
//! Bridges the editor state to [`super::find_replace::FindReplaceState`]
//! and renders the chrome. Split out of mod.rs.

use super::*;

impl CodeEditor {
    // ── Find/Replace ────────────────────────────────────────────────

    pub(super) fn update_find_matches(&mut self) {
        // Resolve the search scope: Selection mode requires an active
        // non-empty selection; fall back to All otherwise.
        let bounds = match self.find_replace.scope {
            FindScope::Selection => self.buffer.selection().and_then(|sel| {
                if sel.is_empty() {
                    None
                } else {
                    Some(sel.ordered())
                }
            }),
            FindScope::All => None,
        };
        self.find_replace.update_matches_scoped(
            self.buffer.lines(),
            self.buffer.edit_version(),
            bounds,
        );
    }

    pub(super) fn find_next(&mut self) {
        if self.find_replace.matches.is_empty() {
            return;
        }
        self.find_replace.current_match =
            (self.find_replace.current_match + 1) % self.find_replace.matches.len();
        self.jump_to_current_match();
    }

    pub(super) fn find_prev(&mut self) {
        if self.find_replace.matches.is_empty() {
            return;
        }
        if self.find_replace.current_match == 0 {
            self.find_replace.current_match = self.find_replace.matches.len() - 1;
        } else {
            self.find_replace.current_match -= 1;
        }
        self.jump_to_current_match();
    }

    pub(super) fn jump_to_current_match(&mut self) {
        if let Some(&(line, col_start, col_end)) = self
            .find_replace
            .matches
            .get(self.find_replace.current_match)
        {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.ensure_cursor_visible();
        }
    }

    pub(super) fn replace_current(&mut self) {
        if self.find_replace.matches.is_empty() || self.config.read_only {
            return;
        }
        self.snapshot_undo(true);
        if let Some(&(line, col_start, col_end)) = self
            .find_replace
            .matches
            .get(self.find_replace.current_match)
        {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.buffer.backspace();
            self.buffer
                .insert_text(&self.find_replace.replacement.clone());
            self.invalidate_token_cache_all();
            self.update_find_matches();
        }
    }

    pub(super) fn replace_all(&mut self) {
        if self.find_replace.matches.is_empty() || self.config.read_only {
            return;
        }
        self.snapshot_undo(true);
        // Replace from bottom to top so positions don't shift
        let replacement = self.find_replace.replacement.clone();
        let mut matches = self.find_replace.matches.clone();
        matches.reverse();
        for (line, col_start, col_end) in matches {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.buffer.backspace();
            self.buffer.insert_text(&replacement);
        }
        self.invalidate_token_cache_all();
        self.update_find_matches();
    }
}
