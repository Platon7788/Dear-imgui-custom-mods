//! Compiler-emit pattern recogniser.
//!
//! Modern compilers (MSVC, Clang, GCC, ICC) lay down stereotyped
//! instruction sequences for things the language doesn't have a
//! single opcode for: process / thread environment access, large
//! stack allocation with `__chkstk`, vtable indirect calls, SEH
//! frame setup, leaf-function recognition, etc. Recognising these
//! sequences turns "hex soup" into "ah, that's a virtual method
//! call".
//!
//! Each recogniser is **pure** and side-effect-free; they take the
//! same `prev / current / next` triple the [`super::idiom`] detector
//! consumes (plus the chosen [`super::abi::Abi`]) and return one
//! [`CompilerPattern`] when a match fires.
//!
//! Pairs with [`super::idiom`] (multi-instruction patterns generic
//! across compilers) and [`super::operand`] (TIB / PEB recognition
//! for the segment-based ones).

use crate::i18n::Locale;

use super::abi::Abi;

/// One recognised compiler pattern. Locale-aware text via
/// [`CompilerPattern::description`].
#[derive(Debug, Clone, Copy)]
pub struct CompilerPattern {
    /// English single-line description.
    pub en: &'static str,
    /// Russian single-line description.
    pub ru: &'static str,
}

impl CompilerPattern {
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

/// Same shape as `idiom::InstructionContext` but kept separate so
/// each module's API can evolve independently. (We could share a
/// type later — for now the duplication is cheap and avoids
/// cross-module coupling.)
pub struct CompilerContext<'a> {
    /// Previous instruction `(mnemonic, operands)`.
    pub prev: Option<(&'a str, &'a str)>,
    /// Current instruction.
    pub current: (&'a str, &'a str),
    /// Next instruction.
    pub next: Option<(&'a str, &'a str)>,
    /// Active calling convention — drives Win64-vs-SysV-specific
    /// recognisers (PEB-via-GS vs PEB-via-FS, shadow-space sub
    /// patterns, etc.).
    pub abi: Abi,
}

