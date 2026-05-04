//! Function / basic-block boundary recogniser.
//!
//! Reverse engineers spend a lot of time asking "is this the start of
//! a function?", "is this the end?", "where does this basic block
//! end?". The decompilers we don't have answer these questions with
//! full CFG analysis; this module answers them with cheap local
//! pattern-matching on the same prev / current / next triple the
//! [`super::idiom`] / [`super::compiler`] / [`super::antidisasm`]
//! detectors consume.
//!
//! The result is "best-effort, pure, single-line label" — fine for an
//! educational tooltip that nudges newcomers toward the right
//! interpretation. It is **not** a substitute for a real CFG / call-
//! graph; it cannot tell that `call __noreturn` ends a function or
//! that `jmp` into the middle of an instruction kicks off a new
//! block.
//!
//! Pairs with:
//! * [`super::idiom`] — already detects the classic 32/64 prologue
//!   `push rbp; mov rbp, rsp` as a single idiom; this module folds
//!   that into a richer "Function start" boundary and adds the
//!   matching epilogue / block-terminator labels.
//! * [`super::compiler`] — detects the Win64 leaf-frame opening
//!   (`sub rsp, N` without `push rbp`); this module promotes that to
//!   a "Function start (leaf)" boundary when the recognisers agree.

use crate::i18n::Locale;

/// One detected boundary. Locale-aware text via
/// [`Boundary::description`].
#[derive(Debug, Clone, Copy)]
pub struct Boundary {
    /// English single-line description.
    pub en: &'static str,
    /// Russian single-line description.
    pub ru: &'static str,
}

impl Boundary {
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

/// Same shape as the other detectors in this module but kept separate
/// so each detector's API can evolve independently.
pub struct BoundaryContext<'a> {
    /// Previous instruction `(mnemonic, operands)`.
    pub prev: Option<(&'a str, &'a str)>,
    /// Current instruction.
    pub current: (&'a str, &'a str),
    /// Next instruction.
    pub next: Option<(&'a str, &'a str)>,
}

/// Run every boundary recogniser. Returns the first match —
/// recognisers are ordered by specificity so the most informative
/// label wins (e.g. "Function epilogue (leave; ret)" beats the bare
/// "Block terminator (ret)").
#[must_use]
pub fn detect(ctx: &BoundaryContext<'_>) -> Option<Boundary> {
    let (mn, op) = ctx.current;

    // ── 1. Function epilogue ─────────────────────────────────────────────
    //
    // `leave; ret` is the canonical x86 epilogue — `leave` reverses
    // `enter`/`push rbp; mov rbp, rsp; sub rsp, N` and `ret` returns.
    if eq(mn, "leave")
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "ret") || eq(next_m, "retn") || eq(next_m, "retf"))
    {
        return Some(BOUNDARY_EPILOGUE_LEAVE_RET);
    }
    // `pop rbp; ret` — manual leave (no `leave` opcode) — common in
    // hand-written or non-MSVC code.
    if (eq(mn, "ret") || eq(mn, "retn") || eq(mn, "retf"))
        && let Some((prev_m, prev_op)) = ctx.prev
        && eq(prev_m, "pop")
        && (eq(prev_op.trim(), "rbp") || eq(prev_op.trim(), "ebp"))
    {
        return Some(BOUNDARY_EPILOGUE_POP_RET);
    }
    // `add rsp, N; ret` — Win64 leaf epilogue (no frame pointer to
    // restore).
    if (eq(mn, "ret") || eq(mn, "retn") || eq(mn, "retf"))
        && let Some((prev_m, prev_op)) = ctx.prev
        && eq(prev_m, "add")
        && prev_op.split_once(',').is_some_and(|(a, _)| {
            eq(a.trim(), "rsp") || eq(a.trim(), "esp")
        })
    {
        return Some(BOUNDARY_EPILOGUE_LEAF_RET);
    }
    // Bare `ret` / `iret` / `sysret` — function end (block terminator
    // gets a more specific label below if neighbours match).
    if eq(mn, "ret") || eq(mn, "retn") || eq(mn, "retf")
        || eq(mn, "iret") || eq(mn, "iretd") || eq(mn, "iretq")
        || eq(mn, "sysret") || eq(mn, "sysexit")
    {
        return Some(BOUNDARY_FUNCTION_END);
    }

    // ── 2. Function prologue ─────────────────────────────────────────────
    //
    // `push rbp; mov rbp, rsp` (or 32-bit `push ebp; mov ebp, esp`)
    // — the classical frame-pointer setup. The current instruction
    // is the `push`; the next is the `mov`. We anchor on the `push`
    // step so the boundary sits on the entry point row.
    if eq(mn, "push")
        && (eq(op.trim(), "rbp") || eq(op.trim(), "ebp"))
        && let Some((nm, no)) = ctx.next
        && eq(nm, "mov")
        && contains_pair(no, "rbp", "rsp")
    {
        return Some(BOUNDARY_FUNCTION_START_FRAMED);
    }
    // Sometimes the prologue is split with an `endbr64` / `endbr32`
    // (CET shadow-stack landing pad) before the `push rbp`. Match
    // the landing pad as a function start in its own right — the
    // CFI-instrumented entry is a high-confidence anchor.
    if eq(mn, "endbr64") || eq(mn, "endbr32") {
        return Some(BOUNDARY_FUNCTION_START_CET);
    }

    // ── 3. Block-level terminators (other than `ret`, handled above) ─────
    //
    // Unconditional jump ends a block — the next instruction (if
    // reachable at all) is the start of a new block.
    if eq(mn, "jmp") || eq(mn, "jmpf") || eq(mn, "jmpq") {
        return Some(BOUNDARY_BLOCK_END_JMP);
    }
    // Conditional jumps fork a block: the fall-through path *and*
    // the taken path each start a new block.
    if is_jcc(mn) {
        return Some(BOUNDARY_BLOCK_FORK_JCC);
    }

    None
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const BOUNDARY_FUNCTION_START_FRAMED: Boundary = Boundary {
    en: "Function start (framed prologue): `push rbp; mov rbp, rsp` sets up a frame pointer — the entry point of a non-leaf function.",
    ru: "Начало функции (framed prologue): `push rbp; mov rbp, rsp` устанавливает frame-указатель — точка входа non-leaf функции.",
};

