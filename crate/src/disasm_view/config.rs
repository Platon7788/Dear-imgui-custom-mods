//! Configuration types for DisasmView.

// ── Column Config ───────────────────────────────────────────────────────────

/// Column widths for the disassembly view.
///
/// The Comment column behaves specially: `comment` is the
/// **minimum** width, but the actual rendered width per frame is
/// `(window_width - everything_else_to_the_left).max(comment)` —
/// Comment has the lowest layout priority and stretches to fill
/// the remaining space (per user request, 2026-04-30).
///
/// "Instruction" as displayed = `mnemonic + operands`. Defaults
/// (2026-04-30) are sized to the user's preferred 200 / 300 /
/// remaining split: Bytes 200 px, Instruction 300 px (mnemonic 80
/// + operands 220), Comment min 120 → grows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnWidths {
    /// Breakpoint margin (left gutter).
    pub margin: f32,
    /// Arrow/branch indicator area.
    pub arrows: f32,
    /// Address column.
    pub address: f32,
    /// Raw bytes column.
    pub bytes: f32,
    /// Mnemonic (opcode) column — first half of the visual
    /// "Instruction" column.
    pub mnemonic: f32,
    /// Operands column — second half of the visual "Instruction"
    /// column.
    pub operands: f32,
    /// **Minimum** Comment column width. Actual render width per
    /// frame is `(window_w - left_columns_total).max(comment)` so
    /// the comment area stretches to fill the host window.
    pub comment: f32,
}

impl Default for ColumnWidths {
    /// Loads `column_widths.ron` — the canonical column geometry.
    /// `disasm_view/config.ron` inlines the same field set; drift is
    /// guarded by `columns_inline_in_config_ron_matches_canonical`.
    fn default() -> Self {
        ron::from_str(include_str!("column_widths.ron"))
            .expect("built-in disasm_view/column_widths.ron is valid")
    }
}

// ── Syntax Colors ───────────────────────────────────────────────────────────

/// Color theme for disassembly syntax highlighting.
///
/// **Backwards-compatibility alias** — the canonical type
/// [`crate::theme::DisasmViewColors`] now lives in `theme::palettes`,
/// so every built-in [`crate::theme::Theme`] can hand out a matching
/// disassembly palette via
/// [`crate::theme::Theme::disasm_view_colors`]. The 26 colour fields
/// (mnemonic per-FlowKind, operand syntax, address/bytes/comment,
/// branch arrows, block tints, breakpoint markers, current-line /
/// selection / hover / header / separator) are unchanged — code that
/// referenced `DisasmColors` directly stays working through this
/// alias.
pub type DisasmColors = crate::theme::DisasmViewColors;

fn default_show_explanation() -> bool {
    true
}

fn default_show_idiom() -> bool {
    true
}

fn default_show_gotcha() -> bool {
    true
}

fn default_show_operand_hint() -> bool {
    true
}

fn default_show_compiler_pattern() -> bool {
    true
}

fn default_show_antidisasm() -> bool {
    true
}

fn default_show_boundary() -> bool {
    true
}

fn default_show_branch_direction() -> bool {
    true
}

fn default_disasm_colors() -> DisasmColors {
    DisasmColors::default()
}

// Forward-compat helpers for the older field set — every public
// `pub` field on `DisasmViewConfig` ships with `#[serde(default =
// "fn")]` so older saved `config.ron` files that pre-date a field
// keep parsing. The values match `config.ron`; keep them in sync.
fn default_columns() -> ColumnWidths {
    ColumnWidths::default()
}
fn default_show_bytes() -> bool {
    true
}
fn default_show_comments() -> bool {
    true
}
fn default_show_arrows() -> bool {
    true
}
fn default_show_breakpoints() -> bool {
    true
}
fn default_show_bookmarks() -> bool {
    true
}
fn default_icons_available() -> bool {
    true
}
fn default_show_block_tints() -> bool {
    false
}
fn default_show_header() -> bool {
    true
}
fn default_show_column_dividers() -> bool {
    true
}
fn default_uppercase() -> bool {
    true
}
fn default_address_width_64() -> bool {
    true
}
fn default_byte_category_colors() -> bool {
    true
}
fn default_editable() -> bool {
    false
}
fn default_follow_execution() -> bool {
    false
}
fn default_base_address() -> u64 {
    0
}
fn default_max_arrows() -> usize {
    256
}

