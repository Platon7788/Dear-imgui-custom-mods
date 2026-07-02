//! Assembly line tokenizer — the state machine that walks one line and
//! classifies registers, mnemonics, directives, labels, immediates,
//! numbers, strings and comments.

use super::tables::{is_directive, is_mnemonic, is_register};
use super::*;

/// Extended ident: allows `.` for GAS local labels / directives.
fn is_asm_ident_continue(b: u8) -> bool {
    is_ident_continue(b) || b == b'.'
}

pub(in crate::code_editor::lang) fn tokenize(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            scan_ws(&mut tokens, bytes, &mut i);
            continue;
        }

        // ── Comments: ; (Intel) or # (AT&T) or // (GAS alternate) ────────
        if b == b';' || b == b'#' || (b == b'/' && i + 1 < len && bytes[i + 1] == b'/') {
            tokens.push(Token {
                kind: TokenKind::Comment,
                start: i,
                len: len - i,
            });
            return tokens;
        }

        // ── C-style block comment /* */ (used by some assemblers) ─────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            scan_until(bytes, &mut i, b"*/");
            tokens.push(Token {
                kind: TokenKind::Comment,
                start,
                len: i - start,
            });
            continue;
        }

        // ── String literal ───────────────────────────────────────────────
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: TokenKind::String,
                start,
                len: i - start,
            });
            continue;
        }

        // ── %word: NASM directive (%define) or AT&T register (%rax) ─────
        if b == b'%' && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if is_directive(word) {
                TokenKind::Attribute // NASM preprocessor: %define, %macro, …
            } else {
                TokenKind::TypeName // AT&T register: %rax, %eax, …
            };
            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // ── AT&T immediate ($42, $-1, $0xFF, $symbol) ────────────────────
        if b == b'$' {
            let start = i;
            i += 1;
            if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'-' || bytes[i] == b'+') {
                if bytes[i] == b'-' || bytes[i] == b'+' {
                    i += 1;
                }
                consume_number(&mut i, bytes);
                tokens.push(Token {
                    kind: TokenKind::Number,
                    start,
                    len: i - start,
                });
            } else if i < len && is_ident_start(bytes[i]) {
                while i < len && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    start,
                    len: i - start,
                });
            } else {
                tokens.push(Token {
                    kind: TokenKind::Operator,
                    start,
                    len: 1,
                });
            }
            continue;
        }

        // ── Number: decimal, hex (0x / 0Fh), binary (0b), octal (0o) ────
        if b.is_ascii_digit() || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'-' {
                i += 1;
            }
            consume_number(&mut i, bytes);
            // NASM-style hex suffix: 0FFh / 1010b / 17o / 25d
            if i < len
                && matches!(
                    bytes[i],
                    b'h' | b'H' | b'b' | b'B' | b'o' | b'O' | b'd' | b'D'
                )
            {
                // Only treat the suffix letter as part of the number if it
                // isn't followed by another ident byte (so `5dword` doesn't
                // eat the `d`).
                if i + 1 >= len || !is_ident_continue(bytes[i + 1]) {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                start,
                len: i - start,
            });
            continue;
        }

        // ── GAS directive (.text, .globl, .cfi_startproc) ────────────────
        if b == b'.' && i + 1 < len && (is_ident_start(bytes[i + 1]) || bytes[i + 1] == b'.') {
            let start = i;
            i += 1;
            while i < len && is_asm_ident_continue(bytes[i]) {
                i += 1;
            }
            // GAS local label definition `.Lfoo:` keeps the colon so it
            // reads as a label rather than a directive.
            if i < len && bytes[i] == b':' {
                i += 1;
                tokens.push(Token {
                    kind: TokenKind::MacroCall,
                    start,
                    len: i - start,
                });
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Attribute,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Identifier / mnemonic / register / label / directive ─────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }

            // Label: identifier followed by `:`
            if i < len && bytes[i] == b':' {
                i += 1; // include the colon
                tokens.push(Token {
                    kind: TokenKind::MacroCall,
                    start,
                    len: i - start,
                });
                continue;
            }

            let word = &line[start..i];

            // Case-insensitive matching for registers and mnemonics.
            let word_lower: String;
            let word_lc = if word.bytes().any(|c| c.is_ascii_uppercase()) {
                word_lower = word.to_ascii_lowercase();
                word_lower.as_str()
            } else {
                word
            };

            let kind = if is_register(word_lc) {
                TokenKind::TypeName
            } else if is_mnemonic(word_lc) {
                TokenKind::Keyword
            } else if is_directive(word) || is_directive(word_lc) {
                TokenKind::Attribute
            } else {
                TokenKind::Identifier
            };

            tokens.push(Token {
                kind,
                start,
                len: i - start,
            });
            continue;
        }

        // ── Operators ────────────────────────────────────────────────────
        if matches!(
            b,
            b'+' | b'-' | b'*' | b'/' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' | b'~'
        ) {
            tokens.push(Token {
                kind: TokenKind::Operator,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(
            b,
            b'(' | b')' | b'[' | b']' | b'{' | b'}' | b':' | b',' | b'.' | b'@'
        ) {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                start: i,
                len: 1,
            });
            i += 1;
            continue;
        }

        // ── Fallback ─────────────────────────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token {
            kind: TokenKind::Identifier,
            start: i,
            len: ch_len,
        });
        i += ch_len;
    }

    tokens
}

/// Consume a number: decimal, hex (0x), binary (0b), octal (0o/0).
///
/// **Why this isn't migrated to `super::consume_number`** (the shared
/// helper): assembly has the NASM / MASM-style hex literal `0FFh` —
/// digit-prefix then `h` suffix — which the body loop below accepts (it
/// eats hex digits regardless of the prefix). The shared helper is
/// strictly digit-only on the decimal branch and would split `0FFh` into
/// `Number(0) + Identifier(FFh)`. ASM also tolerates `_` in hex bodies
/// without a `0x` prefix because some macros emit underscored labels.
fn consume_number(i: &mut usize, bytes: &[u8]) {
    let len = bytes.len();
    if *i >= len {
        return;
    }

    if bytes[*i] == b'0' && *i + 1 < len {
        match bytes[*i + 1] {
            b'x' | b'X' => {
                *i += 2;
                while *i < len && (bytes[*i].is_ascii_hexdigit() || bytes[*i] == b'_') {
                    *i += 1;
                }
                return;
            }
            // `0b` is binary only when followed by a binary digit;
            // otherwise (`0byte`) it's a NASM-hex/decimal body that falls
            // through to the loop below.
            b'b' | b'B' if *i + 2 < len && (bytes[*i + 2] == b'0' || bytes[*i + 2] == b'1') => {
                *i += 2;
                while *i < len && (bytes[*i] == b'0' || bytes[*i] == b'1' || bytes[*i] == b'_') {
                    *i += 1;
                }
                return;
            }
            b'o' | b'O' => {
                *i += 2;
                while *i < len && ((bytes[*i] >= b'0' && bytes[*i] <= b'7') || bytes[*i] == b'_') {
                    *i += 1;
                }
                return;
            }
            _ => {}
        }
    }

    // Decimal or NASM-style hex (0FFh — starts with digit, ends with h)
    while *i < len && (bytes[*i].is_ascii_hexdigit() || bytes[*i] == b'_') {
        *i += 1;
    }

    // Decimal point (for floating-point literals in some assemblers)
    if *i < len && bytes[*i] == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit() {
        *i += 1;
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
    }
}
