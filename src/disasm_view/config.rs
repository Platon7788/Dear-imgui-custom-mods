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

    /// Called every frame when auto-refresh is enabled.
    fn refresh(&mut self) {}
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
}

// ── Branch Arrow ────────────────────────────────────────────────────────────

/// Visual branch arrow connecting two visible instructions.
#[derive(Debug, Clone)]
pub struct BranchArrow {
    /// Source instruction index (in visible range).
    pub from_idx: usize,
    /// Target instruction index (in visible range).
    pub to_idx: usize,
    /// Nesting depth (0 = closest to text, higher = further left).
    pub depth: usize,
    /// Flow kind of the branch (for coloring).
    pub flow_kind: FlowKind,
}

/// Maximum nesting depth for branch arrows.
pub const MAX_ARROW_DEPTH: usize = 6;

/// Compute branch arrows for a set of visible instructions.
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
                    });
                    break;
                }
            }
        }
    }

    // Sort by span size (smallest first = innermost).
    arrows.sort_by_key(|a| {
        let lo = a.from_idx.min(a.to_idx);
        let hi = a.from_idx.max(a.to_idx);
        hi - lo
    });

    // Assign depths to avoid overlaps.
    let mut depth_slots: Vec<Vec<(usize, usize)>> = vec![Vec::new(); MAX_ARROW_DEPTH];
    for arrow in &mut arrows {
        let lo = arrow.from_idx.min(arrow.to_idx);
        let hi = arrow.from_idx.max(arrow.to_idx);
        let mut found_depth = 0;
        'depth: for (d, slot) in depth_slots.iter().enumerate().take(MAX_ARROW_DEPTH) {
            for &(slo, shi) in slot {
                if lo < shi && hi > slo {
                    // Overlaps, try next depth.
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

    arrows
}

// ── Column Config ───────────────────────────────────────────────────────────

/// Column widths for the disassembly view.
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
    /// Mnemonic (opcode) column.
    pub mnemonic: f32,
    /// Operands column.
    pub operands: f32,
    /// Comment column (fills remaining).
    pub comment: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            margin: 14.0,
            arrows: 36.0,
            address: 130.0,
            bytes: 180.0,
            mnemonic: 70.0,
            operands: 200.0,
            comment: 200.0,
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
    /// Show block tinting.
    pub show_block_tints: bool,
    /// Show column header.
    pub show_header: bool,
    /// Address format: true for uppercase hex.
    pub uppercase: bool,
    /// Address width: 32-bit (8 chars) or 64-bit (16 chars).
    pub address_width_64: bool,

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
            show_block_tints: true,
            show_header: true,
            uppercase: true,
            address_width_64: true,

            editable: false,
            follow_execution: false,
            base_address: 0,
            max_arrows: 64,

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
