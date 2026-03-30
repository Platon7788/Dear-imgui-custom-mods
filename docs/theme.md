# Theme

Shared color palette module for consistent styling across all Dear ImGui custom components.

## Overview

The `theme` module provides a centralized set of color constants used by all components. Modify this module to reskin the entire UI.

All colors are `[f32; 4]` RGBA values in `0.0..=1.0` range.

## Color Palette

### Backgrounds

| Constant | RGB (approx) | Usage |
|----------|-------------|-------|
| `BG_WINDOW` | `#1E2129` | Main window background |
| `BG_CHILD` | `#242630` | Child window / panel background |
| `BG_FRAME` | `#2A2E38` | Input frame background |
| `BG_CHILD_HOVER` | `#2E3340` | Hovered child window |

### Accent (Blue)

| Constant | Usage |
|----------|-------|
| `ACCENT` | Primary accent (buttons, links, focus) |
| `ACCENT_HOVER` | Hovered accent |
| `ACCENT_ACTIVE` | Pressed accent |

### Status Colors

| Constant | Color | Usage |
|----------|-------|-------|
| `SUCCESS` | Green | Success indicators, confirmations |
| `SUCCESS_HOVER` | Light green | Hovered success |
| `SUCCESS_ACTIVE` | Dark green | Pressed success |
| `DANGER` | Red | Errors, destructive actions |
| `DANGER_HOVER` | Light red | Hovered danger |
| `DANGER_ACTIVE` | Dark red | Pressed danger |
| `WARNING` | Amber | Warnings, caution |

### Text

| Constant | Usage |
|----------|-------|
| `TEXT_PRIMARY` | Main text |
| `TEXT_SECONDARY` | Secondary/dimmed text |
| `TEXT_MUTED` | Muted/placeholder text |
| `TEXT_ERROR` | Error message text |

### UI Elements

| Constant | Usage |
|----------|-------|
| `BORDER` | Border lines |
| `SEPARATOR` | Separator lines |
| `SELECTION_BG` | Selection highlight (40% alpha) |

## Usage

```rust
use dear_imgui_custom_mod::theme;

// Use in custom rendering
let bg = theme::BG_WINDOW;
let text = theme::TEXT_PRIMARY;
let accent = theme::ACCENT;

// Use for component config overrides
editor.config_mut().colors.background = theme::BG_CHILD;
```

## Architecture

```
theme/
  mod.rs    Color constants — all `pub const` [f32; 4] values
```
