//! Multi-instruction idiom detector for the educational tooltip.
//!
//! Single-instruction explanations live in [`super::mnemonic`]; this
//! module recognises **patterns across neighbouring instructions** —
//! prologue / epilogue / NULL-check / get-IP / etc. — and turns them
//! into a one-line plain-language hint that augments the per-opcode
//! description in the hover tooltip.
//!
//! The detector runs against [`InstructionContext`], a thin view of
//! the previous, current, and next instruction the renderer collects
//! from the active [`super::provider::DisasmDataProvider`]. All
//! matches are best-effort and pure — no side effects, no allocation
//! beyond the returned `&'static str`.

use crate::i18n::Locale;

/// View of an instruction with its immediate neighbours, just enough
/// to recognise short idioms (1- to 3-instruction patterns).
///
/// Renderers populate this from the visible instruction window; the
/// detector below treats `prev` / `next` as `None` near the edges.
pub struct InstructionContext<'a> {
    /// Previous instruction's mnemonic + operands, lower-cased
    /// already (callers should pass the raw forms; the detector
    /// does its own `eq_ignore_ascii_case`-style comparisons).
    pub prev: Option<(&'a str, &'a str)>,
    /// Current instruction's mnemonic + operand string.
    pub current: (&'a str, &'a str),
    /// Next instruction's mnemonic + operands.
    pub next: Option<(&'a str, &'a str)>,
}

/// One detected idiom. Locale-aware text comes from
/// [`Idiom::description`] — the catalogue stores both EN/RU forms.
#[derive(Debug, Clone, Copy)]
pub struct Idiom {
    /// English single-line description.
    pub en: &'static str,
    /// Russian single-line description.
    pub ru: &'static str,
}

