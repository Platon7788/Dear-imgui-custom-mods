//! Disassembly-view palette types — [`DisasmFlowKind`], the 26-field
//! [`DisasmViewColors`] colour-token struct + helpers, and the
//! [`DisasmViewTokens`] seed bundle. Split out of [`super`] to keep each
//! palette file under the size limit. Re-exported from [`super`] so the
//! standard `crate::theme::DisasmViewColors` / `DisasmFlowKind` paths keep
//! working.

/// Instruction control-flow classification — re-exported here so the
/// palette factory below can build per-flow mnemonic / arrow colours
/// without pulling in the whole `disasm_view` module. Mirrors
/// [`crate::disasm_view::FlowKind`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DisasmFlowKind {
    /// Normal sequential instruction (mov, add, lea, etc.).
    #[default]
    Normal,
    /// Unconditional/conditional jump (jmp, je, jne, etc.).
    Jump,
    /// Call instruction.
    Call,
    /// Return instruction (ret, iret).
    Return,
    /// NOP / INT3 / padding instruction.
    Nop,
    /// Stack manipulation (push, pop, sub rsp).
    Stack,
    /// System instruction (syscall, sysenter, int).
    System,
    /// Invalid / undecodable instruction.
    Invalid,
}

/// Complete colour set for the [`crate::disasm_view::DisasmView`] widget
/// — 26 individual fields covering mnemonic colouring (per
/// [`DisasmFlowKind`]), operand syntax tinting, address / bytes /
/// comment, branch arrows, alternating block tints, breakpoint markers,
/// the current-execution highlight, selection / hover / header /
/// separator surfaces.
///
/// Built-in themes expose this via
/// [`crate::theme::Theme::disasm_view_colors`]; bare
/// `DisasmViewConfig::default()` uses [`DisasmViewColors::default`]
/// which mirrors `Theme::Dark.disasm_view_colors()`. Re-exported from
/// `crate::disasm_view` as `DisasmColors` for backwards compatibility.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisasmViewColors {
    // ── Mnemonic colors by flow kind ────────────────────────
    /// Normal instruction mnemonic (mov, add, lea, etc.).
    pub mnemonic_normal: [f32; 4],
    /// Jump/branch mnemonic.
    pub mnemonic_jump: [f32; 4],
    /// Call mnemonic.
    pub mnemonic_call: [f32; 4],
    /// Return mnemonic.
    pub mnemonic_return: [f32; 4],
    /// NOP/INT3/padding.
    pub mnemonic_nop: [f32; 4],
    /// Stack operations (push, pop).
    pub mnemonic_stack: [f32; 4],
    /// System instructions (syscall, int).
    pub mnemonic_system: [f32; 4],
    /// Invalid instruction.
    pub mnemonic_invalid: [f32; 4],

    // ── Operand colors ──────────────────────────────────────
    /// Register names.
    pub operand_register: [f32; 4],
    /// Numeric constants / immediates.
    pub operand_number: [f32; 4],
    /// Memory dereference brackets and operators.
    pub operand_memory: [f32; 4],
    /// String operands.
    pub operand_string: [f32; 4],
    /// Default operand text.
    pub operand_default: [f32; 4],

    // ── Address / bytes ─────────────────────────────────────
    /// Address column color.
    pub address: [f32; 4],
    /// Hex bytes column color — used as the fallback when
    /// per-byte category tinting is disabled.
    pub bytes: [f32; 4],
    /// Comment color.
    pub comment: [f32; 4],

    // ── Byte categories (mirrors `HexViewerColors` cat_*) ───
    // Per-byte semantic tint for the Bytes column. Picks the same
    // 5-tier ByteCategory split hex_viewer uses so the same buffer
    // reads identically across both widgets.
    /// `0x00` byte — null padding.
    pub bytes_cat_zero: [f32; 4],
    /// `0x01..=0x1F` + `0x7F` — control chars.
    pub bytes_cat_control: [f32; 4],
    /// `0x20..=0x7E` — printable ASCII.
    pub bytes_cat_printable: [f32; 4],
    /// `0x80..=0xFE` — high / extended.
    pub bytes_cat_high: [f32; 4],
    /// `0xFF` — all-ones byte.
    pub bytes_cat_full: [f32; 4],

    // ── Branch arrows ───────────────────────────────────────
    /// Jump arrow color.
    pub arrow_jump: [f32; 4],
    /// Call arrow color.
    pub arrow_call: [f32; 4],
    /// Return arrow color.
    pub arrow_return: [f32; 4],
    /// Default arrow color (other flow kinds).
    pub arrow_default: [f32; 4],

    // ── Block tinting ───────────────────────────────────────
    /// Background tint colors for alternating code blocks.
    pub block_tints: Vec<[f32; 4]>,

    // ── UI elements ─────────────────────────────────────────
    /// Breakpoint marker color (fallback when no number available).
    pub breakpoint: [f32; 4],
    /// Breakpoint gutter background.
    pub breakpoint_bg: [f32; 4],
    /// Numbered breakpoint colors (cycle through these).
    pub breakpoint_colors: Vec<[f32; 4]>,
    /// Current execution point (stopped-at) background fill.
    pub current_line_bg: [f32; 4],
    /// Outline drawn around the current-execution row on top of the
    /// translucent fill. Default `danger` family at near-full alpha
    /// — gives the IP marker a crisp 1-px border while keeping the
    /// fill subtle, so the row reads as "stopped here" without
    /// drowning the underlying mnemonic colours.
    pub current_line_border: [f32; 4],
    /// Selected row background.
    pub selection_bg: [f32; 4],
    /// Row hover highlight.
    pub hover_bg: [f32; 4],
    /// Background fill for instruction rows whose bytes match the
    /// active byte search. Sits between `current_line_bg` (warning
    /// hue, exec pointer) and `selection_bg` (accent hue, click)
    /// visually so the three highlights never read as the same
    /// state.
    pub search_match_bg: [f32; 4],
    /// Bookmark ring marker drawn in the breakpoint gutter for any
    /// row whose address is in [`crate::disasm_view::DisasmView::bookmarks`].
    /// Default: `accent` family — reads as a "navigation marker"
    /// distinct from the breakpoint hue (red/danger) and the
    /// current-line fill (warning/amber).
    pub bookmark: [f32; 4],
    /// Column header / separator color.
    pub header: [f32; 4],
    /// Separator line between columns.
    pub separator: [f32; 4],
}

