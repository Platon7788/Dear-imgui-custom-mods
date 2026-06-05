//! Function-boundary heuristic, follow-at-cursor (branch target / operand scan / diagnostics), and `parse_operand_number`.

use super::super::nav::parse_operand_number;
use super::super::*;
use super::three_function_provider;

// ── Function-boundary heuristic ──────────────────────────────────────

#[test]
fn find_function_start_returns_zero_for_first_function() {
    let p = three_function_provider();
    assert_eq!(find_function_start(&p, 0), 0);
    assert_eq!(find_function_start(&p, 1), 0);
    assert_eq!(find_function_start(&p, 2), 0);
}

#[test]
fn find_function_start_returns_post_ret_index() {
    let p = three_function_provider();
    // Cursor in func B → start should be index 3 (right after func A's RET).
    assert_eq!(find_function_start(&p, 3), 3);
    assert_eq!(find_function_start(&p, 4), 3);
    assert_eq!(find_function_start(&p, 5), 3);
    // Cursor in func C → start should be index 6.
    assert_eq!(find_function_start(&p, 6), 6);
    assert_eq!(find_function_start(&p, 8), 6);
}

#[test]
fn find_function_end_returns_first_ret_at_or_after_cursor() {
    let p = three_function_provider();
    // Cursor in func A → end at index 2 (the RET).
    assert_eq!(find_function_end(&p, 0), 2);
    assert_eq!(find_function_end(&p, 1), 2);
    assert_eq!(find_function_end(&p, 2), 2);
    // Cursor in func B → end at index 5.
    assert_eq!(find_function_end(&p, 3), 5);
    assert_eq!(find_function_end(&p, 5), 5);
}

#[test]
fn find_function_end_returns_last_when_no_ret_after() {
    // No-RET tail — end clamps to last instruction.
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(0x2000, vec![0x90], "nop", ""));
    p.push(InstructionEntry::new(0x2001, vec![0x90], "nop", ""));
    p.push(InstructionEntry::new(0x2002, vec![0x90], "nop", ""));
    assert_eq!(find_function_end(&p, 0), 2);
}

#[test]
fn find_function_helpers_handle_empty_provider() {
    let p = VecDisasmProvider::new();
    assert_eq!(find_function_start(&p, 0), 0);
    assert_eq!(find_function_end(&p, 0), 0);
    assert_eq!(find_function_start(&p, 999), 0);
    assert_eq!(find_function_end(&p, 999), 0);
}

#[test]
fn find_function_helpers_clamp_oob_cursor() {
    let p = three_function_provider();
    // Cursor past the end clamps to the last instruction (index 8 = func C RET).
    assert_eq!(find_function_end(&p, 999), 8);
    assert_eq!(find_function_start(&p, 999), 6);
}

#[test]
fn select_function_selects_from_cursor_to_end() {
    let p = three_function_provider();
    let mut view = DisasmView::new("test_select_func");
    // Cursor at index 4 (middle of func B); select_function
    // should select [4, 5] and move cursor to 5 (the RET).
    view.cursor_idx = Some(4);
    view.select_function(&p);
    assert_eq!(view.selected_index(), Some(5));
    assert_eq!(view.selected_indices().len(), 2);
    assert!(view.is_selected(4));
    assert!(view.is_selected(5));
}

#[test]
fn jump_to_function_start_lands_on_post_ret_index() {
    let p = three_function_provider();
    let mut view = DisasmView::new("test_jump_start");
    view.cursor_idx = Some(7); // middle of func C
    view.jump_to_function_start(&p);
    assert_eq!(view.selected_index(), Some(6));
}

#[test]
fn jump_to_function_end_lands_on_ret_index() {
    let p = three_function_provider();
    let mut view = DisasmView::new("test_jump_end");
    view.cursor_idx = Some(7); // middle of func C
    view.jump_to_function_end(&p);
    assert_eq!(view.selected_index(), Some(8));
}

// ── follow_at_cursor ─────────────────────────────────────────────────

