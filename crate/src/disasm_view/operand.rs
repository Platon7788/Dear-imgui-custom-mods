//! Operand-string parser + memory-pattern decoder.
//!
//! Most disassemblers print operands in roughly the same format —
//! a register, an immediate, or a `[base + index*scale + disp]`
//! memory expression with optional size + segment overrides. The
//! tooltip reader benefits enormously from a one-line plain-language
//! breakdown of *what that memory expression actually is* — array
//! indexing, struct field access, stack-relative local, RIP-relative
//! global, TIB/PEB pointer, etc.
//!
//! This module is the **smallest** parser that gets us 95 % of
//! real-world cases on Intel syntax (the syntax this crate's
//! `disasm_view` consumes). It deliberately doesn't try to handle
//! every pathological expression a hand-written assembler can
//! produce — anything we can't decode falls back to
//! [`OperandKind::Unknown`] and the tooltip silently skips the
//! "Operand: …" line for that operand.
//!
//! Pairs with [`super::abi`] (register-role lookup feeds the
//! "Argument N" / "stack pointer" annotations) and renders the
//! breakdown via [`explain_memory`].

use crate::i18n::Locale;

use super::abi::{Abi, RegisterRole};

// ── Public types ────────────────────────────────────────────────────────────

/// Anything we can recognise inside a single operand slot. `Unknown`
/// catches SIMD masks, far pointers, complex expressions our parser
/// doesn't handle — the tooltip just suppresses the operand line in
/// that case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandKind<'a> {
    /// A bare register operand (`rcx`, `eax`, `r8d`, `xmm0`, …).
    Register(&'a str),
    /// An immediate integer operand. Sign-aware: `-0x10` is parsed
    /// as `-16`. The `original` slice keeps the syntax so the
    /// tooltip can echo `0xFFFF_FFF0` rather than `-16` if the user
    /// prefers.
    Immediate { value: i128, original: &'a str },
    /// A memory expression — see [`MemoryOperand`].
    Memory(MemoryOperand<'a>),
    /// Anything we couldn't classify. Carries the raw text for echo.
    Unknown(&'a str),
}

/// Operand size declared by an explicit `byte ptr` / `word ptr` /
/// `dword ptr` / `qword ptr` / `xmmword ptr` / `ymmword ptr` /
/// `zmmword ptr` prefix. Most disassemblers omit it when the size
/// is implied by the mnemonic — `MemSize::Implicit` covers that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemSize {
    /// 8-bit access (`byte ptr […]`).
    Byte,
    /// 16-bit access (`word ptr […]`).
    Word,
    /// 32-bit access (`dword ptr […]`).
    Dword,
    /// 64-bit access (`qword ptr […]`).
    Qword,
    /// 128-bit access (`xmmword ptr […]`, SSE).
    Xmmword,
    /// 256-bit access (`ymmword ptr […]`, AVX).
    Ymmword,
    /// 512-bit access (`zmmword ptr […]`, AVX-512).
    Zmmword,
    /// Size implied by the surrounding mnemonic.
    Implicit,
}

impl MemSize {
    /// Width in bits, or `None` for `Implicit`.
    #[must_use]
    pub fn bits(self) -> Option<u16> {
        match self {
            MemSize::Byte    => Some(8),
            MemSize::Word    => Some(16),
            MemSize::Dword   => Some(32),
            MemSize::Qword   => Some(64),
            MemSize::Xmmword => Some(128),
            MemSize::Ymmword => Some(256),
            MemSize::Zmmword => Some(512),
            MemSize::Implicit => None,
        }
    }

    /// Width in bytes, or `None` for `Implicit`.
    #[must_use]
    pub fn bytes(self) -> Option<u16> {
        self.bits().map(|b| b / 8)
    }
}

/// Decoded `[base + index*scale + disp]` memory operand.
///
/// `is_rip_relative` is set when `base` is `rip`/`eip` — those
/// addresses target a location in the same binary and almost always
/// resolve to a global / import / string literal, which the tooltip
/// flags explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryOperand<'a> {
    /// `byte ptr` / `qword ptr` / etc. or `Implicit`.
    pub size: MemSize,
    /// `fs` / `gs` / `cs` / `ds` / `es` / `ss` — None when not specified.
    pub segment: Option<&'a str>,
    /// Base register, e.g. `rcx`, `esp`, `rip`.
    pub base: Option<&'a str>,
    /// Index register, e.g. `rax`, `rbx`.
    pub index: Option<&'a str>,
    /// Scale on the index, one of {1, 2, 4, 8}. `1` when no `*N`.
    pub scale: u8,
    /// Constant displacement — sign-aware, parsed from `+N` / `-N`.
    pub displacement: i128,
    /// `true` when `base` is `rip` / `eip` — RIP-relative addressing.
    pub is_rip_relative: bool,
}

// ── Parser ──────────────────────────────────────────────────────────────────

/// Parse a single operand from the disassembler's text output.
///
/// Best-effort: returns [`OperandKind::Unknown`] for forms we don't
/// understand. The returned slices borrow from `input`, so callers
/// don't allocate.
#[must_use]
pub fn parse(input: &str) -> OperandKind<'_> {
    let s = input.trim();
    if s.is_empty() {
        return OperandKind::Unknown(input);
    }

    // Memory operand: `<size> <seg>:[<expr>]` or `[<expr>]`.
    if s.contains('[') && s.ends_with(']') {
        if let Some(mem) = parse_memory(s) {
            return OperandKind::Memory(mem);
        }
        return OperandKind::Unknown(input);
    }

    // Immediate: starts with digit, `-`, `+`, `0x`.
    let head = s.as_bytes()[0];
    if head.is_ascii_digit() || head == b'-' || head == b'+' {
        if let Some(value) = parse_immediate(s) {
            return OperandKind::Immediate { value, original: input };
        }
        return OperandKind::Unknown(input);
    }

    // Otherwise assume it's a register name. We don't validate against
    // a full register table — the canonical-root logic in `abi.rs`
    // returns `GeneralPurpose` for unknowns, which is the right
    // behaviour for the tooltip.
    OperandKind::Register(s)
}

