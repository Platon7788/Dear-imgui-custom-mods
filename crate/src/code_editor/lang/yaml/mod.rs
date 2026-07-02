//! YAML syntax tokenizer.
//!
//! Document markers (`---`/`...`), anchors (`&name`), aliases (`*name`),
//! tags (`!type`), directives (`%YAML`), flow collections, keyword literals,
//! and multi-line block scalars (`|`/`>`) whose literal body is carried across
//! lines via [`LineState::YamlBlock`].
//!
//! Split into:
//! - [`tokenize`] — the line tokenizer and block-scalar helpers (+ unit tests).
//!
//! The public entry point is [`YamlLang`], a unit struct implementing
//! [`SyntaxDefinition`]; the dispatcher in [`super`] matches on it.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start, scan_ws};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

#[cfg(test)]
mod tests;
mod tokenize;

pub(super) use tokenize::{block_body_line, tokenize};

const KEYWORDS: &[&str] = &[
    "true", "false", "null", "yes", "no", "on", "off", "True", "False", "Null", "Yes", "No", "On",
    "Off", "TRUE", "FALSE", "NULL", "YES", "NO", "ON", "OFF",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct YamlLang;

impl SyntaxDefinition for YamlLang {
    fn name(&self) -> &str {
        "YAML"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        // Continuation of a `|` / `>` block-scalar body carried from a
        // previous line: blank lines and lines indented `>= indent` stay in
        // the block (whole content coloured String); a dedent falls through
        // to ordinary tokenizing and clears the carry to `Code`.
        if let LineState::YamlBlock { indent } = state
            && let Some(toks) = block_body_line(line, indent)
        {
            return (toks, state);
        }
        tokenize(line)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("#")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[':']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'")]
    }

    fn is_word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-'
    }
}