impl Idiom {
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

/// Run every recogniser against `ctx`. Returns the first match —
/// recognisers are ordered by specificity (most specific first) so
/// e.g. a `xor reg, reg` zeroing idiom wins over a generic "XOR" hit.
#[must_use]
pub fn detect(ctx: &InstructionContext<'_>) -> Option<Idiom> {
    let (mnemonic, operands) = ctx.current;
    let m = mnemonic.trim();
    let op = operands.trim();

    // ── 1. Function prologue / epilogue ─────────────────────────────────
    // `push rbp; mov rbp, rsp` (x64) or `push ebp; mov ebp, esp` (x32).
    if eq(m, "push")
        && let Some((next_m, next_op)) = ctx.next
        && eq(next_m, "mov")
        && (eq(op, "rbp") || eq(op, "ebp"))
        && (contains_pair(next_op, "rbp", "rsp") || contains_pair(next_op, "ebp", "esp"))
    {
        return Some(IDIOM_PROLOGUE);
    }
    // `mov rbp, rsp` standing alone right after a `push rbp` we just
    // saw → still part of the prologue.
    if eq(m, "mov")
        && let Some((prev_m, prev_op)) = ctx.prev
        && eq(prev_m, "push")
        && (eq(prev_op, "rbp") || eq(prev_op, "ebp"))
        && (contains_pair(op, "rbp", "rsp") || contains_pair(op, "ebp", "esp"))
    {
        return Some(IDIOM_PROLOGUE);
    }
    // `leave; ret` epilogue.
    if eq(m, "leave")
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "ret") || eq(next_m, "retn"))
    {
        return Some(IDIOM_EPILOGUE_LEAVE_RET);
    }
    // `pop rbp; ret` epilogue (compilers usually emit this instead of `leave; ret`).
    if (eq(m, "pop") && (eq(op, "rbp") || eq(op, "ebp")))
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "ret") || eq(next_m, "retn"))
    {
        return Some(IDIOM_EPILOGUE_POP_RET);
    }

    // ── 2. Stack frame allocation / cleanup ─────────────────────────────
    // `sub rsp, N` / `sub esp, N` — local variable space.
    if eq(m, "sub") && (starts_with_reg(op, "rsp") || starts_with_reg(op, "esp")) {
        return Some(IDIOM_STACK_ALLOC);
    }
    // `add rsp, N` / `add esp, N` — local variable cleanup before ret.
    if eq(m, "add") && (starts_with_reg(op, "rsp") || starts_with_reg(op, "esp")) {
        return Some(IDIOM_STACK_FREE);
    }

    // ── 3. Idiomatic register zeroing ──────────────────────────────────
    // `xor reg, reg` with both operands the same register.
    if eq(m, "xor")
        && let Some((a, b)) = split_two_operands(op)
        && eq(a, b)
    {
        return Some(IDIOM_ZERO_REG);
    }

    // ── 4. NULL / non-NULL check ───────────────────────────────────────
    // `test reg, reg` — sets ZF based on whether `reg` is zero.
    if eq(m, "test")
        && let Some((a, b)) = split_two_operands(op)
        && eq(a, b)
    {
        return Some(IDIOM_NULL_CHECK);
    }

    // ── 5. Compare-and-branch idioms ───────────────────────────────────
    // `cmp …; jcc …` is the canonical equality / order check.
    if eq(m, "cmp")
        && let Some((next_m, _)) = ctx.next
        && is_jcc(next_m)
    {
        return Some(IDIOM_CMP_BRANCH);
    }
    // Symmetric: `jcc …` immediately after a `cmp` we just saw.
    if is_jcc(m)
        && let Some((prev_m, _)) = ctx.prev
        && (eq(prev_m, "cmp") || eq(prev_m, "test"))
    {
        return Some(IDIOM_CMP_BRANCH);
    }

    // ── 6. Get-IP idiom (x32 PIC) ──────────────────────────────────────
    // `call $+5` followed by `pop reg` — classic position-independent
    // code trick to learn EIP.
    if eq(m, "call")
        && (op.contains("$+5") || op.contains("+ 5"))
    {
        return Some(IDIOM_GET_IP);
    }

    // ── 7. Branch-free -1/0 mask via SBB ───────────────────────────────
    // `sbb reg, reg` produces -1 if CF=1 else 0.
    if eq(m, "sbb")
        && let Some((a, b)) = split_two_operands(op)
        && eq(a, b)
    {
        return Some(IDIOM_SBB_MASK);
    }

    // ── 8. ROP-style gadget (`pop reg; ret`) ───────────────────────────
    if eq(m, "pop")
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "ret") || eq(next_m, "retn"))
        && !(eq(op, "rbp") || eq(op, "ebp"))
    {
        return Some(IDIOM_ROP_GADGET);
    }

    // ── 9. `push X / call func` — argument before a call ───────────────
    if eq(m, "push")
        && let Some((next_m, _)) = ctx.next
        && eq(next_m, "call")
    {
        return Some(IDIOM_PUSH_ARG_CALL);
    }

    // ── 10. `mov reg, X / call` — argument register before a call (x64) ─
    if eq(m, "mov")
        && let Some((next_m, _)) = ctx.next
        && eq(next_m, "call")
        && let Some((dst, _)) = split_two_operands(op)
        && is_x64_arg_register(dst)
    {
        return Some(IDIOM_REG_ARG_CALL);
    }

    // ── 11. Anti-debug: rdtsc-pair timing ──────────────────────────────
    if eq(m, "rdtsc")
        && let Some((prev_m, _)) = ctx.prev
        && eq(prev_m, "rdtsc")
    {
        return Some(IDIOM_RDTSC_PAIR);
    }

    // ── 12. `int 3` debugger breakpoint ───────────────────────────────
    if eq(m, "int") && (op.starts_with('3') || eq(op, "3"))  {
        return Some(IDIOM_INT3_BP);
    }
    // The `int 2D` Windows kernel debugger probe.
    if eq(m, "int") && (op.starts_with("2D") || op.starts_with("2d") || op.starts_with("0x2D") || op.starts_with("0x2d")) {
        return Some(IDIOM_INT2D);
    }

    // ── 13. `nop`-equivalent obfuscation filler ────────────────────────
    if eq(m, "lea")
        && let Some((dst, src)) = split_two_operands(op)
        && operand_is_self_lea(dst, src)
    {
        return Some(IDIOM_LEA_SELF_NOP);
    }
    if eq(m, "mov")
        && let Some((a, b)) = split_two_operands(op)
        && eq(a, b)
    {
        return Some(IDIOM_MOV_SELF_NOP);
    }
    if (eq(m, "rol") || eq(m, "ror")) && (op.ends_with(", 0") || op.ends_with(",0")) {
        return Some(IDIOM_ROTATE_BY_ZERO_NOP);
    }
    if eq(m, "xchg") && eq(op, "eax, eax") {
        return Some(IDIOM_XCHG_EAX_NOP);
    }

    None
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const IDIOM_PROLOGUE: Idiom = Idiom {
    en: "Function prologue: saves the caller's frame pointer and sets up a fresh frame.",
    ru: "Пролог функции: сохраняет фрейм вызывающего и поднимает свой стек-фрейм.",
};

