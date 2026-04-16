//! Configuration types for [`DisasmView`](super::DisasmView).

// ── Flow Kind ───────────────────────────────────────────────────────────────

/// Instruction control-flow classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlowKind {
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
    fn comment(&self) -> Option<&str> { None }
    /// Control flow classification.
    fn flow_kind(&self) -> FlowKind { FlowKind::Normal }
    /// Branch/call target address (if applicable).
    fn branch_target(&self) -> Option<u64> { None }
    /// Logical block index for block-tinting (0-based).
    fn block_index(&self) -> usize { 0 }
    /// Whether a breakpoint is set at this address.
    fn has_breakpoint(&self) -> bool { false }
    /// Breakpoint number (1-based). Used for colored numbered markers.
    /// Returns 0 if no breakpoint.
    fn breakpoint_number(&self) -> u32 { if self.has_breakpoint() { 1 } else { 0 } }
    /// Whether this is the current execution point (stopped-at).
    fn is_current(&self) -> bool { false }
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
    fn toggle_breakpoint(&mut self, _addr: u64) -> bool { false }

    /// Assemble a text instruction into bytes at `addr`.
    /// Returns the assembled bytes or `None` on failure.
    fn assemble(&self, _addr: u64, _text: &str) -> Option<Vec<u8>> { None }

    /// Write bytes at address (for patching).
    fn write_bytes(&mut self, _addr: u64, _bytes: &[u8]) -> bool { false }

    /// Get a human-readable name for an address (symbol, export, label).
    fn symbol_name(&self, _addr: u64) -> Option<String> { None }

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
        self.flow_kind = kind; self
    }
    pub fn with_target(mut self, target: u64) -> Self {
        self.branch_target = Some(target); self
    }
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into()); self
    }
    pub fn with_block(mut self, index: usize) -> Self {
        self.block_index = index; self
    }
    pub fn with_breakpoint(mut self, bp: bool) -> Self {
        self.breakpoint = bp; self
    }
    pub fn with_bp_number(mut self, n: u32) -> Self {
        self.bp_number = n; self.breakpoint = n > 0; self
    }
    pub fn with_current(mut self, current: bool) -> Self {
        self.current = current; self
    }
}

impl Instruction for InstructionEntry {
    fn address(&self) -> u64 { self.address }
    fn bytes(&self) -> &[u8] { &self.bytes }
    fn mnemonic(&self) -> &str { &self.mnemonic }
    fn operands(&self) -> &str { &self.operands }
    fn comment(&self) -> Option<&str> { self.comment.as_deref() }
    fn flow_kind(&self) -> FlowKind { self.flow_kind }
    fn branch_target(&self) -> Option<u64> { self.branch_target }
    fn block_index(&self) -> usize { self.block_index }
    fn has_breakpoint(&self) -> bool { self.breakpoint }
    fn breakpoint_number(&self) -> u32 { self.bp_number }
    fn is_current(&self) -> bool { self.current }
}

// ── Vec Provider ────────────────────────────────────────────────────────────

/// Simple in-memory provider backed by `Vec<InstructionEntry>`.
pub struct VecDisasmProvider {
    instructions: Vec<InstructionEntry>,
}

impl VecDisasmProvider {
    pub fn new() -> Self { Self { instructions: Vec::new() } }
    pub fn from_vec(instructions: Vec<InstructionEntry>) -> Self {
        Self { instructions }
    }
    pub fn push(&mut self, instr: InstructionEntry) {
        self.instructions.push(instr);
    }
    pub fn clear(&mut self) { self.instructions.clear(); }
    pub fn instructions(&self) -> &[InstructionEntry] { &self.instructions }
    pub fn instructions_mut(&mut self) -> &mut Vec<InstructionEntry> { &mut self.instructions }
}

impl Default for VecDisasmProvider {
    fn default() -> Self { Self::new() }
}

impl DisasmDataProvider for VecDisasmProvider {
    fn instruction_count(&self) -> usize { self.instructions.len() }
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
        let max_bp = self.instructions.iter().map(|i| i.bp_number).max().unwrap_or(0);
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
        let _global_i = visible_start_idx + vis_i;
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
#[derive(Debug, Clone)]
pub struct DisasmColors {
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
    /// Hex bytes column color.
    pub bytes: [f32; 4],
    /// Comment color.
    pub comment: [f32; 4],

    // ── Branch arrows ───────────────────────────────────────
    /// Arrow colors by flow kind (jump, call, return, default).
    pub arrow_jump: [f32; 4],
    pub arrow_call: [f32; 4],
    pub arrow_return: [f32; 4],
    pub arrow_default: [f32; 4],

    // ── Block tinting ───────────────────────────────────────
    /// Background tint colors for alternating code blocks.
    pub block_tints: Vec<[f32; 4]>,

