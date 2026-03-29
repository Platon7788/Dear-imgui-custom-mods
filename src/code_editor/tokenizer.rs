//! Syntax tokenizer for Rust, TOML, and RON.
//!
//! Produces a flat list of [`Token`]s per line — no AST, no multi-line state
//! beyond block comment tracking. Fast enough for per-frame re-tokenization
//! of visible lines (typically 40–80 lines).

use super::config::Language;

/// Kind of syntax token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    TypeName,
    Lifetime,
    String,
    CharLit,
    Number,
    Comment,
    Attribute,
    MacroCall,
    Operator,
    Punctuation,
    Identifier,
    Whitespace,
    UserCodeMarker,
}

/// A single token: byte range within a line + kind.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offset from start of line.
    pub start: usize,
    /// Byte length.
    pub len: usize,
}

// ── Rust keywords ────────────────────────────────────────────────────────────

const RUST_KEYWORDS: &[&str] = &[
    // Stable keywords
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield", "macro_rules",
    // Reserved for future use
    "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual",
];

const RUST_BUILTIN_TYPES: &[&str] = &[
    // Primitive types
    "bool", "char", "f32", "f64",
    "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize",
    "str", "never",
    // Standard library — heap/smart pointers
    "String", "Box", "Rc", "Arc", "Weak",
    // Collections
    "Vec", "VecDeque", "LinkedList",
    "HashMap", "BTreeMap", "IndexMap",
    "HashSet", "BTreeSet", "IndexSet",
    // Optional / Result
    "Option", "Result",
    // Synchronization
    "Cell", "RefCell", "Mutex", "RwLock", "MutexGuard", "RwLockReadGuard",
    "RwLockWriteGuard", "Atomic", "AtomicBool", "AtomicI32", "AtomicU32",
    "AtomicI64", "AtomicU64", "AtomicUsize",
    // Pointer types
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
    // Error handling
    "Error", "Infallible",
    // Ordering / comparison
    "Ordering",
    // Format / Display helpers
    "Formatter",
    // Thread
    "Thread", "JoinHandle",
    // Common enum variants (highlighted as TypeName via uppercase-first rule,
    // but listed explicitly for clarity and potential future colour separation)
    "Some", "None", "Ok", "Err",
];

const TOML_KEYWORDS: &[&str] = &["true", "false"];

// ── Public API ───────────────────────────────────────────────────────────────

/// Tokenize a single line of source code.
///
/// `in_block_comment` indicates whether we're inside a `/* */` block from
/// a previous line. Returns `(tokens, still_in_block_comment)`.
pub fn tokenize_line(
    line: &str,
    language: Language,
    in_block_comment: bool,
) -> (Vec<Token>, bool) {
    match language {
        Language::None => {
            let tokens = if line.is_empty() {
                vec![]
            } else {
                vec![Token { kind: TokenKind::Identifier, start: 0, len: line.len() }]
            };
            (tokens, false)
        }
        Language::Rust => tokenize_rust(line, in_block_comment),
        Language::Toml => (tokenize_toml(line), false),
        Language::Ron => tokenize_rust(line, in_block_comment), // RON is close enough to Rust
    }
}

// ── Rust tokenizer ───────────────────────────────────────────────────────────

