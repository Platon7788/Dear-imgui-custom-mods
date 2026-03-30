# Component Proposals — Dear ImGui Custom Mod

Analysis of missing components for: **Debugger**, **Package Manager**, **Rust IDE**.

## Current Component Map

```
┌─────────────────────────────────────────────────────────────────┐
│                    dear-imgui-custom-mod                        │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│ code_editor  │ file_manager │ virtual_table│ virtual_tree      │
│ (editing)    │ (file I/O)   │ (data grid)  │ (hierarchy)       │
├──────────────┼──────────────┼──────────────┼───────────────────┤
│ node_graph   │ page_control │ icons        │ theme / utils     │
│ (visual flow)│ (tabs/pages) │ (MDI 7.4)   │ (colors, helpers) │
└──────────────┴──────────────┴──────────────┴───────────────────┘
```

## Gap Analysis by Application

```
                  Debugger    Pkg Manager    IDE
                 ─────────   ───────────   ─────
Terminal/Console   ████████     ████████    ████████   ← #1 most needed
Property Inspect   ████████     ░░░░░░░░    ████████   ← #2
Diff Viewer        ████░░░░     ████████    ████████   ← #3
Hex Memory View    ████████     ░░░░░░░░    ████░░░░   ← #4
Timeline/Flame     ████████     ░░░░░░░░    ████░░░░   ← #5
Status Bar         ████████     ████████    ████████   ← #6 (small but universal)
Toolbar Builder    ████████     ████████    ████████   ← #7 (small but universal)

████ = critical    ░░░░ = not needed
```

---

## Proposal 1: `terminal` — Virtual Terminal Emulator

**Priority: HIGHEST** — needed in all three apps.

### What

Scrollable output console with ANSI color support, command input line,
history, auto-scroll, filtering, and selectable text. Think VS Code's
integrated terminal panel or GDB's output pane.

### Why It Doesn't Exist

ImGui has `InputTextMultiline` and `LogText`, but neither handles:
- ANSI escape sequences (colors, bold, underline)
- Ring buffer for bounded memory (100K+ lines)
- Mixed input/output (command prompt + scrollback)
- Click-to-select URLs, file paths
- Performance at high throughput (cargo build spams 1000s of lines/sec)

### Architecture

```
terminal/
├── mod.rs          Terminal widget — render, input, scrollback
├── config.rs       TerminalConfig, ANSI palette, prompt style
├── buffer.rs       RingBuffer<Line> with ANSI-parsed spans
├── ansi.rs         ANSI escape parser (SGR colors, bold, reset)
├── history.rs      Command history with search (Ctrl+R)
└── selection.rs    Text selection + copy across wrapped lines

┌──────────────────────────────────────────────────────┐
│ Terminal                                         ≡ ▼ │
├──────────────────────────────────────────────────────┤
│ $ cargo build                                        │
│   Compiling dear-imgui-sys v0.10.4                   │
│   Compiling dear-imgui-rs v0.10.4                    │
│ error[E0308]: mismatched types                       │  ← red via ANSI
│   --> src/main.rs:12:5                               │  ← clickable path
│   Finished dev [unoptimized] in 3.42s                │  ← green via ANSI
│                                                      │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │  ← scrollbar
├──────────────────────────────────────────────────────┤
│ $ █                                              ⏎  │  ← input line
└──────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| ANSI SGR colors | 16 base + 256 extended + 24-bit RGB |
| Ring buffer | Configurable max lines (default 50K), O(1) push |
| Command input | Single-line input with history (↑/↓, Ctrl+R search) |
| Auto-scroll | Lock-to-bottom, unlock on manual scroll up |
| Filtering | Regex/text filter on output lines |
| Text selection | Mouse drag → clipboard, double-click word |
| Clickable paths | Detect `file:line:col` patterns, emit callback |
| Read-only mode | Output-only (no input line) for log viewers |
| Multiple streams | stdout (white), stderr (red), system (yellow) |
| Timestamps | Optional `[HH:MM:SS.mmm]` prefix per line |

### Use Cases

```
Debugger:      GDB/LLDB command output, breakpoint hits, variable watch
Pkg Manager:   cargo install output, download progress, build logs
IDE:           Build output, test runner, integrated shell
```

### API Sketch

```rust
let mut term = Terminal::new("##console");