const IDIOM_EPILOGUE_LEAVE_RET: Idiom = Idiom {
    en: "Function epilogue: tears down the frame and returns. `leave; ret` is a one-instruction-shorter `mov rsp, rbp; pop rbp; ret`.",
    ru: "Эпилог функции: сворачивает фрейм и возвращается. `leave; ret` короче, чем `mov rsp, rbp; pop rbp; ret`.",
};

const IDIOM_EPILOGUE_POP_RET: Idiom = Idiom {
    en: "Function epilogue: restores the caller's frame pointer and returns. Compilers prefer this over `leave; ret`.",
    ru: "Эпилог функции: восстанавливает фрейм вызывающего и возвращается. Компиляторы предпочитают это вместо `leave; ret`.",
};

const IDIOM_STACK_ALLOC: Idiom = Idiom {
    en: "Stack-frame allocation: reserving space on the stack for local variables / spills.",
    ru: "Выделение места на стеке под локальные переменные / spill регистров.",
};

const IDIOM_STACK_FREE: Idiom = Idiom {
    en: "Stack-frame cleanup: releasing local-variable space before returning.",
    ru: "Освобождение места на стеке перед возвратом из функции.",
};

const IDIOM_ZERO_REG: Idiom = Idiom {
    en: "Idiomatic zero-out (`xor reg, reg`) — shorter than `mov reg, 0` and breaks the register-rename dependency chain.",
    ru: "Идиома обнуления (`xor reg, reg`) — короче `mov reg, 0` и разрывает цепочку зависимостей в renamer.",
};

const IDIOM_NULL_CHECK: Idiom = Idiom {
    en: "Zero / non-zero check on a register (typically a NULL-pointer or boolean test). The next Jcc reads ZF.",
    ru: "Проверка регистра на 0 / не-0 (часто NULL-указатель или булев флаг). Следующий Jcc читает ZF.",
};

const IDIOM_CMP_BRANCH: Idiom = Idiom {
    en: "Comparison-then-branch idiom: `cmp` (or `test`) sets the flags, the next Jcc decides which branch to take.",
    ru: "Идиома \"сравнение-и-переход\": `cmp` (или `test`) ставит флаги, следующий Jcc решает, по какой ветке идти.",
};

const IDIOM_GET_IP: Idiom = Idiom {
    en: "Get-IP idiom (x32 PIC): `call $+5` pushes the return address onto the stack so the next `pop reg` reveals the current EIP. In x64 use RIP-relative addressing instead.",
    ru: "Идиома получения IP (x32 PIC): `call $+5` кладёт адрес возврата в стек, следующий `pop reg` достаёт текущий EIP. В x64 используется RIP-relative адресация.",
};

const IDIOM_SBB_MASK: Idiom = Idiom {
    en: "Branch-free -1/0 mask: `sbb reg, reg` writes -1 if CF was 1, otherwise 0. Common after `cmp`/`test` for constant-time selection.",
    ru: "Безветвевая -1/0 маска: `sbb reg, reg` даст -1 если CF=1, иначе 0. Часто после `cmp`/`test` для constant-time выбора.",
};

const IDIOM_ROP_GADGET: Idiom = Idiom {
    en: "ROP-style gadget (`pop reg; ret`) — chained by exploits to build computation out of pre-existing return points. Legitimate code uses this only for register restore in epilogues.",
    ru: "ROP-стиль гаджет (`pop reg; ret`) — эксплоиты строят из них вычисления, переходя по существующим возвратам. В легитимном коде встречается только при восстановлении регистров в эпилоге.",
};

const IDIOM_PUSH_ARG_CALL: Idiom = Idiom {
    en: "Argument-then-call (cdecl / stdcall in x32). Each `push` adds one stack-passed argument, then `call` invokes the function. The caller (cdecl) or callee (stdcall) cleans up afterwards.",
    ru: "Аргумент-затем-вызов (cdecl / stdcall в x32). Каждый `push` — один аргумент через стек, затем `call`. Очистка стека: caller (cdecl) или callee (stdcall).",
};

const IDIOM_REG_ARG_CALL: Idiom = Idiom {
    en: "Register-passed argument before a call. In x64 the first integer args go in RCX/RDX/R8/R9 (Win64) or RDI/RSI/RDX/RCX/R8/R9 (SysV); pick by target ABI.",
    ru: "Аргумент через регистр перед вызовом. В x64 первые целочисленные аргументы — в RCX/RDX/R8/R9 (Win64) или RDI/RSI/RDX/RCX/R8/R9 (SysV); зависит от ABI цели.",
};