fn tokenize_rust(line: &str, mut in_block_comment: bool) -> (Vec<Token>, bool) {
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
                if i >= len {
                    break;
                }
            }
            tokens.push(Token { kind: TokenKind::Comment, start, len: i - start });
            continue;
        }

        let b = bytes[i];

        // ── Whitespace ───────────────────────────────────────────────────
        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
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
                if i >= len {
                    break;
                }
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
                        if depth == 0 {
                            i += 1;
                            break;
                        }
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
                    i += 2; // skip escaped char
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
            while i < len && bytes[i] == b'#' {
                hashes += 1;
                i += 1;
            }
            if i < len && bytes[i] == b'"' {
                i += 1;
                // Find closing "###
                'raw: loop {
                    if i >= len { break; }
                    if bytes[i] == b'"' {
                        let mut end_hashes = 0;
                        let mut j = i + 1;
                        while j < len && bytes[j] == b'#' && end_hashes < hashes {
                            end_hashes += 1;
                            j += 1;
                        }
                        if end_hashes == hashes {
                            i = j;
                            break 'raw;
                        }
                    }
                    i += 1;
                }
                tokens.push(Token { kind: TokenKind::String, start, len: i - start });
                continue;
            }
            // Not actually a raw string — fall through to identifier
            i = start;
        }

        // ── Char literal ─────────────────────────────────────────────────
        if b == b'\'' && i + 1 < len && bytes[i + 1] != b'\'' {
            // Distinguish from lifetime: 'a vs 'x'
            let start = i;
            i += 1;
            if i < len && bytes[i] == b'\\' {
                i += 1; // skip backslash
                if i < len { i += 1; } // skip escaped char
            } else if i < len {
                i += 1; // the char
            }
            if i < len && bytes[i] == b'\'' {
                i += 1;
                tokens.push(Token { kind: TokenKind::CharLit, start, len: i - start });
                continue;
            }
            // Could be a lifetime
            i = start;
        }

        // ── Lifetime ─────────────────────────────────────────────────────
        if b == b'\'' && i + 1 < len && is_ident_start(bytes[i + 1]) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            tokens.push(Token { kind: TokenKind::Lifetime, start, len: i - start });
            continue;
        }

        // ── Number ───────────────────────────────────────────────────────
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            // Hex/bin/oct prefixes
            if b == b'0' && i + 1 < len {
                match bytes[i + 1] {
                    b'x' | b'X' => {
                        i += 2;
                        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                            i += 1;
                        }
                    }
                    b'b' | b'B' => {
                        i += 2;
                        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
                            i += 1;
                        }
                    }
                    b'o' | b'O' => {
                        i += 2;
                        while i < len && ((bytes[i] >= b'0' && bytes[i] <= b'7') || bytes[i] == b'_') {
                            i += 1;
                        }
                    }
                    _ => {
                        consume_decimal(&mut i, bytes);
                    }
                }
            } else {
                consume_decimal(&mut i, bytes);
            }
            // Type suffix (u32, f64, etc.)
            if i < len && is_ident_start(bytes[i]) {
                while i < len && is_ident_continue(bytes[i]) {
                    i += 1;
                }
            }
            tokens.push(Token { kind: TokenKind::Number, start, len: i - start });
            continue;
        }

        // ── Identifier / Keyword / Type / Macro ──────────────────────────
        if is_ident_start(b) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let word = &line[start..i];
            // Macro call: ident!
            if i < len && bytes[i] == b'!' && !RUST_KEYWORDS.contains(&word) {
                i += 1; // include the !
                tokens.push(Token { kind: TokenKind::MacroCall, start, len: i - start });
                continue;
            }
            let kind = if RUST_KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else if RUST_BUILTIN_TYPES.contains(&word) {
                TokenKind::TypeName
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                TokenKind::TypeName
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token { kind, start, len: i - start });
            continue;
        }

        // ── Range operators (.., ..=, ..n) ──────────────────────────────
        if b == b'.' && i + 1 < len && bytes[i + 1] == b'.' {
            let start = i;
            i += 2;
            if i < len && bytes[i] == b'=' {
                i += 1; // ..=
            }
            tokens.push(Token { kind: TokenKind::Operator, start, len: i - start });
            continue;
        }

        // ── Operators ────────────────────────────────────────────────────
        if matches!(b, b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' |
                        b'<' | b'>' | b'&' | b'|' | b'^' | b'~') {
            let start = i;
            i += 1;
            // Two-char operators
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

        // ── Fallback (unknown byte) ──────────────────────────────────────
        tokens.push(Token { kind: TokenKind::Identifier, start: i, len: 1 });
        i += 1;
    }

    (tokens, in_block_comment)
}

// ── TOML tokenizer ───────────────────────────────────────────────────────────

