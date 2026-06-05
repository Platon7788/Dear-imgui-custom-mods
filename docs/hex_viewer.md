# HexViewer

Standalone hex dump widget for Dear ImGui with editing, struct overlays, data inspector, wildcard search, undo/redo, navigation history, semantic byte coloring, and diff highlighting.

## Overview

`HexViewer` provides a classic 3-column hex dump layout: **offset | hex bytes | ASCII**. Designed for binary file inspection, memory viewers, debuggers, and protocol analysis.

## Features

- **3-column layout**: offset, hex bytes, ASCII with synchronized cursor
- **Color regions** (struct overlays) — map byte ranges to colors and labels
- **Data inspector** panel — shows cursor bytes as u8/i8/u16/u32/u64/f32/f64/char/hex (endianness-aware), plus an address + edit-state footer
- **Goto address** popup (Ctrl+G) — jump to hex offset
- **Wildcard search** (Ctrl+F) — hex pattern with `??` wildcards (`4D 5A ?? 00`) plus ASCII / UTF-8 / UTF-16LE string modes
- **Selection** — click and drag to select byte ranges, with `selected_bytes()` accessor
- **Inline editing** — click hex digit to type new values (nibble-by-nibble) with **undo/redo**
- **Undo / Redo** (Ctrl+Z / Ctrl+Y) — configurable stack depth (default: 256)
- **Navigation history** (Alt+Left / Alt+Right) — back/forward address navigation
- **Reference diff** — highlight bytes that differ from a reference snapshot
- **Semantic byte coloring** — 5-tier category palette: Zero (dim salmon), Control (gray), Printable (green), High (purple), 0xFF (amber)
- **6 copy formats** — HexSpaced, HexCompact, CArray, RustArray, Base64, ASCII (Ctrl+C)
- **Search match highlighting** — all pattern matches highlighted in the view
- **Auto-refresh** — configurable frame interval for live data sources
- **HexDataProvider trait** — abstract data source for remote memory, page caches, or memory-mapped files
- **Column headers** — `00 01 02 ...` column labels
- **Byte grouping** — visual spacing: None, Word (2), DWord (4), QWord (8)
- **Configurable bytes per row**: 8 / 12 / 16 / 20 / 24 / 28 / 32 presets, or any multiple of 4 in `4..=64`
- **Endianness toggle** — Little/Big endian for inspector values
- **Dim zeros** — zero bytes displayed as subtle dots instead of `00`
- **Base address** — configurable offset added to displayed addresses
- **Uppercase/lowercase hex** — configurable digit case
- **Keyboard navigation** — arrow keys, PgUp/Dn, Home/End, Shift+Arrow for selection

## Quick Start

```rust
use dear_imgui_custom_mod::hex_viewer::HexViewer;

let data: Vec<u8> = vec![0x4D, 0x5A, 0x90, 0x00, 0x03];
let mut viewer = HexViewer::new("##hex");
viewer.set_data(&data);

// In render loop:
viewer.render(ui);
```

### With Struct Overlays

```rust
use dear_imgui_custom_mod::hex_viewer::{HexViewer, ColorRegion};

let mut viewer = HexViewer::new("##hex");
viewer.set_data(&pe_bytes);
viewer.set_regions(vec![
    ColorRegion::new(0, 2, [0.3, 0.6, 0.9, 0.4], "MZ signature"),
    ColorRegion::new(60, 4, [0.9, 0.4, 0.3, 0.4], "PE offset"),
]);
```

### Custom Data Provider

```rust
use dear_imgui_custom_mod::hex_viewer::HexDataProvider;

struct RemoteMemory { /* page cache, process handle, etc. */ }

impl HexDataProvider for RemoteMemory {
    fn len(&self) -> u64 { 0x7FFF_FFFF_FFFF }
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize { /* ... */ 0 }
    fn write(&mut self, offset: u64, data: &[u8]) -> bool { /* ... */ true }
    fn is_readable(&self, offset: u64) -> bool { /* check region map */ true }
    fn is_changed(&self, offset: u64) -> bool { /* compare snapshots */ false }
    fn refresh(&mut self) { /* re-fetch stale pages */ }
}
```

## Public API

### Construction

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new hex viewer |

### Data Management

