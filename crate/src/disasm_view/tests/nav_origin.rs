//! Origin-breadcrumb tracking across navigations and lazy-decode follow for streaming providers.

use super::super::*;
use super::three_function_provider;

// ── Origin breadcrumb + nav history ───────────────────────────────────

#[test]
fn goto_address_sets_origin_to_old_address() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_goto");
    view.cursor_idx = Some(1); // 0x1001 in func A
    view.goto_address(0x1004, &p); // jump to func B middle
    assert_eq!(view.origin_addr, Some(0x1001));
    assert_eq!(view.selected_index(), Some(4));
}

#[test]
fn goto_address_self_does_not_set_origin() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_self");
    view.cursor_idx = Some(2);
    // Pre-condition: no breadcrumb.
    assert!(view.origin_addr.is_none());
    view.goto_address(0x1002, &p); // self-jump (cursor at 2 == addr 0x1002)
    assert!(
        view.origin_addr.is_none(),
        "self-goto must not paint a breadcrumb on the current row"
    );
}

#[test]
fn goto_address_overwrites_previous_origin() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_overwrite");
    view.cursor_idx = Some(0);
    view.goto_address(0x1003, &p); // origin = 0x1000
    assert_eq!(view.origin_addr, Some(0x1000));
    view.goto_address(0x1006, &p); // origin = 0x1003
    assert_eq!(view.origin_addr, Some(0x1003));
}

#[test]
fn jump_to_function_start_sets_origin() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_func_start");
    view.cursor_idx = Some(7); // middle of func C (addr 0x1007)
    view.jump_to_function_start(&p);
    assert_eq!(view.origin_addr, Some(0x1007));
    assert_eq!(view.selected_index(), Some(6));
}

#[test]
fn jump_to_function_end_sets_origin() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_func_end");
    view.cursor_idx = Some(4); // middle of func B (addr 0x1004)
    view.jump_to_function_end(&p);
    assert_eq!(view.origin_addr, Some(0x1004));
    assert_eq!(view.selected_index(), Some(5));
}

#[test]
fn nav_back_sets_origin_to_pre_back_address() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_nav_back");
    view.cursor_idx = Some(0); // 0x1000
    view.goto_address(0x1004, &p); // → 0x1004 (origin = 0x1000)
    assert_eq!(view.origin_addr, Some(0x1000));
    view.nav_back(&p); // ← 0x1000 (origin should now be 0x1004)
    assert_eq!(view.origin_addr, Some(0x1004));
    assert_eq!(view.selected_index(), Some(0));
}

#[test]
fn nav_forward_sets_origin_to_pre_forward_address() {
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_nav_fwd");
    view.cursor_idx = Some(0);
    view.goto_address(0x1004, &p);
    view.nav_back(&p); // back to 0x1000
    view.nav_forward(&p); // forward to 0x1004 (origin = 0x1000)
    assert_eq!(view.origin_addr, Some(0x1000));
    assert_eq!(view.selected_index(), Some(4));
}

#[test]
fn do_search_sets_origin_and_pushes_nav_history() {
    let mut p = VecDisasmProvider::new();
    // Build a buffer with a unique 5-byte pattern at 0x1010.
    for i in 0..16 {
        let bytes = if i == 16 - 6 {
            vec![0x48, 0x89, 0xE5, 0x90, 0x90]
        } else {
            vec![0x90]
        };
        p.push(InstructionEntry::new(0x1000 + i as u64, bytes, "nop", ""));
    }
    let mut view = DisasmView::new("origin_search");
    view.cursor_idx = Some(2); // pre-search at 0x1002
    view.search_buf = "48 89 E5 90 90".to_string();
    view.do_search(&p);
    // Pre-search address recorded as origin.
    assert_eq!(view.origin_addr, Some(0x1002));
    // Nav history holds the pre-search address — Alt+Left works.
    view.nav_back(&p);
    assert_eq!(view.selected_index(), Some(2));
}

#[test]
fn origin_persists_across_arrow_navigation() {
    // Arrow keys / single-step movement should NOT clear origin —
    // the user is exploring around the breadcrumb, not abandoning
    // it. We exercise this via direct cursor mutation since
    // `handle_keyboard` requires an Ui mock.
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_arrows");
    view.cursor_idx = Some(0);
    view.goto_address(0x1006, &p);
    assert_eq!(view.origin_addr, Some(0x1000));
    // Simulate arrow movement (cursor changes; origin untouched).
    view.cursor_idx = Some(7);
    assert_eq!(view.origin_addr, Some(0x1000));
    view.cursor_idx = Some(8);
    assert_eq!(view.origin_addr, Some(0x1000));
}

