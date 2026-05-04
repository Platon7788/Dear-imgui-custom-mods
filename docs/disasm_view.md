# DisasmView

Standalone disassembly viewer widget for Dear ImGui with branch arrows, breakpoints, block tinting, syntax coloring, inline editing, navigation history, and pluggable decoder backends.

## Overview

`DisasmView` provides a professional-grade disassembly view for code analysis and debugging UIs. It supports any x86/x64 instruction decoder through the `DisasmDataProvider` trait.

## Features

- **5-column layout**: margin │ arrows │ address │ hex bytes │ instruction (mnemonic + operands) │ comment, with optional vertical column dividers
- **Branch arrows** with 6-level nesting, collision avoidance, flow-kind coloring, and **off-window clipping** so long jumps stay visible (`compute_arrows_clipped`, default cap 256)
- **Breakpoint markers** — coloured circles in left gutter (F9 to toggle) with 8-slot numbered palette
- **Per-byte category tint** in the Bytes column — same 5-tier `ByteCategory` split (`hex_viewer` parity)
- **Byte search** — wildcard hex pattern (e.g. `4D 5A ?? 00 89`) via `Ctrl+F`, step with `F3` / `Shift+F3`, cross-instruction matches supported
- **Function navigation** — `Ctrl+Up` / `Ctrl+Down` (start/end) + `Ctrl+L` (select fn body), RET-based heuristic
- **Follow-at-cursor** — `Enter` / `Space` / dblclick on Instruction. Tries `branch_target()` first, then scans operand `Number` tokens; lazy `decode_range` for streaming providers
- **Address-column dblclick copy** — Hand cursor + tooltip + flash-pill animation
- **Origin breadcrumb** — soft "you came from here" highlight on the previous cursor row after Goto / Follow / function-jump / nav-back / search; survives scroll/click
- **8 FlowKind types** — Normal, Jump, Call, Return, Nop, Stack, System, Invalid
- **Syntax coloring**: mnemonics by instruction type, operands by token type
- **Operand highlighting**: registers (cyan), numbers (green), memory brackets (orange), strings (warm yellow)
- **Full x86 register set**: 64/32/16/8-bit GP, SSE/AVX (xmm/ymm), x87 (st0-st7), segment registers
- **Selection** with `Shift+Arrow` extend, `Ctrl+C` copy (address + mnemonic + operands, multi-line for multi-select)
- **Themed context menu** — Copy Address, Copy Instruction (count-aware), Follow Branch, Toggle Breakpoint, Toggle Watchpoint, Add/Remove Bookmark, Goto Address. Each entry is colour-coded by action class (navigation = address blue, follow = call green, function nav = jump amber, breakpoint = red, watchpoint = orange, bookmark = accent).
- **Themed Goto + Settings popups** — `igSetNextWindowPos` centred on the viewer, action-row layout helpers from `utils::popup`
- **Inline editing** — dblclick to patch Bytes / Comment (assembler integration via `DisasmDataProvider::assemble` / `set_comment`)
- **Current execution highlight** — warm-amber background for the row marked `is_current()`
- **Auto-scroll** — `follow_execution` flag for live-debugging hosts
- **Virtualized rendering** — only visible rows drawn (handles 100K+ instructions)
- **DisasmDataProvider trait** — pluggable backend for any decoder (iced-x86, capstone, zydis…)
- **32-bit and 64-bit address formats** — `address_width_64` flag
- **Theme integration** — `DisasmViewConfig::with_theme(Theme)`, palette via `Theme::disasm_view_colors()`

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

### Selection

| Method | Description |
|--------|-------------|
| `selected_index() -> Option<usize>` | Cursor / single-select index |
| `selected_indices() -> &BTreeSet<usize>` | Multi-select set (Shift / Ctrl) |
| `selected_count() -> usize` | Selection size |
| `is_selected(idx) -> bool` | Whether `idx` is selected |
| `select(idx)` | Single-select + set cursor + auto-scroll |
| `clear_selection()` | Drop selection (cursor stays) |

### Navigation

