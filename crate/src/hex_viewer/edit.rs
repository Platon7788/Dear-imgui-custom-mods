//! Edit lifecycle, undo/redo, and selection/navigation helpers.
//!
//! Split out of `input.rs` to keep it under the 500-line ceiling. These
//! methods own the edit-mode state machine (`start_editing` /
//! `stop_editing` / `commit_pending_edit*`), the provider-aware
//! undo/redo path, cursor clamping/scrolling, and clipboard copy. All
//! are `pub(super)` (or `pub` for the documented `undo`/`redo` API) so
//! the keyboard handler in `input.rs`, the mouse handler in `mouse.rs`,
//! and the public API in `api.rs` can call them.

use super::HexViewer;
use super::input::EditColumn;
use super::provider::HexDataProvider;
use super::search::{Selection, format_bytes};
use super::undo::UndoEntry;
use crate::utils::clipboard::{self, set_clipboard};

// ── HexViewer impl: edit lifecycle, undo/redo, navigation helpers ────────────

impl HexViewer {
    pub(super) fn start_editing(&mut self, column: EditColumn) {
        self.edit_column = Some(column);
        self.edit_nibble = None;
        // Switch to English layout for hex input.
        if column == EditColumn::Hex && !self.layout_switched {
            clipboard::activate_english_layout();
            self.layout_switched = true;
        }
    }

    pub(super) fn stop_editing(&mut self) {
        self.edit_column = None;
        self.edit_nibble = None;
        if self.layout_switched {
            clipboard::restore_keyboard_layout();
            self.layout_switched = false;
        }
    }

    /// Provider-less commit, used by public API entry points
    /// (`set_cursor`, `goto`) where no provider is in scope. Reads /
    /// writes the viewer's internal `self.data` directly — equivalent
    /// to the legacy path. Provider-driven render frames go through
    /// [`Self::commit_pending_edit_with`] instead.
    pub(super) fn commit_pending_edit(&mut self) {
        let mut wrapper =
            super::provider::ArcVecDataProvider::from_arc(std::sync::Arc::clone(&self.data));
        self.commit_pending_edit_with(&mut wrapper);
    }

    /// Apply a half-typed hex nibble before the user navigates away.
    ///
    /// In hex-edit mode each byte takes two keystrokes (high nibble +
    /// low nibble). If only the high nibble was typed, navigating to
    /// another byte historically silently dropped it. This commit-on-
    /// leave variant replaces the **upper** nibble of the current byte
    /// and keeps the lower nibble intact — matches HxD/010-style hex
    /// editors and gives the user a way to write a single nibble.
    ///
    /// Pushes a single-byte undo entry only when the new value really
    /// differs from the old (avoids polluting undo history with no-op
    /// "type X then leave with same X" sequences).
    ///
    /// To **discard** instead of committing, call `stop_editing` —
    /// it zeroes `edit_nibble` first.
    ///
    /// Phase 2: `provider` is the data source / sink. Reading the
    /// "old" byte through the provider keeps the diff math (and the
    /// `byte_edit_callback`'s `old_byte` argument) honest for
    /// streaming-memory hosts whose internal `self.data` mirror is
    /// stale or empty. Writes are attempted through the provider
    /// first; if it refuses (`write()` returns `false`, the default
    /// for read-only providers including the legacy
    /// `ArcVecDataProvider` wrapper around `self.data`) we fall back
    /// to mutating the internal `Arc<Vec<u8>>` via `Arc::make_mut` —
    /// matches the pre-refactor behaviour for all legacy callers.
    pub(super) fn commit_pending_edit_with(&mut self, provider: &mut dyn HexDataProvider) {
        let Some(hi) = self.edit_nibble.take() else {
            return;
        };
        let data_len = provider.len();
        if (self.cursor as u64) >= data_len {
            return;
        }
        let mut read_buf = [0u8; 1];
        let n = provider.read(self.cursor as u64, &mut read_buf);
        if n == 0 {
            // Provider couldn't satisfy the read — refuse to commit
            // rather than guessing at the old byte (would corrupt
            // the undo stack with a fabricated old value).
            return;
        }
        let old_byte = read_buf[0];
        let new_byte = (hi << 4) | (old_byte & 0x0F);
        if new_byte == old_byte {
            return;
        }
        self.undo.push(UndoEntry {
            offset: self.cursor as u64,
            old_bytes: vec![old_byte],
            new_bytes: vec![new_byte],
        });
        let va = self.config.base_address + self.cursor as u64;
        // Phase 2: mirror the edit to BOTH the active provider AND
        // the viewer's internal `self.data`. Why both:
        //   * Provider write keeps the *current frame's* render
        //     coherent — the wrapper used by the legacy
        //     [`HexViewer::render`] entry point applies the edit to
        //     its own `Arc<Vec<u8>>` clone via `Arc::make_mut`, so
        //     the immediately-following row-draw loop reads the new
        //     byte and the user sees the change without a frame delay.
        //   * `Arc::make_mut(&mut self.data)` keeps the *next frame's*
        //     wrapper / the host-facing `data()` getter consistent —
        //     the legacy provider is rebuilt at the top of every
        //     frame from `self.data`, so without this step the next
        //     frame would render the pre-edit byte.
        // For read-only / streaming providers (host-supplied) the
        // `provider.write()` call returns `false`; the `self.data`
        // patch then runs in isolation (bounds-checked — empty for
        // pure streaming hosts so it no-ops safely).
        let accepted = provider.write(self.cursor as u64, &[new_byte]);
        if self.cursor < self.data.len() {
            std::sync::Arc::make_mut(&mut self.data)[self.cursor] = new_byte;
        }
        let _ = accepted; // currently unused — kept for future audit hooks
        // Fire the host's edit notification — driven hosts (debugger
        // memory pane, packet editor, ROM patcher) wire this to push
        // the byte change into their backing store. See
        // `HexViewer::set_byte_edit_callback`. The mutation above is
        // applied unconditionally so in-buffer state stays consistent
        // even when the host doesn't accept (or fails to apply) the
        // edit downstream — the host can choose to ignore the
        // notification, refuse the write, and patch the viewer back
        // via `set_data` on the next pull cycle.
        if let Some(cb) = self.byte_edit_callback.as_mut() {
            cb(va, old_byte, new_byte);
        }
    }

