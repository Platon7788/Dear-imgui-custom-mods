//! Line-attached semantic decorations for [`CodeEditor`].
//!
//! Purely additive to the existing token pipeline: decorations are pushed
//! by the host as passive data (`Vec<LineAnnotation>`), the editor reads
//! them during draw and paints wash / rule / ghost / end-pill overlays.
//! No behaviour — no callbacks, no traits, no interior mutability.
//!
//! # Coordinate system
//!
//! All positions are CHAR-COLS (same unit as `CursorPos.col`), NOT byte
//! offsets. This makes them tab-aware (`col_to_x` already handles tabs),
//! UTF-8-safe, and directly composable with cursor/selection positions.
//!
//! # Color
//!
//! [`DecorationColor`] has three sources, resolved against the active
//! [`SyntaxColors`] at draw time. Themed slots (`Reuse` / `Semantic`)
//! keep decorations consistent when the user switches themes; `Raw`
//! escapes for one-off exact colours.

use crate::code_editor::syntax_colors::SyntaxColors;

// ─── Color source ────────────────────────────────────────────────────────────

/// Where a decoration draws its colour from.
///
/// Resolved at draw time via [`DecorationColor::resolve`]. Cheap `Copy` —
/// hosts can store `DecorationColor` values inside their own state without
/// worrying about lifetimes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecorationColor {
    /// Fixed sRGB colour, ignores theme. Prefer [`Reuse`](Self::Reuse) or
    /// [`Semantic`](Self::Semantic) so the palette follows theme switches.
    Raw([f32; 4]),
    /// Reuse an existing named slot from [`SyntaxColors`] (e.g. `HexNull`,
    /// `String`). Zero extra theme surface — piggy-backs on colours the
    /// editor already ships. Useful when a decoration's role happens to
    /// map cleanly onto an existing syntax kind.
    Reuse(ColorSlot),
    /// One of 8 dedicated decoration slots ([`SemanticSlot::S1`] to
    /// [`SemanticSlot::S8`]). Each theme fills all eight harmoniously so
    /// hosts can pick by role — e.g. "opcode" → `S1`, "int field" → `S3`
    /// — and the palette follows theme switches automatically.
    Semantic(SemanticSlot),
}

/// Named pointer into an existing [`SyntaxColors`] field. Kept in sync
/// with the fields on `SyntaxColors` — adding a variant here is safe;
/// removing one is a breaking change for consumers that constructed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
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
    HexNull,
    HexFF,
    HexDefault,
    HexPrintable,
    ErrorUnderline,
    WarningUnderline,
}

/// One of the eight dedicated decoration slots painted by every theme.
/// Hosts document their own mapping (e.g. NxT: `S1 = opcode`, `S2 = subop`,
/// `S3 = integer`, `S4 = string`, `S5 = bytes`, `S6 = ref`, `S7 = warning`,
/// `S8 = highlight`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticSlot {
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
}

impl SemanticSlot {
    /// Index into [`SyntaxColors::decoration_slots`] — 0-based.
    #[inline]
    pub fn index(self) -> usize {
        match self {
            SemanticSlot::S1 => 0,
            SemanticSlot::S2 => 1,
            SemanticSlot::S3 => 2,
            SemanticSlot::S4 => 3,
            SemanticSlot::S5 => 4,
            SemanticSlot::S6 => 5,
            SemanticSlot::S7 => 6,
            SemanticSlot::S8 => 7,
        }
    }
}

