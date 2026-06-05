//! Unit + property tests for `disasm_view` that need no ImGui context.
//!
//! Tests reach into private fields (`view.cursor_idx`, `view.origin_addr`,
//! `view.nav`, …) and private free fns (`parse_address`,
//! `parse_operand_number`) through descendant-module visibility — no API
//! surface is widened just for testing. Split into themed sub-modules so
//! each stays under the 500-line ceiling:
//! - [`state`]      — constructor, providers, breakpoint toggle, toolbar
//!   convenience selectors, bookmark API, nav-history state.
//! - [`tokens`]     — operand tokenizer / classifier, flow colours, block
//!   tint, arrow depth, `parse_address`, parser property tests.
//! - [`comment`]    — theme-palette swap, `set_comment` round-trip.
//! - [`nav_func`]   — function-boundary heuristic, follow-at-cursor (branch
//!   target / operand scan / diagnostics), `parse_operand_number`.
//! - [`nav_arrows`] — clipped branch-arrow window math + 32-bit (PE32)
//!   address coverage.
//! - [`nav_origin`] — origin breadcrumb across navigations + lazy-decode
//!   follow for streaming providers.
//! - [`config`]     — config defaults / ron round-trip / locale + verbosity
//!   guards, watchpoint API, clamp regressions, bulk bookmark restore.
//! - [`knowledge_idiom`] / [`knowledge_patterns`] / [`knowledge_verbosity`]
//!   — catalogue-tier authoring guards for the `disasm-knowledge`
//!   recognisers + verbosity-tier dispatch.

use super::*;
use provider::{InstructionEntry, VecDisasmProvider};

mod comment;
mod config;
mod knowledge_idiom;
mod knowledge_patterns;
mod knowledge_verbosity;
mod nav_arrows;
mod nav_func;
mod nav_origin;
mod state;
mod tokens;

// ── Shared fixtures ───────────────────────────────────────────

/// 8-instruction `push rbp` … `ret` function used by the bulk of the
/// state / nav / comment tests.
pub(super) fn sample_provider() -> VecDisasmProvider {
    let mut p = VecDisasmProvider::new();
    p.push(InstructionEntry::new(0x401000, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack));
    p.push(InstructionEntry::new(
        0x401001,
        vec![0x48, 0x89, 0xE5],
        "mov",
        "rbp, rsp",
    ));
    p.push(
        InstructionEntry::new(0x401004, vec![0x48, 0x83, 0xEC, 0x20], "sub", "rsp, 0x20")
            .with_flow(FlowKind::Stack),
    );
    p.push(
        InstructionEntry::new(
            0x401008,
            vec![0xE8, 0x10, 0x00, 0x00, 0x00],
            "call",
            "0x40101D",
        )
        .with_flow(FlowKind::Call)
        .with_target(0x40101D)
        .with_comment("some_function"),
    );
    p.push(InstructionEntry::new(
        0x40100D,
        vec![0x48, 0x85, 0xC0],
        "test",
        "rax, rax",
    ));
    p.push(
        InstructionEntry::new(0x401010, vec![0x74, 0x05], "je", "0x401017")
            .with_flow(FlowKind::Jump)
            .with_target(0x401017),
    );
    p.push(InstructionEntry::new(0x401012, vec![0xC9], "leave", ""));
    p.push(InstructionEntry::new(0x401013, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
    p
}

/// 3-function provider for boundary / nav tests:
/// - func A: `[0..=2]` ending in RET at index 2
/// - func B: `[3..=5]` ending in RET at index 5
/// - func C: `[6..=8]` ending in RET at index 8
pub(super) fn three_function_provider() -> VecDisasmProvider {
    let mut p = VecDisasmProvider::new();
    // func A
    p.push(InstructionEntry::new(0x1000, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack));
    p.push(InstructionEntry::new(0x1001, vec![0x90], "nop", ""));
    p.push(InstructionEntry::new(0x1002, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
    // func B
    p.push(InstructionEntry::new(0x1003, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack));
    p.push(InstructionEntry::new(0x1004, vec![0x90], "nop", ""));
    p.push(InstructionEntry::new(0x1005, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
    // func C
    p.push(InstructionEntry::new(0x1006, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack));
    p.push(InstructionEntry::new(0x1007, vec![0x90], "nop", ""));
    p.push(InstructionEntry::new(0x1008, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
    p
}

/// Walk every catalogue entry of the given recogniser and assert
/// that all four tier slots (`compact_en`/`compact_ru`/
/// `educational_en`/`educational_ru`) are non-empty. The
/// closure-driven probe pattern lets each recogniser test re-use
/// the same accept-each pattern (they live in different modules).
pub(super) fn assert_tiers_authored<H>(
    label: &str,
    probes: &[(&str, H)],
    tiers_of: impl Fn(&H) -> &crate::disasm_view::HintTiers,
) {
    for (tag, hit) in probes {
        let t = tiers_of(hit);
        assert!(
            !t.compact_en.is_empty(),
            "{label}::{tag}: compact_en still empty"
        );
        assert!(
            !t.compact_ru.is_empty(),
            "{label}::{tag}: compact_ru still empty"
        );
        assert!(
            !t.educational_en.is_empty(),
            "{label}::{tag}: educational_en still empty"
        );
        assert!(
            !t.educational_ru.is_empty(),
            "{label}::{tag}: educational_ru still empty"
        );
    }
}