    /// Undo the last edit. Public entry point with no provider in
    /// scope — operates on `self.data` directly (legacy semantics).
    /// The in-frame variant called from the keyboard handler routes
    /// through [`Self::undo_with`] so the active provider sees the
    /// rollback (host's debug-target memory etc.).
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo.undo() {
            let off = entry.offset as usize;
            let old = entry.old_bytes.clone();
            // `checked_add` so a crafted `UndoEntry::offset` near
            // `usize::MAX` can't overflow the bounds check (debug panic).
            if off
                .checked_add(old.len())
                .is_some_and(|end| end <= self.data.len())
            {
                std::sync::Arc::make_mut(&mut self.data)[off..off + old.len()]
                    .copy_from_slice(&old);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    /// Provider-aware undo. Dual-writes: provider for current-frame
    /// visual coherence + `self.data` via `Arc::make_mut` so the
    /// host-facing `data()` getter and next-frame wrapper see the
    /// rolled-back bytes.
    pub(super) fn undo_with(&mut self, provider: &mut dyn HexDataProvider) {
        if let Some(entry) = self.undo.undo() {
            let off = entry.offset as usize;
            let old = entry.old_bytes.clone();
            let len = super::draw::provider_len_usize(provider);
            let Some(end) = off.checked_add(old.len()) else {
                return;
            };
            if end <= len {
                let _ = provider.write(off as u64, &old);
                if end <= self.data.len() {
                    std::sync::Arc::make_mut(&mut self.data)[off..end].copy_from_slice(&old);
                }
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    /// Redo the most-recently-undone edit. Public legacy entry. See
    /// [`Self::redo_with`] for the in-frame provider-aware variant.
    pub fn redo(&mut self) {
        if let Some(entry) = self.undo.redo() {
            let off = entry.offset as usize;
            let new = entry.new_bytes.clone();
            if off
                .checked_add(new.len())
                .is_some_and(|end| end <= self.data.len())
            {
                std::sync::Arc::make_mut(&mut self.data)[off..off + new.len()]
                    .copy_from_slice(&new);
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    /// Provider-aware redo. Mirror of [`Self::undo_with`] — dual-writes.
    pub(super) fn redo_with(&mut self, provider: &mut dyn HexDataProvider) {
        if let Some(entry) = self.undo.redo() {
            let off = entry.offset as usize;
            let new = entry.new_bytes.clone();
            let len = super::draw::provider_len_usize(provider);
            let Some(end) = off.checked_add(new.len()) else {
                return;
            };
            if end <= len {
                let _ = provider.write(off as u64, &new);
                if end <= self.data.len() {
                    std::sync::Arc::make_mut(&mut self.data)[off..end].copy_from_slice(&new);
                }
                self.cursor = off;
                self.scroll_to_cursor();
            }
        }
    }

    pub(super) fn clamp_cursor(&mut self) {
        // Public API path — no provider in scope, so clamp against
        // the larger of `self.data.len()` and `effective_data_len`
        // (the last render's projected length). This keeps the cursor
        // inside the visible buffer even right after a streaming host
        // resets `self.data` to empty but the viewer is still pinned
        // mid-window.
        let len = self.data.len().max(self.effective_data_len);
        self.cursor = self.cursor.min(len.saturating_sub(1));
    }

    pub(super) fn scroll_to_cursor(&mut self) {
        let bpr = self.config.bytes_per_row.value();
        let row = self.cursor / bpr;
        self.scroll_to_row = Some(row);
    }

    /// Move the cursor to `new_cursor`, optionally extending the
    /// selection. When `shift` is true and selection is currently
    /// empty, anchors `selection.start` at the *previous* cursor
    /// position so the byte under the old cursor is included in the
    /// growing range. When `shift` is false, the selection is cleared.
    ///
    /// Commits any half-typed hex nibble on the *old* byte before
    /// moving — keyboard navigation should never silently drop user
    /// input. The edit-mode itself is preserved (caller is moving
    /// inside the same edit session); to leave edit mode, callers
    /// also invoke `stop_editing`.
    ///
    /// Phase 2: `provider` participates in the pending-edit commit so
    /// that the byte the user just typed reaches the host's backing
    /// store (debugger memory write) before the cursor leaves the
    /// edited byte. The buffer-length clamp uses the provider's
    /// `len()` projection — keeps the cursor inside the visible
    /// streaming window even when `self.data` is empty (host hasn't
    /// committed bytes back yet).
    pub(super) fn move_cursor_with_selection_with(
        &mut self,
        new_cursor: usize,
        shift: bool,
        provider: &mut dyn HexDataProvider,
    ) {
        self.commit_pending_edit_with(provider);
        let len_usize = super::draw::provider_len_usize(provider);
        let old = self.cursor;
        self.cursor = new_cursor.min(len_usize.saturating_sub(1));
        if shift {
            if self.selection.is_empty() {
                self.selection.start = old;
            }
            self.selection.end = self.cursor;
        } else {
            self.selection = Selection::default();
        }
        self.scroll_to_cursor();
    }

    /// Provider-less variant kept for the few callsites outside the
    /// render frame (programmatic API). Wraps `self.data` in an
    /// `ArcVecDataProvider` so the same code path runs for every
    /// caller — no behavioural divergence between this and the
    /// `_with` overload.
    ///
    /// Currently unreferenced inside the crate (every callsite went
    /// through the `_with` rename in Phase 2) but kept on the
    /// surface as the documented "I have a `HexViewer` but no
    /// provider, please move the cursor" entry point — see the
    /// `goto` / `set_cursor` family above.
    #[allow(dead_code)] // kept: used in tests (edit_tests.rs); public entry point for callers without provider
    pub(super) fn move_cursor_with_selection(&mut self, new_cursor: usize, shift: bool) {
        let mut wrapper =
            super::provider::ArcVecDataProvider::from_arc(std::sync::Arc::clone(&self.data));
        self.move_cursor_with_selection_with(new_cursor, shift, &mut wrapper);
    }

    /// Legacy provider-less copy — operates directly on `self.data`.
    /// Kept on the surface for external callers; the in-frame
    /// keyboard handler uses [`Self::copy_selection_with`] which is
    /// provider-aware (streams the selected range through the active
    /// provider, so streaming-memory hosts get the right bytes).
    #[allow(dead_code)] // kept: documented public entry point for non-streaming callers; used in tests
    pub(super) fn copy_selection(&self) {
        let bytes = self.selected_bytes();
        if bytes.is_empty() {
            if self.cursor < self.data.len() {
                let s = format_bytes(
                    &[self.data[self.cursor]],
                    self.config.copy_format,
                    self.config.uppercase,
                );
                set_clipboard(&s);
            }
            return;
        }
        let s = format_bytes(bytes, self.config.copy_format, self.config.uppercase);
        set_clipboard(&s);
    }

    /// Provider-aware copy. Reads the selected range / cursor byte
    /// through the active provider so the clipboard reflects what
    /// the user is *seeing* on screen, even in the legacy
    /// [`HexViewer::render`] path where `self.data` is temporarily
    /// moved out into the wrapper provider. Caps the selection size
    /// at 64 MiB to avoid runaway clipboard payloads — the typical
    /// hex-dump copy targets a small region (struct field / packet
    /// fragment); anything bigger is almost certainly an unintended
    /// Ctrl+A gesture.
    pub(super) fn copy_selection_with(&self, provider: &mut dyn HexDataProvider) {
        const COPY_CAP: usize = 64 * 1024 * 1024;
        if self.selection.is_empty() {
            let len = super::draw::provider_len_usize(provider);
            if self.cursor < len {
                let mut buf = [0u8; 1];
                let n = provider.read(self.cursor as u64, &mut buf);
                if n > 0 {
                    let s = format_bytes(&buf[..n], self.config.copy_format, self.config.uppercase);
                    set_clipboard(&s);
                }
            }
            return;
        }
        let (lo, hi) = self.selection.ordered();
        let len = super::draw::provider_len_usize(provider);
        let lo = lo.min(len);
        let hi = hi.min(len);
        if hi <= lo {
            return;
        }
        let want = (hi - lo).min(COPY_CAP);
        let mut buf = vec![0u8; want];
        let n = provider.read(lo as u64, &mut buf);
        buf.truncate(n);
        let s = format_bytes(&buf, self.config.copy_format, self.config.uppercase);
        set_clipboard(&s);
    }
}
