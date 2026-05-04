//! Font selection types for [`AppConfig`](super::AppConfig).

use std::sync::Arc;

// ── FontChoice ───────────────────────────────────────────────────────────────

/// Font selection for the ImGui context. Default: built-in Hack.
#[derive(Debug, Clone)]
pub enum FontChoice {
    /// One of the fonts shipped with `code_editor`.
    Builtin(crate::fonts::BuiltinFont),
    /// A user-supplied TTF/OTF byte buffer (e.g. via `include_bytes!`).
    Bytes(Arc<[u8]>),
    /// Multiple fonts merged into a single atlas. The first layer is the
    /// **base font** (its `merge` flag is ignored — bases are never merged);
    /// subsequent layers are added on top, typically with `merge = true` so
    /// their glyphs (icons, CJK, math) overlay the base.
    ///
    /// Example — Inter UI + Material Design Icons:
    /// ```ignore
    /// FontChoice::Stack(vec![
    ///     FontLayer::base(include_bytes!("Inter.ttf"), 15.0),
    ///     FontLayer::merge(include_bytes!("MDI.ttf"), 13.0)
    ///         .with_glyph_ranges(GlyphRanges::Custom(vec![[0xF0001, 0xF1FFF]])),
    /// ])
    /// ```
    Stack(Vec<FontLayer>),
}

impl Default for FontChoice {
    fn default() -> Self {
        Self::Builtin(crate::fonts::BuiltinFont::Hack)
    }
}

// ── FontLayer ────────────────────────────────────────────────────────────────

/// One font layer inside a [`FontChoice::Stack`].
#[derive(Debug, Clone)]
pub struct FontLayer {
    /// Raw TTF/OTF bytes. Owned via `Arc<[u8]>` so the same buffer can
    /// appear in multiple `FontLayer`s without allocation churn.
    pub bytes: Arc<[u8]>,
    /// Pixel size in logical units. The framework multiplies by HiDPI
    /// scale automatically.
    pub size: f32,
    /// `true` ⇒ glyphs from this layer overlay the previous layer's atlas.
    /// Ignored on the first layer (always treated as base).
    pub merge: bool,
    /// Codepoint subset to bake. Use [`GlyphRanges::Default`] for
    /// Latin-only, presets for CJK/Cyrillic/Thai/Vietnamese, or
    /// [`GlyphRanges::Custom`] for arbitrary ranges (e.g. icon fonts).
    pub glyph_ranges: GlyphRanges,
}

impl FontLayer {
    /// New base layer (`merge = false`). The first layer of a stack must
    /// be a base.
    pub fn base(bytes: impl Into<Arc<[u8]>>, size: f32) -> Self {
        Self {
            bytes: bytes.into(),
            size,
            merge: false,
            glyph_ranges: GlyphRanges::Default,
        }
    }
    /// New merge layer (`merge = true`). Add after a base layer.
    pub fn merge(bytes: impl Into<Arc<[u8]>>, size: f32) -> Self {
        Self {
            bytes: bytes.into(),
            size,
            merge: true,
            glyph_ranges: GlyphRanges::Default,
        }
    }
    /// Override the glyph-range subset baked from this layer.
    pub fn with_glyph_ranges(mut self, ranges: GlyphRanges) -> Self {
        self.glyph_ranges = ranges;
        self
    }
}

// ── GlyphRanges ──────────────────────────────────────────────────────────────

/// Codepoint-range subset to bake into the font atlas. Maps to Dear ImGui's
/// `ImFontGlyphRanges*` constants.
///
/// **Default** is the right pick for any UI restricted to Latin text.
/// Pick a regional preset for non-Latin UI text. Use `Custom` for icon
/// fonts (Material Design Icons, Font Awesome, Phosphor, etc.) which
/// occupy private-use Unicode planes.
#[derive(Debug, Clone, Default)]
pub enum GlyphRanges {
    /// Basic Latin + Latin-1 supplement (`0x0020..=0x00FF`). Default.
    #[default]
    Default,
    /// Basic Latin + Cyrillic (Russian, Ukrainian, Belarusian, …).
    Cyrillic,
    /// Basic Latin + Hiragana + Katakana + half-width (Japanese).
    Japanese,
    /// Basic Latin + CJK common ideograms (Chinese Simplified).
    ChineseSimplified,
    /// Basic Latin + CJK common ideograms (Chinese Traditional).
    ChineseTraditional,
    /// Basic Latin + Hangul (Korean).
    Korean,
    /// Basic Latin + Thai.
    Thai,
    /// Basic Latin + Vietnamese-specific accented glyphs.
    Vietnamese,
    /// Inclusive `[start, end]` ranges. Last entry ends the list — the
    /// framework appends the required `0` terminator. Useful for icon
    /// fonts: `Custom(vec![[0xF0001, 0xF1FFF]])` for MDI.
    Custom(Vec<[u32; 2]>),
}
