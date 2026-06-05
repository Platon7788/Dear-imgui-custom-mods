# `tab_control`

Modern tab controller for `dear-imgui-rs`. Pure tab strip + content area, inspired by DevExpress XtraTabControl with contemporary touches: pinned tabs, drag-and-drop reorder, hover thumbnail preview, smooth scroll, animated open/close, badges, status indicators (including a "dirty" flag for unsaved changes), keyboard shortcuts, single-pass hit-test, zero per-frame allocations.

```toml
[dependencies]
dear-imgui-custom-mod = { version = "0.10", features = ["tab_control"] }
```

## Quick start

```rust
use dear_imgui_custom_mod::tab_control::*;
use dear_imgui_rs::Ui;

struct MyTab { name: String }

impl TabItem for MyTab {
    fn title(&self) -> &str { &self.name }
    fn render_content(&mut self, ui: &Ui) {
        ui.text("Hello from tab content!");
    }
}

let mut tc: TabControl<MyTab> = TabControl::new("##my_tabs");
tc.add(MyTab { name: "First".into() });
tc.add(MyTab { name: "Second".into() });

// In your render loop:
if let Some(action) = tc.render(ui) {
    match action {
        TabAction::Closed(id)        => println!("closed {}", id),
        TabAction::Activated(id)     => println!("activated {}", id),
        TabAction::DoubleClicked(id) => println!("dblclick {}", id),
        TabAction::Reordered(id)     => println!("reordered {}", id),
        TabAction::AddRequested      => { tc.add(MyTab { name: "New".into() }); }
    }
}
```

## Architecture

```
src/tab_control/
├── mod.rs       — TabControl<T> struct + TabItem trait + construction + render entry
├── api.rs       — tab-management API (add/remove/clear/move_tab/get/set_active/iter)
├── config.rs    — TabControlConfig schema (values live in config.ron)
├── config.ron   — DDD config default values
├── colors.rs    — TabColors palette + Theme::tab_colors() synthesis
├── strings.rs   — TabStrings EN/RU catalogues + for_locale()
├── types.rs     — TabId, TabStatus, Badge, CloseGlyph, TabStyle, TabAction
├── layout.rs    — pixel-width math, layout constants, pinned-partition repair
├── render/      — per-frame renderer (split by concern, every file < 500 lines)
│   ├── mod.rs      — entry point (render_tab_control), animation tick, shared types
│   ├── strip.rs    — tab-strip driver (layout + draw dispatch + event dispatch)
│   ├── hittest.rs  — single-pass hit-test (fill_hit_scratch) + scroll_into_view
│   ├── body.rs     — empty-state placeholder + active-tab body frame
│   ├── events.rs   — click / middle-click / right-click / hover / preview
│   ├── drag.rs     — drag-and-drop reorder (group-clamped swap + ghost tab)
│   ├── keyboard.rs — focus-gated keyboard navigation
│   ├── buttons.rs  — scroll arrows / overflow dropdown / add button / close modal
│   └── draw.rs     — tab styles (pill/underline/square) + content + close glyph
└── tests/       — deterministic unit tests (logic only — no ImGui FFI)
    ├── mod.rs       — Spy fixture + helpers
    ├── lifecycle.rs — add/remove/clear/set_active hooks + id stability
    ├── pinned.rs    — pinned invariant + enforce_pinned_partition + move_tab
    ├── config.rs    — config/palette default pins + popup-id scoping
    ├── scroll.rs    — scroll_into_view regular-vs-pinned offset math
    └── i18n.rs      — locale guard tests (resolve / default / ron round-trip)
```

The renderer uses free functions sharing `pc.hit_scratch`; the `render/`
sub-modules cross-call each other via `super::` paths (`pub(super)` for
sibling-visible helpers). `TabControl`'s public tab-management surface lives in
`api.rs` as a second `impl` block.

## `TabItem` trait

Implement on your data type to define each tab. Two methods are required (`title`, `render_content`); everything else has sensible defaults.

