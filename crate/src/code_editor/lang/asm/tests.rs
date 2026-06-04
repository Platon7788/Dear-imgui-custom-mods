//! Unit tests for the assembly tokenizer. Split out of `tokenize.rs` to
//! keep every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::Language;
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, String)> {
    let (tokens, _) = tokenize_line(line, &Language::Asm, false);
    tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect()
}

#[test]
fn intel_basic() {
    let toks = tok("    mov eax, [rbx+8]");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "mov")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "eax")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "rbx")
    );
}

#[test]
fn att_basic() {
    let toks = tok("    movq %rax, %rbx");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "movq")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "%rax")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "%rbx")
    );
}

#[test]
fn att_immediate() {
    let toks = tok("    addq $42, %rax");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Number && t.1 == "$42")
    );
}

#[test]
fn att_immediate_negative() {
    let toks = tok("    mov $-1, %rax");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Number && t.1 == "$-1")
    );
}

#[test]
fn label() {
    let toks = tok("main:");
    assert_eq!(toks[0].0, TokenKind::MacroCall);
    assert_eq!(toks[0].1, "main:");
}

#[test]
fn gas_local_label() {
    let toks = tok(".Lloop:");
    assert_eq!(toks[0].0, TokenKind::MacroCall);
    assert_eq!(toks[0].1, ".Lloop:");
}

#[test]
fn label_with_instruction() {
    let toks = tok("loop_start: dec ecx");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::MacroCall && t.1 == "loop_start:")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "dec")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "ecx")
    );
}

#[test]
fn hex_number() {
    let toks = tok("    mov eax, 0xFF");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Number && t.1 == "0xFF")
    );
}

#[test]
fn nasm_hex_suffix() {
    let toks = tok("    mov eax, 0FFh");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Number && t.1 == "0FFh")
    );
}

/// The decimal `5` must not eat the `d` of `dword`.
#[test]
fn zero_b_not_binary_when_not_bit() {
    let toks = tok("    mov 5, dword");
    assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "5"));
}

#[test]
fn semicolon_comment() {
    let toks = tok("    ret ; return to caller");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "ret")
    );
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
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Attribute && t.1 == ".globl")
    );
}

#[test]
fn nasm_directive() {
    let toks = tok("section .text");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Attribute && t.1 == "section")
    );
}

#[test]
fn string_literal() {
    let toks = tok(r#"    .asciz "Hello, World!\n""#);
    assert!(toks.iter().any(|t| t.0 == TokenKind::String));
}

#[test]
fn sse_registers() {
    let toks = tok("    movaps xmm0, xmm1");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "xmm0")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "xmm1")
    );
}

#[test]
fn case_insensitive_mnemonics() {
    let toks = tok("    MOV EAX, EBX");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "MOV")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "EAX")
    );
}

#[test]
fn nasm_preprocessor() {
    let toks = tok("%define BUFFER_SIZE 1024");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Attribute && t.1 == "%define")
    );
}

#[test]
fn binary_number() {
    let toks = tok("    mov al, 0b11001010");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Number && t.1 == "0b11001010")
    );
}

/// Unterminated string runs to EOL without panic.
#[test]
fn unterminated_string_no_panic() {
    let toks = tok(r#"    db "unclosed"#);
    assert!(toks.iter().any(|t| t.0 == TokenKind::String));
}

/// Full span coverage with no gaps for representative listings.
#[test]
fn covers_full_line() {
    for line in [
        "    mov eax, [rbx+8] ; load",
        "main: push rbp",
        "    .asciz \"hi\\n\"",
        "%define X 1",
    ] {
        let (toks, _) = tokenize_line(line, &Language::Asm, false);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len(), "span mismatch for {line:?}");
    }
}

/// Keyword-set lookups must stay behaviourally identical to slice
/// membership: every sample entry resolves and unknowns don't.
#[test]
fn keyword_sets_match_slice_contents() {
    use super::tables::{is_directive, is_mnemonic, is_register};
    for r in ["rax", "eax", "al", "rip", "xmm0"] {
        assert!(is_register(r), "register `{r}` lost");
        assert!(!is_register(&format!("not_{r}")));
    }
    for m in ["mov", "add", "ret", "call", "jmp"] {
        assert!(is_mnemonic(m), "mnemonic `{m}` lost");
    }
    for d in [".text", ".data", ".global"] {
        assert!(is_directive(d), "directive `{d}` lost");
    }
    assert!(!is_register("hello"));
    assert!(!is_mnemonic("hello"));
    assert!(!is_directive("hello"));
}
