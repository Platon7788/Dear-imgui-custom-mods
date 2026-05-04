//! Calling-convention awareness for the educational tooltip.
//!
//! Bare register names like `rcx` mean different things depending on
//! the target ABI: Win64 puts the **first integer argument** in RCX,
//! while SysV AMD64 (Linux / macOS) puts the **fourth** argument
//! there and uses RDI for the first. Without ABI context the idiom
//! detector can only say "register-passed argument" — with it, we
//! can promote the line to "1st integer argument under Win64".
//!
//! This module owns:
//!
//! * The [`Abi`] enum that lives on `DisasmViewConfig::abi`.
//! * [`RegisterRole`] — what a particular bare register typically
//!   represents under that ABI (argument N, return value, stack
//!   pointer, frame pointer, string-op counter / source / destination,
//!   mul/div hi-half, segment base, etc.).
//! * [`role`] / [`role_description`] lookups that return the
//!   locale-appropriate string for a (register, ABI) pair.
//!
//! Pairs with [`super::operand`] (parses the operand string into a
//! structured form) and [`super::idiom`] (uses ABI to upgrade
//! generic "register-passed argument" hints to specific ones).

use crate::i18n::Locale;

/// Target calling convention. Surfaced via
/// [`crate::disasm_view::DisasmViewConfig::abi`]; the host picks the
/// right value when constructing the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Abi {
    /// Microsoft x64 (Win64). Integer args: RCX, RDX, R8, R9, then
    /// stack with a 32-byte shadow space. Default — most modern
    /// Windows reverse-engineering targets.
    #[default]
    Win64,
    /// SysV AMD64 (Linux x64 / macOS x64). Integer args: RDI, RSI,
    /// RDX, RCX, R8, R9, then stack. No shadow space.
    SysVAmd64,
    /// x86 cdecl — args via stack right-to-left, **caller** cleans
    /// (so `ret` with no immediate, and the caller emits an
    /// `add esp, N` after the call).
    Cdecl,
    /// x86 stdcall — args via stack right-to-left, **callee** cleans
    /// (`ret N`). The Win32 API convention.
    Stdcall,
    /// x86 fastcall — first 2 integer args in ECX, EDX, the rest on
    /// the stack. Microsoft and Borland flavours; we match Microsoft.
    Fastcall,
    /// Unknown / not applicable — no ABI hints will fire.
    Unknown,
}

impl Abi {
    /// Short tag (`"win64"`, `"sysv"`, …) for logs / debug overlays.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Abi::Win64 => "win64",
            Abi::SysVAmd64 => "sysv-amd64",
            Abi::Cdecl => "cdecl",
            Abi::Stdcall => "stdcall",
            Abi::Fastcall => "fastcall",
            Abi::Unknown => "unknown",
        }
    }

    /// `true` iff this ABI is for 64-bit code.
    #[must_use]
    pub fn is_64_bit(self) -> bool {
        matches!(self, Abi::Win64 | Abi::SysVAmd64)
    }
}

/// Symbolic role of a register under the active ABI. Each variant
/// carries the human-readable EN/RU labels resolved by
/// [`role_description`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterRole {
    /// 1-based integer-argument index (`Argument(1)` = first arg).
    Argument(u8),
    /// Return-value register (RAX / EAX).
    ReturnValue,
    /// Hi half of the 128-/64-bit result of `mul` / `imul` / `div` /
    /// `idiv` (RDX / EDX), depending on the operand size.
    HiHalfMulDiv,
    /// Stack pointer (RSP / ESP) — touching it directly without
    /// preserving alignment usually breaks the ABI.
    StackPointer,
    /// Frame pointer (RBP / EBP) — only when the function uses one;
    /// otherwise it's a general-purpose register.
    FramePointer,
    /// Counter register for `loop` / `rep` / `string` ops (RCX / ECX).
    Counter,
    /// Source pointer for `string` ops (RSI / ESI) — `movs`, `lods`,
    /// `cmps`.
    StringSource,
    /// Destination pointer for `string` ops (RDI / EDI) — `movs`,
    /// `stos`, `scas`, `cmps`.
    StringDestination,
    /// FS / GS segment base — Win32 TIB lives at `fs:[…]`,
    /// Win64 TEB/PEB at `gs:[…]`, Linux TLS at `fs:[…]`.
    SegmentBase(&'static str),
    /// Instruction pointer (RIP / EIP). RIP-relative addressing is
    /// the canonical x64 way to reach globals.
    InstructionPointer,
    /// Plain general-purpose register with no ABI-special role.
    GeneralPurpose,
}

