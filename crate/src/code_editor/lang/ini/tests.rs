//! `ini` tokenizer unit tests — section headers (plain + git-config),
//! keys/separators, value classification (strings, escapes, numbers,
//! booleans, `${VAR}`/`%VAR%` interpolation), comment rules, and the
//! backslash line-continuation carry.

use crate::code_editor::config::{Language, LineState};
use crate::code_editor::lang::tokenize_line;
use crate::code_editor::token::TokenKind;
use TokenKind::*;

/// `(kind, text)` pairs for every token on `line` (fresh `Code` state).
/// The return type is fully qualified: `use TokenKind::*` brings the
/// `String` *variant* into scope, which would otherwise shadow the type.
fn tok(line: &str) -> Vec<(TokenKind, std::string::String)> {
    let (tokens, _) = tokenize_line(line, &Language::Ini, LineState::Code);
    tokens
        .iter()
        .map(|t| (t.kind, line[t.start..t.start + t.len].to_string()))
        .collect()
}

/// Text of every token of `kind` on `line` (fresh `Code` state).
fn lits(line: &str, kind: TokenKind) -> Vec<&str> {
    let (tokens, _) = tokenize_line(line, &Language::Ini, LineState::Code);
    tokens
        .iter()
        .filter(|t| t.kind == kind)
        .map(|t| &line[t.start..t.start + t.len])
        .collect()
}

/// End-of-line carry state for `line` starting from `state`.
fn carry(line: &str, state: LineState) -> LineState {
    tokenize_line(line, &Language::Ini, state).1
}

// ── Section headers ───────────────────────────────────────────────────────────

#[test]
fn section_header_plain_is_single_attribute() {
    let toks = tok("[database]");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], (Attribute, "[database]".to_string()));
}

#[test]
fn section_header_dotted_stays_single_attribute() {
    assert_eq!(lits("[tool.black]", Attribute), vec!["[tool.black]"]);
}