    // ── UI elements ─────────────────────────────────────────
    /// Breakpoint marker color (fallback).
    pub breakpoint: [f32; 4],
    /// Breakpoint gutter background.
    pub breakpoint_bg: [f32; 4],
    /// Numbered breakpoint colors (cycle through these).
    pub breakpoint_colors: Vec<[f32; 4]>,
    /// Current execution point (stopped-at) background.
    pub current_line_bg: [f32; 4],
    /// Selected row background.
    pub selection_bg: [f32; 4],
    /// Row hover highlight.
    pub hover_bg: [f32; 4],
    /// Column header / separator color.
    pub header: [f32; 4],
    /// Separator line between columns.
    pub separator: [f32; 4],
}

impl Default for DisasmColors {
    fn default() -> Self {
        Self {
            // Mnemonic colors (dark theme, high contrast)
            mnemonic_normal:  [0.88, 0.92, 0.97, 1.0],  // near white
            mnemonic_jump:    [0.95, 0.85, 0.35, 1.0],   // yellow
            mnemonic_call:    [0.45, 0.85, 0.45, 1.0],   // green
            mnemonic_return:  [0.90, 0.35, 0.35, 1.0],   // red
            mnemonic_nop:     [0.50, 0.50, 0.50, 0.60],  // dim gray
            mnemonic_stack:   [0.70, 0.55, 0.90, 1.0],   // purple
            mnemonic_system:  [1.00, 0.55, 0.30, 1.0],   // orange
            mnemonic_invalid: [1.00, 0.20, 0.20, 1.0],   // bright red

            // Operand colors
            operand_register: [0.45, 0.80, 0.90, 1.0],   // cyan
            operand_number:   [0.55, 0.85, 0.55, 1.0],   // light green
            operand_memory:   [0.90, 0.65, 0.30, 1.0],   // orange
            operand_string:   [0.85, 0.70, 0.50, 1.0],   // warm yellow
            operand_default:  [0.80, 0.82, 0.85, 1.0],   // light gray

            // Address / bytes
            address:          [0.45, 0.55, 0.70, 1.0],
            bytes:            [0.60, 0.62, 0.65, 0.80],
            comment:          [0.50, 0.65, 0.50, 0.85],

            // Branch arrows
            arrow_jump:       [0.95, 0.85, 0.35, 0.90],  // yellow
            arrow_call:       [0.45, 0.85, 0.45, 0.90],  // green
            arrow_return:     [0.90, 0.35, 0.35, 0.90],  // red
            arrow_default:    [0.60, 0.60, 0.70, 0.70],  // gray

            // Block tints (subtle backgrounds)
            block_tints: vec![
                [0.15, 0.18, 0.25, 0.12],   // blue tint
                [0.25, 0.15, 0.15, 0.10],   // red tint
                [0.15, 0.22, 0.15, 0.10],   // green tint
                [0.25, 0.22, 0.12, 0.10],   // amber tint
                [0.20, 0.15, 0.25, 0.10],   // purple tint
                [0.15, 0.22, 0.25, 0.10],   // teal tint
            ],

            // UI
            breakpoint:       [0.95, 0.25, 0.25, 1.0],   // bright red
            breakpoint_bg:    [0.25, 0.10, 0.10, 0.30],
            breakpoint_colors: vec![
                [0.95, 0.30, 0.30, 1.0],  // 1: red
                [0.30, 0.80, 0.95, 1.0],  // 2: cyan
                [0.95, 0.80, 0.25, 1.0],  // 3: yellow
                [0.50, 0.90, 0.40, 1.0],  // 4: green
                [0.85, 0.50, 0.95, 1.0],  // 5: purple
                [0.95, 0.60, 0.25, 1.0],  // 6: orange
                [0.40, 0.70, 0.95, 1.0],  // 7: blue
                [0.95, 0.50, 0.70, 1.0],  // 8: pink
            ],
            current_line_bg:  [0.40, 0.35, 0.15, 0.35],  // warm yellow tint
            selection_bg:     [0.20, 0.35, 0.55, 0.45],
            hover_bg:         [1.00, 1.00, 1.00, 0.04],
            header:           [0.50, 0.55, 0.60, 0.80],
            separator:        [0.30, 0.32, 0.35, 0.40],
        }
    }
}

impl DisasmColors {
    /// Get mnemonic color for a given flow kind.
    pub fn mnemonic_color(&self, kind: FlowKind) -> [f32; 4] {
        match kind {
            FlowKind::Normal  => self.mnemonic_normal,
            FlowKind::Jump    => self.mnemonic_jump,
            FlowKind::Call    => self.mnemonic_call,
            FlowKind::Return  => self.mnemonic_return,
            FlowKind::Nop     => self.mnemonic_nop,
            FlowKind::Stack   => self.mnemonic_stack,
            FlowKind::System  => self.mnemonic_system,
            FlowKind::Invalid => self.mnemonic_invalid,
        }
    }

    /// Get arrow color for a given flow kind.
    pub fn arrow_color(&self, kind: FlowKind) -> [f32; 4] {
        match kind {
            FlowKind::Jump   => self.arrow_jump,
            FlowKind::Call   => self.arrow_call,
            FlowKind::Return => self.arrow_return,
            _                => self.arrow_default,
        }
    }

    /// Get breakpoint color by number (1-based). Falls back to default breakpoint color.
    pub fn bp_color(&self, number: u32) -> [f32; 4] {
        if number == 0 || self.breakpoint_colors.is_empty() {
            return self.breakpoint;
        }
        self.breakpoint_colors[((number - 1) as usize) % self.breakpoint_colors.len()]
    }

    /// Get block tint color for a given block index.
    pub fn block_tint(&self, block_index: usize) -> [f32; 4] {
        if self.block_tints.is_empty() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        self.block_tints[block_index % self.block_tints.len()]
    }
}

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
