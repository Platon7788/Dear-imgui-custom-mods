# Theme

Unified application-wide theme selector for all Dear ImGui custom components.

## Overview

The `theme` module exposes a single `Theme` enum that owns the full visual
stack for every built-in palette (titlebar, nav panel, confirm dialog, status
bar, and the Dear ImGui widget style). Components take a `Theme` value by
configuration, resolve the matching sub-palette at render time, and fall back
to a per-instance `colors_override` when a caller needs a custom look.

There is no per-component theme enum any more — the old `TitlebarTheme`,
`NavTheme`, and `DialogTheme` have been collapsed into `Theme`.

All raw colours are still `[f32; 4]` RGBA values in `0.0..=1.0` range.

## The `Theme` enum

```rust
use dear_imgui_custom_mod::theme::Theme;

let t: Theme = Theme::default();     // Dark
let next = t.next();                 // cycles through Theme::ALL

for theme in Theme::ALL {
    println!("{}", theme.display_name());
}
```

| Variant | Description |
|---------|-------------|
| `Theme::Dark` | NxT native dark palette (warm grey + blue accent). Default. |
| `Theme::Light` | Readable light palette with visible borders. |
| `Theme::Midnight` | Near-black, OLED-friendly (Tokyo Night accent). |
| `Theme::Solarized` | Solarized Dark (Ethan Schoonover), warm teal surfaces. |
| `Theme::Monokai` | Monokai Pro, warm charcoal + neon accents. |

## Methods

```rust
use dear_imgui_custom_mod::theme::Theme;

let t = Theme::Solarized;
let tb    = t.titlebar();   // TitlebarColors
let nv    = t.nav();        // NavColors
let dlg   = t.dialog();     // DialogColors
let stbar = t.statusbar();  // StatusBarConfig (palette + default geometry)

// Apply the full Dear ImGui widget palette + rounding/sizing.
// Call once at startup and again after every theme switch.
// let mut style = ui.clone_style();
// t.apply_imgui_style(&mut style);
```

| Method | Return | Purpose |
|--------|--------|---------|
| `.titlebar()` | `TitlebarColors` | Palette for `borderless_window` |
| `.nav()` | `NavColors` | Palette for `nav_panel` |
| `.dialog()` | `DialogColors` | Palette for `confirm_dialog` |
| `.statusbar()` | `StatusBarConfig` | Colours + default geometry for `status_bar` |
| `.apply_imgui_style(&mut Style)` | — | Writes rounding/sizing + full widget colours |
| `.next()` | `Theme` | Cycle to the next theme in `Theme::ALL` |
| `.display_name()` | `&'static str` | Human-readable name for menus |
| `Theme::ALL` | `&'static [Theme]` | Ordered array for Settings combo boxes |

## Per-instance palette override

Every themed component (`BorderlessConfig`, `NavPanelConfig`, `DialogConfig`)
exposes two fields:

- `theme: Theme` — the built-in selector, resolved at render time.
- `colors_override: Option<Box<*Colors>>` — when set, bypasses `theme` for
  this instance only.

The builder methods that drive them:

- `.with_theme(Theme)` replaces the theme and clears any override.
- `.with_colors(*Colors)` installs a custom palette override. The next
  `.with_theme(...)` call resets it back to the built-in theme.

```rust
use dear_imgui_custom_mod::borderless_window::{BorderlessConfig, TitlebarColors};
use dear_imgui_custom_mod::theme::Theme;

// Custom palette that bypasses the built-in themes.
let colors = TitlebarColors {
    bg:                 [0.10, 0.10, 0.14, 1.0],
    title:              [0.90, 0.90, 0.90, 1.0],
    separator:          [0.20, 0.20, 0.26, 1.0],
    btn_minimize:       [1.00, 0.75, 0.00, 1.0],
    btn_maximize:       [0.30, 0.75, 1.00, 1.0],
    btn_close:          [1.00, 0.33, 0.33, 1.0],
    btn_hover_bg:       [0.22, 0.22, 0.32, 0.85],
    btn_close_hover_bg: [0.50, 0.10, 0.10, 0.90],
    icon:               [0.90, 0.90, 0.90, 1.0],
    bg_erase:           [0.10, 0.10, 0.14, 1.0],
    drag_hint:          [0.18, 0.18, 0.26, 0.30],
    bg_inactive:        [0.08, 0.08, 0.10, 1.0],
    title_inactive:     [0.45, 0.45, 0.50, 1.0],
};

let _cfg = BorderlessConfig::new("My App")
    .with_theme(Theme::Dark)        // baseline
    .with_colors(colors);           // override — bypasses Theme::Dark
```

## Legacy colour tokens

The module still exports semantic `pub const [f32; 4]` colour tokens for
callers that tint their own widgets against the Dark palette (used today by
`code_editor`, `file_manager`, etc.). Prefer `Theme` methods for new code.

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

A parallel `LIGHT_*` set mirrors the same tokens tuned for the Light theme.

## Architecture

```
theme/
  mod.rs       Theme enum, ALL, sub-palette resolvers, legacy color tokens
  dark.rs      NxT native dark stack (titlebar/nav/dialog/statusbar/style)
  light.rs     Readable light stack
  midnight.rs  Near-black OLED stack
  solarized.rs Solarized Dark stack
  monokai.rs   Monokai Pro stack
```

One theme = one file. Each per-theme module owns the full stack
(`titlebar_colors`, `nav_colors`, `dialog_colors`, `statusbar_config`,
`apply_imgui_style`) so visual changes for a palette stay contained.