fn parse_immediate(s: &str) -> Option<i128> {
    let s = s.trim();
    let (negative, rest) = match s.as_bytes().first()? {
        b'-' => (true,  &s[1..]),
        b'+' => (false, &s[1..]),
        _    => (false, s),
    };
    let rest = rest.trim();
    let raw = if let Some(stripped) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        i128::from_str_radix(stripped.trim_end_matches('h'), 16).ok()?
    } else if rest.ends_with('h') || rest.ends_with('H') {
        i128::from_str_radix(&rest[..rest.len() - 1], 16).ok()?
    } else {
        rest.parse::<i128>().ok()?
    };
    Some(if negative { -raw } else { raw })
}

fn parse_memory(input: &str) -> Option<MemoryOperand<'_>> {
    // Optional size prefix: `byte ptr`, `qword ptr`, etc. Anything
    // before the first `[` is the prefix area; segment override
    // (`fs:`, `gs:`) appears just before the `[`.
    let open = input.find('[')?;
    let close = input.rfind(']')?;
    if close < open {
        return None;
    }
    let prefix = input[..open].trim();
    let inner = &input[open + 1..close];

    let (size, after_size) = parse_size_prefix(prefix);
    let (segment, _) = parse_segment(after_size);

    let mut base: Option<&str> = None;
    let mut index: Option<&str> = None;
    let mut scale: u8 = 1;
    let mut disp: i128 = 0;
    let mut is_rip_relative = false;

    // Split `inner` into +/- separated terms while keeping the sign.
    // We split on the **first** delimiter and recurse the rest, so a
    // negative displacement keeps its sign attached.
    let mut rest = inner.trim();
    let mut sign = 1i128;
    while !rest.is_empty() {
        // Find next + / - that's not inside `*` form (there are no
        // nested brackets in Intel syntax memory expressions).
        let term_end = rest[1..]
            .find(['+', '-'])
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let raw_term = rest[..term_end].trim();
        let next_rest = rest[term_end..].trim_start();
        // Determine sign of the **next** term from the leading char,
        // before we strip it.
        let next_sign = match next_rest.as_bytes().first() {
            Some(b'-') => -1i128,
            _          =>  1i128,
        };
        let next_rest = next_rest.trim_start_matches(['+', '-']).trim();

        // Classify this term.
        if let Some((idx, sc)) = parse_index_scale_term(raw_term) {
            // Even the very first scale-bearing term keeps its
            // canonical positive sign — `[reg-rax*8]` is invalid.
            index = Some(idx);
            scale = sc;
        } else if let Some(value) = parse_immediate(raw_term) {
            disp = disp.saturating_add(sign.saturating_mul(value));
        } else if !raw_term.is_empty() {
            // Treat as a register name — first one is the base, the
            // second one (if there's no `*` form) becomes the index
            // with scale=1.
            if base.is_none() {
                base = Some(raw_term);
                if raw_term.eq_ignore_ascii_case("rip") || raw_term.eq_ignore_ascii_case("eip") {
                    is_rip_relative = true;
                }
            } else if index.is_none() {
                index = Some(raw_term);
                scale = 1;
            } else {
                // We've already filled both slots — give up cleanly.
                return None;
            }
        }

        sign = next_sign;
        rest = next_rest;
    }

    Some(MemoryOperand {
        size,
        segment,
        base,
        index,
        scale,
        displacement: disp,
        is_rip_relative,
    })
}