/// Run every recogniser. Returns the first match — recognisers are
/// ordered by specificity (most specific first).
#[must_use]
pub fn detect(ctx: &CompilerContext<'_>) -> Option<CompilerPattern> {
    let (mn, op) = ctx.current;

    // ── 1. PEB / TEB / TIB access via segment registers ──────────────────
    //
    // `mov reg, gs:[0x60]` — Win64 PEB pointer. The most reliable
    // anchor for finding loaded modules without a `LoadLibrary`.
    if eq(mn, "mov")
        && (op.contains("gs:[0x60]") || op.contains("gs:[60h]") || op.contains("gs:[96]"))
    {
        return Some(PATTERN_PEB_WIN64);
    }
    // `mov reg, fs:[0x30]` — Win32 PEB pointer (or Win64 WoW64 path).
    if eq(mn, "mov")
        && (op.contains("fs:[0x30]") || op.contains("fs:[30h]") || op.contains("fs:[48]"))
    {
        return Some(PATTERN_PEB_WIN32);
    }
    // `mov reg, gs:[0x30]` — Win64 TEB self-pointer.
    if eq(mn, "mov")
        && (op.contains("gs:[0x30]") || op.contains("gs:[30h]") || op.contains("gs:[48]"))
    {
        return Some(PATTERN_TEB_SELF_WIN64);
    }
    // `mov reg, fs:[0x18]` — Win32 TIB self-pointer.
    if eq(mn, "mov")
        && (op.contains("fs:[0x18]") || op.contains("fs:[18h]") || op.contains("fs:[24]"))
    {
        return Some(PATTERN_TIB_SELF_WIN32);
    }
    // `mov fs:[0], esp` — Win32 SEH frame install (closing step of
    // a `try`/`__try` setup). Must run before the generic
    // `fs:[0]` chain-access matcher so the stricter pattern wins.
    if eq(mn, "mov")
        && contains_after_size_prefix(op, "fs:[0]")
        && op.split_once(',').is_some_and(|(lhs, rhs)| {
            lhs.contains("fs:[0]") && eq(rhs.trim(), "esp")
        })
    {
        return Some(PATTERN_SEH_INSTALL_WIN32);
    }
    // `mov fs:[0], <something other than esp>` — Win32 SEH frame
    // removal (handler chain restored on scope exit).
    if eq(mn, "mov")
        && contains_after_size_prefix(op, "fs:[0]")
        && op.split_once(',').is_some_and(|(lhs, rhs)| {
            lhs.contains("fs:[0]") && !eq(rhs.trim(), "esp")
        })
    {
        return Some(PATTERN_SEH_UNINSTALL_WIN32);
    }
    // `mov reg, fs:[0]` — Win32 SEH chain head read.
    if eq(mn, "mov") && (op.contains("fs:[0]") || op.contains("fs:[0x0]")) {
        return Some(PATTERN_SEH_CHAIN_WIN32);
    }

    // ── 2. Stack probe via __chkstk ──────────────────────────────────────
    //
    // Compilers emit a `call __chkstk` (or `call _alloca_probe` on
    // older MSVC) when a function allocates more than one page on
    // the stack. The probe walks the new range to make sure the
    // stack guard page gets a touch.
    if eq(mn, "call")
        && (op.contains("__chkstk") || op.contains("_chkstk")
            || op.contains("_alloca_probe") || op.contains("_alloca"))
    {
        return Some(PATTERN_CHKSTK);
    }

    // ── 3. Win64 leaf function — `sub rsp, N`/`add rsp, N` w/o `push rbp` ─
    //
    // Detect the `sub rsp, N` opening of a leaf function (no frame
    // pointer). The trigger is `sub rsp, N` *without* a preceding
    // `push rbp`. We only flag it when the previous instruction is
    // **not** a `push rbp`, hinting at the leaf-style frame.
    if ctx.abi == Abi::Win64
        && eq(mn, "sub")
        && op.split_once(',').is_some_and(|(a, _)| eq(a.trim(), "rsp"))
        && !ctx.prev.is_some_and(|(pm, po)| eq(pm, "push") && eq(po.trim(), "rbp"))
    {
        return Some(PATTERN_WIN64_LEAF_FRAME);
    }

    // ── 4. Vtable indirect call ──────────────────────────────────────────
    //
    // Two-step pattern: load vtable pointer (`mov reg, [obj]`),
    // then call through a slot (`call [reg+slot]`). The signature
    // we match is the `call` step where the operand is
    // `[reg+disp]` with `disp >= 0` and base is a non-stack
    // register. Strip an optional `qword ptr` / `dword ptr` / etc.
    // size prefix so `call qword ptr [rax+0x10]` matches as well.
    if eq(mn, "call") {
        let trimmed = strip_size_prefix(op.trim());
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if let Some((base, _)) = inner.split_once('+')
                && !is_stack_or_ip(base.trim())
            {
                return Some(PATTERN_VTABLE_CALL);
            }
            // Also matches a bare `call [reg]` — slot 0 of a vtable.
            if !inner.contains('+') && !inner.contains('-')
                && !inner.contains(':') && !is_stack_or_ip(inner.trim())
            {
                return Some(PATTERN_VTABLE_CALL_SLOT0);
            }
        }
    }

    // ── 6. SEH4 / __security_cookie pattern (MSVC) ───────────────────────
    //
    // `mov reg, [__security_cookie]; xor reg, ebp; mov [rsp+...], reg`
    // — MSVC's `/GS` stack-canary load. We anchor on the
    // `__security_cookie` literal in the operand text so we don't
    // need full label-resolution.
    if eq(mn, "mov") && op.contains("__security_cookie") {
        return Some(PATTERN_SECURITY_COOKIE);
    }

    // ── 7. Atomic CAS (`lock cmpxchg`) ───────────────────────────────────
    //
    // The `lock` prefix is its own mnemonic in many disassemblers —
    // we match it as the previous instruction and `cmpxchg` as the
    // current one. Most modern decoders fold `lock cmpxchg` into a
    // single mnemonic, so also match the standalone form.
    if eq(mn, "cmpxchg") {
        return Some(PATTERN_ATOMIC_CAS);
    }
    if eq(mn, "lock")
        && let Some((next_m, _)) = ctx.next
        && eq(next_m, "cmpxchg")
    {
        return Some(PATTERN_ATOMIC_CAS);
    }

    // ── 8. Atomic RMW (`lock xadd`, `lock xchg`, `lock add`) ─────────────
    if eq(mn, "lock")
        && let Some((next_m, _)) = ctx.next
        && (eq(next_m, "xadd") || eq(next_m, "xchg")
            || eq(next_m, "add") || eq(next_m, "or") || eq(next_m, "and"))
    {
        return Some(PATTERN_ATOMIC_RMW);
    }

    // ── 9. CPU feature detection — `xor ecx, ecx; cpuid` / `mov eax, N; cpuid` ─
    if eq(mn, "cpuid") {
        return Some(PATTERN_CPUID_DETECT);
    }

    // ── 10. Indirect tail-call (`jmp [reg]` / `jmp reg`) — common in DLLs and trampolines ─
    if eq(mn, "jmp") {
        let trimmed = strip_size_prefix(op.trim());
        // Pure register: 1- or 2-letter (`r8`/`r15`) or 3-letter (`rax`/`r12d`).
        if trimmed.len() <= 4
            && trimmed.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Some(PATTERN_INDIRECT_TAIL_JMP);
        }
        // Memory indirect: `[reg]`, `[reg+disp]`, `[rip+disp]` — common
        // in IAT thunks (Win) and PLT stubs (ELF).
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.contains("rip") || inner.contains("eip") {
                return Some(PATTERN_IAT_THUNK);
            }
            return Some(PATTERN_INDIRECT_TAIL_JMP);
        }
    }

    None
}

