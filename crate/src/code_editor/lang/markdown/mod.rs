//! Markdown tokenizer.
//!
//! A full-quality Markdown highlighter that is **stateful** across lines via
//! [`LineState::Fenced`]. Fenced code blocks (```` ``` ```` / `~~~`) are the
//! reason multi-line state matters: a `# comment` *inside* a code block must
//! not be coloured as a heading, so the open-fence line stores the fence byte
//! and length in the carried [`LineState`] and every body line stays plain
//! until a matching close fence is seen.
//!
//! Block constructs (per line, after optional indentation) live in [`block`]:
//! * fenced code open/close — ```` ``` ```` or `~~~` (3+ of the same char)
//! * ATX headings — `#`..`######` + space
//! * thematic break / horizontal rule — a line of only `---` / `***` / `___`
//! * blockquotes — leading `>`
//! * list markers — `- ` / `* ` / `+ ` / `1. ` / `1) `
//!
//! Inline constructs (within a normal line, blockquote body, or list body)
//! live in [`inline`]:
//! * inline code — `` `…` `` (run-length matched)
//! * bold — `**…**` / `__…__`, italic — `*…*` / `_…_`
//! * strikethrough — `~~…~~`
//! * links `[text](url)` and images `![alt](url)` — text/alt vs url coloured
//!   distinctly, brackets/parens as punctuation
//! * autolinks — `<http://…>` / `<mailto…>`
//! * backslash escapes — `\*` etc. render literally, never as emphasis
//!
//! Everything else renders as plain text. Unclosed / partial inline markers
//! still tile: they fall back to punctuation or plain text. The tokenizer
//! never panics, always advances at least one byte per step, and its spans
//! exactly tile the line on char boundaries (the invariant enforced by
//! `all_langs_no_panic_full_coverage_both_states`).

use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

mod block;
mod inline;
#[cfg(test)]
mod tests;

use block::{tokenize_fenced, tokenize_normal};

// ── Language definition ─────────────────────────────────────────────────────

pub struct MarkdownLang;

impl SyntaxDefinition for MarkdownLang {
    fn name(&self) -> &str {
        "Markdown"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        match state {
            // Only the fenced-code carry state changes tokenization; every
            // other incoming state (Code, BlockComment, Str, …) is treated as
            // an ordinary Markdown line.
            LineState::Fenced { fence, count } => tokenize_fenced(line, fence, count),
            _ => tokenize_normal(line),
        }
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        // Markdown has no line comment; HTML-style block comments only.
        None
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("<!--", "-->"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('(', ')')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("(", ")"), ("`", "`")]
    }
}

// ── Small helper ────────────────────────────────────────────────────────────

#[inline]
fn tok(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        start,
        len: end - start,
    }
}
