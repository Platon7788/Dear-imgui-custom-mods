//! Configuration types for [`HexViewer`](super::HexViewer).

// ── Data Provider ───────────────────────────────────────────────────────────

/// Trait for abstracting the data source.
///
/// Default implementation works with `Vec<u8>` in memory.
/// Implement this for page-cached remote memory, memory-mapped files, etc.
pub trait HexDataProvider {
    /// Total data length in bytes (may be `u64::MAX` for streaming).
    fn len(&self) -> u64;
    /// Whether the data source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Read bytes at `offset` into `buf`. Returns number of bytes actually read.
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize;
    /// Write bytes at `offset`. Returns `true` on success.
    /// Default: returns `false` (read-only).
    fn write(&mut self, _offset: u64, _data: &[u8]) -> bool {
        false
    }
    /// Whether the given offset is readable (e.g., mapped memory region).
    /// Default: `true` for any offset < len.
    fn is_readable(&self, offset: u64) -> bool {
        offset < self.len()
    }
    /// Whether the byte at `offset` has changed since last snapshot.
    /// Used for diff highlighting in live-memory scenarios.
    /// Default: `false`.
    fn is_changed(&self, _offset: u64) -> bool {
        false
    }
    /// Called every frame when auto-refresh is enabled.
    /// Use this to trigger page re-fetches, poll for changes, etc.
    fn refresh(&mut self) {}
}

/// Default in-memory data provider wrapping `Vec<u8>`.
pub struct VecDataProvider {
    data: Vec<u8>,
    /// Optional reference snapshot for diff highlighting.
    reference: Option<Vec<u8>>,
}

impl VecDataProvider {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            reference: None,
        }
    }
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            reference: None,
        }
    }
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
    pub fn set_data_slice(&mut self, data: &[u8]) {
        self.data = data.to_vec();
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
    pub fn set_reference(&mut self, r: &[u8]) {
        self.reference = Some(r.to_vec());
    }
    pub fn clear_reference(&mut self) {
        self.reference = None;
    }
}

impl HexDataProvider for VecDataProvider {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize {
        let off = offset as usize;
        if off >= self.data.len() {
            return 0;
        }
        let end = (off + buf.len()).min(self.data.len());
        let n = end - off;
        buf[..n].copy_from_slice(&self.data[off..end]);
        n
    }
    fn write(&mut self, offset: u64, data: &[u8]) -> bool {
        let off = offset as usize;
        if off + data.len() > self.data.len() {
            return false;
        }
        self.data[off..off + data.len()].copy_from_slice(data);
        true
    }
    fn is_changed(&self, offset: u64) -> bool {
        if let Some(ref r) = self.reference {
            let i = offset as usize;
            if i < self.data.len() && i < r.len() {
                return self.data[i] != r[i];
            }
        }
        false
    }
}

// ── Color Region ────────────────────────────────────────────────────────────

/// Color region — maps a byte range to a color and label for struct overlays.
#[derive(Debug, Clone)]
pub struct ColorRegion {
    /// Start offset in the data buffer.
    pub offset: usize,
    /// Length in bytes.
    pub len: usize,
    /// RGBA color `[r, g, b, a]` in `0.0..=1.0`.
    pub color: [f32; 4],
    /// Human-readable label (e.g. field name).
    pub label: String,
}

impl ColorRegion {
    pub fn new(offset: usize, len: usize, color: [f32; 4], label: impl Into<String>) -> Self {
        Self {
            offset,
            len,
            color,
            label: label.into(),
        }
    }
}

// ── Byte Category ───────────────────────────────────────────────────────────

/// Semantic byte category for 5-tier coloring (like debugger hex views).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteCategory {
    /// `0x00` — null bytes.
    Zero,
    /// `0x01..=0x1F`, `0x7F` — control characters.
    Control,
    /// `0x20..=0x7E` — printable ASCII.
    Printable,
    /// `0x80..=0xFE` — high / extended bytes.
    High,
    /// `0xFF` — all-ones byte.
    Full,
}

impl ByteCategory {
    /// Classify a byte value.
    pub fn of(byte: u8) -> Self {
        match byte {
            0x00 => Self::Zero,
            0x01..=0x1F | 0x7F => Self::Control,
            0x20..=0x7E => Self::Printable,
            0xFF => Self::Full,
            _ => Self::High, // 0x80..=0xFE
        }
    }
}