fn parse_size_prefix(prefix: &str) -> (MemSize, &str) {
    let p = prefix.trim();
    // Match longest first so `xmmword` doesn't trip the `word` arm.
    for (kw, size) in [
        ("zmmword ptr", MemSize::Zmmword),
        ("ymmword ptr", MemSize::Ymmword),
        ("xmmword ptr", MemSize::Xmmword),
        ("qword ptr",   MemSize::Qword),
        ("dword ptr",   MemSize::Dword),
        ("word ptr",    MemSize::Word),
        ("byte ptr",    MemSize::Byte),
    ] {
        if p.to_ascii_lowercase().starts_with(kw) {
            return (size, p[kw.len()..].trim());
        }
    }
    (MemSize::Implicit, p)
}

fn parse_segment(prefix: &str) -> (Option<&str>, &str) {
    // Match `fs:`, `gs:`, `cs:`, `ds:`, `es:`, `ss:` at the very end
    // of the prefix area.
    for seg in ["fs", "gs", "cs", "ds", "es", "ss", "FS", "GS", "CS", "DS", "ES", "SS"] {
        let needle = format!("{seg}:");
        if let Some(stripped) = prefix.strip_suffix(&needle) {
            // Return a `&'static str` reference matching the case the caller used.
            return (
                Some(match seg.to_ascii_lowercase().as_str() {
                    "fs" => "fs",
                    "gs" => "gs",
                    "cs" => "cs",
                    "ds" => "ds",
                    "es" => "es",
                    "ss" => "ss",
                    _ => unreachable!(),
                }),
                stripped.trim_end(),
            );
        }
    }
    (None, prefix)
}

/// Parse `<reg>*<scale>` — the only memory-term form that carries a scale.
fn parse_index_scale_term(term: &str) -> Option<(&str, u8)> {
    let (reg, scale_str) = term.split_once('*')?;
    let scale: u8 = scale_str.trim().parse().ok()?;
    if !matches!(scale, 1 | 2 | 4 | 8) {
        return None;
    }
    Some((reg.trim(), scale))
}

// ── Memory-pattern decoder ───────────────────────────────────────────────────

