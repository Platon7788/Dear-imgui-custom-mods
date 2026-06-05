//! Byte-pattern search over the instruction stream for `DisasmView`.
//!
//! Split out of `mod.rs` (audit session 043) to keep every file under
//! the 500-line ceiling. The `DisasmView` struct + its fields stay in
//! `mod.rs`; this file only carries an `impl DisasmView { ... }` block.

use super::*;

impl DisasmView {
    /// runs the wildcard-aware matcher
    /// ([`crate::hex_viewer::search::find_pattern_masked`]) and
    /// translates byte offsets back into instruction indices for
    /// row highlighting + step navigation.
    ///
    /// Patterns shorter than [`SEARCH_MIN_BYTES`] (5) are rejected —
    /// state is cleared and the function returns without scanning.
    /// Cross-instruction matches are supported (matches that span
    /// instruction boundaries cover every row they touch).
    pub(super) fn do_search(&mut self, provider: &dyn DisasmDataProvider) {
        let pattern = parse_hex_pattern_masked(&self.search_buf);
        if pattern.len() < SEARCH_MIN_BYTES {
            self.search_pattern.clear();
            self.search_match_starts.clear();
            self.search_match_set.clear();
            return;
        }

        let count = provider.instruction_count();
        // Build concat byte stream + a `(byte_offset,
        // global_instruction_idx)` table. Skipping `None`
        // instructions is mandatory for sparse / lazy providers
        // (they advertise `instruction_count` for the entire
        // address range but legitimately return `None` for
        // not-yet-decoded slots). The pair preserves the global
        // index so the offset → row mapping survives gaps.
        let mut data: Vec<u8> = Vec::with_capacity(count * 3);
        let mut entries: Vec<(usize, usize)> = Vec::with_capacity(count);
        for i in 0..count {
            if let Some(instr) = provider.instruction(i) {
                entries.push((data.len(), i));
                data.extend_from_slice(instr.bytes());
            }
        }

        let matches = find_pattern_masked(&data, &pattern);
        let plen = pattern.len();

        let mut starts: Vec<usize> = Vec::with_capacity(matches.len());
        let mut covered: BTreeSet<usize> = BTreeSet::new();
        for &offset in &matches {
            // `partition_point(|&(off, _)| off <= offset)` returns
            // the FIRST entry with `off > offset` — well-defined
            // last-le semantics even when entries share offsets
            // (which happens when an instruction has zero bytes —
            // never, in practice, but defensive). Use saturating
            // `pos - 1` to guard the impossible case where the
            // match starts before any entry.
            let pos = entries.partition_point(|&(off, _)| off <= offset);
            if pos == 0 {
                continue;
            }
            let start_pos = pos - 1;
            let end_offset = offset + plen;
            // First-ge semantics: entries[end_pos].0 is the first
            // offset that's at or beyond the end of the match.
            let end_pos = entries.partition_point(|&(off, _)| off < end_offset);

            starts.push(entries[start_pos].1);
            for entry in &entries[start_pos..end_pos] {
                covered.insert(entry.1);
            }
        }
        starts.sort_unstable();
        starts.dedup();

        self.search_pattern = pattern;
        self.search_match_starts = starts;
        self.search_match_set = covered;
        self.search_idx = 0;

        if let Some(&first_idx) = self.search_match_starts.first() {
            // Pre-search → first-match navigation pushes nav history
            // and sets the origin breadcrumb so the user can
            // `Alt+Left` back to where they were AND see the
            // pre-search row faintly highlighted while exploring
            // the matches. Self-navigation (search hit on current
            // row) skips both side effects.
            let pre_addr = self
                .cursor_idx
                .and_then(|i| provider.instruction(i))
                .map(|instr| instr.address());
            let dst_addr = provider.instruction(first_idx).map(|i| i.address());
            if let (Some(src), Some(dst)) = (pre_addr, dst_addr)
                && src != dst
            {
                self.nav.push(src);
                self.origin_addr = Some(src);
            }
            self.cursor_idx = Some(first_idx);
            self.scroll_to = Some(first_idx);
        }
    }

    /// Step to the next search match (wraps around).
    pub(super) fn search_next(&mut self) {
        if self.search_match_starts.is_empty() {
            return;
        }
        self.search_idx = (self.search_idx + 1) % self.search_match_starts.len();
        let idx = self.search_match_starts[self.search_idx];
        self.cursor_idx = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Step to the previous search match (wraps around).
    pub(super) fn search_prev(&mut self) {
        if self.search_match_starts.is_empty() {
            return;
        }
        self.search_idx = self
            .search_idx
            .checked_sub(1)
            .unwrap_or(self.search_match_starts.len() - 1);
        let idx = self.search_match_starts[self.search_idx];
        self.cursor_idx = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Format `addr` as a copy-friendly hex literal (`0x...`),
    /// honouring `address_width_64` + `uppercase`. Used by the
    /// address-gutter copy-on-double-click path and the "Copy
    /// Address" context-menu entry.
    pub(super) fn format_address_literal(&self, addr: u64) -> String {
        match (self.config.uppercase, self.config.address_width_64) {
            (true, true) => format!("0x{:016X}", addr),
            (false, true) => format!("0x{:016x}", addr),
            (true, false) => format!("0x{:08X}", addr),
            (false, false) => format!("0x{:08x}", addr),
        }
    }
}
