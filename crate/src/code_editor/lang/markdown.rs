//! Markdown tokenizer.
//!
//! A full-quality Markdown highlighter that is **stateful** across lines via
//! [`LineState::Fenced`]. Fenced code blocks (```` ``` ```` / `~~~`) are the
//! reason multi-line state matters: a `# comment` *inside* a code block must
//! not be coloured as a heading, so the open-fence line stores the fence byte
//! and length in the carried [`LineState`] and every body line stays plain
//! until a matching close fence is seen.
//!
//! Block constructs (per line, after optional indentation):
//! * fenced code open/close — ```` ``` ```` or `~~~` (3+ of the same char)
//! * ATX headings — `#`..`######` + space
//! * thematic break / horizontal rule — a line of only `---` / `***` / `___`
//! * blockquotes — leading `>`
//! * list markers — `- ` / `* ` / `+ ` / `1. ` / `1) `
//!
//! Inline constructs (within a normal line, blockquote body, or list body):
//! * inline code — `` `…` `` (run-length matched)
//! * bold — `**…**` / `__…__`, italic — `*…*` / `_…_`
//! * strikethrough — `~~…~~`
//! * links `[text](url)` and images `![alt](url)` — text/alt vs url coloured
//!   distinctly, brackets/parens as punctuation
//! * autolinks — `<http://…>` / `<mailto…>`
//! * backslash escapes — `\*` etc. render literally, never as emphasis
//!
//! Everything else renders as plain text. Unclosed / partial inline markers
//! still tile: they fall back to punctuation or plain text. The tokenizer
//! never panics, always advances at least one byte per step, and its spans
//! exactly tile the line on char boundaries (the invariant enforced by
//! `all_langs_no_panic_full_coverage_both_states`).

use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

// ── Language definition ─────────────────────────────────────────────────────

pub struct MarkdownLang;

impl SyntaxDefinition for MarkdownLang {
    fn name(&self) -> &str {
        "Markdown"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        match state {
            // Only the fenced-code carry state changes tokenization; every
            // other incoming state (Code, BlockComment, Str, …) is treated as
            // an ordinary Markdown line.
            LineState::Fenced { fence, count } => tokenize_fenced(line, fence, count),
            _ => tokenize_normal(line),
        }
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        // Markdown has no line comment; HTML-style block comments only.
        None
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("<!--", "-->"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('[', ']'), ('(', ')')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &[]
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("[", "]"), ("(", ")"), ("`", "`")]
    }
}

// ── Small helper ────────────────────────────────────────────────────────────

#[inline]
fn tok(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        start,
        len: end - start,
    }
}

// ── Fenced-code body / close ────────────────────────────────────────────────

/// Tokenize a line while inside a fenced code block opened with `count`
/// repetitions of the `fence` byte (`` b'`' `` or `b'~'`).
///
/// A closing fence — optional indentation, a run of `>= count` of the same
/// fence char, then only whitespace — ends the block and returns
/// [`LineState::Code`]. Any other line (including one that merely *looks* like
/// a heading or comment) is coloured as plain code text and stays fenced.
fn tokenize_fenced(line: &str, fence: u8, count: u8) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    if len == 0 {
        // Blank line inside the block — no tokens, stay fenced.
        return (vec![], LineState::Fenced { fence, count });
    }

    // Optional leading whitespace.
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let ws_end = i;

    // Run of fence chars.
    let fence_start = i;
    let mut n = 0usize;
    while i < len && bytes[i] == fence {
        i += 1;
        n += 1;
    }
    let fence_end = i;

    // The remainder of a close fence must be whitespace only.
    let only_ws = bytes[fence_end..].iter().all(|&c| c == b' ' || c == b'\t');

    if n >= count as usize && only_ws {
        // Closing fence — colour it and leave the block.
        let mut tokens = Vec::with_capacity(3);
        if ws_end > 0 {
            tokens.push(tok(TokenKind::Whitespace, 0, ws_end));
        }
        tokens.push(tok(TokenKind::Operator, fence_start, fence_end));
        if fence_end < len {
            tokens.push(tok(TokenKind::Whitespace, fence_end, len));
        }
        return (tokens, LineState::Code);
    }

    // Ordinary code line — whole line as plain code text, still fenced.
    (
        vec![tok(TokenKind::String, 0, len)],
        LineState::Fenced { fence, count },
    )
}

