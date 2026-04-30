//! Configuration types for [`DisasmView`](super::DisasmView).

// ── Flow Kind ───────────────────────────────────────────────────────────────

/// Instruction control-flow classification.
///
/// Re-exported from [`crate::theme::DisasmFlowKind`] so the palette
/// factory in `theme::palettes` can build per-flow mnemonic / arrow
/// colours without depending back on `disasm_view`. All variants are
/// identical to the historic local enum — `FlowKind::Jump` etc keeps
/// resolving to the same value.
pub use crate::theme::DisasmFlowKind as FlowKind;

// ── Instruction Trait ────────────────────────────────────────────────────────

/// Trait for a decoded instruction.
///
/// Implement this for your disassembly backend (iced-x86, capstone, etc.).
pub trait Instruction {
    /// Virtual address of the instruction.
    fn address(&self) -> u64;
    /// Raw instruction bytes.
    fn bytes(&self) -> &[u8];
    /// Mnemonic string (e.g. "mov", "call", "jmp").
    fn mnemonic(&self) -> &str;
    /// Formatted operand string (e.g. "rax, [rbp-0x10]").
    fn operands(&self) -> &str;
    /// Optional comment (string references, call target names).
    fn comment(&self) -> Option<&str> {
        None
    }
    /// Control flow classification.
    fn flow_kind(&self) -> FlowKind {
        FlowKind::Normal
    }
    /// Branch/call target address (if applicable).
    fn branch_target(&self) -> Option<u64> {
        None
    }
    /// Logical block index for block-tinting (0-based).
    fn block_index(&self) -> usize {
        0
    }
    /// Whether a breakpoint is set at this address.
    fn has_breakpoint(&self) -> bool {
        false
    }
    /// Breakpoint number (1-based). Used for colored numbered markers.
    /// Returns 0 if no breakpoint.
    fn breakpoint_number(&self) -> u32 {
        if self.has_breakpoint() { 1 } else { 0 }
    }
    /// Whether this is the current execution point (stopped-at).
    fn is_current(&self) -> bool {
        false
    }
}

// ── Data Provider Trait ─────────────────────────────────────────────────────

/// Trait for providing decoded instructions to the disasm view.
///
/// Implement this to bridge your disassembly engine (iced-x86, capstone, etc.)
/// with the UI component.
pub trait DisasmDataProvider {
    /// Total number of currently decoded instructions.
    fn instruction_count(&self) -> usize;

    /// Get instruction by index. Returns `None` if out of range.
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction>;

    /// Request decoding of instructions starting at `start_addr`.
    /// The provider should decode up to `max_count` instructions forward.
    /// This is called when the view scrolls to a new region.
    fn decode_range(&mut self, start_addr: u64, max_count: usize);

    /// Find the instruction index closest to `addr`.
    /// Returns `None` if the address is outside decoded range.
    fn index_of_address(&self, addr: u64) -> Option<usize>;

    /// Toggle breakpoint at address. Returns the new breakpoint state.
    fn toggle_breakpoint(&mut self, _addr: u64) -> bool {
        false
    }

    /// Assemble a text instruction into bytes at `addr`.
    /// Returns the assembled bytes or `None` on failure.
    fn assemble(&self, _addr: u64, _text: &str) -> Option<Vec<u8>> {
        None
    }

    /// Write bytes at address (for patching).
    fn write_bytes(&mut self, _addr: u64, _bytes: &[u8]) -> bool {
        false
    }

    /// Get a human-readable name for an address (symbol, export, label).
    fn symbol_name(&self, _addr: u64) -> Option<String> {
        None
    }

    /// Set the per-instruction comment at `addr`. Empty `text`
    /// should clear the comment. Returns `true` on success, `false`
    /// on failure (address not decoded yet, read-only provider,
    /// etc.). Default impl is a no-op returning `false` so existing
    /// providers stay non-breaking — implement when you want
    /// double-click-to-edit on the Comment column to round-trip
    /// through your data layer (see [`VecDisasmProvider::set_comment`]
    /// for the canonical impl).
    fn set_comment(&mut self, _addr: u64, _text: &str) -> bool {
        false
    }
}

// ── Default Instruction ─────────────────────────────────────────────────────

/// Concrete instruction entry for use with the built-in `VecDisasmProvider`.
#[derive(Debug, Clone)]
pub struct InstructionEntry {
    pub address: u64,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
    pub comment: Option<String>,
    pub flow_kind: FlowKind,
    pub branch_target: Option<u64>,
    pub block_index: usize,
    pub breakpoint: bool,
    /// Breakpoint number (1-based, 0 = none). Assigned automatically by provider.
    pub bp_number: u32,
    pub current: bool,
}