// ── Disasm View Config ──────────────────────────────────────────────────────

/// Configuration for the disassembly view widget.
///
/// Every `pub` field carries `#[serde(default = "...")]` so older
/// saved `config.ron` files (pre-dating a field) still parse — the
/// migration default matches `config.ron`. Keep the helpers in sync
/// when changing a default on the value side.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisasmViewConfig {
    // ── Layout ──────────────────────────────────────────────
    /// Column widths.
    #[serde(default = "default_columns")]
    pub columns: ColumnWidths,
    /// Show raw hex bytes column.
    #[serde(default = "default_show_bytes")]
    pub show_bytes: bool,
    /// Show comment column.
    #[serde(default = "default_show_comments")]
    pub show_comments: bool,
    /// Show branch arrows.
    #[serde(default = "default_show_arrows")]
    pub show_arrows: bool,
    /// Show breakpoint markers in margin.
    #[serde(default = "default_show_breakpoints")]
    pub show_breakpoints: bool,
    /// Show bookmark markers in the margin gutter for any row whose
    /// address is in [`crate::disasm_view::DisasmView::bookmarks`].
    /// Default `true`. Bookmarks coexist with breakpoints — both can be
    /// drawn on the same row (filled BP dot + bookmark glyph overlay).
    #[serde(default = "default_show_bookmarks")]
    pub show_bookmarks: bool,
    /// Whether the active ImGui font carries the MDI glyph range
    /// (U+F0000–U+FFFFF) used by [`crate::icons`]. When `true`
    /// (default) the bookmark marker draws as the proper
    /// [`crate::icons::BOOKMARK_CHECK_OUTLINE`] glyph; when `false`
    /// it falls back to a ring outline so the marker still reads on
    /// hosts that haven't registered the icon font (`?`-box would
    /// appear otherwise).
    #[serde(default = "default_icons_available")]
    pub icons_available: bool,
    /// Show alternating per-block background tinting (semantic hues
    /// at low alpha, rotated by `block_index`). Default **`false`**
    /// — the tint reads as visual noise during normal browsing /
    /// editing; turn it on only when a block-boundary cue is
    /// explicitly desired (e.g. CFG-aware reverse-engineering view).
    #[serde(default = "default_show_block_tints")]
    pub show_block_tints: bool,
    /// Show column header.
    #[serde(default = "default_show_header")]
    pub show_header: bool,
    /// Show thin vertical dividers between the address / bytes /
    /// instruction / comment columns. Default: `true`. Mirrors
    /// `HexViewerConfig::show_column_dividers` — uses
    /// `colors.separator` with alpha 0.40 so the lines read as a
    /// gentle visual cue, not heavy borders.
    #[serde(default = "default_show_column_dividers")]
    pub show_column_dividers: bool,
    /// Address format: true for uppercase hex.
    #[serde(default = "default_uppercase")]
    pub uppercase: bool,
    /// Address width: 32-bit (8 chars) or 64-bit (16 chars).
    #[serde(default = "default_address_width_64")]
    pub address_width_64: bool,
    /// Per-byte category colouring in the Bytes column. When `true`
    /// (default), each byte is tinted by `ByteCategory` (zero /
    /// control / printable / high / `0xFF`) — same 5-tier scheme
    /// `hex_viewer` uses, so the same buffer reads identically
    /// across both widgets. When `false`, every byte uses the flat
    /// [`DisasmColors::bytes`] colour.
    #[serde(default = "default_byte_category_colors")]
    pub byte_category_colors: bool,

    // ── Behavior ────────────────────────────────────────────
    /// Allow inline instruction editing.
    #[serde(default = "default_editable")]
    pub editable: bool,
    /// Auto-scroll to follow current execution point.
    #[serde(default = "default_follow_execution")]
    pub follow_execution: bool,
    /// Base address offset (for relative display).
    #[serde(default = "default_base_address")]
    pub base_address: u64,
    /// Maximum visible arrows per render (for performance).
    #[serde(default = "default_max_arrows")]
    pub max_arrows: usize,

    /// Show the educational "What it does: …" line at the bottom of
    /// the instruction hover tooltip. Resolves the mnemonic against
    /// [`disasm_knowledge::mnemonic`] and emits a one-line plain-
    /// language description in the active locale. Default `true` —
    /// the line is helpful to beginners and a no-op for the ~5 % of
    /// uncommon mnemonics that aren't catalogued. Set `false` for
    /// senior-RE / minimal-tooltip workflows.
    /// `#[serde(default = "default_show_explanation")]` so older
    /// `config.ron` files still parse and inherit the `true` default.
    #[serde(default = "default_show_explanation")]
    pub show_explanation: bool,

    /// Show the multi-instruction idiom hint at the bottom of the
    /// tooltip ("Pattern: function prologue", "Pattern: cmp+Jcc",
    /// "Pattern: NULL check", …). Pulls from
    /// [`disasm_knowledge::idiom`] which inspects the previous /
    /// current / next instructions. Default `true`.
    #[serde(default = "default_show_idiom")]
    pub show_idiom: bool,

    /// Show the anti-RE / anti-debug / obfuscation "gotcha" line for
    /// mnemonics protectors love to abuse (`rdtsc`, `cpuid`, `int 3`,
    /// `popf`, `xor`/`rol`/`lea` mutator filler, ...). Default `true`
    /// — the warnings teach common reverse-engineering tricks
    /// without forcing the user to memorise them.
    #[serde(default = "default_show_gotcha")]
    pub show_gotcha: bool,

    /// Show the operand-pattern decoder line in the tooltip — turns
    /// `[rcx+rax*8+8]` into "Array indexing: rcx is base, rax is
    /// element index ×8 (qword elements), then add 0x8". Pulls
    /// register-role hints from [`disasm_knowledge::abi`] so
    /// `[rcx]` says "dereferences the 1st argument under Win64".
    /// Default `true`.
    #[serde(default = "default_show_operand_hint")]
    pub show_operand_hint: bool,

    /// Show the compiler-pattern recogniser line in the tooltip —
    /// labels Win64-leaf frames, `__chkstk` probes, vtable indirect
    /// calls, MSVC `/GS` security cookies, atomic CAS / RMW, IAT
    /// thunks, PEB / TEB / TIB accesses via `gs:` / `fs:`. Pulls
    /// from [`disasm_knowledge::compiler`]. Default `true`.
    #[serde(default = "default_show_compiler_pattern")]
    pub show_compiler_pattern: bool,

    /// Show the anti-disasm / anti-debug trick recogniser — flags
    /// stack-based control flow (`push imm; ret`), opaque
    /// predicates, self-modifying code, anti-VM CPUID checks,
    /// trap-flag debugger probes, jump-into-next-byte tricks,
    /// `rdtsc`-delta timing measurements. Pulls from
    /// [`disasm_knowledge::antidisasm`]. Default `true` — these
    /// are the patterns analysts most need flagged when poking at
    /// protected code.
    #[serde(default = "default_show_antidisasm")]
    pub show_antidisasm: bool,

    /// Show the function / basic-block boundary recogniser line —
    /// labels function prologues (framed `push rbp; mov rbp, rsp`,
    /// CET `endbr64` landing pad), epilogues (`leave; ret`,
    /// `pop rbp; ret`, `add rsp, N; ret`), bare returns, and
    /// block-level terminators (unconditional `jmp`, conditional
    /// `Jcc` forks). Pulls from [`disasm_knowledge::boundary`].
    /// Default `true`.
    #[serde(default = "default_show_boundary")]
    pub show_boundary: bool,

    /// Show the branch-direction hint for any instruction with a
    /// resolved branch target — labels forward jumps as typical
    /// `if` / `match` / `switch` skip-overs, backward jumps as
    /// loops, and self-targeting jumps as anti-RE spin / `jmp $`
    /// traps. Pulls from [`disasm_knowledge::branch`]. Default
    /// `true`.
    #[serde(default = "default_show_branch_direction")]
    pub show_branch_direction: bool,

    /// Target calling convention. Drives the
    /// [`disasm_knowledge::abi::role`] table that powers register-
    /// role hints in the tooltip and the idiom detector. Default
    /// [`Abi::Win64`] — the most common modern Windows reverse-
    /// engineering target. Switch to [`Abi::SysVAmd64`] for Linux
    /// / macOS x64 binaries; pick a 32-bit variant for x86 code.
    #[serde(default)]
    pub abi: disasm_knowledge::abi::Abi,

    /// User-visible language for labels, popups, tooltips, and menu
    /// items rendered by the disassembler view. Default
    /// [`crate::i18n::Locale::En`]. Switching to
    /// [`crate::i18n::Locale::Ru`] requires the host to bake
    /// `GlyphRanges::Cyrillic` (or a superset) into the active font
    /// atlas — otherwise non-ASCII characters render as `?`.
    /// `#[serde(default)]` so older `config.ron` files that pre-date
    /// the locale field deserialise cleanly (falling back to English).
    #[serde(default)]
    pub locale: crate::i18n::Locale,

    /// Verbosity tier applied uniformly to every educational tooltip
    /// line (`show_explanation` / `show_idiom` / `show_gotcha` /
    /// `show_operand_hint` / `show_compiler_pattern` /
    /// `show_antidisasm` / `show_boundary` / `show_branch_direction`).
    /// Default [`disasm_knowledge::HintVerbosity::Standard`] — the
    /// one-to-two-sentence wording the catalogue shipped with through
    /// v0.11.x. `Compact` strips to a telegraphic single clause;
    /// `Educational` expands to three-to-five-sentence context for
    /// learners using RE as study material.
    ///
    /// `#[serde(default)]` keeps older saved configs parseable. Lives
    /// in the headless `disasm-knowledge` crate (ADR-030) so other
    /// consumers — CLI tools, IDA plugins — can pick the same tier
    /// without depending on the UI library.
    #[serde(default)]
    pub verbosity: disasm_knowledge::HintVerbosity,

    // ── Colors ──────────────────────────────────────────────
    /// Full color theme.
    #[serde(skip, default = "default_disasm_colors")]
    pub colors: DisasmColors,
}

