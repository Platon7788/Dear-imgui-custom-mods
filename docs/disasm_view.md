# DisasmView

Standalone disassembly viewer widget for Dear ImGui with branch arrows, breakpoints, block tinting, syntax coloring, inline editing, navigation history, and pluggable decoder backends.

## Overview

`DisasmView` provides a professional-grade disassembly view for code analysis and debugging UIs. It supports any x86/x64 instruction decoder through the `DisasmDataProvider` trait.

## Features

- **5-column layout**: margin | arrows | address | hex bytes | mnemonic operands ; comment
- **Branch arrows** with 6-level nesting, collision avoidance, and flow-kind coloring (jump/call/return)
- **Breakpoint markers** — red circles in left gutter (F9 to toggle)
- **Block tinting** — 6 alternating background colors for logical code blocks
- **8 FlowKind types** — Normal, Jump, Call, Return, Nop, Stack, System, Invalid
- **Syntax coloring**: mnemonics by instruction type, operands by token type
- **Operand highlighting**: registers (cyan), numbers (green), memory brackets (orange), strings (warm yellow)
- **Full x86 register set**: 64/32/16/8-bit GP, SSE/AVX (xmm/ymm), x87 (st0-st7), segment registers
- **Keyboard navigation** — arrows, PgUp/Dn, Home/End, Enter → follow branch
- **Navigation history** — Alt+Left/Right (back/forward address stack)
- **Selection** with Ctrl+C copy (address + mnemonic + operands)
- **Context menu** — Copy Address, Copy Instruction, Follow Branch, Toggle Breakpoint, Goto Address
- **Inline editing** — double-click bytes to patch (hex input, assembler integration)
- **Current execution highlight** — yellow background for stopped-at instruction
- **Auto-scroll** — follow execution point option
- **Virtualized rendering** — only visible rows drawn (handles 100K+ instructions)
- **DisasmDataProvider trait** — pluggable backend for any decoder (iced-x86, capstone, etc.)
- **Goto address popup** (G key)
- **Configurable column widths**
- **32-bit and 64-bit address formats**

## Quick Start

```rust
use dear_imgui_custom_mod::disasm_view::{
    DisasmView, InstructionEntry, VecDisasmProvider, FlowKind,
};

let mut provider = VecDisasmProvider::new();
provider.push(
    InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
        .with_flow(FlowKind::Stack)
);
provider.push(
    InstructionEntry::new(0x401001, vec![0x48, 0x89, 0xE5], "mov", "rbp, rsp")
);
provider.push(
    InstructionEntry::new(0x401004, vec![0xE8, 0x10, 0x00, 0x00, 0x00], "call", "0x401019")
        .with_flow(FlowKind::Call)
        .with_target(0x401019)
        .with_comment("my_function")
);

let mut view = DisasmView::new("##disasm");

// In render loop:
view.render(ui, &mut provider);
```

### Custom Data Provider (iced-x86 example)

```rust
use dear_imgui_custom_mod::disasm_view::{DisasmDataProvider, Instruction};

struct IcedDecoder {
    instructions: Vec<MyInstruction>,
    // ... iced-x86 decoder state
}

impl DisasmDataProvider for IcedDecoder {
    fn instruction_count(&self) -> usize { self.instructions.len() }
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction> {
        self.instructions.get(idx).map(|i| i as &dyn Instruction)
    }
    fn decode_range(&mut self, start_addr: u64, max_count: usize) {
        // Decode using iced_x86::Decoder
    }
    fn index_of_address(&self, addr: u64) -> Option<usize> {
        self.instructions.iter().position(|i| i.address == addr)
    }
    fn toggle_breakpoint(&mut self, addr: u64) -> bool { /* ... */ false }
    fn assemble(&self, addr: u64, text: &str) -> Option<Vec<u8>> { /* ... */ None }
    fn write_bytes(&mut self, addr: u64, bytes: &[u8]) -> bool { /* ... */ false }
    fn symbol_name(&self, addr: u64) -> Option<String> { /* ... */ None }
}
```

## Public API

### Construction

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new disassembly view |

### Navigation

| Method | Description |
|--------|-------------|
| `selected_index() -> Option<usize>` | Currently selected instruction index |
| `select(idx)` | Set selected instruction (auto-scrolls) |
| `goto_address(addr, provider)` | Jump to address (records history) |
| `nav_back(provider)` | Navigate back in history (Alt+Left) |
| `nav_forward(provider)` | Navigate forward in history (Alt+Right) |

### State

| Method | Description |
|--------|-------------|
| `is_focused() -> bool` | Whether the widget has focus |
| `config` | Public config field for runtime modification |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui, provider)` | Render the disassembly view |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Up/Down | Move selection |
| Page Up/Down | Jump by visible line count |
| Home/End | First/last instruction |
| Enter | Follow branch target |
| G | Open goto address popup |
| F9 | Toggle breakpoint |
| Ctrl+C | Copy selected instruction |
| Alt+Left | Navigate back |
| Alt+Right | Navigate forward |
| Double-click | Enter inline edit (when editable) |
| Escape | Cancel inline edit |
| Right-click | Context menu |

## Traits

### Instruction Trait

```rust
pub trait Instruction {
    fn address(&self) -> u64;
    fn bytes(&self) -> &[u8];
    fn mnemonic(&self) -> &str;
    fn operands(&self) -> &str;
    fn comment(&self) -> Option<&str>;
    fn flow_kind(&self) -> FlowKind;
    fn branch_target(&self) -> Option<u64>;
    fn block_index(&self) -> usize;
    fn has_breakpoint(&self) -> bool;
    fn is_current(&self) -> bool;
}
```

### DisasmDataProvider Trait

```rust
pub trait DisasmDataProvider {
    fn instruction_count(&self) -> usize;
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction>;
    fn decode_range(&mut self, start_addr: u64, max_count: usize);
    fn index_of_address(&self, addr: u64) -> Option<usize>;
    fn toggle_breakpoint(&mut self, addr: u64) -> bool;
    fn assemble(&self, addr: u64, text: &str) -> Option<Vec<u8>>;
    fn write_bytes(&mut self, addr: u64, bytes: &[u8]) -> bool;
    fn symbol_name(&self, addr: u64) -> Option<String>;
    fn refresh(&mut self);
}
```

### InstructionEntry (Builder Pattern)

```rust
let instr = InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
    .with_flow(FlowKind::Stack)
    .with_target(0x401010)
    .with_comment("function prologue")
    .with_block(0)
    .with_breakpoint(true)
    .with_current(false);
