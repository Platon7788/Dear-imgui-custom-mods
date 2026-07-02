//! `CodeEditor` public API surface — text/config accessors, navigation,
//! marker setters, find toggles, and word-at-cursor. Split out of mod.rs
//! (500-line rule). All methods stay `pub`.

use super::*;

impl CodeEditor {
    // ── Public API ───────────────────────────────────────────────────

    /// Set the entire text content (resets undo, cursor, selection).
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.undo_stack.clear();
        self.bc_version = u64::MAX; // force recompute
        self.bc_dirty_from = Some(0);
        self.fold_version = u64::MAX;
        self.token_cache.clear();
        self.find_replace.matches.clear();
        // Force word-wrap cache recomputation on the next render call.
        // Without this, update_wrap_cache() sees an unchanged version and
        // skips recalculation, leaving a stale single-line layout.
        self.wrap_cached_version = u64::MAX;
        self.wrap_cached_width = 0.0;
        // Reset scroll — otherwise the viewport can sit past end-of-document
        // (pointing into whitespace) until the user scrolls or ensure_cursor_visible
        // pulls it back, which now only fires when the cursor itself moves.
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.target_scroll_y = 0.0;
        self.last_set_scroll_y = 0.0;
    }

    /// Get the entire text content.
    ///
    /// Allocates a fresh `String` on every call — for high-frequency
    /// pollers (save-on-change watchers, streaming UIs) use
    /// [`get_text_into`](Self::get_text_into) to reuse a persistent buffer.
    pub fn get_text(&self) -> String {
        self.buffer.text()
    }

    /// Append the entire text into `buf`, reusing existing capacity.
    ///
    /// `buf` is cleared first. Use this instead of `get_text` when you
    /// poll the editor text every frame and want to avoid the per-frame
    /// heap allocation.
    pub fn get_text_into(&self, buf: &mut String) {
        self.buffer.text_into(buf);
    }

    /// Detected line-ending style from the last `set_text`. Preserved on
    /// `get_text` / `get_text_into` so Windows CRLF files round-trip
    /// without being mangled into LF.
    pub fn line_ending(&self) -> LineEnding {
        self.buffer.line_ending()
    }

    /// Override the detected line ending (e.g. user-forced conversion).
    pub fn set_line_ending(&mut self, ending: LineEnding) {
        self.buffer.set_line_ending(ending);
    }

    /// Whether the loaded text appeared to use tab indentation. Callers
    /// can sync editor config after `set_text`:
    ///
    /// ```ignore
    /// editor.set_text(&file_contents);
    /// editor.config_mut().insert_spaces = !editor.detected_uses_tabs();
    /// ```
    pub fn detected_uses_tabs(&self) -> bool {
        self.buffer.detected_uses_tabs()
    }

    /// Whether the buffer has been modified since last `clear_modified()`.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Mark buffer as clean (e.g., after save).
    pub fn clear_modified(&mut self) {
        self.buffer.clear_modified();
    }

    /// Set syntax language.
    pub fn set_language(&mut self, lang: Language) {
        self.config.language = lang;
        self.bc_version = u64::MAX;
        self.bc_dirty_from = Some(0);
        self.token_cache.clear();
    }

    /// Set read-only mode.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.config.read_only = read_only;
    }

    /// Whether the editor is read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.config.read_only
    }

    /// Navigate to a specific line (0-based).
    pub fn goto_line(&mut self, line: usize) {
        self.buffer.goto_line(line);
        self.ensure_cursor_visible();
    }

    /// Set error/warning markers.
    pub fn set_error_markers(&mut self, markers: Vec<LineMarker>) {
        self.error_lines = markers.iter().map(|m| m.line).collect();
        self.error_markers = markers;
    }

    /// Set breakpoints.
    pub fn set_breakpoints(&mut self, bps: Vec<Breakpoint>) {
        self.breakpoint_lines = bps
            .iter()
            .filter(|bp| bp.enabled)
            .map(|bp| bp.line)
            .collect();
        self.breakpoints = bps;
    }

    /// Get access to the editor configuration.
    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    /// Get mutable access to the editor configuration.
    pub fn config_mut(&mut self) -> &mut EditorConfig {
        &mut self.config
    }

    /// Current cursor position.
    pub fn cursor(&self) -> CursorPos {
        self.buffer.cursor()
    }

    /// Total line count.
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// Get the word (identifier) under the cursor, if any.
    pub fn word_at_cursor(&self) -> Option<String> {
        let pos = self.buffer.cursor();
        let lines = self.buffer.lines();
        let line = lines.get(pos.line)?;
        let chars: Vec<char> = line.chars().collect();
        // Clamp instead of rejecting: a caret one past the last char (col ==
        // len, i.e. end of line) is a valid position, and `foo|` should still
        // resolve to "foo" by expanding left.
        let col = pos.col.min(chars.len());
        // Expand left
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        // Expand right
        let mut end = col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some(chars[start..end].iter().collect())
    }

    /// Insert text at the current cursor position.
    pub fn insert_text(&mut self, text: &str) {
        self.buffer.insert_text(text);
    }

    /// Delete `n` characters before the cursor (like pressing Backspace n times).
    pub fn delete_chars_before(&mut self, n: usize) {
        for _ in 0..n {
            self.buffer.backspace();
        }
    }

    /// Whether the editor is focused.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Get selected text.
    pub fn selected_text(&self) -> String {
        self.buffer.selected_text()
    }

    /// Open the find panel.
    pub fn open_find(&mut self) {
        self.find_replace.open = true;
        self.find_replace.show_replace = false;
        self.find_replace.just_opened = true;
    }

    /// Open the find & replace panel.
    pub fn open_find_replace(&mut self) {
        self.find_replace.open = true;
        self.find_replace.show_replace = true;
        self.find_replace.just_opened = true;
    }

    /// Current text zoom factor (1.0 = default).
    pub fn text_scale(&self) -> f32 {
        self.config.font_size_scale
    }

    /// Set text zoom factor (clamped to 0.4–4.0).
    pub fn set_text_scale(&mut self, scale: f32) {
        self.config.font_size_scale = scale.clamp(0.4, 4.0);
    }

    /// Close the find panel.
    pub fn close_find(&mut self) {
        self.find_replace.open = false;
    }

    /// Toggle fold at a line.
    pub fn toggle_fold(&mut self, line: usize) {
        for region in &mut self.fold_regions {
            if region.start_line == line {
                region.folded = !region.folded;
                return;
            }
        }
    }
}