impl Default for DisasmViewColors {
    /// Mirrors `Theme::Dark.disasm_view_colors()` — see
    /// `theme::widget_tests::disasm_view_colors_default_matches_dark_theme`.
    fn default() -> Self {
        crate::theme::dark::disasm_view_colors()
    }
}

impl DisasmViewColors {
    /// Get mnemonic color for a given flow kind.
    pub fn mnemonic_color(&self, kind: DisasmFlowKind) -> [f32; 4] {
        match kind {
            DisasmFlowKind::Normal => self.mnemonic_normal,
            DisasmFlowKind::Jump => self.mnemonic_jump,
            DisasmFlowKind::Call => self.mnemonic_call,
            DisasmFlowKind::Return => self.mnemonic_return,
            DisasmFlowKind::Nop => self.mnemonic_nop,
            DisasmFlowKind::Stack => self.mnemonic_stack,
            DisasmFlowKind::System => self.mnemonic_system,
            DisasmFlowKind::Invalid => self.mnemonic_invalid,
        }
    }

    /// Get arrow color for a given flow kind.
    pub fn arrow_color(&self, kind: DisasmFlowKind) -> [f32; 4] {
        match kind {
            DisasmFlowKind::Jump => self.arrow_jump,
            DisasmFlowKind::Call => self.arrow_call,
            DisasmFlowKind::Return => self.arrow_return,
            _ => self.arrow_default,
        }
    }

    /// Get breakpoint color by number (1-based). Falls back to
    /// [`Self::breakpoint`] when `number == 0` or no
    /// numbered colours are configured.
    pub fn bp_color(&self, number: u32) -> [f32; 4] {
        if number == 0 || self.breakpoint_colors.is_empty() {
            return self.breakpoint;
        }
        self.breakpoint_colors[((number - 1) as usize) % self.breakpoint_colors.len()]
    }

    /// Per-byte foreground colour for the Bytes column — same
    /// 5-tier `ByteCategory` split that
    /// [`crate::hex_viewer::HexViewerConfig::byte_fg_color`] uses, so
    /// the same buffer reads identically across both widgets. Always
    /// returns a category-tinted colour; pass `colors.bytes`
    /// directly at the call site to opt out.
    pub fn byte_fg_color(&self, byte: u8) -> [f32; 4] {
        match byte {
            0x00 => self.bytes_cat_zero,
            0x01..=0x1F | 0x7F => self.bytes_cat_control,
            0x20..=0x7E => self.bytes_cat_printable,
            0xFF => self.bytes_cat_full,
            _ => self.bytes_cat_high, // 0x80..=0xFE
        }
    }

    /// Get block tint color for a given block index.
    pub fn block_tint(&self, block_index: usize) -> [f32; 4] {
        if self.block_tints.is_empty() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        self.block_tints[block_index % self.block_tints.len()]
    }