// ── Enums ───────────────────────────────────────────────────────────────────

/// How many bytes to display per row.
///
/// Supports: 8, 12, 16, 20, 24, 28, 32.  Arbitrary multiples of 4 also work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Create from an arbitrary value (clamped to 4..=64, rounded to multiple of 4).
    pub fn new(n: usize) -> Self {
        let n = n.clamp(4, 64);
        Self(n - (n % 4).min(n)) // round down to multiple of 4, minimum 4
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// Search mode for the hex viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HexSearchMode {
    /// Hex pattern with optional `??` wildcards (e.g. `4D 5A ?? 00`).
    #[default]
    Hex,
    /// ASCII string search.
    Ascii,
}

impl HexSearchMode {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Ascii => "ASCII",
        }
    }
}

/// Copy format when copying bytes to clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

// ── Undo Entry ──────────────────────────────────────────────────────────────

/// A single undo-able edit operation.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// Byte offset where the edit happened.
    pub offset: u64,
    /// Old byte values before the edit.
    pub old_bytes: Vec<u8>,
    /// New byte values after the edit.
    pub new_bytes: Vec<u8>,
}

/// Undo/redo stack with configurable depth.
#[derive(Debug, Clone)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
    /// Points to the next undo position (entries[pos-1] is last applied).
    pos: usize,
    /// Maximum stack depth (0 = unlimited).
    max_depth: usize,
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            entries: Vec::new(),
            pos: 0,
            max_depth,
        }
    }

    /// Record an edit. Truncates any redo history.
    pub fn push(&mut self, entry: UndoEntry) {
        self.entries.truncate(self.pos);
        self.entries.push(entry);
        self.pos = self.entries.len();
        // Trim oldest if over capacity.
        if self.max_depth > 0 && self.entries.len() > self.max_depth {
            let remove = self.entries.len() - self.max_depth;
            self.entries.drain(..remove);
            self.pos = self.entries.len();
        }
    }

    /// Undo the last edit. Returns the entry to reverse, or `None`.
    pub fn undo(&mut self) -> Option<&UndoEntry> {
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        Some(&self.entries[self.pos])
    }

    /// Redo the next edit. Returns the entry to re-apply, or `None`.
    pub fn redo(&mut self) -> Option<&UndoEntry> {
        if self.pos >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[self.pos];
        self.pos += 1;
        Some(entry)
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.pos > 0
    }
    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.pos < self.entries.len()
    }
    /// Number of undo steps available.
    pub fn undo_count(&self) -> usize {
        self.pos
    }
    /// Number of redo steps available.
    pub fn redo_count(&self) -> usize {
        self.entries.len() - self.pos
    }
    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pos = 0;
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(256)
    }
}

// ── Navigation History ──────────────────────────────────────────────────────

/// Back/forward navigation stack for address history.
///
/// Uses `VecDeque` for O(1) eviction of oldest entries.
#[derive(Debug, Clone, Default)]
pub struct NavHistory {
    back: std::collections::VecDeque<u64>,
    forward: Vec<u64>,
    /// Maximum stack depth.
    max_depth: usize,
}

impl NavHistory {
    pub fn new(max_depth: usize) -> Self {
        Self {
            back: std::collections::VecDeque::new(),
            forward: Vec::new(),
            max_depth,
        }
    }

    /// Record a navigation to `addr`. Call *before* changing the address.
    pub fn push(&mut self, current_addr: u64) {
        self.back.push_back(current_addr);
        self.forward.clear();
        if self.max_depth > 0 && self.back.len() > self.max_depth {
            self.back.pop_front(); // O(1) instead of Vec::remove(0) O(n)
        }
    }

    /// Go back. Returns previous address, or `None`.
    pub fn go_back(&mut self, current_addr: u64) -> Option<u64> {
        let addr = self.back.pop_back()?;
        self.forward.push(current_addr);
        Some(addr)
    }

    /// Go forward. Returns next address, or `None`.
    pub fn go_forward(&mut self, current_addr: u64) -> Option<u64> {
        let addr = self.forward.pop()?;
        self.back.push_back(current_addr);
        Some(addr)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
    pub fn clear(&mut self) {
        self.back.clear();
        self.forward.clear();
    }
}

// ── Hex Viewer Config ───────────────────────────────────────────────────────

/// Configuration for the hex viewer widget.
#[derive(Debug, Clone)]
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
    /// Auto-refresh interval in frames (0 = disabled).
    /// When > 0, calls `HexDataProvider::refresh()` every N frames.
    pub auto_refresh_frames: u32,
    /// Search mode.
    pub search_mode: HexSearchMode,
    /// Default copy format.
    pub copy_format: CopyFormat,
    /// Maximum undo stack depth (0 = unlimited).
    pub max_undo: usize,