// ── Catalogue ────────────────────────────────────────────────────────────────

const PATTERN_PEB_WIN64: CompilerPattern = CompilerPattern {
    en: "Win64 PEB load: `gs:[0x60]` is the canonical pointer to the Process Environment Block — used to enumerate loaded modules / read PEB.BeingDebugged / etc. without calling an API.",
    ru: "Загрузка Win64 PEB: `gs:[0x60]` — канонический указатель на Process Environment Block — позволяет перечислить загруженные модули, прочитать PEB.BeingDebugged и т.д. без вызова API.",
};

const PATTERN_PEB_WIN32: CompilerPattern = CompilerPattern {
    en: "Win32 PEB load: `fs:[0x30]` is the 32-bit equivalent of the Win64 PEB pointer (TEB.ProcessEnvironmentBlock).",
    ru: "Загрузка Win32 PEB: `fs:[0x30]` — 32-битный аналог указателя на PEB (TEB.ProcessEnvironmentBlock).",
};

const PATTERN_TEB_SELF_WIN64: CompilerPattern = CompilerPattern {
    en: "Win64 TEB self-pointer load: `gs:[0x30]` returns the address of the TEB itself, the standard way to grab a TEB pointer in compiler-generated code.",
    ru: "Self-указатель Win64 TEB: `gs:[0x30]` возвращает адрес самого TEB — стандартный способ получить TEB в компилируемом коде.",
};

const PATTERN_TIB_SELF_WIN32: CompilerPattern = CompilerPattern {
    en: "Win32 TIB self-pointer load: `fs:[0x18]` returns the address of the TIB itself.",
    ru: "Self-указатель Win32 TIB: `fs:[0x18]` возвращает адрес самого TIB.",
};

