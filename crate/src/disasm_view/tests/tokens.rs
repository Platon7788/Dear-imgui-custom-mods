//! Operand tokenizer / classifier, flow colours, block tint, arrow depth assignment, `parse_address`, and parser property tests.

use super::super::*;
use super::sample_provider;
use tokens::{OperandTokenizer, TokenKind, classify_operand_token};

#[test]
fn bookmarks_set_is_sorted_for_host_iteration() {
    // Pin the BTreeSet ordering — hosts that round-trip the
    // bookmark set through serde / config files want a stable
    // ascending-address order. Insertion order is intentionally
    // randomised here.
    let mut view = DisasmView::new("t");
    view.add_bookmark(0x401010);
    view.add_bookmark(0x401000);
    view.add_bookmark(0x40100F);
    view.add_bookmark(0x401004);
    let collected: Vec<u64> = view.bookmarks().iter().copied().collect();
    assert_eq!(collected, vec![0x401000, 0x401004, 0x40100F, 0x401010]);
}

#[test]
fn test_flow_kind_colors() {
    let colors = DisasmColors::default();
    // Different flow kinds should have visually distinct mnemonic colors.
    let normal = colors.mnemonic_color(FlowKind::Normal);
    let jump = colors.mnemonic_color(FlowKind::Jump);
    let call = colors.mnemonic_color(FlowKind::Call);
    let ret = colors.mnemonic_color(FlowKind::Return);

    assert_ne!(normal, jump);
    assert_ne!(jump, call);
    assert_ne!(call, ret);
}

#[test]
fn test_arrow_color() {
    let colors = DisasmColors::default();
    let jump_color = colors.arrow_color(FlowKind::Jump);
    let call_color = colors.arrow_color(FlowKind::Call);
    // Should have different colors for different flow types.
    assert_ne!(jump_color, call_color);
}

#[test]
fn test_block_tint() {
    let colors = DisasmColors::default();
    let tint0 = colors.block_tint(0);
    let tint1 = colors.block_tint(1);
    // Block tints should differ between adjacent blocks.
    assert!(tint0 != tint1 || tint0[3] == 0.0);
}

#[test]
fn test_compute_arrows() {
    let p = sample_provider();
    let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
        .filter_map(|i| p.instruction(i))
        .collect();
    let arrows = compute_arrows(&instrs, 0, instrs.len());
    // je at index 5 targets 0x401017 and call targets 0x40101D — both outside
    // our 8 instructions, so no arrows expected in this basic sample.
    // Arrow computation only shows arrows where BOTH endpoints are visible.
    assert!(
        arrows.is_empty() || arrows.len() <= 2,
        "Expected 0-2 arrows, got {}",
        arrows.len()
    );
}

#[test]
fn test_operand_tokenizer_registers() {
    let tokens: Vec<_> = OperandTokenizer::new("rax, rbx").collect();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Register));
}

#[test]
fn test_operand_tokenizer_numbers() {
    let tokens: Vec<_> = OperandTokenizer::new("0x1234").collect();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Number));
}

#[test]
fn test_operand_tokenizer_memory() {
    let tokens: Vec<_> = OperandTokenizer::new("[rsp+8]").collect();
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Memory));
}

#[test]
fn test_classify_operand_register() {
    assert_eq!(classify_operand_token("rax"), TokenKind::Register);
    assert_eq!(classify_operand_token("xmm0"), TokenKind::Register);
    assert_eq!(classify_operand_token("RAX"), TokenKind::Register);
}

#[test]
fn test_classify_operand_number() {
    assert_eq!(classify_operand_token("0x1234"), TokenKind::Number);
    assert_eq!(classify_operand_token("100"), TokenKind::Number);
    assert_eq!(classify_operand_token("FFh"), TokenKind::Number);
}

#[test]
fn test_classify_operand_size() {
    assert_eq!(classify_operand_token("qword"), TokenKind::Memory);
    assert_eq!(classify_operand_token("ptr"), TokenKind::Memory);
}