const IDIOM_RDTSC_PAIR: Idiom = Idiom {
    en: "Anti-debug timing window: a second `rdtsc` follows the first, and the difference reveals if a debugger / single-stepper inflated the interval.",
    ru: "Анти-debug замер времени: второй `rdtsc` следом за первым; разница тактов выдаёт раздутие интервала отладчиком / single-step'ом.",
};

const IDIOM_INT3_BP: Idiom = Idiom {
    en: "`int 3` is the universal debugger software-breakpoint (one-byte 0xCC). Stand-alone uses are anti-debug probes; in normal code only debuggers patch it in.",
    ru: "`int 3` — универсальная программная точка останова отладчика (один байт 0xCC). Отдельная инструкция — анти-debug проверка; в обычном коде её ставит сам отладчик.",
};

const IDIOM_INT2D: Idiom = Idiom {
    en: "`int 2D` is the Windows kernel-debugger probe — execution behaves differently with vs without a kernel debugger attached, exposing the analysis environment.",
    ru: "`int 2D` — проверка на наличие kernel-отладчика Windows: поведение различается с/без отладчика, выдавая среду анализа.",
};

const IDIOM_LEA_SELF_NOP: Idiom = Idiom {
    en: "NOP-equivalent filler: `lea reg, [reg+0]` writes the same value back. Multi-byte form often inserted by mutators to inflate code size without changing semantics.",
    ru: "NOP-эквивалент: `lea reg, [reg+0]` записывает то же значение. Многобайтная форма часто вставляется мутаторами для раздутия кода без смены семантики.",
};

const IDIOM_MOV_SELF_NOP: Idiom = Idiom {
    en: "NOP-equivalent filler: `mov reg, reg` (same source and destination). Mutators chain these to bulk up unimportant regions.",
    ru: "NOP-эквивалент: `mov reg, reg` (источник = приёмник). Мутаторы цепляют такие, чтобы раздуть неважные участки.",
};

const IDIOM_ROTATE_BY_ZERO_NOP: Idiom = Idiom {
    en: "NOP-equivalent filler: rotating by 0 leaves the register unchanged. A common mutator placeholder.",
    ru: "NOP-эквивалент: поворот на 0 не меняет регистр. Типичный заполнитель мутаторов.",
};

const IDIOM_XCHG_EAX_NOP: Idiom = Idiom {
    en: "Single-byte 0x90 NOP — `xchg eax, eax` is the canonical encoding. Mutators don't usually use this since it's already universally recognised.",
    ru: "Однобайтный 0x90 NOP — `xchg eax, eax` это его каноническая кодировка. Мутаторы используют редко: и так известна везде.",
};

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Split a typical two-operand list `"reg, reg"` / `"[mem], reg"` /
/// `"reg, imm"` into trimmed left/right halves. Returns `None` for
/// 0- or 1-operand forms.
#[inline]
fn split_two_operands(operands: &str) -> Option<(&str, &str)> {
    // Find the **first** comma at the top level — operand strings can
    // contain `[base + index*scale + disp]` which embeds no commas, so
    // a plain `split_once(',')` is sufficient.
    let (a, b) = operands.split_once(',')?;
    Some((a.trim(), b.trim()))
}

/// `true` when `operand_string` starts with the named register
/// (case-insensitive) — e.g. `starts_with_reg("rsp, 0x20", "rsp")`.
#[inline]
fn starts_with_reg(operand_string: &str, reg: &str) -> bool {
    operand_string
        .trim_start()
        .as_bytes()
        .iter()
        .zip(reg.as_bytes())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
        && operand_string.trim_start().len() >= reg.len()
}

/// `true` when `operand_string` lists both `dst` and `src` as the two
/// operands (case-insensitive).
#[inline]
fn contains_pair(operand_string: &str, dst: &str, src: &str) -> bool {
    if let Some((a, b)) = split_two_operands(operand_string) {
        eq(a, dst) && eq(b, src)
    } else {
        false
    }
}

/// `true` when an `lea` destination/source pair encodes
/// `lea reg, [reg+0]` — i.e. the destination matches the first
/// register inside the source bracket and there is no displacement.
fn operand_is_self_lea(dst: &str, src: &str) -> bool {
    // strip brackets / leading "byte ptr" / "qword ptr" / etc.
    let inside = src.trim().trim_start_matches(|c: char| c.is_ascii_alphabetic())
        .trim().trim_start_matches("ptr").trim()
        .trim_start_matches('[').trim_end_matches(']').trim();
    // Matches "<dst>" or "<dst>+0" or "<dst> + 0".
    let dst = dst.trim();
    eq(inside, dst)
        || eq(inside.trim_end_matches('0').trim_end_matches('+').trim(), dst)
        || eq(inside.trim_end_matches('0').trim_end().trim_end_matches('+').trim(), dst)
}