impl DecorationColor {
    /// Resolve the colour source into a concrete sRGB tuple against the
    /// supplied [`SyntaxColors`] palette.
    pub fn resolve(&self, colors: &SyntaxColors) -> [f32; 4] {
        match *self {
            DecorationColor::Raw(c) => c,
            DecorationColor::Reuse(slot) => match slot {
                ColorSlot::Keyword => colors.keyword,
                ColorSlot::TypeName => colors.type_name,
                ColorSlot::Lifetime => colors.lifetime,
                ColorSlot::String => colors.string,
                ColorSlot::CharLit => colors.char_lit,
                ColorSlot::Number => colors.number,
                ColorSlot::Comment => colors.comment,
                ColorSlot::Attribute => colors.attribute,
                ColorSlot::MacroCall => colors.macro_call,
                ColorSlot::Operator => colors.operator,
                ColorSlot::Punctuation => colors.punctuation,
                ColorSlot::Identifier => colors.identifier,
                ColorSlot::HexNull => colors.hex_null,
                ColorSlot::HexFF => colors.hex_ff,
                ColorSlot::HexDefault => colors.hex_default,
                ColorSlot::HexPrintable => colors.hex_printable,
                ColorSlot::ErrorUnderline => colors.error_underline,
                ColorSlot::WarningUnderline => colors.warning_underline,
            },
            DecorationColor::Semantic(slot) => colors.decoration_slots[slot.index()],
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

// ─── Annotation strip config ─────────────────────────────────────────────────

/// Fixed vertical space reserved on every line for decoration captions.
///
/// Set once via [`crate::code_editor::EditorConfig::annotation_strip`].
/// Enabling it grows EVERY line by `px`, even lines with no decorations —
/// this keeps scroll math constant-time and word-wrap invariants stable
/// (all lines have identical height). The cost is intentional; without a
/// constant strip, captions have nowhere honest to render.
///
/// `Off` (default) is a zero-cost no-op: the editor behaves byte-for-byte
/// like it did before this feature was added.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum AnnotationStrip {
    /// No caption space reserved. `Rule` / `Wash` captions are dropped
    /// silently on render.
    #[default]
    Off,
    /// Reserve `N` px above every line's text baseline. Captions render
    /// in that top band, text sits below.
    Above(f32),
    /// Reserve `N` px below every line's text baseline. Text sits at the
    /// top, captions render in the bottom band.
    Below(f32),
}

impl AnnotationStrip {
    /// Extra vertical pixels added to `line_height` (0 when [`Off`](Self::Off)).
    #[inline]
    pub fn extra_px(self) -> f32 {
        match self {
            AnnotationStrip::Off => 0.0,
            AnnotationStrip::Above(px) | AnnotationStrip::Below(px) => px.max(0.0),
        }
    }

    /// Vertical shift applied to token / cursor Y so text sits below the
    /// top band on [`Above`](Self::Above); zero for [`Off`](Self::Off)
    /// and [`Below`](Self::Below).
    #[inline]
    pub fn baseline_dy(self) -> f32 {
        match self {
            AnnotationStrip::Above(px) => px.max(0.0),
            _ => 0.0,
        }
    }
}

// ─── Line annotation ────────────────────────────────────────────────────────

/// All decorations attached to a single buffer line. Same lifecycle model
/// as [`crate::code_editor::LineMarker`] — hosts pass ownership, editor
/// only reads.
#[derive(Debug, Clone)]
pub struct LineAnnotation {
    /// 0-based line index. Out-of-range annotations are silently ignored
    /// at draw time (never panic).
    pub line: usize,
    /// Zero or more decorations. Draw order within a line follows
    /// `Wash → Rule → Ghost → EndPill` regardless of vec position.
    pub decorations: Vec<Decoration>,
}

impl LineAnnotation {
    /// Convenience constructor.
    pub fn new(line: usize, decorations: Vec<Decoration>) -> Self {
        Self { line, decorations }
    }
}

// ─── Decoration ─────────────────────────────────────────────────────────────

/// One drawing directive attached to a [`LineAnnotation`].
///
/// All positions are CHAR-COLS (same unit as [`crate::code_editor::CursorPos::col`]),
/// NOT byte offsets. This makes them tab-aware, UTF-8-safe, and directly
/// composable with cursor / selection positions.
#[derive(Debug, Clone)]
pub enum Decoration {
    /// Semi-transparent rectangle painted BEHIND the tokens (between
    /// selection/find and the text). Alpha typically 0.10-0.18.
    /// Split across sub-rows when the range crosses a word-wrap boundary.
    Wash {
        col_start: usize,
        col_len: usize,
        color: DecorationColor,
        /// Optional micro-caption drawn inside the annotation strip
        /// ([`AnnotationStrip::Above`] / [`Below`]). Ignored when the
        /// strip is [`AnnotationStrip::Off`].
        caption: Option<String>,
        /// Optional tooltip shown when the mouse hovers over the range.
        hover: Option<String>,
    },
    /// Thin colored bar (2-3 px) drawn on the text baseline as a
    /// highlighter-style underline. Drawn AFTER tokens (visible on top).
    /// Split by wrap the same way [`Wash`](Self::Wash) is.
    Rule {
        col_start: usize,
        col_len: usize,
        color: DecorationColor,
        caption: Option<String>,
        hover: Option<String>,
    },
    /// Dimmed pseudo-text drawn AFTER `col_start` on the same visual row.
    /// Purely visual — does NOT touch the text buffer, cursor position,
    /// hit-testing, wrap points, or scroll math. Truncated by the code
    /// column's right clip on narrow panels.
    Ghost {
        col_start: usize,
        text: String,
        color: DecorationColor,
    },
    /// Rounded pill drawn at the end of the LAST visual sub-row of the
    /// line, positioned after the last non-whitespace char + a gap of
    /// two `char_advance`s. Truncated by the right clip.
    EndPill {
        text: String,
        fg: DecorationColor,
        bg: DecorationColor,
        border: DecorationColor,
        hover: Option<String>,
    },
    /// Invisible hover region — draws NOTHING, but the editor's hover
    /// pass shows `hover` as a tooltip when the mouse sits over
    /// `[col_start, col_start + col_len)` on this line. Use this when a
    /// host wants column-scoped tooltips WITHOUT modifying the editor's
    /// visual output.
    HoverZone {
        col_start: usize,
        col_len: usize,
        hover: String,
    },
    /// Same footprint as [`HoverZone`] (invisible, column-scoped), but
    /// the tooltip content is a structured [`RichHoverPayload`] rendered
    /// with real ImGui widgets — separators, a table, per-cell colors —
    /// instead of a flat `String`. Use this when a host wants a rich
    /// tooltip whose typography benefits from real widgets (e.g. NxT's
    /// packet-detail tooltip).
    HoverZoneRich {
        col_start: usize,
        col_len: usize,
        payload: RichHoverPayload,
    },
}

// ─── Rich hover payload ─────────────────────────────────────────────────────

/// Column alignment for [`RichHoverStructure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichHoverAlign {
    Left,
    Right,
}

/// Header of a [`RichHoverPayload`]: `[title]  [chips]  [trailing]`
/// laid out horizontally with the chips as inline `key · value` pairs
/// between the title and the trailing note. All strings are rendered
/// with the current mono font.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverHeader {
    /// Left-side title. Rendered with the default text color.
    pub title: String,
    /// Inline `(label, value)` chips shown between title and trailing.
    /// Rendered as `label · value` with the label dim and the value in
    /// the default text color.
    pub chips: Vec<(String, String)>,
    /// Right-aligned trailing note (e.g. size in bytes). Rendered dim.
    pub trailing: String,
}

