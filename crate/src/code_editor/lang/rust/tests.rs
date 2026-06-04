//! Unit tests for the Rust tokenizer. Split out of `tokenize.rs` to keep
//! every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::Language;
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, &str)> {
    let (tokens, _) = tokenize_line(line, &Language::Rust, false);
    tokens
        .iter()
        .map(|t| (t.kind, &line[t.start..t.start + t.len]))
        .collect()
}

#[test]
fn keywords() {
    let toks = tok("fn main() {");
    assert_eq!(toks[0], (TokenKind::Keyword, "fn"));
    assert_eq!(toks[2], (TokenKind::Identifier, "main"));
}

#[test]
fn strings() {
    let toks = tok(r#"let s = "hello \"world\"";"#);
    let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
    assert_eq!(strings.len(), 1);
}

#[test]
fn line_comment() {
    let toks = tok("let x = 5; // comment");
    let last = toks.last().unwrap();
    assert_eq!(last.0, TokenKind::Comment);
    assert!(last.1.contains("comment"));
}

#[test]
fn block_comment() {
    let (toks, still_in) = tokenize_line("/* start", &Language::Rust, false);
    assert!(still_in);
    assert_eq!(toks[0].kind, TokenKind::Comment);

    let (toks2, still_in2) = tokenize_line("middle */code", &Language::Rust, true);
    assert!(!still_in2);
    assert_eq!(toks2[0].kind, TokenKind::Comment);
}

#[test]
fn block_comment_single_line() {
    let (toks, still_in) = tokenize_line("a /* mid */ b", &Language::Rust, false);
    assert!(!still_in);
    // Should be: ident, ws, comment, ws, ident
    assert!(toks.iter().any(|t| t.kind == TokenKind::Comment));
}

/// Rust supports **nested** block comments: `/* /* */ */` closes only
/// the outer one at the second `*/`. A single-line nest must close
/// fully (still_in == false), and one extra unbalanced `/*` must carry
/// over (still_in == true).
#[test]
fn nested_block_comment_single_line() {
    let (_, still_in) = tokenize_line("/* a /* b */ c */", &Language::Rust, false);
    assert!(!still_in, "balanced nested comment should close");

    let (_, still_in2) = tokenize_line("/* a /* b */", &Language::Rust, false);
    assert!(still_in2, "one level still open after inner close");
}

/// Nested comment carrying across lines. Within a single line nesting
/// is exact; across lines the editor's carry state is a single `bool`,
/// so a depth-2 open collapses to "in comment" and the first `*/` on
/// the resume line closes it (documented limitation). What matters for
/// correctness: line 1 still reports "in comment", and the resume line
/// keeps the prefix up to the first `*/` colored as Comment.
#[test]
fn nested_block_comment_multi_line() {
    // Line 1 opens depth 2 → still in a comment.
    let (_, in1) = tokenize_line("/* outer /* inner", &Language::Rust, false);
    assert!(in1);
    // Resume line: prefix up to (and including) the first `*/` is a
    // single Comment token; the remainder re-tokenizes as code.
    let (toks2, still) = tokenize_line("body */ rest", &Language::Rust, true);
    assert_eq!(toks2[0].kind, TokenKind::Comment);
    assert_eq!(&"body */ rest"[..toks2[0].len], "body */");
    assert!(!still, "single `*/` closes the bool-carried comment");
}

/// A single-line nest that stays open by exactly one level carries to
/// the next line and a single `*/` there closes it.
#[test]
fn nested_block_comment_one_level_carryover() {
    let (_, in1) = tokenize_line("code /* a /* b */", &Language::Rust, false);
    assert!(in1, "one inner level still open");
    let (_, in2) = tokenize_line("still */ done", &Language::Rust, true);
    assert!(!in2);
}

#[test]
fn numbers() {
    let toks = tok("let x = 0xFF_u8;");
    let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].1, "0xFF_u8");
}

#[test]
fn lifetime() {
    let toks = tok("fn foo<'a>(x: &'a str)");
    let lifetimes: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Lifetime).collect();
    assert_eq!(lifetimes.len(), 2);
    assert_eq!(lifetimes[0].1, "'a");
}

#[test]
fn macro_call() {
    let toks = tok("println!(\"hi\");");
    assert_eq!(toks[0], (TokenKind::MacroCall, "println!"));
}

