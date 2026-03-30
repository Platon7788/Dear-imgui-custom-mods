//! x86/x86-64 assembly tokenizer (AT&T + Intel/NASM/MASM unified).
//!
//! Covers both AT&T (`%rax`, `$42`, `#` comments) and Intel (`rax`, `;` comments)
//! syntax simultaneously. Registers → [`TokenKind::TypeName`],
//! mnemonics → [`TokenKind::Keyword`], directives → [`TokenKind::Attribute`],
//! labels → [`TokenKind::MacroCall`].

use super::{is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

// ── x86-64 registers (lowercase canonical forms) ────────────────────────────

const REGISTERS: &[&str] = &[
    // 64-bit general purpose
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rsp", "rbp",
    "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
    // 32-bit
    "eax", "ebx", "ecx", "edx", "esi", "edi", "esp", "ebp",
    "r8d", "r9d", "r10d", "r11d", "r12d", "r13d", "r14d", "r15d",
    // 16-bit
    "ax", "bx", "cx", "dx", "si", "di", "sp", "bp",
    "r8w", "r9w", "r10w", "r11w", "r12w", "r13w", "r14w", "r15w",
    // 8-bit
    "al", "bl", "cl", "dl", "sil", "dil", "spl", "bpl",
    "ah", "bh", "ch", "dh",
    "r8b", "r9b", "r10b", "r11b", "r12b", "r13b", "r14b", "r15b",
    // Segment
    "cs", "ds", "es", "fs", "gs", "ss",
    // Instruction pointer / flags
    "rip", "eip", "ip", "rflags", "eflags", "flags",
    // Control / debug
    "cr0", "cr2", "cr3", "cr4", "cr8",
    "dr0", "dr1", "dr2", "dr3", "dr6", "dr7",
    // SSE/AVX
    "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7",
    "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
    "ymm0", "ymm1", "ymm2", "ymm3", "ymm4", "ymm5", "ymm6", "ymm7",
    "ymm8", "ymm9", "ymm10", "ymm11", "ymm12", "ymm13", "ymm14", "ymm15",
    "zmm0", "zmm1", "zmm2", "zmm3", "zmm4", "zmm5", "zmm6", "zmm7",
    "zmm8", "zmm9", "zmm10", "zmm11", "zmm12", "zmm13", "zmm14", "zmm15",
    // x87 FPU
    "st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7",
    // MMX
    "mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6", "mm7",
];

// ── Common x86-64 mnemonics ─────────────────────────────────────────────────

const MNEMONICS: &[&str] = &[
    // Data movement
    "mov", "movabs", "movzx", "movsx", "movsxd", "movq", "movd",
    "movss", "movsd", "movaps", "movups", "movdqa", "movdqu",
    "lea", "xchg", "push", "pop", "pushf", "popf",
    "cmov", "cmove", "cmovne", "cmovz", "cmovnz", "cmovg", "cmovge",
    "cmovl", "cmovle", "cmova", "cmovae", "cmovb", "cmovbe",
    "cmovs", "cmovns",
    // Arithmetic
    "add", "sub", "mul", "imul", "div", "idiv", "neg", "inc", "dec",
    "adc", "sbb", "cmp",
    "addss", "addsd", "subss", "subsd", "mulss", "mulsd", "divss", "divsd",
    // Bitwise
    "and", "or", "xor", "not", "shl", "shr", "sar", "sal", "rol", "ror",
    "rcl", "rcr", "bt", "bts", "btr", "btc", "bsf", "bsr",
    "test", "popcnt", "lzcnt", "tzcnt",
    // Control flow
    "jmp", "je", "jne", "jz", "jnz", "jg", "jge", "jl", "jle",
    "ja", "jae", "jb", "jbe", "js", "jns", "jo", "jno", "jp", "jnp",
    "jcxz", "jecxz", "jrcxz",
    "call", "ret", "leave", "enter",
    "loop", "loope", "loopne",
    "int", "int3", "syscall", "sysenter", "iret", "iretq",
    // Comparison & set
    "sete", "setne", "setg", "setge", "setl", "setle",
    "seta", "setae", "setb", "setbe", "sets", "setns",
    // Stack frame
    "nop", "hlt", "ud2", "cpuid", "rdtsc", "rdtscp",
    // String ops
    "rep", "repe", "repne", "repz", "repnz",
    "movsb", "movsw", "movsd", "movsq",
    "stosb", "stosw", "stosd", "stosq",
    "lodsb", "lodsw", "lodsd", "lodsq",
    "cmpsb", "cmpsw", "cmpsd", "cmpsq",
    "scasb", "scasw", "scasd", "scasq",
    // Conversion
    "cbw", "cwde", "cdqe", "cwd", "cdq", "cqo",
    "cvtsi2ss", "cvtsi2sd", "cvtss2sd", "cvtsd2ss",
    "cvtss2si", "cvtsd2si", "cvttss2si", "cvttsd2si",
    // SSE/AVX
    "pxor", "por", "pand", "pandn",
    "paddb", "paddw", "paddd", "paddq",
    "psubb", "psubw", "psubd", "psubq",
    "pmulld", "pmullw",
    "pcmpeqb", "pcmpeqw", "pcmpeqd",
    "pshufd", "pshufb", "punpcklbw", "punpckhbw",
    "sqrtss", "sqrtsd", "sqrtps", "sqrtpd",
    "minss", "maxss", "minsd", "maxsd",
    "comiss", "comisd", "ucomiss", "ucomisd",
    "shufps", "shufpd", "unpcklps", "unpckhps",
    // AVX prefixed
    "vaddss", "vaddsd", "vsubss", "vsubsd", "vmulss", "vmulsd",
    "vmovss", "vmovsd", "vmovaps", "vmovups", "vmovdqa", "vmovdqu",
    "vxorps", "vandps", "vorps",
    // Misc
    "cld", "std", "clc", "stc", "cmc",
    "lfence", "sfence", "mfence", "pause",
    "lock", "xacquire", "xrelease",
    "prefetch", "prefetcht0", "prefetcht1", "prefetcht2", "prefetchnta",
    // Size specifiers (NASM/Intel)
    "byte", "word", "dword", "qword", "tword", "oword", "yword", "zword",
    "ptr", "near", "far", "short",
];

// ── Assembler directives ────────────────────────────────────────────────────

const DIRECTIVES: &[&str] = &[
    // GAS / AT&T
    ".text", ".data", ".bss", ".rodata", ".section",
    ".global", ".globl", ".local", ".weak", ".hidden", ".protected",
    ".type", ".size", ".align", ".balign", ".p2align",
    ".byte", ".word", ".long", ".quad", ".octa",
    ".ascii", ".asciz", ".string", ".zero", ".fill", ".space",
    ".equ", ".set", ".equiv", ".comm", ".lcomm",
    ".macro", ".endm", ".if", ".else", ".endif", ".ifdef", ".ifndef",
    ".include", ".incbin", ".file", ".loc", ".cfi_startproc", ".cfi_endproc",
    ".cfi_def_cfa_offset", ".cfi_offset", ".cfi_def_cfa_register",
    // NASM / MASM
    "section", "segment", "global", "extern", "default",
    "bits", "org", "times",
    "db", "dw", "dd", "dq", "dt", "do", "dy", "dz",
    "resb", "resw", "resd", "resq", "rest", "reso", "resy", "resz",
    "equ", "incbin", "struc", "endstruc", "istruc", "at", "iend",
    "%define", "%undef", "%macro", "%endmacro", "%if", "%elif", "%else",
    "%endif", "%ifdef", "%ifndef", "%include", "%assign", "%rep", "%endrep",
    // MASM specific
    "PROC", "ENDP", "SEGMENT", "ENDS", "ASSUME", "END",
    "MACRO", "ENDM", "LOCAL", "INVOKE",
    "STRUCT", "UNION", "TYPEDEF",
    "IF", "ELSE", "ENDIF", "IFDEF", "IFNDEF",
    "INCLUDE", "INCLUDELIB", "EXTRN", "PUBLIC",
    ".MODEL", ".STACK", ".CODE", ".DATA",
    ".386", ".486", ".586", ".686", ".MMX", ".XMM",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct AsmLang;

impl SyntaxDefinition for AsmLang {
    fn name(&self) -> &str { "Assembly" }

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        (tokenize(line), false)
    }

    fn line_comment_prefix(&self) -> Option<&str> { Some(";") }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> { None }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('(', ')')]
    }

    fn auto_indent_after(&self) -> &[char] { &[':'] }
    fn auto_dedent_on(&self) -> &[char] { &[] }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("(", ")"), ("\"", "\""), ("'", "'")]
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Check if an identifier (lowercase) matches a register name.
fn is_register(word: &str) -> bool {
    // Registers are always lowercase in our list
    REGISTERS.contains(&word)
}