| Method | Description |
|--------|-------------|
| `set_data(data: &[u8])` | Set data buffer (copies) |
| `set_data_vec(data: Vec<u8>)` | Set data buffer (zero-copy move) |
| `data() -> &[u8]` | Read-only access to data |
| `data_mut() -> &mut Vec<u8>` | Mutable access for external edits |
| `data_len() -> usize` | Total data length |
| `set_reference(ref: &[u8])` | Set reference snapshot for diff highlighting |
| `clear_reference()` | Clear reference snapshot |

### Regions (Struct Overlays)

| Method | Description |
|--------|-------------|
| `set_regions(regions)` | Replace all color regions |
| `add_region(region)` | Add a single color region |
| `clear_regions()` | Remove all color regions |

### Navigation

| Method | Description |
|--------|-------------|
| `cursor() -> usize` | Current cursor byte offset |
| `set_cursor(offset)` | Set cursor position (auto-scrolls, records history) |
| `goto(offset)` | Alias for `set_cursor` |
| `nav_back()` | Navigate to previous address in history (Alt+Left) |
| `nav_forward()` | Navigate to next address in history (Alt+Right) |

### Selection

| Method | Description |
|--------|-------------|
| `selection() -> Selection` | Current selection range |
| `selected_bytes() -> &[u8]` | Selected bytes as a slice |

### Undo / Redo

| Method | Description |
|--------|-------------|
| `undo()` | Undo last edit (Ctrl+Z) |
| `redo()` | Redo last undone edit (Ctrl+Y) |
| `undo_stack() -> &UndoStack` | Access undo stack state |

### State

| Method | Description |
|--------|-------------|
| `is_focused() -> bool` | Whether the widget has focus |
| `config() -> &HexViewerConfig` | Immutable config access |
| `config_mut() -> &mut HexViewerConfig` | Mutable config access |
| `nav_history() -> &NavHistory` | Access navigation history |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui)` | Render the hex viewer widget |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Arrow keys | Move cursor (with Shift: extend selection) |
| Page Up/Down | Jump by screen height |
| Home/End | Line start/end (Ctrl: data start/end) |
| Ctrl+A | Select all |
| Ctrl+C | Copy selection (uses configured copy format) |
| Ctrl+F | Open search popup |
| Ctrl+G | Open goto address popup |
| Ctrl+Z | Undo |
| Ctrl+Y / Ctrl+Shift+Z | Redo |
| Alt+Left | Navigate back |
| Alt+Right | Navigate forward |
| F3 / Shift+F3 | Next / previous search result |
| 0-9, A-F | Hex input (when editable and cursor active) |

## Types

### Selection

```rust
pub struct Selection {
    pub start: usize,
    pub end: usize,
}
impl Selection {
    fn is_empty(&self) -> bool;
    fn contains(&self, offset: usize) -> bool;
    fn ordered(&self) -> (usize, usize);
    fn len(&self) -> usize;
}
```

### PatternByte (Wildcard Search)

```rust
pub enum PatternByte {
    Exact(u8),  // match specific byte
    Any,        // match any byte (??)
}
```

### ByteCategory (Semantic Coloring)

```rust
pub enum ByteCategory {
    Zero,       // 0x00
    Control,    // 0x01..0x1F, 0x7F
    Printable,  // 0x20..0x7E
    High,       // 0x80..0xFE
    Full,       // 0xFF
}
```

### CopyFormat

```rust
pub enum CopyFormat {
    HexSpaced,    // "4D 5A 90 00"
    HexCompact,   // "4D5A9000"
    CArray,       // "{ 0x4D, 0x5A, 0x90, 0x00 }"
    RustArray,    // "[0x4D, 0x5A, 0x90, 0x00]"
    Base64,       // "TVqQAA=="
    Ascii,        // "MZ.."
}
```

### HexSearchMode

```rust
pub enum HexSearchMode {
    Hex,                      // hex bytes with ?? wildcards
    String(StringEncoding),   // text search, encoded to wire bytes
}

