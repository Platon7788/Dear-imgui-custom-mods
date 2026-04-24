# Changelog

## [Unreleased]

### Added — `RowStyle::selection_color` + `selection_text_color` (per-row selection override)
Callers can now override the selected-row tint on a per-row basis without
touching the table-wide `TableConfig::selection_color` / `selection_text_color`.
Two new `Option<[f32; 4]>` fields on `RowStyle`:

| Field | Used when |
|-------|-----------|
| `selection_color`      | Row is selected; overrides `TableConfig::selection_color`      |
| `selection_text_color` | Row is selected; overrides `TableConfig::selection_text_color` |

Both default to `None` → no behavior change for existing code. Works in both
`VirtualTable` and `VirtualTree` (tree shares `RowStyle` via re-export).

Priority when a row is selected:

1. `row_style().selection_text_color` (per-row override)
2. `TableConfig::selection_text_color` (table-wide)
3. `row_style().text_color` (fallback)

Resolution is runtime — the `row_style()` trait method is called each frame
for visible rows (ListClipper-virtualized, so cost stays O(visible)).

**Example — error rows keep their red identity when selected:**

```rust
impl VirtualTreeNode for MyNode {
    fn row_style(&self) -> Option<RowStyle> {
        match self.severity {
            Severity::Error => Some(RowStyle {
                text_color: Some([1.0, 0.55, 0.55, 1.0]),
                bg_color:   Some([0.40, 0.10, 0.10, 0.18]),
                // When selected, keep a dark-red tint instead of generic blue:
                selection_color: Some([0.60, 0.15, 0.15, 0.70]),
                selection_text_color: Some([1.0, 0.85, 0.85, 1.0]),
                ..Default::default()
            }),
            _ => None,
        }
    }
}
```

### Added — `virtual_table::TableConfig::flat_headers` (symmetry with `TreeConfig::flat_headers`)
- **`flat_headers: bool`** field on `TableConfig` (default `false`, no
  behavior change for existing users). When `true`, `render_header`
  wraps each `ui.table_header` call in a **per-column** style-color
  scope pushing `HeaderHovered` / `HeaderActive` to transparent —
  suppresses the default button-like hover/active tint on captions
  for informational (sort-disabled) tables.
- **Per-column scope** (not window-wide): the style guards drop at
  the close-brace before the next column renders, so row-selection
  highlight (which reuses the same style colors) stays intact. Same
  implementation as `virtual_tree::render_header`.
- **`proc_mon/ui.rs` simplified** — the previous window-wide manual
  `push_style_color(HeaderHovered/Active, transparent)` guard is gone
  (12 lines removed); `default_table_config()` now sets
  `flat_headers: true` + `sortable: false` and `VirtualTable` handles
  the rest per-column.
- **`demo_table.rs`** gains a `Flat Headers` checkbox alongside the
  existing `Sortable` toggle — pairs well for informational layouts.

### Changed — BREAKING: `proc_mon` reduced to minimal 5-field `ProcessInfo` (NxT parity)
Alignment with the `IMGUI_NXT` reference engine — the monitor is now
**list-only**, not a full Process Hacker clone. Everything that was
previously opt-in (memory, CPU%, threads, handles, I/O, priority, PPID,
session ID, etc.) has been removed from the module surface entirely.

**Migration:** any code that read `process.working_set`, called
`set_cpu_tracking(true)`, enabled `ColumnConfig { memory: true, .. }`,
or used `format_bytes` / `format_cpu_time` / `format_cpu_percent` /
`format_create_time` will not compile. Bring the needed logic into your
app directly (parse `SYSTEM_PROCESS_INFORMATION` or call `GetProcessMemoryInfo`
from the `windows-sys` crate) — or stay on the 0.9.x tag that included
the full `ProcessInfo`.

- **`ProcessInfo` fields: 19 → 5**. Kept: `pid`, `name`, `bits`, `status`,
  `create_time`. Removed: `ppid`, `session_id`, `priority`, `working_set`,
  `private_bytes`, `virtual_size`, `peak_working_set`, `kernel_time`,
  `user_time`, `cycle_time`, `thread_count`, `handle_count`,
  `io_read_bytes`, `io_write_bytes`, `cpu_percent`.
- **`ColumnConfig` fields: 15 → 2**. Kept: `bits`, `status` (both `true`
  by default). Removed all other toggles.
- **`ProcessMonitor` canonical column layout: 18 → 4** (`Name`, `PID`,
  `Bits`, `Status`). Hidden columns still use `.visible(false)` so
  `cell_display_text` indices remain stable.
- **Removed public items**:
  - `ProcessEnumerator::set_cpu_tracking`, `cpu_tracking`, `logical_cores`
  - Free functions `format_bytes`, `format_cpu_time`, `format_cpu_percent`,
    `format_create_time` (and their re-exports from `proc_mon::*`)
  - Internal helpers `SnapDiff`, `PrevState`, `filetime_now_100ns`
- **Delta detection simplified** — was field-by-field `SnapDiff: PartialEq`
  on 10 fields, now a single `ProcStatus` equality check per PID.
  Matches the NxT engine exactly.
- **Overhead parity**: a headless `ProcessEnumerator`-only user now has
  the same per-tick cost profile as the NxT engine task (one syscall +
  ~300 status compares). GUI cost unchanged from the previous minimum
  (still 30 FPS capped in the demo, 4 columns, no per-frame allocation).
- **Tests:** removed `test_format_bytes`, `test_format_cpu_time`,
  `test_format_cpu_percent`, `test_snapdiff_stable_for_static_fields`.
  Added `test_column_config_visible_count`. `test_monitor_colors_priority`
  updated for the new 5-field `ProcessInfo`. Total lib: **419 passing**,
  2 `#[ignore]` syscall tests.
- `docs/proc_mon.md` rewritten against the new surface (column table
  dropped from 18 rows to 4; CPU-tracking section removed).

### Fixed — `code_editor` hex auto-space double-insert on 2nd-nibble replace
- **Double-space bug fixed.** With `hex_auto_space = true`, editing
  the second nibble of an existing byte (e.g. `"AA "` → replace 2nd A with
  `B`) no longer inserts a duplicate space. Old code triggered auto-space
  because `line.chars().nth(col).is_none_or(|c| c == ' ' || c == '\t')`
  returned `true` both for EOL **and** for an already-existing separator.
- **Decision logic extracted** into `helpers::hex_auto_space_needed(line,
  col)` — a pure function testable in isolation. Insert rules:

  | Next char                      | Action       | Rationale                                |
  |--------------------------------|--------------|------------------------------------------|
  | `None` (EOL)                   | **insert**   | Fresh byte at end — common path          |
  | ASCII hex digit                | skip         | Don't silently merge two byte sequences  |
  | Whitespace (space / tab / NBSP)| skip         | Already a separator — don't duplicate    |
  | Other (`;` / `|` / `,` / …)    | **insert**   | Custom DSLs — keep byte visually distinct |

- Manually-typed spaces are **never** trimmed or modified — auto-space
  is insert-only. The whole mechanism lives on the text-input path and
  does not interact with cursor movement, Delete / Backspace, paste,
  undo, or multi-cursor insertion.
- 5 new unit tests in `code_editor::helpers::tests` covering each row of
  the decision matrix + the exact user-reported "replace 2nd nibble"
  scenario (total lib: **422 passing**, 2 `#[ignore]`).

### Added — `proc_mon` row highlighting (`MonitorColors`)
- **`MonitorColors`** struct — configurable palette for per-row tinting.
  Replaces the previously hard-coded `Suspended` amber. Four layers of
  resolution, first non-`None` wins: `by_pid > by_name > self_process
  > suspended`. Ships with `with_*` / `add_*` / `remove_*` / `clear_all`
  / `resolve` helpers.