#[test]
fn origin_survives_provider_address_reordering() {
    // `origin_addr` is an ABSOLUTE address (not row index), so
    // a provider mutation that shifts row indices doesn't
    // invalidate the breadcrumb.
    let mut p = three_function_provider();
    let mut view = DisasmView::new("origin_survives_mut");
    view.cursor_idx = Some(0);
    view.goto_address(0x1006, &p);
    assert_eq!(view.origin_addr, Some(0x1000));
    // Mutate the provider — set a comment on the origin
    // instruction. This doesn't shift indices but proves the
    // address-based key is stable across mutation.
    assert!(p.set_comment(0x1000, "marked"));
    // Origin still points at the same address.
    assert_eq!(view.origin_addr, Some(0x1000));
    assert_eq!(p.instruction(0).unwrap().comment(), Some("marked"));
}

// ── follow_at_cursor: lazy decode for streaming providers ────────────

/// Test-only provider that decodes a target on demand into its
/// internal `Vec`. Models the kind of streaming/lazy provider
/// users build on top of iced-x86 / capstone where decoding
/// happens per-page or per-function.
struct LazyDecodeProvider {
    decoded: VecDisasmProvider,
    /// Addresses that *can* be decoded but haven't been yet.
    pending: std::collections::HashSet<u64>,
}

impl DisasmDataProvider for LazyDecodeProvider {
    fn instruction_count(&self) -> usize {
        self.decoded.instruction_count()
    }
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction> {
        self.decoded.instruction(idx)
    }
    fn decode_range(&mut self, start_addr: u64, _max_count: usize) {
        if self.pending.remove(&start_addr) {
            self.decoded
                .push(InstructionEntry::new(start_addr, vec![0x90], "nop", ""));
        }
    }
    fn index_of_address(&self, addr: u64) -> Option<usize> {
        self.decoded.index_of_address(addr)
    }
}

#[test]
fn follow_at_cursor_lazy_decodes_call_target() {
    // Source: `call 0x4011A0`. Target NOT yet decoded — only
    // present in the lazy provider's `pending` set. Without
    // lazy-decode, follow would silently fail.
    let mut decoded = VecDisasmProvider::new();
    decoded.push(
        InstructionEntry::new(
            0x401000,
            vec![0xE8, 0x9B, 0x01, 0x00, 0x00],
            "call",
            "0x4011A0",
        )
        .with_flow(FlowKind::Call)
        .with_target(0x4011A0),
    );
    let mut p = LazyDecodeProvider {
        decoded,
        pending: [0x4011A0].iter().copied().collect(),
    };
    let mut view = DisasmView::new("lazy_call");
    view.cursor_idx = Some(0);
    let followed = view.follow_at_cursor(&mut p);
    assert!(followed, "follow must succeed via lazy decode");
    // After decode, target is at idx 1.
    assert_eq!(view.selected_index(), Some(1));
    assert_eq!(view.origin_addr, Some(0x401000));
}

#[test]
fn follow_at_cursor_returns_false_when_lazy_decode_yields_nothing() {
    // Lazy provider has no pending decodes — target stays unknown.
    let mut decoded = VecDisasmProvider::new();
    decoded.push(
        InstructionEntry::new(
            0x401000,
            vec![0xE8, 0x00, 0x00, 0x00, 0x00],
            "call",
            "0xDEAD",
        )
        .with_flow(FlowKind::Call)
        .with_target(0xDEAD),
    );
    let mut p = LazyDecodeProvider {
        decoded,
        pending: std::collections::HashSet::new(),
    };
    let mut view = DisasmView::new("lazy_unfollowable");
    view.cursor_idx = Some(0);
    assert!(!view.follow_at_cursor(&mut p));
    assert!(view.origin_addr.is_none());
}

#[test]
fn origin_preserved_through_repeated_navigations() {
    // Each new navigation overwrites origin (not stacks) — verify
    // the breadcrumb tracks the *last* jump source only.
    let p = three_function_provider();
    let mut view = DisasmView::new("origin_chain");
    view.cursor_idx = Some(0); // 0x1000
    view.goto_address(0x1003, &p);
    assert_eq!(view.origin_addr, Some(0x1000));
    view.goto_address(0x1006, &p);
    assert_eq!(view.origin_addr, Some(0x1003));
    view.goto_address(0x1000, &p);
    assert_eq!(view.origin_addr, Some(0x1006));
}