/// Look up the role of `register` under `abi`. Case-insensitive,
/// zero-allocation. Returns `RegisterRole::GeneralPurpose` for
/// unknown / non-canonical registers (e.g. SIMD ones we don't
/// catalogue) so callers always get a usable value.
#[must_use]
pub fn role(register: &str, abi: Abi) -> RegisterRole {
    let r = register.trim();
    if r.is_empty() {
        return RegisterRole::GeneralPurpose;
    }
    // Map a register name to its canonical 64-bit family root so the
    // role logic only needs to switch on root names. `RCX` / `ECX`
    // / `CX` / `CL` all belong to the `rcx` root.
    let root = canonical_root(r);

    // RIP / EIP — same role regardless of width.
    if root.eq_ignore_ascii_case("rip") {
        return RegisterRole::InstructionPointer;
    }
    // Stack pointer.
    if root.eq_ignore_ascii_case("rsp") {
        return RegisterRole::StackPointer;
    }
    // Frame pointer (the renderer can't tell from the register name
    // alone whether the function actually establishes a frame; flag
    // it as FramePointer and leave that judgement to the caller).
    if root.eq_ignore_ascii_case("rbp") {
        return RegisterRole::FramePointer;
    }
    // Segment registers we annotate.
    if r.eq_ignore_ascii_case("fs") {
        return RegisterRole::SegmentBase("fs");
    }
    if r.eq_ignore_ascii_case("gs") {
        return RegisterRole::SegmentBase("gs");
    }

    // Per-ABI argument / return / scratch tables.
    match abi {
        Abi::Win64 => match root {
            "rax" => RegisterRole::ReturnValue,
            "rcx" => RegisterRole::Argument(1),
            "rdx" => RegisterRole::Argument(2),
            "r8" => RegisterRole::Argument(3),
            "r9" => RegisterRole::Argument(4),
            "rsi" => RegisterRole::StringSource,
            "rdi" => RegisterRole::StringDestination,
            _ => RegisterRole::GeneralPurpose,
        },
        Abi::SysVAmd64 => match root {
            "rax" => RegisterRole::ReturnValue,
            "rdi" => RegisterRole::Argument(1),
            "rsi" => RegisterRole::Argument(2),
            "rdx" => RegisterRole::Argument(3),
            "rcx" => RegisterRole::Argument(4),
            "r8" => RegisterRole::Argument(5),
            "r9" => RegisterRole::Argument(6),
            _ => RegisterRole::GeneralPurpose,
        },
        Abi::Fastcall => match root {
            "rax" => RegisterRole::ReturnValue,
            "rcx" => RegisterRole::Argument(1),
            "rdx" => RegisterRole::Argument(2),
            "rsi" => RegisterRole::StringSource,
            "rdi" => RegisterRole::StringDestination,
            _ => RegisterRole::GeneralPurpose,
        },
        Abi::Cdecl | Abi::Stdcall => match root {
            // Both x86 cdecl and stdcall pass everything via the stack;
            // EAX is the return register, EDX is hi half on division /
            // 64-bit return, ECX/ESI/EDI keep their string-op roles.
            "rax" => RegisterRole::ReturnValue,
            "rcx" => RegisterRole::Counter,
            "rdx" => RegisterRole::HiHalfMulDiv,
            "rsi" => RegisterRole::StringSource,
            "rdi" => RegisterRole::StringDestination,
            _ => RegisterRole::GeneralPurpose,
        },
        Abi::Unknown => RegisterRole::GeneralPurpose,
    }
}

