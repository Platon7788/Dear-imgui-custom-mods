//! YAML tokenizer.
//!
//! Document markers (`---`/`...`), anchors (`&name`), aliases (`*name`),
//! tags (`!type`), directives (`%YAML`), flow collections, keyword literals,
//! and multi-line block scalars (`|`/`>`) whose literal body is carried across
//! lines via [`LineState::YamlBlock`].

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

// ── Block scalar body ───────────────────────────────────────────────────────

/// A line consumed while inside a `|` / `>` block-scalar body.
///
/// Returns `Some(tokens)` when the line stays in the block — it is blank
/// (empty or all-whitespace) or its leading indent is `>= indent` — colouring
/// the scalar content [`TokenKind::String`]. Returns `None` on a dedent so the
/// caller re-tokenizes the line as ordinary YAML. Emitted spans tile the line.
fn block_body_line(line: &str, indent: u16) -> Option<Vec<Token>> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut ws = 0;
    while ws < len && (bytes[ws] == b' ' || bytes[ws] == b'\t') {
        ws += 1;
    }
    let blank = ws == len;
    if !blank && (ws as u16) < indent {
        return None; // dedent — end of the block scalar
    }
    let mut tokens = Vec::new();
    if ws > 0 {
        tokens.push(Token {
            kind: TokenKind::Whitespace,
            start: 0,
            len: ws,
        });
    }
    if ws < len {
        tokens.push(Token {
            kind: TokenKind::String,
            start: ws,
            len: len - ws,
        });
    }
    Some(tokens)
}

