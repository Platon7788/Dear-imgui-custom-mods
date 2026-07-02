//! Rhai scripting language tokenizer.
//!
//! Split into:
//! - [`tokenize`] — the line tokenizer state machine, `consume_token`, the
//!   backtick-template `${…}` interpolation sub-tokenizer, the scan helpers,
//!   and the `KEYWORDS` / `BUILTIN_TYPES` data tables (+ unit tests).
//!
//! The public entry point is [`RhaiLang`], a unit struct implementing
//! [`SyntaxDefinition`]; the dispatcher in [`super`] matches on it.

use super::{NumberOpts, consume_char_literal, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

#[cfg(test)]
mod tests;
mod tokenize;

pub(super) use tokenize::tokenize;

// ── Language definition ─────────────────────────────────────────────────────

pub struct RhaiLang;

impl SyntaxDefinition for RhaiLang {
    fn name(&self) -> &str {
        "Rhai"
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
        &['{']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &['}']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[
            ("(", ")"),
            ("{", "}"),
            ("[", "]"),
            ("\"", "\""),
            ("'", "'"),
            ("`", "`"),
        ]
    }
}
