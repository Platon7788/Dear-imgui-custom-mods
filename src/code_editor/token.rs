//! Token types — shared between the built-in tokenizers and custom syntax extensions.
//!
//! Kept in its own module so that [`crate::code_editor::config::SyntaxDefinition`]
//! can reference [`Token`] without creating a circular dependency with
//! `tokenizer.rs`.

/// Kind of syntax token — determines the render color applied by the editor.
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

    // ── Hex-mode value-based coloring ────────────────────────────────
    /// Null byte `00` — red, stands out as "empty / zero".
    HexNull,
    /// `FF` byte — amber, stands out as "all bits set".
    HexFF,
    /// Control characters `01–1F`, `7F` and high bytes `80–FE` — silver/default.
    HexDefault,
    /// Printable ASCII `20–7E` — green, the readable data.
    HexPrintable,
}

/// A single token: byte range within a line plus its kind.
///
/// `start` and `len` are **byte** offsets, not char offsets.
/// All slicing must go through [`str::is_char_boundary`] checks.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offset from start of line.
    pub start: usize,
    /// Byte length.
    pub len: usize,
}
