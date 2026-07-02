//! RON (Rusty Object Notation) tokenizer.
//!
//! RON is a data-only configuration format with Rust-flavoured syntax. This
//! tokenizer is similar to [`super::rust`] but tuned for RON semantics:
//!
//! - `//` line comments and `/* */` block comments (multi-line, nesting-aware).
//! - String, raw-string (`r"..."`, `r#"..."#`) and char literals.
//! - Hex / octal / binary / decimal numbers with `_` separators; optional
//!   leading sign.
//! - `true` / `false` keywords (RON has no other reserved words).
//! - Identifiers starting with an uppercase letter render as
//!   [`TokenKind::TypeName`] — matches the struct / enum-variant convention
//!   (`Some`, `None`, `Foo`).
//! - Identifiers (and quoted strings) immediately followed by `:` render as
//!   [`TokenKind::Attribute`] — the field-key / map-key role.
//! - `#![enable(...)]` extension attributes render as a single
//!   [`TokenKind::Attribute`] block.

use super::{
    NumberOpts, consume_char_literal, consume_number, is_ident_continue, is_ident_start,
    scan_block_comment,
};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

mod tokenize;

pub(super) use tokenize::tokenize;

const KEYWORDS: &[&str] = &["true", "false"];

// ── Language definition ─────────────────────────────────────────────────────

pub struct RonLang;

impl SyntaxDefinition for RonLang {
    fn name(&self) -> &str {
        "RON"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize(line, state)
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
        &['(', '{', '[']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[')', '}', ']']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'")]
    }
}