pub enum StringEncoding {
    Ascii,     // 1 byte/char (Latin-1 wire bytes)
    Utf8,      // full UTF-8 wire bytes
    Utf16Le,   // 2 bytes/code-unit, little-endian (Windows wchar_t)
}
```

## BytesPerRow

`BytesPerRow` is a newtype struct. All standard presets are provided as associated constants:

| Constant | Bytes |
|----------|-------|
| `BytesPerRow::EIGHT` | 8 |
| `BytesPerRow::TWELVE` | 12 |
| `BytesPerRow::SIXTEEN` | 16 (default) |
| `BytesPerRow::TWENTY` | 20 |
| `BytesPerRow::TWENTY_FOUR` | 24 |
| `BytesPerRow::TWENTY_EIGHT` | 28 |
| `BytesPerRow::THIRTY_TWO` | 32 |

Custom values: `BytesPerRow::new(n)` — clamps `n` to `4..=64`, rounds down to the nearest multiple of 4.

```rust
cfg.bytes_per_row = BytesPerRow::SIXTEEN;     // standard preset
cfg.bytes_per_row = BytesPerRow::new(20);     // custom value
```

## Configuration

```rust
let cfg = viewer.config_mut();

// Layout
cfg.bytes_per_row = BytesPerRow::SIXTEEN;
cfg.grouping = ByteGrouping::DWord;

// Display
cfg.show_ascii = true;
cfg.show_inspector = true;
cfg.show_offsets = true;
cfg.show_column_headers = true;
cfg.uppercase = true;
cfg.dim_zeros = true;
cfg.category_colors = true;         // enable 5-tier byte coloring
cfg.base_address = 0x0040_0000;

// Behavior
cfg.editable = false;
cfg.endianness = Endianness::Little;
cfg.highlight_changes = false;
cfg.search_mode = HexSearchMode::Hex;
cfg.copy_format = CopyFormat::HexSpaced;
cfg.max_undo = 256;
cfg.auto_refresh_frames = 0;        // 0 = disabled
```

### ColorRegion

```rust
pub struct ColorRegion {
    pub offset: usize,
    pub len: usize,
    pub color: [f32; 4],    // RGBA 0.0..=1.0
    pub label: String,
}
ColorRegion::new(0, 4, [0.3, 0.6, 0.9, 0.4], "header");
```

### Colors

#### UI Element Colors

| Field | Description |
|-------|-------------|
| `color_offset` | Offset column text |
| `color_hex` | Normal hex byte text (when `category_colors` is off) |
| `color_ascii` | Printable ASCII character |
| `color_ascii_dot` | Non-printable ASCII dot |
| `color_zero` | Zero byte (legacy, when `dim_zeros` enabled) |
| `color_selection_bg` | Selection highlight |
| `color_changed` | Changed byte (diff mode) |
| `color_cursor_bg` | Cursor highlight |
| `color_header` | Column header text |
| `color_inspector_label` | Inspector field labels |
| `color_inspector_value` | Inspector field values |
| `color_search_match` | Search result highlight |
| `color_unreadable` | Non-readable region background |

#### Semantic Byte Category Colors

| Field | Byte Range | Default |
|-------|-----------|---------|
| `color_cat_zero` | `0x00` | Dim salmon |
| `color_cat_control` | `0x01..0x1F`, `0x7F` | Gray-blue |
| `color_cat_printable` | `0x20..0x7E` | Green |
| `color_cat_high` | `0x80..0xFE` | Muted purple |
| `color_cat_full` | `0xFF` | Amber |

## HexDataProvider Trait

```rust
pub trait HexDataProvider {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool;
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize;
    fn write(&mut self, offset: u64, data: &[u8]) -> bool;
    fn is_readable(&self, offset: u64) -> bool;
    fn is_changed(&self, offset: u64) -> bool;
    fn refresh(&mut self);
}
```

Built-in implementation: `VecDataProvider` wraps `Vec<u8>` with optional reference diff.

## Architecture

The module is split into focused files, each kept under the 500-line
ceiling. Cross-file access uses `pub(super)` visibility — every
sub-module is inside `hex_viewer`, so `super` is the same crate-private
boundary.

```
hex_viewer/
  mod.rs        HexViewer struct + fields, `new`, callback setters,
                locale, `Drop`
  api.rs        public data-management / cursor / VA-native /
                inspector-height / render-entry methods
  config.rs     HexViewerConfig schema + enums (BytesPerRow, ByteGrouping,
                Endianness, AddressWidth, StringEncoding, HexSearchMode,
                CopyFormat) — values live in config.ron
  config.ron    default field values (DDD schema/values split)
  provider.rs   HexDataProvider trait, VecDataProvider, ArcVecDataProvider,
                ColorRegion, ByteCategory
  nav_history.rs  NavHistory back/forward address stack
  undo.rs       UndoEntry, UndoStack
  search.rs     Selection, PatternByte, wildcard/string parsers,
                clipboard-format conversion, do_search, parse_address
  input.rs      keyboard handling, EditColumn, raw char-queue reader
  edit.rs       edit-mode state machine, undo/redo, copy, cursor moves
  mouse.rs      mouse handling + hit-testing (mouse_to_offset / row)
  draw.rs       render entry point + ListClipper-style virtualisation loop
  layout.rs     column-geometry math, header drawing, search-match probe,
                per-byte colour resolution, the inspector splitter
  row.rs        single-row byte drawing (offset / hex / ASCII columns)
  inspector.rs  data-inspector subview (u8..f64 reinterpretations)
  popup.rs      goto / search / context-menu / settings popups
  tests/        themed unit + property tests (search / edit / layout /
                config), declared via `#[cfg(test)] mod tests;`
