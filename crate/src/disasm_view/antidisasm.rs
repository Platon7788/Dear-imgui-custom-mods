//! Anti-disasm / anti-debug trick recogniser.
//!
//! Protectors and obfuscators (VMProtect, Themida, Obsidium, Enigma,
//! a long tail of indie packers) lay down sequences whose **whole
//! point** is to confuse the disassembler or detect a debugger. The
//! analyst can spot them by hand — but it takes time to learn the
//! shapes. This module makes the disasm tooltip do the spotting for
//! the analyst, with a one-line plain-language heads-up:
//!
//! * **Opaque predicate** — `mov reg, X; cmp reg, X; jne dead_branch` —
//!   the comparison's outcome is statically known but the disassembler
//!   has to follow both paths and the dead branch is junk.
//! * **Stack-based control flow** — `push target; ret` — bypasses
//!   CFG / ROP-style indirect dispatch.
//! * **Junk after unconditional jump** — `jmp X; <random byte>` —
//!   the byte right after a `jmp` is decoded into a phantom
//!   instruction by linear-sweep tools.
//! * **Self-modifying code marker** — `xor [rip+disp], reg` etc. —
//!   protector decryption stub writing back into the code stream.
//! * **Debugger probes** — `int 3` standalone, `int 2D`,
//!   `pushf; pop reg; test reg, 0x100` — see [`super::idiom`] /
//!   [`super::mnemonic`] for the simpler ones; this module catches
//!   the multi-step probes the idiom detector misses.
//! * **Anti-VM via CPUID** — `mov eax, 1; cpuid; bt ecx, 31` reads
//!   the hypervisor-present bit; `mov eax, 0x40000000; cpuid` reads
//!   the hypervisor vendor string.
//!
//! All recognisers are pure / no-alloc; they consume the same
//! prev/cur/next triple as the [`super::idiom`] and [`super::compiler`]
//! detectors and emit one [`AntiDisasmTrick`] on a hit.

use crate::i18n::Locale;

/// One recognised anti-disasm / anti-debug trick.
#[derive(Debug, Clone, Copy)]
pub struct AntiDisasmTrick {
    /// English description.
    pub en: &'static str,
    /// Russian description.
    pub ru: &'static str,
}

impl AntiDisasmTrick {
    /// Locale-appropriate description.
    #[inline]
    #[must_use]
    pub fn description(&self, locale: Locale) -> &'static str {
        match locale {
            Locale::En => self.en,
            Locale::Ru => self.ru,
        }
    }
}

/// Same shape as the other detectors' contexts — kept independent on
/// purpose so each module can evolve freely.
pub struct AntiDisasmContext<'a> {
    /// Previous instruction.
    pub prev: Option<(&'a str, &'a str)>,
    /// Current instruction.
    pub current: (&'a str, &'a str),
    /// Next instruction.
    pub next: Option<(&'a str, &'a str)>,
}

