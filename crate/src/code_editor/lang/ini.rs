//! INI / config-file tokenizer.
//!
//! `[section]` headers and `key` names render as attributes, the `=` / `:`
//! separator as an operator, and values as strings / numbers / identifiers.
//! Comments start with `;` or `#`. This tokenizer is stateless.

use super::{NumberOpts, consume_number};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

// ── Language definition ─────────────────────────────────────────────────────

pub struct IniLang;

impl SyntaxDefinition for IniLang {
    fn name(&self) -> &str {
        "INI"
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
        &[('[', ']')]
    }
    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }
    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("\"", "\""), ("'", "'")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

/// Push a whitespace run (spaces / tabs) starting at `*i`, if any.
fn push_ws(tokens: &mut Vec<Token>, bytes: &[u8], i: &mut usize) {
    let len = bytes.len();
    if *i < len && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
        let start = *i;
        while *i < len && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
            *i += 1;
        }
        tokens.push(Token {
            kind: TokenKind::Whitespace,
            start,
            len: *i - start,
        });
    }
}

/// Tokenize the value region (right-hand side of `key =`, remainder of a
/// section line, or a bare line with no separator) from `*i` to end.
fn tokenize_value(line: &str, tokens: &mut Vec<Token>, i: &mut usize) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    while *i < len {
        let b = bytes[*i];

        // Whitespace.
        if b == b' ' || b == b'\t' {
            push_ws(tokens, bytes, i);
            continue;
        }

        // Comment to end of line.
        if b == b';' || b == b'#' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: *i,
                len: len - *i,
            });
            *i = len;
            return;
        }

        // Quoted string.
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = *i;
            *i += 1;
            while *i < len {
                if bytes[*i] == b'\\' && quote == b'"' && *i + 1 < len {
                    *i += 2;
                    continue;
                }
                if bytes[*i] == quote {
                    *i += 1;
                    break;
                }
                *i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: *i - start,
            });
            continue;
        }

        // Number.
        if b.is_ascii_digit() {
            let start = *i;
            consume_number(i, bytes, NumberOpts::JSON);
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: *i - start,
            });
            continue;
        }

        // Bare value run — up to whitespace or a comment marker.
        let start = *i;
        while *i < len {
            let c = bytes[*i];
            if c == b' ' || c == b'\t' || c == b';' || c == b'#' {
                break;
            }
            *i += 1;
        }
        if *i == start {
            // Defensive: guarantee forward progress on any stray byte.
            let adv = line[start..].chars().next().map_or(1, |c| c.len_utf8());
            *i += adv;
        }
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start,
            len: *i - start,
        });
    }
}

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    // Leading whitespace.
    push_ws(&mut tokens, bytes, &mut i);
    if i >= len {
        return tokens;
    }

    // Full-line comment.
    if bytes[i] == b';' || bytes[i] == b'#' {
        tokens.push(Token {
            kind: TokenKind::Comment,
            start: i,
            len: len - i,
        });
        return tokens;
    }

    // Section header `[section]`.
    if bytes[i] == b'[' {
        let start = i;
        while i < len && bytes[i] != b']' {
            i += 1;
        }
        if i < len {
            i += 1; // include the closing `]`
        }
        tokens.push(Token {
            kind: TokenKind::Attribute,
            start,
            len: i - start,
        });
        // Anything trailing (whitespace / comment) is tokenized as a value.
        tokenize_value(line, &mut tokens, &mut i);
        return tokens;
    }

    // Key = value (or key : value). Scan up to the first separator.
    let key_start = i;
    while i < len {
        let c = bytes[i];
        if c == b'=' || c == b':' || c == b';' || c == b'#' {
            break;
        }
        i += 1;
    }
    let stop = i;
    let has_separator = stop < len && (bytes[stop] == b'=' || bytes[stop] == b':');

    if has_separator {
        // Trim trailing whitespace from the key.
        let mut key_end = stop;
        while key_end > key_start && (bytes[key_end - 1] == b' ' || bytes[key_end - 1] == b'\t') {
            key_end -= 1;
        }
        if key_end > key_start {
            tokens.push(Token {
                kind: TokenKind::Attribute,
                start: key_start,
                len: key_end - key_start,
            });
        }
        if key_end < stop {
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                start: key_end,
                len: stop - key_end,
            });
        }
        // Separator operator.
        tokens.push(Token {
            kind: TokenKind::Operator,
            start: stop,
            len: 1,
        });
        i = stop + 1;
    } else {
        // No `=`/`:` — treat the whole remainder as a value (may hold a
        // trailing comment).
        i = key_start;
    }

    tokenize_value(line, &mut tokens, &mut i);
    tokens
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Ini, LineState::Code);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn section_header() {
        let toks = tok("[database]");
        assert_eq!(toks[0].0, TokenKind::Attribute);
        assert_eq!(toks[0].1, "[database]");
    }

    #[test]
    fn key_value_equals() {
        let toks = tok("host = localhost");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "host")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Operator && t.1 == "=")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Identifier && t.1 == "localhost")
        );
    }

    #[test]
    fn key_value_colon() {
        let toks = tok("port: 8080");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "port")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Operator && t.1 == ":")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "8080")
        );
    }

    #[test]
    fn semicolon_comment() {
        let toks = tok("; a comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    #[test]
    fn hash_comment() {
        let toks = tok("# also a comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    #[test]
    fn quoted_string_value() {
        let toks = tok("name = \"hello world\"");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::String && t.1 == "\"hello world\"")
        );
    }

    #[test]
    fn inline_comment_after_value() {
        let toks = tok("x = 1 ; trailing");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Number && t.1 == "1"));
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Comment && t.1 == "; trailing")
        );
    }
}
