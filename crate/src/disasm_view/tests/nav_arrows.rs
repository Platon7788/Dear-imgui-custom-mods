//! Clipped branch-arrow window math and 32-bit (PE32) address coverage for function nav / follow / arrows.

use super::super::nav::parse_operand_number;
use super::super::*;

// ── compute_arrows_clipped ───────────────────────────────────────────

#[test]
fn compute_arrows_clipped_keeps_arrow_when_only_target_visible() {
    // Source at index 0 (offscreen), target at index 5 (visible).
    // Window = [3..7) → 4 rows.
    let mut p = VecDisasmProvider::new();
    for i in 0..10 {
        let entry = if i == 0 {
            InstructionEntry::new(0x1000, vec![0xEB, 0x00], "jmp", "0x1005")
                .with_flow(FlowKind::Jump)
                .with_target(0x1005)
        } else {
            InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 3, 4);
    assert_eq!(arrows.len(), 1);
    let arrow = &arrows[0];
    // Source clamped to top of visible window (idx 0 in local space).
    assert!(arrow.clipped_from);
    assert_eq!(arrow.from_idx, 0);
    // Target visible at global idx 5 → local idx 5 - 3 = 2.
    assert!(!arrow.clipped_to);
    assert_eq!(arrow.to_idx, 2);
}

#[test]
fn compute_arrows_clipped_keeps_arrow_when_only_source_visible() {
    let mut p = VecDisasmProvider::new();
    for i in 0..10 {
        let entry = if i == 2 {
            InstructionEntry::new(0x1002, vec![0xEB, 0x00], "jmp", "0x1009")
                .with_flow(FlowKind::Jump)
                .with_target(0x1009)
        } else {
            InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    // Window [1..5) → source at global 2 visible (local 1),
    // target at global 9 offscreen below (clamped to last_local = 3).
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 1, 4);
    assert_eq!(arrows.len(), 1);
    let arrow = &arrows[0];
    assert!(!arrow.clipped_from);
    assert_eq!(arrow.from_idx, 1);
    assert!(arrow.clipped_to);
    assert_eq!(arrow.to_idx, 3); // last_local
}

#[test]
fn compute_arrows_clipped_drops_same_side_off_window() {
    // Source AND target both above visible window → drop.
    let mut p = VecDisasmProvider::new();
    for i in 0..10 {
        let entry = if i == 0 {
            // jmp from idx 0 → idx 2 (both above window [5..9)).
            InstructionEntry::new(0x1000, vec![0xEB, 0x00], "jmp", "0x1002")
                .with_flow(FlowKind::Jump)
                .with_target(0x1002)
        } else {
            InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 4);
    assert!(arrows.is_empty(), "both-above arrow must drop");
}

#[test]
fn compute_arrows_clipped_keeps_pass_through_arrow_forward() {
    // Source above window, target below → vertical line passes
    // through the entire visible region. Both endpoints clipped,
    // no horizontal stubs, no arrowhead in the renderer.
    let mut p = VecDisasmProvider::new();
    for i in 0..15 {
        let entry = if i == 1 {
            // jmp from idx 1 (above window [5..10)) →
            // idx 12 (below window).
            InstructionEntry::new(0x1001, vec![0xEB, 0x00], "jmp", "0x100C")
                .with_flow(FlowKind::Jump)
                .with_target(0x100C)
        } else {
            InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
    assert_eq!(arrows.len(), 1, "pass-through arrow must survive");
    let arrow = &arrows[0];
    assert!(arrow.clipped_from);
    assert!(arrow.clipped_to);
    assert_eq!(arrow.from_idx, 0); // clamped to top of window
    assert_eq!(arrow.to_idx, 4); // clamped to bottom (last_local)
}

#[test]
fn compute_arrows_clipped_keeps_pass_through_arrow_backward() {
    // Source below window, target above → backward jump
    // crossing through visible region.
    let mut p = VecDisasmProvider::new();
    for i in 0..15 {
        let entry = if i == 12 {
            InstructionEntry::new(0x100C, vec![0xEB, 0x00], "jmp", "0x1001")
                .with_flow(FlowKind::Jump)
                .with_target(0x1001)
        } else {
            InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
        };
        p.push(entry);
    }
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
    assert_eq!(arrows.len(), 1);
    let arrow = &arrows[0];
    assert!(arrow.clipped_from);
    assert!(arrow.clipped_to);
    // Source at global 12 (below) → clamped to last_local = 4.
    assert_eq!(arrow.from_idx, 4);
    // Target at global 1 (above) → clamped to 0.
    assert_eq!(arrow.to_idx, 0);
}

#[test]
fn compute_arrows_clipped_priority_orders_anchored_first() {
    // 3 arrows with different priority tiers — verify post-sort
    // order is anchored → half-clipped → pass-through.
    let mut p = VecDisasmProvider::new();
    for i in 0..20 {
        let entry = match i {
            // idx 6 → idx 8: both inside window [5..10) — anchored.
            6 => InstructionEntry::new(0x1006, vec![0xEB, 0x00], "jmp", "0x1008")
                .with_flow(FlowKind::Jump)
                .with_target(0x1008),
            // idx 7 → idx 15: source visible, target below — half-clipped.
            7 => InstructionEntry::new(0x1007, vec![0xEB, 0x00], "jmp", "0x100F")
                .with_flow(FlowKind::Jump)
                .with_target(0x100F),
            // idx 1 → idx 18: pass-through.
            1 => InstructionEntry::new(0x1001, vec![0xEB, 0x00], "jmp", "0x1012")
                .with_flow(FlowKind::Jump)
                .with_target(0x1012),
            _ => InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", ""),
        };
        p.push(entry);
    }
    let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
    assert_eq!(arrows.len(), 3);
    // Anchored arrow first.
    assert!(!arrows[0].clipped_from && !arrows[0].clipped_to);
    // Half-clipped arrow second.
    assert!(arrows[1].clipped_from ^ arrows[1].clipped_to);
    // Pass-through arrow last (first to be truncated under cap).
    assert!(arrows[2].clipped_from && arrows[2].clipped_to);
}

// ── Architecture coverage: x32 (PE32) addresses ──────────────────────
//
// All addresses are `u64` on the wire so x32 fits naturally in
// the upper-zero range. These tests pin behaviour at the
// typical PE32 image-base 0x401000 to catch regressions where a
// future change accidentally assumes the upper 32 bits are
// populated (e.g. truncates to u32 internally).

fn pe32_three_function_provider() -> VecDisasmProvider {
    // Same shape as `three_function_provider` but at PE32 base.
    let mut p = VecDisasmProvider::new();
    let mut addr = 0x00401000_u64;
    for f in 0..3 {
        // prologue
        p.push(
            InstructionEntry::new(addr, vec![0x55], "push", "ebp")
                .with_flow(FlowKind::Stack)
                .with_block(f),
        );
        addr += 1;
        // body
        p.push(InstructionEntry::new(addr, vec![0x90], "nop", "").with_block(f));
        addr += 1;
        // ret
        p.push(
            InstructionEntry::new(addr, vec![0xC3], "ret", "")
                .with_flow(FlowKind::Return)
                .with_block(f),
        );
        addr += 1;
    }
    p
}

#[test]
fn x32_find_function_works_with_pe32_addresses() {
    let p = pe32_three_function_provider();
    // Func 0: indices 0..=2, addresses 0x401000..=0x401002.
    assert_eq!(find_function_start(&p, 0), 0);
    assert_eq!(find_function_end(&p, 0), 2);
    // Func 1: indices 3..=5.
    assert_eq!(find_function_start(&p, 4), 3);
    assert_eq!(find_function_end(&p, 4), 5);
    // Func 2: indices 6..=8.
    assert_eq!(find_function_start(&p, 7), 6);
    assert_eq!(find_function_end(&p, 7), 8);
}

#[test]
fn x32_follow_at_cursor_resolves_pe32_jump() {
    // Typical x32 binary: jmp from 0x401000 → 0x401005.
    let mut p = VecDisasmProvider::new();
    p.push(
        InstructionEntry::new(
            0x00401000,
            vec![0xE9, 0x00, 0x00, 0x00, 0x00],
            "jmp",
            "0x00401005",
        )
        .with_flow(FlowKind::Jump)
        .with_target(0x00401005),
    );
    p.push(InstructionEntry::new(0x00401005, vec![0x90], "nop", ""));
    let mut view = DisasmView::new("x32_follow");
    view.cursor_idx = Some(0);
    assert!(view.follow_at_cursor(&mut p));
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn x32_follow_at_cursor_resolves_absolute_memory_operand() {
    // x32 absolute memory operand `mov eax, [0x401005]` — the
    // operand-pointer fallback should chase the immediate.
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(
        0x00401000,
        vec![0x8B, 0x05, 0x05, 0x10, 0x40, 0x00],
        "mov",
        "eax, [0x00401005]",
    ));
    p.push(InstructionEntry::new(0x00401005, vec![0x90], "nop", ""));
    let mut view = DisasmView::new("x32_op_follow");
    view.cursor_idx = Some(0);
    assert!(view.follow_at_cursor(&mut p));
    assert_eq!(view.selected_index(), Some(1));
}

#[test]
fn x32_format_address_literal_8_digits_when_not_64bit() {
    let mut view = DisasmView::new("x32_format");
    view.config.address_width_64 = false;
    view.config.uppercase = true;
    assert_eq!(view.format_address_literal(0x00401000), "0x00401000");
    assert_eq!(view.format_address_literal(0xDEADBEEF), "0xDEADBEEF");
    view.config.uppercase = false;
    assert_eq!(view.format_address_literal(0x00401000), "0x00401000");
    // Truncation behaviour for too-wide addresses: `{:08X}`
    // doesn't truncate, it just widens — this is fine for x32
    // because addresses fit in 8 hex digits, and would surface
    // weird-but-not-broken display if a 64-bit address landed
    // here while `address_width_64=false`.
}

#[test]
fn x64_format_address_literal_16_digits_when_64bit() {
    let mut view = DisasmView::new("x64_format");
    view.config.address_width_64 = true;
    view.config.uppercase = true;
    assert_eq!(
        view.format_address_literal(0x00007FF6_12345678),
        "0x00007FF612345678"
    );
    assert_eq!(
        view.format_address_literal(0xFFFF_FFFF_FFFF_FFFF),
        "0xFFFFFFFFFFFFFFFF"
    );
    view.config.uppercase = false;
    assert_eq!(
        view.format_address_literal(0x7FF6_1234_5678),
        "0x00007ff612345678"
    );
}

#[test]
fn parse_operand_number_handles_masm_leading_zero_quirk() {
    // MASM / iced-x86 emit `0FFFFFFFFh` (leading 0 prefix
    // before a hex letter) so the assembler doesn't mistake it
    // for an identifier. Parser must accept this form.
    assert_eq!(parse_operand_number("0FFFFFFFFh"), Some(0xFFFFFFFF));
    assert_eq!(parse_operand_number("0CAFEBABEh"), Some(0xCAFEBABE));
    // Without leading zero — also valid (just an upper-case hex).
    assert_eq!(parse_operand_number("FFFFh"), Some(0xFFFF));
}

#[test]
fn parse_operand_number_handles_full_u64_range() {
    // Verify the parser doesn't truncate to u32 anywhere.
    assert_eq!(parse_operand_number("0xFFFFFFFFFFFFFFFF"), Some(u64::MAX));
    assert_eq!(
        parse_operand_number("0x7FF612345678"),
        Some(0x7FF6_1234_5678)
    );
}
