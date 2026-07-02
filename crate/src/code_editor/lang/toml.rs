//! TOML configuration file tokenizer.

use super::{NumberOpts, consume_number, is_ident_continue, is_ident_start};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &["true", "false"];

// ── Language definition ─────────────────────────────────────────────────────

pub struct TomlLang;

impl SyntaxDefinition for TomlLang {
    fn name(&self) -> &str {
        "TOML"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize(line, state)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("#")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        None
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('{', '}')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

/// Push a token spanning `start..start+len` of the given `kind`.
fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, len: usize) {
    tokens.push(Token { kind, start, len });
}

fn tokenize(line: &str, state: LineState) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    // ── Continuation of a multi-line triple-quoted string (`"""…"""` basic
    //    with escapes, or `'''…'''` literal). Close on the matching triple. ─
    if let LineState::Str {
        quote,
        raw,
        triple: true,
        ..
    } = state
    {
        let (end, closed) = scan_triple_close(bytes, 0, quote, !raw);
        if end > 0 {
            push(&mut tokens, TokenKind::String, 0, end);
        }
        i = end;
        if !closed {
            return (tokens, state);
        }
    }

    while i < len {
        let b = bytes[i];

        // Whitespace
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            push(&mut tokens, TokenKind::Whitespace, start, i - start);
            continue;
        }

        // Comment
        if b == b'#' {
            push(&mut tokens, TokenKind::Comment, i, len - i);
            return (tokens, LineState::Code);
        }

