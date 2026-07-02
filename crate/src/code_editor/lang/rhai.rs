//! Rhai scripting language tokenizer.

use super::{
    NumberOpts, consume_char_literal, consume_number, is_ident_continue, is_ident_start,
    scan_block_comment,
};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

const KEYWORDS: &[&str] = &[
    "let",
    "const",
    "if",
    "else",
    "while",
    "loop",
    "for",
    "in",
    "do",
    "until",
    "break",
    "continue",
    "return",
    "throw",
    "try",
    "catch",
    "fn",
    "private",
    "import",
    "export",
    "as",
    "switch",
    "is",
    "type_of",
    "print",
    "debug",
    "true",
    "false",
    "this",
    "call",
    "curry",
    "is_def_fn",
    "is_def_var",
    "is_shared",
    "eval",
    "global",
];

const BUILTIN_TYPES: &[&str] = &[
    "bool", "char", "i64", "f64", "String", "Array", "Map", "Blob", "Dynamic", "Instant", "FnPtr",
    "Decimal",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct RhaiLang;

impl SyntaxDefinition for RhaiLang {
    fn name(&self) -> &str {
        "Rhai"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize(line, state)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &['{']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &['}']
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[
            ("(", ")"),
            ("{", "}"),
            ("[", "]"),
            ("\"", "\""),
            ("'", "'"),
            ("`", "`"),
        ]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str, state: LineState) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    let mut depth: u32 = if let LineState::BlockComment(d) = state {
        u32::from(d)
    } else {
        0
    };

    // ── Resume a multi-line backtick template from the previous line ──────
    // Rhai backtick strings (`` `…` ``) may span lines; the editor threads
    // `LineState::Str { quote: b'`', … }` back to us. Scan from column 0 as a
    // template body (String text + `${…}` interpolation) until it closes.
    if let LineState::Str { quote: b'`', .. } = state {
        let closed = scan_backtick_template(&mut i, line, bytes, &mut tokens, 0);
        if !closed {
            return (tokens, state);
        }
        // Closed mid-line — fall through to tokenize the remainder as code.
    }

    while i < len {
        // ── Inside a (possibly nested) block comment ─────────────────────
        if depth > 0 {
            let start = i;
            depth = scan_block_comment(&mut i, bytes, depth);
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // Consume one token; a multi-line construct that doesn't close on
        // this line hands back the carry state for the next line.
        if let Some(carry) = consume_token(&mut i, line, bytes, &mut tokens) {
            return (tokens, carry);
        }
    }

    let end = if depth > 0 {
        LineState::BlockComment(depth as u16)
    } else {
        LineState::Code
    };
    (tokens, end)
}

/// Consume exactly one Rhai token at `*i`, pushing it (or, for backtick
/// templates, several) to `tokens` and advancing `*i` by at least one byte.
///
/// Returns `Some(state)` when a multi-line construct (block comment or
/// backtick template) opened here and did **not** close before end-of-line —
/// the caller stops and threads that state to the next line. Returns `None`
/// for an ordinary single-line token.
fn consume_token(
    i: &mut usize,
    line: &str,
    bytes: &[u8],
    tokens: &mut Vec<Token>,
) -> Option<LineState> {
    let len = bytes.len();
    let b = bytes[*i];

    // ── Whitespace ───────────────────────────────────────────────────────
    if b == b' ' || b == b'\t' {
        let start = *i;
        while *i < len && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
            *i += 1;
        }
        push(tokens, TokenKind::Whitespace, start, *i);
        return None;
    }

    // ── Line comment ─────────────────────────────────────────────────────
    if b == b'/' && *i + 1 < len && bytes[*i + 1] == b'/' {
        push(tokens, TokenKind::Comment, *i, len);
        *i = len;
        return None;
    }

    // ── Block comment (nesting-aware) ────────────────────────────────────
    if b == b'/' && *i + 1 < len && bytes[*i + 1] == b'*' {
        let start = *i;
        *i += 2;
        let depth = scan_block_comment(i, bytes, 1);
        push(tokens, TokenKind::Comment, start, *i);
        return (depth > 0).then_some(LineState::BlockComment(depth as u16));
    }

    // ── Backtick template string (with `${…}` interpolation) ─────────────
    if b == b'`' {
        let bt = *i;
        *i += 1;
        if scan_backtick_template(i, line, bytes, tokens, bt) {
            return None;
        }
        return Some(LineState::Str {
            quote: b'`',
            raw: false,
            hashes: 0,
            triple: false,
        });
    }

    // ── Double-quote string literal (single line) ────────────────────────
    if b == b'"' {
        let start = *i;
        *i += 1;
        while *i < len {
            if bytes[*i] == b'\\' && *i + 1 < len {
                *i += 2;
            } else if bytes[*i] == b'"' {
                *i += 1;
                break;
            } else {
                *i += 1;
            }
        }
        push(tokens, TokenKind::String, start, *i);
        return None;
    }

    // ── Char literal ─────────────────────────────────────────────────────
    if b == b'\'' {
        if let Some(end) = consume_char_literal(line, *i) {
            push(tokens, TokenKind::CharLit, *i, end);
            *i = end;
        } else {
            // Stray apostrophe — Rhai has no lifetime / raw-string usage.
            push(tokens, TokenKind::Punctuation, *i, *i + 1);
            *i += 1;
        }
        return None;
    }

    // ── Number ───────────────────────────────────────────────────────────
    if b.is_ascii_digit() || (b == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit()) {
        let start = *i;
        consume_number(i, bytes, NumberOpts::RUST_LIKE);
        push(tokens, TokenKind::Number, start, *i);
        return None;
    }

    // ── Identifier / Keyword / Type ──────────────────────────────────────
    if is_ident_start(b) {
        let start = *i;
        while *i < len && is_ident_continue(bytes[*i]) {
            *i += 1;
        }
        let word = &line[start..*i];
        let kind = if KEYWORDS.contains(&word) {
            TokenKind::Keyword
        } else if BUILTIN_TYPES.contains(&word)
            || word.chars().next().is_some_and(|c| c.is_uppercase())
        {
            TokenKind::TypeName
        } else {
            TokenKind::Identifier
        };
        push(tokens, kind, start, *i);
        return None;
    }

    // ── Operators ────────────────────────────────────────────────────────
    if matches!(
        b,
        b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' | b'~'
    ) {
        let start = *i;
        *i += 1;
        if *i < len
            && matches!(
                (b, bytes[*i]),
                (b'=', b'=')
                    | (b'!', b'=')
                    | (b'<', b'=')
                    | (b'>', b'=')
                    | (b'-', b'>')
                    | (b'=', b'>')
                    | (b'&', b'&')
                    | (b'|', b'|')
                    | (b'+', b'=')
                    | (b'-', b'=')
                    | (b'*', b'=')
                    | (b'/', b'=')
                    | (b'*', b'*') // exponent
                    | (b'<', b'<')
                    | (b'>', b'>')
                    | (b'%', b'=')
                    | (b'&', b'=')
                    | (b'|', b'=')
                    | (b'^', b'=')
            )
        {
            *i += 1;
        }
        push(tokens, TokenKind::Operator, start, *i);
        return None;
    }

    // ── Range operators `..` / `..=` (e.g. `for x in 0..10`) ──────────────
    if b == b'.' && *i + 1 < len && bytes[*i + 1] == b'.' {
        let start = *i;
        *i += 2;
        if *i < len && bytes[*i] == b'=' {
            *i += 1;
        }
        push(tokens, TokenKind::Operator, start, *i);
        return None;
    }

    // ── `?.` null-safe access, `??` null-coalescing ──────────────────────
    if b == b'?' && *i + 1 < len && matches!(bytes[*i + 1], b'.' | b'?') {
        push(tokens, TokenKind::Operator, *i, *i + 2);
        *i += 2;
        return None;
    }

    // ── Punctuation ──────────────────────────────────────────────────────
    if matches!(
        b,
        b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b':' | b',' | b'.' | b'@' | b'?' | b'#'
    ) {
        push(tokens, TokenKind::Punctuation, *i, *i + 1);
        *i += 1;
        return None;
    }

    // ── Fallback: full Unicode scalar ────────────────────────────────────
    let ch_len = line[*i..].chars().next().map_or(1, |c| c.len_utf8());
    push(tokens, TokenKind::Identifier, *i, *i + ch_len);
    *i += ch_len;
    None
}

// ── Backtick template helpers ────────────────────────────────────────────────

/// Push a token spanning the byte range `[start, end)` (a no-op when empty).
#[inline]
fn push(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    if end > start {
        tokens.push(Token {
            kind,
            start,
            len: end - start,
        });
    }
}

/// Scan a backtick-template body from `*i`, emitting `String` literal
/// segments, `${` / `}` punctuation, and normal Rhai tokens for the
/// interpolated expressions. `seg_start` marks where the pending String
/// segment begins — the opening backtick index for a fresh template, or `0`
/// when resuming a multi-line template. Returns `true` if the closing
/// backtick was found on this line.
fn scan_backtick_template(
    i: &mut usize,
    line: &str,
    bytes: &[u8],
    tokens: &mut Vec<Token>,
    mut seg_start: usize,
) -> bool {
    let len = bytes.len();
    while *i < len {
        match bytes[*i] {
            b'`' => {
                *i += 1;
                push(tokens, TokenKind::String, seg_start, *i);
                return true;
            }
            b'$' if *i + 1 < len && bytes[*i + 1] == b'{' => {
                push(tokens, TokenKind::String, seg_start, *i);
                push(tokens, TokenKind::Punctuation, *i, *i + 2);
                *i += 2;
                scan_interp_expr(i, line, bytes, tokens);
                seg_start = *i;
            }
            _ => *i += 1,
        }
    }
    push(tokens, TokenKind::String, seg_start, *i);
    false
}

/// Tokenize an interpolated expression inside `${ … }`, starting just past
/// the `${`. Braces are depth-tracked so the matching `}` (emitted as
/// Punctuation) ends the expression; braces inside nested strings/chars are
/// skipped because [`consume_token`] swallows those as single tokens. Stops
/// at end-of-line if the expression spills across lines.
fn scan_interp_expr(i: &mut usize, line: &str, bytes: &[u8], tokens: &mut Vec<Token>) {
    let len = bytes.len();
    let mut depth = 1u32;
    while *i < len {
        match bytes[*i] {
            b'{' => {
                depth += 1;
                push(tokens, TokenKind::Punctuation, *i, *i + 1);
                *i += 1;
            }
            b'}' => {
                depth -= 1;
                push(tokens, TokenKind::Punctuation, *i, *i + 1);
                *i += 1;
                if depth == 0 {
                    return;
                }
            }
            // Any multi-line construct opened here (`consume_token` returns
            // `Some`) drives `*i` to EOL, so the loop exits cleanly.
            _ => {
                let _ = consume_token(i, line, bytes, tokens);
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
        let (toks, done) =
            tokenize_line("end */ code", &Language::Rhai, LineState::BlockComment(1));
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
            toks.iter().any(
                |t| t.kind == TokenKind::Punctuation && &line[t.start..t.start + t.len] == "${"
            )
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
}
