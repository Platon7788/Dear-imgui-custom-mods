//! Configuration schema for [`HexViewer`](super::HexViewer).
//!
//! This file contains only config structs, enums, and their impl blocks.
//! Non-config types live in sibling modules:
//! - [`super::provider`] — `HexDataProvider`, `VecDataProvider`, `ColorRegion`, `ByteCategory`
//! - [`super::nav_history`] — `NavHistory`
//! - [`super::undo`] — `UndoEntry`, `UndoStack`

// ── Enums ───────────────────────────────────────────────────────────────────

/// How many bytes to display per row.
///
/// Supports: 8, 12, 16, 20, 24, 28, 32.  Arbitrary multiples of 4 also work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BytesPerRow(usize);

impl BytesPerRow {
    pub const EIGHT: Self = Self(8);
    pub const TWELVE: Self = Self(12);
    pub const SIXTEEN: Self = Self(16);
    pub const TWENTY: Self = Self(20);
    pub const TWENTY_FOUR: Self = Self(24);
    pub const TWENTY_EIGHT: Self = Self(28);
    pub const THIRTY_TWO: Self = Self(32);

    /// All standard presets.
    pub const ALL: &'static [BytesPerRow] = &[
        Self::EIGHT,
        Self::TWELVE,
        Self::SIXTEEN,
        Self::TWENTY,
        Self::TWENTY_FOUR,
        Self::TWENTY_EIGHT,
        Self::THIRTY_TWO,
    ];

    /// Create from an arbitrary value (clamped to 4..=64, rounded down to a
    /// multiple of 4).
    pub fn new(n: usize) -> Self {
        let n = n.clamp(4, 64);
        Self(n - (n % 4))
    }

    pub fn value(self) -> usize {
        self.0
    }

    pub fn display_name(self) -> &'static str {
        match self.0 {
            8 => "8",
            12 => "12",
            16 => "16",
            20 => "20",
            24 => "24",
            28 => "28",
            32 => "32",
            _ => "?",
        }
    }
}

impl Default for BytesPerRow {
    fn default() -> Self {
        Self::SIXTEEN
    }
}

/// Byte grouping for visual separation in the hex column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ByteGrouping {
    /// No extra spacing.
    None = 1,
    /// Space every 2 bytes (word).
    Word = 2,
    /// Space every 4 bytes (dword).
    #[default]
    DWord = 4,
    /// Space every 8 bytes (qword).
    QWord = 8,
}

impl ByteGrouping {
    pub fn value(self) -> usize {
        self as usize
    }
}

/// Endianness for multi-byte data inspector values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Endianness {
    #[default]
    Little,
    Big,
}

impl Endianness {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Little => "LE",
            Self::Big => "BE",
        }
    }
}

/// Address-column width policy.
///
/// Controls how many hex digits the address gutter renders per row. The
/// default ([`AddressWidth::Auto`]) picks 64-bit if the buffer's
/// highest address overflows `u32::MAX`, otherwise 32-bit — so a
/// regular file dump stays compact while a memory snapshot of a 64-bit
/// process automatically gets the wider column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AddressWidth {
    /// 32-bit when `base_address + data.len()` fits in `u32::MAX`,
    /// otherwise 64-bit. Default.
    #[default]
    Auto,
    /// Always render addresses as `{:08X}` / `{:08x}` (8 hex digits).
    Bits32,
    /// Always render addresses as `{:016X}` / `{:016x}` (16 hex digits).
    Bits64,
}

impl AddressWidth {
    /// Number of hex digits the address column occupies for the given
    /// (base, length) pair.
    pub fn hex_digits(self, base: u64, len: usize) -> usize {
        match self {
            Self::Bits32 => 8,
            Self::Bits64 => 16,
            Self::Auto => {
                let max = base.saturating_add(len as u64);
                if max > u32::MAX as u64 { 16 } else { 8 }
            }
        }
    }
}