impl Default for DisasmViewConfig {
    fn default() -> Self {
        let mut cfg: DisasmViewConfig = ron::from_str(include_str!("config.ron"))
            .expect("built-in disasm_view/config.ron is valid");
        cfg.colors = DisasmColors::default();
        cfg
    }
}

impl DisasmViewConfig {
    /// Replace the embedded [`DisasmColors`] palette with the given
    /// theme-driven [`crate::theme::DisasmViewColors`]. Use this on
    /// theme switch so the disassembly stays in the same visual
    /// family as `nav_panel` / `status_bar` / `hex_viewer`.
    ///
    /// ```rust,ignore
    /// view.config_mut().apply_theme_colors(&Theme::Solarized.disasm_view_colors());
    /// ```
    pub fn apply_theme_colors(&mut self, p: &crate::theme::DisasmViewColors) {
        self.colors = p.clone();
    }

    /// Convenience builder: start from `Default`, then apply the named
    /// theme's disasm-view palette in one call.
    ///
    /// ```rust,ignore
    /// use dear_imgui_custom_mod::disasm_view::DisasmViewConfig;
    /// use dear_imgui_custom_mod::theme::Theme;
    ///
    /// let cfg = DisasmViewConfig::default().with_theme(Theme::Nord);
    /// ```
    pub fn with_theme(mut self, theme: crate::theme::Theme) -> Self {
        self.apply_theme_colors(&theme.disasm_view_colors());
        self
    }
}
