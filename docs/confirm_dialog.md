# confirm_dialog

Reusable modal confirmation dialog component for Rust + Dear ImGui.

## Overview

`confirm_dialog` provides a fully styled, theme-aware confirmation dialog rendered via Dear ImGui. It replaces ad-hoc inline dialog code with a single function call. Icons are drawn as draw-list primitives (no icon font needed).

## Features

- **Theme-aware** via the unified [`Theme`](theme.md) enum + per-instance custom palette via `colors_override` / `with_colors`
- **4 icon types**: Warning (filled triangle), Error (circle + X), Info (circle + i), Question (circle + ?)
- **Fullscreen dim overlay** behind the dialog (toggleable)
- **Keyboard shortcuts**: Escape = cancel, Enter = confirm (toggleable)
- **Color-coded buttons**: green Cancel (safe), red Confirm (destructive)
- **Compact button layout**: centred, bottom-anchored, generous spacing
- **Accent border**: the dialog border tints to match the icon (orange for Warning,
  red for Error, blue for Info, purple for Question) — toggleable
- **Button glyphs**: small X / power / check glyphs drawn inside the buttons —
  toggleable
- **Builder-pattern configuration**: `DialogConfig::new(...).with_icon(...).with_theme(...)`
- **Font-independent**: all icons drawn as crisp draw-list primitives

## Quick Start

```rust
use dear_imgui_custom_mod::confirm_dialog::{
    DialogConfig, DialogIcon, DialogResult,
};
use dear_imgui_custom_mod::theme::Theme;

let cfg = DialogConfig::new("Close Application", "Are you sure you want to close?")
    .with_icon(DialogIcon::Warning)
    .with_confirm_label("Close")
    .with_cancel_label("Cancel")
    .with_theme(Theme::Dark);

let mut show = true;

// In render loop:
// match render_confirm_dialog(ui, &cfg, &mut show) {
//     DialogResult::Confirmed => { /* do the action */ }
//     DialogResult::Cancelled => { /* user cancelled */ }
//     DialogResult::Open      => { /* still showing */ }
// }
```

`DialogResult` is `#[must_use]` — callers must react to `Confirmed` /
`Cancelled` (the destructive action, or dismissing the dialog).

## Configuration

```rust
use dear_imgui_custom_mod::confirm_dialog::{ConfirmStyle, DialogConfig, DialogIcon};
use dear_imgui_custom_mod::theme::Theme;

let _cfg = DialogConfig::new("Delete File", "This action cannot be undone.")
    .with_theme(Theme::Dark)                        // color theme
    .with_icon(DialogIcon::Error)                  // icon type
    .with_confirm_label("Delete")                  // red button text
    .with_cancel_label("Keep")                     // green button text
    .with_confirm_style(ConfirmStyle::Destructive) // red confirm button
    .with_width(380.0)                             // dialog width
    .with_height(170.0)                            // dialog height
    .with_rounding(8.0)                            // border radius
    .without_dim()                                 // no background overlay
    .without_keyboard();                           // no Esc/Enter shortcuts
```

## Themes

Themes come from the unified [`Theme`](theme.md) enum (see that page for the
current variant list). `Theme::Dark` is the default. For a one-off custom
palette that does not fit any built-in theme, use
`.with_colors(DialogColors)` — it takes priority over the `Theme` selector
for that instance.

## Icons

| Variant | Visual | Description |
|---------|--------|-------------|
| `DialogIcon::Warning` | Filled triangle with "!" | Destructive / caution actions |
| `DialogIcon::Error` | Circle with X | Error state |
| `DialogIcon::Info` | Circle with "i" | Informational |
| `DialogIcon::Question` | Circle with "?" | User choice |
| `DialogIcon::None` | (no icon) | Text only |

## Button Styles

| Style | Cancel Button | Confirm Button |
|-------|--------------|----------------|
| `ConfirmStyle::Destructive` | Green (safe) | Red (danger) |
| `ConfirmStyle::Normal` | Green (safe) | Green (neutral) |

## Localization

`confirm_dialog` integrates with [`crate::i18n`](i18n.md). The dialog's
own default title, message and button labels ship in English and Russian;
set the language with `with_locale` / `set_locale`:

```rust
use dear_imgui_custom_mod::confirm_dialog::DialogConfig;
use dear_imgui_custom_mod::i18n::Locale;

// Russian defaults: title "Подтверждение", message "Вы уверены?",
// confirm "Подтвердить", cancel "Отмена".
let cfg = DialogConfig::default().with_locale(Locale::Ru);
assert_eq!(cfg.resolved_confirm_label(), "Подтвердить");
```

### Precedence: host override wins over the localized default

The four text fields (`title`, `message`, `confirm_label`, `cancel_label`)
default to **empty** in `config.ron`. An empty field is a sentinel for
"no host override" — the render path then substitutes the localized
default for the active `locale`. Any **non-empty** value the host supplies
(via `DialogConfig::new(...)`, `with_confirm_label`, `with_cancel_label`,
or by setting the field directly) always wins and is rendered verbatim,
regardless of `locale`.