- **`MonitorConfig::colors`** — palette is now part of the config.
  Default mirrors previous behavior (only `Suspended` tinted in amber).
- **`ProcessMonitor::colors()` / `colors_mut()` / `set_colors(colors)` /
  `refresh_colors()`** — read, mutate, or replace the palette at runtime.
  `set_colors` automatically re-resolves every tracked row; after using
  `colors_mut` callers invoke `refresh_colors` to apply in-place edits.
- **Self-process highlighting** — `MonitorColors::self_process`, matched
  against `std::process::id()` captured once in `ProcessMonitor::new`.
- **Per-name & per-PID maps** — case-insensitive `by_name` (names stored
  lowercased for O(1) lookup) and explicit `by_pid` overrides.
- **Zero-cost rendering** — color resolution runs once per upsert and
  is cached into `ProcessRow::color_override`. The render path is a
  single `Option<[f32;4]>` copy — no hashing, no `to_lowercase` allocs,
  no rule evaluation per frame. Status flips re-resolve via the delta.
- **`MonitorColors` re-exported** from `proc_mon::*`, serde-serializable
  so full palettes can be shipped as JSON / TOML / config files.
- New `test_monitor_colors_priority` unit test verifying the four-layer
  resolution order (6 passing tests total, 2 `#[ignore]`).
- `docs/proc_mon.md` gains a **Row highlighting** section with examples,
  priority table, and `MonitorColors` API reference.

### Added — `app_window` power-aware GPU selection
- **`PowerMode` enum** in `AppConfig` — `Auto` (default, discrete preferred),
  `LowPower` (iGPU preferred, saves battery on laptops), `HighPerformance`
  (refuses silent fallback to software / CPU renderers like WARP / llvmpipe).
  Accessed via `AppConfig::with_power_mode(PowerMode::..)`.
- **Cascaded adapter fallback chain** — `init_wgpu` now enumerates every
  surface-compatible adapter, scores them, sorts descending, and tries
  `request_device` on each in turn. If a buggy driver on the top-scored
  adapter fails `request_device` (rare but reproducible on old Intel HD
  with outdated drivers), the next candidate is tried instead of panicking.
- **Software-renderer warning** — explicit `eprintln!` when the selected
  adapter is `DeviceType::Cpu` (WARP / llvmpipe), so users understand why
  performance is degraded rather than filing a perf bug.
- **Per-adapter tracing** — every trial logs `"trying adapter … | backend
  … | type …"` with a final `"using adapter …"` or `"skip adapter …"`
  decision, making GPU-selection issues self-diagnosing.

### Added — `proc_mon` module (Windows only)
- **`proc_mon` module** — production-ready process monitor with direct
  NT-syscall enumeration and virtualized `dear-imgui` table view.
  Gated behind the `proc_mon` feature (on by default via `full`);
  requires `virtual_table` + `syscalls` + `serde`; Windows-only.
  - `ProcessEnumerator` — `NtQuerySystemInformation(SystemProcessInformation)`
    with a reusable syscall buffer capped at 64 MiB, bitness cache pruned
    every 15 ticks against the live PID list, stable sort by `CreateTime`
    descending.
  - `ProcessInfo` (19 fields): `pid`, `name`, `bits` (32/64), `ppid`,
    `session_id`, `status` (Running / Suspended via thread-state walk),
    `create_time`, `priority`, `working_set`, `private_bytes`,
    `virtual_size`, `peak_working_set`, `kernel_time`, `user_time`,
    `cycle_time`, `thread_count`, `handle_count`, `io_read_bytes`,
    `io_write_bytes`, `cpu_percent`.
  - **Zero-hash delta** — change detection uses direct field comparison
    on a 10-field `SnapDiff` struct with `PartialEq`, not `std::hash`.
    CPU counters (`kernel_time`, `user_time`, `cycle_time`) excluded
    from the diff so active processes don't spam upserts; memory / I/O
    moves are what actually drive updates.
  - **Optional CPU% tracking** — opt-in via
    `ProcessEnumerator::set_cpu_tracking(true)`. When disabled, the
    enumerator skips `SystemTime::now()`, per-process `HashMap` lookups,
    subtractions, and float math — matching the overhead of a list-only
    monitor like `IMGUI_NXT`'s engine task. CPU% is normalized across
    logical cores: `Δ(kernel+user) / (Δwall × cores) × 100`, clamped
    `[0, 100]`. Toggling resets the baseline automatically.
  - **`foldhash` everywhere** — every `HashMap<u32, _>` uses
    `foldhash::fast::FixedState` (~5× faster than `std`'s SipHash on
    `u32` keys). Same pattern used by `virtual_table` / `virtual_tree`.
  - `ProcessMonitor` UI widget — canonical 18-column layout with stable
    indices regardless of visibility (hidden columns registered with
    `.visible(false)` rather than omitted). Process Name uses
    `.stretch(1.0)`; PID / Bits / Status are fixed-width and pinned to
    the right edge. In-place `ProcessRow` mutation on upsert for known
    PIDs — volatile columns (Memory, I/O, CPU%, CPU time) re-formatted
    via `update_volatile()`, immutable columns (name, create_time)
    stay cached from the initial insert.
  - `ColumnConfig::default()` = Name / PID / Bits / Status (minimal, like
    NxT reference UI). `MonitorConfig::minimal()` / `all_columns()`
    helpers for common presets.
  - Header hover / active highlights suppressed via
    `push_style_color(HeaderHovered/Active, transparent)` inside
    `ProcessMonitor::render` — headers are informative-only since sort
    is fixed.
  - Context-menu routing via `MonitorEvent::ContextMenuRequested(pid)` —
    the widget clears the flag and the caller renders their own popup
    with arbitrary actions (Kill / Copy PID / Details / …).
  - Case-insensitive search across name + PID using a pre-lowercased
    query and a reusable PID-scratch buffer (no `io::Cursor`, no
    per-frame allocation on search hot path).
  - `format_bytes`, `format_cpu_time`, `format_cpu_percent`,
    `format_create_time` helpers — all take `&mut String` for zero-alloc
    formatting into caller-owned buffers.
  - 5 unit tests: `test_format_bytes`, `test_format_cpu_time`,
    `test_format_cpu_percent`, `test_column_config_default`,
    `test_snapdiff_stable_for_static_fields`. Two syscall-hitting tests
    (`test_enumerate_processes`, `test_delta_update`) marked `#[ignore]`
    because they require live NT stubs — run with `cargo test -- --ignored`.
- **`docs/proc_mon.md`** — full component reference (features, quick
  start, configuration, column table, architecture diagram, performance
  notes, API reference, platform support, safety).
- **`examples/demo_proc_mon.rs`** — complete end-to-end app with live
  monitor, search bar, caller-drawn context menu (Copy PID / Details /
  Kill — styled green/red like other demos), status line, manual refresh
  button. Render loop caps at ~30 FPS via `ControlFlow::WaitUntil`.

### Changed — Build profiles
- **`[profile.dev.package."*"] opt-level = 2`** — all dependency crates
  (wgpu, imgui, winit, serde, syscalls, …) now build with near-release
  optimization in debug, keeping our own code at `opt-level = 0` for fast
  iterative compiles and full `debug_assertions`. Render hot paths (wgpu /
  imgui) no longer run as the pathologically-slow debug builds — essential
  for GUI apps where a debug-compiled wgpu is ~10–30× slower than release.
- **`[profile.release]` tightened** to `lto = "fat"`, `codegen-units = 1`,
  `strip = "symbols"`, `panic = "abort"` — matches the aggressive profile
  used by the `IMGUI_NXT` reference engine. Cross-crate inlining, no unwind
  machinery, no PDB data. Release binary for `demo_proc_mon` dropped from
  8.9 MB to 6.5 MB (−27%).