    // ── Colors: semantic byte categories ────────────────────
    /// Color for null bytes (0x00).
    pub color_cat_zero: [f32; 4],
    /// Color for control characters (0x01..0x1F, 0x7F).
    pub color_cat_control: [f32; 4],
    /// Color for printable ASCII (0x20..0x7E).
    pub color_cat_printable: [f32; 4],
    /// Color for high bytes (0x80..0xFE).
    pub color_cat_high: [f32; 4],
    /// Color for 0xFF bytes.
    pub color_cat_full: [f32; 4],

    // ── Colors: UI elements ─────────────────────────────────
    /// Offset column color.
    pub color_offset: [f32; 4],
    /// Normal hex byte color (used when `category_colors` is `false`).
    pub color_hex: [f32; 4],
    /// ASCII printable character color.
    pub color_ascii: [f32; 4],
    /// ASCII non-printable dot color.
    pub color_ascii_dot: [f32; 4],
    /// Zero byte color (legacy, when `dim_zeros` is true and `category_colors` is false).
    pub color_zero: [f32; 4],
    /// Selection highlight background.
    pub color_selection_bg: [f32; 4],
    /// Changed byte highlight color.
    pub color_changed: [f32; 4],
    /// Cursor highlight background.
    pub color_cursor_bg: [f32; 4],
    /// Column header color.
    pub color_header: [f32; 4],
    /// Inspector label color.
    pub color_inspector_label: [f32; 4],
    /// Inspector value color.
    pub color_inspector_value: [f32; 4],
    /// Search match highlight background.
    pub color_search_match: [f32; 4],
    /// Non-readable region background.
    pub color_unreadable: [f32; 4],
}

impl Default for HexViewerConfig {
    fn default() -> Self {
        Self {
            bytes_per_row: BytesPerRow::SIXTEEN,
            grouping: ByteGrouping::DWord,
            show_ascii: true,
            show_inspector: true,
            show_offsets: true,
            show_column_headers: true,
            uppercase: true,
            endianness: Endianness::Little,

            editable: false,
            base_address: 0,
            highlight_changes: false,
            category_colors: true,
            dim_zeros: true,
            auto_refresh_frames: 0,
            search_mode: HexSearchMode::Hex,
            copy_format: CopyFormat::HexSpaced,
            max_undo: 256,

            // Semantic byte category palette (dark theme optimized)
            color_cat_zero: [0.45, 0.35, 0.35, 0.55], // dim salmon
            color_cat_control: [0.50, 0.50, 0.55, 0.70], // dim gray-blue
            color_cat_printable: [0.55, 0.85, 0.55, 1.0], // green
            color_cat_high: [0.65, 0.55, 0.80, 0.85], // muted purple
            color_cat_full: [0.95, 0.75, 0.30, 1.0],  // amber

            // UI colors
            color_offset: [0.45, 0.55, 0.70, 1.0],
            color_hex: [0.85, 0.85, 0.85, 1.0],
            color_ascii: [0.70, 0.80, 0.65, 1.0],
            color_ascii_dot: [0.35, 0.35, 0.40, 0.6],
            color_zero: [0.30, 0.30, 0.35, 0.5],
            color_selection_bg: [0.20, 0.35, 0.55, 0.5],
            color_changed: [1.00, 0.50, 0.20, 1.0],
            color_cursor_bg: [0.30, 0.45, 0.65, 0.7],
            color_header: [0.50, 0.55, 0.60, 0.8],
            color_inspector_label: [0.55, 0.58, 0.65, 1.0],
            color_inspector_value: [0.90, 0.90, 0.90, 1.0],
            color_search_match: [0.80, 0.70, 0.20, 0.35],
            color_unreadable: [0.40, 0.15, 0.15, 0.25],
        }
    }
}

impl HexViewerConfig {
    /// Get the foreground color for a byte based on its category.
    pub fn byte_fg_color(&self, byte: u8) -> [f32; 4] {
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