| Method | Default | Purpose |
|--------|---------|---------|
| `title(&self) -> &str` | — | tab label |
| `icon(&self) -> Option<&str>` | `None` | MDI glyph; ignored unless `cfg.icons_available = true` |
| `badge(&self) -> Option<Badge>` | `None` | small pill after title (e.g. unread count) |
| `status(&self) -> TabStatus` | `Active` | dot color: Active / Inactive / Warning / Error / Dirty / None |
| `tooltip(&self) -> Option<&str>` | `None` | classic tooltip on hover |
| `tab_color(&self) -> Option<[u8; 3]>` | `None` | accent override (active tab border / drag ghost) |
| `dot_color(&self) -> Option<[u8; 3]>` | `None` | per-tab status dot override (wins over status palette) |
| `text_color(&self) -> Option<[u8; 3]>` | `None` | per-tab title text override — applies in both active and inactive states; honoured by drag ghost, regular tabs, and pinned compact glyph/letter |
| `is_closable(&self) -> bool` | `true` | enable close button |
| `is_pinned(&self) -> bool` | `false` | pin to compact, non-scrolling left strip |
| `show_preview(&self) -> bool` | `true` | per-tab opt-out for hover preview |
| `on_activated(&mut self)` | no-op | lifecycle hook |
| `on_deactivated(&mut self)` | no-op | lifecycle hook |
| `render_content(&mut self, ui)` | — | draw the tab body |
| `render_preview(&mut self, ui)` | calls `render_content` | hover-preview body (live thumbnail by default) |

## `TabControl<T>` API

```rust
// Construction
TabControl::new(id)
TabControl::with_config(id, TabControlConfig { … })

// Tab management
tc.add(item) -> TabId       // pinned items go into the pinned prefix
tc.remove(id) -> Option<T>
tc.clear()
tc.move_tab(from, to) -> bool   // clamped to source's group (pinned ↔ regular)

// Access
tc.get(id) / tc.get_mut(id)
tc.iter() / tc.iter_mut()
tc.tab_count() / tc.is_empty()

// Active tab
tc.active_id() -> Option<TabId>
tc.set_active(id)

// Layout / scroll
tc.force_invalidate()       // call after mutating cfg.tab_min_width etc.
tc.scroll_to_active()       // deferred to next render

// Render
tc.render(ui) -> Option<TabAction>

// State surface for context menus (read after render):
tc.pending_close: Option<TabId>   // tab pending close confirmation
tc.context_tab:   Option<TabId>   // last right-clicked tab (caller resets)
tc.open_context_menu: bool        // true for one frame after right-click
```

## Visual styles

```rust
TabStyle::Pill        // (default) fully rounded
TabStyle::Underline   // flat with bottom accent bar (Material)
TabStyle::Square      // rectangular with small top rounding
```

Switch live: `tc.config.tab_style = TabStyle::Underline;`.

## Status indicators

```rust
TabStatus::Active     // green dot (default)
TabStatus::Inactive   // muted dot (idle/disconnected)
TabStatus::Warning    // amber dot, pulsing
TabStatus::Error      // red dot, pulsing
TabStatus::Dirty      // cyan circle in the close-button slot ("unsaved changes")
TabStatus::None       // no dot (per-tab opt-out)
```

**Dot can be disabled three ways:**
1. Globally: `cfg.show_status_dot = false`.
2. Per-tab: `fn status() -> TabStatus { TabStatus::None }`.
3. Globally hide icon font fallbacks: `cfg.icons_available = false` keeps icons silent.

**Dot colour customization:**
- Edit the palette: `cfg.colors.status_active = [r, g, b]`.
- Per-tab override: `fn dot_color() -> Option<[u8; 3]> { Some([r, g, b]) }`.

**Layout-jump caveat:** `Active ↔ Dirty` keeps a stable layout (the dot slot is reserved in both states). However `None ↔ Dirty` shifts the tab content by ≈11 px on toggle. Prefer `Inactive ↔ Dirty` if you want the slot to remain reserved.

## Pinned tabs

```rust
fn is_pinned(&self) -> bool { true }
```

- Live in a compact, non-scrolling strip on the left.
- Width = `cfg.pinned_tab_width` (default 36 px).
- Show the icon (or first letter of title as fallback when `icons_available = false`).
- Never display a close button — close them via `tc.remove(id)`.
- Drag-reorder is restricted to the pinned group; cannot escape into the regular strip.
- A thin vertical separator (`PINNED_SEPARATOR_W = 8 px`) is drawn between pinned and regular zones.