### Changed — MSRV
- **MSRV bumped from 1.94 → 1.95.** Pins updated in `rust-toolchain.toml`,
  `Cargo.toml (rust-version)`, `clippy.toml (msrv)`, and the
  `msrv (rust 1.95)` CI job. Users on the prior stable channel must
  `rustup update stable` before building. Rust 1.95 brings `cfg_select!`,
  `if-let` guards in `match`, `core::hint::cold_path()`,
  `Atomic{Ptr,Bool,Isize,Usize}::update()`, `Vec::push_mut()`, and
  const-stable `fmt::from_fn()` — available for use in future work.
- **5 `clippy::collapsible_match` findings** resolved by collapsing
  `match + if` into pattern guards (`force_graph::mod`, `node_graph::render::input`
  x2, `toolbar::mod` x2). Clippy 1.95 promoted the lint; same semantics,
  more idiomatic.

### Added
- **`notifications` module** — modern toast-notification center with
  `NotificationCenter` holding the live stack between frames.
  Gated behind the `notifications` feature (on by default via `full`).
  - 5 severity levels (`Info`, `Success`, `Warning`, `Error`, `Debug`),
    each with a dedicated draw-list icon (font-independent, matches the
    `confirm_dialog` approach).
  - 6 stack placements: 4 corners + `TopCenter` / `BottomCenter`.
    Stack newest-at-anchor; older toasts push outward. Margin and
    inter-toast spacing are configurable — callers with custom titlebars
    raise `margin[1]` to clear the chrome (the center uses
    `io.display_size()` and does not know about host windows).
  - Auto-dismiss via `Duration::Timed(secs)` with an optional bottom
    progress bar; `Duration::Sticky` for user-closed toasts.
  - Pause-on-hover so long bodies stay readable.
  - `AnimationKind::{Fade, SlideIn, None}` with configurable duration.
  - Action buttons with caller-defined ids surfaced via
    `NotificationEvent::ActionClicked { id, action_id }`.
  - Manual `×` close (`NotificationEvent::Dismissed`) and body-click
    (`NotificationEvent::Clicked`).
  - Per-toast `with_custom_color([r,g,b,a])` accent override on top of the
    severity default.
  - `max_visible` cap with graceful overflow fade-out.
  - 5 built-in palettes (`NotificationColors::dark/light/midnight/
    solarized/monokai`) wired through `Theme::notifications()`; custom
    palettes via `CenterConfig::with_colors`.
  - 5-pass render pipeline: advance animations → layout stack
    (pre-measured heights, single pass) → draw toasts → tick timers
    (paused while hovered) → reap finished notifications.
  - 8 unit tests covering id uniqueness, dismiss flags, builder chain,
    severity labels, and placement orientation helpers.
- **`docs/notifications.md`** — full component reference (features,
  quick start, configuration, API reference, `app_window` integration).
- **`examples/demo_app_window.rs`** — rewritten to showcase `notifications`
  end-to-end alongside the existing `AppWindow` / theme / confirm-dialog
  demo. Buttons for every severity, sticky / custom-color / actions /
  burst / dismiss-all, live `Placement` / `AnimationKind` combos, sliders
  for `max_visible` + `pause_on_hover`, counter and theme changes push
  toasts of their own, events mirrored to the event log.

### Fixed
- **`virtual_table` / `virtual_tree` — last rows unreachable via manual
  scroll inside tightly-sized containers** (reproduced in NxT
  `packet_monitor` on a `child_window [300, 300]` hosting 500 rows).
  `ListClipper::items_height` was set to the bare `row_h`, but ImGui's
  table adds `2 * CellPadding.y` around every row (`TableBeginCell` cursor
  offset + `TableEndCell` RowPosY2 expansion; see `imgui_tables.cpp:1915,
  2188, 2247`). The clipper's final `SeekCursorForItem(ItemsCount)`
  therefore understated the inner scroll-window's content size by
  `row_count * 2*CellPadding.y`, so `scroll_max_y` clamped before the
  last rows and they could not be revealed by dragging the scrollbar.
  This also affected `render_external` / `render_slice` /
  `render_lookup` paths and the `snap_last_row` quantisation formula.
  Matches the upstream hint at `imgui.cpp:3319`.
- **`virtual_table` — `snap_last_row` quantisation now uses the true
  row stride**, so the quantised outer height actually matches a whole
  number of rendered rows (previously it quantised by `row_h` and
  left a fractional row below the fold).

### Added
- **`virtual_table::row_height_to_stride(row_h, cell_padding_y)`**
  `pub(crate)` helper with the ImGui-reference derivation in its doc
  comment, re-used by `virtual_tree`. New `snap_outer_height(avail_h,
  header_h, row_stride)` helper for the quantisation path. Seven new
  unit tests in `virtual_table::layout_tests`.
- **`StatusBarConfig::highlight_hover: bool`** (default `false`).
  When off, the bar paints no hover background at all — the panel stays
  fully static visually. Clickable items still emit
  `StatusBarEvent`s and tooltips still fire regardless of the flag.
  Set to `true` to restore the pre-0.8.1 Windows-style hover/active
  feedback. All five bundled themes (`Dark`, `Light`, `Midnight`,
  `Monokai`, `Solarized`) default the flag to `false`.

### Tests
- `388` → `396` library tests (7 layout tests for `virtual_table`,
  `config_defaults` hover assertion and a theme-preset sweep for the
  new `StatusBar` flag). All green, `cargo clippy -D warnings` stays
  clean.

## [0.8.0] — 2026-04-17 — BREAKING

### Changed
- **Unified theme system.** Dropped per-component theme enums
  (`TitlebarTheme`, `NavTheme`, `DialogTheme`) in favor of a single
  crate-wide `theme::Theme` (Dark / Light / Midnight / Solarized / Monokai).
  Each variant owns the full stack via its per-theme module
  (`theme::{dark,light,midnight,solarized,monokai}`) and exposes
  `.titlebar()`, `.nav()`, `.dialog()`, `.statusbar()`,
  `.apply_imgui_style()`, `.next()`, `Theme::ALL`.
- **Config shape.** `BorderlessConfig`, `NavPanelConfig`, `DialogConfig`
  now each carry `theme: Theme` + optional
  `colors_override: Option<Box<*Colors>>` for custom palettes, plus a
  `pub(crate) fn resolved_colors()` that resolves override vs theme default.
- **Theme files are palette-only.** `src/borderless_window/theme.rs` /
  `src/nav_panel/theme.rs` / `src/confirm_dialog/theme.rs` shrunk to just
  the `TitlebarColors` / `NavColors` / `DialogColors` structs — no enum,
  no `From<&OtherEnum>` adapters, no per-module luminance helpers.
- **`app_window::style::apply_imgui_style_for_theme`** is now a thin
  wrapper over `Theme::apply_imgui_style`.
- **Demos** (`demo_app_window`, `demo_borderless`, `demo_nav_panel`)
  migrated — identity conversions collapsed into `*t` where the orphan
  rule previously forced helper functions.

### Added
- **`borderless_window::render_titlebar_overlay`** (added earlier in the
  0.7 series) — renders through `ui.get_foreground_draw_list()` at an
  explicit screen origin without a host window; content clicks pass
  through instead of being swallowed.
- **`nav_panel::render_nav_panel_overlay(ui, cfg, state, origin, size)`**
  — overlay variant matching the titlebar pattern. Panel draws on the
  foreground draw list; the submenu flyout still opens as a dedicated
  ImGui window (it needs input focus).
- **`StatusBar::render_overlay(ui, origin, size)`** — same overlay
  pattern for the status bar. Hover detection uses position-only checks
  in overlay mode (skips `is_window_hovered()`).

### Refactored
- `status_bar::render` internals extracted as
  `render_impl(origin, size, draw, use_window_hovered)`; `render()` is
  now a thin wrapper computing origin/size from the current window and
  calling impl with the legacy flag.