#[test]
fn test_column_widths_default() {
    let cols = ColumnWidths::default();
    assert!(cols.address > 0.0);
    // User-requested widths (2026-04-30): Bytes 200,
    // Instruction 300 (= mnemonic + operands), Comment is
    // a *minimum* width (renders dynamic in `frame_comment_w`).
    assert_eq!(cols.bytes, 200.0, "bytes column must be 200 px");
    assert_eq!(
        cols.mnemonic + cols.operands,
        300.0,
        "instruction (mnemonic + operands) must total 300 px"
    );
    assert!(
        cols.comment >= 100.0,
        "comment min should keep edit-cell usable"
    );
}

#[test]
fn test_disasm_config_default() {
    let cfg = DisasmViewConfig::default();
    assert!(cfg.show_arrows);
    assert!(cfg.show_breakpoints);
    assert!(!cfg.show_block_tints);
    assert!(cfg.show_header);
    assert!(!cfg.editable);
    assert!(cfg.address_width_64);
}

#[test]
fn test_select_and_goto() {
    let p = sample_provider();
    let mut view = DisasmView::new("test");
    view.select(3);
    assert_eq!(view.selected_index(), Some(3));

    view.goto_address(0x401000, &p);
    assert_eq!(view.selected_index(), Some(0));
}

#[test]
fn test_nav_history() {
    let p = sample_provider();
    let mut view = DisasmView::new("test");

    view.select(0); // at 0x401000
    view.goto_address(0x401008, &p); // jump to call
    assert_eq!(view.selected_index(), Some(3));

    view.nav_back(&p);
    assert_eq!(view.selected_index(), Some(0));

    view.nav_forward(&p);
    assert_eq!(view.selected_index(), Some(3));
}

#[test]
fn test_parse_address() {
    // Explicit 0x prefix → hex.
    assert_eq!(parse_address("0x401000"), Some(0x401000));
    assert_eq!(parse_address("0X401000"), Some(0x401000));
    // No prefix, no hex letters → decimal.
    assert_eq!(parse_address("256"), Some(256));
    assert_eq!(parse_address("4080"), Some(4080));
    assert_eq!(parse_address("401000"), Some(401000));
    // No prefix, contains a hex letter → hex.
    assert_eq!(parse_address("4abc"), Some(0x4abc));
    assert_eq!(parse_address("DEAD"), Some(0xDEAD));
    assert_eq!(parse_address("cafef00d"), Some(0xcafef00d));
    // Whitespace is trimmed.
    assert_eq!(parse_address("  0xff  "), Some(0xff));
    // Garbage → None.
    assert_eq!(parse_address("hello"), None);
    assert_eq!(parse_address(""), None);
}

#[test]
fn test_arrow_depth_assignment() {
    // Create instructions with nested branches.
    let mut p = VecDisasmProvider::new();
    for i in 0..10 {
        let mut entry = InstructionEntry::new(0x1000 + i * 2, vec![0x90], "nop", "");
        entry.flow_kind = FlowKind::Normal;
        p.push(entry);
    }
    // Add two overlapping jumps.
    p.instructions_mut()[2] = InstructionEntry::new(0x1004, vec![0xEB, 0x08], "jmp", "0x100E")
        .with_flow(FlowKind::Jump)
        .with_target(0x100E);
    p.instructions_mut()[1] = InstructionEntry::new(0x1002, vec![0x74, 0x0C], "je", "0x1010")
        .with_flow(FlowKind::Jump)
        .with_target(0x1010);

    let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
        .filter_map(|i| p.instruction(i))
        .collect();
    let arrows = compute_arrows(&instrs, 0, instrs.len());

    // If both targets are in range, should have different depths.
    if arrows.len() >= 2 {
        assert_ne!(
            arrows[0].depth, arrows[1].depth,
            "Overlapping arrows should have different depths"
        );
    }
}