```rust
# use dear_imgui_custom_mod::confirm_dialog::DialogConfig;
# use dear_imgui_custom_mod::i18n::Locale;
// `new` sets title/message (host override); confirm/cancel left empty.
let cfg = DialogConfig::new("Quit?", "Discard changes?").with_locale(Locale::Ru);
assert_eq!(cfg.resolved_title(), "Quit?");              // host wins
assert_eq!(cfg.resolved_confirm_label(), "Подтвердить"); // localized default
```

The resolved values are exposed via `resolved_title()`,
`resolved_message()`, `resolved_confirm_label()` and
`resolved_cancel_label()` — these are exactly what `render_confirm_dialog`
draws. `locale` is `#[serde(default)]`, so it round-trips through ron and
older saved configs (without the field) load as English.

> **Note:** `Locale::Ru` requires the host to bake `GlyphRanges::Cyrillic`
> (or a superset) into the active font atlas — otherwise Cyrillic
> characters render as `?` placeholders.

## API Reference

### `render_confirm_dialog(ui, cfg, open) -> DialogResult`

Renders the dialog if `*open` is `true`. Sets `*open = false` on confirm or cancel.

Returns:
- `DialogResult::Confirmed` — user clicked confirm or pressed Enter
- `DialogResult::Cancelled` — user clicked cancel or pressed Escape
- `DialogResult::Open` — dialog still visible, no action

### `DialogConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `title` | `String` | `""` (→ localized `"Confirm"`) | Header text. Empty ⇒ localized default for `locale` |
| `message` | `String` | `""` (→ localized `"Are you sure?"`) | Body message. Empty ⇒ localized default |
| `confirm_label` | `String` | `""` (→ localized `"Confirm"`) | Confirm button text. Empty ⇒ localized default |
| `cancel_label` | `String` | `""` (→ localized `"Cancel"`) | Cancel button text. Empty ⇒ localized default |
| `icon` | `DialogIcon` | `Warning` | Icon type |
| `confirm_style` | `ConfirmStyle` | `Destructive` | Button color style |
| `theme` | `Theme` | `Dark` | Color theme |
| `colors_override` | `Option<DialogColors>` | `None` | Per-instance palette override (no longer boxed — `DialogColors` is small) |
| `width` | `f32` | `340.0` | Dialog width (px) |
| `height` | `f32` | `160.0` | Dialog height (px) |
| `padding` | `f32` | `16.0` | Inner padding (px) |
| `button_height` | `f32` | `27.0` | Base button height (px) |
| `button_width` | `f32` | `75.0` | Fixed button width; `0.0` auto-sizes both buttons to content |
| `button_gap` | `f32` | `60.0` | Pixel gap between buttons (literal — no implicit `× 1.6` multiplier) |
| `button_padding_x` | `f32` | `22.0` | Horizontal padding inside each button cell (auto-size only) |
| `button_icon_scale` | `f32` | `0.16` | In-button glyph radius as a fraction of button height |
| `header_icon_size` | `f32` | `16.0` | Header icon canvas radius |
| `button_bottom_factor` | `f32` | `0.35` | Button-row bottom margin as fraction of `padding` |
| `dim_background` | `bool` | `true` | Draw overlay behind dialog |
| `keyboard_shortcuts` | `bool` | `true` | Esc/Enter handling |
| `rounding` | `f32` | `6.0` | Border radius (px) |
| `border_thickness` | `f32` | `1.5` | Border line thickness (px) |
| `accent_border` | `bool` | `true` | Tint border with the icon color |
| `show_separator` | `bool` | `false` | Draw line between message and buttons |
| `show_button_icons` | `bool` | `true` | Draw X / power / check glyphs in buttons |
| `locale` | `Locale` | `En` | Language for the **default** title/message/labels (see [Localization](#localization)) |

## Render-loop integration

Call `render_confirm_dialog` once per frame while the dialog should be
visible. It owns the `open` flag — set it `true` to show the dialog, and the
function flips it back to `false` on confirm or cancel.

```rust,ignore
use dear_imgui_custom_mod::confirm_dialog::{
    DialogConfig, DialogIcon, DialogResult, render_confirm_dialog,
};
use dear_imgui_custom_mod::theme::Theme;
use dear_imgui_rs::Ui;

struct MyApp { show_confirm: bool }

impl MyApp {
    fn render(&mut self, ui: &Ui) {
        if self.show_confirm {
            let cfg = DialogConfig::new("Close", "Are you sure?")
                .with_icon(DialogIcon::Warning)
                .with_confirm_label("Close")
                .with_theme(Theme::Dark);

            if let DialogResult::Confirmed =
                render_confirm_dialog(ui, &cfg, &mut self.show_confirm)
            {
                // perform the confirmed action (e.g. request app exit)
            }
        }
    }
}
```
