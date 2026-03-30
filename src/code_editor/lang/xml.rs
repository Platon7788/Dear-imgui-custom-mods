//! XML / HTML tokenizer.
//!
//! Tag names → [`TokenKind::Keyword`], attribute names → [`TokenKind::TypeName`],
//! attribute values → [`TokenKind::String`], entity references → [`TokenKind::MacroCall`],
//! processing instructions → [`TokenKind::Attribute`].
//! Multi-line `<!-- -->` comments are tracked via `in_block_comment`.

use super::{is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

// ── Language definition ─────────────────────────────────────────────────────

pub struct XmlLang;

impl SyntaxDefinition for XmlLang {
    fn name(&self) -> &str { "XML" }

    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool) {
        tokenize(line, in_block_comment)
    }

    /// XML has no single-line comment syntax.
    fn line_comment_prefix(&self) -> Option<&str> { None }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> { Some(("<!--", "-->")) }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']'), ('<', '>')]
    }

    fn auto_indent_after(&self) -> &[char] { &['>'] }
    fn auto_dedent_on(&self) -> &[char] { &[] }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'"), ("<", ">")]
    }

    fn is_word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '.'
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str, mut in_block_comment: bool) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    while i < len {
        // ── Inside XML comment <!-- ... --> ───────────────────────────────
        if in_block_comment {
            let start = i;
            loop {
                if i + 2 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
                    i += 3;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
            tokens.push(Token { kind: TokenKind::Whitespace, start, len: i - start });
            continue;
        }

        // ── Comment start <!-- ───────────────────────────────────────────
        if b == b'<' && i + 3 < len
            && bytes[i + 1] == b'!' && bytes[i + 2] == b'-' && bytes[i + 3] == b'-'
        {
            let start = i;
            i += 4;
            in_block_comment = true;
            loop {
                if i + 2 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
                    i += 3;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        // ── CDATA <![CDATA[...]]> ────────────────────────────────────────
        if b == b'<' && i + 8 < len && &line[i..i + 9] == "<![CDATA[" {
            let start = i;
            i += 9;
            loop {
                if i + 2 < len && bytes[i] == b']' && bytes[i + 1] == b']' && bytes[i + 2] == b'>' {
                    i += 3;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::String, start, len: i - start });
            continue;
        }

        // ── Processing instruction <?...?> ───────────────────────────────
        if b == b'<' && i + 1 < len && bytes[i + 1] == b'?' {
            let start = i;
            i += 2;
            loop {
                if i + 1 < len && bytes[i] == b'?' && bytes[i + 1] == b'>' {
                    i += 2;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Attribute, start, len: i - start });
            continue;
        }

        // ── DOCTYPE / other declarations <!...> ──────────────────────────
        if b == b'<' && i + 1 < len && bytes[i + 1] == b'!' {
            let start = i;
            let mut depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth = depth.saturating_sub(1);
                        i += 1;
                        if depth == 0 { break; }
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
            tokens.push(Token { kind: TokenKind::Attribute, start, len: i - start });
            continue;
        }

        // ── Tag (open, close, self-closing) ──────────────────────────────
        if b == b'<' {
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'/' { i += 1; }
            tokens.push(Token { kind: TokenKind::Punctuation, start, len: i - start });

            // Tag name
            if i < len && (is_ident_start(bytes[i]) || bytes[i] == b':') {
                let name_start = i;
                while i < len && (is_ident_continue(bytes[i])
                    || bytes[i] == b'-' || bytes[i] == b':' || bytes[i] == b'.') {
                    i += 1;
                }
                tokens.push(Token { kind: TokenKind::Keyword, start: name_start, len: i - name_start });
            }

            // Attributes
            while i < len && bytes[i] != b'>' {
                if bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' {
                    let ws = i;
                    while i < len && matches!(bytes[i], b' ' | b'\t' | b'\n') { i += 1; }
                    tokens.push(Token { kind: TokenKind::Whitespace, start: ws, len: i - ws });
                    continue;
                }

                // Self-close `/>`
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>' {
                    tokens.push(Token { kind: TokenKind::Punctuation, start: i, len: 2 });
                    i += 2;
                    break;
                }

                // Attribute name
                if is_ident_start(bytes[i]) || bytes[i] == b':' {
                    let attr_start = i;
                    while i < len && (is_ident_continue(bytes[i])
                        || bytes[i] == b'-' || bytes[i] == b':') {
                        i += 1;
                    }
                    tokens.push(Token { kind: TokenKind::TypeName, start: attr_start, len: i - attr_start });
                    continue;
                }

                // `=`
                if bytes[i] == b'=' {
                    tokens.push(Token { kind: TokenKind::Operator, start: i, len: 1 });
                    i += 1;
                    continue;
                }

                // Attribute value (quoted)
                if bytes[i] == b'"' || bytes[i] == b'\'' {
                    let quote = bytes[i];
                    let val_start = i;
                    i += 1;
                    while i < len && bytes[i] != quote { i += 1; }
                    if i < len { i += 1; }
                    tokens.push(Token { kind: TokenKind::String, start: val_start, len: i - val_start });
                    continue;
                }

                // Unknown char inside tag — skip
                let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
                tokens.push(Token { kind: TokenKind::Identifier, start: i, len: ch_len });
                i += ch_len;
            }

            // Closing `>`
            if i < len && bytes[i] == b'>' {
                tokens.push(Token { kind: TokenKind::Punctuation, start: i, len: 1 });
                i += 1;
            }
            continue;
        }

        // ── Entity reference (&amp; etc.) ────────────────────────────────
        if b == b'&' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b';' && bytes[i] != b' ' && bytes[i] != b'<' {
                i += 1;
            }
            if i < len && bytes[i] == b';' { i += 1; }
            tokens.push(Token { kind: TokenKind::MacroCall, start, len: i - start });
            continue;
        }

        // ── Text content ─────────────────────────────────────────────────
        {
            let start = i;
            while i < len && bytes[i] != b'<' && bytes[i] != b'&' { i += 1; }
            if i > start {
                tokens.push(Token { kind: TokenKind::Identifier, start, len: i - start });
            }
        }
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
        let (tokens, _) = tokenize_line(line, &Language::Xml, false);
        tokens.iter().map(|t| (t.kind, line[t.start..t.start + t.len].to_string())).collect()
    }

    #[test]
    fn tag_with_attributes() {
        let toks = tok(r#"<div class="main">"#);
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "div"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::TypeName && t.1 == "class"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::String && t.1 == r#""main""#));
    }

    #[test]
    fn self_closing() {
        let toks = tok("<br/>");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "br"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::Punctuation && t.1 == "/>"));
    }

    #[test]
    fn closing_tag() {
        let toks = tok("</div>");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Punctuation && t.1 == "</"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "div"));
    }

    #[test]
    fn comment_multiline() {
        let (_, still_in) = tokenize_line("<!-- start", &Language::Xml, false);
        assert!(still_in);
        let (toks, done) = tokenize_line("end --> text", &Language::Xml, true);
        assert!(!done);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn comment_single_line() {
        let (toks, bc) = tokenize_line("<!-- full comment -->", &Language::Xml, false);
        assert!(!bc);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn entity() {
        let toks = tok("&amp;");
        assert_eq!(toks[0].0, TokenKind::MacroCall);
        assert_eq!(toks[0].1, "&amp;");
    }

    #[test]
    fn processing_instruction() {
        let toks = tok(r#"<?xml version="1.0"?>"#);
        assert_eq!(toks[0].0, TokenKind::Attribute);
    }

    #[test]
    fn cdata() {
        let toks = tok("<![CDATA[some data]]>");
        assert_eq!(toks[0].0, TokenKind::String);
    }

    #[test]
    fn mixed_content() {
        let toks = tok("Hello &amp; <b>world</b>");
        assert!(toks.iter().any(|t| t.0 == TokenKind::Identifier && t.1 == "Hello "));
        assert!(toks.iter().any(|t| t.0 == TokenKind::MacroCall && t.1 == "&amp;"));
        assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "b"));
    }
}