The component maintains a **pinned-prefix invariant** automatically: `add()` inserts new pinned tabs after the last existing pinned tab; `move_tab` clamps `to` into the source's group; `enforce_pinned_partition()` runs every frame to absorb runtime `is_pinned()` flips (O(n) early exit when already partitioned, O(n) in-place rotate-right repair otherwise — no allocations).

## Hover preview (Windows-taskbar-peek style)

```rust
let cfg = TabControlConfig {
    preview_hover_ms: Some(450),    // 450 ms hover -> popup
    preview_size: [370.0, 250.0],   // tooltip width is fixed at preview_size[0]
    preview_font_scale: 0.85,       // shrink fonts so more content fits
    ..Default::default()
};
```

Implementation: when an *inactive* tab has been hovered for `preview_hover_ms`, the controller opens an ImGui tooltip of fixed width and renders the tab's content into it. By default this is a **live thumbnail** — `render_preview` calls `render_content` recursively. The tooltip auto-grows vertically to fit content, never shows a scrollbar (locked-width + auto-height via `igSetNextWindowSizeConstraints`).

Override `render_preview` for a cheaper or differently-shaped preview when re-rendering full content is too expensive:

```rust
impl TabItem for ExpensiveTab {
    fn render_preview(&mut self, ui: &Ui) {
        // Cheap textual peek instead of full re-render.
        ui.text(format!("{} ({} items)", self.title, self.count));
    }
    // …
}
```

Or opt-out per-tab: `fn show_preview(&self) -> bool { false }`.

The preview is **never shown during drag** and **never for the active tab** (no point peeking content already visible).

## Hover-activate (Edge / Win11 style)

```rust
cfg.hover_activate_ms = Some(350);
```

After `350 ms` of hovering an inactive tab, it auto-activates without a click. `None` disables (default).

## Keyboard shortcuts

Gated by `cfg.keyboard_nav` (default `true`) and window focus.

| Combo | Action |
|-------|--------|
| `Left` / `Right` | Step prev / next (no wrap) |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Cycle (with wraparound, browser-style) |
| `Ctrl+1..8` | Jump to N-th tab |
| `Ctrl+9` | Jump to last tab (Chrome convention) |
| `Ctrl+T` | Emit `TabAction::AddRequested` |
| `Ctrl+W` | Close active tab (with confirmation if `confirm_close = true`) |

## Close button glyph

```rust
cfg.close_glyph = CloseGlyph::SquareX;
```

| Variant | Look |
|---------|------|
| `Cross` (default) | thin diagonal X |
| `CrossBold` | thicker X for busy backgrounds |
| `SquareX` | X inside a thin rounded square |
| `CircleX` | X inside a circle |

All variants render through the draw list (no font glyph required), so they work regardless of `icons_available`.

**Dirty indicator:** when `status() == TabStatus::Dirty` and the tab is **not** hovered, the close-button slot shows a filled cyan circle instead of the X. On hover the X reappears so the tab can still be closed; the close confirmation popup then uses the more urgent `strings.close_confirm_dirty` text.

## Drag-and-drop reorder

Enabled by default (`cfg.draggable = true`). Drag a tab to swap with its neighbour; restricted to its own group (pinned ↔ pinned, regular ↔ regular).

A vertical accent line tracks the cursor; a translucent ghost of the dragged tab follows the mouse. The actual swap happens one neighbour at a time as the cursor crosses tab midpoints, so reordering is animated naturally.

## Overflow dropdown

When the regular tabs don't fit, a `…` button appears on the right and opens a popup listing all tabs. Click an entry to activate that tab; the popup auto-closes.

## Body frame model

The active tab's `render_content()` runs inside a two-rectangle visible frame:

```text
[Tab strip]
|-------------------------------------|   <- outer rect (frame_bg)
||-----------------------------------||   <- inner child_window (body_bg)
||                                   ||      gap = frame_bg
||      widgets clipped here         ||      inset by `body_inset`
|-------------------------------------|
```