impl InstructionEntry {
    pub fn new(
        address: u64,
        bytes: Vec<u8>,
        mnemonic: impl Into<String>,
        operands: impl Into<String>,
    ) -> Self {
        Self {
            address,
            bytes,
            mnemonic: mnemonic.into(),
            operands: operands.into(),
            comment: None,
            flow_kind: FlowKind::Normal,
            branch_target: None,
            block_index: 0,
            breakpoint: false,
            bp_number: 0,
            current: false,
        }
    }

    pub fn with_flow(mut self, kind: FlowKind) -> Self {
        self.flow_kind = kind;
        self
    }
    pub fn with_target(mut self, target: u64) -> Self {
        self.branch_target = Some(target);
        self
    }
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }
    pub fn with_block(mut self, index: usize) -> Self {
        self.block_index = index;
        self
    }
    pub fn with_breakpoint(mut self, bp: bool) -> Self {
        self.breakpoint = bp;
        self
    }
    pub fn with_bp_number(mut self, n: u32) -> Self {
        self.bp_number = n;
        self.breakpoint = n > 0;
        self
    }
    pub fn with_current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
}

impl Instruction for InstructionEntry {
    fn address(&self) -> u64 {
        self.address
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn mnemonic(&self) -> &str {
        &self.mnemonic
    }
    fn operands(&self) -> &str {
        &self.operands
    }
    fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }
    fn flow_kind(&self) -> FlowKind {
        self.flow_kind
    }
    fn branch_target(&self) -> Option<u64> {
        self.branch_target
    }
    fn block_index(&self) -> usize {
        self.block_index
    }
    fn has_breakpoint(&self) -> bool {
        self.breakpoint
    }
    fn breakpoint_number(&self) -> u32 {
        self.bp_number
    }
    fn is_current(&self) -> bool {
        self.current
    }
}

// ── Vec Provider ────────────────────────────────────────────────────────────

/// Simple in-memory provider backed by `Vec<InstructionEntry>`.
pub struct VecDisasmProvider {
    instructions: Vec<InstructionEntry>,
}

impl VecDisasmProvider {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }
    pub fn from_vec(instructions: Vec<InstructionEntry>) -> Self {
        Self { instructions }
    }
    pub fn push(&mut self, instr: InstructionEntry) {
        self.instructions.push(instr);
    }
    pub fn clear(&mut self) {
        self.instructions.clear();
    }
    pub fn instructions(&self) -> &[InstructionEntry] {
        &self.instructions
    }
    pub fn instructions_mut(&mut self) -> &mut Vec<InstructionEntry> {
        &mut self.instructions
    }
}

impl Default for VecDisasmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DisasmDataProvider for VecDisasmProvider {
    fn instruction_count(&self) -> usize {
        self.instructions.len()
    }
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction> {
        self.instructions.get(idx).map(|i| i as &dyn Instruction)
    }
    fn decode_range(&mut self, _start_addr: u64, _max_count: usize) {
        // VecProvider has all instructions pre-loaded.
    }
    fn index_of_address(&self, addr: u64) -> Option<usize> {
        self.instructions.iter().position(|i| i.address == addr)
    }
    fn toggle_breakpoint(&mut self, addr: u64) -> bool {
        let max_bp = self
            .instructions
            .iter()
            .map(|i| i.bp_number)
            .max()
            .unwrap_or(0);
        if let Some(instr) = self.instructions.iter_mut().find(|i| i.address == addr) {
            instr.breakpoint = !instr.breakpoint;
            if instr.breakpoint {
                instr.bp_number = max_bp + 1;
            } else {
                instr.bp_number = 0;
            }
            return instr.breakpoint;
        }
        false
    }

    fn set_comment(&mut self, addr: u64, text: &str) -> bool {
        // Find by address — `Vec` scan is fine for typical-size
        // disassemblies (a few thousand instructions). For
        // GB-scale buffers the user should provide their own
        // address-indexed provider.
        if let Some(instr) = self.instructions.iter_mut().find(|i| i.address == addr) {
            // Empty input clears the comment; non-empty stores
            // a trimmed copy. Trimming guards against accidental
            // trailing whitespace from clipboard pastes.
            let trimmed = text.trim();
            instr.comment = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
            return true;
        }
        false
    }
}

