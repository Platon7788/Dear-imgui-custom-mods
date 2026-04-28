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

    // x86 register names.
    static REGS: &[&str] = &[
        // 64-bit
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15", // 32-bit
        "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "r8d", "r9d", "r10d", "r11d",
        "r12d", "r13d", "r14d", "r15d", // 16-bit
        "ax", "bx", "cx", "dx", "si", "di", "bp", "sp", // 8-bit
        "al", "bl", "cl", "dl", "ah", "bh", "ch", "dh", "sil", "dil", "bpl", "spl", "r8b", "r9b",
        "r10b", "r11b", "r12b", "r13b", "r14b", "r15b", // Segment
        "cs", "ds", "es", "fs", "gs", "ss", // Special
        "rip", "eip", "rflags", "eflags", // SSE/AVX
        "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "xmm8", "xmm9", "xmm10",
        "xmm11", "xmm12", "xmm13", "xmm14", "xmm15", "ymm0", "ymm1", "ymm2", "ymm3", "ymm4",
        "ymm5", "ymm6", "ymm7", "ymm8", "ymm9", "ymm10", "ymm11", "ymm12", "ymm13", "ymm14",
        "ymm15", // x87
        "st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7",
    ];

    let lower = token.to_ascii_lowercase();

    // Size keywords → memory context (check before registers).
    if matches!(
        lower.as_str(),
        "byte" | "word" | "dword" | "qword" | "ptr" | "xmmword" | "ymmword"
    ) {
        return TokenKind::Memory;
    }

    if REGS.contains(&lower.as_str()) {
        return TokenKind::Register;
    }

    // Number: 0x..., decimal, or hex with 'h' suffix.
    if token.starts_with("0x") || token.starts_with("0X") {
        return TokenKind::Number;
    }
    if (token.ends_with('h') || token.ends_with('H'))
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