/// Middle section of a [`RichHoverPayload`]: a labeled block of
/// key-value rows. Rendered with a colored arrow prefix, the label in
/// bold-ish default color, and the rows indented as a two-column grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverFocus {
    /// Focus block label (e.g. the currently-focused field name).
    pub label: String,
    /// `(key, value)` rows shown under the label.
    pub rows: Vec<(String, String)>,
}

/// A single cell in [`RichHoverStructure`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverCell {
    pub text: String,
    /// Optional per-cell text color override. `None` uses the default.
    pub color: Option<[f32; 4]>,
}

impl RichHoverCell {
    /// Plain cell with default color.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
        }
    }
    /// Cell with an explicit color.
    pub fn colored(text: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            text: text.into(),
            color: Some(color),
        }
    }
}

/// A single row in [`RichHoverStructure`]. `cells.len()` must equal
/// `structure.columns.len()`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverRow {
    pub cells: Vec<RichHoverCell>,
    /// When `true`, dim the whole row (e.g. for `unknown` trailing bytes).
    pub muted: bool,
}

/// One column header + alignment in [`RichHoverStructure`].
#[derive(Debug, Clone, PartialEq)]
pub struct RichHoverColumn {
    pub name: String,
    pub align: RichHoverAlign,
}

/// Bottom section of a [`RichHoverPayload`]: a titled table with a
/// header row, N columns, and per-row `focused_row` highlight.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverStructure {
    /// Table title (e.g. "Full structure"). Empty string = no title row.
    pub title: String,
    pub columns: Vec<RichHoverColumn>,
    pub rows: Vec<RichHoverRow>,
    /// Index of the row that matches the currently-hovered decoration.
    /// The renderer highlights that row with a subtle background tint.
    pub focused_row: Option<usize>,
}

