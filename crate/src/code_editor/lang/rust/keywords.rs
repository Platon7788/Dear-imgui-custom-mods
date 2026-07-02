//! Rust keyword and built-in-type tables.
//!
//! Pure data — the tokenizer in [`super::tokenize`] looks these up to
//! classify identifiers. Kept in their own module so the table churn
//! doesn't bloat the tokenizer's diff history.

pub(super) const KEYWORDS: &[&str] = &[
    // Stable
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "yield",
    "union",
    "macro_rules",
    // Reserved
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "virtual",
];

pub(super) const BUILTIN_TYPES: &[&str] = &[
    // Primitives
    "bool",
    "char",
    "f16",
    "f32",
    "f64",
    "f128",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "str",
    "never",
    // Heap / smart pointers
    "String",
    "Box",
    "Rc",
    "Arc",
    "Weak",
    // Collections
    "Vec",
    "VecDeque",
    "LinkedList",
    "HashMap",
    "BTreeMap",
    "IndexMap",
    "HashSet",
    "BTreeSet",
    "IndexSet",
    "BinaryHeap",
    // Option / Result
    "Option",
    "Result",
    // Sync
    "Cell",
    "RefCell",
    "Mutex",
    "RwLock",
    "MutexGuard",
    "RwLockReadGuard",
    "RwLockWriteGuard",
    "Atomic",
    "AtomicBool",
    "AtomicI32",
    "AtomicU32",
    "AtomicI64",
    "AtomicU64",
    "AtomicUsize",
    // Pointers
    "Pin",
    "NonNull",
    "MaybeUninit",
    "ManuallyDrop",
    // Borrowed
    "Cow",
    "Ref",
    "RefMut",
    // Strings / paths
    "OsStr",
    "OsString",
    "CStr",
    "CString",
    "Path",
    "PathBuf",
    // Ranges
    "Range",
    "RangeInclusive",
    "RangeFull",
    "RangeFrom",
    "RangeTo",
    "RangeToInclusive",
    // Time
    "Duration",
    "Instant",
    "SystemTime",
    // I/O
    "File",
    "BufReader",
    "BufWriter",
    "Cursor",
    // Error
    "Error",
    "Infallible",
    // Misc
    "Ordering",
    "Formatter",
    "Thread",
    "JoinHandle",
    // Common enum variants
    "Some",
    "None",
    "Ok",
    "Err",
];

use std::collections::HashSet;
use std::sync::OnceLock;

/// [`KEYWORDS`] as a hash set — one hash + probe per identifier instead of a
/// linear scan over ~50 `&str`s (mirrors `asm/tables.rs`). Built once, lazily.
pub(super) fn keywords_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| KEYWORDS.iter().copied().collect())
}

/// [`BUILTIN_TYPES`] as a hash set (~90 entries; the hottest linear scan).
pub(super) fn builtin_types_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| BUILTIN_TYPES.iter().copied().collect())
}