// Push output (from background thread via channel)
term.push_line("Compiling foo v0.1.0...");
term.push_ansi("\x1b[32mFinished\x1b[0m dev in 2.1s");
term.push_colored("error: type mismatch", [1.0, 0.3, 0.3, 1.0]);

// Render
if let Some(cmd) = term.render(ui) {
    // User pressed Enter in input line
    execute_command(&cmd);
}
```

---

## Proposal 2: `property_inspector` — Hierarchical Property Editor

**Priority: HIGH** — essential for debugger watches and IDE settings.

### What

A two-column tree-table for editing typed key-value pairs. Like Unity's
Inspector, Unreal's Details panel, or Chrome DevTools' object expander.

### Why Not Just Use virtual_tree?

`virtual_tree` is great for homogeneous node lists (files, tasks), but
a property inspector needs:
- Heterogeneous value types per row (string, int, float, bool, color, enum, vec, nested object)
- Automatic editor selection based on value type
- Search/filter that matches keys OR values
- Diff mode: highlight changed values (debugger: "what changed since last step?")
- Read-only computed fields alongside editable ones
- Category grouping with headers

### Architecture

```
property_inspector/
├── mod.rs          PropertyInspector widget
├── config.rs       InspectorConfig, display options
├── value.rs        PropertyValue enum (Bool, Int, Float, String, Color, Vec, Object...)
├── node.rs         PropertyNode — key, value, children, metadata
└── editor.rs       Per-type inline editors

┌─────────────────────────────────────────────────────┐
│ 🔍 Filter...                                       │
├─────────────────────────┬───────────────────────────┤
│ Property                │ Value                     │
├─────────────────────────┼───────────────────────────┤
│ ▾ Transform             │                           │  ← category header
│   position              │ [120.0, 45.0, 0.0]       │  ← Vec3 editor
│   rotation              │ 15.0°                     │  ← float + suffix
│   scale                 │ [1.0, 1.0]               │  ← Vec2
├─────────────────────────┼───────────────────────────┤
│ ▾ Material              │                           │
│   color                 │ ████ #FF6B35              │  ← color swatch + hex
│   opacity               │ ████████░░ 0.80           │  ← slider
│   shader                │ [PBR Standard    ▼]       │  ← dropdown
│   double_sided          │ ☑                         │  ← checkbox
├─────────────────────────┼───────────────────────────┤
│ ▾ Debug                 │                           │
│   frame_time            │ 16.2ms                    │  ← read-only, dimmed
│   draw_calls            │ 142 ⊿+3                   │  ← diff: changed!
│   ▸ allocations         │ {5 entries}               │  ← collapsed object
└─────────────────────────┴───────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| 15+ value types | Bool, I32, I64, F32, F64, String, Color3/4, Vec2/3/4, Enum, Flags, Object, Array, Path, Range |
| Auto-editor | Type → editor mapping (bool=checkbox, enum=combo, float=slider, color=picker) |
| Categories | Collapsible section headers with optional icons |
| Diff highlight | Orange flash / marker on values that changed since last snapshot |
| Search | Filter by key name or value content |
| Nested objects | Tree expand to show child properties |
| Read-only | Per-property or global read-only mode |
| Copy value | Right-click → Copy as text / JSON |
| Callbacks | `on_changed(key, old, new)` |

### Use Cases

```
Debugger:      Watch window — variables, registers, memory regions
               Highlight changed values after step-over
Pkg Manager:   Package metadata inspector (name, version, deps, features)
IDE:           Project settings, build config, plugin options
```

---