- `nav_panel::render_nav_panel` body extracted as
  `render_nav_panel_impl(origin, size, use_foreground)`; same
  wrapping pattern.

### Fixed
- `nav_panel` hidden-tab branch dropped a redundant
  `cfg.resolved_colors()` call — the outer `colors` is still in scope.
- `#[must_use]` added to `TitlebarResult`, `NavPanelResult`, and
  `DialogResult` so silently dropping user-action output becomes a
  compile warning.

### Clippy
- Cleared `needless_borrows_for_generic_args` (4× in
  `borderless_window/mod.rs`) and `clone_on_copy` (5× across demos)
  surfaced by the new `Theme: Copy` impl.

### Migration guide (0.7.x → 0.8.0)

```diff
- use dear_imgui_custom_mod::borderless_window::TitlebarTheme;
+ use dear_imgui_custom_mod::theme::Theme;

- .with_theme(TitlebarTheme::Dark)
+ .with_theme(Theme::Dark)

- let palette = TitlebarTheme::Dark.colors();
+ let palette = Theme::Dark.titlebar();

- fn my_theme_bridge(t: AppTheme) -> NavTheme { /* identity match */ }
+ fn my_theme_bridge(t: AppTheme) -> Theme { /* identity match */ }
```

`TitlebarTheme::Custom(colors)` becomes `with_theme(Theme::*)` +
`with_colors(colors)` (configs preserve an override on top of a
semantic theme selection).

## [0.7.1] — 2026-04-17