/// Text encoding used by [`HexSearchMode::String`] when converting the
/// user-typed search query to bytes.
///
/// - [`Self::Ascii`] — single byte per char, treats the input as raw
///   `s.bytes()`. Latin-1 subset only; non-ASCII chars become their
///   leading UTF-8 byte (good for matching wire-level ASCII text in
///   binary buffers).
/// - [`Self::Utf8`] — full Unicode, encoded as UTF-8 wire bytes via
///   `s.as_bytes()`. Use for Cyrillic / Asian / emoji strings stored
///   as UTF-8 (Linux file content, modern web protocols).
/// - [`Self::Utf16Le`] — 2 bytes per code unit, little-endian. The
///   canonical Windows string encoding: PE / .NET strings, Win32
///   `wchar_t`, registry values, MSGS, MUI resources. `"Hello"` →
///   `48 00 65 00 6C 00 6C 00 6F 00`.
///
/// Surrogate-pair characters (U+10000+) are encoded as two UTF-16
/// code units (4 bytes total) — `String::encode_utf16` handles that
/// transparently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum StringEncoding {
    /// 1 byte per char, ASCII / Latin-1 wire bytes.
    #[default]
    Ascii,
    /// Full UTF-8 wire bytes (Cyrillic / Asian text stored as UTF-8).
    Utf8,
    /// 2 bytes per UTF-16 code unit, little-endian (Windows
    /// `wchar_t` / `LPCWSTR` / .NET strings).
    Utf16Le,
}

impl StringEncoding {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Ascii => "ASCII",
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16LE",
        }
    }

    /// All variants in display order — for radio-button rendering.
    pub const ALL: &'static [StringEncoding] = &[
        StringEncoding::Ascii,
        StringEncoding::Utf8,
        StringEncoding::Utf16Le,
    ];
}

/// Search mode for the hex viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HexSearchMode {
    /// Hex pattern with optional `??` wildcards (e.g. `4D 5A ?? 00`).
    #[default]
    Hex,
    /// String search — input is text, encoded into wire bytes via the
    /// chosen [`StringEncoding`] before pattern matching.
    String(StringEncoding),
}

impl HexSearchMode {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::String(_) => "String",
        }
    }

    /// Whether this mode is the `String` variant. Convenience for UI
    /// code that needs to render an encoding sub-selector when a
    /// string mode is active.
    pub fn is_string(self) -> bool {
        matches!(self, Self::String(_))
    }
}

/// Copy format when copying bytes to clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CopyFormat {
    /// Space-separated hex: `4D 5A 90 00`
    #[default]
    HexSpaced,
    /// Continuous hex: `4D5A9000`
    HexCompact,
    /// C array: `{ 0x4D, 0x5A, 0x90, 0x00 }`
    CArray,
    /// Rust array: `[0x4D, 0x5A, 0x90, 0x00]`
    RustArray,
    /// Base64 encoded.
    Base64,
    /// Raw ASCII (printable only).
    Ascii,
}

// ── Forward-compat helpers ──────────────────────────────────────────────────
//
// Per-field `#[serde(default = "fn")]` so older saved `.ron` files
// (pre-dating a field) still parse. Add a helper here whenever you
// introduce a new field with serde-default — see `show_group_dividers`
// for the canonical case.

fn default_show_group_dividers() -> bool {
    false
}

// ── Hex Viewer Config ───────────────────────────────────────────────────────