// ── Property-based tests ─────────────────────────────────────────────

use proptest::prelude::*;

proptest! {
    /// `parse_address` accepts arbitrary strings without panicking.
    #[test]
    fn prop_parse_address_never_panics(s in ".{0,32}") {
        let _ = parse_address(&s);
    }

    /// Hex-prefixed addresses round-trip cleanly.
    #[test]
    fn prop_parse_address_hex_roundtrips(value in any::<u64>()) {
        let s = format!("0x{value:X}");
        prop_assert_eq!(parse_address(&s), Some(value));
    }
}

// ── OperandTokenizer edge cases ──────────────────────────────────────

fn tokens_of(s: &str) -> Vec<(String, TokenKind)> {
    OperandTokenizer::new(s)
        .map(|t| (t.text.to_string(), t.kind))
        .collect()
}

#[test]
fn tokenizer_empty_input() {
    assert!(tokens_of("").is_empty());
}

#[test]
fn tokenizer_only_punctuation_collapses() {
    // Run of `, +-*: ` is consumed as a single Plain token.
    let toks = tokens_of(",,, ");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, ",,, ");
    assert_eq!(toks[0].1, TokenKind::Plain);
}

#[test]
fn tokenizer_trailing_comma() {
    // `rax,` → Register("rax"), Plain(",").
    let toks = tokens_of("rax,");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].0, "rax");
    assert_eq!(toks[0].1, TokenKind::Register);
    assert_eq!(toks[1].0, ",");
    assert_eq!(toks[1].1, TokenKind::Plain);
}

#[test]
fn tokenizer_two_operands() {
    // `rax, rbx` splits into reg, plain (", "), reg.
    let toks = tokens_of("rax, rbx");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0].1, TokenKind::Register);
    assert_eq!(toks[1].1, TokenKind::Plain);
    assert_eq!(toks[2].1, TokenKind::Register);
    assert_eq!(toks[2].0, "rbx");
}

#[test]
fn tokenizer_memory_brackets() {
    // `[rsp+8]` → `[`, `rsp`, `+`, `8`, `]`.
    let toks = tokens_of("[rsp+8]");
    let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
    let texts: Vec<&str> = toks.iter().map(|t| t.0.as_str()).collect();
    assert_eq!(texts, vec!["[", "rsp", "+", "8", "]"]);
    assert_eq!(
        kinds,
        vec![
            TokenKind::Memory,
            TokenKind::Register,
            TokenKind::Plain,
            TokenKind::Number,
            TokenKind::Memory,
        ]
    );
}

#[test]
fn tokenizer_nested_brackets_size_keyword() {
    // `qword ptr [rax + 0x10]` exercises size keywords + memory + register + hex.
    let toks = tokens_of("qword ptr [rax + 0x10]");
    let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
    // qword/ptr classify as Memory; rax Register; 0x10 Number; brackets Memory.
    assert!(kinds.contains(&TokenKind::Memory));
    assert!(kinds.contains(&TokenKind::Register));
    assert!(kinds.contains(&TokenKind::Number));
    assert_eq!(toks.first().unwrap().0, "qword");
    assert_eq!(toks.last().unwrap().0, "]");
    assert_eq!(toks.last().unwrap().1, TokenKind::Memory);
}

#[test]
fn tokenizer_unterminated_string() {
    // Missing closing quote: tokenizer must consume to end-of-input, not panic.
    let toks = tokens_of("\"hello world");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].1, TokenKind::String);
    assert_eq!(toks[0].0, "\"hello world");
}

#[test]
fn tokenizer_hex_suffix_h() {
    // MASM-style `1Fh` is classified as a number.
    let toks = tokens_of("1Fh");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].1, TokenKind::Number);
}

#[test]
fn tokenizer_unknown_word_is_plain() {
    // `gibberish` is not a register, not a number → Plain.
    let toks = tokens_of("gibberish");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].1, TokenKind::Plain);
}