const BOUNDARY_FUNCTION_START_CET: Boundary = Boundary {
    en: "Function start (CET landing pad): `endbr64`/`endbr32` is the Intel CET shadow-stack landing pad — the indirect-branch target marker placed at the top of every function compiled with CET.",
    ru: "Начало функции (CET landing pad): `endbr64`/`endbr32` — Intel CET landing pad для shadow-stack — маркер цели indirect-branch в начале каждой функции, скомпилированной с CET.",
};

const BOUNDARY_EPILOGUE_LEAVE_RET: Boundary = Boundary {
    en: "Function epilogue (`leave; ret`): the canonical x86 exit sequence — `leave` undoes `enter` (or its `push rbp; mov rbp, rsp; sub rsp, N` expansion), `ret` returns.",
    ru: "Эпилог функции (`leave; ret`): канонический выход x86 — `leave` отменяет `enter` (или его раскрытие `push rbp; mov rbp, rsp; sub rsp, N`), `ret` возвращает управление.",
};

const BOUNDARY_EPILOGUE_POP_RET: Boundary = Boundary {
    en: "Function epilogue (`pop rbp; ret`): manual frame teardown without the `leave` opcode — common in hand-written assembly or non-MSVC compilers.",
    ru: "Эпилог функции (`pop rbp; ret`): ручная разборка фрейма без `leave` — типично для рукописного асма или non-MSVC компиляторов.",
};

const BOUNDARY_EPILOGUE_LEAF_RET: Boundary = Boundary {
    en: "Leaf-function epilogue (`add rsp, N; ret`): the function uses no frame pointer — the closing `add rsp, N` reverses the matching `sub rsp, N` from the prologue.",
    ru: "Эпилог leaf-функции (`add rsp, N; ret`): функция не использует frame-указатель — закрывающее `add rsp, N` отменяет соответствующее `sub rsp, N` из пролога.",
};

const BOUNDARY_FUNCTION_END: Boundary = Boundary {
    en: "Function return (`ret` / `iret` / `sysret`): hands control back to the caller — the next bytes are usually the start of another function (or alignment padding).",
    ru: "Возврат из функции (`ret` / `iret` / `sysret`): возвращает управление вызывающему — далее обычно следует начало другой функции (или выравнивающий паддинг).",
};

