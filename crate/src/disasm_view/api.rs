//! Host-facing convenience selectors + bookmark API for `DisasmView`.
//!
//! Split out of `mod.rs` (audit session 043) to keep every file under
//! the 500-line ceiling. The `DisasmView` struct + its fields stay in
//! `mod.rs`; this file only carries an `impl DisasmView { ... }` block.

use super::*;

impl DisasmView {
    // ── Convenience selectors (host toolbar helpers) ─────────────────
    //
    // The five methods below let a host implement a "Top / Bottom /
    // Current IP / Breakpoint / cycle BPs" toolbar in pure
    // `if button { view.method() }` style — no manual scan-loop over
    // the provider. They are pure view-domain operations and don't
    // cross into the host's debugger backend (stepping, run/pause,
    // register/memory reads stay on the backend side; the view only
    // reflects whatever provider state `is_current()` /
    // `has_breakpoint()` reports).

    /// Find and select the instruction the provider marks as
    /// [`Instruction::is_current`] (typically the debugger's IP /
    /// program counter). Returns `true` when an IP row was found
    /// and selection moved, `false` otherwise (host can disable
    /// the corresponding toolbar button on `false`).
    pub fn select_current_ip(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        let hit = (0..count).find(|&i| {
            provider
                .instruction(i)
                .is_some_and(|instr| instr.is_current())
        });
        if let Some(i) = hit {
            self.select(i);
            return true;
        }
        false
    }