/// Render a one-line plain-language breakdown of `mem` in the active
/// locale, augmented with ABI-specific register-role hints. Returns
/// an empty string when the operand has nothing useful to say (e.g. a
/// bare `[reg]` with no displacement and no annotation-worthy
/// register).
#[must_use]
pub fn explain_memory(mem: &MemoryOperand<'_>, abi: Abi, locale: Locale) -> String {
    // Highest-specificity patterns first — those win over the generic
    // "memory at [base + …]" fallback.

    // ── Segment-specific OS internals ──────────────────────────────
    if let Some(seg) = mem.segment {
        if seg.eq_ignore_ascii_case("fs") {
            return tib_or_tls_hint(mem, locale);
        }
        if seg.eq_ignore_ascii_case("gs") {
            return teb_or_peb_hint(mem, locale);
        }
    }

    // ── RIP-relative — global / import / literal ──────────────────
    if mem.is_rip_relative {
        return match locale {
            Locale::En => format!(
                "RIP-relative memory access (x64): pointer to a global, import, or literal in this binary at offset {:#x} from the next instruction.",
                mem.displacement,
            ),
            Locale::Ru => format!(
                "RIP-relative обращение к памяти (x64): указатель на глобальную переменную / импорт / литерал в этом бинарнике, со смещением {:#x} от следующей инструкции.",
                mem.displacement,
            ),
        };
    }

    // ── Stack-relative ──────────────────────────────────────────────
    if let Some(base) = mem.base
        && (base.eq_ignore_ascii_case("rsp") || base.eq_ignore_ascii_case("esp"))
    {
        return stack_pointer_hint(mem, locale);
    }
    // ── Frame-relative ──────────────────────────────────────────────
    if let Some(base) = mem.base
        && (base.eq_ignore_ascii_case("rbp") || base.eq_ignore_ascii_case("ebp"))
    {
        return frame_pointer_hint(mem, locale);
    }

    // ── Array indexing: base + index*scale (+disp) ─────────────────
    if mem.base.is_some() && mem.index.is_some() && mem.scale > 1 {
        return array_indexing_hint(mem, abi, locale);
    }

    // ── Plain field offset: base + disp (no index) ─────────────────
    if let Some(base) = mem.base
        && mem.index.is_none()
        && mem.displacement != 0
    {
        return field_offset_hint(base, mem, abi, locale);
    }

    // ── Bare base dereference: `[reg]` ─────────────────────────────
    if let Some(base) = mem.base
        && mem.index.is_none()
        && mem.displacement == 0
    {
        return bare_dereference_hint(base, mem.size, abi, locale);
    }

    // ── Absolute address with no register ──────────────────────────
    if mem.base.is_none() && mem.index.is_none() {
        return match locale {
            Locale::En => format!(
                "Absolute memory address {:#x}{}.",
                mem.displacement,
                size_suffix(mem.size, locale),
            ),
            Locale::Ru => format!(
                "Абсолютный адрес памяти {:#x}{}.",
                mem.displacement,
                size_suffix(mem.size, locale),
            ),
        };
    }

    // Generic fallback.
    match locale {
        Locale::En => format!(
            "Memory access at the computed address{}.",
            size_suffix(mem.size, locale),
        ),
        Locale::Ru => format!(
            "Обращение к памяти по вычисленному адресу{}.",
            size_suffix(mem.size, locale),
        ),
    }
}