/// Locale-aware single-line description of `role`. `register` is the
/// raw text of the operand so the description can say "RCX" or
/// "ECX" without re-deriving it. Returns `None` when the role is
/// `GeneralPurpose` (no useful annotation).
#[must_use]
pub fn role_description(role: RegisterRole, register: &str, locale: Locale) -> Option<String> {
    Some(match role {
        RegisterRole::Argument(n) => match locale {
            Locale::En => format!(
                "{register} = integer argument #{n} (caller passes it here under the active ABI)"
            ),
            Locale::Ru => format!(
                "{register} = целочисленный аргумент №{n} (через него передаёт вызывающая сторона по текущему ABI)"
            ),
        },
        RegisterRole::ReturnValue => match locale {
            Locale::En => format!("{register} = function return value"),
            Locale::Ru => format!("{register} = возвращаемое значение функции"),
        },
        RegisterRole::HiHalfMulDiv => match locale {
            Locale::En => format!(
                "{register} = high half of mul/div / 64-bit return pair (EDX:EAX, RDX:RAX)"
            ),
            Locale::Ru => format!(
                "{register} = старшая половина mul/div / 64-битного возврата (EDX:EAX, RDX:RAX)"
            ),
        },
        RegisterRole::StackPointer => match locale {
            Locale::En => format!("{register} = stack pointer (touch only via push/pop/sub/add)"),
            Locale::Ru => format!("{register} = указатель стека (трогать только push/pop/sub/add)"),
        },
        RegisterRole::FramePointer => match locale {
            Locale::En => format!(
                "{register} = frame pointer in this function (locals via [{register}-N], args via [{register}+N])"
            ),
            Locale::Ru => format!(
                "{register} = frame-указатель этой функции (локалки через [{register}-N], аргументы через [{register}+N])"
            ),
        },
        RegisterRole::Counter => match locale {
            Locale::En => format!("{register} = loop / string-op counter"),
            Locale::Ru => format!("{register} = счётчик loop / string-операций"),
        },
        RegisterRole::StringSource => match locale {
            Locale::En => format!("{register} = source pointer for string ops (movs/lods/cmps)"),
            Locale::Ru => format!("{register} = src-указатель для string-операций (movs/lods/cmps)"),
        },
        RegisterRole::StringDestination => match locale {
            Locale::En => format!(
                "{register} = destination pointer for string ops (movs/stos/scas/cmps)"
            ),
            Locale::Ru => format!(
                "{register} = dst-указатель для string-операций (movs/stos/scas/cmps)"
            ),
        },
        RegisterRole::SegmentBase("fs") => match locale {
            Locale::En => format!("{register} = FS segment base (Win32 TIB; Linux TLS in some setups)"),
            Locale::Ru => format!(
                "{register} = база FS-сегмента (TIB в Win32; TLS в Linux в некоторых конфигурациях)"
            ),
        },
        RegisterRole::SegmentBase("gs") => match locale {
            Locale::En => format!(
                "{register} = GS segment base (Win64 TEB → PEB at gs:[0x60]; macOS TLS)"
            ),
            Locale::Ru => format!(
                "{register} = база GS-сегмента (TEB → PEB по gs:[0x60] в Win64; TLS в macOS)"
            ),
        },
        RegisterRole::SegmentBase(_) => return None,
        RegisterRole::InstructionPointer => match locale {
            Locale::En => format!(
                "{register} = instruction pointer (used as base for x64 RIP-relative addressing)"
            ),
            Locale::Ru => format!(
                "{register} = указатель инструкций (база для RIP-relative адресации в x64)"
            ),
        },
        RegisterRole::GeneralPurpose => return None,
    })
}

