//! Navigation: goto / history / function jumps / follow-at-cursor.
//!
//! Split out of `mod.rs` (audit session 043) to keep every file under
//! the 500-line ceiling. The `DisasmView` struct + its fields stay in
//! `mod.rs`; this file only carries an `impl DisasmView { ... }` block.

use super::*;

impl DisasmView {
    /// Scroll to and select the instruction at `addr`.
    ///
    /// Side effects on jump (when `addr != current cursor address`):
    /// - Pushes the source address onto the 64-entry nav history
    ///   (`Alt+Left` / `Alt+Right` walk back / forward).
    /// - Sets [`Self::origin_addr`] to the source so the previous
    ///   row paints with the soft "you came from here" breadcrumb.
    ///
    /// No-op when `addr` doesn't resolve through
    /// [`DisasmDataProvider::index_of_address`]. Self-jumps
    /// (target == current) skip the side effects so the breadcrumb
    /// doesn't land on the current row.
    pub fn goto_address(&mut self, addr: u64, provider: &dyn DisasmDataProvider) {
        let Some(idx) = provider.index_of_address(addr) else {
            return;
        };
        let old_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address());
        if let Some(old) = old_addr
            && old != addr
        {
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(idx);
    }

    /// Navigate back in address history. Pushes a breadcrumb at
    /// the current row before stepping back, so a subsequent
    /// `Alt+Right` lands on the same place visually.
    pub fn nav_back(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_back(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            if addr != current_addr {
                self.origin_addr = Some(current_addr);
            }
            self.select(idx);
        }
    }

    /// Navigate forward in address history. Symmetrical breadcrumb
    /// behaviour to [`Self::nav_back`].
    pub fn nav_forward(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_forward(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            if addr != current_addr {
                self.origin_addr = Some(current_addr);
            }
            self.select(idx);
        }
    }

    // ── Function navigation ──────────────────────────────────────────

