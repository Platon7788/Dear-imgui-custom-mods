//! Unit tests for the Rhai tokenizer. Split out of `tokenize.rs` to keep
//! every source file under the 500-line ceiling (CLAUDE.md).

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;

fn tok(line: &str) -> Vec<(TokenKind, String)> {
    let (tokens, _) = tokenize_line(line, &Language::Rhai, LineState::Code);
    tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect()
}

const BACKTICK: LineState = LineState::Str {
    quote: b'`',
    raw: false,
    hashes: 0,
    triple: false,
};

#[test]
fn range_operator_single_token() {
    let toks = tok("for x in 0..10 {}");
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Operator && s == "..")
    );
}

#[test]
fn until_and_global_keywords() {
    assert!(
        tok("do {} until x")
            .iter()
            .any(|(k, s)| *k == TokenKind::Keyword && s == "until")
    );
    assert!(
        tok("global::X")
            .iter()
            .any(|(k, s)| *k == TokenKind::Keyword && s == "global")
    );
}

#[test]
fn null_safe_operator() {
    let toks = tok("a?.b");
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Operator && s == "?.")
    );
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

/// A `${…}` interpolation splits into `String` + expression tokens +
/// `String`, with `${` / `}` as Punctuation.
#[test]
fn backtick_interpolation_splits() {
    let toks = tok("`Hello ${name}!`");
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::String && s.contains("Hello"))
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Punctuation && s == "${")
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Identifier && s == "name")
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Punctuation && s == "}")
    );
    // Two String runs: the text before and the text after the hole.
    let strings = toks.iter().filter(|(k, _)| *k == TokenKind::String).count();
    assert_eq!(
        strings, 2,
        "text before/after `${{…}}` are separate: {toks:?}"
    );
}

/// The expression inside `${…}` is tokenized with normal Rhai rules.
#[test]
fn backtick_interpolation_expression_tokens() {
    let toks = tok("`sum = ${a + b * 2}`");
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Operator && s == "+")
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Operator && s == "*")
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Number && s == "2")
    );
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::Identifier && s == "a")
    );
}