const BOUNDARY_BLOCK_END_JMP: Boundary = Boundary {
    en: "Block terminator (unconditional `jmp`): control transfers unconditionally — the next byte starts a new basic block (only reachable via separate flow).",
    ru: "Конец блока (безусловный `jmp`): управление передаётся безусловно — следующий байт начинает новый basic block (доступный только через другой поток).",
};

const BOUNDARY_BLOCK_FORK_JCC: Boundary = Boundary {
    en: "Block fork (conditional jump): the fall-through and the taken path each start a new basic block.",
    ru: "Развилка блока (условный переход): путь fall-through и путь taken — каждый начинает новый basic block.",
};

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Test whether the operand text is `<dst>, <src>` for the given
/// register pair (case-insensitive, trim-tolerant).
fn contains_pair(op: &str, dst: &str, src: &str) -> bool {
    let Some((a, b)) = op.split_once(',') else {
        return false;
    };
    eq(a.trim(), dst) && eq(b.trim(), src)
}

/// Is `mn` one of the conditional-jump mnemonics? Covers the full
/// short / near Jcc family the disassemblers in common use emit.
fn is_jcc(mn: &str) -> bool {
    matches!(
        mn.to_ascii_lowercase().as_str(),
        "ja" | "jae" | "jb" | "jbe" | "jc" | "je" | "jg" | "jge"
            | "jl" | "jle" | "jna" | "jnae" | "jnb" | "jnbe" | "jnc"
            | "jne" | "jng" | "jnge" | "jnl" | "jnle" | "jno" | "jnp"
            | "jns" | "jnz" | "jo" | "jp" | "jpe" | "jpo" | "js" | "jz"
            | "jcxz" | "jecxz" | "jrcxz" | "loop" | "loope" | "loopne"
            | "loopz" | "loopnz",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        prev: Option<(&'a str, &'a str)>,
        cur: (&'a str, &'a str),
        next: Option<(&'a str, &'a str)>,
    ) -> BoundaryContext<'a> {
        BoundaryContext { prev, current: cur, next }
    }

    #[test]
    fn detects_framed_prologue() {
        let b = detect(&ctx(None, ("push", "rbp"), Some(("mov", "rbp, rsp")))).unwrap();
        assert!(b.en.contains("Function start"));
    }

    #[test]
    fn detects_cet_landing_pad() {
        let b = detect(&ctx(None, ("endbr64", ""), None)).unwrap();
        assert!(b.en.contains("CET"));
    }

    #[test]
    fn detects_leave_ret() {
        let b = detect(&ctx(None, ("leave", ""), Some(("ret", "")))).unwrap();
        assert!(b.en.contains("leave; ret"));
    }

    #[test]
    fn detects_pop_ret() {
        let b = detect(&ctx(Some(("pop", "rbp")), ("ret", ""), None)).unwrap();
        assert!(b.en.contains("pop rbp; ret"));
    }

    #[test]
    fn detects_leaf_ret() {
        let b = detect(&ctx(Some(("add", "rsp, 0x28")), ("ret", ""), None)).unwrap();
        assert!(b.en.contains("Leaf-function"));
    }

    #[test]
    fn detects_bare_ret_as_function_end() {
        // Without a recognisable predecessor, a `ret` is still
        // labelled as a function return (just not the more specific
        // epilogue patterns).
        let b = detect(&ctx(None, ("ret", ""), None)).unwrap();
        assert!(b.en.contains("Function return"));
    }

    #[test]
    fn detects_unconditional_jmp_as_block_end() {
        let b = detect(&ctx(None, ("jmp", "0x401234"), None)).unwrap();
        assert!(b.en.contains("Block terminator"));
    }

    #[test]
    fn detects_jcc_as_block_fork() {
        let b = detect(&ctx(None, ("je", "0x401234"), None)).unwrap();
        assert!(b.en.contains("Block fork"));
    }

    #[test]
    fn no_match_for_plain_mov() {
        assert!(detect(&ctx(None, ("mov", "rax, rbx"), None)).is_none());
    }

    #[test]
    fn no_match_for_arithmetic() {
        assert!(detect(&ctx(None, ("add", "rax, 1"), None)).is_none());
    }

    #[test]
    fn loop_recognised_as_block_fork() {
        // `loop` is a conditional jump (`ecx != 0`).
        let b = detect(&ctx(None, ("loop", "0x401234"), None)).unwrap();
        assert!(b.en.contains("Block fork"));
    }
}