## Proposal 3: `diff_viewer` — Side-by-Side Diff

**Priority: HIGH** — essential for IDE and package manager.

### What

Two-panel synchronized diff viewer with syntax highlighting.
Shows added/removed/modified lines with color coding and
synchronized scrolling. Like `git diff` but rendered as a widget.

### Architecture

```
diff_viewer/
├── mod.rs          DiffViewer widget — dual-pane render
├── config.rs       DiffConfig, color scheme
├── diff.rs         Myers diff algorithm (LCS-based)
├── hunk.rs         DiffHunk, LineChange types
└── sync.rs         Synchronized scroll logic

┌─────────────────────────────┬─────────────────────────────┐
│ old.rs (before)             │ new.rs (after)              │
├─────────────────────────────┼─────────────────────────────┤
│  1  fn main() {             │  1  fn main() {             │
│  2 ─ println!("old");       │  2 + println!("new");       │  ← red / green
│  3   let x = 10;            │  3   let x = 10;            │
│    ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─   │  4 + let y = 20;            │  ← added line
│    ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─   │  5 + let z = x + y;         │
│  4   println!("{x}");       │  6   println!("{x}");        │
│  5  }                       │  7  }                        │
├─────────────────────────────┴─────────────────────────────┤
│ 2 files compared • 1 modified, 2 added, 1 removed        │
└───────────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| Myers diff | O(ND) algorithm, optimal edit distance |
| Side-by-side | Two panels with synchronized scroll |
| Unified mode | Single panel with +/- prefixes (toggle) |
| Syntax highlight | Reuses `code_editor` tokenizers for both panels |
| Inline diff | Character-level diff within changed lines (word highlighting) |
| Hunk navigation | Jump to next/previous change (F7/Shift+F7) |
| Fold unchanged | Collapse identical regions ("... 42 unchanged lines") |
| Mini-map | Colored bar showing change density (scrollbar overlay) |
| Merge mode | Accept left / right / both per hunk |

### Use Cases

```
Debugger:      Compare variable state between two breakpoints
Pkg Manager:   Show Cargo.toml changes before/after dependency update
IDE:           Git diff, unsaved changes, merge conflict resolution
```

---

## Proposal 4: `hex_viewer` — Memory / Binary Hex Viewer

**Priority: HIGH** — essential for debugger.

### What

Standalone hex dump widget for raw memory/binary inspection.
Classic 3-column layout: offset | hex bytes | ASCII. With regions,
highlighting, editing, goto, data inspector sidebar.

### Why Not code_editor with Language::Hex?

`code_editor` Hex mode is for editing hex text. This is for **binary data**:
- Input is `&[u8]`, not text
- Shows offset addresses
- Side-by-side hex + ASCII columns
- Highlights data types (struct fields mapped to color regions)
- Data inspector (u8, u16, u32, f32, string at cursor)
- No line numbers — uses memory addresses

### Architecture

```
hex_viewer/
├── mod.rs          HexViewer widget
├── config.rs       HexConfig, column count, grouping
├── region.rs       ColorRegion — map byte ranges to colors/labels
├── inspector.rs    DataInspector — interpret bytes as various types
└── search.rs       Byte pattern search (hex string or ASCII)

