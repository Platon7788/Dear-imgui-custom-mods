//! Unit tests for the Markdown tokenizer. Split out of the tokenizer modules
//! to keep every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn md(line: &str, state: LineState) -> (Vec<crate::code_editor::token::Token>, LineState) {
    tokenize_line(line, &Language::Markdown, state)
}

/// Spans must tile the line exactly (contiguous, on char boundaries).
fn assert_tiles(line: &str, toks: &[crate::code_editor::token::Token]) {
    let mut pos = 0usize;
    for t in toks {
        assert_eq!(t.start, pos, "non-contiguous span on {line:?}: {toks:?}");
        assert!(
            line.is_char_boundary(t.start) && line.is_char_boundary(t.start + t.len),
            "span off char boundary on {line:?}"
        );
        pos += t.len;
    }
    assert_eq!(pos, line.len(), "span total != len on {line:?}");
}

#[test]
fn fenced_block_carry() {
    // Opening fence → enters Fenced state.
    let (toks, st) = md("```rust", LineState::Code);
    assert_tiles("```rust", &toks);
    assert!(
        matches!(st, LineState::Fenced { fence: b'`', .. }),
        "opening ``` should enter Fenced, got {st:?}"
    );
    assert_eq!(toks[0].kind, TokenKind::Operator);

    // A body line that *looks* like a heading must stay plain code.
    let body = "# not a heading";
    let (btoks, bst) = md(body, st);
    assert_tiles(body, &btoks);
    assert!(
        matches!(bst, LineState::Fenced { .. }),
        "body line should stay Fenced, got {bst:?}"
    );
    assert!(
        btoks.iter().all(|t| t.kind != TokenKind::Keyword),
        "`#` inside a fence must NOT be a heading: {btoks:?}"
    );
    assert_eq!(btoks.len(), 1);
    assert_eq!(btoks[0].kind, TokenKind::String);

    // Closing fence → back to Code.
    let (ctoks, cst) = md("```", bst);
    assert_tiles("```", &ctoks);
    assert_eq!(cst, LineState::Code, "closing ``` should return Code");
    assert_eq!(ctoks[0].kind, TokenKind::Operator);
}

#[test]
fn tilde_fence_and_close() {
    let (_t, st) = md("~~~", LineState::Code);
    assert!(matches!(
        st,
        LineState::Fenced {
            fence: b'~',
            count: 3
        }
    ));
    // A `` ``` `` line does NOT close a `~~~` block (different fence char).
    let (_b, st2) = md("```", st);
    assert!(matches!(st2, LineState::Fenced { fence: b'~', .. }));
    let (_c, st3) = md("~~~~", st2);
    assert_eq!(st3, LineState::Code);
}

#[test]
fn atx_heading() {
    let line = "## Title here";
    let (toks, st) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert_eq!(st, LineState::Code);
    assert!(toks.iter().any(|t| t.kind == TokenKind::Operator)); // the `##`
    assert!(toks.iter().any(|t| t.kind == TokenKind::Keyword)); // the text
    // `#tag` (no space) is NOT a heading.
    let (t2, _) = md("#tag", LineState::Code);
    assert!(t2.iter().all(|t| t.kind != TokenKind::Keyword));
}

#[test]
fn inline_code_and_bold() {
    let line = "use `code` and **bold** here";
    let (toks, _) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    let strings: Vec<&str> = toks
        .iter()
        .filter(|t| t.kind == TokenKind::String)
        .map(|t| &line[t.start..t.start + t.len])
        .collect();
    assert!(strings.contains(&"`code`"), "inline code span: {strings:?}");
    assert!(strings.contains(&"**bold**"), "bold span: {strings:?}");
}

#[test]
fn link_colours() {
    let line = "see [text](http://x) now";
    let (toks, _) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert!(
        toks.iter()
            .any(|t| t.kind == TokenKind::Identifier && &line[t.start..t.start + t.len] == "text"),
        "link text should be Identifier: {toks:?}"
    );
    assert!(
        toks.iter()
            .any(|t| t.kind == TokenKind::String && &line[t.start..t.start + t.len] == "http://x"),
        "link url should be String: {toks:?}"
    );
}

#[test]
fn image_and_list_and_quote() {
    // Image alt vs url.
    let img = "![alt](pic.png)";
    let (it, _) = md(img, LineState::Code);
    assert_tiles(img, &it);
    assert!(
        it.iter()
            .any(|t| t.kind == TokenKind::String && &img[t.start..t.start + t.len] == "pic.png")
    );

    // Bullet list marker.
    let (lt, _) = md("- item one", LineState::Code);
    assert!(lt.iter().any(|t| t.kind == TokenKind::Operator));

    // Ordered list marker.
    let ol = "3. third";
    let (ot, _) = md(ol, LineState::Code);
    assert_tiles(ol, &ot);
    assert!(ot.iter().any(|t| t.kind == TokenKind::Number));

    // Blockquote.
    let (qt, _) = md("> quoted", LineState::Code);
    assert_eq!(qt[0].kind, TokenKind::Comment);
}

