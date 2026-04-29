//! Operand tokenizer and token classification used by the disassembly
//! syntax highlighter.
//!
//! Splits an operand string like `"qword ptr [rsp + 0x10], rax"` into a
//! stream of [`OperandToken`]s with [`TokenKind`]s — `Register`, `Number`,
//! `Memory`, `String`, `Plain` — that the renderer maps to colors via
//! [`super::config::DisasmColors`].

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenKind {
    Register,
    Number,
    Memory,
    String,
    Plain,
}

pub(super) struct OperandToken<'a> {
    pub(super) text: &'a str,
    pub(super) kind: TokenKind,
}

/// Simple operand tokenizer for syntax coloring.
pub(super) struct OperandTokenizer<'a> {
    remaining: &'a str,
}

impl<'a> OperandTokenizer<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Self { remaining: text }
    }
}

impl<'a> Iterator for OperandTokenizer<'a> {
    type Item = OperandToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        // Consume leading whitespace/punctuation as plain tokens.
        let first = self.remaining.as_bytes()[0];
        if matches!(first, b' ' | b',' | b'+' | b'-' | b'*' | b':') {
            let end = self
                .remaining
                .bytes()
                .position(|b| !matches!(b, b' ' | b',' | b'+' | b'-' | b'*' | b':'))
                .unwrap_or(self.remaining.len());
            let (tok, rest) = self.remaining.split_at(end);
            self.remaining = rest;
            return Some(OperandToken {
                text: tok,
                kind: TokenKind::Plain,
            });
        }

        // Memory brackets.
        if first == b'[' || first == b']' {
            let (tok, rest) = self.remaining.split_at(1);
            self.remaining = rest;
            return Some(OperandToken {
                text: tok,
                kind: TokenKind::Memory,
            });
        }

        // String literal.
        if first == b'"' {
            let end = self.remaining[1..]
                .find('"')
                .map(|p| p + 2)
                .unwrap_or(self.remaining.len());
            let (tok, rest) = self.remaining.split_at(end);
            self.remaining = rest;
            return Some(OperandToken {
                text: tok,
                kind: TokenKind::String,
            });
        }

        // Find end of word.
        let end = self
            .remaining
            .bytes()
            .position(|b| matches!(b, b' ' | b',' | b'+' | b'-' | b'*' | b':' | b'[' | b']'))
            .unwrap_or(self.remaining.len());
        let (word, rest) = self.remaining.split_at(end);
        self.remaining = rest;

        let kind = classify_operand_token(word);
        Some(OperandToken { text: word, kind })
    }
}

/// Classify an operand token as register, number, memory keyword, or plain.
pub(super) fn classify_operand_token(token: &str) -> TokenKind {
    if token.is_empty() {
        return TokenKind::Plain;
    }

    let lower = token.to_ascii_lowercase();

    // Size keywords → memory context (check before registers).
    // Covers MASM/Intel/iced-x86 default formatter outputs:
    //   `byte`, `word`, `dword`, `fword`, `qword`, `tbyte`, `oword`,
    //   `xmmword`, `ymmword`, `zmmword`, `ptr`.
    if matches!(
        lower.as_str(),
        "byte"
            | "word"
            | "dword"
            | "fword"
            | "qword"
            | "tbyte"
            | "oword"
            | "xmmword"
            | "ymmword"
            | "zmmword"
            | "ptr"
    ) {
        return TokenKind::Memory;
    }

    if is_x86_register(&lower) {
        return TokenKind::Register;
    }

    // Number — accept `0x...` / `0X...` (must have ≥1 hex digit after
    // the prefix), `...h` / `...H` (MASM/iced-x86 default formatter,
    // must have ≥1 leading hex digit), and plain decimal. Empty hex
    // bodies were previously accepted ("h" / "0x" alone) — pinned as a
    // bug now: those tokens fall through to `Plain`.
    if let Some(rest) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_hexdigit())
    {
        return TokenKind::Number;
    }
    if token.len() > 1
        && (token.ends_with('h') || token.ends_with('H'))
        && token[..token.len() - 1]
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return TokenKind::Number;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return TokenKind::Number;
    }

    TokenKind::Plain
}

