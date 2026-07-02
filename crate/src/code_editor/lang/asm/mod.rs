//! x86/x86-64 assembly tokenizer (AT&T + Intel/NASM/MASM unified).
//!
//! Covers both AT&T (`%rax`, `$42`, `#` comments) and Intel (`rax`, `;` comments)
//! syntax simultaneously. Registers → [`TokenKind::TypeName`],
//! mnemonics → [`TokenKind::Keyword`], directives → [`TokenKind::Attribute`],
//! labels → [`TokenKind::MacroCall`].
//!
//! Split into:
//! - [`tables`] — register / mnemonic / directive data + lookup caches.
//! - [`tokenize`] — the line tokenizer state machine (+ unit tests).

use super::{is_ident_continue, is_ident_start, scan_until, scan_ws};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

mod tables;
#[cfg(test)]
mod tests;
mod tokenize;

pub(super) use tokenize::tokenize;

// ── Language definition ─────────────────────────────────────────────────────

pub struct AsmLang;

impl SyntaxDefinition for AsmLang {
    fn name(&self) -> &str {
        "Assembly"
    }

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        (tokenize(line), LineState::Code)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some(";")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('(', ')')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[':']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("(", ")"), ("\"", "\""), ("'", "'")]
    }
}