        // Section headers [section] / [[array.of.tables]] — ONLY when the
        // bracket is the first non-whitespace token (else it's an inline array).
        if b == b'[' && tokens.iter().all(|t| t.kind == TokenKind::Whitespace) {
            let start = i;
            let mut depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'[' => depth += 1,
                    b']' => {
                        depth = depth.saturating_sub(1);
                        i += 1;
                        if depth == 0 {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
            push(&mut tokens, TokenKind::Attribute, start, i - start);
            continue;
        }

        // String — triple-quoted (multi-line) or single-line. Basic
        // (double-quoted) strings process `\` escapes; literal (single) don't.
        if b == b'"' || b == b'\'' {
            let quote = b;
            // Triple-quoted? `"""` / `'''` — may span lines.
            if i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
                let raw = quote == b'\'';
                let (end, closed) = scan_triple_close(bytes, i + 3, quote, !raw);
                push(&mut tokens, TokenKind::String, i, end - i);
                i = end;
                if !closed {
                    return (
                        tokens,
                        LineState::Str {
                            quote,
                            raw,
                            hashes: 0,
                            triple: true,
                        },
                    );
                }
                continue;
            }
            // Single-line string.
            let escapes = quote == b'"';
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if escapes && bytes[i] == b'\\' && i + 1 < len {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            push(&mut tokens, TokenKind::String, start, i - start);
            continue;
        }

        // Datetime (RFC3339) — before the number branch so a date isn't split.
        if let Some(end) = match_datetime(bytes, i) {
            push(&mut tokens, TokenKind::Number, i, end - i);
            i = end;
            continue;
        }

        // Signed special floats: +inf / -inf / +nan / -nan.
        if let Some(n) = signed_special_float_len(bytes, i) {
            push(&mut tokens, TokenKind::Number, i, n);
            i += n;
            continue;
        }

        // Number — decimal, hex/oct/bin radix, `_` separators and exponent.
        if b.is_ascii_digit()
            || ((b == b'-' || b == b'+') && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            consume_number(&mut i, bytes, NumberOpts::RUST_LIKE);
            push(&mut tokens, TokenKind::Number, start, i - start);
            continue;
        }

        // Identifier / keyword / bare key. Every segment of a dotted key
        // (`a.b.c`) is Attribute; value-position `inf`/`nan` are Numbers.
        if is_ident_start(b) {
            let start = i;
            while i < len && (is_ident_continue(bytes[i]) || bytes[i] == b'-') {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if key_path_reaches_eq(bytes, i) {
                TokenKind::Attribute
            } else if word == "inf" || word == "nan" {
                TokenKind::Number
            } else {
                TokenKind::Identifier
            };
            push(&mut tokens, kind, start, i - start);
            continue;
        }

        // Operator (=)
        if b == b'=' {
            push(&mut tokens, TokenKind::Operator, i, 1);
            i += 1;
            continue;
        }

        // Punctuation (inline-array/table brackets and dotted-key dots)
        if matches!(b, b'{' | b'}' | b',' | b'.' | b'[' | b']') {
            push(&mut tokens, TokenKind::Punctuation, i, 1);
            i += 1;
            continue;
        }

        // Fallback
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        push(&mut tokens, TokenKind::Identifier, i, ch_len);
        i += ch_len;
    }

    (tokens, LineState::Code)
}

// ── Scan helpers ────────────────────────────────────────────────────────────

/// Scan for a triple-quote close (`"""` / `'''`) from `start`. `escapes`
/// skips `\`-escapes (basic strings). Returns `(end, closed)`: `end` is the
/// byte index just past the close (or `len` if still open).
fn scan_triple_close(bytes: &[u8], start: usize, quote: u8, escapes: bool) -> (usize, bool) {
    let len = bytes.len();
    let mut i = start;
    while i < len {
        if escapes && bytes[i] == b'\\' {
            i += if i + 1 < len { 2 } else { 1 };
            continue;
        }
        if bytes[i] == quote && i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
            return (i + 3, true);
        }
        i += 1;
    }
    (len, false)
}

/// `true` if `bytes[start..start+count]` exist and are all ASCII digits.
fn digits(bytes: &[u8], start: usize, count: usize) -> bool {
    start + count <= bytes.len() && bytes[start..start + count].iter().all(u8::is_ascii_digit)
}

/// Match a `YYYY-MM-DD` date at `p`; returns the end index.
fn match_date(bytes: &[u8], p: usize) -> Option<usize> {
    (digits(bytes, p, 4)
        && bytes.get(p + 4) == Some(&b'-')
        && digits(bytes, p + 5, 2)
        && bytes.get(p + 7) == Some(&b'-')
        && digits(bytes, p + 8, 2))
    .then_some(p + 10)
}

/// Match a `HH:MM:SS(.fraction)` time at `p`; returns the end index.
fn match_time(bytes: &[u8], p: usize) -> Option<usize> {
    if digits(bytes, p, 2)
        && bytes.get(p + 2) == Some(&b':')
        && digits(bytes, p + 3, 2)
        && bytes.get(p + 5) == Some(&b':')
        && digits(bytes, p + 6, 2)
    {
        let mut e = p + 8;
        if bytes.get(e) == Some(&b'.') && digits(bytes, e + 1, 1) {
            e += 1;
            while e < bytes.len() && bytes[e].is_ascii_digit() {
                e += 1;
            }
        }
        Some(e)
    } else {
        None
    }
}

/// Match an RFC3339 zone (`Z` / `±HH:MM`) at `p`; returns the end index.
fn match_offset(bytes: &[u8], p: usize) -> Option<usize> {
    match bytes.get(p) {
        Some(&b'Z') | Some(&b'z') => Some(p + 1),
        Some(&b'+') | Some(&b'-')
            if digits(bytes, p + 1, 2)
                && bytes.get(p + 3) == Some(&b':')
                && digits(bytes, p + 4, 2) =>
        {
            Some(p + 6)
        }
        _ => None,
    }
}

/// Match an RFC3339 date, time, or datetime at `i` (local or offset, `T`- or
/// space-separated). Returns the end index of the whole literal, or `None`.
fn match_datetime(bytes: &[u8], i: usize) -> Option<usize> {
    let Some(after_date) = match_date(bytes, i) else {
        return match_time(bytes, i);
    };
    let sep_ok = matches!(bytes.get(after_date), Some(&(b'T' | b't' | b' ')));
    match sep_ok.then(|| match_time(bytes, after_date + 1)).flatten() {
        Some(t) => Some(match_offset(bytes, t).unwrap_or(t)),
        None => Some(after_date),
    }
}

/// `true` if the segment run starting at `j` (just past a bare key) reaches an
/// `=`, treating `.`-separated bare segments as one dotted key path.
fn key_path_reaches_eq(bytes: &[u8], mut j: usize) -> bool {
    let len = bytes.len();
    loop {
        while j < len && matches!(bytes[j], b' ' | b'\t') {
            j += 1;
        }
        match bytes.get(j) {
            Some(&b'=') => return true,
            Some(&b'.') => {
                j += 1;
                while j < len && matches!(bytes[j], b' ' | b'\t') {
                    j += 1;
                }
                if !bytes.get(j).is_some_and(|&c| is_ident_start(c)) {
                    return false;
                }
                while j < len && (is_ident_continue(bytes[j]) || bytes[j] == b'-') {
                    j += 1;
                }
            }
            _ => return false,
        }
    }
}

/// Byte length of a signed special float (`+inf` / `-inf` / `+nan` / `-nan`)
/// at `i`, or `None`. Bare `inf`/`nan` are classified in the identifier arm.
fn signed_special_float_len(bytes: &[u8], i: usize) -> Option<usize> {
    if !matches!(bytes.get(i), Some(&b'+') | Some(&b'-')) || i + 4 > bytes.len() {
        return None;
    }
    let seg = &bytes[i + 1..i + 4];
    ((seg == b"inf" || seg == b"nan") && (i + 4 == bytes.len() || !is_ident_continue(bytes[i + 4])))
        .then_some(4)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::code_editor::config::{Language, LineState};
    use crate::code_editor::lang::tokenize_line;
    use crate::code_editor::token::TokenKind;

    fn lits(line: &str, kind: TokenKind) -> Vec<&str> {
        let (toks, _) = tokenize_line(line, &Language::Toml, LineState::Code);
        toks.iter()
            .filter(|t| t.kind == kind)
            .map(|t| &line[t.start..t.start + t.len])
            .collect()
    }

    #[test]
    fn section_headers() {
        assert_eq!(lits("[package]", TokenKind::Attribute), vec!["[package]"]);
        assert_eq!(
            lits("[[dependencies.serde]]", TokenKind::Attribute),
            vec!["[[dependencies.serde]]"]
        );
    }

    #[test]
    fn inline_array_value_is_not_a_section_header() {
        let line = r#"members = ["crate", "app"]"#;
        assert_eq!(lits(line, TokenKind::Punctuation), vec!["[", ",", "]"]);
        assert_eq!(lits(line, TokenKind::Attribute), vec!["members"]);
        assert_eq!(lits(line, TokenKind::String), vec!["\"crate\"", "\"app\""]);
    }

    #[test]
    fn keys_and_values() {
        assert_eq!(lits("name = \"hello\"", TokenKind::Attribute), vec!["name"]);
        assert_eq!(
            lits("name = \"hello\"", TokenKind::String),
            vec!["\"hello\""]
        );
        // Bare key (with dash) is Attribute; a bare value stays Identifier.
        assert_eq!(lits("my-key = 42", TokenKind::Attribute), vec!["my-key"]);
        assert_eq!(lits("color = red", TokenKind::Identifier), vec!["red"]);
    }

    #[test]
    fn literal_string_no_escape() {
        // Single-quoted strings keep `\` verbatim and still close at `'`.
        assert_eq!(
            lits(r"path = 'C:\temp\new'", TokenKind::String),
            vec![r"'C:\temp\new'"]
        );
    }

    #[test]
    fn comment() {
        let (toks, _) = tokenize_line("# comment", &Language::Toml, LineState::Code);
        assert_eq!(toks[0].kind, TokenKind::Comment);
    }

    #[test]
    fn numbers_radix_signs_underscores() {
        for (line, want) in [
            ("x = -0xFF", "-0xFF"),
            ("x = +0xDEAD_BEEF", "+0xDEAD_BEEF"),
            ("x = +0b1010", "+0b1010"),
            ("x = -0o755", "-0o755"),
            ("a = 1_000_000", "1_000_000"),
            ("a = 1.5e10", "1.5e10"),
            ("a = -42", "-42"),
        ] {
            assert_eq!(lits(line, TokenKind::Number), vec![want], "{line:?}");
        }
    }

    /// Triple-quoted strings span lines: `"""…"""` (basic) opens a `Str`
    /// state that a later line closes; `'''…'''` (literal) is raw.
    #[test]
    fn multiline_triple_strings() {
        let (t1, s1) = tokenize_line("x = \"\"\"start", &Language::Toml, LineState::Code);
        assert!(matches!(s1, LineState::Str { raw: false, .. }));
        assert!(matches!(s1, LineState::Str { triple: true, .. }));
        assert_eq!(t1.last().unwrap().kind, TokenKind::String);
        let (_t2, s2) = tokenize_line("middle", &Language::Toml, s1);
        assert_eq!(s2, s1, "still open on line 2");
        let (t3, s3) = tokenize_line("end\"\"\"", &Language::Toml, s2);
        assert_eq!(s3, LineState::Code);
        assert_eq!(t3[0].kind, TokenKind::String);

        // Literal `'''…'''` is raw (no `\` escapes); closes on `'''`.
        let (_l, ls) = tokenize_line(r"y = '''C:\raw", &Language::Toml, LineState::Code);
        assert!(matches!(ls, LineState::Str { raw: true, .. }));
        let (lt, le) = tokenize_line("done'''", &Language::Toml, ls);
        assert_eq!(le, LineState::Code);
        assert_eq!(lt[0].kind, TokenKind::String);
    }

    /// A single-line triple `"""x"""` closes immediately (state stays Code).
    #[test]
    fn single_line_triple_closes_immediately() {
        let (_toks, s) = tokenize_line("x = \"\"\"y\"\"\"", &Language::Toml, LineState::Code);
        assert_eq!(s, LineState::Code);
        assert_eq!(
            lits("x = \"\"\"y\"\"\"", TokenKind::String),
            vec!["\"\"\"y\"\"\""]
        );
    }

    /// RFC3339 dates, times, and datetimes are single Number tokens.
    #[test]
    fn datetimes_are_numbers() {
        for (line, want) in [
            ("d = 2024-01-01", "2024-01-01"),
            ("t = 07:32:00", "07:32:00"),
            ("t = 07:32:00.999", "07:32:00.999"),
            ("dt = 2024-01-01T07:32:00", "2024-01-01T07:32:00"),
            ("dt = 2024-01-01T07:32:00Z", "2024-01-01T07:32:00Z"),
            ("z = 2024-01-01T07:32:00+07:00", "2024-01-01T07:32:00+07:00"),
            ("s = 2024-01-01 07:32:00", "2024-01-01 07:32:00"),
        ] {
            assert_eq!(lits(line, TokenKind::Number), vec![want], "{line:?}");
        }
    }

    /// Every segment of a dotted key `a.b.c = v` is an Attribute; the dots
    /// between segments are Punctuation.
    #[test]
    fn dotted_key_every_segment_is_attribute() {
        assert_eq!(lits("a.b.c = 1", TokenKind::Attribute), vec!["a", "b", "c"]);
        assert_eq!(lits("a.b.c = 1", TokenKind::Punctuation), vec![".", "."]);
    }

    #[test]
    fn special_floats_are_numbers() {
        for (line, want) in [
            ("x = inf", "inf"),
            ("x = +inf", "+inf"),
            ("x = -inf", "-inf"),
            ("x = nan", "nan"),
            ("x = +nan", "+nan"),
            ("x = -nan", "-nan"),
        ] {
            assert_eq!(lits(line, TokenKind::Number), vec![want], "{line:?}");
        }
    }
}
