//! Unit tests for the XML / HTML tokenizer. Split out of `mod.rs` to keep
//! every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::Language;
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, String)> {
    let (tokens, _) = tokenize_line(line, &Language::Xml, false);
    tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect()
}

#[test]
fn tag_with_attributes() {
    let toks = tok(r#"<div class="main">"#);
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "div")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "class")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == r#""main""#)
    );
}

#[test]
fn self_closing() {
    let toks = tok("<br/>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "br")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Punctuation && t.1 == "/>")
    );
}

#[test]
fn closing_tag() {
    let toks = tok("</div>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Punctuation && t.1 == "</")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "div")
    );
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

/// Multi-byte text immediately before a `<![CDATA[` marker must not
/// trigger a non-char-boundary slicing panic (the marker detection is
/// byte-based).
#[test]
fn cdata_after_multibyte_no_panic() {
    // `你` is 3 bytes; ensure the CDATA scan doesn't panic on the window.
    let toks = tok("你好<![CDATA[x]]>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1.starts_with("<![CDATA["))
    );
}

/// A `<` near EOL that looks like the start of `<![CDATA[` but is
/// truncated must not panic.
#[test]
fn truncated_cdata_marker_no_panic() {
    let _ = tok("<![CDA");
    let _ = tok("<![CDATA");
}

/// Unterminated tag / attribute value runs to EOL without panic.
#[test]
fn unterminated_tag_no_panic() {
    let _ = tok("<div class=\"unclosed");
    let _ = tok("<!-- unclosed comment");
}

#[test]
fn mixed_content() {
    let toks = tok("Hello &amp; <b>world</b>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Identifier && t.1 == "Hello ")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::MacroCall && t.1 == "&amp;")
    );
    assert!(toks.iter().any(|t| t.0 == TokenKind::Keyword && t.1 == "b"));
}
