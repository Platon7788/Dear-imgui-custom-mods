//! Hex-viewer palette type — the [`HexViewerColors`] colour-token struct
//! and its `from_tokens` factory. Split out of [`super`] to keep each
//! palette file under the size limit. Re-exported from [`super`] so the
//! standard `crate::theme::HexViewerColors` path keeps working.

/// Complete palette for the [`crate::hex_viewer::HexViewer`] widget.
///
/// 18 colour tokens grouped by purpose:
/// - **5 byte categories** (`cat_zero`, `cat_control`, `cat_printable`,
///   `cat_high`, `cat_full`) — semantic byte-value tinting.
/// - **8 UI text** (`offset`, `hex`, `ascii`, `ascii_dot`, `header`,
///   `inspector_label`, `inspector_value`, `zero_legacy`) — gutter,
///   content, header row, data inspector.
/// - **5 highlight surfaces** (`selection_bg`, `cursor_bg`, `changed`,
///   `search_match`, `unreadable`) — interactive overlays.
///
/// Built-in themes expose this via
/// [`crate::theme::Theme::hex_viewer_colors`]; bare
/// `HexViewerConfig::default()` uses [`HexViewerColors::default`]
/// which mirrors `Theme::Dark.hex_viewer_colors()`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct HexViewerColors {
    // ── Byte categories ─────────────────────────────────────────
    /// `0x00` byte.
    pub cat_zero: [f32; 4],
    /// `0x01..=0x1F` + `0x7F` (control chars).
    pub cat_control: [f32; 4],
    /// `0x20..=0x7E` (printable ASCII).
    pub cat_printable: [f32; 4],
    /// `0x80..=0xFE` (high / extended).
    pub cat_high: [f32; 4],
    /// `0xFF` byte.
    pub cat_full: [f32; 4],

    // ── UI text ─────────────────────────────────────────────────
    /// Address gutter colour — primary "where am I" cue.
    pub offset: [f32; 4],
    /// Default hex byte text colour (used when category tinting is off).
    pub hex: [f32; 4],
    /// Printable char in the ASCII column.
    pub ascii: [f32; 4],
    /// `.` placeholder for non-printable bytes in the ASCII column.
    pub ascii_dot: [f32; 4],
    /// Column-header row ("Offset / 00 01 02 ... / ASCII").
    pub header: [f32; 4],
    /// Inspector label colour (e.g. `u16=`).
    pub inspector_label: [f32; 4],
    /// Inspector value colour (the decoded number itself).
    pub inspector_value: [f32; 4],
    /// Legacy zero-byte colour (used when `category_colors == false` and
    /// `dim_zeros == true`).
    pub zero_legacy: [f32; 4],

    // ── Highlight surfaces ──────────────────────────────────────
    /// Background fill behind selected bytes.
    pub selection_bg: [f32; 4],
    /// Background fill behind the cursor byte.
    pub cursor_bg: [f32; 4],
    /// Foreground colour for bytes that differ from the reference snapshot.
    pub changed: [f32; 4],
    /// Background fill behind the bytes that match the active search.
    pub search_match: [f32; 4],
    /// Background fill for bytes the data provider reports as
    /// unreadable (gaps in a memory dump).
    pub unreadable: [f32; 4],
}

impl Default for HexViewerColors {
    /// Mirrors `Theme::Dark.hex_viewer_colors()` — see
    /// `theme::widget_tests::hex_viewer_colors_default_matches_dark_theme`.
    fn default() -> Self {
        crate::theme::dark::hex_viewer_colors()
    }
}

/// Semantic token bundle each theme passes to [`HexViewerColors::from_tokens`].
/// Lets every per-theme palette be expressed in 9 lines instead of
/// reproducing all 18 hex_viewer fields by hand.
#[doc(hidden)]
pub struct HexViewerTokens {
    /// Primary content text — the colour the theme uses for `FG`.
    pub fg: [f32; 4],
    /// Muted text — `FG_MUTED`.
    pub fg_muted: [f32; 4],
    /// Theme accent — drives `offset` + `cursor_bg` (alpha-modulated).
    pub accent: [f32; 4],
    /// Semantic green (printable bytes, success).
    pub success: [f32; 4],
    /// Semantic amber (`0xFF` byte, search highlight).
    pub warning: [f32; 4],
    /// Semantic red (`changed`, `unreadable`).
    pub danger: [f32; 4],
    /// Purple-family hue for `0x80..0xFE` "high" bytes — pick a colour
    /// distinct from accent and success so the category really stands out.
    pub purple: [f32; 4],
}

impl HexViewerColors {
    /// Build a [`HexViewerColors`] from a small bundle of semantic
    /// tokens. Used by every per-theme `hex_viewer_colors()` so the
    /// 18-field palette stays consistent — only the seed colours
    /// change between themes.
    pub fn from_tokens(t: &HexViewerTokens) -> Self {
        // Helper: same RGB as `c`, but with alpha overridden.
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        Self {
            // Byte categories — distinct hues so users can scan dumps quickly.
            cat_zero: with_a(t.fg_muted, 0.45),
            cat_control: with_a(t.fg_muted, 0.70),
            cat_printable: t.success,
            cat_high: t.purple,
            cat_full: t.warning,

            // UI text.
            offset: t.accent,
            hex: t.fg,
            ascii: t.success,
            ascii_dot: with_a(t.fg_muted, 0.45),
            // Column header captions ("Offset", "00 01 ... 1F", "ASCII")
            // — pinned to `fg` (full-strength text colour) so they read
            // as bright white in dark themes and bold dark in light
            // themes. Earlier this token landed on `fg_muted`, which
            // made the header row visually closer to a comment than to
            // a label and the user reported it as washed-out.
            header: t.fg,
            inspector_label: t.fg_muted,
            inspector_value: t.fg,
            zero_legacy: with_a(t.fg_muted, 0.40),

            // Highlight surfaces — alpha-modulated theme tokens so the
            // overlays tint the underlying byte text instead of clobbering it.
            selection_bg: with_a(t.accent, 0.40),
            cursor_bg: with_a(t.accent, 0.45),
            changed: t.warning,
            search_match: with_a(t.warning, 0.35),
            unreadable: with_a(t.danger, 0.25),
        }
    }
}