```

## Provider-driven rendering

Three render entry points share one provider-agnostic pipeline:

- `render(ui)` — legacy zero-arg path; wraps the internal buffer in an
  `ArcVecDataProvider` (single `Arc::clone`).
- `render_with_provider(ui, &mut p)` — generic over any
  `HexDataProvider`; the viewer reads every visible byte through the
  provider (one batched read per visible row + one for the inspector)
  and routes edits through `provider.write()`. For streaming / sliding-
  window sources (debugger memory pane, raw-disk view) `set_data*` is
  irrelevant — the provider is the source of truth.
- `render_with_dyn_provider(ui, &mut dyn HexDataProvider)` — trait-object
  overload.

Only the visible row window is read and drawn each frame; a streaming
provider returning `u64::MAX` from `len()` is clamped so scroll-extent
math stays finite.

## VA-native API

For hosts driving a sliding-window view (VA-first, like `disasm_view`):
`cursor_address()`, `contains_va(va)`, `goto_address(va)`,
`viewport_first_va()` / `viewport_last_va()`,
`set_viewport_first_va(va)` (scroll-only follow), plus
`set_va_goto_callback` (re-anchor on out-of-window goto),
`set_byte_edit_callback` (`(va, old, new)` per committed edit), and
`set_address_formatter` (per-row `module+offset` style labels).

## Localisation

The column headers, goto/search/settings popups, context menu, per-byte
hover tooltip, and inspector footer are localised through
`crate::i18n::hex_viewer`. Switch with
`HexViewer::new(...).with_locale(Locale::Ru)` (or `set_locale` /
`locale()`); the locale lives on `HexViewerConfig::locale` and
round-trips through ron. Russian requires the host to bake
`GlyphRanges::Cyrillic` into the active font atlas.

## Tests

85 unit + property tests, split across `tests/`:
- **search_tests** — address/pattern parsing, wildcard + hex/ASCII/
  UTF-8/UTF-16LE search, end-of-buffer selection bounds, all 6 copy
  formats, Base64 (RFC 4648), parser property tests (never-panic).
- **edit_tests** — undo/redo round-trip + depth-limit trim +
  overflow-offset hardening, nav-history bounds (empty back/forward,
  forward-clear on diverge), pending-nibble commit, cursor movement /
  shift-select / select-all, selection single-byte/empty/out-of-range.
- **layout_tests** — address-width policy (incl. u64::MAX saturation),
  offset/hex/ASCII column geometry, address-literal formatting,
  inspector-height accessors, region/diff colour overrides, search-match
  binary probe.
- **config_tests** — config defaults + ron round-trip, the four i18n
  guard tests, byte-category boundaries, providers, popup-trigger API,
  VA-native API (contains_va / goto_address / cursor_address).

## Configuration & localisation

`HexViewerConfig` follows the project-wide DDD config pattern: schema
in `src/hex_viewer/config.rs`, default values in
`src/hex_viewer/config.ron`. See [`docs/config_pattern.md`](./config_pattern.md).

The viewer's column headers, goto/search popups, settings popup,
context menu, per-byte hover tooltip, and inspector footer are all
localised through `crate::i18n::hex_viewer`. Switch with
`HexViewer::new(...).with_locale(Locale::Ru)` — the locale is stored
on `HexViewerConfig::locale` and round-trips through ron. See
[`docs/i18n.md`](./i18n.md).