┌─────────────────────────────────────────────────────────────────┐
│ Hex Viewer                              Goto: [0x00401000    ] │
├──────────┬──────────────────────────────────┬──────────────────┤
│ Offset   │ 00 01 02 03  04 05 06 07  08-0F │ ASCII            │
├──────────┼──────────────────────────────────┼──────────────────┤
│ 00000000 │ 4D 5A 90 00  03 00 00 00  04 00 │ MZ..........     │  ← PE header
│ 00000010 │ 00 00 FF FF  00 00 B8 00  00 00 │ ..ÿÿ..¸...      │
│ 00000020 │ 00 00 00 00  40 00 00 00  00 00 │ ....@.....       │
│ 00000030 │ 00 00 00 00  00 00 00 00  00 00 │ ..........       │
│ 00000040 │ PE\0\0 4C 01  06 00 A2 B3  C4 D5 │ PE..L.....       │  ← highlighted
│          │ ▲▲▲▲▲▲▲▲▲▲                       │                  │
│          │ struct_field: magic               │                  │
├──────────┴──────────────────────────────────┴──────────────────┤
│ Inspector: u8=0x4D  u16=0x5A4D  u32=0x00905A4D  f32=4.26e-37 │
│            i8=77    i16=23117   ascii="MZ"       utf8="MZ"    │
└───────────────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| Virtual scroll | Handle gigabyte files, renders only visible rows |
| Column config | 8 / 16 / 32 bytes per row, grouping (1/2/4/8) |
| Color regions | Map `(offset, len)` → color + label (struct fields) |
| Data inspector | Sidebar: interpret selected bytes as u8..u64, f32/f64, string |
| Edit mode | Click byte → type new hex value, optional write-back |
| Goto address | Jump to offset (decimal or hex) |
| Search | Find byte pattern, hex string, or ASCII text |
| Selection | Range select → copy as hex, C array, Rust slice, Base64 |
| Diff regions | Highlight bytes that differ from a reference buffer |
| Endianness | Toggle LE/BE for multi-byte inspector values |

### Use Cases

```
Debugger:      Memory view at address, stack dump, register contents
               Struct overlay: color-code fields of a known struct
Pkg Manager:   Inspect binary crate artifacts, .rlib internals
IDE:           Binary file preview, .wasm inspection
```

---

## Proposal 5: `timeline` — Profiler Timeline / Flame Graph

**Priority: MEDIUM** — debugger and IDE profiling.

### What

Zoomable horizontal timeline for profiler data. Shows nested call
spans as colored bars. Can display flame graphs (bottom-up),
icicle charts (top-down), and event markers.

### Architecture

```
timeline/
├── mod.rs          Timeline widget — pan/zoom, rendering
├── config.rs       TimelineConfig, colors, zoom limits
├── span.rs         Span { start, end, depth, label, color }
├── track.rs        Track — named row of spans (thread, category)
├── flame.rs        Flame graph aggregation (merge identical stacks)
└── ruler.rs        Time ruler with adaptive tick marks

┌──────────────────────────────────────────────────────────────┐
│ Timeline   [Flame ▼]  ◀ ● ▶  Fit  │ 0ms      50ms    100ms│
├──────────────────────────────────────────────────────────────┤
│ Main Thread                                                  │
│ ┃ ████████████████████ frame() ████████████████████████████ │
│ ┃ ┃ ████ update() ████ ┃ ████████ render() █████████████  │
│ ┃ ┃ ┃ ██ physics ██   ┃ ┃ ██ draw_nodes ██ ┃ ██ swap ██  │
│ ┃ ┃ ┃              ┃   ┃ ┃ ┃ ██ batch ██  ┃ ┃            │
├──────────────────────────────────────────────────────────────┤
│ GPU Thread                                                   │
│ ┃ ░░░░░░░ idle ░░░░░░ ┃ ████████ submit ████ ┃ ██ present │
├──────────────────────────────────────────────────────────────┤
│ ▼ Hovered: render() — 48.2ms (62%) — src/engine.rs:142      │
└──────────────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| Pan + zoom | Mouse drag to pan, scroll to zoom, fit-to-content |
| Multi-track | Separate rows per thread / category |
| Nested spans | Call tree depth rendered as stacked bars |
| Flame graph | Aggregate identical call stacks (bottom-up) |
| Tooltip | Hover → span name, duration, percentage, source location |
| Selection | Click span → callback with span data |
| Markers | Vertical lines for events (frame boundaries, GC pauses) |
| Color coding | By function, by module, by duration (hot = red) |
| Time ruler | Adaptive ticks (ns → μs → ms → s) |
| Data streaming | Append spans in real-time (profiler connected) |

### Use Cases

```
Debugger:      Execution timeline, function call profiling
               Thread visualization, lock contention spans