/// Configuration for the hex viewer widget.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HexViewerConfig {
    // ── Layout ──────────────────────────────────────────────
    /// Bytes per row.
    pub bytes_per_row: BytesPerRow,
    /// Visual grouping of hex bytes.
    pub grouping: ByteGrouping,
    /// Show the ASCII column on the right.
    pub show_ascii: bool,
    /// Show the data inspector panel at the bottom.
    pub show_inspector: bool,
    /// Show the offset column on the left.
    pub show_offsets: bool,
    /// Show column headers (00 01 02 ...).
    pub show_column_headers: bool,
    /// Show thin vertical dividers between offset / hex / ASCII columns.
    /// Default: `true` — improves visual scan-ability of the three
    /// regions without competing with the byte text.
    pub show_column_dividers: bool,
    /// Show dashed vertical dividers between byte groups inside the
    /// hex column (every `grouping` bytes). Off by default — the
    /// natural extra-space gap between groups is usually enough; flip
    /// this on for dense layouts (32 bpr / group=4) where the eye
    /// loses track of group boundaries. Dashes are drawn 4 px on /
    /// 3 px off through the row strip so they read as a calm guide
    /// rather than a solid divider that would compete with column
    /// boundaries.
    #[serde(default = "default_show_group_dividers")]
    pub show_group_dividers: bool,
    /// Show a draggable horizontal splitter between the hex area and
    /// the data inspector. Default: `false` — the inspector uses its
    /// natural fixed height. Enable for memory-explorer style UIs
    /// where the user needs more inspector space on demand.
    pub show_splitter: bool,
    /// Address-column width policy (32 / 64 / auto). Default: `Auto`
    /// — picks 64-bit when the buffer extends beyond `u32::MAX`.
    pub address_width: AddressWidth,
    /// Use uppercase hex digits (A-F vs a-f).
    pub uppercase: bool,
    /// Endianness for multi-byte inspector values.
    pub endianness: Endianness,

    // ── Behavior ────────────────────────────────────────────
    /// Allow editing byte values (click → type hex).
    pub editable: bool,
    /// Base address added to displayed offsets (for memory viewers).
    pub base_address: u64,
    /// Highlight changed bytes (via `HexDataProvider::is_changed` or reference).
    pub highlight_changes: bool,
    /// Enable semantic byte-category coloring (zero/control/printable/high/FF).
    pub category_colors: bool,
    /// Zero-byte display style: dim dots instead of "00".
    pub dim_zeros: bool,
    /// Auto-refresh interval in frames (0 = disabled). When non-zero, the
    /// viewer increments an internal frame counter on each render; once
    /// `auto_refresh_frames` is reached, [`HexViewer::take_refresh_pending`]
    /// returns `true` exactly once so the caller can re-fetch live data
    /// (process memory, MMAP, remote target) and call `set_data*` to
    /// replace the buffer. The viewer itself never invokes the data
    /// provider — refresh policy stays explicit on the caller's side.
    pub auto_refresh_frames: u32,
    /// Search mode.
    pub search_mode: HexSearchMode,
    /// Default copy format.
    pub copy_format: CopyFormat,
    /// Maximum undo stack depth (0 = unlimited).
    pub max_undo: usize,
    /// Whether the host has loaded the Material Design Icons (MDI)
    /// atlas (`U+F0000`..`U+F1FFF` range). When `true` the popup /
    /// settings UI uses MDI glyphs (`wrench-cog` for Settings, etc.)
    /// for stronger visual cues; when `false` it falls back to
    /// atlas-safe characters (`\u{2026}` ellipsis, etc.) so nothing
    /// renders as a `?` placeholder. Default: `true` — every
    /// in-tree consumer (`IMGUI_NXT`, the demo) ships the MDI atlas.
    pub icons_available: bool,

    /// User-visible language for labels, popups, tooltips, and menu
    /// items rendered by the viewer. Default
    /// [`crate::i18n::Locale::En`]. Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` (or a superset) into the active font
    /// atlas — otherwise non-ASCII characters render as `?`.
    /// `#[serde(default)]` so older `config.ron` files that pre-date
    /// the locale field deserialise cleanly (falling back to English).
    #[serde(default)]
    pub locale: crate::i18n::Locale,

    // ── Colors: semantic byte categories ────────────────────
    /// Color for null bytes (0x00).
    #[serde(skip, default)]
    pub color_cat_zero: [f32; 4],
    /// Color for control characters (0x01..0x1F, 0x7F).
    #[serde(skip, default)]
    pub color_cat_control: [f32; 4],
    /// Color for printable ASCII (0x20..0x7E).
    #[serde(skip, default)]
    pub color_cat_printable: [f32; 4],
    /// Color for high bytes (0x80..0xFE).
    #[serde(skip, default)]
    pub color_cat_high: [f32; 4],
    /// Color for 0xFF bytes.
    #[serde(skip, default)]
    pub color_cat_full: [f32; 4],

    // ── Colors: UI elements ─────────────────────────────────
    /// Offset column color.
    #[serde(skip, default)]
    pub color_offset: [f32; 4],
    /// Normal hex byte color (used when `category_colors` is `false`).
    #[serde(skip, default)]
    pub color_hex: [f32; 4],
    /// ASCII printable character color.
    #[serde(skip, default)]
    pub color_ascii: [f32; 4],
    /// ASCII non-printable dot color.
    #[serde(skip, default)]
    pub color_ascii_dot: [f32; 4],
    /// Zero byte color (legacy, when `dim_zeros` is true and `category_colors` is false).
    #[serde(skip, default)]
    pub color_zero: [f32; 4],
    /// Selection highlight background.
    #[serde(skip, default)]
    pub color_selection_bg: [f32; 4],
    /// Changed byte highlight color.
    #[serde(skip, default)]
    pub color_changed: [f32; 4],
    /// Cursor highlight background.
    #[serde(skip, default)]
    pub color_cursor_bg: [f32; 4],
    /// Column header color.
    #[serde(skip, default)]
    pub color_header: [f32; 4],
    /// Inspector label color.
    #[serde(skip, default)]
    pub color_inspector_label: [f32; 4],
    /// Inspector value color.
    #[serde(skip, default)]
    pub color_inspector_value: [f32; 4],
    /// Search match highlight background.
    #[serde(skip, default)]
    pub color_search_match: [f32; 4],
    /// Non-readable region background.
    #[serde(skip, default)]
    pub color_unreadable: [f32; 4],
}