const PATTERN_SEH_CHAIN_WIN32: CompilerPattern = CompilerPattern {
    en: "Win32 SEH chain access: `fs:[0]` is the head of the structured-exception-handler linked list. Reading it is part of `try`/`__try` setup; writing it installs a new handler.",
    ru: "Win32 SEH chain: `fs:[0]` — голова цепочки SEH-обработчиков. Чтение — настройка `try`/`__try`; запись — установка нового обработчика.",
};

const PATTERN_CHKSTK: CompilerPattern = CompilerPattern {
    en: "Stack probe (`__chkstk`): the function is about to allocate more than one page on the stack — `__chkstk` walks the range page by page to ensure the OS commits memory and the guard page fires correctly.",
    ru: "Stack probe (`__chkstk`): функция собирается выделить на стеке больше одной страницы — `__chkstk` пройдёт диапазон постранично, чтобы ОС зафиксировала память и сработала guard-страница.",
};

const PATTERN_WIN64_LEAF_FRAME: CompilerPattern = CompilerPattern {
    en: "Win64 leaf-function frame: `sub rsp, N` without a preceding `push rbp` — the function uses no frame pointer and won't need to be unwound dynamically (typical for small leaf helpers).",
    ru: "Win64 leaf-функция: `sub rsp, N` без предшествующего `push rbp` — функция не использует frame-указатель и не требует динамической раскрутки (типично для маленьких leaf-хелперов).",
};

const PATTERN_VTABLE_CALL: CompilerPattern = CompilerPattern {
    en: "Vtable indirect call: `call [reg+slot]` — invoking a virtual method through the vtable pointer in `reg`. The slot offset / 8 is the method index in the vtable layout.",
    ru: "Косвенный вызов через vtable: `call [reg+slot]` — вызов виртуального метода через указатель на vtable в `reg`. `slot` / 8 — индекс метода в раскладке vtable.",
};

const PATTERN_VTABLE_CALL_SLOT0: CompilerPattern = CompilerPattern {
    en: "Vtable slot-0 call: `call [reg]` — invoking the first virtual method (or the destructor in some layouts) through the vtable pointer in `reg`.",
    ru: "Vtable slot-0: `call [reg]` — вызов первого виртуального метода (или деструктора в некоторых раскладках) через указатель на vtable в `reg`.",
};

const PATTERN_SEH_INSTALL_WIN32: CompilerPattern = CompilerPattern {
    en: "Win32 SEH frame install: `mov fs:[0], esp` links a new exception-handler record into the SEH chain head — this is the closing step of a `try`/`__try` setup sequence.",
    ru: "Установка SEH-фрейма (Win32): `mov fs:[0], esp` подвязывает новую запись обработчика в голову цепочки SEH — финальный шаг настройки `try`/`__try`.",
};

const PATTERN_SEH_UNINSTALL_WIN32: CompilerPattern = CompilerPattern {
    en: "Win32 SEH frame removal: a value other than `esp` is being written to `fs:[0]` — the previous handler is being restored when leaving a `try`/`__try` scope.",
    ru: "Снятие SEH-фрейма (Win32): в `fs:[0]` пишется значение, отличное от `esp` — восстанавливается предыдущий обработчик при выходе из `try`/`__try`.",
};

const PATTERN_SECURITY_COOKIE: CompilerPattern = CompilerPattern {
    en: "MSVC `/GS` stack canary: `__security_cookie` is loaded into a register, mixed with the frame pointer (or RBP), and stashed on the stack — a tampering check reruns the mix on epilogue and `__report_gsfailure`s on mismatch.",
    ru: "MSVC `/GS` stack canary: `__security_cookie` загружается в регистр, смешивается с frame-указателем (или RBP) и сохраняется на стеке — на эпилоге смешение повторяется и при несовпадении вызывается `__report_gsfailure`.",
};

