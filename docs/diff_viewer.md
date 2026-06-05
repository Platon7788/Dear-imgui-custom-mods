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

Uses the Myers diff algorithm (`diff_lines`) which produces a **minimal**
edit script of `DiffOp` operations:

| Operation | Description |
|-----------|-------------|
| `Equal { old_idx, new_idx }` | Line is unchanged (the two indices always reference identical lines) |
| `Delete { old_idx }` | Line was removed from old text |
| `Insert { new_idx }` | Line was added in new text |

Hunks are grouped with `group_hunks(ops, context_lines)` for navigation.

### Correctness guarantees

The edit script is verified (by a DP-LCS oracle and a deterministic
fuzz test) to satisfy three invariants for *any* input:

1. every `Equal` pairs lines that are genuinely identical;
2. `Equal`+`Delete` reconstructs `old` exactly, `Equal`+`Insert`
   reconstructs `new` exactly (no dropped/duplicated/reordered lines);
3. the edit count equals `n + m − 2·LCS` (the script is minimal).

Edge cases handled: empty inputs (one or both sides), identical
inputs, fully-disjoint inputs, common prefix/suffix, single-line
inputs, very long lines (compared atomically), and Unicode. `str::lines()`
strips a trailing `\r\n` *and* a lone `\n`, so a CRLF document and an
LF document with identical content diff as fully equal.

### Large-input guard

Combined inputs above `diff::MAX_DIFF_INPUT_LINES` (20 000 lines) fall
back to a `delete-all-then-insert-all` diff — correct but not minimal —
to bound the Myers trace memory (`O((N+M)²)` in the worst case).
Callers with bigger documents should pre-chunk them.

## Performance

- **Diff is cached** — `set_texts` recomputes the diff and display
  lines once; `render` only reads the precomputed `left_lines` /
  `right_lines`, never re-diffing per frame.
- **Viewport culling** — both the side-by-side panels and the unified
  pane draw only the rows intersecting the visible scroll region
  (plus one row of slack), so the per-frame draw cost is proportional
  to what is on screen, not to the total diff size.
- Row height is read from the font size via `Ui::text_line_height()`
  rather than a per-frame `CalcTextSize` glyph walk.

## Architecture

```
diff_viewer/
  mod.rs      DiffViewer struct, public API, state, recompute, tests
  build.rs    DiffViewer::build_display_lines (DiffOp -> paired display rows, folding)
  render.rs   DiffViewer::render + header / side-by-side / unified drawing
  config.rs   DiffViewerConfig, DiffMode (schema)
  config.ron  default values (DDD config pattern)
  diff.rs     Myers diff algorithm, DiffOp, DiffHunk, group_hunks
```

## Configuration & localisation

`DiffViewerConfig` follows the project-wide DDD config pattern:
schema in `src/diff_viewer/config.rs`, default values in
`src/diff_viewer/config.ron`. See [`docs/config_pattern.md`](./config_pattern.md).

The toolbar Prev/Next hunk-navigation buttons are localised through
`crate::i18n::diff_viewer`. Switch with
`DiffViewer::new(...).with_locale(Locale::Ru)`. Diff content (the
old/new texts and any user labels) stays host-driven. See
[`docs/i18n.md`](./i18n.md).