// ── Branch Arrow ────────────────────────────────────────────────────────────

/// Visual branch arrow connecting two visible instructions.
///
/// Indices (`from_idx` / `to_idx`) are **visible-local** (0-based
/// within the current render window): 0 = first visible row, the
/// renderer multiplies by line-height to position the arrow.
///
/// `clipped_from` / `clipped_to` are raised when the corresponding
/// endpoint is *outside* the visible window — index is then clamped
/// to `0` (above) or `visible_count - 1` (below) so the arrow draws
/// at the window edge. The renderer suppresses the arrowhead at any
/// clipped end so the arrow visually reads as "continues offscreen"
/// rather than "lands here". See [`compute_arrows_clipped`] for the
/// scanner that produces clipped arrows.
#[derive(Debug, Clone)]
pub struct BranchArrow {
    /// Source instruction index (visible-local, clamped when clipped).
    pub from_idx: usize,
    /// Target instruction index (visible-local, clamped when clipped).
    pub to_idx: usize,
    /// Nesting depth (0 = closest to text, higher = further left).
    pub depth: usize,
    /// Flow kind of the branch (for coloring).
    pub flow_kind: FlowKind,
    /// Source is outside the visible window — arrowhead at the
    /// `from_idx` end MUST be suppressed by the renderer.
    pub clipped_from: bool,
    /// Target is outside the visible window — arrowhead at the
    /// `to_idx` end MUST be suppressed by the renderer.
    pub clipped_to: bool,
}

/// Maximum nesting depth for branch arrows.
pub const MAX_ARROW_DEPTH: usize = 6;

/// Compute branch arrows for a set of visible instructions.
///
/// **Visible-only**: only arrows whose source AND target both fall
/// inside the `instructions` slice are produced. Arrows leaving or
/// entering the visible range are silently dropped. Use
/// [`compute_arrows_clipped`] when arrow continuity past the
/// visible-window edge matters (the renderer pipeline does).
///
/// Returns arrows sorted by span size (smallest first, drawn closest to text).
pub fn compute_arrows(
    instructions: &[&dyn Instruction],
    visible_start_idx: usize,
    visible_count: usize,
) -> Vec<BranchArrow> {
    let mut arrows = Vec::new();
    let end_idx = visible_start_idx + visible_count;

    for (vis_i, instr) in instructions.iter().enumerate() {
        if let Some(target) = instr.branch_target() {
            // Find target in visible range.
            for (vis_j, other) in instructions.iter().enumerate() {
                let global_j = visible_start_idx + vis_j;
                if other.address() == target && global_j < end_idx {
                    arrows.push(BranchArrow {
                        from_idx: vis_i,
                        to_idx: vis_j,
                        depth: 0, // assigned below
                        flow_kind: instr.flow_kind(),
                        clipped_from: false,
                        clipped_to: false,
                    });
                    break;
                }
            }
        }
    }

    assign_arrow_depths(&mut arrows);
    arrows
}