/// Check if a word matches a mnemonic (case-insensitive for Intel compat).
fn is_mnemonic(word: &str) -> bool {
    MNEMONICS.contains(&word)
}

/// Check if a word matches a directive.
fn is_directive(word: &str) -> bool {
    DIRECTIVES.contains(&word)
}

/// Extended ident: allows `.` at start for GAS directives, `%` for NASM macros.
fn is_asm_ident_continue(b: u8) -> bool {
    is_ident_continue(b) || b == b'.'
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    // Note: future label-context-aware parsing could track line_start here.

    while i < len {
        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
            tokens.push(Token { kind: TokenKind::Whitespace, start, len: i - start });
            continue;
        }

        // ── Comments: ; (Intel) or # (AT&T) or // (GAS alternate) ────────
        if b == b';' || b == b'#'
            || (b == b'/' && i + 1 < len && bytes[i + 1] == b'/')
        {
            tokens.push(Token { kind: TokenKind::Comment, start: i, len: len - i });
            return tokens;
        }

        // ── C-style block comment /* */ (used by some assemblers) ─────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        // ── String literal ───────────────────────────────────────────────
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token { kind: TokenKind::String, start, len: i - start });
            continue;
        }

        // ── %word: NASM directive (%define) or AT&T register (%rax) ─────
        if b == b'%' && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) { i += 1; }
            let word = &line[start..i];
            let kind = if is_directive(word) {
                TokenKind::Attribute   // NASM preprocessor: %define, %macro, …
            } else {
                TokenKind::TypeName    // AT&T register: %rax, %eax, …
            };
            tokens.push(Token { kind, start, len: i - start });
            continue;
        }

        // ── AT&T immediate ($42, $0xFF) ──────────────────────────────────
        if b == b'$' {
            let start = i;
            i += 1;
            if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'-') {
                if i < len && bytes[i] == b'-' { i += 1; }
                consume_number(&mut i, bytes);
                tokens.push(Token { kind: TokenKind::Number, start, len: i - start });
            } else if i < len && is_ident_start(bytes[i]) {
                while i < len && is_ident_continue(bytes[i]) { i += 1; }
                tokens.push(Token { kind: TokenKind::Identifier, start, len: i - start });
            } else {
                tokens.push(Token { kind: TokenKind::Operator, start, len: 1 });
            }
            continue;
        }

        // ── Number: decimal, hex (0x / 0Fh), binary (0b), octal (0o) ────
        if b.is_ascii_digit()
            || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            if b == b'-' { i += 1; }
            consume_number(&mut i, bytes);
            // NASM-style hex suffix: 0FFh
            if i < len && (bytes[i] == b'h' || bytes[i] == b'H') { i += 1; }
            tokens.push(Token { kind: TokenKind::Number, start, len: i - start });

            continue;
        }

        // ── GAS directive (.text, .globl, .cfi_startproc) ────────────────
        if b == b'.' && i + 1 < len && (is_ident_start(bytes[i + 1]) || bytes[i + 1] == b'.') {
            let start = i;
            i += 1;
            while i < len && is_asm_ident_continue(bytes[i]) { i += 1; }
            tokens.push(Token { kind: TokenKind::Attribute, start, len: i - start });

            continue;
        }

        // ── Identifier / mnemonic / register / label / directive ─────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) { i += 1; }

            // Label: identifier followed by `:`
            if i < len && bytes[i] == b':' {
                i += 1; // include the colon
                tokens.push(Token { kind: TokenKind::MacroCall, start, len: i - start });

                continue;
            }

            let word = &line[start..i];
            let word_lower: String;

            // Case-insensitive matching for registers and mnemonics
            let word_lc = if word.chars().any(|c| c.is_uppercase()) {
                word_lower = word.to_ascii_lowercase();
                word_lower.as_str()
            } else {
                word
            };

            let kind = if is_register(word_lc) {
                TokenKind::TypeName
            } else if is_mnemonic(word_lc) {
                TokenKind::Keyword
            } else if is_directive(word) || is_directive(word_lc) {
                TokenKind::Attribute
            } else {
                TokenKind::Identifier
            };

            tokens.push(Token { kind, start, len: i - start });

            continue;
        }

        // ── Operators ────────────────────────────────────────────────────
        if matches!(b, b'+' | b'-' | b'*' | b'/' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' | b'~') {
            tokens.push(Token { kind: TokenKind::Operator, start: i, len: 1 });
            i += 1;

            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}' |
                        b':' | b',' | b'.' | b'@') {
            tokens.push(Token { kind: TokenKind::Punctuation, start: i, len: 1 });
            i += 1;

            continue;
        }

        // ── Fallback ─────────────────────────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token { kind: TokenKind::Identifier, start: i, len: ch_len });
        i += ch_len;
    }

    tokens
}