/// Run every recogniser. Returns the first match — recognisers are
/// ordered by specificity (most specific first).
#[must_use]
pub fn detect(ctx: &AntiDisasmContext<'_>) -> Option<AntiDisasmTrick> {
    let (mn, op) = ctx.current;

    // ── 1. Stack-based control flow: `push target; ret` ─────────────────
    //
    // The defining ROP/JOP gadget signature; legitimate code never
    // emits `push imm32; ret` deliberately. (`pop reg; ret` is
    // covered by the idiom detector — it's also a ROP signal but
    // shows up in normal epilogues, so we leave it there.)
    if eq(mn, "push")
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "ret") || eq(next_m, "retn"))
        && (op.starts_with("0x") || op.chars().next().is_some_and(|c| c.is_ascii_digit()))
    {
        return Some(TRICK_PUSH_RET_CFG);
    }

    // ── 2. Opaque predicate: identical compare → conditional branch ─────
    //
    // `mov reg, X` ... `cmp reg, X` ... `jne` — the test's outcome
    // is statically true (or false), but the disassembler still
    // walks the dead path. We match the **`cmp imm, imm`** form
    // (constant on both sides) and the **`cmp reg, X` after `mov
    // reg, X`** form.
    if eq(mn, "cmp") {
        if let Some((a, b)) = split_two(op)
            && both_are_immediates(a, b)
            && let Some((next_m, _)) = ctx.next
            && is_jcc(next_m)
        {
            return Some(TRICK_OPAQUE_CONST);
        }
        if let Some((dst, src)) = split_two(op)
            && let Some((prev_m, prev_op)) = ctx.prev
            && eq(prev_m, "mov")
            && let Some((mov_dst, mov_src)) = split_two(prev_op)
            && eq(mov_dst, dst)
            && eq(mov_src, src)
        {
            return Some(TRICK_OPAQUE_AFTER_MOV);
        }
    }

    // ── 3. Self-modifying code: write into RIP-relative or fixed code address ─
    if (eq(mn, "mov") || eq(mn, "xor") || eq(mn, "add") || eq(mn, "sub")
        || eq(mn, "or") || eq(mn, "and"))
        && let Some((dst, _)) = split_two(op)
        && dst.starts_with('[')
        && (dst.contains("rip") || dst.contains("eip"))
    {
        return Some(TRICK_SMC_RIP_WRITE);
    }

    // ── 4. Anti-VM via CPUID hypervisor-bit / vendor-leaf ───────────────
    //
    // The setup steps before `cpuid` are what give it away:
    // `mov eax, 1; cpuid` then `bt ecx, 31` — hypervisor-present.
    // `mov eax, 0x40000000; cpuid` — read hypervisor vendor.
    if eq(mn, "bt")
        && let Some((reg, bit)) = split_two(op)
        && (eq(reg, "ecx") || eq(reg, "rcx"))
        && (eq(bit, "31") || eq(bit, "0x1F") || eq(bit, "31d"))
        && let Some((prev_m, _)) = ctx.prev
        && eq(prev_m, "cpuid")
    {
        return Some(TRICK_HYPERVISOR_BIT);
    }
    if eq(mn, "cpuid")
        && let Some((prev_m, prev_op)) = ctx.prev
        && eq(prev_m, "mov")
        && let Some((dst, src)) = split_two(prev_op)
        && (eq(dst, "eax") || eq(dst, "rax"))
        && (src.contains("0x40000") || src.contains("40000000"))
    {
        return Some(TRICK_HYPERVISOR_VENDOR);
    }

    // ── 5. Trap-flag arming via popf / popfq ────────────────────────────
    //
    // `or [rsp/esp], 0x100; popf(q)` — the protector hands the
    // CPU a Trap Flag, the next instruction triggers a single-step
    // exception that the protector's own SEH handles to detect a
    // debugger.
    if (eq(mn, "popf") || eq(mn, "popfq" ) || eq(mn, "popfd"))
        && let Some((prev_m, prev_op)) = ctx.prev
        && eq(prev_m, "or")
        && (prev_op.contains("rsp") || prev_op.contains("esp"))
        && (prev_op.contains("0x100") || prev_op.contains("100h") || prev_op.contains("256"))
    {
        return Some(TRICK_TRAP_FLAG_ARM);
    }

    // ── 6. Anti-disasm "jump into instruction" — `jmp short $+2` ────────
    //
    // The destination address is two bytes ahead, which lands the
    // disassembler in the middle of what would have been the next
    // instruction. Common in mutation-engine output where the byte
    // after `jmp` is junk that real execution skips over.
    if eq(mn, "jmp") && (op.contains("$+2") || op.contains("+ 2") || op.ends_with("+2")) {
        return Some(TRICK_JMP_INTO_INSTRUCTION);
    }

    // ── 7. Anti-debug timing pair across a span (`rdtsc … rdtsc; sub`) ─
    //
    // Distinct from the `rdtsc; rdtsc` adjacent pair (which the
    // idiom detector already catches): if the **previous**
    // instruction is `rdtsc` and the current one is `sub` taking
    // the previous timestamp, the protector is measuring elapsed
    // cycles right now. We anchor on the `sub` step.
    if eq(mn, "sub")
        && let Some((prev_m, _)) = ctx.prev
        && eq(prev_m, "rdtsc")
    {
        return Some(TRICK_RDTSC_DELTA);
    }

    None
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const TRICK_PUSH_RET_CFG: AntiDisasmTrick = AntiDisasmTrick {
    en: "Stack-based indirect jump (`push imm; ret`) — bypasses CFG / RFG analysis and breaks naive call-graph builders. Treat the immediate as the real jump target.",
    ru: "Косвенный переход через стек (`push imm; ret`) — обходит CFG / RFG-анализ и ломает наивные построители call-графа. Иммедиат — реальная цель перехода.",
};

