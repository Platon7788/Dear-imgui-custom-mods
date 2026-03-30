//! Hex byte stream tokenizer.
//!
//! Format: space-separated byte pairs (`XX`), e.g. `DE AD BE EF`.
//! `//` line comments are supported.
//!
//! Token mapping:
//! - `[0-9A-Fa-f]{2}` → value-based kinds ([`TokenKind::HexNull`] etc.)
//! - Lone hex nibble → [`TokenKind::Attribute`] (amber warning)
//! - Non-hex chars → [`TokenKind::Operator`] (error marker)

use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

// ── Language definition ─────────────────────────────────────────────────────

pub struct HexLang;

impl SyntaxDefinition for HexLang {
    fn name(&self) -> &str { "Hex" }

    fn tokenize_line(&self, line: &str, _in_block_comment: bool) -> (Vec<Token>, bool) {
        (tokenize(line), false)
    }

    fn line_comment_prefix(&self) -> Option<&str> { Some("//") }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> { None }
    fn bracket_pairs(&self) -> &[(char, char)] { &[] }
    fn auto_indent_after(&self) -> &[char] { &[] }
    fn auto_dedent_on(&self) -> &[char] { &[] }
    fn auto_close_pairs(&self) -> &[(&str, &str)] { &[] }

    fn is_word_char(&self, c: char) -> bool {
        c.is_ascii_hexdigit() || c == ' '
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

#[inline]
fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(len / 3 + 4);
    let mut i = 0;

    while i < len {
        // ── Line comment ─────────────────────────────────────────────────
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            tokens.push(Token { kind: TokenKind::Comment, start: i, len: len - i });
            return tokens;
        }

        // ── Whitespace ───────────────────────────────────────────────────
        if bytes[i] == b' ' || bytes[i] == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
            tokens.push(Token { kind: TokenKind::Whitespace, start, len: i - start });
            continue;
        }

        // ── Hex byte (1 or 2 nibbles) ────────────────────────────────────
        if bytes[i].is_ascii_hexdigit() {
            let start = i;
            let mut nibbles = 0u8;
            while i < len && bytes[i].is_ascii_hexdigit() && nibbles < 2 {
                i += 1;
                nibbles += 1;
            }
            let kind = if nibbles == 2 {
                let hi = hex_nibble(bytes[start]);
                let lo = hex_nibble(bytes[start + 1]);
                let val = (hi << 4) | lo;
                match val {
                    0x00       => TokenKind::HexNull,
                    0xFF       => TokenKind::HexFF,
                    0x20..=0x7E => TokenKind::HexPrintable,
                    _          => TokenKind::HexDefault,
                }
            } else {
                TokenKind::Attribute // lone nibble — amber warning
            };
            tokens.push(Token { kind, start, len: i - start });
            continue;
        }

        // ── Invalid character ────────────────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token { kind: TokenKind::Operator, start: i, len: ch_len });
        i += ch_len;
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
        let (tokens, _) = tokenize_line(line, &Language::Hex, false);
        tokens.iter().map(|t| (t.kind, line[t.start..t.start + t.len].to_string())).collect()
    }

    #[test]
    fn byte_values() {
        let toks = tok("00 41 FF");
        assert_eq!(toks[0].0, TokenKind::HexNull);      // 00
        assert_eq!(toks[2].0, TokenKind::HexPrintable);  // 41 = 'A'
        assert_eq!(toks[4].0, TokenKind::HexFF);         // FF
    }

    #[test]
    fn lone_nibble() {
        let toks = tok("A ");
        assert_eq!(toks[0].0, TokenKind::Attribute); // amber warning
    }

    #[test]
    fn comment() {
        let toks = tok("DE AD // header");
        assert!(toks.last().unwrap().0 == TokenKind::Comment);
    }

    #[test]
    fn invalid_chars() {
        let toks = tok("GG");
        // 'G' is not valid hex
        assert!(toks.iter().any(|t| t.0 == TokenKind::Operator));
    }
}