/// Structured payload carried by [`Decoration::HoverZoneRich`]. Each
/// section is optional-by-emptiness: a payload with an empty focus
/// simply skips the middle section, an empty `structure.rows` skips the
/// table, and so on. The renderer draws one `ui.separator()` between
/// non-empty sections.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichHoverPayload {
    pub header: RichHoverHeader,
    pub focus: RichHoverFocus,
    pub structure: RichHoverStructure,
}

/// Result of resolving a hovered decoration to its tooltip content.
///
/// The editor's hover pass returns this so the renderer can dispatch
/// on payload kind — plain text for [`Decoration::HoverZone`] / `Wash` /
/// `Rule`, structured widgets for [`Decoration::HoverZoneRich`].
#[derive(Debug, PartialEq)]
pub enum HoverContent<'a> {
    Text(&'a str),
    Rich(&'a RichHoverPayload),
}

// ─── Geometry helpers ───────────────────────────────────────────────────────

/// Clip a decoration's char-col range `[col_start, col_start + col_len)`
/// against a sub-row's visible column range `[sub_col_start, sub_col_end)`.
///
/// Returns `Some((visible_col_start, visible_col_len))` if the decoration
/// has any visible portion on this sub-row, or `None` if it doesn't
/// intersect at all. Zero-length decorations always yield `None` — a
/// decoration with no width can't be rendered.
///
/// Pure geometry — no editor state, unit-testable in isolation. Used by
/// the wash / rule draw passes to split a decoration that crosses a
/// word-wrap boundary into per-sub-row segments.
pub(super) fn clip_range_to_sub_row(
    col_start: usize,
    col_len: usize,
    sub_col_start: usize,
    sub_col_end: usize,
) -> Option<(usize, usize)> {
    if col_len == 0 {
        return None;
    }
    let deco_end = col_start.saturating_add(col_len);
    let visible_start = col_start.max(sub_col_start);
    let visible_end = deco_end.min(sub_col_end);
    if visible_start >= visible_end {
        return None;
    }
    Some((visible_start, visible_end - visible_start))
}

/// Find the first `Wash` / `Rule` / `HoverZone` / `HoverZoneRich`
/// decoration whose column range covers `hover_col` AND carries hover
/// content. Returns a [`HoverContent`] borrow, or `None` if nothing
/// matches.
///
/// Vec-order wins on overlaps — first entry with hover content takes
/// the tip. Ghost and EndPill are intentionally skipped: Ghost has no
/// hover field by design, and EndPill uses pixel hit-testing (its
/// position depends on `line_str`, not on column ranges).
pub(super) fn find_hovered_decoration(
    decorations: &[Decoration],
    hover_col: usize,
) -> Option<HoverContent<'_>> {
    for deco in decorations {
        let (dc, dlen, content) = match deco {
            Decoration::Wash {
                col_start,
                col_len,
                hover: Some(h),
                ..
            }
            | Decoration::Rule {
                col_start,
                col_len,
                hover: Some(h),
                ..
            } => (*col_start, *col_len, HoverContent::Text(h.as_str())),
            Decoration::HoverZone {
                col_start,
                col_len,
                hover,
            } => (*col_start, *col_len, HoverContent::Text(hover.as_str())),
            Decoration::HoverZoneRich {
                col_start,
                col_len,
                payload,
            } => (*col_start, *col_len, HoverContent::Rich(payload)),
            _ => continue,
        };
        if dlen == 0 {
            continue;
        }
        if hover_col >= dc && hover_col < dc + dlen {
            return Some(content);
        }
    }
    None
}