const PATTERN_ATOMIC_CAS: CompilerPattern = CompilerPattern {
    en: "Atomic compare-and-swap: `lock cmpxchg` is the foundation of every lock-free CAS-based primitive (mutexes, atomic refcounts, hazard pointers).",
    ru: "Атомарный compare-and-swap: `lock cmpxchg` — фундамент всех lock-free CAS-операций (мьютексы, атомарные refcount'ы, hazard pointers).",
};

const PATTERN_ATOMIC_RMW: CompilerPattern = CompilerPattern {
    en: "Atomic read-modify-write — `lock`-prefixed RMW under the hood of every `Atomic{Add,Or,And,Xchg}` operation in C++/Rust.",
    ru: "Атомарный read-modify-write — `lock`-префиксная RMW под капотом любых `Atomic{Add,Or,And,Xchg}` в C++/Rust.",
};

const PATTERN_CPUID_DETECT: CompilerPattern = CompilerPattern {
    en: "CPU-feature / vendor detection: the leaf is selected by EAX (and ECX for some extended leaves), the answer comes back in EAX/EBX/ECX/EDX. Anti-VM checks read leaves 0x40000000+ for hypervisor vendor strings.",
    ru: "Определение CPU / vendor'а: лист выбирается через EAX (а для расширенных — и через ECX), результат — в EAX/EBX/ECX/EDX. Анти-VM проверки читают листы 0x40000000+ ради vendor-строк гипервизора.",
};

const PATTERN_INDIRECT_TAIL_JMP: CompilerPattern = CompilerPattern {
    en: "Indirect tail-jump: `jmp reg` or `jmp [reg+disp]` — the function is forwarding control without a return record. Common in trampolines, dispatch tables, and thunk wrappers.",
    ru: "Косвенный tail-jump: `jmp reg` или `jmp [reg+disp]` — функция передаёт управление дальше без записи возврата. Типично для трамплинов, dispatch-таблиц и thunk-обёрток.",
};

const PATTERN_IAT_THUNK: CompilerPattern = CompilerPattern {
    en: "IAT / GOT thunk: `jmp [rip+disp]` (Win64 IAT) or `jmp [eip+disp]` / `jmp ds:[label]` (Win32 IAT, ELF GOT) — the import address table lookup that lands in the imported function. Tail-call style.",
    ru: "IAT / GOT thunk: `jmp [rip+disp]` (Win64 IAT) или `jmp [eip+disp]` / `jmp ds:[label]` (Win32 IAT, ELF GOT) — переход по таблице импортов в импортируемую функцию. Tail-call стиль.",
};

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn is_stack_or_ip(reg: &str) -> bool {
    matches!(
        reg.trim().to_ascii_lowercase().as_str(),
        "rsp" | "esp" | "sp" | "rbp" | "ebp" | "bp" | "rip" | "eip",
    )
}

/// Strip a leading `qword ptr` / `dword ptr` / `word ptr` / `byte ptr`
/// / `xmmword ptr` / `ymmword ptr` size hint that disassemblers (e.g.
/// MSVC `dumpbin`, IDA, Capstone with explicit-size mode) prepend to
/// memory operands. Returns the operand without the prefix and any
/// surrounding whitespace; idempotent if no prefix is present.
fn strip_size_prefix(op: &str) -> &str {
    let lower = op.trim_start();
    for prefix in [
        "qword ptr ",
        "dword ptr ",
        "word ptr ",
        "byte ptr ",
        "xmmword ptr ",
        "ymmword ptr ",
        "zmmword ptr ",
        "tbyte ptr ",
        "fword ptr ",
    ] {
        if lower.len() >= prefix.len()
            && lower[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            return lower[prefix.len()..].trim_start();
        }
    }
    lower
}