/// Resolve a register name (any width) to its canonical 64-bit family
/// root, lower-cased. `eax` / `ax` / `al` / `ah` → `"rax"`,
/// `r8d` / `r8w` / `r8b` → `"r8"`, etc.
fn canonical_root(reg: &str) -> &'static str {
    let lower = reg.to_ascii_lowercase();
    match lower.as_str() {
        // RAX family
        "rax" | "eax" | "ax" | "al" | "ah" => "rax",
        // RBX family
        "rbx" | "ebx" | "bx" | "bl" | "bh" => "rbx",
        // RCX family
        "rcx" | "ecx" | "cx" | "cl" | "ch" => "rcx",
        // RDX family
        "rdx" | "edx" | "dx" | "dl" | "dh" => "rdx",
        // RSI / RDI / RBP / RSP
        "rsi" | "esi" | "si" | "sil" => "rsi",
        "rdi" | "edi" | "di" | "dil" => "rdi",
        "rbp" | "ebp" | "bp" | "bpl" => "rbp",
        "rsp" | "esp" | "sp" | "spl" => "rsp",
        // R8..R15 (x64 only)
        "r8" | "r8d" | "r8w" | "r8b" => "r8",
        "r9" | "r9d" | "r9w" | "r9b" => "r9",
        "r10" | "r10d" | "r10w" | "r10b" => "r10",
        "r11" | "r11d" | "r11w" | "r11b" => "r11",
        "r12" | "r12d" | "r12w" | "r12b" => "r12",
        "r13" | "r13d" | "r13w" | "r13b" => "r13",
        "r14" | "r14d" | "r14w" | "r14b" => "r14",
        "r15" | "r15d" | "r15w" | "r15b" => "r15",
        // Instruction pointer
        "rip" | "eip" => "rip",
        // Anything else (segment regs, simd, etc.) — return as-is
        // (the caller compares case-insensitively).
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_default_is_win64() {
        assert_eq!(Abi::default(), Abi::Win64);
    }

    #[test]
    fn rcx_is_first_arg_under_win64() {
        assert!(matches!(role("RCX", Abi::Win64), RegisterRole::Argument(1)));
        assert!(matches!(role("rcx", Abi::Win64), RegisterRole::Argument(1)));
        assert!(matches!(role("ecx", Abi::Win64), RegisterRole::Argument(1)));
        assert!(matches!(role("cx",  Abi::Win64), RegisterRole::Argument(1)));
        assert!(matches!(role("cl",  Abi::Win64), RegisterRole::Argument(1)));
    }

    #[test]
    fn rcx_is_fourth_arg_under_sysv() {
        assert!(matches!(role("rcx", Abi::SysVAmd64), RegisterRole::Argument(4)));
    }

    #[test]
    fn rdi_is_first_under_sysv_but_string_dest_under_win64() {
        assert!(matches!(role("rdi", Abi::SysVAmd64), RegisterRole::Argument(1)));
        assert!(matches!(role("rdi", Abi::Win64),     RegisterRole::StringDestination));
    }

    #[test]
    fn rax_is_return_value_in_every_abi() {
        for abi in [Abi::Win64, Abi::SysVAmd64, Abi::Cdecl, Abi::Stdcall, Abi::Fastcall] {
            assert!(matches!(role("rax", abi), RegisterRole::ReturnValue), "{abi:?}");
        }
    }

    #[test]
    fn rsp_and_rbp_are_special() {
        assert!(matches!(role("rsp", Abi::Win64), RegisterRole::StackPointer));
        assert!(matches!(role("esp", Abi::Cdecl), RegisterRole::StackPointer));
        assert!(matches!(role("rbp", Abi::SysVAmd64), RegisterRole::FramePointer));
    }

    #[test]
    fn segment_registers_recognised() {
        assert!(matches!(role("fs", Abi::Win64), RegisterRole::SegmentBase("fs")));
        assert!(matches!(role("gs", Abi::Win64), RegisterRole::SegmentBase("gs")));
    }

    #[test]
    fn unknown_register_falls_back_to_general_purpose() {
        assert!(matches!(role("xmm0", Abi::Win64), RegisterRole::GeneralPurpose));
        assert!(matches!(role("",     Abi::Win64), RegisterRole::GeneralPurpose));
    }

    #[test]
    fn role_description_localises_argument() {
        let en = role_description(RegisterRole::Argument(1), "RCX", Locale::En).unwrap();
        let ru = role_description(RegisterRole::Argument(1), "RCX", Locale::Ru).unwrap();
        assert!(en.contains("argument") && en.contains("RCX") && en.contains('1'));
        assert!(ru.contains("аргумент") && ru.contains("RCX") && ru.contains('1'));
    }

    #[test]
    fn role_description_omits_general_purpose() {
        assert!(role_description(RegisterRole::GeneralPurpose, "RBX", Locale::En).is_none());
    }

    #[test]
    fn unknown_abi_yields_no_argument_hint() {
        // Under `Abi::Unknown` even canonical arg registers should
        // come back as plain general-purpose so the tooltip doesn't
        // lie.
        assert!(matches!(role("rcx", Abi::Unknown), RegisterRole::GeneralPurpose));
        assert!(matches!(role("rdi", Abi::Unknown), RegisterRole::GeneralPurpose));
    }

    #[test]
    fn abi_is_64_bit_predicate() {
        assert!(Abi::Win64.is_64_bit());
        assert!(Abi::SysVAmd64.is_64_bit());
        assert!(!Abi::Cdecl.is_64_bit());
        assert!(!Abi::Stdcall.is_64_bit());
        assert!(!Abi::Fastcall.is_64_bit());
        assert!(!Abi::Unknown.is_64_bit());
    }

    #[test]
    fn abi_serde_round_trip() {
        for abi in [Abi::Win64, Abi::SysVAmd64, Abi::Cdecl, Abi::Stdcall, Abi::Fastcall, Abi::Unknown] {
            let s = ron::ser::to_string(&abi).unwrap();
            let back: Abi = ron::from_str(&s).unwrap();
            assert_eq!(abi, back);
        }
    }
}