    /// Build a [`DisasmViewColors`] from a small bundle of semantic
    /// tokens. Used by every per-theme `disasm_view_colors()` so the
    /// 26-field palette stays consistent — only the seed colours
    /// change between themes.
    pub fn from_tokens(t: &DisasmViewTokens) -> Self {
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        Self {
            // ── Mnemonics — semantic per FlowKind ────────────────
            mnemonic_normal: t.fg,
            mnemonic_jump: t.warning,
            mnemonic_call: t.success,
            mnemonic_return: t.danger,
            mnemonic_nop: with_a(t.fg_muted, 0.60),
            mnemonic_stack: t.purple,
            mnemonic_system: t.orange,
            mnemonic_invalid: t.danger,

            // ── Operands ─────────────────────────────────────────
            operand_register: t.cyan,
            operand_number: t.success,
            operand_memory: t.orange,
            operand_string: with_a(t.warning, 0.85),
            operand_default: with_a(t.fg, 0.95),

            // ── Address / bytes / comment ────────────────────────
            address: with_a(t.accent, 0.85),
            bytes: with_a(t.fg_muted, 0.85),
            comment: with_a(t.success, 0.75),

            // ── Byte categories — mirror hex_viewer ──────────────
            // Same 5-tier split (`HexViewerColors::from_tokens`) so
            // the Bytes column reads identically in both widgets.
            bytes_cat_zero: with_a(t.fg_muted, 0.45),
            bytes_cat_control: with_a(t.fg_muted, 0.70),
            bytes_cat_printable: t.success,
            bytes_cat_high: t.purple,
            bytes_cat_full: t.warning,

            // ── Branch arrows — same family as mnemonics ─────────
            arrow_jump: with_a(t.warning, 0.90),
            arrow_call: with_a(t.success, 0.90),
            arrow_return: with_a(t.danger, 0.90),
            arrow_default: with_a(t.fg_muted, 0.70),

            // ── Block tints — derived from semantic hues so the
            //    rotation reads on both light and dark surfaces. Alpha
            //    deliberately tiny (≤ 0.10) — the tint should hint
            //    at block boundaries, not compete with the byte text.
            block_tints: vec![
                with_a(t.accent, 0.07),
                with_a(t.danger, 0.06),
                with_a(t.success, 0.06),
                with_a(t.warning, 0.06),
                with_a(t.purple, 0.06),
                with_a(t.cyan, 0.06),
            ],

            // ── UI ───────────────────────────────────────────────
            breakpoint: t.danger,
            breakpoint_bg: with_a(t.danger, 0.30),
            breakpoint_colors: vec![
                t.danger,
                t.cyan,
                t.warning,
                t.success,
                t.purple,
                t.orange,
                t.accent,
                with_a(t.danger, 0.85),
            ],
            // Translucent fill — halved twice from the historical
            // 0.35 (0.35 → 0.18 → 0.09) so the per-row mnemonic /
            // operand colours read through almost untouched.
            // Paired with `current_line_border` for the crisp
            // marker outline.
            current_line_bg: with_a(t.warning, 0.09),
            // Thin red border traced around the current-execution
            // row. `danger` family @ 0.90 alpha gives the IP marker
            // a clear 1-px frame on top of the translucent fill.
            current_line_border: with_a(t.danger, 0.90),
            selection_bg: with_a(t.accent, 0.45),
            hover_bg: with_a(t.fg, 0.04),
            // Search-match row background — semantic-green (success)
            // hue at low alpha. Distinct from warning (exec) and
            // accent (selection) so the three row states never
            // collide visually.
            search_match_bg: with_a(t.success, 0.32),
            // Bookmark ring — accent family so the marker reads as
            // a "navigation aid" hue, distinct from breakpoint
            // (danger / red) and current-line (warning / amber).
            bookmark: t.accent,
            // Header row labels ("Address", "Bytes", "Instruction",
            // "Comment") — pinned to `fg` (full-strength text) so
            // they read as bright white in dark themes and bold dark
            // in light themes. Mirrors `HexViewerColors::header`,
            // which had the same washed-out problem before being
            // bumped from `fg_muted` to `fg`.
            header: t.fg,
            separator: with_a(t.fg_muted, 0.40),
        }
    }
}

/// Semantic token bundle each theme passes to
/// [`DisasmViewColors::from_tokens`]. Lets every per-theme palette be
/// expressed in 9 lines instead of reproducing all 26 disasm_view
/// fields by hand.
#[doc(hidden)]
pub struct DisasmViewTokens {
    /// Primary content text — `FG` of the theme.
    pub fg: [f32; 4],
    /// Muted text — `FG_MUTED`.
    pub fg_muted: [f32; 4],
    /// Theme accent — drives `address` (alpha-modulated) + `selection_bg`.
    pub accent: [f32; 4],
    /// Semantic green — `Call` mnemonic, numeric operands, comments.
    pub success: [f32; 4],
    /// Semantic amber — `Jump` mnemonic, current-line bg, jump arrows.
    pub warning: [f32; 4],
    /// Semantic red — `Return` / `Invalid` mnemonics, breakpoint marker.
    pub danger: [f32; 4],
    /// Purple-family hue — `Stack` mnemonic (`push` / `pop`), distinct
    /// from accent / warning so stack ops stand out.
    pub purple: [f32; 4],
    /// Orange-family hue — `System` mnemonic + memory operand brackets
    /// (`[rsp+0x10]`); MUST contrast against `cyan` and `success`.
    pub orange: [f32; 4],
    /// Cyan-family hue — register operand colour (`rax`, `xmm0`, etc.).
    /// Must contrast against the theme accent so registers don't merge
    /// into the address gutter.
    pub cyan: [f32; 4],
}