#[test]
fn follow_at_cursor_uses_branch_target_first() {
    // Controlled 2-instruction provider: a jmp at 0x500 with
    // resolvable target 0x510 (existing as instruction at idx 1).
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(0x500, vec![0xEB, 0x00], "jmp", "0x510")
            .with_flow(FlowKind::Jump)
            .with_target(0x510),
    );
    p.push(InstructionEntry::new(0x510, vec![0x90], "nop", ""));
    let mut view = DisasmView::new("test_follow_branch");
    view.cursor_idx = Some(0);
    let followed = view.follow_at_cursor(&mut p);
    assert!(followed);
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn follow_at_cursor_falls_back_to_operand_pointer() {
    // No `branch_target`, but operand string contains `0x500`
    // which matches the address of an existing instruction.
    // `mov rax, [0x500]` → follow_at_cursor should jump there.
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(
        0x100,
        vec![0x48, 0x8B, 0x05],
        "mov",
        "rax, [0x500]",
    ));
    p.push(InstructionEntry::new(0x500, vec![0x90], "nop", ""));
    let mut view = DisasmView::new("test_follow_op");
    view.cursor_idx = Some(0);
    let followed = view.follow_at_cursor(&mut p);
    assert!(followed);
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn follow_at_cursor_returns_false_when_nothing_to_follow() {
    // Operand contains a number but it doesn't resolve to any
    // known instruction → no navigation.
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(
        0x100,
        vec![0xB8, 0x10, 0x00, 0x00, 0x00],
        "mov",
        "eax, 0x10",
    ));
    let mut view = DisasmView::new("test_no_follow");
    view.cursor_idx = Some(0);
    assert!(!view.follow_at_cursor(&mut p));
}

#[test]
fn follow_at_cursor_call_indirect_memory_skips_displacement() {
    // `call qword ptr [rip+0x1234]` — `0x1234` is a displacement,
    // NOT the call target. Pre-fix, the operand-scan fallback
    // would chase 0x1234 (and either land on a wrong row or
    // quietly fail with `decode_range` no-op). Post-fix: for
    // Call/Jump rows, in-bracket numbers are skipped → no
    // false navigation, the diagnostic returns NoTargetAndNoNumber.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(
            0x401000,
            vec![0xFF, 0x15, 0x34, 0x12, 0x00, 0x00],
            "call",
            "qword ptr [rip+0x1234]",
        )
        .with_flow(FlowKind::Call),
    );
    // Add a row at 0x1234 so any accidental chase would land here —
    // its presence proves the skip is intentional, not a side-effect
    // of "target missing".
    p.push(InstructionEntry::new(0x1234, vec![0x90], "nop", ""));

    let mut view = DisasmView::new("test_call_indirect");
    view.cursor_idx = Some(0);
    assert!(!view.follow_at_cursor(&mut p));
    // Cursor is unchanged from where the test placed it (0).
    assert_eq!(view.selected_index(), Some(0));
    // Diagnostic surfaces the precise reason.
    view.cursor_idx = Some(0);
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::NoTargetAndNoNumber,
    );
}

#[test]
fn follow_at_cursor_jmp_indirect_memory_skips_displacement() {
    // Same as the call-indirect case but for `jmp [rip+0x500]`
    // (IAT thunk shape).
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(
            0x401000,
            vec![0xFF, 0x25, 0x00, 0x05, 0x00, 0x00],
            "jmp",
            "qword ptr [rip+0x500]",
        )
        .with_flow(FlowKind::Jump),
    );
    p.push(InstructionEntry::new(0x500, vec![0x90], "nop", ""));

    let mut view = DisasmView::new("test_jmp_indirect");
    view.cursor_idx = Some(0);
    assert!(!view.follow_at_cursor(&mut p));
}

