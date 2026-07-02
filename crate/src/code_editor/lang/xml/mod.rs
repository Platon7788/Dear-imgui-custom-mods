//! XML / HTML tokenizer.
//!
//! Tag names → [`TokenKind::Keyword`], attribute names → [`TokenKind::TypeName`],
//! attribute values → [`TokenKind::String`], entity references → [`TokenKind::MacroCall`],
//! processing instructions → [`TokenKind::Attribute`].
//!
//! Multi-line state is threaded through [`LineState`]:
//! * `<!-- -->` comments carry as [`LineState::BlockComment`] (they do not
//!   nest, so the depth is always `1`).
//! * `<script>` / `<style>` **raw-text** bodies carry as
//!   [`LineState::HtmlRaw`]. Inside them a `<` is *not* markup (e.g. `a < b`
//!   in JS stays raw text) until the matching `</script>` / `</style>`.

use super::{is_ident_continue, is_ident_start, scan_until};
use crate::code_editor::config::{LineState, SyntaxDefinition};
use crate::code_editor::token::{Token, TokenKind};

#[cfg(test)]
mod tests;

// ── Language definition ─────────────────────────────────────────────────────

pub struct XmlLang;

impl SyntaxDefinition for XmlLang {
    fn name(&self) -> &str {
        "XML"
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        tokenize(line, state)
    }