/// A brace inside a nested string within `${…}` must not close the hole.
#[test]
fn backtick_interpolation_brace_in_nested_string() {
    let toks = tok(r#"`${ f("a}b") }`"#);
    // The `}` inside the nested "a}b" string is not the closing brace, so
    // the call argument survives as a single String token.
    assert!(
        toks.iter()
            .any(|(k, s)| *k == TokenKind::String && s == r#""a}b""#)
    );
}

#[test]
fn block_comment() {
    let (_, still_in) = tokenize_line("/* start", &Language::Rhai, LineState::Code);
    assert_eq!(still_in, LineState::BlockComment(1));
    let (toks, done) = tokenize_line("end */ code", &Language::Rhai, LineState::BlockComment(1));
    assert_eq!(done, LineState::Code);
    assert_eq!(toks[0].kind, TokenKind::Comment);
}

/// Rhai block comments nest (`/* /* */ */`).
#[test]
fn nested_block_comment() {
    let (_, still_in) = tokenize_line("/* a /* b */ c */", &Language::Rhai, LineState::Code);
    assert_eq!(still_in, LineState::Code, "balanced nest closes");
    let (_, still_in2) = tokenize_line("/* a /* b */", &Language::Rhai, LineState::Code);
    assert_eq!(
        still_in2,
        LineState::BlockComment(1),
        "one level still open"
    );
}

/// Unterminated string / backtick must run to EOL without panic.
#[test]
fn unterminated_string_no_panic() {
    let toks = tok(r#"let s = "no close"#);
    assert!(toks.iter().any(|t| t.0 == TokenKind::String));
    let toks2 = tok("let s = `no close");
    assert!(toks2.iter().any(|t| t.0 == TokenKind::String));
}

/// An unterminated backtick template carries `Str { quote: b'`' }` so the
/// next line resumes inside the template until the closing backtick.
#[test]
fn multiline_backtick_carries() {
    let (_, st) = tokenize_line("let s = `line one", &Language::Rhai, LineState::Code);
    assert!(
        matches!(st, LineState::Str { quote: b'`', .. }),
        "unclosed backtick must carry a backtick Str state: {st:?}"
    );
    let (toks, st2) = tokenize_line("line two`;", &Language::Rhai, st);
    assert_eq!(st2, LineState::Code, "closing backtick ends the template");
    assert!(toks.iter().any(|t| t.kind == TokenKind::String));
}

/// Interpolation is still tokenized on a continuation line.
#[test]
fn multiline_backtick_interpolation_on_resume() {
    let (_, st) = tokenize_line("`start", &Language::Rhai, LineState::Code);
    let line = "mid ${x} end`";
    let (toks, st2) = tokenize_line(line, &Language::Rhai, st);
    assert_eq!(st2, LineState::Code);
    assert!(
        toks.iter()
            .any(|t| t.kind == TokenKind::Identifier && &line[t.start..t.start + t.len] == "x")
    );
    assert!(
        toks.iter()
            .any(|t| t.kind == TokenKind::Punctuation && &line[t.start..t.start + t.len] == "${")
    );
}

#[test]
fn builtin_types() {
    let toks = tok("let a: Dynamic = 42;");
    assert!(
        toks.iter()
            .any(|t| t.0 == TokenKind::TypeName && t.1 == "Dynamic")
    );
}

/// Multi-byte char literals classify as a single `CharLit`.
/// Regression for ADR-027 phase 3.
#[test]
fn char_literal_unicode() {
    for (input, want_lit) in [
        ("let c = 'é';", "'é'"),
        ("let c = '你';", "'你'"),
        (r"let c = '\n';", r"'\n'"),
    ] {
        let toks = tok(input);
        let chars: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::CharLit).collect();
        assert_eq!(chars.len(), 1, "input {input:?} produced {toks:?}");
        assert_eq!(chars[0].1, want_lit);
    }
}

/// Rhai 1.0+ supports hex, binary and octal radix prefixes plus
/// underscore separators (and float exponent for decimals).
/// Regression for ADR-027 phase 2 — rhai previously only had
/// `0x`, missed `0b`/`0o`.
#[test]
fn radix_and_underscore_separators() {
    for (line, want_lit) in [
        ("let a = 0xDEAD_BEEF;", "0xDEAD_BEEF"),
        ("let a = 0b1010;", "0b1010"),
        ("let a = 0o755;", "0o755"),
        ("let a = 1_000;", "1_000"),
        ("let a = 1.5e10;", "1.5e10"),
    ] {
        let toks = tok(line);
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1, "input {line:?} produced {nums:?}");
        assert_eq!(nums[0].1, want_lit, "input: {line:?}");
    }
}

/// Adversarial interpolation / multi-line inputs must tile the line
/// exactly (contiguous spans on char boundaries, total == line length)
/// in every entry state — the span-tiling invariant.
#[test]
fn interpolation_and_resume_tile_exactly() {
    let cases: &[(&str, LineState)] = &[
        ("`Hello ${name}!`", LineState::Code),
        ("`a ${ b + `nested` } c`", LineState::Code),
        ("`unterminated ${x", LineState::Code),
        ("`only text no close", LineState::Code),
        ("`${}`", LineState::Code),
        ("``", LineState::Code),
        ("resume text`;", BACKTICK),
        ("more ${y} tail`", BACKTICK),
        ("", BACKTICK),
        ("你好 ${世界}`", LineState::Code),
        ("`${ #{\"k\": 1} }`", LineState::Code),
    ];
    for (line, st) in cases {
        let (toks, _) = tokenize_line(line, &Language::Rhai, *st);
        let mut pos = 0;
        for t in &toks {
            assert_eq!(t.start, pos, "gap before token in {line:?}: {toks:?}");
            assert!(
                line.is_char_boundary(t.start) && line.is_char_boundary(t.start + t.len),
                "span not on char boundary in {line:?}: {toks:?}"
            );
            pos += t.len;
        }
        assert_eq!(pos, line.len(), "coverage mismatch in {line:?}: {toks:?}");
    }
}