const TRICK_OPAQUE_CONST: AntiDisasmTrick = AntiDisasmTrick {
    en: "Opaque predicate: `cmp` of two constants whose outcome is statically known — the conditional branch always goes one way; the other path is junk inserted to confuse the disassembler.",
    ru: "Opaque predicate: `cmp` двух констант, результат известен статически — условный переход всегда идёт по одной ветке; другая — мусор для путаницы дизассемблера.",
};

const TRICK_OPAQUE_AFTER_MOV: AntiDisasmTrick = AntiDisasmTrick {
    en: "Opaque predicate: a `mov reg, X` immediately followed by `cmp reg, X` — the comparison is rigged to a known result, so the next Jcc has a predetermined branch.",
    ru: "Opaque predicate: `mov reg, X` сразу за которым `cmp reg, X` — сравнение подтасовано к известному результату, следующий Jcc предсказуем.",
};

const TRICK_SMC_RIP_WRITE: AntiDisasmTrick = AntiDisasmTrick {
    en: "Self-modifying code marker: a write into `[rip+disp]` (or `[eip+disp]`) — the protector is patching code in place. Decryption stubs and runtime polymorphism look like this.",
    ru: "Маркер самомодификации: запись в `[rip+disp]` (или `[eip+disp]`) — протектор патчит код на лету. Так выглядят декрипт-стабы и runtime-полиморфизм.",
};

const TRICK_HYPERVISOR_BIT: AntiDisasmTrick = AntiDisasmTrick {
    en: "Anti-VM check: `bt ecx, 31` after `cpuid` reads bit 31 of CPUID leaf 1 ECX — the \"hypervisor present\" flag. Set ⇒ running inside a VM, protector typically refuses to continue.",
    ru: "Анти-VM проверка: `bt ecx, 31` после `cpuid` читает бит 31 ECX листа 1 — флаг \"hypervisor present\". Установлен ⇒ виртуалка, протектор обычно отказывается работать.",
};

const TRICK_HYPERVISOR_VENDOR: AntiDisasmTrick = AntiDisasmTrick {
    en: "Anti-VM check: `cpuid` with EAX=0x40000000 returns the hypervisor vendor string in EBX/ECX/EDX (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Compared against a blacklist to refuse analysis VMs.",
    ru: "Анти-VM проверка: `cpuid` с EAX=0x40000000 возвращает vendor-строку гипервизора в EBX/ECX/EDX (\"VMwareVMware\", \"KVMKVMKVM\", \"Microsoft Hv\", …). Сравнивается с blacklist'ом, чтобы отказать в analysis-VM.",
};

const TRICK_TRAP_FLAG_ARM: AntiDisasmTrick = AntiDisasmTrick {
    en: "Anti-debug: `or [rsp], 0x100; popf` arms the Trap Flag manually — the next instruction raises a single-step exception that the protector's own SEH catches. If a debugger swallows the exception instead, the protector knows it's being analysed.",
    ru: "Анти-debug: `or [rsp], 0x100; popf` вручную выставляет Trap Flag — следующая инструкция вызовет single-step исключение, которое перехватит SEH протектора. Если отладчик перехватит — протектор поймёт что его анализируют.",
};

const TRICK_JMP_INTO_INSTRUCTION: AntiDisasmTrick = AntiDisasmTrick {
    en: "Anti-disasm: `jmp short $+2` jumps into the byte right after itself, derailing linear-sweep disassemblers. The byte the jump lands on is the real first byte of the next instruction; the byte immediately after `jmp` is junk to confuse static analysis.",
    ru: "Анти-disasm: `jmp short $+2` перепрыгивает в байт сразу после себя, ломая линейный sweep-дизассемблер. Байт, в который попадает прыжок, — реальное начало следующей инструкции; байт сразу после `jmp` — мусор для путаницы статического анализа.",
};

const TRICK_RDTSC_DELTA: AntiDisasmTrick = AntiDisasmTrick {
    en: "Anti-debug timing measurement: the previous `rdtsc` reading is being subtracted right here — a debugger's single-step / interrupt overhead inflates the delta, exposing analysis. Pair this with an earlier `rdtsc; mov [...], edx:eax`.",
    ru: "Анти-debug замер времени: разница со счётчиком тактов от прошлого `rdtsc` вычисляется прямо здесь — single-step / прерывания отладчика раздувают её, выдавая анализ. Парная инструкция — более ранний `rdtsc; mov [...], edx:eax`.",
};

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[inline]
fn split_two(operands: &str) -> Option<(&str, &str)> {
    let (a, b) = operands.split_once(',')?;
    Some((a.trim(), b.trim()))
}