```

## Types

### FlowKind

```rust
pub enum FlowKind {
    Normal,   // mov, add, lea, etc.
    Jump,     // jmp, je, jne, etc.
    Call,     // call
    Return,   // ret, iret
    Nop,      // nop, int3
    Stack,    // push, pop, sub rsp
    System,   // syscall, sysenter, int
    Invalid,  // undecodable
}
```

### BranchArrow

```rust
pub struct BranchArrow {
    pub from_idx: usize,    // source row
    pub to_idx: usize,      // target row
    pub depth: usize,       // nesting level (0 = closest to text)
    pub flow_kind: FlowKind,
}
```

## Configuration

```rust
let cfg = &mut view.config;

// Layout
cfg.columns = ColumnWidths::default();
cfg.show_bytes = true;
cfg.show_comments = true;
cfg.show_arrows = true;
cfg.show_breakpoints = true;
cfg.show_block_tints = true;
cfg.show_header = true;
cfg.uppercase = true;
cfg.address_width_64 = true;    // 16-char addresses (vs 8)

// Behavior
cfg.editable = false;
cfg.follow_execution = false;   // auto-scroll to current
cfg.base_address = 0;
cfg.max_arrows = 64;            // max arrows per frame
```

### Column Widths

```rust
pub struct ColumnWidths {
    pub margin: f32,     // 20.0  — breakpoint gutter
    pub arrows: f32,     // 60.0  — branch arrow area
    pub address: f32,    // 130.0 — address column
    pub bytes: f32,      // 180.0 — hex bytes column
    pub mnemonic: f32,   // 70.0  — mnemonic column
    pub operands: f32,   // 200.0 — operands column
    pub comment: f32,    // 200.0 — comment column
}
```

### Color Theme (DisasmColors)

#### Mnemonic Colors

| Field | FlowKind | Default Color |
|-------|----------|---------------|
| `mnemonic_normal` | Normal | Near white |
| `mnemonic_jump` | Jump | Yellow |
| `mnemonic_call` | Call | Green |
| `mnemonic_return` | Return | Red |
| `mnemonic_nop` | Nop | Dim gray |
| `mnemonic_stack` | Stack | Purple |
| `mnemonic_system` | System | Orange |
| `mnemonic_invalid` | Invalid | Bright red |

#### Operand Colors

| Field | Token Type | Default Color |
|-------|-----------|---------------|
| `operand_register` | Register names | Cyan |
| `operand_number` | Immediates / constants | Light green |
| `operand_memory` | Brackets, `ptr`, size specifiers | Orange |
| `operand_string` | String literals | Warm yellow |
| `operand_default` | Other tokens | Light gray |

#### Arrow Colors

| Field | FlowKind | Default Color |
|-------|----------|---------------|
| `arrow_jump` | Jump | Yellow |
| `arrow_call` | Call | Green |
| `arrow_return` | Return | Red |
| `arrow_default` | Other | Gray |

#### Block Tints (6 alternating)

Blue, Red, Green, Amber, Purple, Teal — all with subtle alpha (10-12%).

#### UI Colors

| Field | Description |
|-------|-------------|
| `breakpoint` | Breakpoint circle color (bright red) |
| `breakpoint_bg` | Breakpoint gutter background |
| `current_line_bg` | Stopped-at instruction highlight (warm yellow) |
| `selection_bg` | Selected row background |
| `hover_bg` | Row hover highlight |
| `header` | Column header text |
| `separator` | Column separator lines |

## Built-in Providers

### VecDisasmProvider

Simple in-memory provider backed by `Vec<InstructionEntry>`:

```rust
let mut provider = VecDisasmProvider::new();
provider.push(InstructionEntry::new(...));
// or
let provider = VecDisasmProvider::from_vec(instructions);
```

Methods: `push()`, `clear()`, `instructions()`, `instructions_mut()`.

## Architecture

```
disasm_view/
  mod.rs      DisasmView widget, rendering (rows, arrows, margin, operand tokenizer),
              input handling (keyboard, mouse, edit), goto/context popups
  config.rs   DisasmViewConfig, DisasmColors, ColumnWidths, FlowKind,
              Instruction trait, DisasmDataProvider trait, InstructionEntry,
              VecDisasmProvider, BranchArrow, compute_arrows()
```

## Tests

27 unit tests covering:
- InstructionEntry builder pattern
- VecDisasmProvider (count, lookup, index_of_address)
- Breakpoint toggle
- FlowKind color mapping
- Arrow color mapping
- Block tint wrapping
- Branch arrow computation
- Arrow depth assignment (non-overlapping)
- Operand tokenizer (registers, numbers, memory, strings)
- Token classification (register names, hex/dec numbers, size keywords)
- Column width defaults
- Config defaults
- Select and goto_address
- Navigation history (back/forward)
- Address parsing
