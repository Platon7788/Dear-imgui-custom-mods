//! Rust / RON syntax tokenizer.

use super::{consume_decimal, is_ident_continue, is_ident_start};
use crate::code_editor::config::SyntaxDefinition;
use crate::code_editor::token::{Token, TokenKind};

// ── Keywords ────────────────────────────────────────────────────────────────

const KEYWORDS: &[&str] = &[
    // Stable
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield", "macro_rules",
    // Reserved
    "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual",
];

const BUILTIN_TYPES: &[&str] = &[
    // Primitives
    "bool", "char", "f32", "f64",
    "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize",
    "str", "never",
    // Heap / smart pointers
    "String", "Box", "Rc", "Arc", "Weak",
    // Collections
    "Vec", "VecDeque", "LinkedList",
    "HashMap", "BTreeMap", "IndexMap",
    "HashSet", "BTreeSet", "IndexSet",
    // Option / Result
    "Option", "Result",
    // Sync
    "Cell", "RefCell", "Mutex", "RwLock", "MutexGuard", "RwLockReadGuard",
    "RwLockWriteGuard", "Atomic", "AtomicBool", "AtomicI32", "AtomicU32",
    "AtomicI64", "AtomicU64", "AtomicUsize",
    // Pointers
    "Pin", "NonNull", "MaybeUninit", "ManuallyDrop",
    // Borrowed
    "Cow", "Ref", "RefMut",
    // Strings / paths
    "OsStr", "OsString", "CStr", "CString", "Path", "PathBuf",
    // Ranges
    "Range", "RangeInclusive", "RangeFull", "RangeFrom", "RangeTo",
    "RangeToInclusive",
    // Time
    "Duration", "Instant", "SystemTime",
    // I/O
    "File", "BufReader", "BufWriter", "Cursor",
    // Error
    "Error", "Infallible",
    // Misc
    "Ordering", "Formatter", "Thread", "JoinHandle",
    // Common enum variants
    "Some", "None", "Ok", "Err",
];

// ── Language definition ─────────────────────────────────────────────────────

pub struct RustLang;

impl SyntaxDefinition for RustLang {
    fn name(&self) -> &str { "Rust" }

    fn tokenize_line(&self, line: &str, in_block_comment: bool) -> (Vec<Token>, bool) {
        tokenize(line, in_block_comment)
    }

    fn line_comment_prefix(&self) -> Option<&str> { Some("//") }
    fn block_comment_delimiters(&self) -> Option<(&str, &str)> { Some(("/*", "*/")) }

    fn bracket_pairs(&self) -> &[(char, char)] {
        &[('(', ')'), ('{', '}'), ('[', ']')]
    }

    fn auto_indent_after(&self) -> &[char] { &['{'] }
    fn auto_dedent_on(&self) -> &[char] { &['}'] }

    fn auto_close_pairs(&self) -> &[(&str, &str)] {
        &[("(", ")"), ("{", "}"), ("[", "]"), ("\"", "\""), ("'", "'")]
    }
}

// ── Tokenizer ───────────────────────────────────────────────────────────────

