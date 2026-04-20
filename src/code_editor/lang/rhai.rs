//! Rhai scripting language tokenizer.

use super::{consume_decimal, is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &[
    "let",
    "const",
    "if",
    "else",
    "while",
    "loop",
    "for",
    "in",
    "do",
    "break",
    "continue",
    "return",
    "throw",
    "try",
    "catch",
    "fn",
    "private",
    "import",
    "export",
    "as",
    "switch",
    "is",
    "type_of",
    "print",
    "debug",
    "true",
    "false",
    "this",
    "call",
    "curry",
    "is_def_fn",
    "is_def_var",
    "is_shared",
    "eval",
];

const BUILTIN_TYPES: &[&str] = &[
    "bool", "char", "i64", "f64", "String", "Array", "Map", "Blob", "Dynamic", "Instant", "FnPtr",
    "Decimal",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct RhaiLang;

impl SyntaxDefinition for RhaiLang {
    fn name(&self) -> &str {
        "Rhai"
    }

    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool) {
        tokenize(line, in_block_comment)
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

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str, mut in_block_comment: bool) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    while i < len {
        // ── Inside block comment ─────────────────────────────────────────
        if in_block_comment {
            let start = i;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len {
                    break;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

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

        // ── Line comment ─────────────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return (tokens, in_block_comment);
        }

        // ── Block comment ────────────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            in_block_comment = true;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len {
                    break;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── String literal (double-quote or backtick) ────────────────────
        if b == b'"' || b == b'`' {
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

        // ── Char literal ─────────────────────────────────────────────────
        if b == b'\'' {
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'\\' {
                i += 1;
                if i < len {
                    i += 1;
                }
            } else if i < len {
                i += 1;
            }
            if i < len && bytes[i] == b'\'' {
                i += 1;
                tokens.push(Token {
                    kind: TokenKind::CharLit,
                    start,
                    len: i - start,
                });
                continue;
            }
            i = start + 1;
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start,
                len: 1,
            });
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'0' && i + 1 < len && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                i += 2;
                while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                consume_decimal(&mut i, bytes);
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Identifier / Keyword / Type ──────────────────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if BUILTIN_TYPES.contains(&word)
                || word.chars().next().is_some_and(|c| c.is_uppercase())
            {
                TokenKind::TypeName
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

        // ── Operators ────────────────────────────────────────────────────
        if matches!(
            b,
            b'+' | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'='
                | b'!'
                | b'<'
                | b'>'
                | b'&'
                | b'|'
                | b'^'
                | b'~'
        ) {
            let start = i;
            i += 1;
            if i < len
                && matches!(
                    (b, bytes[i]),
                    (b'=', b'=')
                        | (b'!', b'=')
                        | (b'<', b'=')
                        | (b'>', b'=')
                        | (b'-', b'>')
                        | (b'=', b'>')
                        | (b'&', b'&')
                        | (b'|', b'|')
                        | (b'+', b'=')
                        | (b'-', b'=')
                        | (b'*', b'=')
                        | (b'/', b'=')
                )
            {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(
            b,
            b'(' | b')'
                | b'{'
                | b'}'
                | b'['
                | b']'
                | b';'
                | b':'
                | b','
                | b'.'
                | b'@'
                | b'?'
                | b'#'
        ) {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Fallback ─────────────────────────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    (tokens, in_block_comment)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Rhai, false);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn keywords() {
        let toks = tok("let x = fn() { return 42; };");
        assert_eq!(toks[0].0, TokenKind::Keyword);
        assert_eq!(toks[0].1, "let");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "fn")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Keyword && t.1 == "return")
        );
    }

    #[test]
    fn strings() {
        let toks = tok(r#"let s = "hello world";"#);
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].1, r#""hello world""#);
    }

    #[test]
    fn backtick_string() {
        let toks = tok("let s = `template`;");
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].1, "`template`");
    }

    #[test]
    fn block_comment() {
        let (_, still_in) = tokenize_line("/* start", &Language::Rhai, false);
        assert!(still_in);
        let (toks, done) = tokenize_line("end */ code", &Language::Rhai, true);
        assert!(!done);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn builtin_types() {
        let toks = tok("let a: Dynamic = 42;");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::TypeName && t.1 == "Dynamic")
        );
    }
}