/// Cross-window arrow scanner — produces arrows that intersect the
/// visible range `[visible_start, visible_start + visible_count)` in
/// any of three ways:
///
/// - **Both endpoints visible** (anchored arrow, `clipped_* == false`).
/// - **One endpoint visible**, the other above or below the window
///   (clamped to `0` / `visible_count - 1` with the corresponding
///   `clipped_*` flag raised).
/// - **Pass-through**: source above + target below (or vice versa).
///   Both `clipped_*` flags raised; renderer paints a vertical line
///   across the entire window with no arrowhead and no horizontal
///   stubs — visually reads as "long jump traversing this view".
///
/// Arrows where BOTH endpoints sit on the same side of the window
/// (both above OR both below) are silently dropped — they don't
/// cross the visible region, so there's nothing to render.
///
/// Returned arrows are sorted **by priority then span** so that
/// caller-side `truncate(N)` (driven by [`DisasmViewConfig::max_arrows`])
/// drops the *least informative* arrows first:
///
/// 1. Anchored (both endpoints visible) — preserved first
/// 2. Single-clipped (one endpoint visible)
/// 3. Pass-through (both endpoints clipped) — first to drop on overflow
///
/// Within each priority tier, smaller spans come first so depth
/// assignment puts them at depth 0 (closest to text).
///
/// **Cost**: linear scan of all provider instructions per frame —
/// `O(N)` for `N` total instructions, plus the cost of each
/// `provider.index_of_address(target)` call (typically `O(log N)`
/// or `O(1)` in well-built providers; `VecDisasmProvider`'s
/// linear-scan impl is `O(N)` so the effective total is `O(N²)` —
/// fine for typical few-thousand-instruction disassemblies, switch
/// to an indexed provider for 100k+).
pub fn compute_arrows_clipped(
    provider: &dyn DisasmDataProvider,
    visible_start_idx: usize,
    visible_count: usize,
) -> Vec<BranchArrow> {
    if visible_count == 0 {
        return Vec::new();
    }
    let total = provider.instruction_count();
    if total == 0 {
        return Vec::new();
    }
    let visible_end = (visible_start_idx + visible_count).min(total);
    let last_local = visible_count.saturating_sub(1);

    let mut arrows = Vec::new();

    for src in 0..total {
        let Some(instr) = provider.instruction(src) else {
            continue;
        };
        let Some(target_addr) = instr.branch_target() else {
            continue;
        };
        let Some(dst) = provider.index_of_address(target_addr) else {
            continue;
        };

        let src_above = src < visible_start_idx;
        let src_below = src >= visible_end;
        let dst_above = dst < visible_start_idx;
        let dst_below = dst >= visible_end;
        let src_in = !src_above && !src_below;
        let dst_in = !dst_above && !dst_below;

        // Drop arrows that don't cross the window — both above or
        // both below. Pass-through (one above + one below) is
        // explicitly KEPT so long jumps remain visible while
        // scrolling through their middle.
        let both_above = src_above && dst_above;
        let both_below = src_below && dst_below;
        if both_above || both_below {
            continue;
        }

        // Map global → visible-local with clamping.
        let from_idx = if src_in {
            src - visible_start_idx
        } else if src_above {
            0
        } else {
            last_local
        };
        let to_idx = if dst_in {
            dst - visible_start_idx
        } else if dst_above {
            0
        } else {
            last_local
        };

        arrows.push(BranchArrow {
            from_idx,
            to_idx,
            depth: 0, // assigned below
            flow_kind: instr.flow_kind(),
            clipped_from: !src_in,
            clipped_to: !dst_in,
        });
    }

    // Sort by (priority, span) — anchored first, single-clipped
    // next, pass-through last. Within each tier, smaller spans come
    // first so depth assignment gives them depth 0 (visually
    // closest to the text column). The renderer's `truncate` then
    // chops the *tail* of this list, dropping low-priority arrows
    // before high-priority ones.
    arrows.sort_by_key(|a| {
        let priority: u8 = match (a.clipped_from, a.clipped_to) {
            (false, false) => 0, // anchored
            (false, true) | (true, false) => 1, // half-clipped
            (true, true) => 2, // pass-through
        };
        let lo = a.from_idx.min(a.to_idx);
        let hi = a.from_idx.max(a.to_idx);
        (priority, hi - lo)
    });

    // In-order depth assignment — must NOT re-sort, otherwise the
    // priority order above would be lost and pass-through arrows
    // (which are big-span by definition) would migrate to depth 0,
    // crowding out anchored small arrows.
    assign_arrow_depths_in_order(&mut arrows);
    arrows
}

/// Sort arrows by span (smallest first → drawn closest to text)
/// and pack them into [`MAX_ARROW_DEPTH`] horizontal lanes so
/// overlapping arrows nest visually instead of stacking on top
/// of each other.
///
/// Used by the legacy [`compute_arrows`] path — that scanner
/// has no priority concept (everything is anchored), so plain
/// span-ascending sort is correct. [`compute_arrows_clipped`]
/// uses [`assign_arrow_depths_in_order`] instead so its
/// priority ordering survives.
fn assign_arrow_depths(arrows: &mut [BranchArrow]) {
    arrows.sort_by_key(|a| {
        let lo = a.from_idx.min(a.to_idx);
        let hi = a.from_idx.max(a.to_idx);
        hi - lo
    });
    assign_arrow_depths_in_order(arrows);
}

