//! Unit tests for the XML / HTML tokenizer. Split out of `mod.rs` to keep
//! every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, String)> {
    let (tokens, _) = tokenize_line(line, &Language::Xml, LineState::Code);
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
    let (_, still_in) = tokenize_line("<!-- start", &Language::Xml, LineState::Code);
    assert_eq!(still_in, LineState::BlockComment(1));
    let (toks, done) = tokenize_line("end --> text", &Language::Xml, LineState::BlockComment(1));
    assert_eq!(done, LineState::Code);
    assert_eq!(toks[0].kind, TokenKind::Comment);
}

#[test]
fn comment_single_line() {
    let (toks, bc) = tokenize_line("<!-- full comment -->", &Language::Xml, LineState::Code);
    assert_eq!(bc, LineState::Code);
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

// ── <script> / <style> raw-text bodies ──────────────────────────────────────

/// Tokenize `line` starting from `state`, returning `(kind, text)` pairs plus
/// the carry-state at end of line.
fn tok_state(line: &str, state: LineState) -> (Vec<(TokenKind, String)>, LineState) {
    let (tokens, end) = tokenize_line(line, &Language::Xml, state);
    let pairs = tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect();
    (pairs, end)
}

#[test]
fn script_open_enters_raw_text() {
    let (toks, state) = tok_state("<script>", LineState::Code);
    assert_eq!(state, LineState::HtmlRaw { is_style: false });
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "script")
    );
}

#[test]
fn style_open_enters_raw_text() {
    let (_toks, state) = tok_state("<style>", LineState::Code);
    assert_eq!(state, LineState::HtmlRaw { is_style: true });
}

#[test]
fn script_with_attrs_enters_raw_text() {
    let (_toks, state) = tok_state(r#"<script type="text/javascript">"#, LineState::Code);
    assert_eq!(state, LineState::HtmlRaw { is_style: false });
}

/// Self-closed `<script/>` must NOT enter raw-text mode.
#[test]
fn self_closed_script_stays_code() {
    let (_toks, state) = tok_state("<script/>", LineState::Code);
    assert_eq!(state, LineState::Code);
}

/// A `<` in a raw-text body (e.g. `a < b` in JS) must stay raw text — it does
/// NOT start a tag — and the state stays `HtmlRaw`.
#[test]
fn raw_body_lt_stays_string() {
    let entry = LineState::HtmlRaw { is_style: false };
    let (toks, state) = tok_state("  if (a < b) return;", entry);
    assert_eq!(state, entry, "no close tag → still raw");
    assert!(
        toks.iter().all(|t| t.0 == TokenKind::String),
        "raw body must be all String, got {toks:?}"
    );
    // The whole line is one raw-text run (a `<` did not split it).
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].1, "  if (a < b) return;");
}

/// The matching `</script>` closes the raw-text body: the close tag is markup
/// again and the state returns to `Code`.
#[test]
fn raw_body_close_returns_code() {
    let entry = LineState::HtmlRaw { is_style: false };
    let (toks, state) = tok_state("var x = 1;</script>", entry);
    assert_eq!(state, LineState::Code);
    // Body before the close is raw String.
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == "var x = 1;")
    );
    // The close tag tokenizes as markup.
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Punctuation && t.1 == "</")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Keyword && t.1 == "script")
    );
}

/// A `</style>` close is matched case-insensitively.
#[test]
fn raw_style_close_case_insensitive() {
    let entry = LineState::HtmlRaw { is_style: true };
    let (_toks, state) = tok_state("body { color: red; }</STYLE>", entry);
    assert_eq!(state, LineState::Code);
}

/// A `</script>` while in a `<style>` body must NOT close it (mismatched).
#[test]
fn raw_style_ignores_script_close() {
    let entry = LineState::HtmlRaw { is_style: true };
    let (_toks, state) = tok_state("a < b </script> c", entry);
    assert_eq!(state, entry, "mismatched close tag keeps style raw");
}

/// Whole `<script>…</script>` on one line opens and closes, ending in `Code`.
#[test]
fn script_open_and_close_same_line() {
    let (toks, state) = tok_state("<script>a<b;</script>", LineState::Code);
    assert_eq!(state, LineState::Code);
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == "a<b;"),
        "raw body between tags is String, got {toks:?}"
    );
}

// ── Unquoted attribute values ───────────────────────────────────────────────

#[test]
fn unquoted_attr_value_is_string() {
    let toks = tok("<input type=text>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "type"),
        "attribute name should be TypeName"
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == "text"),
        "unquoted value should be String, got {toks:?}"
    );
}

#[test]
fn unquoted_attr_value_with_spaces_around_eq() {
    let toks = tok("<input type = text >");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == "text")
    );
}

#[test]
fn unquoted_attr_then_self_close() {
    let toks = tok("<input type=text/>");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == "text")
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Punctuation && t.1 == "/>")
    );
}

// ── Entities inside quoted attribute values ─────────────────────────────────

#[test]
fn entity_in_quoted_attr_value() {
    let toks = tok(r#"<a href="a&amp;b">"#);
    // The entity colours distinctly from the surrounding String segments.
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::MacroCall && t.1 == "&amp;"),
        "entity inside attr value should be MacroCall, got {toks:?}"
    );
    // Plain quoted value with no entity stays a single String.
    let plain = tok(r#"<a href="ab">"#);
    assert!(
        plain
            .iter()
            .any(|t| t.0 == TokenKind::String && t.1 == r#""ab""#)
    );
}
