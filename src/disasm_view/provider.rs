//! Data providers and instruction types for disasm_view.

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
    /// Whether a **watchpoint** is set on this address — fires when
    /// the running process reads or writes the byte at the address.
    /// The viewer renders this as the `RW` glyph in the gutter and
    /// owns no notion of "read-only" vs "write-only" watchpoints —
    /// hosts that need finer-grained access (e.g. read-only data
    /// breakpoints) sort that out on the engine side and report
    /// the union back through this single flag. Distinct from
    /// execution breakpoints ([`Self::has_breakpoint`]).
    fn has_watchpoint(&self) -> bool {
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

    /// Toggle the **watchpoint** at `addr` — host-side this wires
    /// through to the debugger backend's data-breakpoint API so the
    /// running process traps on the next read **or** write of the
    /// byte at `addr`. The viewer keeps the surface area minimal
    /// (single `RW` glyph, single context-menu entry); hosts that
    /// distinguish read-only vs write-only watchpoints handle that
    /// on the engine side and report the union back through
    /// [`Instruction::has_watchpoint`]. Returns the new state
    /// (`true` = watchpoint now set). Default impl is a no-op
    /// returning `false` so existing providers stay non-breaking.
    fn toggle_watchpoint(&mut self, _addr: u64) -> bool {
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
///
/// Field-stability note: this struct is intentionally **not**
/// `#[non_exhaustive]` — the sibling `IMGUI_NXT` consumer constructs
/// it directly via record syntax (`InstructionEntry { ... }`) and
/// breaking that pattern would force every host to switch to a
/// builder. New fields are additive and must come with sane defaults
/// inside the crate's own constructors / builders. If you are
/// extending this struct, mirror the new field in `Instruction`'s
/// trait defaults so external `Instruction` impls keep working.
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
    /// Watchpoint set on this address — fires on read **or** write
    /// of the byte at `address`. Renders as the `RW` glyph in the
    /// gutter. Hosts that distinguish read-only vs write-only
    /// data breakpoints sort that out on the engine side and
    /// report the union back through this single flag.
    pub watchpoint: bool,
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
            watchpoint: false,
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
    fn has_watchpoint(&self) -> bool {
        self.watchpoint
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

    fn toggle_watchpoint(&mut self, addr: u64) -> bool {
        if let Some(instr) = self.instructions.iter_mut().find(|i| i.address == addr) {
            instr.watchpoint = !instr.watchpoint;
            return instr.watchpoint;
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