fn array_indexing_hint(mem: &MemoryOperand<'_>, _abi: Abi, locale: Locale) -> String {
    let base  = mem.base.unwrap_or("?");
    let index = mem.index.unwrap_or("?");
    let elem_label = match mem.scale {
        2 => "16-bit (word)",
        4 => "32-bit (dword)",
        8 => "64-bit (qword)",
        _ => "byte",
    };
    let elem_label_ru = match mem.scale {
        2 => "16-битным (word)",
        4 => "32-битным (dword)",
        8 => "64-битным (qword)",
        _ => "байтовым",
    };
    let disp_clause = match (mem.displacement, locale) {
        (0, _) => String::new(),
        (d, Locale::En) if d > 0 => format!(", then add {:#x} (skip header / first N elements)", d),
        (d, Locale::En)          => format!(", then subtract {:#x}", -d),
        (d, Locale::Ru) if d > 0 => format!(", затем прибавить {:#x} (пропуск заголовка / первых N элементов)", d),
        (d, Locale::Ru)          => format!(", затем вычесть {:#x}", -d),
    };
    match locale {
        Locale::En => format!(
            "Array indexing: `{base}` is the array base, `{index}` is the element index, scaled by {} ({elem_label} elements){disp_clause}.",
            mem.scale,
        ),
        Locale::Ru => format!(
            "Индексация массива: `{base}` — база, `{index}` — индекс элемента, ×{} ({elem_label_ru} элементы){disp_clause}.",
            mem.scale,
        ),
    }
}

fn field_offset_hint(base: &str, mem: &MemoryOperand<'_>, _abi: Abi, locale: Locale) -> String {
    let direction = if mem.displacement >= 0 { '+' } else { '-' };
    let mag = mem.displacement.unsigned_abs();
    match locale {
        Locale::En => format!(
            "Struct-field access: read{} memory at offset {direction}{mag:#x} from `{base}` (typical for `obj->field` style accesses).",
            size_clause(mem.size, locale),
        ),
        Locale::Ru => format!(
            "Доступ к полю структуры: чтение{} памяти со смещением {direction}{mag:#x} от `{base}` (типично для `obj->field`).",
            size_clause(mem.size, locale),
        ),
    }
}

fn bare_dereference_hint(base: &str, size: MemSize, abi: Abi, locale: Locale) -> String {
    let role = super::abi::role(base, abi);
    let role_clause = role_clause_for_dereference(role, base, locale);
    match locale {
        Locale::En => format!(
            "Pointer dereference: read{} memory at the address held in `{base}`{role_clause}.",
            size_clause(size, locale),
        ),
        Locale::Ru => format!(
            "Разыменование указателя: чтение{} памяти по адресу из `{base}`{role_clause}.",
            size_clause(size, locale),
        ),
    }
}

fn stack_pointer_hint(mem: &MemoryOperand<'_>, locale: Locale) -> String {
    let direction = if mem.displacement >= 0 { '+' } else { '-' };
    let mag = mem.displacement.unsigned_abs();
    match locale {
        Locale::En => format!(
            "Stack access at offset {direction}{mag:#x} from the stack pointer — usually a local variable, spill slot, or pushed argument{}.",
            size_clause(mem.size, locale),
        ),
        Locale::Ru => format!(
            "Стек: чтение со смещением {direction}{mag:#x} от указателя стека — обычно локальная переменная, spill-слот или сохранённый аргумент{}.",
            size_clause(mem.size, locale),
        ),
    }
}

fn frame_pointer_hint(mem: &MemoryOperand<'_>, locale: Locale) -> String {
    let direction = if mem.displacement >= 0 { '+' } else { '-' };
    let mag = mem.displacement.unsigned_abs();
    let role_note = if mem.displacement < 0 {
        match locale {
            Locale::En => " (negative offsets ⇒ local variables)",
            Locale::Ru => " (отрицательные смещения ⇒ локальные переменные)",
        }
    } else if mem.displacement >= 16 {
        match locale {
            Locale::En => " (positive offsets ≥ 16 ⇒ stack-passed function arguments)",
            Locale::Ru => " (положительные смещения ≥ 16 ⇒ аргументы функции через стек)",
        }
    } else {
        match locale {
            Locale::En => " (small positive offsets ⇒ saved RBP/return address area)",
            Locale::Ru => " (малые положительные смещения ⇒ область сохранённого RBP / адреса возврата)",
        }
    };
    match locale {
        Locale::En => format!(
            "Frame access at offset {direction}{mag:#x} from the frame pointer{role_note}{}.",
            size_clause(mem.size, locale),
        ),
        Locale::Ru => format!(
            "Frame: смещение {direction}{mag:#x} от frame-указателя{role_note}{}.",
            size_clause(mem.size, locale),
        ),
    }
}

