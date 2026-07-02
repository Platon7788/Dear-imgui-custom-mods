//! Word-wrap, scroll/viewport virtualization, fold-region detection, and
//! keyboard-layout switching for [`CodeEditor`]. Split out of mod.rs.

use super::*;

impl CodeEditor {
    // ── Word wrap ─────────────────────────────────────────────────────

    /// Recompute the word-wrap cache if the text changed or the
    /// available width changed.
    pub(super) fn update_wrap_cache(&mut self, text_width: f32) {
        if !self.config.word_wrap {
            // Ensure offsets are identity when wrap is off.
            if !self.wrap_cols.is_empty() {
                self.wrap_cols.clear();
                self.wrap_hashes.clear();
                self.wrap_row_offsets.clear();
                self.wrap_row_offsets.push(0);
                self.wrap_cached_version = u64::MAX;
            }
            return;
        }
        let version = self.buffer.edit_version();
        let width_changed = (text_width - self.wrap_cached_width).abs() > 0.5;
        if version == self.wrap_cached_version && !width_changed {
            return;
        }
        self.wrap_cached_version = version;
        self.wrap_cached_width = text_width;

        let line_count = self.buffer.line_count();
        self.wrap_cols.resize_with(line_count, Vec::new);
        // u64::MAX = "never wrapped" sentinel so a real hash forces the rebuild.
        self.wrap_hashes.resize(line_count, u64::MAX);
        self.wrap_row_offsets.resize(line_count + 1, 0);

        // Scratch allocated once per rebuild (not per line) and reused across
        // every line — the old code allocated two Vecs per line per keystroke.
        let mut widths: Vec<f32> = Vec::new();
        let mut is_ws: Vec<bool> = Vec::new();

        let mut cumulative = 0usize;
        for i in 0..line_count {
            self.wrap_row_offsets[i] = cumulative;
            // Only re-wrap a line whose content changed (or on a width change).
            // Typing on one line leaves every other line's wrap untouched
            // instead of re-wrapping the whole document each keystroke.
            let h = hash_line(self.buffer.line(i));
            if width_changed || self.wrap_hashes[i] != h {
                self.wrap_hashes[i] = h;
                let line = self.buffer.line(i);
                compute_wrap_points_into(
                    line,
                    text_width,
                    self.char_advance,
                    self.config.tab_size,
                    &mut widths,
                    &mut is_ws,
                    &mut self.wrap_cols[i],
                );
            }
            cumulative += self.wrap_cols[i].len() + 1;
        }
        self.wrap_row_offsets[line_count] = cumulative;
    }

    /// Total number of visual rows (accounting for word wrap).
    pub(super) fn total_visual_rows(&self) -> usize {
        if !self.config.word_wrap || self.wrap_row_offsets.len() <= 1 {
            return self.buffer.line_count();
        }
        *self.wrap_row_offsets.last().unwrap_or(&0)
    }

    /// Convert a buffer (line, col) to a visual row index.
    pub(super) fn visual_row_of(&self, line: usize, col: usize) -> usize {
        if !self.config.word_wrap
            || line >= self.wrap_cols.len()
            || line >= self.wrap_row_offsets.len()
        {
            return line;
        }
        let base = self.wrap_row_offsets[line];
        let wraps = &self.wrap_cols[line];
        // Find which sub-row this col falls in.
        let sub = wraps.iter().position(|&wc| col < wc).unwrap_or(wraps.len());
        base + sub
    }