| Method | Description |
|--------|-------------|
| `goto_address(addr, provider)` | Jump to address (records nav history + breadcrumb) |
| `nav_back(provider)` | Navigate back in address history (`Alt+Left`) |
| `nav_forward(provider)` | Navigate forward (`Alt+Right`) |
| `can_nav_back() -> bool` | Back stack non-empty (host-toolbar disabled-state) |
| `can_nav_forward() -> bool` | Forward stack non-empty |
| `cursor_address(provider) -> Option<u64>` | Address under cursor — for status bar / Goto pre-fill |
| `jump_to_function_start(provider)` | Walk back to function entry (`Ctrl+Up`) |
| `jump_to_function_end(provider)` | Walk forward to function `ret` (`Ctrl+Down`) |
| `select_function(provider)` | Select cursor-row → function-end inclusive (`Ctrl+L`) |
| `follow_at_cursor(provider) -> bool` | Follow `branch_target()` (call/jmp/jcc), or scan operand for resolvable address. Lazy-decodes through `provider.decode_range`. Returns `false` when nothing followable — host can fall through to a different action. (`Enter` / `Space` / dblclick on Instruction column.) |

### Bookmarks (UI navigation aid, view-state)

Up to **64 addresses** can be bookmarked for quick navigation. Bookmarks
are pure view-state — they're an editor-style aid, not tied to
runtime-debugger concepts like breakpoints. Hosts that want
cross-session persistence read the set on shutdown and replay it at
startup.

| Method | Description |
|--------|-------------|
| `is_bookmarked(addr) -> bool` | Whether `addr` is currently bookmarked |
| `add_bookmark(addr) -> bool` | Insert; idempotent. Returns `false` only when the 64-cap is hit *and* `addr` wasn't already in the set. |
| `remove_bookmark(addr) -> bool` | Returns `true` if an entry was removed |
| `toggle_bookmark(addr) -> bool` | Flip state; returns the **new** state |
| `bookmarks() -> &BTreeSet<u64>` | Read-only access — sorted ascending, suitable for save/export |
| `bookmark_count() -> usize` | `<=` `MAX_BOOKMARKS` |
| `clear_bookmarks()` | Drop every bookmark |
| `MAX_BOOKMARKS` (assoc const) | `64` |

Visual: bookmark rows render an outline ring in the breakpoint gutter
(`colors.bookmark`, default `theme.accent()` family). Coexists with
breakpoint dot — both can be drawn on the same row. Toggle via
right-click context menu (label flips to "Add" / "Remove" depending on
state) or `Ctrl+B` on the cursor row. Disable rendering globally with
`config.show_bookmarks = false`.

```rust
// Host-side persistence example:
let saved: Vec<u64> = view.bookmarks().iter().copied().collect();
// … write `saved` to config file …

// On startup:
for addr in saved_from_config {
    view.add_bookmark(addr);
}
```

### Host-toolbar convenience helpers

These let a host implement a Top / Bottom / Current IP / Breakpoint
toolbar in pure `if button { view.method() }` style — no manual scan
loop over the provider. Pure view-domain operations; do **not** cross
into the host's debugger backend (stepping, run/pause, register/memory
reads stay on the backend side; the view only reflects whatever
provider state `is_current()` / `has_breakpoint()` reports).