#[test]
fn git_style_section_splits_quoted_subsection() {
    // `[core "filter"]` — bare part + quoted sub-section + closing bracket.
    let toks = tok(r#"[remote "origin"]"#);
    assert_eq!(toks[0], (Attribute, "[remote ".to_string()));
    assert_eq!(toks[1], (String, "\"origin\"".to_string()));
    assert_eq!(toks[2], (Attribute, "]".to_string()));
}

#[test]
fn unterminated_section_still_tiles() {
    // No closing bracket — must not panic; whole run is an attribute.
    assert_eq!(lits("[unterminated", Attribute), vec!["[unterminated"]);
}

// ── Keys and separators ───────────────────────────────────────────────────────

#[test]
fn key_value_equals() {
    let toks = tok("host = localhost");
    assert!(toks.contains(&(Attribute, "host".to_string())));
    assert!(toks.contains(&(Operator, "=".to_string())));
    assert!(toks.contains(&(Identifier, "localhost".to_string())));
}

#[test]
fn key_value_colon() {
    let toks = tok("port: 8080");
    assert!(toks.contains(&(Attribute, "port".to_string())));
    assert!(toks.contains(&(Operator, ":".to_string())));
    assert!(toks.contains(&(Number, "8080".to_string())));
}

#[test]
fn keyword_in_key_position_is_attribute_not_keyword() {
    // A key literally named `true` must render as an attribute — booleans
    // are only recognised in value position.
    let toks = tok("true = 1");
    assert!(toks.contains(&(Attribute, "true".to_string())));
    assert!(toks.iter().all(|t| t.0 != Keyword));
}

// ── Value: booleans / keywords ────────────────────────────────────────────────

#[test]
fn boolean_keywords_case_insensitive() {
    for word in ["true", "False", "YES", "no", "On", "OFF", "none", "Null"] {
        let line = format!("flag = {word}");
        assert_eq!(
            lits(&line, Keyword),
            vec![word],
            "boolean `{word}` should be a Keyword"
        );
    }
}

#[test]
fn non_keyword_bare_value_is_identifier() {
    assert_eq!(lits("color = red", Identifier), vec!["red"]);
}

// ── Value: numbers ────────────────────────────────────────────────────────────

#[test]
fn numbers_signed_float_radix() {
    for (line, want) in [
        ("a = 42", "42"),
        ("a = -5", "-5"),
        ("a = +7", "+7"),
        ("a = 3.14", "3.14"),
        ("a = -0.5", "-0.5"),
        ("a = .5", ".5"),
        ("a = 0xFF", "0xFF"),
        ("a = 0b1010", "0b1010"),
        ("a = 1_000", "1_000"),
        ("a = 1.5e10", "1.5e10"),
    ] {
        assert_eq!(lits(line, Number), vec![want], "{line:?}");
    }
}

#[test]
fn dash_before_non_digit_is_not_a_number() {
    // `-name` is a bare identifier, not a signed number.
    assert_eq!(lits("x = -name", Number), Vec::<&str>::new());
    assert_eq!(lits("x = -name", Identifier), vec!["-name"]);
}

// ── Value: interpolation ──────────────────────────────────────────────────────

#[test]
fn braced_variable_is_macro_call() {
    assert_eq!(lits("home = ${HOME}", MacroCall), vec!["${HOME}"]);
}

#[test]
fn bare_dollar_variable_is_macro_call() {
    assert_eq!(lits("home = $HOME", MacroCall), vec!["$HOME"]);
}

#[test]
fn variable_then_path_tail_splits() {
    // `${BASE}/bin` — variable is highlighted, `/bin` trails as identifier.
    let toks = tok("path = ${BASE}/bin");
    assert!(toks.contains(&(MacroCall, "${BASE}".to_string())));
    assert!(toks.contains(&(Identifier, "/bin".to_string())));
}

#[test]
fn windows_percent_variable_is_macro_call() {
    assert_eq!(lits("p = %APPDATA%", MacroCall), vec!["%APPDATA%"]);
}

#[test]
fn lone_percent_is_not_a_variable() {
    // `50%` — the `%` is a value byte, not an unclosed interpolation.
    assert_eq!(lits("used = 50%", MacroCall), Vec::<&str>::new());
    assert!(lits("used = 50%", Number).contains(&"50"));
}

// ── Value: strings + escapes ──────────────────────────────────────────────────

#[test]
fn quoted_string_no_escape_is_single_token() {
    assert_eq!(
        lits("name = \"hello world\"", String),
        vec!["\"hello world\""]
    );
}

#[test]
fn double_quoted_escape_splits_into_charlit() {
    // `"a\nb"` — String("a) + CharLit(\n) + String(b").
    let toks = tok(r#"msg = "a\nb""#);
    assert!(toks.contains(&(String, "\"a".to_string())));
    assert!(toks.contains(&(CharLit, "\\n".to_string())));
    assert!(toks.contains(&(String, "b\"".to_string())));
}

#[test]
fn single_quoted_string_is_literal() {
    // Single quotes don't process escapes — one String token, `\` included.
    assert_eq!(lits(r"win = 'C:\temp'", String), vec![r"'C:\temp'"]);
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[test]
fn full_line_comments() {
    assert_eq!(tok("; a comment")[0].0, Comment);
    assert_eq!(tok("# also a comment")[0].0, Comment);
}

#[test]
fn inline_comment_after_value() {
    let toks = tok("x = 1 ; trailing");
    assert!(toks.contains(&(Number, "1".to_string())));
    assert!(toks.contains(&(Comment, "; trailing".to_string())));
}

#[test]
fn marker_glued_to_value_is_not_a_comment() {
    // `#`/`;` immediately preceded by a value byte stays part of the value.
    assert!(lits("pass#word", Identifier).contains(&"pass#word"));
    assert!(lits("url = http://a;b", Identifier).contains(&"http://a;b"));
    assert!(tok("pass#word").iter().all(|t| t.0 != Comment));
}

#[test]
fn spaced_marker_opens_a_comment() {
    let toks = tok("key = value ; real comment");
    assert!(toks.contains(&(Identifier, "value".to_string())));
    assert!(toks.contains(&(Comment, "; real comment".to_string())));
}

// ── Line continuation ─────────────────────────────────────────────────────────

#[test]
fn trailing_backslash_opens_continuation() {
    // A value line ending in a lone `\` carries onto the next line.
    let state = carry(r"paths = a;b;c \", LineState::Code);
    assert!(matches!(state, LineState::Str { quote: b'\\', .. }));
}

#[test]
fn continuation_line_resumes_and_can_close() {
    let open = carry(r"paths = one \", LineState::Code);
    assert!(matches!(open, LineState::Str { .. }));
    // Second line still open (also ends with `\`).
    let still = carry(r"    two \", open);
    assert!(matches!(still, LineState::Str { .. }));
    // Third line has no trailing `\` — continuation closes.
    let closed = carry("    three", still);
    assert_eq!(closed, LineState::Code);
}

#[test]
fn escaped_backslash_does_not_continue() {
    // `\\` at end is a literal backslash pair, not a continuation.
    assert_eq!(carry(r"x = C:\\", LineState::Code), LineState::Code);
}

#[test]
fn continuation_line_tiles_and_is_value() {
    // A continuation line is tokenized as a value region.
    let open = LineState::Str {
        quote: b'\\',
        raw: true,
        hashes: 0,
        triple: false,
    };
    let (toks, _) = tokenize_line("more values 42", &Language::Ini, open);
    assert!(toks.iter().any(|t| t.kind == Number));
    // Tiling: spans cover the whole line contiguously.
    let total: usize = toks.iter().map(|t| t.len).sum();
    assert_eq!(total, "more values 42".len());
}

// ── Blank line ────────────────────────────────────────────────────────────────

#[test]
fn blank_line_is_empty_and_code() {
    let (toks, state) = tokenize_line("", &Language::Ini, LineState::Code);
    assert!(toks.is_empty());
    assert_eq!(state, LineState::Code);
}