fn tib_or_tls_hint(mem: &MemoryOperand<'_>, locale: Locale) -> String {
    // Common Win32 TIB offsets at fs:[…].
    let well_known = match mem.displacement {
        0x00 => Some(("Win32 SEH chain head (NT_TIB.ExceptionList)",
                       "Голова цепочки SEH (NT_TIB.ExceptionList) в Win32")),
        0x04 => Some(("Stack base (NT_TIB.StackBase)", "База стека (NT_TIB.StackBase)")),
        0x08 => Some(("Stack limit (NT_TIB.StackLimit)", "Лимит стека (NT_TIB.StackLimit)")),
        0x18 => Some(("TIB self-pointer (NT_TIB.Self)",
                       "Self-указатель TIB (NT_TIB.Self) — классический способ получить адрес TIB")),
        0x24 => Some(("Thread ID slot (TIB.ClientId.UniqueThread on Win32)",
                       "Идентификатор потока (TIB.ClientId.UniqueThread в Win32)")),
        0x30 => Some(("Pointer to PEB (Win32 TEB.ProcessEnvironmentBlock)",
                       "Указатель на PEB (TEB.ProcessEnvironmentBlock в Win32) — классический способ дойти до PEB")),
        _    => None,
    };
    match (well_known, locale) {
        (Some((en, _)), Locale::En) => format!("FS-segment access at {:#x}: {en}.", mem.displacement),
        (Some((_, ru)), Locale::Ru) => format!("FS-сегмент по {:#x}: {ru}.", mem.displacement),
        (None, Locale::En) => format!(
            "FS-segment access at {:#x} — Win32 TIB area (or Linux TLS in some setups).",
            mem.displacement,
        ),
        (None, Locale::Ru) => format!(
            "FS-сегмент по {:#x} — область Win32 TIB (или Linux TLS в некоторых конфигурациях).",
            mem.displacement,
        ),
    }
}

fn teb_or_peb_hint(mem: &MemoryOperand<'_>, locale: Locale) -> String {
    // Common Win64 TEB/PEB offsets at gs:[…].
    let well_known = match mem.displacement {
        0x00 => Some(("Win64 SEH chain head (TEB.NtTib.ExceptionList)",
                       "Голова цепочки SEH (TEB.NtTib.ExceptionList) в Win64")),
        0x08 => Some(("Stack base (Win64 TEB)", "База стека (Win64 TEB)")),
        0x10 => Some(("Stack limit (Win64 TEB)", "Лимит стека (Win64 TEB)")),
        0x30 => Some(("TEB self-pointer (Win64 TEB.NtTib.Self)",
                       "Self-указатель TEB (Win64 TEB.NtTib.Self)")),
        0x48 => Some(("Thread ID (Win64 TEB.ClientId.UniqueThread)",
                       "Идентификатор потока (Win64 TEB.ClientId.UniqueThread)")),
        0x60 => Some(("Pointer to PEB (Win64 TEB.ProcessEnvironmentBlock)",
                       "Указатель на PEB (Win64 TEB.ProcessEnvironmentBlock) — классический путь к PEB в x64")),
        _    => None,
    };
    match (well_known, locale) {
        (Some((en, _)), Locale::En) => format!("GS-segment access at {:#x}: {en}.", mem.displacement),
        (Some((_, ru)), Locale::Ru) => format!("GS-сегмент по {:#x}: {ru}.", mem.displacement),
        (None, Locale::En) => format!(
            "GS-segment access at {:#x} — Win64 TEB area (or macOS TLS).",
            mem.displacement,
        ),
        (None, Locale::Ru) => format!(
            "GS-сегмент по {:#x} — область Win64 TEB (или macOS TLS).",
            mem.displacement,
        ),
    }
}

