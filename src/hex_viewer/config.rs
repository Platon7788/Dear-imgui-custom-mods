//! Configuration types for [`HexViewer`](super::HexViewer).

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
        Self { offset, len, color, label: label.into() }
    }
}

/// How many bytes to display per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum BytesPerRow {
    Eight = 8,
    #[default]
    Sixteen = 16,
    ThirtyTwo = 32,
}


impl BytesPerRow {
    pub fn value(self) -> usize { self as usize }
    pub const ALL: &'static [BytesPerRow] = &[Self::Eight, Self::Sixteen, Self::ThirtyTwo];
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Eight => "8",
            Self::Sixteen => "16",
            Self::ThirtyTwo => "32",
        }
    }
}

/// Byte grouping for visual separation in the hex column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
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
    pub fn value(self) -> usize { self as usize }
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

/// Configuration for the hex viewer widget.
#[derive(Debug, Clone)]
pub struct HexViewerConfig {
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
    /// Use uppercase hex digits (A-F vs a-f).
    pub uppercase: bool,
    /// Endianness for multi-byte inspector values.
    pub endianness: Endianness,
    /// Allow editing byte values (click → type hex).
    pub editable: bool,
    /// Base address added to displayed offsets (for memory viewers).
    pub base_address: u64,
    /// Highlight changed bytes (compared to a reference snapshot).
    pub highlight_changes: bool,
    /// Show column headers (00 01 02 ...).
    pub show_column_headers: bool,
    /// Zero-byte display style: dim dots instead of "00".
    pub dim_zeros: bool,

    // ── Colors ───────────────────────────────────────────────
    /// Offset column color.
    pub color_offset: [f32; 4],
    /// Normal hex byte color.
    pub color_hex: [f32; 4],
    /// ASCII printable character color.
    pub color_ascii: [f32; 4],
    /// ASCII non-printable dot color.
    pub color_ascii_dot: [f32; 4],
    /// Zero byte color (when dim_zeros is true).
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
}

impl Default for HexViewerConfig {
    fn default() -> Self {
        Self {
            bytes_per_row:      BytesPerRow::Sixteen,
            grouping:           ByteGrouping::DWord,
            show_ascii:         true,
            show_inspector:     true,
            show_offsets:       true,
            uppercase:          true,
            endianness:         Endianness::Little,
            editable:           false,
            base_address:       0,
            highlight_changes:  false,
            show_column_headers: true,
            dim_zeros:          true,

            color_offset:       [0.45, 0.55, 0.70, 1.0],
            color_hex:          [0.85, 0.85, 0.85, 1.0],
            color_ascii:        [0.70, 0.80, 0.65, 1.0],
            color_ascii_dot:    [0.35, 0.35, 0.40, 0.6],
            color_zero:         [0.30, 0.30, 0.35, 0.5],
            color_selection_bg: [0.20, 0.35, 0.55, 0.5],
            color_changed:      [1.00, 0.60, 0.20, 1.0],
            color_cursor_bg:    [0.30, 0.45, 0.65, 0.7],
            color_header:       [0.50, 0.55, 0.60, 0.8],
            color_inspector_label: [0.55, 0.58, 0.65, 1.0],
            color_inspector_value: [0.90, 0.90, 0.90, 1.0],
        }
    }
}