    /// XML has no single-line comment syntax.
    fn line_comment_prefix(&self) -> Option<&str> {
        None
    }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> {
        Some(("<!--", "-->"))
    }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']'), ('<', '>')]
    }

    fn auto_indent_after(&self) -> &[char] {
        &['>']
    }
    fn auto_dedent_on(&self) -> &[char] {
        &[]
    }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[
            ("(", ")"),
            ("{", "}"),
            ("[", "]"),
            ("\"", "\""),
            ("'", "'"),
            ("<", ">"),
        ]
    }

    fn is_word_char(&self, c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '.'
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

/// Push one token — keeps the tokenizer body terse while every span still
/// tiles the line exactly.
#[inline]
fn emit(tokens: &mut Vec<Token>, kind: TokenKind, start: usize, len: usize) {
    tokens.push(Token { kind, start, len });
}

/// Byte offset of the matching raw-text close tag (`</script` or `</style`,
/// case-insensitive) at or after `from`, or `None` if the line has none.
///
/// The match must be followed by a non-identifier byte (or end-of-line) so a
/// prefix like `</scripts>` is not mistaken for a `</script>` close.
fn find_raw_close(bytes: &[u8], from: usize, is_style: bool) -> Option<usize> {
    let needle: &[u8] = if is_style { b"</style" } else { b"</script" };
    let nlen = needle.len();
    let len = bytes.len();
    let mut i = from;
    while i + nlen <= len {
        if bytes[i] == b'<'
            && needle
                .iter()
                .enumerate()
                .all(|(k, n)| bytes[i + k].eq_ignore_ascii_case(n))
            && bytes.get(i + nlen).is_none_or(|c| !is_ident_continue(*c))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Tokenize one line, threading multi-line state (see module docs).
fn tokenize(line: &str, state: LineState) -> (Vec<Token>, LineState) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;
    // XML comments do not nest, so this is a plain "inside a comment" flag.
    let mut in_block_comment = matches!(state, LineState::BlockComment(_));
    // `<script>`/`<style>` raw-text body — carried from a previous line, or
    // entered mid-line right after an opening raw-text tag. `Some(is_style)`
    // while active.
    let mut raw: Option<bool> = match state {
        LineState::HtmlRaw { is_style } => Some(is_style),
        _ => None,
    };

    while i < len {
        // ── Inside a <script>/<style> raw-text body ──────────────────────
        // A `<` here is *not* markup; only the matching close tag ends it.
        if let Some(is_style) = raw {
            match find_raw_close(bytes, i, is_style) {
                Some(p) => {
                    if p > i {
                        emit(&mut tokens, TokenKind::String, i, p - i);
                    }
                    i = p;
                    raw = None; // fall through: main loop tokenizes the close tag
                    continue;
                }
                None => {
                    if len > i {
                        emit(&mut tokens, TokenKind::String, i, len - i);
                    }
                    i = len; // stays in `raw` — reported at end of line
                    continue;
                }
            }
        }

        // ── Inside XML comment <!-- ... --> ───────────────────────────────
        if in_block_comment {
            let start = i;
            if scan_until(bytes, &mut i, b"-->") {
                in_block_comment = false;
            }
            emit(&mut tokens, TokenKind::Comment, start, i - start);
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            emit(&mut tokens, TokenKind::Whitespace, start, i - start);
            continue;
        }

        // ── Comment start <!-- ───────────────────────────────────────────
        if b == b'<'
            && i + 3 < len
            && bytes[i + 1] == b'!'
            && bytes[i + 2] == b'-'
            && bytes[i + 3] == b'-'
        {
            let start = i;
            i += 4;
            in_block_comment = !scan_until(bytes, &mut i, b"-->");
            emit(&mut tokens, TokenKind::Comment, start, i - start);
            continue;
        }

        // ── CDATA <![CDATA[...]]> ────────────────────────────────────────
        // Byte-compare (not `&line[i..i+9]`) so a multi-byte codepoint
        // straddling the window can never trigger a non-char-boundary
        // slicing panic.
        if b == b'<' && i + 9 <= len && &bytes[i..i + 9] == b"<![CDATA[" {
            let start = i;
            i += 9;
            scan_until(bytes, &mut i, b"]]>");
            emit(&mut tokens, TokenKind::String, start, i - start);
            continue;
        }

        // ── Processing instruction <?...?> ───────────────────────────────
        if b == b'<' && i + 1 < len && bytes[i + 1] == b'?' {
            let start = i;
            i += 2;
            scan_until(bytes, &mut i, b"?>");
            emit(&mut tokens, TokenKind::Attribute, start, i - start);
            continue;
        }

        // ── DOCTYPE / other declarations <!...> ──────────────────────────
        if b == b'<' && i + 1 < len && bytes[i + 1] == b'!' {
            let start = i;
            let mut depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'<' => depth += 1,
                    b'>' => {
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
            emit(&mut tokens, TokenKind::Attribute, start, i - start);
            continue;
        }

        // ── Tag (open, close, self-closing) ──────────────────────────────
        if b == b'<' {
            let start = i;
            i += 1;
            let mut is_close_tag = false;
            if i < len && bytes[i] == b'/' {
                i += 1;
                is_close_tag = true;
            }
            emit(&mut tokens, TokenKind::Punctuation, start, i - start);

            // Tag name
            let mut tag_name: Option<(usize, usize)> = None;
            if i < len && (is_ident_start(bytes[i]) || bytes[i] == b':') {
                let name_start = i;
                while i < len
                    && (is_ident_continue(bytes[i])
                        || bytes[i] == b'-'
                        || bytes[i] == b':'
                        || bytes[i] == b'.')
                {
                    i += 1;
                }
                emit(&mut tokens, TokenKind::Keyword, name_start, i - name_start);
                tag_name = Some((name_start, i));
            }

            // Attributes
            let mut just_saw_eq = false;
            let mut self_closed = false;
            while i < len && bytes[i] != b'>' {
                // Whitespace — does NOT reset `just_saw_eq` (`type = text`).
                if matches!(bytes[i], b' ' | b'\t' | b'\n') {
                    let ws = i;
                    while i < len && matches!(bytes[i], b' ' | b'\t' | b'\n') {
                        i += 1;
                    }
                    emit(&mut tokens, TokenKind::Whitespace, ws, i - ws);
                    continue;
                }

                // Self-close `/>`
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>' {
                    emit(&mut tokens, TokenKind::Punctuation, i, 2);
                    i += 2;
                    self_closed = true;
                    break;
                }

                // `=`
                if bytes[i] == b'=' {
                    emit(&mut tokens, TokenKind::Operator, i, 1);
                    i += 1;
                    just_saw_eq = true;
                    continue;
                }

                // Attribute value (quoted) — split out `&entity;` refs so they
                // colour distinctly while the segments still tile the value.
                if bytes[i] == b'"' || bytes[i] == b'\'' {
                    let quote = bytes[i];
                    let val_start = i;
                    i += 1;
                    let mut seg_start = val_start;
                    while i < len && bytes[i] != quote {
                        if bytes[i] == b'&' {
                            if i > seg_start {
                                emit(&mut tokens, TokenKind::String, seg_start, i - seg_start);
                            }
                            let ent = i;
                            i += 1;
                            while i < len
                                && !matches!(bytes[i], b';' | b'&' | b'<')
                                && bytes[i] != quote
                            {
                                i += 1;
                            }
                            if i < len && bytes[i] == b';' {
                                i += 1;
                            }
                            emit(&mut tokens, TokenKind::MacroCall, ent, i - ent);
                            seg_start = i;
                        } else {
                            i += 1;
                        }
                    }
                    if i < len {
                        i += 1; // closing quote
                    }
                    if i > seg_start {
                        emit(&mut tokens, TokenKind::String, seg_start, i - seg_start);
                    }
                    just_saw_eq = false;
                    continue;
                }

                // Unquoted attribute value (after `=`): `<input type=text>`.
                if just_saw_eq {
                    let vs = i;
                    while i < len
                        && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'>')
                        && !(bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>')
                    {
                        i += 1;
                    }
                    if i > vs {
                        emit(&mut tokens, TokenKind::String, vs, i - vs);
                    }
                    just_saw_eq = false;
                    continue;
                }

                // Attribute name
                if is_ident_start(bytes[i]) || bytes[i] == b':' {
                    let attr_start = i;
                    while i < len
                        && (is_ident_continue(bytes[i]) || bytes[i] == b'-' || bytes[i] == b':')
                    {
                        i += 1;
                    }
                    emit(&mut tokens, TokenKind::TypeName, attr_start, i - attr_start);
                    continue;
                }

                // Unknown char inside tag — skip one codepoint.
                let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
                emit(&mut tokens, TokenKind::Identifier, i, ch_len);
                i += ch_len;
            }

            // Closing `>`
            let mut closed_gt = false;
            if i < len && bytes[i] == b'>' {
                emit(&mut tokens, TokenKind::Punctuation, i, 1);
                i += 1;
                closed_gt = true;
            }

            // Enter raw-text mode: an opening `<script>`/`<style>` that closed
            // with a plain `>` on this line makes the rest of the body raw.
            if closed_gt
                && !is_close_tag
                && !self_closed
                && let Some((s, e)) = tag_name
            {
                let name = &line[s..e];
                if name.eq_ignore_ascii_case("script") {
                    raw = Some(false);
                } else if name.eq_ignore_ascii_case("style") {
                    raw = Some(true);
                }
            }
            continue;
        }

        // ── Entity reference (&amp; etc.) ────────────────────────────────
        if b == b'&' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b';' && bytes[i] != b' ' && bytes[i] != b'<' {
                i += 1;
            }
            if i < len && bytes[i] == b';' {
                i += 1;
            }
            emit(&mut tokens, TokenKind::MacroCall, start, i - start);
            continue;
        }

        // ── Text content ─────────────────────────────────────────────────
        {
            let start = i;
            while i < len && bytes[i] != b'<' && bytes[i] != b'&' {
                i += 1;
            }
            if i > start {
                emit(&mut tokens, TokenKind::Identifier, start, i - start);
            }
        }
    }

    let end = if in_block_comment {
        LineState::BlockComment(1)
    } else if let Some(is_style) = raw {
        LineState::HtmlRaw { is_style }
    } else {
        LineState::Code
    };
    (tokens, end)
}
