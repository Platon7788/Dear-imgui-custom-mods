//! Unified-diff / patch tokenizer.
//!
//! Whole-line colouring driven by the line's leading character(s):
//! file headers and hunk ranges stand out, added lines read as "String"
//! (green), removed lines as "Operator" (red), and metadata lines
//! (`diff`, `index`, `rename`, …) as keywords. A single token spans the
//! whole line, so the span-tiling invariant holds trivially.

use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

// ── Language definition ─────────────────────────────────────────────────────

pub struct DiffLang;

impl SyntaxDefinition for DiffLang {
    fn name(&self) -> &str {
        "Diff"
    }

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        (tokenize(line), LineState::Code)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        None
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[]
    }
    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }
    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

/// Classify a diff line purely by its prefix.
fn classify(line: &str) -> TokenKind {
    // Hunk range header (`@@ -a,b +c,d @@`).
    if line.starts_with("@@") {
        return TokenKind::Attribute;
    }
    // File headers (`--- a/file` / `+++ b/file`). Checked before the
    // single-char `+`/`-` added/removed cases below.
    if line.starts_with("+++ ") || line.starts_with("--- ") {
        return TokenKind::Attribute;
    }
    // Metadata / index lines emitted by `git diff`.
    for kw in [
        "diff ",
        "index ",
        "similarity",
        "rename",
        "new file",
        "deleted file",
    ] {
        if line.starts_with(kw) {
            return TokenKind::Keyword;
        }
    }
    // Added / removed content. `TokenKind::Operator` is reused here purely
    // as the "removed line" colour (red in every bundled theme).
    match line.as_bytes()[0] {
        b'+' => TokenKind::String,   // added line (green)
        b'-' => TokenKind::Operator, // removed line (red)
        _ => TokenKind::Identifier,  // context / other → plain
    }
}

fn tokenize(line: &str) -> Vec<Token> {
    if line.is_empty() {
        return Vec::new();
    }
    vec![Token {
        kind: classify(line),
        start: 0,
        len: line.len(),
    }]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn kind(line: &str) -> Option<TokenKind> {
        let (tokens, _) = tokenize_line(line, &Language::Diff, LineState::Code);
        assert!(
            tokens.len() <= 1,
            "diff should emit at most one token per line"
        );
        tokens.first().map(|t| t.kind)
    }

    #[test]
    fn empty_line_no_tokens() {
        assert_eq!(kind(""), None);
    }

    #[test]
    fn file_headers_are_attributes() {
        assert_eq!(kind("--- a/src/main.rs"), Some(TokenKind::Attribute));
        assert_eq!(kind("+++ b/src/main.rs"), Some(TokenKind::Attribute));
    }

    #[test]
    fn hunk_header_is_attribute() {
        assert_eq!(
            kind("@@ -1,4 +1,6 @@ fn main()"),
            Some(TokenKind::Attribute)
        );
    }

    #[test]
    fn added_and_removed_lines() {
        assert_eq!(kind("+    let x = 1;"), Some(TokenKind::String));
        assert_eq!(kind("-    let x = 0;"), Some(TokenKind::Operator));
    }

    #[test]
    fn metadata_lines_are_keywords() {
        assert_eq!(kind("diff --git a/x b/x"), Some(TokenKind::Keyword));
        assert_eq!(kind("index e69de29..0000000"), Some(TokenKind::Keyword));
        assert_eq!(kind("new file mode 100644"), Some(TokenKind::Keyword));
    }

    #[test]
    fn context_line_is_plain() {
        assert_eq!(kind(" unchanged context"), Some(TokenKind::Identifier));
    }

    #[test]
    fn whole_line_span() {
        let line = "+added content here";
        let (toks, _) = tokenize_line(line, &Language::Diff, LineState::Code);
        assert_eq!(toks[0].len, line.len());
    }
}
