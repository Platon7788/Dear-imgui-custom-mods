//! Token cache, block-comment state, undo/redo glue, and cursor blink.
//!
//! Split out of mod.rs. These methods keep the per-line tokenization cache
//! and the undo stack in sync with buffer edits.

use super::*;

impl CodeEditor {
    // ── Token cache management ──────────────────────────────────────

    pub(super) fn ensure_token_cache_size(&mut self) {
        let count = self.buffer.line_count();
        self.token_cache.resize_with(count, || None);
    }

    pub(super) fn get_cached_tokens(&mut self, line_idx: usize) -> Rc<Vec<Token>> {
        let line_str = self.buffer.line(line_idx);
        let content_hash = hash_line(line_str);
        let in_bc = self
            .block_comment_states
            .get(line_idx)
            .copied()
            .unwrap_or(false);

        // Check cache hit — Rc::clone is just a refcount bump, no Vec copy.
        if let Some(Some(cached)) = self.token_cache.get(line_idx)
            && cached.content_hash == content_hash
            && cached.in_block_comment == in_bc
        {
            return Rc::clone(&cached.tokens);
        }

        // Cache miss — tokenize
        let (tokens, _ends_in_bc) = tokenize_line(line_str, &self.config.language, in_bc);
        let rc = Rc::new(tokens);

        // Store in cache
        if line_idx < self.token_cache.len() {
            self.token_cache[line_idx] = Some(CachedLineTokens {
                content_hash,
                in_block_comment: in_bc,
                tokens: Rc::clone(&rc),
            });
        }

        rc
    }

    /// Read-only token lookup — returns cached tokens or empty.
    /// Call `get_cached_tokens` first to ensure the cache is populated.
    pub(super) fn cached_tokens(&self, line_idx: usize) -> Rc<Vec<Token>> {
        if let Some(Some(cached)) = self.token_cache.get(line_idx) {
            Rc::clone(&cached.tokens)
        } else {
            Rc::new(Vec::new())
        }
    }

    pub(super) fn invalidate_token_cache_at(&mut self, line: usize) {
        if line < self.token_cache.len() {
            self.token_cache[line] = None;
        }
        // Mark bc state dirty from this line so incremental recompute starts here.
        self.bc_dirty_from = Some(self.bc_dirty_from.map_or(line, |old| old.min(line)));
        self.bc_version = u64::MAX;
    }

    /// Invalidate token cache from `from_line` onward (for structural edits
    /// that insert/remove lines). Entries before `from_line` stay valid.
    pub(super) fn invalidate_token_cache_from(&mut self, from_line: usize) {
        self.token_cache.truncate(from_line);
        self.bc_dirty_from = Some(
            self.bc_dirty_from
                .map_or(from_line, |old| old.min(from_line)),
        );
        self.bc_version = u64::MAX;
    }

    pub(super) fn invalidate_token_cache_all(&mut self) {
        self.invalidate_token_cache_from(0);
    }

    // ── Undo/Redo ───────────────────────────────────────────────────

    pub(super) fn snapshot_undo(&mut self, force: bool) {
        let version = self.buffer.edit_version();
        // Skip the expensive `buffer.text()` clone when grouping would
        // discard the snapshot anyway. On a 1 MB buffer this used to
        // allocate a full copy on every keystroke — even though the
        // resulting entry was immediately dropped by the grouping logic.
        if !self.undo_stack.should_push(version, force) {
            // Feed the stack a zero-alloc placeholder so it can still bump
            // last_push_version and clear redo.
            let marker = UndoEntry {
                text: String::new(),
                cursor: self.buffer.cursor(),
                version,
            };
            self.undo_stack.push(marker, false);
            return;
        }
        let entry = UndoEntry {
            text: self.buffer.text(),
            cursor: self.buffer.cursor(),
            version,
        };
        if force {
            self.undo_stack.force_snapshot(entry);
        } else {
            self.undo_stack.push(entry, false);
        }
    }

    pub(super) fn current_undo_entry(&self) -> UndoEntry {
        UndoEntry {
            text: self.buffer.text(),
            cursor: self.buffer.cursor(),
            version: self.buffer.edit_version(),
        }
    }

    /// Perform undo.
    pub fn undo(&mut self) {
        let current = self.current_undo_entry();
        if let Some(entry) = self.undo_stack.undo(current) {
            // `restore_from_undo` preserves `modified = true` and bumps
            // `edit_version` (unlike `set_text` which resets both). Without
            // this the save-prompt dirty indicator silently goes false
            // after Ctrl+Z, and version-keyed caches would see a DROP in
            // version and reuse stale entries.
            self.buffer.restore_from_undo(&entry.text, entry.cursor);
            self.invalidate_token_cache_all();
            self.ensure_cursor_visible();
        }
    }

    /// Perform redo.
    pub fn redo(&mut self) {
        let current = self.current_undo_entry();
        if let Some(entry) = self.undo_stack.redo(current) {
            self.buffer.restore_from_undo(&entry.text, entry.cursor);
            self.invalidate_token_cache_all();
            self.ensure_cursor_visible();
        }
    }

    // ── Cursor blink ────────────────────────────────────────────────

    pub(super) fn update_blink(&mut self, dt: f32) {
        if self.config.cursor_blink_rate <= 0.0 {
            self.cursor_visible = true;
            return;
        }
        self.blink_timer += dt;
        if self.blink_timer >= self.config.cursor_blink_rate {
            self.blink_timer -= self.config.cursor_blink_rate;
            self.cursor_visible = !self.cursor_visible;
        }
    }

    pub(super) fn reset_blink(&mut self) {
        self.blink_timer = 0.0;
        self.cursor_visible = true;
    }
}