/// Last char-column of a line that contains a non-whitespace character,
/// counted so that `col_end == last_non_whitespace_col(line)` sits
/// immediately after the last visible glyph.
///
/// Trailing spaces / tabs are excluded — an `EndPill` should hug the
/// visible text, not float far to the right after a run of tabs.
pub(super) fn last_non_whitespace_col(line: &str) -> usize {
    let mut last = 0usize;
    let mut col = 0usize;
    for ch in line.chars() {
        col += 1;
        if !ch.is_whitespace() {
            last = col;
        }
    }
    last
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_resolves_to_itself() {
        let colors = SyntaxColors::default();
        let raw = DecorationColor::Raw([0.9, 0.4, 0.35, 0.5]);
        assert_eq!(raw.resolve(&colors), [0.9, 0.4, 0.35, 0.5]);
    }

    #[test]
    fn reuse_hex_null_matches_theme() {
        let colors = SyntaxColors::default();
        let reused = DecorationColor::Reuse(ColorSlot::HexNull);
        assert_eq!(reused.resolve(&colors), colors.hex_null);
    }

    #[test]
    fn reuse_string_matches_theme() {
        let colors = SyntaxColors::default();
        let reused = DecorationColor::Reuse(ColorSlot::String);
        assert_eq!(reused.resolve(&colors), colors.string);
    }

    #[test]
    fn semantic_reads_from_decoration_slots() {
        let mut colors = SyntaxColors::default();
        let want = [0.12, 0.34, 0.56, 0.78];
        colors.decoration_slots[2] = want;
        let sem = DecorationColor::Semantic(SemanticSlot::S3);
        assert_eq!(sem.resolve(&colors), want);
    }

    #[test]
    fn semantic_slot_index_covers_all_eight() {
        // Round-trip every slot to make sure `index()` maps 1:1 and stays
        // within 0..8 — a regression here would silently shift theme colours.
        let all = [
            SemanticSlot::S1,
            SemanticSlot::S2,
            SemanticSlot::S3,
            SemanticSlot::S4,
            SemanticSlot::S5,
            SemanticSlot::S6,
            SemanticSlot::S7,
            SemanticSlot::S8,
        ];
        for (want_ix, slot) in all.iter().enumerate() {
            assert_eq!(slot.index(), want_ix);
        }
    }

    #[test]
    fn annotation_strip_off_costs_nothing() {
        assert_eq!(AnnotationStrip::Off.extra_px(), 0.0);
        assert_eq!(AnnotationStrip::Off.baseline_dy(), 0.0);
    }

    #[test]
    fn annotation_strip_above_reserves_and_shifts() {
        let s = AnnotationStrip::Above(12.0);
        assert_eq!(s.extra_px(), 12.0);
        assert_eq!(s.baseline_dy(), 12.0);
    }

    #[test]
    fn annotation_strip_below_reserves_without_shift() {
        let s = AnnotationStrip::Below(10.0);
        assert_eq!(s.extra_px(), 10.0);
        assert_eq!(s.baseline_dy(), 0.0);
    }

    #[test]
    fn annotation_strip_clamps_negative_pixels() {
        // Guarding against nonsense inputs — negative reserve would break
        // scroll math and produce sub-zero row heights.
        assert_eq!(AnnotationStrip::Above(-5.0).extra_px(), 0.0);
        assert_eq!(AnnotationStrip::Below(-5.0).extra_px(), 0.0);
    }

    #[test]
    fn annotation_strip_default_is_off() {
        // A newly-defaulted config must not silently start reserving space.
        assert_eq!(AnnotationStrip::default(), AnnotationStrip::Off);
    }

    /// Every built-in theme must fill all 8 decoration slots with
    /// theme-specific colours (not the neutral grey placeholder).
    /// Verified by asserting no slot equals the `default_decoration_slots`
    /// fallback grey — a regression here means a preset forgot to
    /// override the field.
    #[test]
    fn all_presets_override_decoration_slots() {
        use crate::code_editor::syntax_colors::EditorTheme;
        let placeholder = SyntaxColors::default_decoration_slots()[0];
        for theme in EditorTheme::ALL {
            let colors = theme.colors();
            for (ix, slot) in colors.decoration_slots.iter().enumerate() {
                assert_ne!(
                    *slot,
                    placeholder,
                    "theme {:?} slot S{} still at placeholder grey — preset missing decoration_slots override",
                    theme,
                    ix + 1
                );
            }
        }
    }

    /// Every theme's 8 decoration slots must be visually distinct so
    /// hosts can rely on "S1 ≠ S2 ≠ …" for role differentiation. Also
    /// enforces opaque source (alpha = 1.0) — decorations apply their
    /// own alpha at draw time via `with_alpha`.
    #[test]
    fn preset_decoration_slots_are_distinct_and_opaque() {
        use crate::code_editor::syntax_colors::EditorTheme;
        for theme in EditorTheme::ALL {
            let colors = theme.colors();
            let slots = &colors.decoration_slots;
            for i in 0..8 {
                assert!(
                    (slots[i][3] - 1.0).abs() < 1e-6,
                    "theme {:?} S{} not opaque (alpha={})",
                    theme,
                    i + 1,
                    slots[i][3]
                );
                for j in (i + 1)..8 {
                    assert_ne!(
                        slots[i],
                        slots[j],
                        "theme {:?} slots S{} and S{} are identical",
                        theme,
                        i + 1,
                        j + 1
                    );
                }
            }
        }
    }

    /// Default `EditorConfig` must have the annotation strip disabled —
    /// enabling it is opt-in because it adds vertical pixels to every
    /// line, and existing consumers shouldn't see any visual change on
    /// upgrade. Also verifies the built-in `config.ron` still parses
    /// after adding a new field.
    #[test]
    fn editor_config_default_has_annotation_strip_off() {
        use crate::code_editor::EditorConfig;
        let cfg = EditorConfig::default();
        assert_eq!(cfg.annotation_strip, AnnotationStrip::Off);
    }

    // ── find_hovered_decoration — hover lookup ───────────────────────

    fn wash_hover(col_start: usize, col_len: usize, hover: &str) -> Decoration {
        Decoration::Wash {
            col_start,
            col_len,
            color: DecorationColor::Semantic(SemanticSlot::S1),
            caption: None,
            hover: Some(hover.into()),
        }
    }

    #[test]
    fn hover_returns_matching_wash_text() {
        let decos = vec![wash_hover(0, 2, "op = 0x1F")];
        assert_eq!(
            find_hovered_decoration(&decos, 1),
            Some(HoverContent::Text("op = 0x1F"))
        );
    }

    #[test]
    fn hover_matches_first_of_multiple_overlaps() {
        // First-hit-wins — decorations near the top of the vec take
        // precedence. Predictable ordering matters for hosts that layer
        // per-field vs per-group hovers.
        let decos = vec![wash_hover(0, 10, "outer"), wash_hover(2, 4, "inner")];
        assert_eq!(
            find_hovered_decoration(&decos, 3),
            Some(HoverContent::Text("outer"))
        );
    }

    #[test]
    fn hover_column_outside_all_ranges_is_none() {
        let decos = vec![wash_hover(0, 3, "op"), wash_hover(10, 4, "field")];
        assert_eq!(find_hovered_decoration(&decos, 6), None);
    }

    #[test]
    fn hover_skips_decorations_without_hover_text() {
        // Wash with hover: None must not steal a hit from a later Wash
        // that does carry a tooltip on the same column.
        let no_hover = Decoration::Wash {
            col_start: 0,
            col_len: 5,
            color: DecorationColor::Semantic(SemanticSlot::S1),
            caption: None,
            hover: None,
        };
        let with_hover = wash_hover(0, 5, "field");
        let decos = vec![no_hover, with_hover];
        assert_eq!(
            find_hovered_decoration(&decos, 2),
            Some(HoverContent::Text("field"))
        );
    }

    #[test]
    fn hover_ignores_ghost_and_pill() {
        // Ghost has no hover field; EndPill hover is handled separately
        // by pixel hit-test, not by column-hover. Both must be skipped.
        let decos = vec![
            Decoration::Ghost {
                col_start: 3,
                text: "hint".into(),
                color: DecorationColor::Raw([1.0, 1.0, 1.0, 1.0]),
            },
            Decoration::EndPill {
                text: "pill".into(),
                fg: DecorationColor::Raw([1.0; 4]),
                bg: DecorationColor::Raw([0.0; 4]),
                border: DecorationColor::Raw([0.5; 4]),
                hover: Some("pill-hover".into()),
            },
        ];
        assert_eq!(find_hovered_decoration(&decos, 3), None);
    }

    #[test]
    fn hover_returns_rich_payload_for_hover_zone_rich() {
        // HoverZoneRich carries a structured payload — the lookup must
        // return the Rich variant (borrowing the payload in place), not
        // fall through to a Text tooltip.
        let payload = RichHoverPayload {
            header: RichHoverHeader {
                title: "Packet".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let decos = vec![Decoration::HoverZoneRich {
            col_start: 0,
            col_len: 4,
            payload: payload.clone(),
        }];
        assert_eq!(
            find_hovered_decoration(&decos, 2),
            Some(HoverContent::Rich(&payload))
        );
    }

    #[test]
    fn hover_rich_zone_outside_range_is_none() {
        let decos = vec![Decoration::HoverZoneRich {
            col_start: 0,
            col_len: 3,
            payload: RichHoverPayload::default(),
        }];
        assert_eq!(find_hovered_decoration(&decos, 5), None);
    }

    // ── last_non_whitespace_col — EndPill anchor ─────────────────────

    #[test]
    fn last_col_of_plain_line() {
        // "hello" (5 chars) → last col == 5, EndPill starts after that.
        assert_eq!(last_non_whitespace_col("hello"), 5);
    }

    #[test]
    fn last_col_ignores_trailing_whitespace() {
        // Trailing spaces don't push the pill further right — the tail
        // whitespace is invisible on dark themes.
        assert_eq!(last_non_whitespace_col("ab cd    "), 5);
    }

    #[test]
    fn last_col_ignores_trailing_tabs() {
        // Same policy applies to tabs.
        assert_eq!(last_non_whitespace_col("ab\t\t"), 2);
    }

    #[test]
    fn last_col_of_empty_line_is_zero() {
        assert_eq!(last_non_whitespace_col(""), 0);
        assert_eq!(last_non_whitespace_col("     "), 0);
    }

    // ── clip_range_to_sub_row — pure geometry ─────────────────────────

    #[test]
    fn clip_wholly_inside_sub_row() {
        // Decoration [5..8) fully within sub-row cols [0..20).
        assert_eq!(clip_range_to_sub_row(5, 3, 0, 20), Some((5, 3)));
    }

    #[test]
    fn clip_starts_before_sub_row() {
        // [2..7) vs sub-row [5..10) → visible portion is [5..7).
        assert_eq!(clip_range_to_sub_row(2, 5, 5, 10), Some((5, 2)));
    }

    #[test]
    fn clip_ends_after_sub_row() {
        // [5..15) vs sub-row [0..10) → visible portion is [5..10).
        assert_eq!(clip_range_to_sub_row(5, 10, 0, 10), Some((5, 5)));
    }

    #[test]
    fn clip_wholly_before_sub_row_is_none() {
        // [0..3) is entirely to the left of [5..10) — invisible on this row.
        assert_eq!(clip_range_to_sub_row(0, 3, 5, 10), None);
    }

    #[test]
    fn clip_wholly_after_sub_row_is_none() {
        // [15..20) is past the right of [0..10) — invisible.
        assert_eq!(clip_range_to_sub_row(15, 5, 0, 10), None);
    }

    #[test]
    fn clip_zero_length_is_none() {
        // Zero-length decoration ranges are always invisible — even when
        // col_start sits inside the sub-row.
        assert_eq!(clip_range_to_sub_row(5, 0, 0, 20), None);
    }

    #[test]
    fn clip_touching_right_boundary_is_none() {
        // [5..10) vs [10..20) — meets exactly, but no visible cells.
        assert_eq!(clip_range_to_sub_row(5, 5, 10, 20), None);
    }

    // ── API tests (setters + query on CodeEditor) ────────────────────

    fn wash(col_start: usize, col_len: usize) -> Decoration {
        Decoration::Wash {
            col_start,
            col_len,
            color: DecorationColor::Semantic(SemanticSlot::S1),
            caption: None,
            hover: None,
        }
    }

    #[test]
    fn set_line_annotations_replaces_all() {
        use crate::code_editor::CodeEditor;
        let mut ed = CodeEditor::new("test");
        ed.set_line_annotations(vec![
            LineAnnotation::new(0, vec![wash(0, 2)]),
            LineAnnotation::new(1, vec![wash(3, 4)]),
        ]);
        assert_eq!(ed.line_annotations().len(), 2);
        // Replace with a single-line set — the previous two vanish entirely.
        ed.set_line_annotations(vec![LineAnnotation::new(5, vec![wash(0, 1)])]);
        assert_eq!(ed.line_annotations().len(), 1);
        assert_eq!(ed.line_annotations()[0].line, 5);
    }

    #[test]
    fn set_line_annotations_for_upserts_one_line() {
        use crate::code_editor::CodeEditor;
        let mut ed = CodeEditor::new("test");
        ed.set_line_annotations(vec![
            LineAnnotation::new(0, vec![wash(0, 2)]),
            LineAnnotation::new(1, vec![wash(3, 4)]),
        ]);
        // Update only line 1 — line 0 must stay intact.
        ed.set_line_annotations_for(1, vec![wash(9, 9), wash(11, 1)]);
        assert_eq!(ed.line_annotations().len(), 2);
        let ann0 = ed.line_annotations().iter().find(|a| a.line == 0).unwrap();
        let ann1 = ed.line_annotations().iter().find(|a| a.line == 1).unwrap();
        assert_eq!(ann0.decorations.len(), 1);
        assert_eq!(ann1.decorations.len(), 2);
    }

    #[test]
    fn set_line_annotations_for_inserts_missing_line() {
        use crate::code_editor::CodeEditor;
        let mut ed = CodeEditor::new("test");
        assert!(ed.line_annotations().is_empty());
        ed.set_line_annotations_for(7, vec![wash(0, 3)]);
        assert_eq!(ed.line_annotations().len(), 1);
        assert_eq!(ed.line_annotations()[0].line, 7);
    }

    #[test]
    fn set_line_annotations_for_empty_removes_entry() {
        use crate::code_editor::CodeEditor;
        let mut ed = CodeEditor::new("test");
        ed.set_line_annotations_for(2, vec![wash(0, 2)]);
        assert_eq!(ed.line_annotations().len(), 1);
        // Passing an empty vec is the documented way to clear a single line.
        ed.set_line_annotations_for(2, vec![]);
        assert!(ed.line_annotations().is_empty());
    }

    #[test]
    fn clear_line_annotations_wipes_everything() {
        use crate::code_editor::CodeEditor;
        let mut ed = CodeEditor::new("test");
        ed.set_line_annotations(vec![
            LineAnnotation::new(0, vec![wash(0, 2)]),
            LineAnnotation::new(9, vec![wash(1, 3)]),
        ]);
        ed.clear_line_annotations();
        assert!(ed.line_annotations().is_empty());
    }

    #[test]
    fn set_cursor_moves_caret() {
        use crate::code_editor::{CodeEditor, buffer::CursorPos};
        let mut ed = CodeEditor::new("test");
        ed.set_text("line one\nline two\nline three");
        ed.set_cursor(CursorPos { line: 2, col: 4 });
        let c = ed.cursor();
        assert_eq!(c.line, 2);
        assert_eq!(c.col, 4);
    }

    #[test]
    fn set_cursor_clamps_out_of_range() {
        // Out-of-range positions must clamp, not panic — hosts call this
        // from response paths that don't know the current line count.
        use crate::code_editor::{CodeEditor, buffer::CursorPos};
        let mut ed = CodeEditor::new("test");
        ed.set_text("only line");
        ed.set_cursor(CursorPos {
            line: 999,
            col: 999,
        });
        let c = ed.cursor();
        assert_eq!(c.line, 0); // clamped to only-line
        assert_eq!(c.col, 9); // clamped to line length
    }

    #[test]
    fn line_annotation_constructor_preserves_fields() {
        let ann = LineAnnotation::new(
            3,
            vec![Decoration::Wash {
                col_start: 0,
                col_len: 2,
                color: DecorationColor::Semantic(SemanticSlot::S1),
                caption: Some("op".into()),
                hover: None,
            }],
        );
        assert_eq!(ann.line, 3);
        assert_eq!(ann.decorations.len(), 1);
        match &ann.decorations[0] {
            Decoration::Wash {
                col_start,
                col_len,
                caption,
                ..
            } => {
                assert_eq!(*col_start, 0);
                assert_eq!(*col_len, 2);
                assert_eq!(caption.as_deref(), Some("op"));
            }
            _ => panic!("expected Wash variant"),
        }
    }
}