/// Pack arrows into [`MAX_ARROW_DEPTH`] horizontal lanes WITHOUT
/// re-sorting — assumes the caller has already arranged the slice
/// in render-priority order (smaller / more important arrows
/// first). Each arrow gets the lowest depth slot whose existing
/// entries don't overlap its `[lo, hi]` span; arrows that
/// would exceed `MAX_ARROW_DEPTH - 1` get clamped to the
/// outermost lane.
fn assign_arrow_depths_in_order(arrows: &mut [BranchArrow]) {
    let mut depth_slots: Vec<Vec<(usize, usize)>> = vec![Vec::new(); MAX_ARROW_DEPTH];
    for arrow in arrows {
        let lo = arrow.from_idx.min(arrow.to_idx);
        let hi = arrow.from_idx.max(arrow.to_idx);
        let mut found_depth = 0;
        'depth: for (d, slot) in depth_slots.iter().enumerate().take(MAX_ARROW_DEPTH) {
            for &(slo, shi) in slot {
                if lo < shi && hi > slo {
                    found_depth = d + 1;
                    continue 'depth;
                }
            }
            found_depth = d;
            break;
        }
        let depth = found_depth.min(MAX_ARROW_DEPTH - 1);
        arrow.depth = depth;
        depth_slots[depth].push((lo, hi));
    }
}

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
#[derive(Debug, Clone)]
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
    fn default() -> Self {
        Self {
            margin: 14.0,
            arrows: 36.0,
            address: 130.0,
            bytes: 200.0,
            mnemonic: 80.0,
            operands: 220.0,
            comment: 120.0,
        }
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

// ── Disasm View Config ──────────────────────────────────────────────────────

/// Configuration for the disassembly view widget.
#[derive(Debug, Clone)]
pub struct DisasmViewConfig {
    // ── Layout ──────────────────────────────────────────────
    /// Column widths.
    pub columns: ColumnWidths,
    /// Show raw hex bytes column.
    pub show_bytes: bool,
    /// Show comment column.
    pub show_comments: bool,
    /// Show branch arrows.
    pub show_arrows: bool,
    /// Show breakpoint markers in margin.
    pub show_breakpoints: bool,
    /// Show alternating per-block background tinting (semantic hues
    /// at low alpha, rotated by `block_index`). Default **`false`**
    /// — the tint reads as visual noise during normal browsing /
    /// editing; turn it on only when a block-boundary cue is
    /// explicitly desired (e.g. CFG-aware reverse-engineering view).
    pub show_block_tints: bool,
    /// Show column header.
    pub show_header: bool,
    /// Show thin vertical dividers between the address / bytes /
    /// instruction / comment columns. Default: `true`. Mirrors
    /// `HexViewerConfig::show_column_dividers` — uses
    /// `colors.separator` with alpha 0.40 so the lines read as a
    /// gentle visual cue, not heavy borders.
    pub show_column_dividers: bool,
    /// Address format: true for uppercase hex.
    pub uppercase: bool,
    /// Address width: 32-bit (8 chars) or 64-bit (16 chars).
    pub address_width_64: bool,
    /// Per-byte category colouring in the Bytes column. When `true`
    /// (default), each byte is tinted by `ByteCategory` (zero /
    /// control / printable / high / `0xFF`) — same 5-tier scheme
    /// `hex_viewer` uses, so the same buffer reads identically
    /// across both widgets. When `false`, every byte uses the flat
    /// [`DisasmColors::bytes`] colour.
    pub byte_category_colors: bool,

    // ── Behavior ────────────────────────────────────────────
    /// Allow inline instruction editing.
    pub editable: bool,
    /// Auto-scroll to follow current execution point.
    pub follow_execution: bool,
    /// Base address offset (for relative display).
    pub base_address: u64,
    /// Maximum visible arrows per render (for performance).
    pub max_arrows: usize,

    // ── Colors ──────────────────────────────────────────────
    /// Full color theme.
    pub colors: DisasmColors,
}

impl Default for DisasmViewConfig {
    fn default() -> Self {
        Self {
            columns: ColumnWidths::default(),
            show_bytes: true,
            show_comments: true,
            show_arrows: true,
            show_breakpoints: true,
            show_block_tints: false,
            show_header: true,
            show_column_dividers: true,
            uppercase: true,
            address_width_64: true,
            byte_category_colors: true,

            editable: false,
            follow_execution: false,
            base_address: 0,
            // 256 chosen so heavily-jumped functions don't hit the
            // cap during normal browsing — bumped from 64 on
            // 2026-04-30 after the user reported arrows
            // disappearing while scrolling. Cost per arrow is ~5
            // draw_list ops (3 lines + maybe arrowhead), so 256
            // ≈ 1.3k ops ≪ frame budget. Increase further only
            // if visual clutter at depth 5 (clamped lane)
            // becomes a problem.
            max_arrows: 256,

            colors: DisasmColors::default(),
        }
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