/// Whether `lower` (already lowercased) is a recognised x86/x86-64
/// register name.
///
/// Recognises the legacy core sets (rax/rbx/.../r15 + 32/16/8-bit
/// flavours, segment, FLAGS, RIP) and the parameterised families that
/// would explode the static array — `r8..=r15` with `b`/`w`/`d`
/// suffixes, `xmm0..=xmm31`, `ymm0..=ymm31`, `zmm0..=zmm31`,
/// `k0..=k7` (AVX-512 mask), `st0..=st7` (x87), `mm0..=mm7` (MMX),
/// `cr0..=cr15` (control), `dr0..=dr15` (debug), `tr0..=tr7` (test).
///
/// The expanded coverage maps directly onto iced-x86's
/// `IntelFormatter` / `MasmFormatter` register output, so a
/// disassembly view backed by iced-x86 highlights AVX-512 and
/// kernel-mode operands without extra adapter code.
fn is_x86_register(lower: &str) -> bool {
    match lower {
        // 64-bit GP.
        "rax" | "rbx" | "rcx" | "rdx" | "rsi" | "rdi" | "rbp" | "rsp" => return true,
        // 32-bit GP.
        "eax" | "ebx" | "ecx" | "edx" | "esi" | "edi" | "ebp" | "esp" => return true,
        // 16-bit GP.
        "ax" | "bx" | "cx" | "dx" | "si" | "di" | "bp" | "sp" => return true,
        // 8-bit GP (incl. REX-only `sil`/`dil`/`bpl`/`spl`).
        "al" | "bl" | "cl" | "dl" | "ah" | "bh" | "ch" | "dh" | "sil" | "dil" | "bpl" | "spl" => {
            return true;
        }
        // Segment.
        "cs" | "ds" | "es" | "fs" | "gs" | "ss" => return true,
        // Instruction pointer + FLAGS.
        "rip" | "eip" | "ip" | "rflags" | "eflags" | "flags" => return true,
        _ => {}
    }

    // r8..r15 with optional b/w/d suffix (e.g. `r10`, `r10w`, `r12d`).
    if let Some(rest) = lower.strip_prefix('r') {
        let body = rest
            .strip_suffix(|c: char| matches!(c, 'b' | 'w' | 'd'))
            .unwrap_or(rest);
        if let Ok(n) = body.parse::<u32>()
            && (8..=15).contains(&n)
        {
            return true;
        }
    }

    // SIMD families: xmm0..31 (legacy 0..15 + AVX-512 16..31),
    // ymm0..31, zmm0..31.
    for prefix in ["xmm", "ymm", "zmm"] {
        if let Some(idx) = lower.strip_prefix(prefix)
            && let Ok(n) = idx.parse::<u32>()
            && n <= 31
        {
            return true;
        }
    }

    // AVX-512 mask k0..k7.
    if let Some(idx) = lower.strip_prefix('k')
        && let Ok(n) = idx.parse::<u32>()
        && n <= 7
    {
        return true;
    }

    // x87 st0..st7.
    if let Some(idx) = lower.strip_prefix("st")
        && let Ok(n) = idx.parse::<u32>()
        && n <= 7
    {
        return true;
    }

    // MMX mm0..mm7.
    if let Some(idx) = lower.strip_prefix("mm")
        && let Ok(n) = idx.parse::<u32>()
        && n <= 7
    {
        return true;
    }

    // System / debug / test families. Range upper bounds chosen to
    // match the architecturally-defined slots — anything past those
    // is iced-x86 extension territory and would round-trip to Plain
    // (acceptable: those operands stay coloured as default text).
    for (prefix, max) in [("cr", 15u32), ("dr", 15u32), ("tr", 7u32)] {
        if let Some(idx) = lower.strip_prefix(prefix)
            && let Ok(n) = idx.parse::<u32>()
            && n <= max
        {
            return true;
        }
    }

    false
}
