# DiffViewer

Side-by-side or unified diff viewer for Dear ImGui with synchronized scrolling, line numbers, change highlighting, fold unchanged regions, and hunk navigation.

## Overview

`DiffViewer` is a self-contained diff widget that takes two text inputs (old and new), computes a line-level diff using the Myers algorithm, and renders it with full syntax coloring for additions, deletions, and fold markers.

## Features

- **Two display modes**: Side-by-side (two panels) and Unified (single panel with +/- prefixes)
- **Hunk navigation** with Prev/Next buttons (F7 / Shift+F7) and current hunk indicator
- **Fold unchanged regions** — collapses long equal runs with configurable context lines
- **Line numbers** per-panel (old numbers on left, new numbers on right)
- **Change statistics** — header shows +added -removed ~modified counts
- **Hover row highlighting** — subtle highlight on mouse-over rows
- **Current hunk accent bar** — blue vertical bar marking the active hunk in unified mode
- **Configurable colors** — 14 color slots for full theme control

## Quick Start

```rust
use dear_imgui_custom_mod::diff_viewer::DiffViewer;

let mut dv = DiffViewer::new("##diff");
dv.set_texts("old content\nline 2", "new content\nline 2\nline 3");

// In render loop:
let events = dv.render(ui);
for event in events {
    match event {
        DiffViewerEvent::HunkSelected { index } => { /* jumped to hunk */ }
    }
}
```

## Public API

### Construction & Data

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new diff viewer with the given ImGui ID |
| `set_texts(old, new)` | Set both texts and recompute the diff |

### Navigation

| Method | Description |
|--------|-------------|
| `hunk_count()` | Number of hunks (change groups) |
| `next_hunk()` | Navigate to next hunk (wraps around) |
| `prev_hunk()` | Navigate to previous hunk (wraps around) |

### Labels

| Field | Description |
|-------|-------------|
| `old_label` | Label for old/left panel (default: `"old"`) |
| `new_label` | Label for new/right panel (default: `"new"`) |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui) -> Vec<DiffViewerEvent>` | Render the widget. Returns events |

## Events

| Event | Description |
|-------|-------------|
| `HunkSelected { index }` | User navigated to a hunk via Prev/Next buttons |

## Configuration

All configuration is in `dv.config`:

```rust
let cfg = &mut dv.config;

cfg.mode = DiffMode::SideBySide;  // or DiffMode::Unified
cfg.show_line_numbers = true;
cfg.fold_unchanged = true;
cfg.context_lines = 3;            // context lines around changes when folding
cfg.show_minimap = false;
cfg.sync_scroll = true;
```

### DiffMode

| Mode | Description |
|------|-------------|
| `SideBySide` | Two panels — old on left, new on right (default) |
| `Unified` | Single panel with `+`/`-` prefixes (git-style) |

### Colors

| Field | Description |
|-------|-------------|
| `color_bg` | Background color |
| `color_gutter_bg` | Gutter (line number area) background |
| `color_line_number` | Line number text color |
| `color_text` | Normal text color |
| `color_added_bg` | Added line background (green tint) |
| `color_added_text` | Added line text color |
| `color_removed_bg` | Removed line background (red tint) |
| `color_removed_text` | Removed line text color |
| `color_modified_bg` | Modified line background |
| `color_inline_change` | Character-level inline change highlight |
| `color_fold` | Fold marker text and separator |
| `color_header` | Header/filename text |
| `color_separator` | Panel separator line |
| `color_current_hunk` | Current hunk accent highlight |

## Diff Algorithm

Uses the Myers diff algorithm (`diff_lines`) which produces a minimal edit script of `DiffOp` operations:

| Operation | Description |
|-----------|-------------|
| `Equal { old_idx, new_idx }` | Line is unchanged |
| `Delete { old_idx }` | Line was removed from old text |
| `Insert { new_idx }` | Line was added in new text |

Hunks are grouped with `group_hunks(ops, context_lines)` for navigation.

## Architecture

```
diff_viewer/
  mod.rs      DiffViewer struct, rendering, display line computation
  config.rs   DiffViewerConfig, DiffMode
  diff.rs     Myers diff algorithm, DiffOp, DiffHunk, group_hunks
```