fn both_are_immediates(a: &str, b: &str) -> bool {
    is_immediate(a) && is_immediate(b)
}

fn is_immediate(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with("0x") || t.starts_with("0X") {
        return t[2..].chars().all(|c| c.is_ascii_hexdigit() || c == '_');
    }
    let head = t.as_bytes()[0];
    head.is_ascii_digit() || head == b'-' || head == b'+'
}

fn is_jcc(mnemonic: &str) -> bool {
    matches!(
        mnemonic.to_ascii_lowercase().as_str(),
        "je" | "jz" | "jne" | "jnz" | "jl" | "jnge" | "jle" | "jng"
        | "jg" | "jnle" | "jge" | "jnl" | "jb" | "jc" | "jnae"
        | "jbe" | "jna" | "ja" | "jnbe" | "jae" | "jnc" | "jnb"
        | "js" | "jns" | "jo" | "jno" | "jp" | "jpe" | "jnp" | "jpo"
        | "jcxz" | "jecxz" | "jrcxz"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        prev: Option<(&'a str, &'a str)>,
        cur: (&'a str, &'a str),
        next: Option<(&'a str, &'a str)>,
    ) -> AntiDisasmContext<'a> {
        AntiDisasmContext { prev, current: cur, next }
    }

    #[test]
    fn detects_push_ret_indirect() {
        let t = detect(&ctx(None, ("push", "0x401000"), Some(("ret", "")))).unwrap();
        assert!(t.en.contains("Stack-based"));
    }

    #[test]
    fn ignores_push_reg_ret_for_this_module() {
        // `push reg; ret` is the more general ROP gadget — the
        // idiom detector handles it. We only flag `push imm; ret`.
        let t = detect(&ctx(None, ("push", "rax"), Some(("ret", ""))));
        assert!(t.is_none());
    }

    #[test]
    fn detects_opaque_constant_compare() {
        let t = detect(&ctx(None, ("cmp", "5, 5"), Some(("jne", "0x401000")))).unwrap();
        assert!(t.en.contains("Opaque predicate"));
    }

    #[test]
    fn detects_opaque_after_mov() {
        let t = detect(&ctx(
            Some(("mov", "eax, 5")),
            ("cmp", "eax, 5"),
            Some(("jne", "0x401000")),
        )).unwrap();
        assert!(t.en.contains("Opaque predicate"));
    }

    #[test]
    fn detects_smc_rip_write() {
        let t = detect(&ctx(None, ("xor", "[rip+0x100], eax"), None)).unwrap();
        assert!(t.en.contains("Self-modifying"));
    }

    #[test]
    fn detects_hypervisor_bit() {
        let t = detect(&ctx(
            Some(("cpuid", "")),
            ("bt", "ecx, 31"),
            None,
        )).unwrap();
        assert!(t.en.contains("Anti-VM"));
        assert!(t.en.contains("hypervisor"));
    }

    #[test]
    fn detects_hypervisor_vendor() {
        let t = detect(&ctx(
            Some(("mov", "eax, 0x40000000")),
            ("cpuid", ""),
            None,
        )).unwrap();
        assert!(t.en.contains("vendor"));
    }

    #[test]
    fn detects_trap_flag_arming() {
        let t = detect(&ctx(
            Some(("or", "[rsp], 0x100")),
            ("popf", ""),
            None,
        )).unwrap();
        assert!(t.en.contains("Trap Flag") || t.en.contains("anti-debug") || t.en.contains("Anti-debug"));
    }

    #[test]
    fn detects_jmp_into_next_byte() {
        let t = detect(&ctx(None, ("jmp", "short $+2"), None)).unwrap();
        assert!(t.en.contains("Anti-disasm"));
    }

    #[test]
    fn detects_rdtsc_delta() {
        let t = detect(&ctx(
            Some(("rdtsc", "")),
            ("sub", "eax, [rsp+8]"),
            None,
        )).unwrap();
        assert!(t.en.contains("timing"));
    }

    #[test]
    fn no_match_for_normal_code() {
        let t = detect(&ctx(None, ("mov", "rax, rbx"), None));
        assert!(t.is_none());
    }
}