impl Default for HexViewerConfig {
    /// Non-color fields come from `config.ron` (the values file); colour
    /// fields are theme-derived and applied via `apply_theme_colors`.
    /// To re-skin a viewer for a different theme, call `with_theme(...)`.
    fn default() -> Self {
        let mut cfg: Self = ron::from_str(include_str!("config.ron"))
            .expect("built-in hex_viewer/config.ron is valid");
        cfg.apply_theme_colors(&crate::theme::HexViewerColors::default());
        cfg
    }
}

impl HexViewerConfig {
    /// Copy a theme-driven [`crate::theme::HexViewerColors`] palette
    /// into the 18 individual `color_*` fields of this config. Use
    /// this whenever the host theme changes so the viewer stays in
    /// the same visual family as `nav_panel` / `status_bar` etc.
    ///
    /// ```rust,ignore
    /// // Theme switch:
    /// viewer.config_mut().apply_theme_colors(&Theme::Solarized.hex_viewer_colors());
    /// ```
    pub fn apply_theme_colors(&mut self, p: &crate::theme::HexViewerColors) {
        self.color_cat_zero = p.cat_zero;
        self.color_cat_control = p.cat_control;
        self.color_cat_printable = p.cat_printable;
        self.color_cat_high = p.cat_high;
        self.color_cat_full = p.cat_full;

        self.color_offset = p.offset;
        self.color_hex = p.hex;
        self.color_ascii = p.ascii;
        self.color_ascii_dot = p.ascii_dot;
        self.color_header = p.header;
        self.color_inspector_label = p.inspector_label;
        self.color_inspector_value = p.inspector_value;
        self.color_zero = p.zero_legacy;

        self.color_selection_bg = p.selection_bg;
        self.color_cursor_bg = p.cursor_bg;
        self.color_changed = p.changed;
        self.color_search_match = p.search_match;
        self.color_unreadable = p.unreadable;
    }

    /// Convenience builder: start from `Default`, then apply the named
    /// theme's hex-viewer palette in one call.
    ///
    /// ```rust,ignore
    /// use dear_imgui_custom_mod::hex_viewer::HexViewerConfig;
    /// use dear_imgui_custom_mod::theme::Theme;
    ///
    /// let cfg = HexViewerConfig::default().with_theme(Theme::Nord);
    /// ```
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme_colors(&theme.hex_viewer_colors());
        self
    }

    /// Get the foreground color for a byte based on its category.
    pub fn byte_fg_color(&self, byte: u8) -> [f32; 4] {
        use super::provider::ByteCategory;
        if !self.category_colors {
            if byte == 0 && self.dim_zeros {
                return self.color_zero;
            }
            return self.color_hex;
        }
        match ByteCategory::of(byte) {
            ByteCategory::Zero => self.color_cat_zero,
            ByteCategory::Control => self.color_cat_control,
            ByteCategory::Printable => self.color_cat_printable,
            ByteCategory::High => self.color_cat_high,
            ByteCategory::Full => self.color_cat_full,
        }
    }
}