fn role_clause_for_dereference(role: RegisterRole, _base: &str, locale: Locale) -> &'static str {
    // Only annotate dereferences when the role actually adds info.
    match (role, locale) {
        (RegisterRole::Argument(1), Locale::En) => " — that's the 1st integer argument under the active ABI",
        (RegisterRole::Argument(1), Locale::Ru) => " — это 1-й целочисленный аргумент по текущему ABI",
        (RegisterRole::Argument(2), Locale::En) => " — that's the 2nd integer argument under the active ABI",
        (RegisterRole::Argument(2), Locale::Ru) => " — это 2-й целочисленный аргумент по текущему ABI",
        _ => "",
    }
}

fn size_clause(size: MemSize, locale: Locale) -> &'static str {
    match (size, locale) {
        (MemSize::Byte,    Locale::En) => " a byte (8-bit)",
        (MemSize::Byte,    Locale::Ru) => " байта (8 бит)",
        (MemSize::Word,    Locale::En) => " a word (16-bit)",
        (MemSize::Word,    Locale::Ru) => " слова (16 бит)",
        (MemSize::Dword,   Locale::En) => " a dword (32-bit)",
        (MemSize::Dword,   Locale::Ru) => " dword (32 бита)",
        (MemSize::Qword,   Locale::En) => " a qword (64-bit)",
        (MemSize::Qword,   Locale::Ru) => " qword (64 бита)",
        (MemSize::Xmmword, Locale::En) => " a 128-bit XMM value",
        (MemSize::Xmmword, Locale::Ru) => " 128-битного XMM-значения",
        (MemSize::Ymmword, Locale::En) => " a 256-bit YMM value",
        (MemSize::Ymmword, Locale::Ru) => " 256-битного YMM-значения",
        (MemSize::Zmmword, Locale::En) => " a 512-bit ZMM value",
        (MemSize::Zmmword, Locale::Ru) => " 512-битного ZMM-значения",
        (MemSize::Implicit, _) => "",
    }
}

