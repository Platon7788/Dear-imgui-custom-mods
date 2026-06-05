//! Comprehensive hover tooltip for a single disassembly row.
//!
//! Split out of [`super::rows`] so the per-row paint logic stays under
//! the file-size ceiling. The body is the educational tooltip pipeline
//! documented in `disasm_view::mod` — address (with 32-bit shadow in
//! 64-bit mode), size / raw bytes, mnemonic + operands, flow kind,
//! branch target + offset, block / breakpoint / current-IP markers,
//! comment, and the eight opt-in knowledge layers (explanation, idiom,
//! gotcha, operand hint, compiler pattern, anti-disasm, boundary,
//! branch direction).

use super::*;
use crate::disasm_view::provider::{FlowKind, Instruction};
use crate::utils::hex::byte_hex;

impl DisasmView {
    /// Paint the multi-section hover tooltip for the row at `addr`
    /// (the row's own address; `prev_instr` / `next_instr` feed the
    /// multi-instruction detectors). Called by `draw_instruction_row`
    /// only when the row is hovered.
    pub(super) fn draw_row_tooltip(
        &self,
        ui: &dear_imgui_rs::Ui,
        instr: &dyn Instruction,
        prev_instr: Option<&dyn Instruction>,
        next_instr: Option<&dyn Instruction>,
        addr: u64,
    ) {
        let cfg = &self.config;
        // Honour `cfg.address_width_64` AND `cfg.uppercase` — fixes
        // the long-standing bug where the gutter respected both
        // flags but the tooltip hard-coded uppercase 16-digit
        // formatting regardless of widget config. The 32-bit
        // shadow is only emitted in 64-bit mode (in 32-bit mode
        // the primary line already shows 8 digits — a duplicate
        // would just clutter the popup).
        let strings = self.strings();
        let upper = cfg.uppercase;
        let is_64 = cfg.address_width_64;
        crate::utils::themed_tooltip(ui, || {
            let primary = match (upper, is_64) {
                (true, true) => format!("{}0x{addr:016X}", strings.tooltip_address_prefix),
                (false, true) => format!("{}0x{addr:016x}", strings.tooltip_address_prefix),
                (true, false) => format!("{}0x{addr:08X}", strings.tooltip_address_prefix),
                (false, false) => format!("{}0x{addr:08x}", strings.tooltip_address_prefix),
            };
            ui.text(primary);
            if is_64 && addr <= 0xFFFF_FFFF {
                let addr32 = addr as u32;
                let shadow = if upper {
                    format!("{}0x{addr32:08X}", strings.tooltip_address32_prefix)
                } else {
                    format!("{}0x{addr32:08x}", strings.tooltip_address32_prefix)
                };
                ui.text(shadow);
            }

            let bytes = instr.bytes();
            ui.text(format!(
                "{}{} {}",
                strings.tooltip_size_label,
                bytes.len(),
                strings.tooltip_unit_bytes,
            ));
            let mut hex_str = String::with_capacity(bytes.len() * 3);
            for (i, b) in bytes.iter().enumerate() {
                if i > 0 {
                    hex_str.push(' ');
                }
                hex_str.push_str(byte_hex(*b, upper));
            }
            ui.text(format!("{}{hex_str}", strings.tooltip_bytes_label));

            ui.text(format!(
                "{}{} {}",
                strings.tooltip_instr_label,
                instr.mnemonic(),
                instr.operands(),
            ));

            let flow_desc = match instr.flow_kind() {
                FlowKind::Normal => strings.flow_normal,
                FlowKind::Jump => strings.flow_jump,
                FlowKind::Call => strings.flow_call,
                FlowKind::Return => strings.flow_return,
                FlowKind::Nop => strings.flow_nop,
                FlowKind::Stack => strings.flow_stack,
                FlowKind::System => strings.flow_system,
                FlowKind::Invalid => strings.flow_invalid,
            };
            ui.text(format!("{}{flow_desc}", strings.tooltip_flow_label));

            if let Some(target) = instr.branch_target() {
                let target_str = match (upper, is_64) {
                    (true, true) => format!("{}0x{target:016X}", strings.tooltip_target_label),
                    (false, true) => format!("{}0x{target:016x}", strings.tooltip_target_label),
                    (true, false) => format!("{}0x{target:08X}", strings.tooltip_target_label),
                    (false, false) => format!("{}0x{target:08x}", strings.tooltip_target_label),
                };
                ui.text(target_str);
                let off = target as i64 - addr as i64;
                let (sign, mag) = if off >= 0 { ('+', off) } else { ('-', -off) };
                let off_hex = if upper {
                    format!("0x{mag:X}")
                } else {
                    format!("0x{mag:x}")
                };
                ui.text(format!(
                    "Offset: {sign}{off_hex} ({mag} {})",
                    strings.tooltip_unit_bytes,
                ));
                // Discoverability: spell out the gesture so users
                // who haven't read the docs know `call` / `jmp` /
                // `Jcc` rows are followable. Only painted when
                // `branch_target` is set — the gesture is a no-op
                // otherwise (operand-scan fallback succeeds for a
                // narrow set of forms only). Mirrors the
                // address-gutter "Double-click to copy" hint.
                ui.text(strings.tooltip_double_click_follow);
            }

            ui.text(format!(
                "{}{}",
                strings.tooltip_block_label,
                instr.block_index(),
            ));

            if instr.has_breakpoint() {
                let bp_num = instr.breakpoint_number();
                if bp_num > 0 {
                    ui.text(format!("{}#{bp_num}", strings.tooltip_breakpoint_label));
                } else {
                    ui.text(strings.tooltip_breakpoint_yes);
                }
            }

            if instr.is_current() {
                ui.text(strings.tooltip_current_ip);
            }

            if let Some(comment) = instr.comment() {
                ui.text(format!("{}{comment}", strings.tooltip_comment_label));
            }

            // ── Educational block (opt-out per cfg flag) ────────
            //
            // 1. Mnemonic explainer (`cfg.show_explanation`) —
            //    plain-language description of the current opcode.
            // 2. Idiom detector (`cfg.show_idiom`) — recognises
            //    multi-instruction patterns from prev/current/next
            //    (prologue / cmp+Jcc / get-IP / NULL-check / ...).
            // 3. Gotcha (`cfg.show_gotcha`) — anti-debug /
            //    anti-disasm / obfuscation warnings tied to
            //    specific mnemonics (rdtsc timing, int 2D probe,
            //    push/ret ROP gadgets, ...).
            //
            // Each block has its own toggle so senior REs can
            // strip the tooltip down to the raw fields.
            let info = super::super::mnemonic::lookup(instr.mnemonic());

            // Single accumulator (audit H2/draw): each detector
            // emits its own separator only on the first actual
            // emission. The previous code chained a growing
            // disjunction `!(cfg.show_X || cfg.show_Y || ...)`
            // through every block — by the branch-direction
            // block it had **7** disjuncts. Adding a new
            // educational toggle silently broke older arms
            // (drift bug). The new pattern is O(1) per block
            // and adding a block touches only its own scope.
            let mut emitted_any = false;
            let maybe_separator = |emitted_any: &mut bool, ui: &dear_imgui_rs::Ui| {
                if !*emitted_any {
                    ui.separator();
                    *emitted_any = true;
                }
            };

            if cfg.show_explanation
                && let Some(info) = info
            {
                maybe_separator(&mut emitted_any, ui);
                ui.text(format!(
                    "{}{}",
                    strings.tooltip_explanation_label,
                    info.description_for(cfg.locale.into(), cfg.verbosity),
                ));
            }

            if cfg.show_idiom {
                let prev_pair = prev_instr.map(|p| (p.mnemonic(), p.operands()));
                let next_pair = next_instr.map(|n| (n.mnemonic(), n.operands()));
                let ctx = super::super::idiom::InstructionContext {
                    prev: prev_pair,
                    current: (instr.mnemonic(), instr.operands()),
                    next: next_pair,
                };
                if let Some(idiom) = super::super::idiom::detect(&ctx) {
                    maybe_separator(&mut emitted_any, ui);
                    ui.text(format!(
                        "{}{}",
                        strings.tooltip_idiom_label,
                        idiom.description_for(cfg.locale.into(), cfg.verbosity),
                    ));
                }
            }

            if cfg.show_gotcha
                && let Some(info) = info
                && let Some(gotcha) = info.gotcha_for(cfg.locale.into(), cfg.verbosity)
            {
                maybe_separator(&mut emitted_any, ui);
                ui.text(format!("{}{gotcha}", strings.tooltip_gotcha_label));
            }

            // Operand-pattern decoder — turns `[rcx+rax*8+8]` into
            // "Array indexing: rcx is base, rax is index ×8 …".
            // Walks every operand in the `instr.operands()` text,
            // emits one line per memory operand we can decode.
            // Bare register operands fire only when the register
            // has an ABI-special role (argument N, return value,
            // segment base, etc.) — the rest are skipped to keep
            // the tooltip from spamming "RBX = general-purpose"
            // for every plain reg/reg move.
            if cfg.show_operand_hint {
                for raw in super::split_operand_list(instr.operands()) {
                    match super::super::operand::parse(raw) {
                        super::super::operand::OperandKind::Memory(mem) => {
                            let line = super::super::operand::explain_memory(
                                &mem,
                                cfg.abi,
                                cfg.locale.into(),
                            );
                            if !line.is_empty() {
                                maybe_separator(&mut emitted_any, ui);
                                ui.text(format!("{}{line}", strings.tooltip_operand_label));
                            }
                        }
                        super::super::operand::OperandKind::Register(reg) => {
                            let role = super::super::abi::role(reg, cfg.abi);
                            if let Some(desc) =
                                super::super::abi::role_description(role, reg, cfg.locale.into())
                            {
                                maybe_separator(&mut emitted_any, ui);
                                ui.text(format!("{}{desc}", strings.tooltip_operand_label));
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Compiler-pattern recogniser — labels Win64 leaf
            // frames, `__chkstk` probes, vtable indirect calls,
            // MSVC `/GS` security cookies, atomic CAS / RMW, IAT
            // thunks, PEB / TEB / TIB accesses via `gs:` / `fs:`.
            if cfg.show_compiler_pattern {
                let prev_pair = prev_instr.map(|p| (p.mnemonic(), p.operands()));
                let next_pair = next_instr.map(|n| (n.mnemonic(), n.operands()));
                let cctx = super::super::compiler::CompilerContext {
                    prev: prev_pair,
                    current: (instr.mnemonic(), instr.operands()),
                    next: next_pair,
                    abi: cfg.abi,
                };
                if let Some(pat) = super::super::compiler::detect(&cctx) {
                    maybe_separator(&mut emitted_any, ui);
                    ui.text(format!(
                        "{}{}",
                        strings.tooltip_compiler_label,
                        pat.description_for(cfg.locale.into(), cfg.verbosity),
                    ));
                }
            }

            // Anti-disasm / anti-debug recogniser — flags
            // stack-based control flow (`push imm; ret`), opaque
            // predicates, self-modifying code, anti-VM CPUID
            // checks, trap-flag debugger probes, jump-into-next-
            // byte tricks, `rdtsc`-delta timing measurements.
            if cfg.show_antidisasm {
                let prev_pair = prev_instr.map(|p| (p.mnemonic(), p.operands()));
                let next_pair = next_instr.map(|n| (n.mnemonic(), n.operands()));
                let actx = super::super::antidisasm::AntiDisasmContext {
                    prev: prev_pair,
                    current: (instr.mnemonic(), instr.operands()),
                    next: next_pair,
                };
                if let Some(trick) = super::super::antidisasm::detect(&actx) {
                    maybe_separator(&mut emitted_any, ui);
                    ui.text(format!(
                        "{}{}",
                        strings.tooltip_antidisasm_label,
                        trick.description_for(cfg.locale.into(), cfg.verbosity),
                    ));
                }
            }

            // Boundary recogniser — labels function prologues
            // (framed `push rbp; mov rbp, rsp`, CET `endbr64`),
            // epilogues (`leave; ret`, `pop rbp; ret`,
            // `add rsp, N; ret`), bare returns, and block
            // terminators (unconditional `jmp`, conditional
            // `Jcc` forks). Pairs with `idiom`'s prologue idiom
            // — `idiom` recognises the *fact*, `boundary`
            // labels the *boundary*, so the analyst sees both
            // angles.
            if cfg.show_boundary {
                let prev_pair = prev_instr.map(|p| (p.mnemonic(), p.operands()));
                let next_pair = next_instr.map(|n| (n.mnemonic(), n.operands()));
                let bctx = super::super::boundary::BoundaryContext {
                    prev: prev_pair,
                    current: (instr.mnemonic(), instr.operands()),
                    next: next_pair,
                };
                if let Some(b) = super::super::boundary::detect(&bctx) {
                    maybe_separator(&mut emitted_any, ui);
                    ui.text(format!(
                        "{}{}",
                        strings.tooltip_boundary_label,
                        b.description_for(cfg.locale.into(), cfg.verbosity),
                    ));
                }
            }

            // Branch-direction hint — uses the host-resolved
            // `branch_target()` rather than parsing the operand
            // string, so labels and PC-relative encodings work
            // identically. Forward jumps read as
            // `if`/`match`/`switch` skip-overs; backward jumps
            // are almost always loops; self-targeting jumps
            // (`jmp $`) are anti-RE spin traps.
            if cfg.show_branch_direction
                && let Some(target) = instr.branch_target()
            {
                let hint = super::super::branch::classify(addr, target);
                maybe_separator(&mut emitted_any, ui);
                ui.text(format!(
                    "{}{}",
                    strings.tooltip_branch_label,
                    hint.description_for(cfg.locale.into(), cfg.verbosity),
                ));
            }
        });
    }
}
