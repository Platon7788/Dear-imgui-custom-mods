//! SQL tokenizer.
//!
//! Case-insensitive keyword highlighting, `--` line comments and
//! `/* … */` block comments (scanned within a single line — this
//! tokenizer is stateless), single-quoted strings with `''` escaping,
//! and double-quoted / backtick quoted identifiers.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

/// Reserved words compared against the **uppercased** identifier, so
/// `select`, `Select` and `SELECT` all match.
const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "ALTER",
    "DROP",
    "TABLE",
    "INDEX",
    "VIEW",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "ON",
    "AS",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "LIKE",
    "BETWEEN",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "UNION",
    "ALL",
    "DISTINCT",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "DEFAULT",
    "UNIQUE",
    "CHECK",
    "CONSTRAINT",
    "CASCADE",
    "INT",
    "INTEGER",
    "BIGINT",
    "SMALLINT",
    "VARCHAR",
    "CHAR",
    "TEXT",
    "BOOLEAN",
    "DATE",
    "TIMESTAMP",
    "DECIMAL",
    "NUMERIC",
    "FLOAT",
    "DOUBLE",
    "SERIAL",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "EXISTS",
    "TRUE",
    "FALSE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "TRANSACTION",
    "WITH",
    "RETURNING",
    "IF",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct SqlLang;

impl SyntaxDefinition for SqlLang {
    fn name(&self) -> &str {
        "SQL"
    }

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        (tokenize(line), LineState::Code)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("--")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("'", "'"), ("\"", "\"")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

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

        // ── Line comment (`-- …`) — before the `-` operator ──────────────
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // ── Block comment (`/* … */`) — non-nesting, single line ─────────
        // Scanned to the first `*/`; if unterminated, runs to end of line.
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i < len {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Single-quoted string (`''` = escaped quote) ──────────────────
        if b == b'\'' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\'' {
                    if i + 1 < len && bytes[i + 1] == b'\'' {
                        i += 2; // doubled quote — stays inside the string
                        continue;
                    }
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

        // ── Quoted identifiers: "…" and `…` ──────────────────────────────
        if b == b'"' || b == b'`' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                i += 1;
            }
            if i < len {
                i += 1; // consume closing quote
            }
            tokens.push(Token {
                kind: TokenKind::Identifier,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Number (decimal / float, no radix or underscores) ────────────
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

        // ── Identifier / keyword (case-insensitive) ──────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            let upper = word.to_ascii_uppercase();
            let kind = if KEYWORDS.contains(&upper.as_str()) {
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

        // ── Operators (`= < > <= >= <> != + - * / %`) ────────────────────
        match b {
            b'<' => {
                let n = if i + 1 < len && (bytes[i + 1] == b'=' || bytes[i + 1] == b'>') {
                    2
                } else {
                    1
                };
                tokens.push(Token {
                    kind: TokenKind::Operator,
                    start: i,
                    len: n,
                });
                i += n;
                continue;
            }
            b'>' | b'!' => {
                let n = if i + 1 < len && bytes[i + 1] == b'=' {
                    2
                } else {
                    1
                };
                tokens.push(Token {
                    kind: TokenKind::Operator,
                    start: i,
                    len: n,
                });
                i += n;
                continue;
            }
            b'=' | b'+' | b'-' | b'*' | b'/' | b'%' => {
                tokens.push(Token {
                    kind: TokenKind::Operator,
                    start: i,
                    len: 1,
                });
                i += 1;
                continue;
            }
            _ => {}
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(b, b'(' | b')' | b',' | b';' | b'.') {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Fallback (non-ASCII / unknown byte) ──────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
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
        let (tokens, _) = tokenize_line(line, &Language::Sql, LineState::Code);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn keywords_case_insensitive() {
        for kw in ["SELECT", "select", "Select"] {
            let toks = tok(&format!("{kw} *"));
            assert!(
                toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == kw),
                "expected keyword for {kw:?}"
            );
        }
    }

    #[test]
    fn line_comment() {
        let toks = tok("SELECT 1 -- trailing");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Comment && t.1 == "-- trailing")
        );
    }

    #[test]
    fn block_comment_inline() {
        let toks = tok("a /* c */ b");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Comment && t.1 == "/* c */")
        );
    }

    #[test]
    fn block_comment_unterminated_runs_to_eol() {
        let toks = tok("x /* open");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Comment && t.1 == "/* open")
        );
    }

    #[test]
    fn string_with_doubled_quote() {
        let line = "'it''s ok'";
        let toks = tok(line);
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, TokenKind::String);
        assert_eq!(toks[0].1, line);
    }

    #[test]
    fn quoted_identifier() {
        let toks = tok("\"my col\" + `other`");
        let idents: Vec<_> = toks
            .iter()
            .filter(|t| t.0 == TokenKind::Identifier)
            .map(|t| t.1.clone())
            .collect();
        assert!(idents.contains(&"\"my col\"".to_string()));
        assert!(idents.contains(&"`other`".to_string()));
    }

    #[test]
    fn operators() {
        let toks = tok("a <= b <> c != d >= 1");
        let ops: Vec<_> = toks
            .iter()
            .filter(|t| t.0 == TokenKind::Operator)
            .map(|t| t.1.clone())
            .collect();
        assert!(ops.contains(&"<=".to_string()));
        assert!(ops.contains(&"<>".to_string()));
        assert!(ops.contains(&"!=".to_string()));
        assert!(ops.contains(&">=".to_string()));
    }

    #[test]
    fn number_literal() {
        let toks = tok("WHERE price > 3.14");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "3.14")
        );
    }
}