fn tokenize_toml(line: &str) -> Vec<Token> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        if b == b' ' || b == b'\t' {
            let start = i;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
            tokens.push(Token { kind: TokenKind::Whitespace, start, len: i - start });
            continue;
        }

        // Comment
        if b == b'#' {
            tokens.push(Token { kind: TokenKind::Comment, start: i, len: len - i });
            return tokens;
        }

        // Section headers [section] or [[array]]
        if b == b'[' {
            let start = i;
            while i < len && bytes[i] != b']' { i += 1; }
            if i < len { i += 1; }
            if i < len && bytes[i] == b']' { i += 1; }
            tokens.push(Token { kind: TokenKind::Attribute, start, len: i - start });
            continue;
        }

        // String
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < len { i += 1; }
                i += 1;
            }
            if i < len { i += 1; }
            tokens.push(Token { kind: TokenKind::String, start, len: i - start });
            continue;
        }

        // Number
        if b.is_ascii_digit() || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'-' { i += 1; }
            consume_decimal(&mut i, bytes);
            tokens.push(Token { kind: TokenKind::Number, start, len: i - start });
            continue;
        }

        // Identifier / keyword
        if is_ident_start(b) {
            let start = i;
            while i < len && (is_ident_continue(bytes[i]) || bytes[i] == b'-') { i += 1; }
            let word = &line[start..i];
            let kind = if TOML_KEYWORDS.contains(&word) {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token { kind, start, len: i - start });
            continue;
        }

        // Operators (=)
        if b == b'=' {
            tokens.push(Token { kind: TokenKind::Operator, start: i, len: 1 });
            i += 1;
            continue;
        }

        // Punctuation
        if matches!(b, b'{' | b'}' | b',' | b'.' | b']') {
            tokens.push(Token { kind: TokenKind::Punctuation, start: i, len: 1 });
            i += 1;
            continue;
        }

        tokens.push(Token { kind: TokenKind::Identifier, start: i, len: 1 });
        i += 1;
    }

    tokens
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn consume_decimal(i: &mut usize, bytes: &[u8]) {
    let len = bytes.len();
    while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
        *i += 1;
    }
    // Decimal point
    if *i < len && bytes[*i] == b'.' && *i + 1 < len && bytes[*i + 1].is_ascii_digit() {
        *i += 1;
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
    }
    // Exponent
    if *i < len && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < len && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tok_kinds(line: &str) -> Vec<(TokenKind, &str)> {
        let (tokens, _) = tokenize_line(line, Language::Rust, false);
        tokens.iter().map(|t| (t.kind, &line[t.start..t.start + t.len])).collect()
    }

    #[test]
    fn test_keywords() {
        let toks = tok_kinds("fn main() {");
        assert_eq!(toks[0], (TokenKind::Keyword, "fn"));
        assert_eq!(toks[2], (TokenKind::Identifier, "main"));
    }

    #[test]
    fn test_strings() {
        let toks = tok_kinds(r#"let s = "hello \"world\"";"#);
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
        assert!(strings[0].1.starts_with('"'));
    }

    #[test]
    fn test_line_comment() {
        let toks = tok_kinds("let x = 5; // comment");
        let last = toks.last().unwrap();
        assert_eq!(last.0, TokenKind::Comment);
        assert!(last.1.contains("comment"));
    }

    #[test]
    fn test_block_comment() {
        let (toks, still_in) = tokenize_line("/* start", Language::Rust, false);
        assert!(still_in);
        assert_eq!(toks[0].kind, TokenKind::Comment);

        let (toks2, still_in2) = tokenize_line("middle */code", Language::Rust, true);
        assert!(!still_in2);
        assert_eq!(toks2[0].kind, TokenKind::Comment);
    }

    #[test]
    fn test_numbers() {
        let toks = tok_kinds("let x = 0xFF_u8;");
        let nums: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Number).collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "0xFF_u8");
    }

    #[test]
    fn test_lifetime() {
        let toks = tok_kinds("fn foo<'a>(x: &'a str)");
        let lifetimes: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::Lifetime).collect();
        assert_eq!(lifetimes.len(), 2);
        assert_eq!(lifetimes[0].1, "'a");
    }

    #[test]
    fn test_macro_call() {
        let toks = tok_kinds("println!(\"hi\");");
        assert_eq!(toks[0], (TokenKind::MacroCall, "println!"));
    }

    #[test]
    fn test_attribute() {
        let toks = tok_kinds("#[derive(Debug)]");
        assert_eq!(toks[0].0, TokenKind::Attribute);
    }

    #[test]
    fn test_type_name() {
        let toks = tok_kinds("let v: Vec<String> = Vec::new();");
        let types: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::TypeName).collect();
        assert!(types.len() >= 2); // Vec and String
    }

    #[test]
    fn test_user_code_marker() {
        let toks = tok_kinds("    // USER CODE BEGIN on_click");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, TokenKind::UserCodeMarker);
    }

    #[test]
    fn test_raw_string() {
        let toks = tok_kinds(r###"let s = r#"hello"#;"###);
        let strings: Vec<_> = toks.iter().filter(|t| t.0 == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
    }

    #[test]
    fn test_toml_basic() {
        let (toks, _) = tokenize_line("[package]", Language::Toml, false);
        assert_eq!(toks[0].kind, TokenKind::Attribute);

        let (toks2, _) = tokenize_line("name = \"hello\"", Language::Toml, false);
        let has_string = toks2.iter().any(|t| t.kind == TokenKind::String);
        let has_ident = toks2.iter().any(|t| t.kind == TokenKind::Identifier);
        assert!(has_string);
        assert!(has_ident);
    }

    #[test]
    fn test_empty_line() {
        let (toks, _) = tokenize_line("", Language::Rust, false);
        assert!(toks.is_empty());
    }

    #[test]
    fn test_covers_full_line() {
        let line = "pub fn foo(x: i32) -> bool { true }";
        let (toks, _) = tokenize_line(line, Language::Rust, false);
        // Verify tokens cover every byte
        let total: usize = toks.iter().map(|t| t.len).sum();
        assert_eq!(total, line.len());
    }
}