- **Outer rect**: filled with `colors.frame_bg` directly on the parent's draw list. Default mirrors `colors.strip_bg` so the strip + frame read as one chrome surface.
- **Inner child_window**: borderless, filled with `colors.body_bg` via `ChildBg` push. Holds the user widgets and clips them to the inner rect (text wrap, button max width, etc. respect the frame).
- **Inset**: `body_inset` ([horizontal, vertical] in pixels) controls the visible gap between outer and inner rect. Default `[4.0, 4.0]`.

Inside `render_content()` `ui.content_region_avail()` already reflects the inset — host widgets need no manual offset.

```rust
// More breathing room (e.g. for forms / property panels):
tc.config.body_inset = [8.0, 8.0];

// Full-bleed (charts, hex dumps, anything that wants every pixel):
tc.config.body_inset_enabled = false;

// Recolour the gap independently of the strip:
tc.config.colors.frame_bg = [0x18, 0x1c, 0x24];
```

A defensive guard short-circuits to plain `render_content()` when the inner rectangle would degenerate (window narrower than `2 * pad`, etc.) — ImGui's `BeginChild` panics on Windows for `0`-or-negative sizes, so the fallback keeps the host alive.

Stacks cleanly with `external_content: true` — disable both if your host renders content outside `tc.render(ui)` and applies its own padding.

### Body background color

The inner child-window's `ChildBg` is driven from `colors.body_bg`. Default is **slightly lighter** than `colors.strip_bg` so the body reads as a distinct surface and the inset gap registers as a visible frame around it (see `tab_control::tests::config::body_bg_default_differs_from_strip_bg_for_visible_frame`).

```rust
// Distinct body surface — strip stays nav.bg, body goes white.
tc.config.colors.body_bg = [0xFC, 0xFC, 0xFA];
```

### Active-pane border (opt-in)

Off by default; enables an outlined rectangle drawn over the outer fill so the host gets an "active pane" highlight matching the strip's selected-tab hue (IDE-style):

```rust
tc.config.body_inset_border = true;
tc.config.body_inset_border_thickness = 1.5;     // Default
tc.config.colors.frame_border = [0xff, 0x80, 0x10];   // Default mirrors `accent`
```

## Theme integration

`TabControl` plugs into the crate-wide `Theme` system via the
`Theme::tab_colors()` accessor — the tab strip stays in the same
visual ecosystem as `nav_panel` / `status_bar` (same surfaces,
hover/active lifts, status indicator hues):

```rust
use dear_imgui_custom_mod::theme::Theme;

let theme = Theme::Dark;
tc.config.colors = theme.tab_colors();   // synthesised from theme.nav() + statusbar_colors()
```

Per-tab overrides (`tab_color`, `dot_color`, `text_color`) win over the palette, so colored-by-domain tabs keep their hue across theme changes.

## Configuration cheat sheet

```rust
TabControlConfig {
    // Behavior
    closable:            true,
    confirm_close:       true,
    middle_click_close:  true,
    scroll_with_wheel:   true,
    keyboard_nav:        true,
    show_add_button:     false,   // shows a "+" at the right
    context_menu:        true,    // right-click → context_tab + open_context_menu
    external_content:    false,   // skip render_content (caller draws content)
    body_inset_enabled: true,  // wrap render_content in a borderless child + visible frame
    body_inset:    [4.0, 4.0], // [horizontal, vertical] inset; default 4 px visible gap
    body_inset_border: false,  // opt-in outlined rect over outer rect (active-pane cue)
    body_inset_border_thickness: 1.5,
    draggable:           true,
    show_overflow_dropdown: true,

    // Optional features
    icons_available:    false,    // set true after registering MDI font
    hover_activate_ms:  None,     // Some(ms) for Edge-style auto-switch
    preview_hover_ms:   None,     // Some(ms) for Windows-peek thumbnail
    preview_size:       [370.0, 250.0],
    preview_font_scale: 0.85,
    close_glyph:        CloseGlyph::Cross,
    pinned_tab_width:   36.0,
    show_status_dot:    true,

    // Layout
    tab_style:          TabStyle::Pill,
    show_tab_underline: true,
    tab_height:         26.0,
    tab_rounding:       6.0,
    tab_padding_h:      10.0,
    tab_gap:            2.0,
    tab_min_width:      80.0,
    tab_max_width:      320.0,
    close_btn_size:     12.0,
    close_btn_gap:      6.0,
    strip_padding_v:    4.0,
    scroll_btn_width:   24.0,
    scroll_speed:       220.0,
    smooth_scroll:      true,
    animate_open:       true,
    animate_close:      true,
    show_empty_placeholder: true,

    // Theming
    colors:  TabColors::default(),
    strings: TabStrings::default(),
}
```