/// End-of-line carry: enter a block-scalar body when a trailing `|` / `>`
/// indicator opened one, otherwise plain `Code`.
fn end_state(pending_block: Option<u16>) -> LineState {
    match pending_block {
        Some(indent) => LineState::YamlBlock { indent },
        None => LineState::Code,
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    let mut lead_indent: u16 = 0;
    // Set when a trailing block-scalar indicator opens a multi-line body.
    let mut pending_block: Option<u16> = None;

    // Leading whitespace (significant in YAML) — also the line's indent.
    if i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        let start = i;
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        lead_indent = (i - start) as u16;
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
        return (tokens, LineState::Code);
    }

    // Directive (%YAML, %TAG)
    if i < len && bytes[i] == b'%' {
        tokens.push(Token {
            kind: TokenKind::Attribute,
            start: i,
            len: len - i,
        });
        return (tokens, LineState::Code);
    }

    // Document markers (`---` / `...`) may carry trailing content, so emit the
    // 3-char marker and keep tokenizing the rest of the line.
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

        // ── Comment (only when `#` follows whitespace / line start) ───────
        if b == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return (tokens, end_state(pending_block));
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

        // ── Block scalar indicator `|` / `>` (+ chomping -/+ and indent) ──
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
            // A block scalar opens only when the indicator is the last
            // non-comment content on the line; the body's indent is
            // approximated as 1 + this line's indent.
            let mut k = i;
            while k < len && (bytes[k] == b' ' || bytes[k] == b'\t') {
                k += 1;
            }
            if k >= len || bytes[k] == b'#' {
                pending_block = Some(lead_indent.saturating_add(1));
            }
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
            while i < len && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b',') {
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
        // `"…"` honours `\` escapes; `'…'` treats a doubled `''` as an
        // escaped quote (only a lone `'` closes the scalar).
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len && quote == b'"' {
                    i += 2;
                } else if bytes[i] == quote {
                    if quote == b'\'' && i + 1 < len && bytes[i + 1] == b'\'' {
                        i += 2; // doubled '' — an escaped quote, stay in string
                    } else {
                        i += 1;
                        break;
                    }
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
        // YAML 1.1 radix/underscore/float, but the tail must end at
        // whitespace or structural punctuation; otherwise it is a bare
        // scalar (e.g. `2:30`, `1.2.3`).
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            let save = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            if i >= len || matches!(bytes[i], b' ' | b'\t' | b'#' | b',' | b']' | b'}') {
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
            // Guaranteed-advance guard: a leading delimiter that slipped past
            // consumes one char as punctuation so the scan can't spin.
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
            // Trim trailing whitespace from the token.
            let mut end = i;
            while end > start && matches!(bytes[end - 1], b' ' | b'\t') {
                end -= 1;
            }
            // A scan that stopped at a `: ` separator marks a mapping key.
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

    (tokens, end_state(pending_block))
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

    fn has(line: &str, kind: TokenKind, s: &str) -> bool {
        tok(line).iter().any(|(k, v)| *k == kind && v == s)
    }

    #[test]
    fn hash_in_scalar_is_not_a_comment() {
        assert!(!has("url: http://x#frag", TokenKind::Comment, "#frag"));
        assert!(
            !tok("url: http://x#frag")
                .iter()
                .any(|(k, _)| *k == TokenKind::Comment)
        );
    }

    #[test]
    fn indicators_and_markers() {
        assert!(has("body: |", TokenKind::Operator, "|"));
        assert!(has("---", TokenKind::Keyword, "---"));
        assert!(has("--- !tag", TokenKind::Keyword, "---"));
        assert!(tok("--- !tag").iter().any(|(_, s)| s.contains("tag")));
    }

    #[test]
    fn key_value_and_nesting() {
        // `name` / `port` are mapping keys → Attribute; values keep their role.
        assert!(has("name: hello", TokenKind::Attribute, "name"));
        assert!(has("name: hello", TokenKind::Operator, ":"));
        assert!(has("name: hello", TokenKind::Identifier, "hello"));
        assert!(has("  port: 8080", TokenKind::Attribute, "port"));
        assert!(has("  port: 8080", TokenKind::Number, "8080"));
    }

    #[test]
    fn bare_scalar_value_is_identifier() {
        assert!(has("- plain_value", TokenKind::Identifier, "plain_value"));
        assert!(
            !tok("- plain_value")
                .iter()
                .any(|t| t.0 == TokenKind::Attribute)
        );
        assert!(has("  - item", TokenKind::Punctuation, "-"));
    }

    #[test]
    fn comment_line() {
        assert_eq!(tok("# this is a comment")[0].0, TokenKind::Comment);
    }

    #[test]
    fn keywords_anchor_tag() {
        assert!(has("enabled: true", TokenKind::Keyword, "true"));
        assert!(has("base: &default", TokenKind::MacroCall, "&default"));
        assert!(has(
            "timestamp: !!timestamp 2024-01-01",
            TokenKind::TypeName,
            "!!timestamp"
        ));
    }

    #[test]
    fn unterminated_quoted_no_panic() {
        assert!(
            tok(r#"key: "unclosed"#)
                .iter()
                .any(|t| t.0 == TokenKind::String)
        );
    }

    /// YAML 1.1 numbers: decimal, hex/octal/binary radix, `_` separators and
    /// floats. The literal ends at whitespace or structural punctuation.
    #[test]
    fn radix_and_underscore_separators() {
        for (line, want) in [
            ("a: 0xDEAD_BEEF", "0xDEAD_BEEF"),
            ("a: 0o755", "0o755"),
            ("a: 0b1010", "0b1010"),
            ("a: 1_000_000", "1_000_000"),
            ("a: 3.14", "3.14"),
        ] {
            assert!(has(line, TokenKind::Number, want), "input {line:?}");
        }
    }

    /// `2:30` is a bare string, not a number followed by a colon.
    #[test]
    fn bare_string_not_number() {
        assert!(!tok("time: 2:30").iter().any(|t| t.0 == TokenKind::Number));
    }

    /// A single-quoted scalar treats a doubled `''` as one escaped quote:
    /// `'it''s'` is a *single* string token, not two.
    #[test]
    fn single_quote_doubled_escape() {
        let strs: Vec<_> = tok("v: 'it''s'")
            .into_iter()
            .filter(|t| t.0 == TokenKind::String)
            .collect();
        assert_eq!(strs.len(), 1, "got {strs:?}");
        assert_eq!(strs[0].1, "'it''s'");
    }

    /// A `key: |` opens a block scalar whose body is carried across lines via
    /// `LineState::YamlBlock`: indented / blank lines stay `String`; a dedent
    /// exits back to `Code`.
    #[test]
    fn block_scalar_carry_state() {
        let (_, st) = tokenize_line("body: |", &Language::Yaml, LineState::Code);
        assert_eq!(st, LineState::YamlBlock { indent: 1 });

        // Indented body line stays a String and stays in the block.
        let (toks, st2) = tokenize_line("  hello world", &Language::Yaml, st);
        assert!(toks.iter().any(|t| t.kind == TokenKind::String));
        assert_eq!(st2, LineState::YamlBlock { indent: 1 });

        // Blank lines belong to the block.
        let (_, st3) = tokenize_line("", &Language::Yaml, st2);
        assert_eq!(st3, LineState::YamlBlock { indent: 1 });

        // A dedent (column 0) exits and re-tokenizes as ordinary YAML.
        let (toks4, st4) = tokenize_line("next: 1", &Language::Yaml, st3);
        assert_eq!(st4, LineState::Code);
        assert!(toks4.iter().any(|t| t.kind == TokenKind::Attribute));
    }

    /// The block-scalar body indent tracks the indicator line's indent, so a
    /// sibling key dedents out while a more-indented line stays in.
    #[test]
    fn block_scalar_indent_tracks_key() {
        let (_, st) = tokenize_line("  data: >-", &Language::Yaml, LineState::Code);
        assert_eq!(st, LineState::YamlBlock { indent: 3 });

        let (_, sibling) = tokenize_line("  other: x", &Language::Yaml, st);
        assert_eq!(sibling, LineState::Code);

        let (toks, deep) = tokenize_line("      line", &Language::Yaml, st);
        assert!(toks.iter().any(|t| t.kind == TokenKind::String));
        assert_eq!(deep, LineState::YamlBlock { indent: 3 });
    }
}
