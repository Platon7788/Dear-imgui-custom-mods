//! YAML tokenizer.
//!
//! Handles document markers (`---`/`...`), anchors (`&name`), aliases (`*name`),
//! tags (`!type`), directives (`%YAML`), flow collections, and keyword literals.

use super::{consume_decimal, is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

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

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        (tokenize(line), false)
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

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    // Leading whitespace (significant in YAML)
    if i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        let start = i;
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        tokens.push(Token {
            kind: TokenKind::Whitespace,
            start,
            len: i - start,
        });
    }

    // Full-line comment
    if i < len && bytes[i] == b'#' {
        tokens.push(Token {
            kind: TokenKind::Comment,
            start: i,
            len: len - i,
        });
        return tokens;
    }

    // Directive (%YAML, %TAG)
    if i < len && bytes[i] == b'%' {
        tokens.push(Token {
            kind: TokenKind::Attribute,
            start: i,
            len: len - i,
        });
        return tokens;
    }

    // Document markers (--- or ...)
    {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "..." {
            tokens.push(Token {
                kind: TokenKind::Keyword,
                start: i,
                len: len - i,
            });
            return tokens;
        }
    }

    while i < len {
        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Comment ──────────────────────────────────────────────────────
        if b == b'#' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // ── Anchor (&name) / Alias (*name) ───────────────────────────────
        if (b == b'&' || b == b'*') && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::MacroCall,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Tag (!!type or !custom) ──────────────────────────────────────
        if b == b'!' {
            let start = i;
            i += 1;
            while i < len
                && bytes[i] != b' '
                && bytes[i] != b'\t'
                && bytes[i] != b'\n'
                && bytes[i] != b','
            {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::TypeName,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Quoted string ────────────────────────────────────────────────
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len && quote == b'"' {
                    i += 2;
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Key-value colon ──────────────────────────────────────────────
        if b == b':' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── List dash ────────────────────────────────────────────────────
        if b == b'-' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Flow punctuation ─────────────────────────────────────────────
        if matches!(b, b'{' | b'}' | b'[' | b']' | b',') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let save = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            if i + 1 < len && bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                i += 2;
                while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                consume_decimal(&mut i, bytes);
            }
            // Only treat as number if followed by whitespace/end/punctuation
            if i >= len
                || bytes[i] == b' '
                || bytes[i] == b'\t'
                || bytes[i] == b'#'
                || bytes[i] == b','
                || bytes[i] == b']'
                || bytes[i] == b'}'
            {
                tokens.push(Token {
                    kind: TokenKind::Number,
                    start,
                    len: i - start,
                });
                continue;
            }
            i = save; // not a number — fall through to unquoted string
        }

        // ── Unquoted string / bare value ─────────────────────────────────
        {
            let start = i;
            while i < len {
                let c = bytes[i];
                if c == b'#' {
                    break;
                }
                if c == b':' && (i + 1 >= len || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') {
                    break;
                }
                if matches!(c, b'{' | b'}' | b'[' | b']' | b',') {
                    break;
                }
                i += 1;
            }
            // Trim trailing whitespace from the token
            let mut end = i;
            while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
                end -= 1;
            }
            if end > start {
                let word = &line[start..end];
                let kind = if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else {
                    TokenKind::Identifier
                };
                tokens.push(Token {
                    kind,
                    start,
                    len: end - start,
                });
            }
            if end < i {
                tokens.push(Token {
                    kind: TokenKind::Whitespace,
                    start: end,
                    len: i - end,
                });
            }
        }
    }

    tokens
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Yaml, false);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn key_value() {
        let toks = tok("name: hello");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Identifier && t.1 == "name")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Operator && t.1 == ":")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Identifier && t.1 == "hello")
        );
    }

    #[test]
    fn comment() {
        let toks = tok("# this is a comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    #[test]
    fn document_marker() {
        let toks = tok("---");
        assert_eq!(toks[0].0, TokenKind::Keyword);
    }

    #[test]
    fn yaml_keywords() {
        let toks = tok("enabled: true");
        let kws: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Keyword).collect();
        assert_eq!(kws.len(), 1);
        assert_eq!(kws[0].1, "true");
    }

    #[test]
    fn anchor_alias() {
        let toks = tok("base: &default");
        let macros: Vec<_> = toks
            .iter()
            .filter(|t| t.0 == TokenKind::MacroCall)
            .collect();
        assert_eq!(macros.len(), 1);
        assert_eq!(macros[0].1, "&default");
    }

    #[test]
    fn tag() {
        let toks = tok("timestamp: !!timestamp 2024-01-01");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::TypeName && t.1 == "!!timestamp")
        );
    }

    #[test]
    fn list_item() {
        let toks = tok("  - item");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Punctuation && t.1 == "-")
        );
    }
}