    /// Jump to the first instruction of the function containing the
    /// cursor — uses [`find_function_start`]. The pre-jump address
    /// is pushed onto nav history (Alt+Left returns to it) AND
    /// recorded as [`Self::origin_addr`] so the source row paints
    /// with the soft breadcrumb. Self-jumps (already at start)
    /// skip both side effects. No-op when there's no cursor.
    pub fn jump_to_function_start(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let start = find_function_start(provider, cur);
        if start == cur {
            return;
        }
        if let Some(instr) = provider.instruction(cur) {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(start);
    }

    /// Jump to the last instruction of the function containing the
    /// cursor — uses [`find_function_end`]. Symmetrical breadcrumb +
    /// nav-history behaviour to [`Self::jump_to_function_start`].
    pub fn jump_to_function_end(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let end = find_function_end(provider, cur);
        if end == cur {
            return;
        }
        if let Some(instr) = provider.instruction(cur) {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(end);
    }

    /// Select every instruction from the cursor through the end of
    /// the function (inclusive). Cursor moves to the function-end
    /// instruction; selection range is `[cursor_at_call ..= end]`.
    /// Useful for "copy whole tail of this function" workflows.
    ///
    /// Pushes the original cursor address onto the nav history and
    /// records it as the origin breadcrumb (mirrors
    /// [`Self::jump_to_function_start`] / `_end`) so `Alt+Left`
    /// returns to the user's pre-select location and the source
    /// row paints the breadcrumb. No-op when there's no cursor.
    pub fn select_function(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let end = find_function_end(provider, cur);
        if cur != end
            && let Some(instr) = provider.instruction(cur)
        {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select_range(cur, end);
        self.cursor_idx = Some(end);
        self.sel_anchor = Some(cur);
        self.scroll_to = Some(end);
    }

    /// "Cheat-Engine-style" follow at cursor — navigate to whatever
    /// the current instruction points at:
    ///
    /// 1. If [`Instruction::branch_target`] is `Some`, jump there.
    ///    For targets not yet decoded, calls
    ///    [`DisasmDataProvider::decode_range`] once to give
    ///    streaming/lazy providers a chance to populate the
    ///    target window before re-checking. Static providers
    ///    (`VecDisasmProvider`) implement `decode_range` as a
    ///    no-op so the lazy retry is free for pre-loaded data.
    /// 2. Otherwise scan the operand string for a
    ///    [`tokens::TokenKind::Number`] that parses as an address —
    ///    same lazy-decode + retry treatment.
    ///
    ///    For `call` / `jmp` / `Jcc` rows the scanner deliberately
    ///    **ignores numbers inside `[...]`** — `call qword ptr
    ///    [rip+0x1234]` hides a displacement, not a call target;
    ///    chasing it lands the cursor on the wrong row (or, more
    ///    commonly, on nothing at all and the user concludes "double-
    ///    click is broken"). For non-branching rows (`mov rax,
    ///    [0x401000]`) the scanner still considers in-bracket numbers
    ///    so memory-pointer follow keeps working.
    ///
    /// Returns `true` when navigation actually happened. Pushes
    /// nav history + sets the origin breadcrumb in both paths.
    /// Returns `false` for unfollowable rows (no branch, no
    /// resolvable operand) so callers like the double-click
    /// handler can fall through to the edit-cell path.
    ///
    /// **Tip for host integrators**: for predictable follow on
    /// `call` / `jmp` / `Jcc` rows, set the branch target on the
    /// entry via `InstructionEntry::with_target`, or implement
    /// `Instruction::branch_target` for custom providers. Operand-
    /// scan is a best-effort fallback — it does not understand
    /// label syntax (`call kernel32!CreateFileW`),
    /// register-indirect (`call rax`), or symbolic relocations.
    pub fn follow_at_cursor(&mut self, provider: &mut dyn DisasmDataProvider) -> bool {
        matches!(
            self.follow_at_cursor_diagnostic(provider),
            FollowOutcome::Followed { .. },
        )
    }

    /// Diagnostic variant of [`Self::follow_at_cursor`] — returns a
    /// [`FollowOutcome`] explaining *why* navigation succeeded /
    /// failed. Hosts that want a status-line message (`"Cannot follow:
    /// target 0x4011A0 not in provider"`) call this; the boolean
    /// `follow_at_cursor` thin-wraps it for the legacy callers
    /// (double-click handler, Enter / Space key).
    pub fn follow_at_cursor_diagnostic(
        &mut self,
        provider: &mut dyn DisasmDataProvider,
    ) -> FollowOutcome {
        let Some(cur) = self.cursor_idx else {
            return FollowOutcome::NoCursor;
        };
        let (branch, flow, from_addr, operands_owned) = match provider.instruction(cur) {
            Some(instr) => (
                instr.branch_target(),
                instr.flow_kind(),
                instr.address(),
                instr.operands().to_string(),
            ),
            None => return FollowOutcome::NoCursor,
        };

        // Helper: try to navigate to `addr`, decoding lazily if the
        // target isn't yet in the provider. Returns `true` on
        // successful navigation. `goto_address` itself handles nav
        // history + origin breadcrumb when navigation lands.
        let try_goto = |this: &mut Self, addr: u64, provider: &mut dyn DisasmDataProvider| {
            if provider.index_of_address(addr).is_none() {
                // Lazy-decode a small window at the target — 32
                // instructions covers a typical function prologue
                // and gives the user something to look at on
                // arrival. Streaming providers honour this; the
                // built-in `VecDisasmProvider` is a no-op so this
                // is free for pre-loaded data.
                provider.decode_range(addr, 32);
            }
            if provider.index_of_address(addr).is_some() {
                this.goto_address(addr, provider);
                true
            } else {
                false
            }
        };

        if let Some(target) = branch {
            return if try_goto(self, target, provider) {
                FollowOutcome::Followed {
                    from: from_addr,
                    to: target,
                }
            } else {
                FollowOutcome::TargetOutsideProvider(target)
            };
        }

        // Operand-pointer scan — only when there's no branch_target.
        //
        // For Call/Jump rows, displacement numbers inside `[...]`
        // are NOT call targets (they're memory disp); chasing them
        // is the most common cause of "follow appears broken" on
        // hosts that forgot `.with_target(...)`. Track bracket depth
        // and skip in-bracket numbers for branching flow kinds; for
        // non-branching rows (`mov rax, [0x401000]`) keep the old
        // behaviour so memory-pointer follow still works.
        let prefer_outside_brackets = matches!(flow, FlowKind::Call | FlowKind::Jump);
        let mut bracket_depth: i32 = 0;
        for tok in tokens::OperandTokenizer::new(&operands_owned) {
            if tok.kind == tokens::TokenKind::Memory {
                match tok.text {
                    "[" => {
                        bracket_depth = bracket_depth.saturating_add(1);
                        continue;
                    }
                    "]" => {
                        bracket_depth = (bracket_depth - 1).max(0);
                        continue;
                    }
                    _ => continue, // size keywords like "qword"/"ptr"
                }
            }
            if tok.kind == tokens::TokenKind::Number
                && !(prefer_outside_brackets && bracket_depth > 0)
                && let Some(addr) = parse_operand_number(tok.text)
                && try_goto(self, addr, provider)
            {
                return FollowOutcome::Followed {
                    from: from_addr,
                    to: addr,
                };
            }
        }
        FollowOutcome::NoTargetAndNoNumber
    }
}

// ── Function-boundary detection ──────────────────────────────────────────────
//
// Heuristic — the trait gives us per-instruction `flow_kind()` but no
// CFG metadata. We use `FlowKind::Return` as the canonical function
// terminator (RET / IRET / RETF / RETN). Tail calls (`jmp some_func`
// at end of function) are NOT detected — a real disassembler would
// need provider-supplied function-boundary metadata for that. Padding
// (`Nop` / INT3) between functions folds into whichever side it
// neighbours; acceptable trade-off for a heuristic.

/// Return the index of the instruction that *ends* the function
/// containing `idx` — first [`FlowKind::Return`] at or after `idx`.
/// Returns `count - 1` (last decoded instruction) when no `Return`
/// is found before the buffer ends.
///
/// Empty providers return `0`. `idx` is clamped to `count - 1` first
/// so out-of-range cursors land at the buffer tail rather than
/// panicking.
#[must_use]
pub fn find_function_end(provider: &dyn DisasmDataProvider, idx: usize) -> usize {
    let count = provider.instruction_count();
    if count == 0 {
        return 0;
    }
    let start = idx.min(count - 1);
    for i in start..count {
        if let Some(instr) = provider.instruction(i)
            && instr.flow_kind() == FlowKind::Return
        {
            return i;
        }
    }
    count - 1
}

/// Return the index of the instruction that *starts* the function
/// containing `idx` — instruction immediately after the previous
/// [`FlowKind::Return`] boundary, or `0` if no boundary exists
/// before `idx`.
///
/// When the cursor sits ON a `Return`, we still walk strictly
/// *backward* from `cur - 1` so the function this RET belongs to
/// (not the one it ends) is the one returned. Empty providers
/// return `0`.
#[must_use]
pub fn find_function_start(provider: &dyn DisasmDataProvider, idx: usize) -> usize {
    let count = provider.instruction_count();
    if count == 0 {
        return 0;
    }
    let cur = idx.min(count - 1);
    if cur == 0 {
        return 0;
    }
    let mut i = cur;
    while i > 0 {
        i -= 1;
        if let Some(instr) = provider.instruction(i)
            && instr.flow_kind() == FlowKind::Return
        {
            // Function starts at the instruction immediately after
            // the previous RET. `i + 1` is always `<= cur` (we
            // entered the loop with `cur > 0` and `i` started at
            // `cur - 1`), so no clamp needed — the historic
            // `(i + 1).min(count - 1)` was a bogus cap that
            // incorrectly returned the previous-function RET when
            // `cur` was the last instruction in the buffer.
            return i + 1;
        }
    }
    0
}

// ── Free helpers ─────────────────────────────────────────────────────────────

/// Outcome of a [`DisasmView::follow_at_cursor_diagnostic`] call —
/// every reason the gesture could succeed or quietly fail. Hosts use
/// this to surface a status-line / toast hint ("Cannot follow:
/// target 0x4011A0 not yet decoded") instead of leaving the user
/// guessing why the double-click "did nothing".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowOutcome {
    /// Navigation happened — `from` was the row's address, `to` is
    /// the address the cursor landed on.
    Followed {
        /// Source row address.
        from: u64,
        /// Destination row address.
        to: u64,
    },
    /// No row is selected (`cursor_idx == None`).
    NoCursor,
    /// The provider supplied a `branch_target`, but
    /// [`provider::DisasmDataProvider::index_of_address`] still
    /// returns `None` after a lazy `decode_range` retry. Typical
    /// causes: target is outside the loaded view of a streaming
    /// provider, or the host built a `VecDisasmProvider` whose
    /// `decode_range` is a no-op and forgot to populate the target
    /// row.
    TargetOutsideProvider(u64),
    /// The row has no `branch_target` AND no
    /// [`tokens::TokenKind::Number`] in the operand string that
    /// resolves to an existing instruction. Typical for
    /// `call rax`, `call kernel32!CreateFileW`, or any
    /// register-indirect / symbolic-label form. The host should
    /// either set `branch_target` directly via
    /// [`provider::InstructionEntry::with_target`], or accept that
    /// these forms can only be followed at runtime (after the
    /// indirect resolves).
    NoTargetAndNoNumber,
}

impl FollowOutcome {
    /// Convenience: did navigation actually happen?
    #[inline]
    #[must_use]
    pub fn is_followed(&self) -> bool {
        matches!(self, FollowOutcome::Followed { .. })
    }
}

/// Parse a single operand token classified as
/// [`tokens::TokenKind::Number`] into an absolute `u64` address.
/// Accepts:
///   - `0x...` / `0X...` hex prefix
///   - `...h` / `...H` MASM/iced-x86 hex suffix
///   - bare decimal
///
/// Returns `None` for unparseable text. Used by
/// [`DisasmView::follow_at_cursor`] to chase numeric-immediate
/// pointers that the provider didn't tag as branch targets.
pub(super) fn parse_operand_number(s: &str) -> Option<u64> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    if s.len() > 1
        && (s.ends_with('h') || s.ends_with('H'))
        && s[..s.len() - 1].chars().all(|c| c.is_ascii_hexdigit())
    {
        return u64::from_str_radix(&s[..s.len() - 1], 16).ok();
    }
    s.parse::<u64>().ok()
}

/// Parse a goto-popup / API-supplied address string.
///
/// Acceptance order (first match wins):
/// 1. **`0x` / `0X` prefix** → unconditional hex parse.
/// 2. **Contains a hex letter (`a–f` / `A–F`)** → unambiguous hex, parse as base 16.
/// 3. Otherwise → parse as base 10.
///
/// The previous heuristic accepted hex on length alone (`s.len() > 4` with
/// any hex digits) which made `"4080"` (4 chars) decimal but `"40810"`
/// (5 chars) hex — a confusing length-cliff. Now decimal is the default
/// for all-digit input; hex requires either an explicit `0x` prefix or
/// at least one `a–f` letter.
pub(super) fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    let has_hex_letter = s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    if has_hex_letter && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u64::from_str_radix(s, 16).ok();
    }
    s.parse::<u64>().ok()
}
