//! INI / config-file syntax (`ini/`).
//!
//! Full-featured highlighting for the INI / conf / properties family:
//!
//! - **Section headers** `[section]` render as attributes; the git-config
//!   form `[core "sub"]` pulls the quoted sub-section out as a string.
//! - **Keys** on the left of `=` / `:` are attributes; the separator is an
//!   operator; trailing whitespace is trimmed off the key.
//! - **Values** are classified: quoted strings (with `\`-escape
//!   highlighting), `${VAR}` / `$VAR` / `%VAR%` interpolation, signed /
//!   float / radix numbers, boolean & null keywords (`true`/`false`/`yes`/
//!   `no`/`on`/`off`/`none`/`null`, case-insensitive), and bare runs.
//! - **Comments** open with `;` or `#` at the start of a region or after
//!   whitespace; a marker glued to a value byte (`pass#word`) stays value.
//! - **Line continuation** — a value line ending in a lone trailing `\`
//!   carries onto the next line (tracked with a repurposed [`LineState`]
//!   carry; see [`tokenize`](tokenize::tokenize)).
//!
//! The tokenizer keeps the hard span-tiling invariant (see
//! `lang::tests::all_langs_no_panic_full_coverage_all_states`): the emitted
//! tokens contiguously tile every line, on char boundaries, for every
//! incoming [`LineState`].

#[cfg(test)]
mod tests;
mod tokenize;

use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::Token;

// ── Language definition ─────────────────────────────────────────────────────

pub struct IniLang;

impl SyntaxDefinition for IniLang {
    fn name(&self) -> &str {
        "INI"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize::tokenize(line, state)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some(";")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        // `{`/`}` covers `${VAR}` interpolation for bracket-match highlighting.
        &[('[', ']'), ('{', '}')]
    }
    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }
    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")]
    }

    /// `-` and `.` join the default word set so `my-key`, `a.b.c` and dotted
    /// values select as one word on double-click / Ctrl+arrow navigation.
    fn is_word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
    }
}