### Changed
- **confirm_dialog** — Modernised visuals matching the user's reference mock-up
  - Border now tints to the icon color (orange for `Warning`, red for `Error`,
    blue for `Info`, purple for `Question`) — controlled by new
    `accent_border: bool` field (default `true`)
  - Border thickness configurable via new `border_thickness: f32` field
    (default `1.5`)
  - Horizontal separator between message and buttons is now opt-in
    (`show_separator: bool`, default `false`)
  - Cancel and Confirm buttons now render small draw-list glyphs:
    `×` on Cancel, `⏻` on destructive Confirm, `✓` on normal Confirm
    (toggle via new `show_button_icons: bool`, default `true`)
  - Buttons rendered via custom `InvisibleButton` + draw-list path so the
    glyphs sit correctly inside the button rect with proper hover/active states
  - Warning icon color shifted from amber-yellow to orange in the Dark and
    Midnight themes (closer to the user's mock-up)
  - New builder methods: `with_border_thickness`, `with_accent_border`,
    `with_separator`, `with_button_icons`
- **docs/confirm_dialog.md** — Updated feature list and config table

### Fixed
- **Clippy: 41 → 0 warnings** across 5 modules (Edition 2024 / Rust 1.94)
  - `disasm_view/mod.rs` — 17 `collapsible_if` collapsed into `&& let`-chains
    (multi-level nests merged into one chained `if`)
  - `disasm_view/config.rs` — `needless_range_loop` → `iter().enumerate().take()`
    (loop label `'depth:` preserved)
  - `hex_viewer/mod.rs` — `redundant_closure`, `manual_div_ceil` (verified
    arithmetic identity for all `len`), 2× `manual_is_multiple_of`,
    4× `collapsible_if`; click-handler also re-indented and an inner
    `if/else { if/else }` flattened to `if/else if/else`
  - `utils/export.rs` — 9× `manual_strip` → `strip_prefix`,
    1× `if_same_then_else` (NaN/Infinite branches merged via `||`)
  - `virtual_table/mod.rs`, `virtual_tree/mod.rs` — 4× `collapsible_if`
- All 370 unit tests still pass; no `#[allow(...)]` was added

## [0.7.0] — 2026-04-16

### Added
- **nav_panel** — Modern navigation panel (activity bar) component
  - 3 docking positions: Left, Right, Top (Bottom reserved for StatusBar)
  - Left/Right: vertical icon strip with active indicator bar
  - Top: horizontal bar with `IconOnly`, `IconWithLabel`, `LabelOnly` button styles
  - Flyout submenu on any button with icons, keyboard shortcut hints, separators
  - Auto-hide with slide animation + auto-show on cursor edge hover
  - Toggle arrow button (double chevron, direction-aware per dock position)
  - Badge (notification counter / dot) anchored to button top-right corner
  - Configurable `button_spacing` (gap between buttons, default 4px)
  - Optional `show_button_separators` (thin lines between buttons, default on)
  - Per-button tooltip control (`without_tooltip()`) + global `without_tooltips()`
  - Custom icon color per button via `with_color([r,g,b,a])`
  - 6 built-in color themes + `Custom(Box<NavColors>)` (16 color slots)
  - `content_offset_y` / `content_offset_x` for correct edge detection with borderless titlebar
  - Builder-pattern `NavPanelConfig` with 20+ builder methods
  - `NavPanelState`: active button, visibility, animation progress, submenu state
  - `NavPanelResult` with events + `occupied_size` for layout coordination
  - Restore tab (chevron arrow) when panel is hidden via toggle
  - 9 unit tests covering config, state, themes, buttons, submenus
  - Renders via parent window draw list (no extra ImGui window except submenu flyout)
  - DrawListMut scoped correctly to prevent `A DrawListMut is already in use` panic
- **demo_nav_panel** — Full interactive NavPanel + StatusBar integration demo
  - Config panel with all properties: position, dimensions, behavior flags, spacing, rounding
  - Live state display: visible, animation_progress, active button
  - Action buttons: Show/Hide, +Badge, Clear
  - StatusBar at bottom for layout compatibility testing
- **docs/nav_panel.md** — Full component documentation

### Changed
- **utils/color** — `pack_color_f32()` now used as shared `c32()` replacement in `nav_panel`
  (removes 1 of 5 inline duplicates)

## [0.6.1] — 2026-04-15

### Added
- **confirm_dialog** — Reusable modal confirmation dialog component
  - 6 built-in themes (Dark, Light, Midnight, Nord, Solarized, Monokai) + `Custom(DialogColors)`
  - 4 icon types drawn as draw-list primitives (Warning, Error, Info, Question)
  - Fullscreen dim overlay behind the dialog (toggleable)
  - Keyboard shortcuts: Escape = cancel, Enter = confirm (toggleable)
  - Color-coded buttons: green Cancel (safe), red Confirm (destructive)
  - Compact bottom-anchored button layout with generous spacing
  - `ConfirmStyle::Destructive` / `ConfirmStyle::Normal` button presets
  - Builder-pattern `DialogConfig` with 13 builder methods
  - `render_confirm_dialog(ui, cfg, open) -> DialogResult` — single-function API
  - 5 unit tests covering config, themes, builder chain, icon variants
- **borderless_window/platform** — `hwnd_of(window)` exported as public utility
- **app_window** — Re-exports `TitlebarTheme`, `BorderlessConfig`, `ButtonConfig`, `ExtraButton`, `CloseMode`, `TitleAlign` from `borderless_window` — users no longer need to import both modules
- **docs/confirm_dialog.md** — Full component documentation

### Changed
- **demo_app_window** — Close confirmation dialog replaced with `confirm_dialog` component (50 lines → 15 lines)
- **demo_borderless** — Close confirmation dialog replaced with `confirm_dialog` component (50 lines → 14 lines)

### Fixed
- **app_window/mod.rs** — Removed duplicate `hwnd_of()` function; now uses shared `borderless_window::platform::hwnd_of()`

## [0.6.0] — 2026-04-15

### Added
- **borderless_window** — Fully custom borderless titlebar rendered via Dear ImGui draw lists
  - 6 built-in themes: Dark, Light, Midnight, Nord, Solarized, Monokai + `Custom(TitlebarColors)`
  - Minimize / Maximize / Close buttons drawn as draw-list primitives (crisp at any DPI)
  - 8-direction edge resize detection — returns `ResizeEdge` every frame for cursor updates
  - `CloseMode::Confirm` — deferred close; call `TitlebarState::confirm_close()` from your dialog
  - Custom extra buttons (`ExtraButton`) rendered left of the standard window-control buttons
  - `TitleAlign::Left` / `TitleAlign::Center` for title text
  - Optional icon glyph before the title (`with_icon()`)
  - Optional drag-zone hover hint (default on, `without_drag_hint()` to disable)
  - Optional 1-px separator below titlebar (default on, `without_separator()` to disable)
  - Optional focus-dim: `with_focus_dim()` — dims titlebar when window loses OS focus (default off)
  - `WindowAction::IconClick` — click on the window icon area
  - `impl Default for TitlebarResult` for ergonomic no-op initialization
  - Full doc-comments on all `BorderlessConfig` builder methods
- **app_window** — Zero-boilerplate application window combining wgpu + winit + Dear ImGui
  - `AppWindow::run<H: AppHandler>(handler)` — replaces ~300 lines of setup code
  - `AppHandler` trait: `render()`, `on_close_requested()`, `on_extra_button()`, `on_icon_click()`, `on_theme_changed()`
  - `AppConfig` builder: `with_min_size`, `with_fps_limit`, `with_font_size`, `with_start_position`, `with_theme`, `with_titlebar`
  - `StartPosition`: `CenterScreen` (default), `TopLeft`, `Custom(x, y)`
  - Auto GPU backend selection: DX12 → Vulkan → GL (software fallback) on Windows
  - Auto HiDPI: DPI scale clamped to `[1.0, 3.0]`, font scaled accordingly
  - Auto surface-format detection: prefers sRGB, gracefully falls back
  - FPS cap: `WaitUntil(1/fps)` sleep; `fps_limit=0` → explicit `ControlFlow::Poll`
  - `AppState::set_theme(TitlebarTheme)` — deferred; applied after frame closes:
    1. Updates `borderless_window` titlebar palette
    2. Reapplies full Dear ImGui widget color palette via `apply_imgui_style_for_theme()`
    3. Calls `AppHandler::on_theme_changed()` callback
  - `AppState`: `exit()`, `toggle_maximized()`, `set_maximized()`, `set_theme()`
  - `app_window/style.rs` — complete ImGui widget palette for all 6 themes
    - Covers `StyleColor`: `WindowBg`, `ChildBg`, `PopupBg`, `Border`, `FrameBg`, `TitleBg*`, `MenuBarBg`, `ScrollbarBg`, `ScrollbarGrab*`, `CheckMark`, `SliderGrab*`, `Button*`, `Header*`, `Separator*`, `ResizeGrip*`, `Tab*`, `Text`, `TextDisabled`
- **demo_borderless** — Standalone `borderless_window` demo
  - All 6 built-in themes switchable at runtime
  - Edge resize cursor feedback
  - Close confirmation dialog
  - Extra button demo
- **demo_app_window** — `AppWindow` + `AppHandler` demo
  - Click counter widget
  - Theme picker for all 6 themes
  - Scrollable event log (FIFO, capped at 50 entries)
  - Maximize toggle
  - Custom close confirmation dialog

### Changed
- **Cargo.toml** — All dependencies pinned to explicit latest stable versions:
  - `dear-imgui-rs` / `dear-imgui-wgpu` / `dear-imgui-winit` → `0.11.0`
  - `wgpu` → `29.0.1`
  - `winit` → `0.30.13`
  - `windows-sys` → `0.61.2`
  - `pollster` → `0.4.0`
  - `foldhash` → `0.2.0`
- **borderless_window** — `focus_dim` default changed from `true` → `false`

### Fixed
- **borderless_window/mod.rs** — `calc_text_size` now calls `ui.current_font().calc_text_size(...)` (moved from `Ui` to `Font` in dear-imgui-rs 0.11)
- **borderless_window/mod.rs** — Removed dead `$close` macro parameter and tautological if-branch from `btn_cell!`
- **borderless_window/actions.rs** — Removed `#[cfg(test)]` gate from `TitlebarResult::none()`; added `impl Default`
- **app_window/gpu.rs** — `surface_caps.formats[0]` → `.first().copied().or_else(...)` (panic-free)
- **app_window/gpu.rs** — `render_draw_data().expect()` → `if let Err(e)` (graceful GPU error handling)
- **app_window/gpu.rs** — Double-maximize bug: `maximize_toggle` flag now cleared after OS call
- **app_window/style.rs** — `TabActive` → `TabSelected`; added `TabDimmed`, `TabDimmedSelected` (dear-imgui-rs 0.11)
- **app_window/style.rs** — `clamp_add` now preserves source alpha `c[3]` instead of hardcoding `1.0`
- **disasm_view/mod.rs** — `let mut p` → `let p` (unused-mut warning)

## [0.5.0] — 2026-03-30

### Added
- **hex_viewer** — Binary hex dump viewer widget
  - Offset/hex/ASCII column layout with configurable bytes-per-row (8, 16, 32)
  - Color regions for visual data segmentation
  - Data inspector panel with multi-type decoding (u8–u64, i8–i64, f32, f64)
  - Goto address (hex `0x` or decimal), pattern search with match navigation
  - Selection (click + shift-click), diff highlighting for changed bytes
  - Hover byte tooltips: offset (hex+dec), hex/dec/octal/binary values, ASCII
  - Hover row highlight, zero-dimmed byte styling
  - Little/big endian toggle, configurable column widths
- **timeline** — Zoomable profiler timeline widget
  - Multi-track layout with per-track collapse/expand
  - Nested span rendering with depth-based vertical offset
  - Flame graph view mode
  - Named markers on the time ruler
  - Pan (drag) + zoom (scroll) with Shift+scroll for horizontal pan
  - Adaptive time ruler with auto-scaled tick intervals
  - Color modes: by duration, by category, by name hash
  - Span tooltips with label, duration, category, source info
  - Configurable track height, span padding, colors
  - `Span::new` validates start/end and rejects NaN/Infinity
  - Division-by-zero guard in `x_to_time`
- **diff_viewer** — Side-by-side and unified diff viewer
  - Myers diff algorithm with O((N+M)D) time, capped at max_d=50,000
  - Side-by-side (two-panel) and unified view modes
  - Synchronized scrolling between panels
  - Fold/unfold unchanged regions with configurable context lines
  - Hunk navigation (prev/next) with keyboard support
  - Hover row highlights in both panel and unified modes
  - Current hunk blue accent bar in unified mode
  - `+`/`-` prefix characters in unified mode
  - Diff stats: additions, deletions, unchanged count
  - Hunk context preservation across hunk boundaries
- **property_inspector** — Hierarchical property editor
  - 15+ value types: Bool, I32, I64, F32, F64, String, Color3, Color4, Vec2, Vec3, Vec4, Enum, Flags, Object, Array
  - Categories with collapsible headers (click to toggle)
  - Property nodes with expand/collapse for nested children
  - Recursive child rendering with `std::mem::take` pattern
  - Type badges (dimmed type name right-aligned on each row)
  - Hover highlight on all rows
  - Search/filter support, diff highlighting
  - Builder API for categories and properties
- **toolbar** — Configurable horizontal toolbar widget
  - Buttons, toggles, separators, dropdowns, spacers
  - Icon support via `with_icon()` builder (MDI Unicode glyphs)
  - Hover underline accent with configurable color and thickness
  - Window-hovered guard to prevent click-through
  - Flexible spacer layout (auto-distributes remaining width)
  - Dropdown cycles through options on click
  - Builder pattern API with `with_enabled()`, `with_icon()`
- **status_bar** — Composable bottom status bar widget
  - Left/center/right sections with independent item lists
  - Status indicators: Success, Warning, Error, Info (colored dots)
  - Progress bar items with label (0.0..=1.0)
  - Clickable items with event emission (`StatusBarEvent`)
  - Icon support via `with_icon()` builder
  - Hover highlight on all items (subtle for non-clickable, stronger for clickable)
  - Window-hovered guard, tooltips via `with_tooltip()`
  - Color override via `with_color()`
- **demo_hex_viewer** — Interactive HexViewer demo with PE header sample, color regions, config panel
- **demo_timeline** — Timeline demo with 4 tracks, 50+ spans, markers, color mode switching
- **demo_diff_viewer** — DiffViewer demo with 4 sample datasets, mode/fold/context config
- **demo_property_inspector** — PropertyInspector demo with 5 categories, 20+ properties
- **demo_status_toolbar** — Combined Toolbar + StatusBar demo with event log
- **icons** — Expanded to 7,400+ Material Design Icons v7.4 constants

### Improved
- **node_graph** — Tooltip hover tracking moved after hit testing (was running before, so tooltips never triggered)
- **node_graph** — `collect_node_aabbs` now reuses a buffer instead of allocating Vec every frame
- **node_graph** — `NgColors` derives `Debug + Clone + Copy`, `NodeGraphConfig` derives `Debug + Clone`
- **node_graph** — Null-pointer guard on `igGetCurrentWindow()` unsafe calls with `debug_assert`
- **node_graph** — 42 new tests covering Graph slab, Viewport transforms, math functions (bezier, point-to-segment), InteractionState, config
- **toolbar** — Config derives `Copy` to avoid per-frame clones
- **status_bar** — Config derives `Copy`
- **property_inspector** — Config derives `Copy`, `PropertyValue` derives `PartialEq` and implements `Default`
- **diff_viewer** — Per-frame `clone()` of display lines eliminated via `render_panel_static` with raw slices

### Fixed
- **node_graph** — `node_to_top` comment corrected from "O(1)" to "O(n) find + O(1) swap_remove"
- **toolbar** — Dropdown panic on empty options list (added `!options.is_empty()` guard)
- **toolbar** — Dropdown selected index clamped at construction
- **toolbar** — ImGui `SetCursorPos` assertion crash (added `ui.dummy([0.0, 0.0])` after cursor advance)
- **status_bar** — ImGui `SetCursorPos` assertion crash (same fix)
- **timeline** — Division by zero in `x_to_time` when `pixels_per_second` is zero (clamped to `1e-9`)
- **timeline** — `Span::new` now validates and swaps start>end, rejects NaN/Infinity
- **timeline** — Shift+scroll conflict with zoom (now properly separated)
- **diff_viewer** — Myers algorithm capped at `max_d=50,000` to prevent excessive memory on large inputs
- **diff_viewer** — Hunk context loss: trailing context now preserved as leading context for next hunk
- **diff_viewer** — `render_unified` bounds check for mismatched line counts
- **code_editor/tokenizer** — Panic on multi-byte UTF-8 characters (Cyrillic, emoji) fixed

## [0.4.0] — 2026-03-30

### Added
- **code_editor** — Full-featured code editor widget built on ImGui DrawList API
  - `CodeEditor` widget with syntax highlighting, line numbers, cursor/selection, undo/redo
  - 10 built-in languages: Rust, TOML, RON, Rhai, JSON, YAML, XML, ASM (x86/ARM/RISC-V), Hex, None
  - Custom language support via `SyntaxDefinition` trait (`Language::Custom(Arc<dyn SyntaxDefinition>)`)
  - ASM tokenizer: AT&T + Intel + NASM syntax, registers, directives, labels, numeric literals
  - 6 built-in themes: DarkDefault, Monokai, OneDark, SolarizedDark, SolarizedLight, GithubLight
  - 3 embedded monospace fonts: Hack (default), JetBrains Mono NL, JetBrains Mono
  - MDI icons (Material Design Icons v7.4) merged into font atlas
  - `install_code_editor_font()` / `install_code_editor_font_ex()` — zero-config font setup
  - `BuiltinFont` enum with `Hack`, `JetBrainsMonoNL`, `JetBrainsMono` variants
  - Code folding with MDI chevron icons, hover highlight, and `"... N lines"` collapsed badge
  - `show_fold_indicators` config option — toggle fold UI and gutter column
  - Word wrap with smart word-boundary breaking
  - Find/replace bar with case-insensitive toggle, match navigation, replace-all
  - Multi-cursor support (Ctrl+D to select next occurrence)
  - Bracket matching and auto-close for `()`, `{}`, `[]`, quotes
  - Text transforms: UPPERCASE, lowercase, Title Case, trim whitespace
  - Line operations: duplicate, delete, move up/down
  - Toggle comment (Ctrl+/)
  - Font zoom (Ctrl+Scroll, Ctrl+Plus/Minus)
  - Hex editing mode: auto-space, auto-uppercase, value-based coloring
  - Color swatches next to hex color literals
  - Error/warning markers with underlines and gutter icons
  - Breakpoints with gutter indicators
  - Right-click context menu with 12 configurable sections (`ContextMenuConfig`)
  - `max_lines` and `max_line_length` config options (0 = unlimited)
  - Auto English keyboard layout on focus (Windows, opt-in)
  - `EditorConfig` with 20+ configurable options
- **demo_code_editor** — Interactive demo with font switcher, config panel, all features

### Improved
- **code_editor** — Adaptive smooth scrolling: faster catch-up when cursor moves rapidly (Enter spam)
- **code_editor** — Scroll dummy height includes bottom padding + 1px dummy for correct ImGui scroll extent
- **code_editor** — Wrap cache re-synced after input handling to prevent stale scroll targets on paste
- **code_editor** — `compute_wrap_points` rewritten: overflow checked BEFORE adding character width, re-evaluates current char after break (handles lines >2× max_width correctly)
- **code_editor** — Gutter layout: `| line numbers | fold icon | code |` with proper spacing

### Fixed
- **code_editor** — Scrollbar could not reach bottom of document with word wrap + large text
- **code_editor** — Rare HEX word-wrap overflow: last byte on a row could exceed the vertical boundary
- **code_editor/lang/asm** — NASM preprocessor directives (`%define`, `%macro`) were misclassified as AT&T registers
- **code_editor/lang/asm** — 12 clippy warnings about unused `line_start` variable

## [0.3.2] — 2026-04-09

### Added
- **virtual_table** — Keyboard navigation: Up/Down, Home/End, PageUp/PageDown move selection and auto-scroll
- **virtual_table** — `scroll_to_row(idx)` — programmatic scroll to any row
- **virtual_table** — `select_row(idx)` — programmatic select + scroll
- **virtual_table** — `selection_text_color` config option — override text color for selected rows (default: white)
- **virtual_table** — `pending_scroll_to` internal field for deferred scroll (works from click, keyboard, and API)
- **virtual_tree** — Public modules: `filter`, `flat_view` — `FilterState`, `FlatView`, `FlatRow`, `NodeSlot` now exported for advanced use

### Improved
- **virtual_table** — Selection highlight visibility: `selection_color` alpha increased from 0.55 to 0.75, selection text now white by default
- **virtual_table** — Selection text color overrides both default and row_style text color (cell_style still takes precedence)

### Changed
- **virtual_tree/arena** — `NodeSlot<T>` visibility changed from `pub(crate)` to `pub`
- **virtual_tree/filter** — `FilterState` visibility changed from `pub(crate)` to `pub`
- **virtual_tree/flat_view** — `FlatView`, `FlatRow` visibility changed from `pub(crate)` to `pub`
- Tests moved from `src/` to `examples/demo_table.rs` and `examples/demo_tree.rs`
- Removed test-only methods: `set_capacity_unclamped()`, `new_unclamped()`
- Deleted `src/virtual_tree/bench.rs` — stress tests now in `examples/demo_tree.rs`

## [0.3.1] — 2026-03-26

### Added
- **virtual_tree** — `MAX_TREE_NODES` constant (1,000,000) — hard capacity limit with graceful `None` returns on insert
- **virtual_tree** — `TreeArena::with_capacity(n)` — pre-allocate arena with custom capacity limit (`1..=MAX_TREE_NODES`)
- **virtual_tree** — Configurable per-instance capacity: `set_capacity(n)` / `capacity()` on both `TreeArena` and `VirtualTree`
- **virtual_tree** — Optional FIFO eviction: `set_evict_on_overflow(true)` — auto-removes oldest root subtree when at capacity
- **virtual_tree** — `TreeConfig::max_nodes` and `TreeConfig::evict_on_overflow` — declarative capacity control
- **virtual_table** — `MAX_TABLE_ROWS` constant (1,000,000) — capacity clamped on `RingBuffer::new()`
- **virtual_tree** — `ExpandStyle::Glyph` — custom expand/collapse glyphs with optional color

### Improved

#### 500K-node optimization pass
- **virtual_tree/flat_view** — `index_of()` is now O(1) via `HashMap<NodeId, usize>` (was O(n) linear scan)
- **virtual_tree/flat_view** — Eliminated `visible_children.collect()` per expanded node — two-pass count+iterate without allocation (was 100K–300K temp Vec allocations per rebuild)
- **virtual_tree/filter** — Reusable `matching_buf` Vec across filter calls (no re-allocation)
- **virtual_tree/filter** — Safe early-break in ancestor walk when `auto_expand` is false (skip already-marked ancestors)
- **virtual_tree/arena** — `remove()` / `move_node()` use `position()` + `swap_remove()`/`remove()` instead of `retain()` — O(1) detach vs O(siblings)
- **virtual_tree/mod** — `deselect_descendants()` directly removes from HashSet instead of collecting into intermediate Vec
- **virtual_tree/mod** — Glyph expand button: zero-allocation rendering — button ID written into `cell_buf` tail, glyph text reused without clone
- **virtual_tree/mod** — `take_cell_value()` moves String out of edit buffer instead of cloning (zero-copy commit)
- **virtual_table/mod** — `handle_sort()` uses raw pointer to sort specs instead of `Vec::clone()`
- **virtual_table/mod** — `render_editor_inline()` uses raw pointer to `CellEditor` instead of `editor.clone()` (avoids cloning `Vec<String>` per frame)
- **virtual_table/mod** — `take_cell_value()` moves String out of edit buffer instead of cloning
- **virtual_table/mod** — All `unwrap()` calls in render_row replaced with safe `if let Some(row)` / `let Some(row) else continue` patterns — no panics at runtime
- **virtual_tree/flat_view** — Iterative DFS replaces recursive `walk()` — no stack overflow at any depth (tested at 10K levels)
- **virtual_tree/arena** — `remove()` and `update_subtree_depth()` converted from recursive to iterative — safe at any depth
- **virtual_tree** — `insert_root()` / `insert_root_at()` now return `Option<NodeId>` (capacity-aware)
- **virtual_tree/arena** — `depth` field uses `saturating_add(1)` — no u16 overflow at extreme depths
- **virtual_table/row** — Color formatting clamps `f32` to `0.0..=1.0` before `* 255 as u8` — no overflow
- **virtual_table/edit** + **virtual_tree/edit** — `i64→i32` and `f64→f32` casts clamped to prevent silent truncation
- **virtual_table/mod** — Shift+Click selection range clamped to `data.len()` — no out-of-bounds indices
- **virtual_table/mod** + **virtual_tree/mod** — `unreachable!()` in ComboBox/Button editor paths replaced with safe `deactivate() + return`
- **virtual_tree/mod** — `tree_column` clamped to `col_count - 1` — no silent skip on misconfigured index
- **bench** — Runtime stress tests for 500K and 1M nodes: insert, expand, flat_view rebuild, filter, remove, deep chain, memory estimate

## [0.3.0] — 2026-03-18

### Added
- **virtual_tree** — Hierarchical tree-table component for 100k+ nodes
  - `VirtualTree<T>` widget with `VirtualTreeNode` trait
  - `TreeArena<T>` — generational slab storage with `NodeId`, parent/children links, O(1) insert/remove/lookup
  - `FlatView` — cached linearization rebuilt only on structural changes (not every frame)
  - ListClipper virtualization for visible rows only
  - Multi-column support reusing `ColumnDef`/`CellEditor` from `virtual_table`
  - Inline editing: text, checkbox, combo, slider, color, button, custom
  - Selection: None, Single, Multi (Ctrl+Click toggle, Shift+Click range on flat view)
  - Sibling-scoped sorting via ImGui table headers
  - Drag-and-drop node reparenting with `accepts_drop()` / `is_draggable()` control
  - Filter/search with auto-expand matching branches
  - Tree lines — vertical/horizontal connector lines via `continuation_mask: u64` bitmask
  - Striped rows (alternating backgrounds) via `config.striped`
  - Scroll-to-node — `scroll_to_node(id)` expands ancestors + scrolls into view
  - `NodeIcon` variants: `Glyph`, `GlyphColored`, `ColorSwatch`, `Custom`
  - `badge()` trait method — optional text after node label
  - Clip tooltips — automatic hover tooltip when cell text exceeds column width
  - Lazy children loading via callback
  - Keyboard navigation: Up/Down (flat), Left (collapse/parent), Right (expand/child)
  - `TreeConfig` wrapping `TableConfig` with tree-specific settings
  - `children_count(id)`, `ensure_visible(id)`, `flat_row_count()`, `flat_index_of(id)` API
- **demo_tree** — Full interactive VirtualTree example
  - TaskNode with 6 kinds (Folder, RustFile, Config, Document, Test, Asset) and 4 priority levels
  - 6 columns: Name (TextInput), Done (Checkbox), Progress (SliderFloat), Priority (ComboBox), Size, Action (Button)
  - Colored icons per node type, per-row styling (dimmed done items), per-cell colored priority text
  - Toolbar: filter, expand/collapse all, stress test 10K nodes, add root, tree lines/striped/drag-drop toggles
  - Context menu: Add Child File, Add Subfolder, Toggle Done, Set Priority submenu, Delete
- **virtual_table** — New `ColumnDef` features
  - `ColumnSizing::AutoFit(f32)` — auto-fit column to content width
  - `clip_tooltip: bool` — automatic tooltip when cell text is wider than column (default: `true`)
  - `default_sort: Option<bool>` — default sort direction (ascending/descending) for column header
  - Builder methods: `.auto_fit()`, `.clip_tooltip()`, `.no_clip_tooltip()`, `.default_sort(ascending)`
  - Clip tooltip rendering in both read-only and editable row paths
- **docs/virtual_tree.md** — Full component documentation

### Improved
- **virtual_tree** — Zero per-frame allocations: `write!` into scratch buffer instead of `format!()`, `mem::take` for arena children ops, unsafe pointer for CellEditor access during render
- **virtual_tree** — `mem::forget` on TreeNodeToken with `NO_TREE_PUSH_ON_OPEN` to prevent ID stack corruption
- **virtual_tree** — Filter ancestor walk always reaches root (removed unsafe early-break optimization)
- **virtual_tree** — `.map().flatten()` → `.and_then()` cleanup
- **README.md** — Updated with virtual_tree component, docs links, demo command, project structure

## [0.2.1] — 2026-03-17

### Added
- **node_graph** — Stats overlay drawn on the canvas corner (node count, wire count, zoom level, selection count)
  - Configurable corner (`stats_overlay_corner: u8`, 0–3) and margin (`stats_overlay_margin: f32`)
  - Toggle via `show_stats_overlay: bool` in config
- **node_graph** — Orthogonal wire style (`WireStyle::Orthogonal`): 3-segment forward routing, 5-segment backward routing with obstacle avoidance
- **node_graph** — `body_height()` method on `NodeGraphViewer` trait — per-node body height override for nodes with multiple widget rows
- **node_graph** — Frustum culling: only visible nodes are rendered each frame, enabling graphs with up to 100,000 nodes

### Improved
- **node_graph** — `selected` field changed from `Vec<NodeId>` to `HashSet<NodeId>` — all selection operations are now O(1)
- **node_graph** — `selected()` now returns `Vec<NodeId>` (collected from HashSet) instead of `&[NodeId]`
- **node_graph** — Node body rendered inside `with_clip_rect()` — widgets can no longer overflow node boundaries
- **node_graph** — Bezier tangent length uses adaptive extent-based scaling instead of a fixed 50px value — curves look correct at all zoom levels and node distances
- **node_graph** — Minimap navigation: removed confusing viewport rectangle; click or drag on minimap navigates directly to that position
- **node_graph** — Minimap drag remains active when cursor leaves minimap bounds (coordinates clamped to valid range)
- **node_graph** — Removed scrollbar rendering and config fields (`show_scrollbar_h`, `show_scrollbar_v`, `scrollbar_thickness`)
- **node_graph** — Stats display moved from external toolbar to built-in canvas overlay
- **demo_node_graph** — `body_height()` implemented for Vec2 (54.0) and Color (42.0) nodes to fit their widget content

### Fixed
- **node_graph** — Wire drag-and-drop was broken: `is_mouse_dragging()` returns `false` on mouse-release frame; replaced with `mouse_drag_delta()` threshold check
- **node_graph** — ImGui assertion `SetCursorScreenPos() requires subsequent item` — added `ui.dummy()` after `set_cursor_screen_pos()` for body height reservation
- **node_graph** — Orthogonal wire hit-test now matches rendering exactly (removed erroneous `abs < 2.0` fallback condition)

## [0.2.0] — 2026-03-17

### Added
- **node_graph** — Visual node graph editor component
  - `NodeGraph<T>` widget with `NodeGraphViewer<T>` trait
  - Slab-based `Graph<T>` storage (O(1) insert/remove) + `HashSet<Wire>`
  - Pan/zoom canvas with scroll-to-cursor zoom
  - Bezier and straight-line wire rendering via native `ImDrawList`
  - 4 pin shapes: Circle, Triangle, Square, Diamond
  - Per-pin color, stroke, and wire style overrides (`PinInfo` builder)
  - Custom node headers with color tinting
  - Node body rendering with `&mut T` (sliders, combos, color pickers, etc.)
  - Multi-select (Ctrl+Click) and rectangle selection
  - Node collapse/expand with chevron button
  - Snap-to-grid with configurable grid size
  - Interactive mini-map (click/drag to navigate)
  - Wire yanking (Ctrl+Click wire to detach and redirect)
  - Dropped wire on canvas fires `DroppedWireOut`/`DroppedWireIn` actions for auto-connect menus
  - Context menus: right-click on canvas (`CanvasMenu`) or node (`NodeMenu`)
  - Keyboard: Delete (remove selected), Ctrl+A (select all), Escape (cancel wire/rect)
  - LOD culling: labels, pins, and bodies hidden at low zoom levels
  - Wire layer control: behind or above nodes
  - Tooltips on nodes and individual pins
  - `HashMap<PinId, [f32; 2]>` for O(1) pin position lookup
  - `HashSet<NodeId>` for O(1) draw order membership check
  - Fixed-size array for diamond pin geometry (zero per-frame allocations)
  - Multiple actions per frame via `Vec<GraphAction>` return type
  - `NodeToggled` and `SelectAll` handled internally
  - Multi-select snap drift fix (delta computed from snapped position)
  - Viewer trait lifetime fix: `&str` methods can return data from `&T` or `&self`
- **demo_node_graph** — Full interactive example
  - 8 node types: Float, Vec2, Color, Add/Sub/Mul/Div, Clamp, Mix, Output
  - Typed pins with different shapes and colors
  - Context menu to add nodes, auto-connect on dropped wires
  - Toolbar: Fit, Reset, Grid, Snap, Minimap, Wire Layer toggles
- **docs/** — Per-component documentation
  - `docs/file_manager.md` — FileManager guide with API reference
  - `docs/virtual_table.md` — VirtualTable guide with trait reference
  - `docs/page_control.md` — PageControl guide with tab styles
  - `docs/node_graph.md` — NodeGraph guide with full configuration reference

### Improved
- **node_graph** viewer trait: unified lifetime `'a` on `title()`, `input_label()`, `output_label()`, tooltip methods — returned `&str` can now come from node data, not just the viewer
- **node_graph** `select_node()` and `deselect_all()` now public API
- **node_graph** `fit_to_content()` uses actual node dimensions via `config.node_height()` + `viewer.node_width()` instead of hardcoded values
- **node_graph** `screen_to_graph()` guards against division-by-zero on `zoom <= 0`
- **node_graph** single `draw_order` clone per frame (was cloned twice)
- **node_graph** removed unused `_viewer` parameter from `is_collapse_button_hit()`
- **README.md** — Updated with node_graph component, docs links, all 4 examples

## [0.1.1] — 2026-03-15

### Improved
- **page_control** — 4 tab styles (Pill, Underline, Card, Square) with runtime switching
- **page_control** — Close button on dashboard tiles with confirmation dialog
- **page_control** — `Hash` derive on all public enums (`PageStatus`, `ContentView`, `TabStyle`, `PageAction`)
- **page_control** — Modern Rust patterns: `.is_some_and()`, let-chains, `AtomicU32` for static counters
- **page_control** — `Box<NestedPage>` to reduce enum variant size disparity in demo
- **file_manager** — Footer layout: filename input + buttons on a single row (SaveFile mode)
- **file_manager** — Filter dropdown + buttons on a single row (OpenFile mode)
- **file_manager** — Disabled confirm button rendered as dimmed button (preserves layout)
- **file_manager** — Content area height correctly reserves space for footer (no scroll needed)
- **file_manager** — All collapsible_if warnings fixed with let-chains
- **virtual_table** — All collapsible_if warnings fixed with let-chains
- **demo** — MDI font loading via `FontSource::TtfData` with merge mode (dynamic glyph loading)
- **demo** — Tab style switcher button in toolbar

### Fixed
- Unnecessary `as i32` cast on `ImGuiCond_Appearing` (already `i32`)
- `#[allow(clippy::too_many_arguments)]` on render functions that genuinely need many params
- Zero clippy warnings across the entire project

## [0.1.0] — 2026-03-06

### Added
- **file_manager** — Universal file/folder picker dialog
  - Modes: SelectFolder, OpenFile, SaveFile
  - Drive selector, breadcrumb navigation, file filters
  - Favorites sidebar, back/forward history, type-to-search
  - Rename, delete, new folder/file creation
  - Overwrite confirmation modal
  - Multi-select (Ctrl+Click), keyboard navigation
  - Zero per-frame allocations
- **virtual_table** — Virtualized table component
  - `VirtualTableRow` trait for custom row types
  - `RingBuffer<T>` — fixed-capacity O(1) ring buffer
  - `ColumnDef` — fixed, stretch, centered columns with builder pattern
  - ListClipper integration for 100k+ row rendering
  - Inline editing: text, checkbox, combo, slider, spinner, color, progress, custom
  - Selection modes: None, Single, Multi
  - Sortable columns (multi-column)
- **page_control** — Generic tabbed container
  - Dashboard view (interactive tile grid with status indicators)
  - Tabs view (pill-shaped tab strip with scroll buttons)
  - Close confirmation popups, badges, context menu
  - Keyboard navigation (arrow keys, Ctrl+W)
- **icons** — Material Design Icons v7.4 constants (160+ icons)
- **theme** — Dark color palette with semantic tokens
- **utils** — Color packing (RGBA to u32), text measurement wrapper
- **demo** — Interactive showcase with tabs for all components