| Method | Description |
|--------|-------------|
| `select_current_ip(provider) -> bool` | Find + select the row marked `is_current()` (debugger IP). Returns `false` when there's no current IP — host can fade the button. |
| `select_first_breakpoint(provider) -> bool` | Select lowest-index BP. |
| `select_next_breakpoint(provider) -> bool` | Cycle forward (wraps). |
| `select_prev_breakpoint(provider) -> bool` | Cycle backward (wraps). |

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
| `Up` / `Down` | Move cursor |
| `Shift+Arrow` | Extend selection (anchor stays put) |
| `Page Up` / `Page Down` | Jump by visible line count |
| `Home` / `End` | First / last instruction |
| `Enter` / `Space` | Follow branch target / resolvable operand at cursor |
| `G` | Open Goto-address popup |
| `Ctrl+F` | Open byte-search popup (wildcard hex pattern, e.g. `4D 5A ?? 00 89`) |
| `F3` / `Shift+F3` | Step to next / previous byte-search match |
| `F9` | Toggle breakpoint at cursor |
| `Ctrl+B` | Toggle bookmark at cursor (up to 64 bookmarks) |
| `Ctrl+Up` / `Ctrl+Down` | Jump to function start / end |
| `Ctrl+L` | Select cursor → function end |
| `Ctrl+C` | Copy selected instruction(s) |
| `Alt+Left` / `Alt+Right` | Navigate address history back / forward |
| `Esc` | Clear origin breadcrumb / cancel inline edit |
| Double-click on Address | Copy address (flash-pill animation) |
| Double-click on Instruction | Follow branch (mirrors `Enter`) |
| Double-click on Bytes / Comment | Enter inline edit (when `editable = true`) |
| Right-click | Themed context menu |

## Traits

### Instruction Trait

Required methods: `address`, `bytes`, `mnemonic`, `operands`. All others have default implementations.

```rust
pub trait Instruction {
    fn address(&self) -> u64;
    fn bytes(&self) -> &[u8];
    fn mnemonic(&self) -> &str;
    fn operands(&self) -> &str;
    // Optional (defaults shown):
    fn comment(&self) -> Option<&str>  { None }
    fn flow_kind(&self) -> FlowKind    { FlowKind::Normal }
    fn branch_target(&self) -> Option<u64> { None }
    fn block_index(&self) -> usize     { 0 }
    fn has_breakpoint(&self) -> bool   { false }
    fn breakpoint_number(&self) -> u32 { 0 }  // 1-based; 0 = no breakpoint
    fn is_current(&self) -> bool       { false }
    /// Whether a data watchpoint (read-or-write trap) is set. Renders
    /// as the `RW` glyph in the gutter. Hosts that distinguish
    /// read-only vs write-only data breakpoints handle that on the
    /// engine side and report the union back through this single
    /// flag.
    fn has_watchpoint(&self) -> bool   { false }
}
```

### DisasmDataProvider Trait

The trait is the boundary between the view and the host's decoder /
debugger backend. View knows nothing about ptrace / Win32 Debug API /
lldb-server — it only consults instruction state through these methods
and lets the host's impl propagate mutations downstream (e.g. a host
provider can override `toggle_breakpoint` to also call
`backend.add_breakpoint(addr)`).

```rust
pub trait DisasmDataProvider {
    // Required:
    fn instruction_count(&self) -> usize;
    fn instruction(&self, idx: usize) -> Option<&dyn Instruction>;

    /// Streaming hook: decode a window starting at `start_addr`. Called
    /// by `follow_at_cursor` when the target isn't yet decoded. The
    /// built-in `VecDisasmProvider` is a no-op (assumes pre-loaded data).
    fn decode_range(&mut self, start_addr: u64, max_count: usize);

    /// Address → index lookup. Default impl scans linearly; override
    /// with a sorted-binary-search or hashmap for large providers.
    fn index_of_address(&self, addr: u64) -> Option<usize> { /* default */ unimplemented!() }

    // Optional (default-no-op so old impls keep working):
    fn toggle_breakpoint(&mut self, _addr: u64) -> bool { false }
    /// Toggle the data watchpoint at `addr`. Single method (not
    /// separate read / write) — host engine sorts read-only vs
    /// write-only on its side and reports the union back through
    /// `Instruction::has_watchpoint()`.
    fn toggle_watchpoint(&mut self, _addr: u64) -> bool { false }
    fn assemble(&self, _addr: u64, _text: &str) -> Option<Vec<u8>> { None }
    fn write_bytes(&mut self, _addr: u64, _bytes: &[u8]) -> bool { false }
    fn set_comment(&mut self, _addr: u64, _text: &str) -> bool { false }
    fn symbol_name(&self, _addr: u64) -> Option<String> { None }
}
```