// ── Iced-x86 / extended register coverage ────────────────────────────
//
// iced-x86's default `IntelFormatter` outputs operand text that we
// need to colour-code correctly. Pin the cases that previously fell
// through to `Plain` so a regression in `is_x86_register` /
// `classify_operand_token` is caught with a meaningful diagnostic.

#[test]
fn classify_extended_gp_registers() {
    // r8..r15 with optional b/w/d suffix.
    for r in [
        "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "r8b", "r9w", "r10d", "r15b", "r12w",
    ] {
        assert_eq!(
            classify_operand_token(r),
            TokenKind::Register,
            "{r} should classify as Register",
        );
    }
}

#[test]
fn classify_avx512_registers() {
    // SIMD 16..31 (AVX-512) + zmm + mask registers.
    for r in [
        "xmm15", "xmm16", "xmm31", "ymm0", "ymm17", "ymm31", "zmm0", "zmm15", "zmm31", "k0", "k1",
        "k7",
    ] {
        assert_eq!(
            classify_operand_token(r),
            TokenKind::Register,
            "{r} should classify as Register",
        );
    }
}

#[test]
fn classify_system_registers() {
    // Control / debug / test / MMX — used by kernel-mode + legacy disasm.
    for r in [
        "cr0", "cr2", "cr3", "cr4", "cr8", "cr15", "dr0", "dr6", "dr7", "tr0", "tr7", "mm0", "mm7",
    ] {
        assert_eq!(
            classify_operand_token(r),
            TokenKind::Register,
            "{r} should classify as Register",
        );
    }
}

#[test]
fn classify_size_keywords_extended() {
    // `fword`, `tbyte`, `oword`, `zmmword` — not in pre-iced-x86 set.
    for kw in ["fword", "tbyte", "oword", "zmmword"] {
        assert_eq!(
            classify_operand_token(kw),
            TokenKind::Memory,
            "{kw} should classify as Memory",
        );
    }
}

#[test]
fn classify_rejects_register_lookalikes() {
    // Regression guards: invalid range reads as Plain (NOT Register).
    // `r0` (no extended r0 exists), `xmm32` (out of range),
    // `zmm99`, `cr16`, `mm8`, `k8`, `r10x` (bad suffix).
    for tok in ["r0", "r7", "xmm32", "zmm99", "cr16", "mm8", "k8", "r10x"] {
        assert_eq!(
            classify_operand_token(tok),
            TokenKind::Plain,
            "{tok} must NOT be classified as Register",
        );
    }
}

#[test]
fn classify_number_edge_cases() {
    // Empty hex bodies (`h`, `0x`) are NOT numbers.
    assert_eq!(classify_operand_token("h"), TokenKind::Plain);
    assert_eq!(classify_operand_token("H"), TokenKind::Plain);
    assert_eq!(classify_operand_token("0x"), TokenKind::Plain);
    assert_eq!(classify_operand_token("0X"), TokenKind::Plain);
    // ...but minimal valid hex stays a Number.
    assert_eq!(classify_operand_token("0x0"), TokenKind::Number);
    assert_eq!(classify_operand_token("Fh"), TokenKind::Number);
}

#[test]
fn tokenizer_iced_x86_no_space_after_comma() {
    // iced-x86's default IntelFormatter outputs without a space
    // after the operand separator: `mov rax,qword ptr [rsp+10h]`.
    // Pin that the tokenizer still produces correct kinds.
    let toks = tokens_of("rax,qword ptr [rsp+10h]");
    let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
    // First token must be the register.
    assert_eq!(toks[0].0, "rax");
    assert_eq!(toks[0].1, TokenKind::Register);
    // `qword`, `ptr`, `[`, `]` all classify as Memory.
    assert!(kinds.contains(&TokenKind::Memory));
    // `rsp` → Register, `10h` → Number.
    assert!(kinds.contains(&TokenKind::Register));
    assert!(kinds.contains(&TokenKind::Number));
}