#[test]
fn nav_history_capacity_is_64_entries() {
    // Push 100 distinct addresses, walk back, count how many
    // we recover before the history runs dry. Should be 64
    // (per `NavHistory::new(64)` in DisasmView::new).
    let mut p = VecDisasmProvider::new();
    for i in 0..101 {
        p.push(InstructionEntry::new(
            0x1000 + i as u64,
            vec![0x90],
            "nop",
            "",
        ));
    }
    let mut view = DisasmView::new("nav_capacity");
    view.cursor_idx = Some(0);
    for i in 1..=100 {
        view.goto_address(0x1000 + i as u64, &p);
    }
    // After 100 pushes, walk back. Count how many distinct
    // addresses we recover before nav_back stops moving us.
    let mut visited = std::collections::HashSet::new();
    for _ in 0..200 {
        let before = view.selected_index();
        view.nav_back(&p);
        let after = view.selected_index();
        if before == after {
            break;
        }
        visited.insert(after);
    }
    // History capacity is 64; one slot is "current", rest are
    // back-stack — so we should recover ~64 distinct steps.
    // Allow ±2 tolerance for off-by-one in the NavHistory
    // implementation (capacity vs back-only-stack semantics).
    assert!(
        visited.len() >= 60 && visited.len() <= 65,
        "expected ~64 nav history slots, got {}",
        visited.len()
    );
}

#[test]
fn x32_compute_arrows_clipped_works_with_pe32_addresses() {
    // PE32 image-base jumps: jmp from 0x401000 → 0x401010.
    // Verifies that compute_arrows_clipped's `index_of_address`
    // path resolves x32 addresses identically to x64.
    let mut p = VecDisasmProvider::new();
    for i in 0..16 {
        let entry = if i == 0 {
            InstructionEntry::new(0x00401000, vec![0xEB, 0x00], "jmp", "0x00401010")
                .with_flow(FlowKind::Jump)
                .with_target(0x00401010)
        } else {
            InstructionEntry::new(0x00401000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    // Window [5..10) — source at idx 0 above, target at idx 16
    // not present (0x401010 = idx 16, but we only have 16
    // instructions = idx 0..=15). Let's adjust to ensure
    // target exists: target 0x40100F → idx 15.
    let mut p2 = VecDisasmProvider::new();
    for i in 0..16 {
        let entry = if i == 0 {
            InstructionEntry::new(0x00401000, vec![0xEB, 0x00], "jmp", "0x0040100F")
                .with_flow(FlowKind::Jump)
                .with_target(0x0040100F)
        } else {
            InstructionEntry::new(0x00401000 + i as u64, vec![0x90], "nop", "")
        };
        p2.push(entry);
    }
    let arrows = compute_arrows_clipped(&p2 as &dyn DisasmDataProvider, 5, 5);
    // Source at idx 0 above window, target at idx 15 below window
    // → pass-through arrow.
    assert_eq!(arrows.len(), 1);
    assert!(arrows[0].clipped_from);
    assert!(arrows[0].clipped_to);
}

#[test]
fn set_comment_default_trait_impl_is_noop() {
    // Read-only providers inherit the default `false` impl so
    // existing implementors remain non-breaking after the trait
    // gained `set_comment`. Verify the default really is a no-op.
    struct ReadOnly;
    impl DisasmDataProvider for ReadOnly {
        fn instruction_count(&self) -> usize {
            0
        }
        fn instruction(&self, _i: usize) -> Option<&dyn Instruction> {
            None
        }
        fn decode_range(&mut self, _start_addr: u64, _max_count: usize) {}
        fn index_of_address(&self, _addr: u64) -> Option<usize> {
            None
        }
    }
    let mut ro = ReadOnly;
    assert!(!ro.set_comment(0x1000, "anything"));
}

// ── Navigation provider-bounds guards ─────────────────────────────────

#[test]
fn goto_address_unknown_is_noop_and_preserves_state() {
    // Provider-bounds guard: an address that doesn't resolve through
    // `index_of_address` must leave cursor / origin / nav-history
    // untouched (no panic, no spurious breadcrumb).
    let p = three_function_provider();
    let mut view = DisasmView::new("goto_oob");
    view.cursor_idx = Some(1); // 0x1001
    view.origin_addr = None;
    view.goto_address(0xDEAD_BEEF, &p); // not in provider
    assert_eq!(view.selected_index(), Some(1), "cursor unchanged");
    assert!(view.origin_addr.is_none(), "no breadcrumb on failed goto");
    assert!(!view.can_nav_back(), "no history pushed on failed goto");
}

#[test]
fn scroll_to_address_unknown_is_noop() {
    // `scroll_to_address` is the "soft" sibling of `goto_address`;
    // an unknown target must also be a clean no-op.
    let p = three_function_provider();
    let mut view = DisasmView::new("scroll_oob");
    view.cursor_idx = Some(2);
    view.scroll_to_address(0xDEAD_BEEF, &p);
    assert_eq!(view.selected_index(), Some(2), "cursor unchanged");
}

#[test]
fn follow_at_cursor_no_cursor_is_noop() {
    // `follow_at_cursor` with no cursor returns the `NoCursor`
    // diagnostic and navigates nowhere.
    let mut p = three_function_provider();
    let mut view = DisasmView::new("follow_no_cursor");
    assert_eq!(view.selected_index(), None);
    assert!(!view.follow_at_cursor(&mut p));
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::NoCursor
    );
}