#[test]
fn follow_at_cursor_mov_with_memory_pointer_still_follows_in_bracket_number() {
    // Regression guard for the in-bracket-skip change: it must
    // ONLY apply to Call/Jump rows. For non-branching rows the
    // memory-pointer follow keeps working as before.
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(
        0x100,
        vec![0x48, 0x8B, 0x05],
        "mov",
        "rax, [0x500]",
    )); // FlowKind::Normal by default
    p.push(InstructionEntry::new(0x500, vec![0x90], "nop", ""));

    let mut view = DisasmView::new("test_mov_ptr");
    view.cursor_idx = Some(0);
    assert!(view.follow_at_cursor(&mut p));
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn follow_at_cursor_call_register_indirect_returns_no_target_and_no_number() {
    // `call rax` — no number, no branch_target, no operand to
    // chase. Diagnostic must say so explicitly.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(0x401000, vec![0xFF, 0xD0], "call", "rax").with_flow(FlowKind::Call),
    );

    let mut view = DisasmView::new("test_call_reg");
    view.cursor_idx = Some(0);
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::NoTargetAndNoNumber,
    );
}

#[test]
fn follow_at_cursor_call_with_target_outside_provider_signals_diagnostic() {
    // Provider explicitly reports a `branch_target` that
    // isn't decoded; static `VecDisasmProvider::decode_range` is
    // a no-op so the lazy retry can't help. Diagnostic must
    // return `TargetOutsideProvider(target)` instead of an
    // opaque `false` — host can show a status hint.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(
            0x401000,
            vec![0xE8, 0x00, 0x00, 0x00, 0x00],
            "call",
            "0x4011A0",
        )
        .with_flow(FlowKind::Call)
        .with_target(0x4011A0),
    );
    // Note: NO row at 0x4011A0 in the provider.

    let mut view = DisasmView::new("test_call_missing_target");
    view.cursor_idx = Some(0);
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::TargetOutsideProvider(0x4011A0),
    );
    assert!(!view.follow_at_cursor(&mut p));
}

#[test]
fn follow_at_cursor_call_symbolic_label_returns_no_target_and_no_number() {
    // `call kernel32!CreateFileW` — symbolic operand, no
    // numeric immediate, no branch_target. Should fail
    // gracefully with a clear diagnostic.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(
            0x401000,
            vec![0xE8, 0x00, 0x00, 0x00, 0x00],
            "call",
            "kernel32!CreateFileW",
        )
        .with_flow(FlowKind::Call),
    );

    let mut view = DisasmView::new("test_call_symbolic");
    view.cursor_idx = Some(0);
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::NoTargetAndNoNumber,
    );
}

#[test]
fn follow_at_cursor_diagnostic_no_cursor() {
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(0x100, vec![0x90], "nop", ""));
    let mut view = DisasmView::new("test_no_cursor");
    // cursor_idx is None by default.
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::NoCursor,
    );
}

#[test]
fn follow_at_cursor_diagnostic_followed_carries_from_to() {
    // Confirms the success outcome includes both addresses for
    // host-side status logging.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(0x500, vec![0xEB, 0x00], "jmp", "0x510")
            .with_flow(FlowKind::Jump)
            .with_target(0x510),
    );
    p.push(InstructionEntry::new(0x510, vec![0x90], "nop", ""));

    let mut view = DisasmView::new("test_followed_outcome");
    view.cursor_idx = Some(0);
    assert_eq!(
        view.follow_at_cursor_diagnostic(&mut p),
        FollowOutcome::Followed {
            from: 0x500,
            to: 0x510
        },
    );
}

// ── parse_operand_number ─────────────────────────────────────────────

#[test]
fn parse_operand_number_accepts_hex_decimal_masm() {
    assert_eq!(parse_operand_number("0x401000"), Some(0x401000));
    assert_eq!(parse_operand_number("0X401000"), Some(0x401000));
    assert_eq!(parse_operand_number("401000h"), Some(0x401000));
    assert_eq!(parse_operand_number("DEADh"), Some(0xDEAD));
    assert_eq!(parse_operand_number("100"), Some(100));
    assert_eq!(parse_operand_number(""), None);
    assert_eq!(parse_operand_number("h"), None);
    assert_eq!(parse_operand_number("0x"), None);
    assert_eq!(parse_operand_number("garbage"), None);
}