// ── Normal (non-fenced) line ────────────────────────────────────────────────

fn tokenize_normal(line: &str) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return (vec![], LineState::Code);
    }

    let mut tokens = Vec::with_capacity(8);

    // Leading whitespace (block-level indentation).
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i > 0 {
        tokens.push(tok(TokenKind::Whitespace, 0, i));
    }
    let cs = i;

    if cs >= len {
        // Whitespace-only line.
        return (tokens, LineState::Code);
    }

    // ── Fenced code open (``` or ~~~, 3+) ────────────────────────────────────
    let first = bytes[cs];
    if first == b'`' || first == b'~' {
        let mut k = cs;
        let mut n = 0usize;
        while k < len && bytes[k] == first {
            k += 1;
            n += 1;
        }
        if n >= 3 {
            tokens.push(tok(TokenKind::Operator, cs, k));
            if k < len {
                // Info string (language tag / rest of line).
                tokens.push(tok(TokenKind::Keyword, k, len));
            }
            let count = n.min(u8::MAX as usize) as u8;
            return (
                tokens,
                LineState::Fenced {
                    fence: first,
                    count,
                },
            );
        }
        // Fewer than 3 → not a fence; fall through to inline handling
        // (`` `code` ``, `~~strike~~`, etc.).
    }

    // ── Thematic break / horizontal rule ─────────────────────────────────────
    if is_thematic_break(bytes, cs, len) {
        tokens.push(tok(TokenKind::Operator, cs, len));
        return (tokens, LineState::Code);
    }

    // ── ATX heading ──────────────────────────────────────────────────────────
    if first == b'#' {
        let mut k = cs;
        let mut h = 0usize;
        while k < len && bytes[k] == b'#' {
            k += 1;
            h += 1;
        }
        if h <= 6 && (k >= len || bytes[k] == b' ' || bytes[k] == b'\t') {
            tokens.push(tok(TokenKind::Operator, cs, k)); // the `#` run
            if k < len {
                tokens.push(tok(TokenKind::Keyword, k, len)); // heading text
            }
            return (tokens, LineState::Code);
        }
        // e.g. `#tag` (no space) or 7+ `#` → not a heading; fall through.
    }

    // ── Blockquote ───────────────────────────────────────────────────────────
    if first == b'>' {
        let mut k = cs + 1;
        while k < len && (bytes[k] == b' ' || bytes[k] == b'\t') {
            k += 1;
        }
        tokens.push(tok(TokenKind::Comment, cs, k)); // `>` + following spaces
        inline(line, k, &mut tokens);
        return (tokens, LineState::Code);
    }

    // ── Bullet list marker (`- ` / `* ` / `+ `) ──────────────────────────────
    if matches!(first, b'-' | b'*' | b'+')
        && cs + 1 < len
        && (bytes[cs + 1] == b' ' || bytes[cs + 1] == b'\t')
    {
        tokens.push(tok(TokenKind::Operator, cs, cs + 1)); // the bullet
        inline(line, cs + 1, &mut tokens);
        return (tokens, LineState::Code);
    }

    // ── Ordered list marker (`1. ` / `1) `) ──────────────────────────────────
    if first.is_ascii_digit() {
        let mut k = cs;
        while k < len && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k < len
            && (bytes[k] == b'.' || bytes[k] == b')')
            && (k + 1 >= len || bytes[k + 1] == b' ' || bytes[k + 1] == b'\t')
        {
            tokens.push(tok(TokenKind::Number, cs, k)); // the digits
            tokens.push(tok(TokenKind::Punctuation, k, k + 1)); // `.` or `)`
            inline(line, k + 1, &mut tokens);
            return (tokens, LineState::Code);
        }
        // e.g. `1.5` or `2024 ...` → not a list marker; fall through.
    }

    // ── Ordinary paragraph text ──────────────────────────────────────────────
    inline(line, cs, &mut tokens);
    (tokens, LineState::Code)
}

