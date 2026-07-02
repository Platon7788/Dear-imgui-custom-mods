//! Dockerfile tokenizer.
//!
//! The leading instruction keyword (`FROM`, `RUN`, `COPY`, …) is matched
//! case-insensitively, `#` starts a comment (except `# syntax=` /
//! `# escape=` parser directives), `$VAR` / `${VAR}` are variables, and
//! `--flag` options highlight as attributes. This tokenizer is stateless.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start, scan_ws};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Dockerfile instructions, matched against the **uppercased** first word.
const INSTRUCTIONS: &[&str] = &[
    "FROM",
    "RUN",
    "CMD",
    "LABEL",
    "MAINTAINER",
    "EXPOSE",
    "ENV",
    "ADD",
    "COPY",
    "ENTRYPOINT",
    "VOLUME",
    "USER",
    "WORKDIR",
    "ARG",
    "ONBUILD",
    "STOPSIGNAL",
    "HEALTHCHECK",
    "SHELL",
];

/// [`INSTRUCTIONS`] as a hash set (uppercased keys) — one hash + probe per
/// leading word instead of a linear scan (mirrors `sql` / `asm::tables`).
fn instructions_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| INSTRUCTIONS.iter().copied().collect())
}

// ── Language definition ─────────────────────────────────────────────────────

pub struct DockerfileLang;

impl SyntaxDefinition for DockerfileLang {
    fn name(&self) -> &str {
        "Dockerfile"
    }

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        (tokenize(line), LineState::Code)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("#")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(12);
    let mut i = 0;

    // Leading whitespace.
    scan_ws(&mut tokens, bytes, &mut i);

    if i >= len {
        return tokens;
    }

    // Comment or parser directive.
    if bytes[i] == b'#' {
        // `# syntax=` / `# escape=` parser directives render as attributes.
        let directive = line[i + 1..].trim_start().to_ascii_lowercase();
        let kind = if directive.starts_with("syntax=") || directive.starts_with("escape=") {
            TokenKind::Attribute
        } else {
            TokenKind::Comment
        };
        tokens.push(Token {
            kind,
            start: i,
            len: len - i,
        });
        return tokens;
    }

    // Leading instruction keyword (first word only).
    if is_ident_start(bytes[i]) {
        let start = i;
        while i < len && is_ident_continue(bytes[i]) {
            i += 1;
        }
        let word = &line[start..i];
        // Only allocate the uppercased copy when the word contains a
        // lowercase byte; otherwise borrow and compare in place.
        let word_upper: String;
        let word_uc = if word.bytes().any(|c| c.is_ascii_lowercase()) {
            word_upper = word.to_ascii_uppercase();
            word_upper.as_str()
        } else {
            word
        };
        let kind = if instructions_set().contains(word_uc) {
            TokenKind::Keyword
        } else {
            TokenKind::Identifier
        };
        tokens.push(Token {
            kind,
            start,
            len: i - start,
        });
    }

    // Remainder of the line.
    while i < len {
        let b = bytes[i];

        // Whitespace.
        if b == b' ' || b == b'\t' {
            scan_ws(&mut tokens, bytes, &mut i);
            continue;
        }

        // Inline comment.
        if b == b'#' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // Quoted string.
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && quote == b'"' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            continue;
        }

        // Variable: `$VAR` or `${VAR}`.
        if b == b'$' {
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'{' {
                i += 1;
                while i < len && bytes[i] != b'}' {
                    i += 1;
                }
                if i < len {
                    i += 1; // include `}`
                }
            } else {
                while i < len && is_ident_continue(bytes[i]) {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: TokenKind::MacroCall,
                start,
                len: i - start,
            });
            continue;
        }

        // Option flag: `--flag`.
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            let start = i;
            i += 2;
            while i < len && bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'=' {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Attribute,
                start,
                len: i - start,
            });
            continue;
        }

        // Number.
        if b.is_ascii_digit() {
            let start = i;
            consume_number(&mut i, bytes, NumberOpts::JSON);
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // Identifier / `AS` keyword (multi-stage builds).
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if word.eq_ignore_ascii_case("AS") {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // Fallback: `=` reads as an operator, anything else stays plain.
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        let kind = if b == b'=' {
            TokenKind::Operator
        } else {
            TokenKind::Identifier
        };
        tokens.push(Token {
            kind,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    tokens
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Dockerfile, LineState::Code);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn instruction_keyword() {
        let toks = tok("FROM rust:1.90 AS builder");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "FROM")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "AS")
        );
    }

    #[test]
    fn instruction_case_insensitive() {
        let toks = tok("run echo hi");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "run")
        );
    }

    #[test]
    fn comment() {
        let toks = tok("# a comment");
        assert_eq!(toks[0].0, TokenKind::Comment);
    }

    #[test]
    fn parser_directive() {
        let toks = tok("# syntax=docker/dockerfile:1");
        assert_eq!(toks[0].0, TokenKind::Attribute);
    }

    #[test]
    fn variables() {
        let toks = tok("ENV PATH=$HOME");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::MacroCall && t.1 == "$HOME")
        );
        let toks = tok("RUN echo ${MY_VAR}");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::MacroCall && t.1 == "${MY_VAR}")
        );
    }

    #[test]
    fn option_flag() {
        let toks = tok("COPY --from=builder /app /app");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "--from")
        );
    }

    #[test]
    fn string_value() {
        let toks = tok("CMD [\"echo\", \"hi\"]");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::String && t.1 == "\"echo\"")
        );
    }
}