    /// Find and select the *first* instruction with a breakpoint
    /// (lowest index → lowest address in a sorted provider).
    /// Returns `true` when one was found.
    pub fn select_first_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        let hit = (0..count).find(|&i| {
            provider
                .instruction(i)
                .is_some_and(|instr| instr.has_breakpoint())
        });
        if let Some(i) = hit {
            self.select(i);
            return true;
        }
        false
    }

    /// Cycle to the next breakpoint **strictly after** the current
    /// cursor (or, if the cursor is past the last breakpoint, wraps
    /// to the first). Returns `true` when a breakpoint exists at
    /// all. Standard disassembler UX — the IDE-style "next BP" button.
    pub fn select_next_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        if count == 0 {
            return false;
        }
        // `+ 1` can wrap past `count` when the cursor is the last row;
        // `min(count)` keeps the forward half empty in that case so the
        // wrap-around half does the work — no out-of-range index reaches
        // the provider.
        let start = self
            .cursor_idx
            .map_or(0, |c| c.saturating_add(1))
            .min(count);
        let has_bp = |i: usize| {
            provider
                .instruction(i)
                .is_some_and(|instr| instr.has_breakpoint())
        };
        // Search forward from `start`, then wrap around to `0..start`.
        let hit = (start..count).chain(0..start).find(|&i| has_bp(i));
        if let Some(i) = hit {
            self.select(i);
            return true;
        }
        false
    }

    /// Cycle to the previous breakpoint **strictly before** the
    /// current cursor (wraps to the last). Symmetric to
    /// [`Self::select_next_breakpoint`].
    pub fn select_prev_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        if count == 0 {
            return false;
        }
        // Clamp to `count` so an out-of-range cursor (provider shrank
        // since the last select) still produces a valid scan window.
        let start = self.cursor_idx.unwrap_or(count).min(count);
        let has_bp = |i: usize| {
            provider
                .instruction(i)
                .is_some_and(|instr| instr.has_breakpoint())
        };
        // Search backward from `cursor-1`, then wrap around from the end.
        let hit = (0..start)
            .rev()
            .chain((start..count).rev())
            .find(|&i| has_bp(i));
        if let Some(i) = hit {
            self.select(i);
            return true;
        }
        false
    }

    /// Whether the back / forward address-history stack has anything
    /// to consume. Use these to render `<< Back` / `Fwd >>` toolbar
    /// buttons as disabled when there's nothing to walk to. Mirrors
    /// the corresponding `Alt+Left` / `Alt+Right` shortcut state.
    #[must_use]
    pub fn can_nav_back(&self) -> bool {
        self.nav.can_go_back()
    }

    /// See [`Self::can_nav_back`].
    #[must_use]
    pub fn can_nav_forward(&self) -> bool {
        self.nav.can_go_forward()
    }

    /// Address of the row under the cursor, or `None` when the view
    /// has no cursor / the cursor index doesn't resolve through the
    /// provider. Useful for status-bar `Addr: 0x…` displays and as
    /// the prefill value for a host-rendered "Goto address" input.
    #[must_use]
    pub fn cursor_address(&self, provider: &dyn DisasmDataProvider) -> Option<u64> {
        let i = self.cursor_idx?;
        provider.instruction(i).map(|instr| instr.address())
    }

    // ── Bookmarks (UI navigation aid, view-state) ───────────────────
    //
    // Bookmarks let the user mark "interesting" addresses for quick
    // recall — the gutter paints a coloured ring on bookmarked rows
    // (`colors.bookmark`), the right-click menu offers an
    // add/remove-toggle entry, and `Ctrl+B` toggles the bookmark on
    // the cursor row. Capacity is fixed at [`Self::MAX_BOOKMARKS`]
    // (64); calls past the cap silently no-op so the host can wire
    // a button without managing the limit.
    //
    // Bookmarks are *view-state*, not provider-state — they are an
    // editor-style navigation aid, not tied to a running-process
    // concept like a breakpoint. Hosts that need cross-session
    // persistence read the set via [`Self::bookmarks`] on shutdown
    // and replay through [`Self::add_bookmark`] on startup.

    /// Whether `addr` is currently bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, addr: u64) -> bool {
        self.bookmarks.contains(&addr)
    }

    /// Number of bookmarks currently set (`<=` [`Self::MAX_BOOKMARKS`]).
    #[must_use]
    pub fn bookmark_count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Read-only access to the full bookmark set, sorted by address.
    /// Use this for host-side save / export.
    #[must_use]
    pub fn bookmarks(&self) -> &BTreeSet<u64> {
        &self.bookmarks
    }

    /// Add `addr` to the bookmark set. Returns `true` when the
    /// address is bookmarked after the call (i.e. the operation
    /// succeeded **or** the address was already bookmarked); `false`
    /// only when the [`Self::MAX_BOOKMARKS`] cap is reached and
    /// `addr` wasn't already in the set.
    pub fn add_bookmark(&mut self, addr: u64) -> bool {
        if self.bookmarks.contains(&addr) {
            return true;
        }
        if self.bookmarks.len() >= Self::MAX_BOOKMARKS {
            return false;
        }
        self.bookmarks.insert(addr);
        true
    }

    /// Remove `addr` from the bookmark set. Returns `true` when an
    /// entry was removed, `false` when the address wasn't in the set.
    pub fn remove_bookmark(&mut self, addr: u64) -> bool {
        self.bookmarks.remove(&addr)
    }

    /// Toggle bookmark state on `addr`. Returns the **new** state
    /// (`true` = bookmarked after the call). When transitioning
    /// from off → on and the [`Self::MAX_BOOKMARKS`] cap is reached,
    /// returns `false` and leaves the set unchanged.
    pub fn toggle_bookmark(&mut self, addr: u64) -> bool {
        if self.bookmarks.contains(&addr) {
            self.bookmarks.remove(&addr);
            false
        } else {
            self.add_bookmark(addr)
        }
    }

    /// Drop every bookmark.
    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.clear();
    }

    /// Bulk-restore the bookmark set in one call (audit M1). Replaces
    /// the entire set with the input addresses, silently capped at
    /// [`Self::MAX_BOOKMARKS`] (oldest-by-sort-order win — `BTreeSet`
    /// iteration is sorted ascending). Returns the count actually
    /// stored, which equals `input.len()` when below the cap.
    ///
    /// Use case: the host saved bookmarks via [`Self::bookmarks`] in
    /// a previous session and now wants to restore them on startup
    /// without making 64 individual [`Self::add_bookmark`] calls.
    pub fn set_bookmarks<I: IntoIterator<Item = u64>>(&mut self, addrs: I) -> usize {
        self.bookmarks.clear();
        for addr in addrs.into_iter().take(Self::MAX_BOOKMARKS) {
            self.bookmarks.insert(addr);
        }
        self.bookmarks.len()
    }

    /// Drain the goto-address request emitted by the popup so the host
    /// can re-anchor the backing buffer when the user typed an address
    /// outside the currently decoded range. Returns `Some(addr)` once
    /// per popup commit, `None` otherwise.
    pub fn take_pending_goto_request(&mut self) -> Option<u64> {
        self.pending_goto_request.take()
    }
}