/// Like `op.contains(needle)` but tolerant of a leading size hint
/// (`dword ptr fs:[0]` still matches the `fs:[0]` needle even though
/// the literal substring is preceded by `dword ptr `). Used by the
/// SEH frame matcher.
fn contains_after_size_prefix(op: &str, needle: &str) -> bool {
    op.contains(needle) || strip_size_prefix(op).contains(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        prev: Option<(&'a str, &'a str)>,
        cur: (&'a str, &'a str),
        next: Option<(&'a str, &'a str)>,
        abi: Abi,
    ) -> CompilerContext<'a> {
        CompilerContext { prev, current: cur, next, abi }
    }

    #[test]
    fn detects_peb_win64() {
        let p = detect(&ctx(None, ("mov", "rax, gs:[0x60]"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("PEB"));
        assert!(p.ru.contains("PEB"));
    }

    #[test]
    fn detects_peb_win32() {
        let p = detect(&ctx(None, ("mov", "eax, fs:[0x30]"), None, Abi::Cdecl)).unwrap();
        assert!(p.en.contains("Win32 PEB"));
    }

    #[test]
    fn detects_chkstk() {
        let p = detect(&ctx(None, ("call", "__chkstk"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("Stack probe"));
    }

    #[test]
    fn detects_win64_leaf_frame() {
        // No `push rbp` immediately before — it's a leaf frame.
        let p = detect(&ctx(
            Some(("xor", "eax, eax")),
            ("sub", "rsp, 0x28"),
            None,
            Abi::Win64,
        )).unwrap();
        assert!(p.en.contains("leaf-function"));
    }

    #[test]
    fn does_not_misclassify_classic_prologue_as_leaf() {
        // `push rbp` then `sub rsp, 0x20` is a classical frame, not leaf.
        let p = detect(&ctx(
            Some(("push", "rbp")),
            ("sub", "rsp, 0x20"),
            None,
            Abi::Win64,
        ));
        assert!(p.is_none());
    }

    #[test]
    fn detects_vtable_call() {
        let p = detect(&ctx(None, ("call", "qword ptr [rax+0x10]"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("vtable") || p.en.contains("Vtable"));
    }

    #[test]
    fn detects_vtable_slot0() {
        let p = detect(&ctx(None, ("call", "qword ptr [rcx]"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("slot-0"));
    }

    #[test]
    fn detects_seh_install_win32() {
        let p = detect(&ctx(None, ("mov", "dword ptr fs:[0], esp"), None, Abi::Cdecl)).unwrap();
        assert!(p.en.contains("SEH frame install"));
    }

    #[test]
    fn detects_security_cookie() {
        let p = detect(&ctx(None, ("mov", "rax, [__security_cookie]"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("/GS") || p.en.contains("canary"));
    }

    #[test]
    fn detects_atomic_cas() {
        let p1 = detect(&ctx(None, ("cmpxchg", "[rax], rdx"), None, Abi::Win64)).unwrap();
        assert!(p1.en.contains("compare-and-swap"));
        let p2 = detect(&ctx(None, ("lock", ""), Some(("cmpxchg", "[rax], rdx")), Abi::Win64)).unwrap();
        assert!(p2.en.contains("compare-and-swap"));
    }

    #[test]
    fn detects_cpuid() {
        let p = detect(&ctx(Some(("xor", "ecx, ecx")), ("cpuid", ""), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("CPU-feature") || p.en.contains("vendor"));
    }

    #[test]
    fn detects_iat_thunk() {
        let p = detect(&ctx(None, ("jmp", "qword ptr [rip+0x1234]"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("IAT") || p.en.contains("GOT"));
    }

    #[test]
    fn detects_indirect_tail_jmp_register() {
        let p = detect(&ctx(None, ("jmp", "rax"), None, Abi::Win64)).unwrap();
        assert!(p.en.contains("Indirect tail-jump"));
    }

    #[test]
    fn no_match_for_normal_mov() {
        let p = detect(&ctx(None, ("mov", "rax, rbx"), None, Abi::Win64));
        assert!(p.is_none());
    }
}