#[test]
fn thematic_break() {
    for hr in ["---", "***", "___", "- - -"] {
        let (toks, st) = md(hr, LineState::Code);
        assert_tiles(hr, &toks);
        assert_eq!(st, LineState::Code);
        assert!(
            toks.iter().any(|t| t.kind == TokenKind::Operator),
            "{hr:?} should be a horizontal rule: {toks:?}"
        );
    }
}

#[test]
fn from_extension_markdown() {
    assert_eq!(Language::from_extension("md"), Some(Language::Markdown));
    assert_eq!(
        Language::from_extension("markdown"),
        Some(Language::Markdown)
    );
    assert_eq!(Language::from_extension("mkd"), Some(Language::Markdown));
    assert_eq!(Language::from_extension(".MDOWN"), Some(Language::Markdown));
    assert_eq!(Language::from_path("README.md"), Some(Language::Markdown));
}

#[test]
fn escapes_are_literal() {
    let line = r"\*not italic\* and \\ backslash";
    let (toks, _) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    // No emphasis String tokens — the `*` are escaped.
    assert!(toks.iter().all(|t| t.kind != TokenKind::String));
}

#[test]
fn indented_code_block() {
    // 4+ leading spaces → the whole remainder is code, never inline emphasis.
    let line = "    *not italic*";
    let (toks, st) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert_eq!(st, LineState::Code);
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].kind, TokenKind::Whitespace);
    assert_eq!(toks[1].kind, TokenKind::String);
    assert_eq!(
        &line[toks[1].start..toks[1].start + toks[1].len],
        "*not italic*",
        "the `*` must NOT open an italic span in indented code"
    );
    // A single leading tab counts as >= 4 columns.
    let tabbed = "\tcode();";
    let (tt, _) = md(tabbed, LineState::Code);
    assert_tiles(tabbed, &tt);
    assert!(tt.iter().any(|t| t.kind == TokenKind::String));
    // 3 leading spaces is NOT enough — still an ordinary paragraph.
    let three = "   *italic*";
    let (t3, _) = md(three, LineState::Code);
    assert_tiles(three, &t3);
    assert!(
        t3.iter().any(|t| t.kind == TokenKind::String),
        "3-space indent should still tokenize inline emphasis: {t3:?}"
    );
}

#[test]
fn table_row_pipes_and_cells() {
    let line = "| a | b |";
    let (toks, st) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert_eq!(st, LineState::Code);
    // Each `|` is a Punctuation token.
    let pipes: Vec<&str> = toks
        .iter()
        .filter(|t| t.kind == TokenKind::Punctuation)
        .map(|t| &line[t.start..t.start + t.len])
        .collect();
    assert_eq!(pipes.len(), 3, "each `|` is Punctuation: {toks:?}");
    assert!(pipes.iter().all(|p| *p == "|"));
    // Cell text goes through the inline tokenizer.
    assert!(toks.iter().any(|t| t.kind == TokenKind::Identifier));
}

#[test]
fn table_delimiter_row() {
    let line = "|---|:--:|";
    let (toks, st) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert_eq!(st, LineState::Code);
    // Dash/colon runs colour as Operator, pipes as Punctuation.
    assert!(
        toks.iter().any(|t| t.kind == TokenKind::Operator),
        "delimiter dashes/colons should be Operator: {toks:?}"
    );
    assert_eq!(
        toks.iter()
            .filter(|t| t.kind == TokenKind::Punctuation)
            .count(),
        3
    );
    // A delimiter row has no inline emphasis spans.
    assert!(toks.iter().all(|t| t.kind != TokenKind::String));
}

#[test]
fn fenced_body_indent_and_pipe_stay_code() {
    let (_o, st) = md("```", LineState::Code);
    assert!(matches!(st, LineState::Fenced { .. }));
    // A 4-space-indented body line stays fenced code (one String token) —
    // the new indented-code rule must NOT fire inside a fence.
    let indented = "    still code";
    let (it, ist) = md(indented, st);
    assert_tiles(indented, &it);
    assert!(
        matches!(ist, LineState::Fenced { .. }),
        "indent inside a fence must stay fenced, got {ist:?}"
    );
    assert_eq!(it.len(), 1);
    assert_eq!(it[0].kind, TokenKind::String);
    // A body line containing a pipe stays fenced code (not a GFM table).
    let piped = "| not | a table |";
    let (pt, pst) = md(piped, ist);
    assert_tiles(piped, &pt);
    assert!(
        matches!(pst, LineState::Fenced { .. }),
        "pipe inside a fence must stay fenced, got {pst:?}"
    );
    assert_eq!(pt.len(), 1);
    assert_eq!(pt[0].kind, TokenKind::String);
}

#[test]
fn paragraph_with_stray_pipe_tiles() {
    let line = "a | b or c";
    let (toks, st) = md(line, LineState::Code);
    assert_tiles(line, &toks);
    assert_eq!(st, LineState::Code);
    // The lone `|` is Punctuation; the surrounding text is inline.
    assert!(
        toks.iter()
            .any(|t| t.kind == TokenKind::Punctuation && &line[t.start..t.start + t.len] == "|"),
        "stray pipe should be Punctuation: {toks:?}"
    );
    assert!(toks.iter().any(|t| t.kind == TokenKind::Identifier));
}