    /// Convert a visual row to (buffer_line, sub_row_index).
    pub(super) fn visual_row_to_line(&self, vrow: usize) -> (usize, usize) {
        let line_count = self.buffer.line_count();
        // Fall back to identity when wrap is off or cache is stale/empty.
        if !self.config.word_wrap || self.wrap_row_offsets.len() < line_count + 1 {
            return (vrow.min(line_count.saturating_sub(1)), 0);
        }
        // Binary search: find largest line whose offset <= vrow.
        let mut lo = 0usize;
        let mut hi = line_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.wrap_row_offsets[mid + 1] <= vrow {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let line = lo.min(line_count.saturating_sub(1));
        let sub = vrow.saturating_sub(self.wrap_row_offsets[line]);
        (line, sub)
    }

    /// Get the column range for a sub-row of a line.
    pub(super) fn sub_row_col_range(&self, line: usize, sub_row: usize) -> (usize, usize) {
        if !self.config.word_wrap || line >= self.wrap_cols.len() {
            return (0, self.buffer.line(line).chars().count());
        }
        let wraps = &self.wrap_cols[line];
        let start = if sub_row == 0 {
            0
        } else {
            wraps.get(sub_row - 1).copied().unwrap_or(0)
        };
        let line_chars = self.buffer.line(line).chars().count();
        let end = wraps.get(sub_row).copied().unwrap_or(line_chars);
        // Clamp: during the in-frame window between a buffer edit (Ctrl+A +
        // Delete, paste, etc.) and the next `update_wrap_cache` call, wraps
        // may reference columns past the now-shorter line. Returning start>end
        // would underflow the `end - start` subtraction in `handle_mouse`.
        let start = start.min(line_chars);
        let end = end.max(start);
        (start, end)
    }

    // ── Keyboard layout switching ──────────────────────────────────

    /// On focus gain: save current layout and switch to English (US).
    /// On focus loss: restore the previously saved layout.
    pub(super) fn handle_input_locale_switch(&mut self) {
        let gained = self.focused && !self.was_focused;
        let lost = !self.focused && self.was_focused;
        self.was_focused = self.focused;

        if !self.config.force_english_on_focus {
            return;
        }

        #[cfg(target_os = "windows")]
        {
            // English (US) keyboard layout identifier: 0x0409
            const EN_US: usize = 0x0409;

            unsafe extern "system" {
                fn GetKeyboardLayout(thread_id: u32) -> usize;
                fn ActivateKeyboardLayout(hkl: usize, flags: u32) -> usize;
            }

            if gained {
                let current = unsafe { GetKeyboardLayout(0) };
                self.saved_input_locale = current;
                // ActivateKeyboardLayout with 0 flags = KLF_SETFORPROCESS not set
                // — applies only to the current thread.
                unsafe {
                    ActivateKeyboardLayout(EN_US, 0);
                }
            } else if lost && self.saved_input_locale != 0 {
                unsafe {
                    ActivateKeyboardLayout(self.saved_input_locale, 0);
                }
                self.saved_input_locale = 0;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (gained, lost);
        }
    }

    // ── Smooth scrolling ────────────────────────────────────────────

    pub(super) fn update_smooth_scroll(&mut self, dt: f32) {
        if !self.config.smooth_scrolling {
            return;
        }

        let diff = self.target_scroll_y - self.scroll_y;
        if diff.abs() < 0.5 {
            self.scroll_y = self.target_scroll_y;
            return;
        }

        // When the gap is large (rapid Enter / PgDn), snap harder so the
        // cursor never drifts off-screen.  For small gaps the original
        // smooth ease-out is used.
        let big_gap = diff.abs() > self.line_height * 3.0;
        let speed = if big_gap { 25.0_f32 } else { 12.0_f32 };
        let factor = 1.0 - (-speed * dt).exp();
        self.scroll_y += diff * factor;
    }

    // ── Scroll management ───────────────────────────────────────────

    pub(super) fn visible_lines(&self) -> usize {
        if self.line_height > 0.0 {
            (self.visible_height / self.line_height) as usize
        } else {
            30
        }
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        let cursor = self.buffer.cursor();
        let vrow = self.visual_row_of(cursor.line, cursor.col);
        let cursor_y = vrow as f32 * self.line_height;

        // Vertical
        let target = if cursor_y < self.scroll_y {
            cursor_y
        } else if cursor_y + self.line_height > self.scroll_y + self.visible_height {
            cursor_y + self.line_height - self.visible_height
        } else {
            return;
        };

        if self.config.smooth_scrolling {
            self.target_scroll_y = target;
        } else {
            self.scroll_y = target;
            self.target_scroll_y = target;
        }
    }

    /// Truncate pasted text to respect `max_lines` and `max_line_length`.
    pub(super) fn truncate_paste(&self, text: &str) -> String {
        let max_lines = self.config.max_lines;
        let max_len = self.config.max_line_length;

        let mut result = String::with_capacity(text.len());
        let current_lines = self.buffer.line_count();
        let mut added_newlines = 0usize;

        for (i, line) in text.split('\n').enumerate() {
            // Check line count budget
            if max_lines > 0 && i > 0 && current_lines + added_newlines >= max_lines {
                break;
            }
            if i > 0 {
                result.push('\n');
                added_newlines += 1;
            }
            // Truncate line length
            if max_len > 0 && line.chars().count() > max_len {
                result.extend(line.chars().take(max_len));
            } else {
                result.push_str(line);
            }
        }
        result
    }

    // ── Block comment state tracking ────────────────────────────────

    pub(super) fn update_block_comment_states(&mut self) {
        let version = self.buffer.edit_version();
        if self.bc_version == version {
            return;
        }
        self.bc_version = version;

        let count = self.buffer.line_count();
        let start_from = self.bc_dirty_from.unwrap_or(0).min(count);
        self.bc_dirty_from = None;

        // Resize to match line count (preserves existing correct entries).
        self.block_comment_states.resize(count, false);

        // Determine the bc state entering `start_from`.
        let mut in_bc = if start_from == 0 {
            false
        } else {
            let prev_bc = self.block_comment_states[start_from - 1];
            let (_, still_in) = tokenize_line(
                self.buffer.line(start_from - 1),
                &self.config.language,
                prev_bc,
            );
            still_in
        };

        for i in start_from..count {
            self.block_comment_states[i] = in_bc;
            let (_, still_in) = tokenize_line(self.buffer.line(i), &self.config.language, in_bc);
            in_bc = still_in;

            // Early exit: if the bc state entering the next line matches
            // what was already stored, all downstream states are correct.
            if i + 1 < count && self.block_comment_states[i + 1] == in_bc {
                break;
            }
        }
    }

    // ── Code folding ────────────────────────────────────────────────

    pub(super) fn update_fold_regions(&mut self) {
        let version = self.buffer.edit_version();
        if self.fold_version == version {
            return;
        }
        self.fold_version = version;

        let new_regions = detect_fold_regions(self.buffer.lines());

        // Preserve fold state from existing regions — HashMap for O(1) lookup
        let was_folded: std::collections::HashMap<usize, bool> = self
            .fold_regions
            .iter()
            .map(|r| (r.start_line, r.folded))
            .collect();

        self.fold_regions = new_regions;

        for region in &mut self.fold_regions {
            if let Some(&folded) = was_folded.get(&region.start_line) {
                region.folded = folded;
            }
        }
    }

    /// Build list of (line_index, screen_row) for visible lines,
    /// skipping folded regions.
    pub(super) fn build_visible_lines(
        &self,
        first_visible: usize,
        last_visible: usize,
    ) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(last_visible - first_visible);
        // Precompute a `start_line → end_line` map of folded regions so
        // the per-line fold-check is O(1) instead of O(fold_count). On
        // a file with many folds and many visible lines the previous
        // `iter().find()` was O(V·N); this is O(N + V).
        let folded_ends: std::collections::HashMap<usize, usize> = self
            .fold_regions
            .iter()
            .filter(|r| r.folded)
            .map(|r| (r.start_line, r.end_line))
            .collect();

        let mut line_idx = first_visible;
        while line_idx < last_visible && line_idx < self.buffer.line_count() {
            result.push((line_idx, line_idx));

            if let Some(&end) = folded_ends.get(&line_idx) {
                line_idx = end + 1;
            } else {
                line_idx += 1;
            }
        }
        result
    }
}