fn tokenize(line: &str, mut in_block_comment: bool) -> (Vec<Token>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    // USER CODE markers — whole-line tokens
    {
        let trimmed = line.trim();
        if trimmed.starts_with("// USER CODE BEGIN") || trimmed.starts_with("// USER CODE END") {
            tokens.push(Token { kind: TokenKind::UserCodeMarker, start: 0, len: line.len() });
            return (tokens, in_block_comment);
        }
    }

    while i < len {
        // ── Inside block comment ─────────────────────────────────────────
        if in_block_comment {
            let start = i;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
            tokens.push(Token { kind: TokenKind::Whitespace, start, len: i - start });
            continue;
        }

        // ── Line comment ─────────────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            tokens.push(Token { kind: TokenKind::Comment, start: i, len: len - i });
            return (tokens, in_block_comment);
        }

        // ── Block comment start ──────────────────────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            in_block_comment = true;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_block_comment = false;
                    break;
                }
                i += 1;
                if i >= len { break; }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        // ── Attribute ────────────────────────────────────────────────────
        if b == b'#' && i + 1 < len && (bytes[i + 1] == b'[' || bytes[i + 1] == b'!') {
            let start = i;
            let mut depth = 0u32;
            while i < len {
                match bytes[i] {
                    b'[' => depth += 1,
                    b']' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 { i += 1; break; }
                    }
                    _ => {}
                }
                i += 1;
            }
            tokens.push(Token { kind: TokenKind::Attribute, start, len: i - start });
            continue;
        }

        // ── String literal ───────────────────────────────────────────────
        if b == b'"' {
            let start = i;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token { kind: TokenKind::String, start, len: i - start });
            continue;
        }

        // ── Raw string (r"..." or r#"..."#) ──────────────────────────────
        if b == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let start = i;
            i += 1;
            let mut hashes = 0usize;
            while i < len && bytes[i] == b'#' { hashes += 1; i += 1; }
            if i < len && bytes[i] == b'"' {
                i += 1;
                'raw: loop {
                    if i >= len { break; }
                    if bytes[i] == b'"' {
                        let mut end_hashes = 0;
                        let mut j = i + 1;
                        while j < len && bytes[j] == b'#' && end_hashes < hashes {
                            end_hashes += 1;
                            j += 1;
                        }
                        if end_hashes == hashes { i = j; break 'raw; }
                    }
                    i += 1;
                }
                tokens.push(Token { kind: TokenKind::String, start, len: i - start });
                continue;
            }
            i = start; // not a raw string — fall through
        }

        // ── Char literal ─────────────────────────────────────────────────
        if b == b'\'' && i + 1 < len && bytes[i + 1] != b'\'' {
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'\\' {
                i += 1;
                if i < len { i += 1; }
            } else if i < len {
                i += 1;
            }
            if i < len && bytes[i] == b'\'' {
                i += 1;
                tokens.push(Token { kind: TokenKind::CharLit, start, len: i - start });
                continue;
            }
            i = start;
        }

        // ── Lifetime ─────────────────────────────────────────────────────
        if b == b'\'' && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) { i += 1; }
            tokens.push(Token { kind: TokenKind::Lifetime, start, len: i - start });
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'0' && i + 1 < len {
                match bytes[i + 1] {
                    b'x' | b'X' => {
                        i += 2;
                        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') { i += 1; }
                    }
                    b'b' | b'B' => {
                        i += 2;
                        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') { i += 1; }
                    }
                    b'o' | b'O' => {
                        i += 2;
                        while i < len && ((bytes[i] >= b'0' && bytes[i] <= b'7') || bytes[i] == b'_') { i += 1; }
                    }
                    _ => consume_decimal(&mut i, bytes),
                }
            } else {
                consume_decimal(&mut i, bytes);
            }
            // Type suffix
            if i < len && is_ident_start(bytes[i]) {
                while i < len && is_ident_continue(bytes[i]) { i += 1; }
            }
            tokens.push(Token { kind: TokenKind::Number, start, len: i - start });
            continue;
        }

        // ── Identifier / Keyword / Type / Macro ──────────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) { i += 1; }
            let word = &line[start..i];
            if i < len && bytes[i] == b'!' && !KEYWORDS.contains(&word) {
                i += 1;
                tokens.push(Token { kind: TokenKind::MacroCall, start, len: i - start });
                continue;
            }
            let kind = if KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if BUILTIN_TYPES.contains(&word)
                || word.chars().next().is_some_and(|c| c.is_uppercase())
            {
                TokenKind::TypeName
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token { kind, start, len: i - start });
            continue;
        }

        // ── Range operators (.., ..=) ────────────────────────────────────
        if b == b'.' && i + 1 < len && bytes[i + 1] == b'.' {
            let start = i;
            i += 2;
            if i < len && bytes[i] == b'=' { i += 1; }
            tokens.push(Token { kind: TokenKind::Operator, start, len: i - start });
            continue;
        }

        // ── Operators ────────────────────────────────────────────────────
        if matches!(b, b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' |
                        b'<' | b'>' | b'&' | b'|' | b'^' | b'~') {
            let start = i;
            i += 1;
            if i < len && matches!((b, bytes[i]),
                (b'=', b'=') | (b'!', b'=') | (b'<', b'=') | (b'>', b'=') |
                (b'-', b'>') | (b'=', b'>') | (b'&', b'&') | (b'|', b'|') |
                (b'<', b'<') | (b'>', b'>') | (b'+', b'=') | (b'-', b'=') |
                (b'*', b'=') | (b'/', b'=') | (b'%', b'=') | (b'&', b'=') |
                (b'|', b'=') | (b'^', b'='))
            {
                i += 1;
            }
            tokens.push(Token { kind: TokenKind::Operator, start, len: i - start });
            continue;
        }

        // ── Punctuation ──────────────────────────────────────────────────
        if matches!(b, b'(' | b')' | b'{' | b'}' | b'[' | b']' |
                        b';' | b':' | b',' | b'.' | b'@' | b'?' | b'#') {
            tokens.push(Token { kind: TokenKind::Punctuation, start: i, len: 1 });
            i += 1;
            continue;
        }

        // ── Fallback: full Unicode scalar ────────────────────────────────
        let ch_len = line[i..].chars().next().map_or(1, |c| c.len_utf8());
        tokens.push(Token { kind: TokenKind::Identifier, start: i, len: ch_len });
        i += ch_len;
    }

    (tokens, in_block_comment)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_editor::config::Language;
    use crate::code_editor::lang::tokenize_line;

    fn tok(line: &str) -> Vec<(TokenKind, &str)> {
        let (tokens, _) = tokenize_line(line, &Language::Rust, false);
        tokens.iter().map(|t| (t.kind, &line[t.start..t.start + t.len])).collect()
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

    #[test]
    fn attribute() {
        let toks = tok("#[derive(Debug)]");
        assert_eq!(toks[0].0, TokenKind::Attribute);
    }

    #[test]
    fn type_name() {
        let toks = tok("let v: Vec<String> = Vec::new();");
        let types: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::TypeName).collect();
        assert!(types.len() >= 2);
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
}