/// Consume a number: decimal, hex (0x), binary (0b), octal (0o/0).
fn consume_number(i: &mut usize, bytes: &[u8]) {
    let len = bytes.len();
    if *i >= len { return; }

    if bytes[*i] == b'0' && *i + 1 < len {
        match bytes[*i + 1] {
            b'x' | b'X' => {
                *i += 2;
                while *i < len && (bytes[*i].is_ascii_hexdigit() || bytes[*i] == b'_') { *i += 1; }
                return;
            }
            b'b' | b'B' => {
                *i += 2;
                while *i < len && (bytes[*i] == b'0' || bytes[*i] == b'1' || bytes[*i] == b'_') { *i += 1; }
                return;
            }
            b'o' | b'O' => {
                *i += 2;
                while *i < len && ((bytes[*i] >= b'0' && bytes[*i] <= b'7') || bytes[*i] == b'_') { *i += 1; }
                return;
            }
            _ => {}
        }
    }

    // Decimal or NASM-style hex (0FFh — starts with digit, ends with h)
    while *i < len && (bytes[*i].is_ascii_hexdigit() || bytes[*i] == b'_') { *i += 1; }

    // Decimal point (for floating-point literals in some assemblers)
    if *i < len && bytes[*i] == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit() {
        *i += 1;
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') { *i += 1; }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Asm, false);
        tokens.iter().map(|t| (t.kind, line[t.start..t.start + t.len].to_string())).collect()
    }

    #[test]
    fn intel_basic() {
        let toks = tok("    mov eax, [rbx+8]");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "mov"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "eax"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "rbx"));
    }

    #[test]
    fn att_basic() {
        let toks = tok("    movq %rax, %rbx");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "movq"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "%rax"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "%rbx"));
    }

    #[test]
    fn att_immediate() {
        let toks = tok("    addq $42, %rax");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "$42"));
    }

    #[test]
    fn label() {
        let toks = tok("main:");
        assert_eq!(toks[0].0, TokenKind::MacroCall);
        assert_eq!(toks[0].1, "main:");
    }

    #[test]
    fn label_with_instruction() {
        let toks = tok("loop_start: dec ecx");
        assert!(toks.iter().any(|t| t.0 == TokenKind::MacroCall && t.1 == "loop_start:"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "dec"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "ecx"));
    }

    #[test]
    fn hex_number() {
        let toks = tok("    mov eax, 0xFF");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "0xFF"));
    }

    #[test]
    fn nasm_hex_suffix() {
        let toks = tok("    mov eax, 0FFh");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "0FFh"));
    }

    #[test]
    fn semicolon_comment() {
        let toks = tok("    ret ; return to caller");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "ret"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::Comment));
    }

    #[test]
    fn hash_comment() {
        let toks = tok("# this is AT&T style comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    #[test]
    fn gas_directive() {
        let toks = tok("    .globl main");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Attribute && t.1 == ".globl"));
    }

    #[test]
    fn nasm_directive() {
        let toks = tok("section .text");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Attribute && t.1 == "section"));
    }

    #[test]
    fn string_literal() {
        let toks = tok(r#"    .asciz "Hello, World!\n""#);
        assert!(toks.iter().any(|t| t.0 == TokenKind::String));
    }

    #[test]
    fn sse_registers() {
        let toks = tok("    movaps xmm0, xmm1");
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "xmm0"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "xmm1"));
    }

    #[test]
    fn case_insensitive_mnemonics() {
        let toks = tok("    MOV EAX, EBX");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "MOV"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "EAX"));
    }

    #[test]
    fn nasm_preprocessor() {
        let toks = tok("%define BUFFER_SIZE 1024");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Attribute && t.1 == "%define"));
    }

    #[test]
    fn binary_number() {
        let toks = tok("    mov al, 0b11001010");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "0b11001010"));
    }
}