fn size_suffix(size: MemSize, locale: Locale) -> &'static str {
    match (size, locale) {
        (MemSize::Byte,    Locale::En) => " (byte)",
        (MemSize::Byte,    Locale::Ru) => " (байт)",
        (MemSize::Word,    Locale::En) => " (word)",
        (MemSize::Word,    Locale::Ru) => " (word)",
        (MemSize::Dword,   Locale::En) => " (dword)",
        (MemSize::Dword,   Locale::Ru) => " (dword)",
        (MemSize::Qword,   Locale::En) => " (qword)",
        (MemSize::Qword,   Locale::Ru) => " (qword)",
        (MemSize::Xmmword, Locale::En) => " (xmmword)",
        (MemSize::Xmmword, Locale::Ru) => " (xmmword)",
        (MemSize::Ymmword, Locale::En) => " (ymmword)",
        (MemSize::Ymmword, Locale::Ru) => " (ymmword)",
        (MemSize::Zmmword, Locale::En) => " (zmmword)",
        (MemSize::Zmmword, Locale::Ru) => " (zmmword)",
        (MemSize::Implicit, _) => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_register_operand() {
        assert!(matches!(parse("rcx"), OperandKind::Register("rcx")));
        assert!(matches!(parse(" rax "), OperandKind::Register("rax")));
    }

    #[test]
    fn parse_immediate_decimal_and_hex() {
        let imm = parse("42");
        assert!(matches!(imm, OperandKind::Immediate { value: 42, .. }));
        let imm = parse("0x401000");
        assert!(matches!(imm, OperandKind::Immediate { value: 0x401000, .. }));
        let imm = parse("-0x10");
        assert!(matches!(imm, OperandKind::Immediate { value: -16, .. }));
    }

    #[test]
    fn parse_bare_memory_dereference() {
        let m = parse("[rax]");
        let OperandKind::Memory(mem) = m else { panic!("expected memory: {m:?}") };
        assert_eq!(mem.base, Some("rax"));
        assert!(mem.index.is_none());
        assert_eq!(mem.displacement, 0);
        assert_eq!(mem.size, MemSize::Implicit);
    }

    #[test]
    fn parse_base_plus_disp() {
        let OperandKind::Memory(mem) = parse("qword ptr [rbp+0x10]") else { panic!() };
        assert_eq!(mem.base, Some("rbp"));
        assert_eq!(mem.displacement, 0x10);
        assert_eq!(mem.size, MemSize::Qword);
    }

    #[test]
    fn parse_negative_displacement() {
        let OperandKind::Memory(mem) = parse("[rbp-0x8]") else { panic!() };
        assert_eq!(mem.displacement, -0x8);
    }

    #[test]
    fn parse_array_indexing_with_scale() {
        let OperandKind::Memory(mem) = parse("[rcx+rax*8+8]") else { panic!() };
        assert_eq!(mem.base, Some("rcx"));
        assert_eq!(mem.index, Some("rax"));
        assert_eq!(mem.scale, 8);
        assert_eq!(mem.displacement, 8);
    }

    #[test]
    fn parse_rip_relative_in_x64() {
        let OperandKind::Memory(mem) = parse("[rip+0x1000]") else { panic!() };
        assert!(mem.is_rip_relative);
        assert_eq!(mem.displacement, 0x1000);
    }

    #[test]
    fn parse_segment_override_fs() {
        let OperandKind::Memory(mem) = parse("dword ptr fs:[0x30]") else { panic!() };
        assert_eq!(mem.segment, Some("fs"));
        assert_eq!(mem.displacement, 0x30);
        assert_eq!(mem.size, MemSize::Dword);
    }

    #[test]
    fn parse_segment_override_gs() {
        let OperandKind::Memory(mem) = parse("qword ptr gs:[0x60]") else { panic!() };
        assert_eq!(mem.segment, Some("gs"));
        assert_eq!(mem.displacement, 0x60);
    }

    #[test]
    fn explain_array_indexing() {
        let OperandKind::Memory(mem) = parse("[rcx+rax*8+8]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        let ru = explain_memory(&mem, Abi::Win64, Locale::Ru);
        assert!(en.to_lowercase().contains("array indexing"));
        assert!(en.contains("rcx"));
        assert!(en.contains("rax"));
        assert!(ru.contains("Индексация"));
    }

    #[test]
    fn explain_stack_access() {
        let OperandKind::Memory(mem) = parse("qword ptr [rsp+0x20]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        assert!(en.to_lowercase().contains("stack access"));
    }

    #[test]
    fn explain_frame_local_variable() {
        let OperandKind::Memory(mem) = parse("dword ptr [rbp-0x8]") else { panic!() };
        let en = explain_memory(&mem, Abi::SysVAmd64, Locale::En);
        assert!(en.contains("frame pointer") || en.contains("Frame"));
        assert!(en.contains("local variables"));
    }

    #[test]
    fn explain_rip_relative_global() {
        let OperandKind::Memory(mem) = parse("[rip+0x1234]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        assert!(en.contains("RIP-relative"));
        assert!(en.contains("global"));
    }

    #[test]
    fn explain_peb_pointer_via_gs_60() {
        let OperandKind::Memory(mem) = parse("qword ptr gs:[0x60]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        let ru = explain_memory(&mem, Abi::Win64, Locale::Ru);
        assert!(en.contains("PEB"));
        assert!(ru.contains("PEB"));
    }

    #[test]
    fn explain_tib_pointer_via_fs_30() {
        let OperandKind::Memory(mem) = parse("dword ptr fs:[0x30]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        assert!(en.contains("PEB"));
    }

    #[test]
    fn explain_dereference_of_first_arg_under_win64() {
        let OperandKind::Memory(mem) = parse("[rcx]") else { panic!() };
        let en = explain_memory(&mem, Abi::Win64, Locale::En);
        // RCX = 1st arg under Win64 — the dereference hint should
        // surface that.
        assert!(en.contains("1st integer argument"));
    }
}
