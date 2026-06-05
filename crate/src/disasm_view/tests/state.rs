//! Constructor, providers, breakpoint toggle, convenience selectors, bookmark API, and nav-history state.

use super::super::*;
use super::sample_provider;

#[test]
fn test_new_view() {
    let view = DisasmView::new("test");
    assert_eq!(view.selected_index(), None);
    assert!(!view.is_focused());
}

#[test]
fn test_instruction_entry() {
    let entry = InstructionEntry::new(0x1000, vec![0x90], "nop", "");
    assert_eq!(entry.address(), 0x1000);
    assert_eq!(entry.bytes(), &[0x90]);
    assert_eq!(entry.mnemonic(), "nop");
    assert_eq!(entry.operands(), "");
    assert_eq!(entry.flow_kind(), FlowKind::Normal);

    let entry2 = InstructionEntry::new(0x2000, vec![0xEB, 0x10], "jmp", "0x2010")
        .with_flow(FlowKind::Jump)
        .with_target(0x2010)
        .with_comment("loop top");
    assert_eq!(entry2.flow_kind(), FlowKind::Jump);
    assert_eq!(entry2.branch_target(), Some(0x2010));
    assert_eq!(entry2.comment(), Some("loop top"));
}

#[test]
fn test_vec_provider() {
    let p = sample_provider();
    assert_eq!(p.instruction_count(), 8);
    assert!(p.instruction(0).is_some());
    assert!(p.instruction(8).is_none());
    assert_eq!(p.index_of_address(0x401004), Some(2));
    assert_eq!(p.index_of_address(0xFF0000), None);
}

#[test]
fn test_toggle_breakpoint() {
    let mut p = sample_provider();
    p.toggle_breakpoint(0x401000);
    assert!(p.instruction(0).unwrap().has_breakpoint());
    p.toggle_breakpoint(0x401000);
    assert!(!p.instruction(0).unwrap().has_breakpoint());
}

// ── Convenience selectors (host toolbar API, session 032) ──────────

#[test]
fn select_current_ip_finds_marked_row() {
    let mut p = sample_provider();
    // Mark idx 3 (the `call`) as the current IP.
    p.instructions_mut()[3].current = true;

    let mut view = DisasmView::new("t");
    assert!(view.select_current_ip(&p));
    assert_eq!(view.selected_index(), Some(3));
}

#[test]
fn select_current_ip_returns_false_when_no_ip() {
    let p = sample_provider(); // no `current` flag set
    let mut view = DisasmView::new("t");
    assert!(!view.select_current_ip(&p));
    assert_eq!(view.selected_index(), None);
}

#[test]
fn select_first_breakpoint_finds_lowest_index() {
    let mut p = sample_provider();
    p.toggle_breakpoint(0x40100D); // idx 4
    p.toggle_breakpoint(0x401004); // idx 2

    let mut view = DisasmView::new("t");
    assert!(view.select_first_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(2), "lowest-index BP wins");
}

#[test]
fn select_first_breakpoint_returns_false_when_none() {
    let p = sample_provider();
    let mut view = DisasmView::new("t");
    assert!(!view.select_first_breakpoint(&p));
}

