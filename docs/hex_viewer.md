# HexViewer

Standalone hex dump widget for Dear ImGui with editing, struct overlays, data inspector, byte search, and diff highlighting.

## Overview

`HexViewer` provides a classic 3-column hex dump layout: **offset | hex bytes | ASCII**. Designed for binary file inspection, memory viewers, and protocol analysis.

## Features

- **3-column layout**: offset, hex bytes, ASCII with synchronized cursor
- **Color regions** (struct overlays) — map byte ranges to colors and labels
- **Data inspector** panel — shows cursor byte as i8/u8/i16/u16/i32/u32/i64/u64/f32/f64
- **Goto address** popup (Ctrl+G) — jump to hex offset
- **Byte search** popup (Ctrl+F) — find hex pattern with next/prev result navigation
- **Selection** — click and drag to select byte ranges, with `selected_bytes()` accessor
- **Inline editing** — click hex digit to type new values (nibble-by-nibble)
- **Reference diff** — highlight bytes that differ from a reference snapshot
- **Column headers** — `00 01 02 ...` column labels
- **Byte grouping** — visual spacing: None, Word (2), DWord (4), QWord (8)
- **Configurable bytes per row**: 8, 16, or 32
- **Endianness toggle** — Little/Big endian for inspector values
- **Dim zeros** — zero bytes displayed as subtle dots instead of `00`
- **Base address** — configurable offset added to displayed addresses
- **Uppercase/lowercase hex** — configurable digit case
- **Keyboard navigation** — arrow keys to move cursor, scroll follows

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
| `set_cursor(offset)` | Set cursor position (auto-scrolls) |
| `goto(offset)` | Alias for `set_cursor` |

### Selection

| Method | Description |
|--------|-------------|
| `selection() -> Selection` | Current selection range |
| `selected_bytes() -> &[u8]` | Selected bytes as a slice |

### State

| Method | Description |
|--------|-------------|
| `is_focused() -> bool` | Whether the widget has focus |
| `config() -> &HexViewerConfig` | Immutable config access |
| `config_mut() -> &mut HexViewerConfig` | Mutable config access |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui)` | Render the hex viewer widget |

## Selection Type

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

## Configuration

```rust
let cfg = viewer.config_mut();

// Layout
cfg.bytes_per_row = BytesPerRow::Sixteen; // Eight, Sixteen, ThirtyTwo
cfg.grouping = ByteGrouping::DWord;       // None, Word, DWord, QWord

// Display
cfg.show_ascii = true;
cfg.show_inspector = true;
cfg.show_offsets = true;
cfg.show_column_headers = true;
cfg.uppercase = true;
cfg.dim_zeros = true;
cfg.base_address = 0x0040_0000;           // offset shown as base + cursor

// Behavior
cfg.editable = false;                     // enable hex editing
cfg.endianness = Endianness::Little;      // Little or Big
cfg.highlight_changes = false;            // diff vs reference
```

### ColorRegion

```rust
pub struct ColorRegion {
    pub offset: usize,       // start byte
    pub len: usize,          // length in bytes
    pub color: [f32; 4],     // RGBA 0.0..=1.0
    pub label: String,       // human-readable name (shown on hover)
}

ColorRegion::new(0, 4, [0.3, 0.6, 0.9, 0.4], "header");
```

### Colors

| Field | Description |
|-------|-------------|
| `color_offset` | Offset column text |
| `color_hex` | Normal hex byte text |
| `color_ascii` | Printable ASCII character |
| `color_ascii_dot` | Non-printable ASCII dot |
| `color_zero` | Zero byte (when `dim_zeros` enabled) |
| `color_selection_bg` | Selection highlight |
| `color_changed` | Changed byte (diff mode) |
| `color_cursor_bg` | Cursor highlight |
| `color_header` | Column header text |
| `color_inspector_label` | Inspector field labels |
| `color_inspector_value` | Inspector field values |

## Architecture

```
hex_viewer/
  mod.rs      HexViewer struct, rendering, input, goto/search popups
  config.rs   HexViewerConfig, ColorRegion, BytesPerRow, ByteGrouping, Endianness
```