After mutating layout-affecting fields (`tab_min_width`, `tab_max_width`, `tab_padding_h`, `pinned_tab_width`, `close_btn_size`, `close_btn_gap`, `icons_available`, or anything that changes `compute_tab_width`), call `tc.force_invalidate()` to rebuild the width cache on the next frame. All other fields take effect immediately.

## Nested controllers

Because `TabControl<T>` is fully generic, nesting is trivial — your `TabItem` simply holds another `TabControl` and forwards `render_content`:

```rust
struct OuterTab { inner: TabControl<InnerTab> }

impl TabItem for OuterTab {
    fn title(&self) -> &str { "Outer" }
    fn render_content(&mut self, ui: &Ui) {
        self.inner.render(ui);
    }
}
```

Each `TabControl` has its own ImGui-scoped popup IDs, scratch buffers, and animation state, so multiple instances coexist safely.

## Performance notes

- **Single-pass hit-test.** A pre-pass fills `hit_scratch` with `(idx, x0, x1, tw, hovered, close_hit)`; both drawing and event handling read from the same buffer — no duplicate geometry.
- **Cached layout.** `tab_widths_cache` is invalidated only when something layout-relevant changes (add / remove / drag swap / `force_invalidate`). The open animation does **not** invalidate the cache (it multiplies width into the scratch buffer, leaving base widths intact).
- **Cached popup IDs.** `close_popup_id` and `overflow_popup_id` are built once at `with_config()`. No `format!()` per frame.
- **In-place pinned partitioning.** `enforce_pinned_partition` is O(n) when already partitioned, in-place `rotate_right` repair when not — zero allocations.
- **Scratch buffers grow only.** `hit_scratch` and `tab_widths_cache` reach steady-state capacity early and never shrink.

## Demo

```bash
cargo run --example demo_tab_control --features "tab_control,app_window"
```

Showcases: pinned tabs (Home, Settings — compact left strip), nested TabControl, badges (Inbox unread count), pulsing error status (Diagnostics), per-tab preview opt-out (Diagnostics has none), live editor with dirty indicator (`readme.md`), keyboard shortcuts, drag-reorder, overflow dropdown, all three styles selectable from the Settings tab, close confirmation with stronger text for unsaved changes.

## Tests

```bash
cargo test --lib --features tab_control tab_control
```

64 unit tests cover deterministic state, split into themed files under
`tests/`: lifecycle (add/remove/clear/set_active hooks + id stability), pinned
(invariant + `enforce_pinned_partition` + `move_tab` clamping), config
(defaults / palette pins / popup-id scoping / `SMOOTH_SCROLL_COEF`), scroll
(`scroll_into_view` regular-vs-pinned offset math), and i18n (the four canonical
locale guard tests). The `scroll::*` tests hand-populate `tab_widths_cache` to
drive the scroll math without ImGui text measurement. Rendering itself is
verified manually via the demo — it requires an initialized ImGui context
that's hard to mock without spinning up a window.

## Configuration & localisation

`TabControlConfig` follows the project-wide DDD config pattern:
schema in `src/tab_control/config.rs`, default values in
`src/tab_control/config.ron`. See [`docs/config_pattern.md`](./config_pattern.md).

`TabStrings::en()` / `TabStrings::ru()` / `for_locale(Locale)` cover
the close-tab confirmation, dirty-tab confirmation, no-tabs / empty
hint placeholders, overflow tooltip, and add-tab button. Switch with
`TabControl::new(...).with_locale(Locale::Ru)` — the builder
auto-refreshes `config.strings`. Tab labels themselves come from the
host via `TabItem` and stay host-driven. See [`docs/i18n.md`](./i18n.md).