/// `!=` must NOT be swallowed into a MacroCall — `a != b` is the
/// not-equal operator, not a macro named `a`.
#[test]
fn not_equal_is_not_macro() {
    let toks = tok("a != b");
    assert!(
        !toks.iter().any(|t| t.0 == TokenKind::MacroCall),
        "`!=` must not become a macro call: {toks:?}"
    );
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Operator && t.1 == "!=")
    );
}

#[test]
fn macro_rules_is_macro_call() {
    let toks = tok("macro_rules! foo {");
    assert_eq!(toks[0], (TokenKind::MacroCall, "macro_rules!"));
}

#[test]
fn attribute() {
    let toks = tok("#[derive(Debug)]");
    assert_eq!(toks[0].0, TokenKind::Attribute);
}

#[test]
fn inner_attribute() {
    let toks = tok("#![allow(dead_code)]");
    assert_eq!(toks[0].0, TokenKind::Attribute);
    assert_eq!(toks[0].1, "#![allow(dead_code)]");
}

#[test]
fn type_name() {
    let toks = tok("let v: Vec<String> = Vec::new();");
    let types: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::TypeName).collect();
    assert!(types.len() >= 2);
}

#[test]
fn union_keyword() {
    let toks = tok("union MyUnion {");
    assert_eq!(toks[0], (TokenKind::Keyword, "union"));
}

#[test]
fn user_code_marker() {
    let toks = tok("    // USER CODE BEGIN on_click");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, TokenKind::UserCodeMarker);
}

#[test]
fn raw_string() {
    let toks = tok(r###"let s = r#"hello"#;"###);
    let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
    assert_eq!(strings.len(), 1);
}

#[test]
fn byte_string_and_byte_char() {
    let toks = tok(r#"let b = b"bytes";"#);
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::String && t.1 == r#"b"bytes""#)
    );

    let toks2 = tok(r"let c = b'A';");
    assert!(
        toks2
            .iter()
            .any(|t| t.0 == TokenKind::CharLit && t.1 == "b'A'")
    );

    let toks3 = tok(r##"let r = br#"raw"#;"##);
    assert!(
        toks3
            .iter()
            .any(|t| t.0 == TokenKind::String && t.1.starts_with("br#"))
    );
}

/// `b` alone (not a byte-string prefix) is an ordinary identifier.
#[test]
fn b_identifier_not_eaten() {
    let toks = tok("let b = 5;");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::Identifier && t.1 == "b")
    );
}

/// Multi-byte char literals must classify as a single `CharLit`,
/// not fragment into the fallback bucket.
#[test]
fn char_literal_unicode() {
    for (input, want_lit) in [
        ("'a'", "'a'"),
        ("'é'", "'é'"),
        ("'你'", "'你'"),
        ("'😀'", "'😀'"),
        (r"'\n'", r"'\n'"),
        (r"'\\'", r"'\\'"),
        (r"'\''", r"'\''"),
        (r"'\x41'", r"'\x41'"),
        (r"'\u{1F600}'", r"'\u{1F600}'"),
    ] {
        let toks = tok(input);
        let chars: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::CharLit).collect();
        assert_eq!(chars.len(), 1, "input {input:?} produced {toks:?}");
        assert_eq!(chars[0].1, want_lit);
    }
}

/// Unterminated char (`'a`) must NOT classify as CharLit — the
/// lifetime branch picks it up afterwards.
#[test]
fn unterminated_char_falls_through_to_lifetime() {
    let toks = tok("fn foo<'a>(x: &'a T)");
    let lifetimes: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Lifetime).collect();
    assert_eq!(lifetimes.len(), 2);
    let chars: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::CharLit).collect();
    assert!(chars.is_empty());
}

/// Unterminated string runs to EOL without panicking and produces a
/// single String token covering the rest of the line.
#[test]
fn unterminated_string_no_panic() {
    let toks = tok(r#"let s = "no close"#);
    let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings[0].1, r#""no close"#);
}

/// Token spans must cover the entire line with no gaps or overlaps,
/// even with tricky escapes and nested comments.
#[test]
fn covers_full_line() {
    for line in [
        r#"pub fn foo(x: i32) -> bool { true }"#,
        r#"let s = "a\tb"; /* /* x */ */ y"#,
        "#[derive(Clone, Debug)]",
        "let 你好 = 42; // 注释",
    ] {
        let (toks, _) = tokenize_line(line, &Language::Rust, false);
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len(), "span mismatch for {line:?}");
        // Verify contiguity.
        let mut pos = 0;
        for t in &toks {
            assert_eq!(t.start, pos, "gap before token in {line:?}");
            pos += t.len;
        }
    }
}
