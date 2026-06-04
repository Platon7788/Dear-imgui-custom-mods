//! Rust syntax tokenizer.
//!
//! Split into:
//! - [`keywords`] — the `KEYWORDS` / `BUILTIN_TYPES` data tables.
//! - [`tokenize`] — the line tokenizer state machine (+ unit tests).
//!
//! The public entry point is [`RustLang`], a unit struct implementing
//! [`SyntaxDefinition`]; the dispatcher in [`super`] matches on it.

use super::{NumberOpts, consume_char_literal, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

mod keywords;
#[cfg(test)]
mod tests;
mod tokenize;

pub(super) use tokenize::tokenize;

// ── Language definition ─────────────────────────────────────────────────────

pub struct RustLang;

impl SyntaxDefinition for RustLang {
    fn name(&self) -> &str {
        "Rust"
    }

    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool) {
        tokenize(line, in_block_comment)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &['{']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &['}']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'")]
    }
}
