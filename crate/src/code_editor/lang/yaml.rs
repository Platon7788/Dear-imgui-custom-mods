//! YAML tokenizer.
//!
//! Handles document markers (`---`/`...`), anchors (`&name`), aliases (`*name`),
//! tags (`!type`), directives (`%YAML`), flow collections, and keyword literals.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::{LineState, SyntaxDefinition};
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

    fn tokenize_line(&self, line: &str, _state: LineState) -> (Vec<Token>, LineState) {
        (tokenize(line), LineState::Code)
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

    // Document markers (--- or ...) — may carry trailing content
    // (`--- !tag`, `--- key: val`), so emit the 3-char marker and keep
    // tokenizing the rest of the line instead of returning.
    if (bytes[i..].starts_with(b"---") || bytes[i..].starts_with(b"..."))
        && (i + 3 == len || bytes[i + 3] == b' ' || bytes[i + 3] == b'\t')
    {
        tokens.push(Token {
            kind: TokenKind::Keyword,
            start: i,
            len: 3,
        });
        i += 3;
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
        // YAML only starts a comment when `#` is preceded by whitespace (or at
        // line start); `http://x#frag` and `a#b` are scalars, not comments.
        if b == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // ── Null tilde `~` ───────────────────────────────────────────────
        if b == b'~' {
            tokens.push(Token {
                kind: TokenKind::Keyword,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Block scalar indicator `|` / `>` (+ chomping -/+ and indent digit) ──
        // Only the indicator is coloured; the indented body needs cross-line
        // state the stateless tokenizer doesn't carry.
        if b == b'|' || b == b'>' {
            let start = i;
            i += 1;
            while i < len && matches!(bytes[i], b'-' | b'+' | b'0'..=b'9') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                start,
                len: i - start,
            });
            continue;
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
        // YAML 1.1 supports decimal, `0x`, `0b`, `0o`, `_` separators,
        // and floats. But — unlike Rust/RON — a "number" tail must end
        // at whitespace or structural punctuation; otherwise the run
        // is a bare-string scalar (e.g. `2:30`, `1.2.3`).
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let save = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            // Only treat as number if followed by whitespace/end/punctuation.
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
                // Comment only when `#` follows whitespace — keep `#` inside
                // scalars like `http://x#frag` / `a#b`.
                if c == b'#' && i > 0 && matches!(bytes[i - 1], b' ' | b'\t') {
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
            // Guaranteed-advance guard: if a leading delimiter ever slipped
            // past the earlier branches, the loop broke at `start` with `i`
            // unmoved — consume one char as punctuation so the outer scan
            // can't spin forever.
            if i == start && i < len {
                let adv = line[i..].chars().next().map_or(1, |c| c.len_utf8());
                tokens.push(Token {
                    kind: TokenKind::Punctuation,
                    start,
                    len: adv,
                });
                i += adv;
                continue;
            }
            // Trim trailing whitespace from the token
            let mut end = i;
            while end > start && (bytes[end - 1] == b' ' || bytes[end - 1] == b'\t') {
                end -= 1;
            }
            // The bare-string scan stops at a `: ` mapping separator — when
            // it did, this token is a mapping *key*, so colour it as an
            // Attribute (key role) rather than a plain bare scalar.
            let stopped_at_key_colon = i < len && bytes[i] == b':';
            if end > start {
                let word = &line[start..end];
                let kind = if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else if stopped_at_key_colon {
                    TokenKind::Attribute
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
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn tok(line: &str) -> Vec<(TokenKind, String)> {
        let (tokens, _) = tokenize_line(line, &Language::Yaml, LineState::Code);
        tokens
            .iter()
            .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
            .collect()
    }

    #[test]
    fn hash_in_scalar_is_not_a_comment() {
        // `#` not preceded by whitespace stays part of the scalar.
        let toks = tok("url: http://x#frag");
        assert!(!toks.iter().any(|(k, _)| *k == TokenKind::Comment));
    }

    #[test]
    fn block_scalar_indicator_is_colored() {
        let toks = tok("body: |");
        assert!(toks.iter().any(|(k, s)| *k == TokenKind::Operator && s == "|"));
    }

    #[test]
    fn document_marker_with_trailing_content() {
        let toks = tok("--- !tag");
        assert!(toks.iter().any(|(k, s)| *k == TokenKind::Keyword && s == "---"));
        assert!(toks.iter().any(|(_, s)| s.contains("tag")));
    }

    #[test]
    fn key_value() {
        let toks = tok("name: hello");
        // `name` is a mapping key → Attribute.
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "name")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Operator && t.1 == ":")
        );
        // `hello` is a bare scalar value → Identifier.
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Identifier && t.1 == "hello")
        );
    }

    /// Indented (nested) keys are also Attributes.
    #[test]
    fn nested_key_is_attribute() {
        let toks = tok("  port: 8080");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Attribute && t.1 == "port")
        );
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Number && t.1 == "8080")
        );
    }

    /// A bare scalar with no following `:` stays an Identifier (not a key).
    #[test]
    fn bare_scalar_value_is_identifier() {
        let toks = tok("- plain_value");
        assert!(
            toks.iter()
                .any(|t| t.0 == TokenKind::Identifier && t.1 == "plain_value")
        );
        assert!(!toks.iter().any(|t| t.0 == TokenKind::Attribute));
    }

    /// Unterminated quoted string runs to EOL without panic.
    #[test]
    fn unterminated_quoted_no_panic() {
        let toks = tok(r#"key: "unclosed"#);
        assert!(toks.iter().any(|t| t.0 == TokenKind::String));
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

    /// YAML 1.1 supports decimal, hex (`0x`), octal (`0o`), binary
    /// (`0b`) and underscore separators. Number ends at whitespace or
    /// structural punctuation; otherwise it's a bare-string scalar.
    /// Regression for ADR-027 phase 2.
    #[test]
    fn radix_and_underscore_separators() {
        for (line, want_lit) in [
            ("a: 0xDEAD_BEEF", "0xDEAD_BEEF"),
            ("a: 0o755", "0o755"),
            ("a: 0b1010", "0b1010"),
            ("a: 1_000_000", "1_000_000"),
            ("a: 3.14", "3.14"),
        ] {
            let toks = tok(line);
            let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
            assert_eq!(nums.len(), 1, "input {line:?} produced {nums:?}");
            assert_eq!(nums[0].1, want_lit);
        }
    }

    /// Trailing-context check is preserved: `2:30` is a bare string,
    /// not a number followed by colon.
    #[test]
    fn bare_string_not_number() {
        let toks = tok("time: 2:30");
        // `2:30` should NOT produce a Number token (no whitespace
        // terminator after `2`, `:` is not a number-terminating punct).
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert!(
            nums.is_empty(),
            "expected no number tokens for bare-string `2:30`, got {nums:?}"
        );
    }
}