/// `true` when `mnemonic` is one of the conditional-jump opcodes — used
/// by the cmp/test+Jcc detector. Matches the same family the mnemonic
/// catalogue documents.
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

/// Recognise the integer-argument registers under x64 ABIs (Win64
/// uses RCX/RDX/R8/R9; SysV uses RDI/RSI/RDX/RCX/R8/R9). A future
/// `crate::disasm_view::abi` module will resolve the position and
/// ABI-specific role; for now we just flag "this is a known arg
/// register".
fn is_x64_arg_register(reg: &str) -> bool {
    let r = reg.trim().to_ascii_lowercase();
    matches!(
        r.as_str(),
        "rcx" | "ecx" | "cx" | "cl"
        | "rdx" | "edx" | "dx" | "dl"
        | "r8"  | "r8d" | "r8w" | "r8b"
        | "r9"  | "r9d" | "r9w" | "r9b"
        | "rdi" | "edi" | "di"  | "dil"
        | "rsi" | "esi" | "si"  | "sil"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        prev: Option<(&'a str, &'a str)>,
        current: (&'a str, &'a str),
        next: Option<(&'a str, &'a str)>,
    ) -> InstructionContext<'a> {
        InstructionContext { prev, current, next }
    }

    #[test]
    fn detects_zero_register_idiom() {
        let c = ctx(None, ("xor", "eax, eax"), None);
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("zero-out"));
        assert!(idiom.ru.contains("обнуления"));
    }

    #[test]
    fn detects_null_check() {
        let c = ctx(None, ("test", "rax, rax"), Some(("je", "0x401000")));
        let idiom = detect(&c).unwrap();
        // `test reg, reg` is more specific than the cmp/test+Jcc pair
        // recogniser, so the NULL-check description wins.
        assert!(idiom.en.contains("zero / non-zero") || idiom.en.contains("Zero / non-zero"));
    }

    #[test]
    fn detects_prologue() {
        // `push rbp` followed by `mov rbp, rsp`.
        let c = ctx(None, ("push", "rbp"), Some(("mov", "rbp, rsp")));
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("prologue"));
        assert!(idiom.ru.contains("Пролог"));
    }

    #[test]
    fn detects_epilogue_leave_ret() {
        let c = ctx(None, ("leave", ""), Some(("ret", "")));
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("epilogue"));
    }

    #[test]
    fn detects_epilogue_pop_ret() {
        let c = ctx(None, ("pop", "rbp"), Some(("ret", "")));
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("epilogue"));
    }

    #[test]
    fn detects_stack_alloc_and_free() {
        let alloc = detect(&ctx(None, ("sub", "rsp, 0x20"), None)).unwrap();
        assert!(alloc.en.contains("Stack"));

        let free = detect(&ctx(None, ("add", "rsp, 0x20"), None)).unwrap();
        assert!(free.en.contains("Stack"));
    }

    #[test]
    fn detects_cmp_branch() {
        let c = ctx(None, ("cmp", "eax, 1"), Some(("je", "0x401000")));
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("Comparison"));
    }

    #[test]
    fn detects_get_ip_idiom() {
        let c = ctx(None, ("call", "$+5"), Some(("pop", "ebx")));
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("Get-IP"));
    }

    #[test]
    fn detects_rdtsc_timing_pair() {
        let c = ctx(Some(("rdtsc", "")), ("rdtsc", ""), None);
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("timing"));
    }

    #[test]
    fn detects_int3_bp() {
        let c = ctx(None, ("int", "3"), None);
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("breakpoint"));
    }

    #[test]
    fn detects_nop_filler_lea_self() {
        let c = ctx(None, ("lea", "rax, [rax+0]"), None);
        let idiom = detect(&c).unwrap();
        assert!(idiom.en.contains("NOP"));
    }

    #[test]
    fn no_match_for_plain_instruction() {
        // A `mov rax, rbx` between unrelated instructions is plain
        // data movement — no idiom.
        let c = ctx(Some(("mov", "rcx, rdx")), ("mov", "rax, rbx"), Some(("add", "rax, 1")));
        assert!(detect(&c).is_none());
    }

    #[test]
    fn descriptions_are_terse() {
        let c = ctx(None, ("xor", "eax, eax"), None);
        let i = detect(&c).unwrap();
        assert!(i.en.chars().count() <= 240, "EN idiom too long");
        assert!(i.ru.chars().count() <= 240, "RU idiom too long");
    }
}