#[test]
fn select_next_breakpoint_cycles_forward_with_wraparound() {
    let mut p = sample_provider();
    p.toggle_breakpoint(0x401001); // idx 1
    p.toggle_breakpoint(0x401010); // idx 5

    let mut view = DisasmView::new("t");
    view.select(3); // cursor between the two BPs

    // Next from idx 3 → idx 5.
    assert!(view.select_next_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(5));
    // Next from idx 5 → wraps to idx 1.
    assert!(view.select_next_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn select_prev_breakpoint_cycles_backward_with_wraparound() {
    let mut p = sample_provider();
    p.toggle_breakpoint(0x401001); // idx 1
    p.toggle_breakpoint(0x401010); // idx 5

    let mut view = DisasmView::new("t");
    view.select(3);
    // Prev from idx 3 → idx 1.
    assert!(view.select_prev_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(1));
    // Prev from idx 1 → wraps to idx 5.
    assert!(view.select_prev_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(5));
}

#[test]
fn select_next_breakpoint_from_last_row_wraps_to_first() {
    // Regression: cursor on the LAST row makes `cursor + 1 == count`,
    // which `.min(count)` keeps in range — the forward half is empty
    // and the wrap-around half finds the only BP without ever calling
    // `provider.instruction(count)`.
    let mut p = sample_provider();
    p.toggle_breakpoint(0x401001); // idx 1 (only BP)
    let mut view = DisasmView::new("t");
    view.select(p.instruction_count() - 1); // last row
    assert!(view.select_next_breakpoint(&p));
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn breakpoint_cycle_clamps_out_of_range_cursor() {
    // Regression (audit, session 043): when the provider shrinks
    // between frames the stale `cursor_idx` can sit past the new
    // tail. The `.min(count)` clamp in `select_next/prev_breakpoint`
    // keeps the scan window valid so the only BP is still found
    // (and no out-of-range index reaches the provider).
    let mut p = sample_provider();
    p.toggle_breakpoint(0x401004); // idx 2 (only BP)
    let mut view = DisasmView::new("t");
    view.select(999); // cursor far past the 8-row tail

    assert!(
        view.select_next_breakpoint(&p),
        "next-BP must find the BP despite an OOB cursor"
    );
    assert_eq!(view.selected_index(), Some(2));

    view.select(999);
    assert!(
        view.select_prev_breakpoint(&p),
        "prev-BP must find the BP despite an OOB cursor"
    );
    assert_eq!(view.selected_index(), Some(2));
}

#[test]
fn breakpoint_selectors_no_op_on_empty_provider() {
    // Bounds guard: an empty provider must make every breakpoint
    // selector return `false` without panicking on `count - 1`.
    let p = VecDisasmProvider::new();
    let mut view = DisasmView::new("t");
    assert!(!view.select_first_breakpoint(&p));
    assert!(!view.select_next_breakpoint(&p));
    assert!(!view.select_prev_breakpoint(&p));
    assert!(!view.select_current_ip(&p));
    assert_eq!(view.selected_index(), None);
}

#[test]
fn can_nav_back_forward_track_history_state() {
    let p = sample_provider();
    let mut view = DisasmView::new("t");
    // Empty history at construction.
    assert!(!view.can_nav_back());
    assert!(!view.can_nav_forward());

    // First goto seeds the back stack (origin → push).
    view.goto_address(0x401000, &p);
    // Still nothing on the back stack — first selection has no
    // prior cursor to push.
    assert!(!view.can_nav_back());

    view.goto_address(0x40100D, &p);
    assert!(view.can_nav_back(), "second goto must populate back");
    assert!(!view.can_nav_forward());

    view.nav_back(&p);
    assert!(view.can_nav_forward(), "back must populate forward");
}

#[test]
fn cursor_address_matches_selected_instruction() {
    let p = sample_provider();
    let mut view = DisasmView::new("t");
    assert_eq!(view.cursor_address(&p), None);

    view.select(3);
    assert_eq!(view.cursor_address(&p), Some(0x401008));
}

// ── Bookmarks ────────────────────────────────────────────────────

#[test]
fn bookmark_default_empty() {
    let view = DisasmView::new("t");
    assert_eq!(view.bookmark_count(), 0);
    assert!(view.bookmarks().is_empty());
    assert!(!view.is_bookmarked(0x401000));
}

#[test]
fn add_bookmark_inserts_and_is_idempotent() {
    let mut view = DisasmView::new("t");
    assert!(view.add_bookmark(0x401000));
    assert!(view.is_bookmarked(0x401000));
    assert_eq!(view.bookmark_count(), 1);
    // Adding the same address again still returns true (idempotent)
    // and doesn't duplicate.
    assert!(view.add_bookmark(0x401000));
    assert_eq!(view.bookmark_count(), 1);
}

#[test]
fn add_bookmark_capped_at_max() {
    let mut view = DisasmView::new("t");
    for i in 0..DisasmView::MAX_BOOKMARKS as u64 {
        assert!(view.add_bookmark(0x400000 + i));
    }
    assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS);
    // The 65th unique address must fail without mutating the set.
    assert!(!view.add_bookmark(0x4FFFFF));
    assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS);
    assert!(!view.is_bookmarked(0x4FFFFF));
}

#[test]
fn remove_bookmark_returns_true_when_present() {
    let mut view = DisasmView::new("t");
    view.add_bookmark(0x401000);
    assert!(view.remove_bookmark(0x401000));
    assert!(!view.is_bookmarked(0x401000));
    // Subsequent removal of the same address returns false.
    assert!(!view.remove_bookmark(0x401000));
}

#[test]
fn toggle_bookmark_round_trip() {
    let mut view = DisasmView::new("t");
    // off → on
    assert!(view.toggle_bookmark(0x401000));
    assert!(view.is_bookmarked(0x401000));
    // on → off
    assert!(!view.toggle_bookmark(0x401000));
    assert!(!view.is_bookmarked(0x401000));
}

#[test]
fn toggle_bookmark_at_cap_returns_false_for_new_address() {
    let mut view = DisasmView::new("t");
    for i in 0..DisasmView::MAX_BOOKMARKS as u64 {
        view.add_bookmark(0x400000 + i);
    }
    // New address at cap → toggle on must fail.
    assert!(!view.toggle_bookmark(0x4FFFFF));
    assert!(!view.is_bookmarked(0x4FFFFF));
    // Existing address must still toggle off correctly.
    assert!(!view.toggle_bookmark(0x400000));
    assert!(!view.is_bookmarked(0x400000));
    assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS - 1);
}

#[test]
fn clear_bookmarks_empties_set() {
    let mut view = DisasmView::new("t");
    view.add_bookmark(0x401000);
    view.add_bookmark(0x401004);
    view.add_bookmark(0x401010);
    assert_eq!(view.bookmark_count(), 3);
    view.clear_bookmarks();
    assert_eq!(view.bookmark_count(), 0);
    assert!(view.bookmarks().is_empty());
}

#[test]
fn show_bookmarks_default_is_true_independently_of_breakpoints() {
    // C1 from session 034 audit: bookmark visibility was once
    // gated inside the `show_breakpoints` block, so disabling
    // breakpoints would silently hide bookmarks too. Pin the
    // independent default flags.
    let cfg = super::DisasmViewConfig::default();
    assert!(cfg.show_breakpoints);
    assert!(cfg.show_bookmarks);
}