**Note**: `refresh()` was removed in 0.10.0 — orphan trait method with
no in-tree callers. Hosts implementing `refresh` should drop the
override.

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
cfg.show_bookmarks = true;           // outline-ring marker in the gutter
cfg.show_block_tints = false;        // disabled by default — use theme tints sparingly
cfg.show_column_dividers = true;     // vertical lines between Address / Bytes / Instruction / Comment
cfg.show_header = true;
cfg.uppercase = true;
cfg.address_width_64 = true;         // 16-char addresses (vs 8)
cfg.byte_category_colors = true;     // per-byte tint in Bytes column (mirrors hex_viewer)

// Behavior
cfg.editable = false;
cfg.follow_execution = false;        // auto-scroll to current
cfg.base_address = 0;
cfg.max_arrows = 256;                // max arrows per frame (heavy fns no longer hit cap)
```

### Theme integration

`DisasmViewConfig` plugs into the crate-wide `Theme` system — every
built-in theme exposes a fully-themed `DisasmViewColors` palette:

```rust
use dear_imgui_custom_mod::theme::Theme;

// One-shot apply at construction:
let cfg = DisasmViewConfig::default().with_theme(Theme::Nord);

// Or update an existing config when the host swaps themes:
view.config.apply_theme_colors(&Theme::Catppuccin.disasm_view_colors());
```

Available accessor on `Theme`: `Theme::disasm_view_colors() ->
DisasmViewColors`. Built-in themes: `Dark`, `Light`, `Midnight`,
`Solarized`, `Monokai`, `Catppuccin`, `Nord`.

### Column Widths

```rust
pub struct ColumnWidths {
    pub margin: f32,     // 14.0  — breakpoint gutter
    pub arrows: f32,     // 36.0  — branch arrow area
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
| `breakpoint` | Default breakpoint circle color (bright red) |
| `breakpoint_colors` | `Vec<[f32; 4]>` — per-number breakpoint colors (index by `breakpoint_number - 1`) |
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

96+ unit tests covering:
- InstructionEntry builder pattern
- VecDisasmProvider (count, lookup, index_of_address)
- Breakpoint toggle
- FlowKind color mapping
- Arrow color mapping (default, per-flow, branch-arrow clipping for off-window endpoints, x32/x64 PE32 addresses)
- Block tint wrapping
- Branch arrow computation, depth assignment (non-overlapping), priority sort
- Operand tokenizer (registers, numbers, memory, strings, hex suffixes)
- Token classification (register names, hex/dec numbers, size keywords)
- Column width defaults + dynamic comment-column reflow
- Config defaults + theme-derived palette equivalence
- Select / goto_address / nav back/forward + can_nav_back/forward
- `do_search` sparse-provider correctness (`partition_point` over `(byte_offset, global_idx)` pairs)
- `find_function_start` / `_end` (no off-by-one on last instruction)
- `follow_at_cursor` — branch-target priority + operand-pointer fallback + lazy decode for streaming providers
- Per-byte category coloring round-trip vs hex_viewer
- Comment editing (round-trip, trim-on-write, clear-on-empty, default-impl no-op)
- Convenience helpers: `select_current_ip`, `select_first_breakpoint`, `select_next/prev_breakpoint` (cycle + wraparound), `cursor_address` matches selection

## Configuration & localisation

`DisasmViewConfig` follows the project-wide DDD config pattern: schema
in `src/disasm_view/config.rs`, default values in
`src/disasm_view/config.ron` plus `column_widths.ron` for the column
geometry sub-struct. See [`docs/config_pattern.md`](./config_pattern.md).

The goto/search popups, context menu, settings popup, instruction
hover tooltip, and copy-address tooltip are all localised through
`crate::i18n::disasm_view`. Switch with `DisasmView::new(...).with_locale(Locale::Ru)`.
The instruction hover tooltip honours both `cfg.address_width_64` and
`cfg.uppercase` for the address line, the 32-bit shadow, branch
targets, and the bytes block. See [`docs/i18n.md`](./i18n.md).