IDE:           Build profiler (which crate took how long?)
               cargo build timing, proc-macro expansion time
```

---

## Proposal 6: `status_bar` — Bottom Status Bar

**Priority: LOW** (small widget, but universal).

```
┌───────────────────────────────────────────────────────────────┐
│ ● Connected │ Ln 42, Col 15 │ UTF-8 │ Rust │ LF │ 4 spaces │
└───────────────────────────────────────────────────────────────┘
```

Composable sections: left/center/right aligned, clickable items,
progress indicators, colored status dots.

---

## Proposal 7: `toolbar` — Configurable Toolbar

**Priority: LOW** (small widget, but universal).

```
┌────────────────────────────────────────────────────────────┐
│ 📄 💾 │ ↩ ↪ │ ▶ Run │ 🔍 │ [Debug ▼] │ ··· │ ⚙        │
└────────────────────────────────────────────────────────────┘
```

Button, separator, dropdown, toggle, search field, spacer.
Builder pattern: `toolbar.button(icon, tooltip).separator().dropdown(...)`.

---

## Recommendation Order

```
Priority  Component            Effort   Impact   Shared across apps
────────  ──────────────────   ──────   ──────   ──────────────────
  #1      terminal             ~5 days   █████   Debugger + Pkg + IDE
  #2      property_inspector   ~4 days   ████░   Debugger + IDE
  #3      diff_viewer          ~4 days   ████░   Pkg + IDE
  #4      hex_viewer           ~3 days   ███░░   Debugger (+IDE)
  #5      timeline             ~5 days   ███░░   Debugger (+IDE)
  #6      status_bar           ~1 day    ██░░░   All three
  #7      toolbar              ~1 day    ██░░░   All three
```

### Suggested build order

```
Phase 1 (foundation):  terminal → property_inspector
Phase 2 (IDE focus):   diff_viewer → status_bar → toolbar
Phase 3 (debug focus): hex_viewer → timeline
```

**Terminal** is #1 because it unblocks the most workflows — any app
that runs external processes needs it. **Property inspector** is #2
because debugger watch windows and IDE settings both need it, and
it's architecturally unique (no existing component covers typed
hierarchical editing).

---

## Component Interaction Map

```
                         ┌─────────────┐
                    ┌───▶│ page_control │◀──── tab per tool window
                    │    └──────┬──────┘
                    │           │ hosts
                    │           ▼
    ┌───────────┐   │  ┌─────────────────┐    ┌──────────────┐
    │ toolbar   │───┤  │  code_editor    │───▶│ diff_viewer  │
    └───────────┘   │  └────────┬────────┘    └──────────────┘
                    │           │ errors                ▲
                    │           ▼                       │ compare
    ┌───────────┐   │  ┌─────────────────┐    ┌────────┴─────┐
    │ status_bar│───┤  │    terminal     │───▶│ file_manager │
    └───────────┘   │  └────────┬────────┘    └──────────────┘
                    │           │ output
                    │           ▼
                    │  ┌─────────────────┐    ┌──────────────┐
                    │  │property_inspect │    │  hex_viewer  │
                    │  └────────┬────────┘    └──────┬───────┘
                    │           │ watches             │ memory
                    │           ▼                     ▼
                    │  ┌─────────────────┐    ┌──────────────┐
                    └─▶│  virtual_tree   │    │  timeline    │
                       └─────────────────┘    └──────────────┘
```

Each new component reuses the existing foundation:
- **terminal** reuses `RingBuffer` from virtual_table, ANSI → `SyntaxColors`
- **property_inspector** reuses `CellEditor` types from virtual_table
- **diff_viewer** reuses `code_editor` tokenizers for syntax highlighting
- **hex_viewer** reuses `code_editor` hex coloring logic + DrawList batching
- **timeline** reuses `node_graph` pan/zoom + DrawList rendering patterns