/// A thematic break is a line whose content is only `-`, `*`, or `_`
/// (a single kind, 3+ of them) optionally separated by spaces/tabs.
fn is_thematic_break(bytes: &[u8], cs: usize, len: usize) -> bool {
    let mut marker = 0u8;
    let mut count = 0usize;
    let mut k = cs;
    while k < len {
        match bytes[k] {
            b' ' | b'\t' => {}
            c @ (b'-' | b'*' | b'_') => {
                if marker == 0 {
                    marker = c;
                } else if c != marker {
                    return false;
                }
                count += 1;
            }
            _ => return false,
        }
        k += 1;
    }
    count >= 3
}

// ── Inline tokenizer ────────────────────────────────────────────────────────

/// Tokenize the inline span `line[start..]`, appending to `tokens`.
///
/// Every branch advances by at least one byte; runs stop only on ASCII
/// "special" bytes, so multi-byte UTF-8 sequences are always consumed whole
/// and every span lands on a char boundary.
fn inline(line: &str, start: usize, tokens: &mut Vec<Token>) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = start;

    while i < len {
        match bytes[i] {
            // ── Inline code span (run-length matched) ────────────────────────
            b'`' => {
                let open = i;
                let mut n = 0usize;
                while i < len && bytes[i] == b'`' {
                    i += 1;
                    n += 1;
                }
                // Search for a closing run of exactly `n` backticks.
                let mut j = i;
                let mut close = None;
                while j < len {
                    if bytes[j] == b'`' {
                        let mut m = 0usize;
                        while j < len && bytes[j] == b'`' {
                            j += 1;
                            m += 1;
                        }
                        if m == n {
                            close = Some(j);
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                match close {
                    Some(end) => {
                        tokens.push(tok(TokenKind::String, open, end));
                        i = end;
                    }
                    None => {
                        // Unclosed — the opening run is plain punctuation.
                        tokens.push(tok(TokenKind::Punctuation, open, i));
                    }
                }
            }

            // ── Emphasis: bold (`**`/`__`) or italic (`*`/`_`) ───────────────
            d @ (b'*' | b'_') => {
                if i + 1 < len && bytes[i + 1] == d {
                    // Bold — find the closing double delimiter.
                    let open = i;
                    let mut j = i + 2;
                    let mut found = None;
                    while j + 1 < len {
                        if bytes[j] == d && bytes[j + 1] == d {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 2));
                            i = cl + 2;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 2));
                            i = open + 2;
                        }
                    }
                } else {
                    // Italic — find the closing single delimiter.
                    let open = i;
                    let mut j = i + 1;
                    let mut found = None;
                    while j < len {
                        if bytes[j] == d {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 1));
                            i = cl + 1;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 1));
                            i = open + 1;
                        }
                    }
                }
            }

            // ── Strikethrough (`~~…~~`) ──────────────────────────────────────
            b'~' => {
                if i + 1 < len && bytes[i + 1] == b'~' {
                    let open = i;
                    let mut j = i + 2;
                    let mut found = None;
                    while j + 1 < len {
                        if bytes[j] == b'~' && bytes[j + 1] == b'~' {
                            found = Some(j);
                            break;
                        }
                        j += 1;
                    }
                    match found {
                        Some(cl) => {
                            tokens.push(tok(TokenKind::String, open, cl + 2));
                            i = cl + 2;
                        }
                        None => {
                            tokens.push(tok(TokenKind::Punctuation, open, open + 2));
                            i = open + 2;
                        }
                    }
                } else {
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Link `[text](url)` ───────────────────────────────────────────
            b'[' => {
                if let Some((cb, cp)) = link_bounds(bytes, len, i) {
                    push_link(tokens, i, cb, cp);
                    i = cp + 1;
                } else {
                    tokens.push(tok(TokenKind::Punctuation, i, i + 1));
                    i += 1;
                }
            }

            // ── Image `![alt](url)` ──────────────────────────────────────────
            b'!' => {
                if i + 1 < len
                    && bytes[i + 1] == b'['
                    && let Some((cb, cp)) = link_bounds(bytes, len, i + 1)
                {
                    tokens.push(tok(TokenKind::Punctuation, i, i + 1)); // `!`
                    push_link(tokens, i + 1, cb, cp);
                    i = cp + 1;
                } else {
                    // Lone `!`.
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Autolink `<http://…>` / `<user@host>` ────────────────────────
            b'<' => {
                let open = i;
                let mut j = i + 1;
                while j < len && bytes[j] != b'>' {
                    j += 1;
                }
                if j < len {
                    let inner = &line[open + 1..j];
                    if inner.contains("://") || (inner.contains('@') && !inner.contains(' ')) {
                        tokens.push(tok(TokenKind::String, open, j + 1));
                        i = j + 1;
                        continue;
                    }
                }
                // Not an autolink (e.g. an HTML tag `<b>`): lone `<`.
                tokens.push(tok(TokenKind::Punctuation, open, open + 1));
                i = open + 1;
            }

            // ── Backslash escape ─────────────────────────────────────────────
            b'\\' => {
                if i + 1 < len {
                    let nb = bytes[i + 1];
                    let adv = if nb < 0x80 {
                        2
                    } else {
                        1 + line[i + 1..].chars().next().map_or(1, |c| c.len_utf8())
                    };
                    tokens.push(tok(TokenKind::Identifier, i, i + adv));
                    i += adv;
                } else {
                    tokens.push(tok(TokenKind::Identifier, i, i + 1));
                    i += 1;
                }
            }

            // ── Plain run (stops at the next inline-special ASCII byte) ───────
            _ => {
                let run_start = i;
                while i < len
                    && !matches!(
                        bytes[i],
                        b'`' | b'*' | b'_' | b'~' | b'[' | b'!' | b'<' | b'\\'
                    )
                {
                    i += 1;
                }
                tokens.push(tok(TokenKind::Identifier, run_start, i));
            }
        }
    }
}

/// Return `(close_bracket, close_paren)` byte offsets if a full inline link
/// `[…](…)` starts at `open_bracket` (which must be a `[`), else `None`.
fn link_bounds(bytes: &[u8], len: usize, open_bracket: usize) -> Option<(usize, usize)> {
    let mut j = open_bracket + 1;
    while j < len && bytes[j] != b']' {
        j += 1;
    }
    if j >= len {
        return None; // no closing `]`
    }
    if j + 1 >= len || bytes[j + 1] != b'(' {
        return None; // `]` not immediately followed by `(`
    }
    let mut k = j + 2;
    while k < len && bytes[k] != b')' {
        k += 1;
    }
    if k >= len {
        return None; // no closing `)`
    }
    Some((j, k))
}

/// Push the six-part token sequence for a link/image body whose `[` is at
/// `open_bracket`, `]` at `cb`, and `)` at `cp` (with `(` at `cb + 1`).
fn push_link(tokens: &mut Vec<Token>, open_bracket: usize, cb: usize, cp: usize) {
    tokens.push(tok(TokenKind::Punctuation, open_bracket, open_bracket + 1)); // `[`
    if cb > open_bracket + 1 {
        tokens.push(tok(TokenKind::Identifier, open_bracket + 1, cb)); // text / alt
    }
    tokens.push(tok(TokenKind::Punctuation, cb, cb + 1)); // `]`
    tokens.push(tok(TokenKind::Punctuation, cb + 1, cb + 2)); // `(`
    if cp > cb + 2 {
        tokens.push(tok(TokenKind::String, cb + 2, cp)); // url
    }
    tokens.push(tok(TokenKind::Punctuation, cp, cp + 1)); // `)`
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
                .any(|t| t.kind == TokenKind::Identifier
                    && &line[t.start..t.start + t.len] == "text"),
            "link text should be Identifier: {toks:?}"
        );
        assert!(
            toks.iter()
                .any(|t| t.kind == TokenKind::String
                    && &line[t.start..t.start + t.len] == "http://x"),
            "link url should be String: {toks:?}"
        );
    }

    #[test]
    fn image_and_list_and_quote() {
        // Image alt vs url.
        let img = "![alt](pic.png)";
        let (it, _) = md(img, LineState::Code);
        assert_tiles(img, &it);
        assert!(it.iter().any(|t| t.kind == TokenKind::String
            && &img[t.start..t.start + t.len] == "pic.png"));

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
}
