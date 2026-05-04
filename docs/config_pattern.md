# Config pattern — schema in `config.rs`, values in `config.ron`

This crate aligns every widget's settings to a Domain-Driven Design
split: the **schema** (struct definition + serde derives) lives in
Rust source, and the **values** (numeric defaults, layout dimensions,
toggle flags) live in a sibling `.ron` file loaded at
`Default::default()` via `include_str!`.

Established across sessions 036–041; every config struct in the
crate now follows it. New widgets and edits to existing widgets
must too.

## Why

- Values are **data**, schema is **code** — DDD separation of concerns.
- Defaults are readable, diff-friendly, and editable without a
  recompile cycle in dev workflows.
- The same `Default::default()` round-trip flow that a widget uses
  internally also drives `ron::to_string(&cfg)` for save/restore on
  the host side.

## The pattern

```rust
// src/my_widget/config.rs ── SCHEMA only
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MyWidgetConfig {
    pub size: f32,
    pub enabled: bool,
    /// Runtime-only data — `#[serde(skip)]` because it's not a value.
    #[serde(skip, default)]
    pub callback: Option<MyCallback>,
}

impl Default for MyWidgetConfig {
    fn default() -> Self {
        ron::from_str(include_str!("config.ron"))
            .expect("built-in my_widget/config.ron is valid")
    }
}
```

```ron
// src/my_widget/config.ron ── VALUES only
(
    size: 12.0,
    enabled: true,
)
```

`callback` is intentionally absent from the ron — `#[serde(skip)]`
fields are populated from their type-default on parse.

## Composite sub-structs

When a config holds another struct that itself has a value-set
(numeric / bool defaults — not a single canonical variant), the
sub-struct gets its **own** `.ron`:

```
src/app_window/config/
├── default.ron          ── AppConfig defaults
├── titlebar_main.ron    ── TitlebarConfig defaults (full chrome)
├── titlebar_tool.ron    ── TitlebarConfig::tool() preset
├── titlebar_dialog.ron  ── TitlebarConfig::dialog() preset
└── buttons.ron          ── Buttons defaults
```

ron 0.8 has no `include` mechanism, so the parent ron must inline
the sub-struct's field set. **Drift between the inline copy and the
sub-struct's own ron is guarded by a test** that compares the two
field-by-field — see
`app_window::config::titlebar::tests::buttons_inline_in_titlebar_main_matches_canonical`
for the reference pattern.

## Adding a field to an existing config

1. Add the field to the struct in `config.rs` with `#[serde(default)]`
   so older saved ron files still parse.
2. Add the same key to `config.ron` with the new default value.
3. If the type is a composite sub-struct that already has its own
   `.ron`, inline the matching defaults in the parent ron and add the
   field to the drift-test.

## Things that stay in `.rs`

Four exception buckets — these are **not** values, so ron is wrong
for them:

1. **Atomic enum invariants** marked with `#[default]`:
   ```rust
   #[derive(Default)]
   enum WindowKind { #[default] Main, Splash, Tool, Dialog }
   ```
   The choice of canonical variant is part of the type's contract,
   not user-tunable configuration.

2. **Theme-derived palettes** computed from the `Theme` enum:
   ```rust
   impl Default for HexViewerColors {
       fn default() -> Self { Theme::Dark.hex_viewer_colors() }
   }
   ```
   Palettes are derived state. Externalising them would break the
   compile-time guarantee that every `Theme` variant has a complete
   palette and balloon the file count to *N* widgets × *M* themes.

3. **Identity-element constructors** (`T::default = T::new()`):
   ```rust
   impl Default for NodeStyle {
       fn default() -> Self { Self::new("") }   // all Option = None
   }
   ```
   This is `Vec::new()` semantics — an empty / identity instance, not
   a configuration default.

4. **Skip-field fallbacks** — anything `#[serde(skip)]` needs a
   type-default in code because it's not in the ron by definition
   (runtime byte buffers, function pointers, `Arc<...>`, etc.).

## Reference state at session 042

| Module | Root config.ron | Sub-struct rons |
|---|---|---|
| `app_window` | `default.ron` | `titlebar_main/tool/dialog.ron`, `buttons.ron` |
| `code_editor` | `config.ron` | `context_menu.ron` |
| `disasm_view` | `config.ron` | `column_widths.ron` |
| 15 other widgets | `config.ron` | — (no composite sub-structs needed) |

## Reading list

- **CLAUDE.md** — terse DDD rule for agents working on this repo.
- **CHANGELOG.md** entries for sessions 036, 039, 041 — original
  rollout transcripts of this pattern.
- **docs/i18n.md** — sister guide for adding `Locale` support to a
  new widget; `locale: Locale` lives on the same config that this
  pattern governs.
