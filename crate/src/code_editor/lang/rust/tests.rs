//! Unit tests for the Rust tokenizer. Split out of `tokenize.rs` to keep
//! every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, &str)> {
    let (tokens, _) = tokenize_line(line, &Language::Rust, LineState::Code);
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
fn gen_keyword_2024() {
    assert_eq!(tok("gen fn g() {}")[0], (TokenKind::Keyword, "gen"));
}

#[test]
fn c_string_literals() {
    assert!(tok(r#"let s = c"hi";"#).contains(&(TokenKind::String, r#"c"hi""#)));
    assert!(
        tok(r##"let s = cr#"raw"#;"##)
            .iter()
            .any(|t| t.0 == TokenKind::String && t.1.starts_with("cr#\""))
    );
}

#[test]
fn doc_comments_are_distinct() {
    assert_eq!(tok("/// outer")[0], (TokenKind::DocComment, "/// outer"));
    assert_eq!(tok("//! inner")[0], (TokenKind::DocComment, "//! inner"));
    // Plain and 4-slash comments are NOT doc comments.
    assert_eq!(tok("// plain")[0].0, TokenKind::Comment);
    assert_eq!(tok("//// four")[0].0, TokenKind::Comment);
    // Block doc vs plain vs empty.
    assert_eq!(tok("/** doc */")[0].0, TokenKind::DocComment);
    assert_eq!(tok("/* plain */")[0].0, TokenKind::Comment);
    assert_eq!(tok("/**/")[0].0, TokenKind::Comment);
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
    let (toks, still_in) = tokenize_line("/* start", &Language::Rust, LineState::Code);
    assert_eq!(still_in, LineState::BlockComment(1));
    assert_eq!(toks[0].kind, TokenKind::Comment);

    let (toks2, still_in2) =
        tokenize_line("middle */code", &Language::Rust, LineState::BlockComment(1));
    assert_eq!(still_in2, LineState::Code);
    assert_eq!(toks2[0].kind, TokenKind::Comment);
}

#[test]
fn block_comment_single_line() {
    let (toks, still_in) = tokenize_line("a /* mid */ b", &Language::Rust, LineState::Code);
    assert_eq!(still_in, LineState::Code);
    // Should be: ident, ws, comment, ws, ident
    assert!(toks.iter().any(|t| t.kind == TokenKind::Comment));
}

/// Rust supports **nested** block comments: `/* /* */ */` closes only
/// the outer one at the second `*/`. A single-line nest must close
/// fully (still_in == false), and one extra unbalanced `/*` must carry
/// over (still_in == true).
#[test]
fn nested_block_comment_single_line() {
    let (_, still_in) = tokenize_line("/* a /* b */ c */", &Language::Rust, LineState::Code);
    assert_eq!(
        still_in,
        LineState::Code,
        "balanced nested comment should close"
    );

    let (_, still_in2) = tokenize_line("/* a /* b */", &Language::Rust, LineState::Code);
    assert_eq!(
        still_in2,
        LineState::BlockComment(1),
        "one level still open after inner close"
    );
}

/// Nested comment carrying across lines. The rich `LineState` carry threads
/// the exact open-comment depth from one line to the next, so a depth-2 open
/// stays open past the first `*/` — only the inner comment closes. Nested
/// multi-line block comments now behave correctly (they used to collapse to
/// depth 1 and close on the first `*/`).
#[test]
fn nested_block_comment_multi_line() {
    // Line 1 opens depth 2 → carries BlockComment(2).
    let (_, in1) = tokenize_line("/* outer /* inner", &Language::Rust, LineState::Code);
    assert_eq!(in1, LineState::BlockComment(2));
    // Resume at depth 2: the whole line stays a Comment and the first `*/`
    // closes only the inner comment, leaving depth 1 still open.
    let (toks2, still) = tokenize_line("body */ rest", &Language::Rust, in1);
    assert_eq!(toks2[0].kind, TokenKind::Comment);
    assert_eq!(
        toks2[0].len,
        "body */ rest".len(),
        "whole line stays comment"
    );
    assert_eq!(
        still,
        LineState::BlockComment(1),
        "outer comment still open after inner close"
    );
}

/// A single-line nest that stays open by exactly one level carries to
/// the next line and a single `*/` there closes it.
#[test]
fn nested_block_comment_one_level_carryover() {
    let (_, in1) = tokenize_line("code /* a /* b */", &Language::Rust, LineState::Code);
    assert_eq!(
        in1,
        LineState::BlockComment(1),
        "one inner level still open"
    );
    let (_, in2) = tokenize_line("still */ done", &Language::Rust, in1);
    assert_eq!(in2, LineState::Code);
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
        let (toks, _) = tokenize_line(line, &Language::Rust, LineState::Code);
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

// ── Multi-line string carry ──────────────────────────────────────────────────

/// Carry state for a plain `"`-delimited string.
const STR: LineState = LineState::Str {
    quote: b'"',
    raw: false,
    hashes: 0,
    triple: false,
};

/// A regular string that opens but doesn't close carries `Str` state; the
/// next line resumes from column 0 and closes it, then re-tokenizes the rest.
#[test]
fn multiline_regular_string_carries() {
    let (toks, st) = tokenize_line(r#"let s = "line one"#, &Language::Rust, LineState::Code);
    assert_eq!(st, STR, "unclosed string carries Str: {st:?}");
    assert!(toks.iter().any(|t| t.kind == TokenKind::String));

    let line2 = r#"line two"; let x = 1;"#;
    let (toks2, st2) = tokenize_line(line2, &Language::Rust, st);
    assert_eq!(st2, LineState::Code, "closing quote ends the string");
    assert_eq!(toks2[0].kind, TokenKind::String);
    assert_eq!(&line2[..toks2[0].len], r#"line two""#);
    // The remainder is ordinary code again.
    assert!(toks2.iter().any(|t| t.kind == TokenKind::Keyword));
}

/// A trailing backslash is a line-continuation — the string stays open.
#[test]
fn multiline_string_backslash_continuation() {
    let (_, st) = tokenize_line(r#"let s = "a b \"#, &Language::Rust, LineState::Code);
    assert_eq!(st, STR);
}

/// An escaped quote on the continuation line does not close the string.
#[test]
fn multiline_string_resume_escaped_quote() {
    let (_, st) = tokenize_line(r#"let s = "start"#, &Language::Rust, LineState::Code);
    let line2 = r#"esc \" still" end"#;
    let (toks2, st2) = tokenize_line(line2, &Language::Rust, st);
    assert_eq!(st2, LineState::Code);
    assert_eq!(&line2[..toks2[0].len], r#"esc \" still""#);
}

/// A raw string carries its hash count; the close needs `"` + N `#`.
#[test]
fn multiline_raw_string_carries_hashes() {
    let raw1 = LineState::Str {
        quote: b'"',
        raw: true,
        hashes: 1,
        triple: false,
    };
    let (_, st) = tokenize_line(
        r###"let s = r#"raw start"###,
        &Language::Rust,
        LineState::Code,
    );
    assert_eq!(st, raw1);
    // A lone `"` without the trailing `#` does NOT close it.
    let (_, st_open) = tokenize_line(r#"still " open"#, &Language::Rust, st);
    assert_eq!(
        st_open, raw1,
        "close needs `\"#`; a bare quote keeps it open"
    );
    // `"#` closes it.
    let line3 = r##"done"# rest"##;
    let (toks3, st3) = tokenize_line(line3, &Language::Rust, st);
    assert_eq!(st3, LineState::Code);
    assert_eq!(toks3[0].kind, TokenKind::String);
    assert_eq!(&line3[..toks3[0].len], r##"done"#"##);
}

/// Byte / c-string prefixes reuse the same multi-line close logic.
#[test]
fn multiline_byte_and_c_string_carry() {
    let (_, st) = tokenize_line(r#"let b = b"bytes"#, &Language::Rust, LineState::Code);
    assert_eq!(st, STR);
    let (_, st2) = tokenize_line(r#"let c = c"cstr"#, &Language::Rust, LineState::Code);
    assert_eq!(st2, STR);
    let (_, st3) = tokenize_line(r###"let r = br#"raw"###, &Language::Rust, LineState::Code);
    assert_eq!(
        st3,
        LineState::Str {
            quote: b'"',
            raw: true,
            hashes: 1,
            triple: false
        }
    );
}

/// A line beginning inside a string is never mistaken for a USER CODE marker.
#[test]
fn user_code_marker_ignored_inside_string() {
    let (toks, _) = tokenize_line("// USER CODE BEGIN foo", &Language::Rust, STR);
    assert!(toks.iter().all(|t| t.kind != TokenKind::UserCodeMarker));
    assert_eq!(toks[0].kind, TokenKind::String);
}

/// Span-tiling invariant with an incoming `Str` state and adversarial input.
#[test]
fn multiline_string_states_tile_exactly() {
    let raw1 = LineState::Str {
        quote: b'"',
        raw: true,
        hashes: 1,
        triple: false,
    };
    let cases: &[(&str, LineState)] = &[
        (r#""opens no close"#, LineState::Code),
        ("resume plain\" then code", STR),
        ("", STR),
        ("你好 multibyte\" tail", STR),
        (r#"\ backslash then close" x"#, STR),
        ("raw close here\"# rest", raw1),
        ("no hash close \" stays", raw1),
    ];
    for (line, st) in cases {
        let (toks, _) = tokenize_line(line, &Language::Rust, *st);
        let mut pos = 0;
        for t in &toks {
            assert_eq!(t.start, pos, "gap in {line:?}: {toks:?}");
            assert!(
                line.is_char_boundary(t.start) && line.is_char_boundary(t.start + t.len),
                "non-boundary span in {line:?}"
            );
            pos += t.len;
        }
        assert_eq!(pos, line.len(), "coverage mismatch in {line:?}: {toks:?}");
    }
}
