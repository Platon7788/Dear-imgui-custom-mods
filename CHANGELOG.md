# Changelog

## [Unreleased]

### Removed (BREAKING)

- **`app_window` module deleted entirely** (session 044, 2026-05-06,
  ADR-029). The runner machinery (event loop, wgpu surface lifecycle,
  ImGui platform / renderer init, font atlas, frame scheduler, proxy)
  duplicated `dear-app` and was the sole source of the borderless-on-
  small-screen, surface-reconfigure, and keyboard-event ordering bugs
  reported across sessions 040-044. Public types removed: `AppWindow`,
  `AppHandler`, `AppState`, `AppProxy`, `AppConfig`, `BorderStyle`,
  `Chrome` enum (now `Option<TitlebarConfig>`), `ExtraButton`,
  `FontChoice`, `FontLayer`, `FormStyle`, `FpsMode`, `GlyphRanges`,
  `Position`, `PowerMode`, `RenderMode`, `WindowIcon`, `WindowKind`,
  `TitlebarState`. Cargo feature `app_window` removed (chrome is
  always-on infrastructure now). See [docs/chrome.md](docs/chrome.md)
  for the migration recipe; `test-dear-imgui-rs` (sibling repo) is the
  end-to-end reference for the new pattern.

### Added

- **`chrome` module** — borderless-window helpers as stateless / explicit-
  state helpers (no runner). Exposes `render_titlebar()`,
  `whole_window_resize()`, a `Chrome` convenience wrapper,
  `TitlebarConfig`, `Buttons`, `TitleAlign`, `CloseMode`, `ResizeEdge`,
  `TitlebarAction`, `TitlebarResult`, `ContentArea`, plus the
  `chrome::win32::*` Win32 helpers (`setup_window`, `sync_region`,
  `set_opacity`, `hwnd_of`, `is_win11`). Hosts wire chrome into any
  `winit`-based runner — typically `dear-app` via `on_gpu_init` +
  `on_event` + `on_frame` callbacks.

- **`disasm_view::follow_at_cursor_diagnostic` + `FollowOutcome` enum**
  (session 043, follow-up audit). Diagnostic variant of the existing
  `follow_at_cursor()` boolean — surfaces the *reason* the
  double-click / Enter gesture succeeded or quietly failed:
  `Followed { from, to }`, `NoCursor`, `TargetOutsideProvider(addr)`,
  `NoTargetAndNoNumber`. Hosts that want a status-line hint
  ("Cannot follow: target 0x4011A0 not in provider") use this
  instead of the bool wrapper. Mod docs gain a "Follow-at-cursor
  gesture" section listing the resolution order, the bracket-aware
  fallback, and the common host-side mistakes that make follow
  appear broken (forgot `.with_target()`, target outside provider,
  register-indirect operand).

### Fixed

- **`disasm_view` follow-at-cursor hit-zone widened + middle-click
  added** (session 043, follow-up audit #2). User reported that
  even `jmp` rows didn't navigate on double-click. Root cause:
  the previous handler restricted follow to the
  Mnemonic+Operands column only (`mouse_in_instruction_column`),
  but wide operands like `jmp qword ptr [rip+0x12345678]`
  overflow into the Comment-column area at typical zoom levels,
  so the click landed in the edit-cell handler (silent no-op
  when `editable=false`). Three changes:
  - **Hit-zone widened** — double-click anywhere on a row (not
    just Instruction column, address-gutter still has its own
    copy-to-clipboard gesture above) now attempts follow first.
    Edit-cell handler runs only when follow declines (no branch
    target, no resolvable operand). Removes the dead
    `mouse_in_instruction_column` helper.
  - **Middle-click follow added** — IDA / Cheat-Engine convention.
    Single deliberate middle-click bypasses ImGui's double-click
    time / position thresholds entirely; more discoverable for
    users unsure whether the gesture is wired.
  - **Tooltip discoverability hint** — when a row exposes
    `branch_target`, the hover tooltip now emits a
    `tooltip_double_click_follow` line ("Double-click /
    middle-click to follow" / "Двойной клик / средняя кнопка —
    переход") right after the existing Target / Offset lines.
    Mirrors the long-standing "Double-click to copy" hint on
    the address gutter.
- **`disasm_view::follow_at_cursor` no longer chases displacement
  numbers inside `[...]` for `Call` / `Jump` rows** (session 043,
  follow-up audit). Pre-fix, `call qword ptr [rip+0x1234]` would
  send the operand-string fallback chasing `0x1234` (the
  displacement, not the call target) — typically a ghost
  `decode_range` call followed by a quiet no-op, leaving the user
  to wonder why double-click "doesn't work". Post-fix the
  scanner tracks bracket depth and skips in-bracket numbers for
  branching flow kinds; non-branching rows (`mov rax, [0x401000]`)
  keep the old behaviour so memory-pointer follow still works.
  Pinned by 8 new follow tests (15 total) covering call-indirect,
  jmp-indirect, register-indirect, symbolic-label, target-missing,
  and no-cursor cases.

### Added

- **`disasm_view` educational tooltip uplift — 4-pass programme**
  (session 043, four commits Заход 1-4). Turns the hover tooltip into
  a beginner-friendly RE tutor that labels every instruction with up
  to seven layers of educational context, all toggleable, all
  locale-aware (EN / RU). Default `true` for every layer with
  `#[serde(default = "fn")]` for forward compatibility.
  - **`abi.rs` + `operand.rs`** (Заход 1, commit 38739b3). New
    [`Abi`] enum (Win64 / SysVAmd64 / Cdecl / Stdcall / Fastcall /
    Unknown) drives a per-register role table — calls `[rcx]` "the
    1st argument under Win64", `[rdi]` "the 1st argument under
    SysV". `operand::parse` decodes operand strings into typed
    `OperandKind`; `explain_memory` turns `[rcx+rax*8+8]` into
    "Array indexing: rcx is base, rax is index ×8 (qword elements),
    then add 0x8". Recognises stack-relative, frame-relative,
    RIP-relative, and TIB / PEB segment access at `fs:[0x18]` /
    `gs:[0x60]` etc. 29 unit tests.
  - **`compiler.rs` + `antidisasm.rs`** (Заход 2, commit 22af708).
    `compiler::detect` labels MSVC / Clang / GCC stereotyped
    sequences: PEB / TEB / TIB segment access, `__chkstk` probe,
    Win64 leaf frame, vtable indirect call (slot-N and slot-0),
    SEH frame install / uninstall, MSVC `/GS` `__security_cookie`,
    atomic CAS / RMW, `cpuid` feature probe, IAT / GOT thunk,
    indirect tail jump (13 patterns). `antidisasm::detect`
    recognises stack-based control flow (`push imm; ret`), opaque
    predicates, SMC writes to `[rip+disp]`, hypervisor bit
    detection (`bt ecx, 31` after `cpuid`), hypervisor vendor leaf
    `0x40000000`, trap-flag arming (`or [rsp], 0x100; popf`),
    `jmp`-into-instruction tricks, `rdtsc`-delta timing prints
    (9 patterns). Both pure / no-allocation, both strip
    `qword ptr` / `dword ptr` size hints. 25 unit tests.
  - **`boundary.rs` + `branch.rs`** (Заход 3, commit 3ca3929).
    `boundary::detect` labels function prologues (framed
    `push rbp; mov rbp, rsp`, CET `endbr64` landing pad),
    epilogues (`leave; ret`, `pop rbp; ret`,
    `add rsp, N; ret`), bare returns, and block-level
    terminators (unconditional `jmp`, conditional `Jcc` /
    `loop*` / `j[er]cxz` forks). `branch::classify` reads the
    host-resolved `branch_target()` and labels forward jumps as
    `if` / `match` / `switch` skip-overs, backward jumps as loops,
    self-targeting jumps as anti-RE spin traps — embeds the signed
    delta in the description so distance is immediate. 15 unit
    tests.
  - **i18n + settings + canonical pipeline doc** (Заход 4 — this
    commit). Eight new tooltip labels (`tooltip_compiler_label` /
    `tooltip_antidisasm_label` / `tooltip_boundary_label` /
    `tooltip_branch_label` and four matching `settings_show_*`
    keys) plus `show_compiler_pattern` / `show_antidisasm` /
    `show_boundary` / `show_branch_direction` config flags
    surfaced as four checkboxes in the disasm Settings popup.
    `disasm_view` mod docs gain a "Pipeline" section listing all
    seven detector layers in render order with a one-line summary
    of each — analysts can see at a glance which layer fires for
    which question.
- **`crate::i18n` extended to `code_editor` — final batch** (session
  042, batch 4/4 — i18n now covers **9/9 standalone widgets**).
  ~58 keys: full right-click context menu (Cut/Copy/Paste/Select All
  + Undo/Redo + Toggle Comment/Duplicate Line/Delete Line + Transform
  submenu with UPPERCASE/lowercase/Title Case/Trim Whitespace + Find
  + View submenu with 7 toggles + Language submenu header + Theme
  submenu header + Font scale label & ± tooltips + cursor-info row).
  Find/replace bar fully localised (placeholders, "No matches" badge,
  6 button tooltips, Replace/All buttons).
  Format-template helper `i18n::code_editor::cursor_info(locale, line,
  col, total)` localises `"Ln 12, Col 5  /  100 lines"`.
  `EditorConfig::locale` field + `CodeEditor::with_locale` /
  `set_locale` / `locale()`. Programming-language identifiers
  (`Rust`/`RON`/`JSON`/…) and theme names stay untranslated as proper
  nouns; keyboard shortcuts (`Ctrl+X`, `F3`, `Esc`) likewise stay in
  cross-locale technical form.
- **`crate::i18n` extended to `force_graph`** (session 042, batch 3).
  ~36 keys across the sidebar (Filters / Color Groups / Display /
  Export / Physics sections, all sliders, all toggles) and the
  right-click context menu (Pin/Unpin, Select neighbours, Focus/Clear
  focus, Activate). `ViewerConfig::locale` field
  (`#[serde(default)]`); `GraphViewer::with_locale` / `set_locale` /
  `locale()` builder. `query_hint` (Color Groups secondary line) now
  takes the locale catalogue too — keeps the visible "Query: …"
  hints in sync with the active language. `ColorGroupQuery::All` /
  newly-added groups reuse the localised default name from the
  catalogue (`new_group_default_name`).
- **`crate::i18n` extended to `timeline`, `diff_viewer`, `nav_panel`**
  (session 042, batch 2/N). Three small-surface widgets:
  - `timeline`: 4 tooltip labels (Category / Source / Start-End /
    Depth) localised. `TimelineConfig::locale` field +
    `Timeline::with_locale` / `set_locale` builder. Format-template
    helper `i18n::timeline::start_end(locale, f64, f64)` for the
    Start/End line. Tests: `timeline_strings_resolve`,
    `timeline_start_end_helper_localises`.
  - `diff_viewer`: 2 toolbar buttons ("Prev (Shift+F7)" / "Next (F7)").
    `DiffViewerConfig::locale` + `DiffViewer::with_locale` / `set_locale`.
    Test: `diff_viewer_strings_resolve`.
  - `nav_panel`: 2 panel-toggle tooltips ("Show panel" / "Toggle panel").
    `NavPanelConfig::locale` (functional API — no struct builder; hosts
    set the field directly). Test: `nav_panel_strings_resolve`.
  - Per-widget user labels (`NavItem::label`, `Track::name`,
    `Span::label`, etc.) remain host-driven by design.
- **`crate::i18n` extended to `file_manager` and `tab_control`** (session
  042, batch 1/N). Both widgets had pre-existing English string tables
  (`FmStrings::STRINGS_EN`, `TabStrings` default impl) but no Russian
  catalogue or `Locale` integration — hosts had to wire localisation by
  hand. Now:
  - `file_manager`: `STRINGS_RU` static added; `strings_for_locale(Locale)`
    resolves either catalogue. `FileManagerConfig::locale` field
    (`#[serde(default)]`) carries the choice through ron.
    `FileManager::with_locale` / `set_locale` builders auto-refresh
    `config.strings` when the locale changes, so a config loaded from
    ron with `locale: Ru` already comes up Russian.
  - `tab_control`: `TabStrings::en()` / `TabStrings::ru()` /
    `for_locale(Locale)` constructors. `TabControlConfig::locale` field.
    `TabControl::with_locale` / `set_locale` mirror the file_manager
    API. Existing `TabStrings::default()` still returns English.
  - Forward-compat: both modules' configs accept ron files saved
    before the locale field landed (parsed back as English). Pinned
    by `locale_field_optional_in_ron` tests.
- **DDD-pure structural defaults — composite sub-structs in their own
  ron files** (session 041). Three remaining composite sub-structs
  whose `Default` was hand-coded in Rust now load from sibling ron
  files, mirroring the `TitlebarConfig` ↔ `titlebar_*.ron` pattern
  introduced in session 039:
  - `Buttons` → `app_window/config/buttons.ron`. The three titlebar
    presets (`titlebar_main/tool/dialog.ron`) inline the same field
    set with their own `minimize`/`maximize` overrides; drift is
    guarded by `buttons_inline_in_titlebar_{main,tool,dialog}_matches_canonical`.
  - `ColumnWidths` → `disasm_view/column_widths.ron`. Inlined under
    `columns:` in `disasm_view/config.ron`; drift guarded by
    `columns_inline_in_config_ron_matches_canonical`.
  - `ContextMenuConfig` → `code_editor/context_menu.ron`. Inlined
    under `context_menu:` in `code_editor/config.ron`; drift guarded by
    `context_menu_inline_in_config_ron_matches_canonical`.
  After this pass every composite sub-struct that holds a value-set
  (as opposed to an identity element) lives in ron. Atomic
  enum invariants (`#[default]` on `WindowKind`, `Locale`, …) stay
  in Rust because they're schema, not values; theme palettes
  (`HexViewerColors`, `DisasmViewColors`, etc.) stay in Rust because
  they're derived state through `Theme::*_colors()`; `NodeStyle` /
  `EdgeStyle` stay in Rust because their `Default` is just a thin
  wrapper around `Self::new()` (identity element / alternative
  constructor, no real value-set to externalise).
- **`crate::i18n` — English / Russian localisation for `hex_viewer` and
  `disasm_view`** (session 040). New `i18n::Locale` enum (defaults to
  `En`, `Ru` second variant) carried per-widget. Static `Strings`
  catalogues for both widgets cover every user-visible label, tooltip,
  popup, context-menu entry, settings switch, and search hint —
  ~75 keys total. Format-template helpers
  (`result_n_of_m`, `pattern_too_short`, `copy_n_instructions`)
  cover counts and parameterised messages.
  - Builder API: `HexViewer::new(...).with_locale(Locale::Ru)` /
    `DisasmView::new(...).with_locale(Locale::Ru)`. Mid-flight switch
    via `set_locale(...)`. Default stays English so existing hosts
    are unaffected.
  - Russian deployment requires the host to bake
    `GlyphRanges::Cyrillic` (or a superset) into the active font
    atlas — without it Cyrillic characters render as `?` placeholders.
    Documented on `Locale::Ru`.
  - **Locale lives on the config struct** (`HexViewerConfig::locale`
    and `DisasmViewConfig::locale`), not on the widget itself, so
    `ron::to_string(&cfg)` round-trips the user's choice along with
    every other display setting. The shipped `config.ron` files now
    carry `locale: En`. The field is `#[serde(default)]` — older
    saved configs that pre-date the locale field still parse cleanly
    (forward-compat tests pin this).
- **`crate::utils::popup::action_row_labeled`** — variant of
  `action_row` letting the caller supply the cancel-button label
  alongside the primary label. Used by `hex_viewer` / `disasm_view`
  to feed both buttons from their `i18n` catalogue. The original
  `action_row` keeps its English-default `"Cancel"` for callers
  that don't drive localisation.

### Fixed

- **`Cargo.toml` `[[example]]` declarations restored** (session 042b).
  An earlier refactor in this release dropped the 15 `[[example]]`
  blocks that wire `examples/demo_*.rs` files into `cargo build
  --examples`. Files were untouched on disk but
  `cargo build --examples --all-features` silently no-op'd. Restored
  with the original `required-features` gating so each demo compiles
  only when its widget feature is enabled.
  Also added per-demo locale-switch hints (commented snippets, no
  behavioural change) in `demo_hex_viewer`, `demo_disasm_view`,
  `demo_code_editor`, `demo_file_manager`, `demo_force_graph`, and
  `demo_timeline` so users see the i18n API at a glance.

- **`disasm_view` hover tooltip x64 / x32 consistency** (session 040).
  The instruction-row tooltip historically formatted the address as
  `{:016X}` regardless of `cfg.address_width_64`, and emitted the
  32-bit shadow line whenever `addr <= 0xFFFFFFFF` — including when
  the widget was already in 32-bit mode (where the primary line is
  already 8 digits, so the shadow is redundant). Now the tooltip
  honours both `address_width_64` and `uppercase`:
  - 32-bit mode → 8-digit primary address, no 32-bit shadow line.
  - 64-bit mode → 16-digit primary, plus the 8-digit shadow line
    only when the address fits in `u32::MAX`.
  - Branch target / byte block / hex byte payload all follow the
    same `uppercase` flag instead of hard-coding uppercase.
- **`disasm_view` clipboard payload x32 / case consistency** (session
  040). `Ctrl+C` (multi-instruction copy) used to hard-code uppercase
  byte / address rendering even when `cfg.uppercase = false`. Now
  every field of the clipboard line — address, bytes, comment —
  matches the widget's display configuration.
- **`hex_viewer` per-byte tooltip** now honours `cfg.uppercase` for
  the Hex / Dec / Oct triple (the gutter respected the flag, the
  tooltip didn't).

### Performance

- **Hot-path render audit + cross-module micro-optimisations** (session 039).
  After the Vex0r profiler-FPS investigation surfaced unrelated waste in
  the per-frame chrome, swept every module for the same patterns:
  - **`TitlebarColorsU32` packed-colour cache** (`app_window/chrome`):
    `TitlebarColors` (9 × `[f32;4]`) is now also stored in pre-packed `u32`
    form on the `GpuState`, refreshed alongside `cached_titlebar` on theme
    change. The chrome render previously called `pack_color_f32` 8-12
    times per frame for the same palette — now reads `u32` words
    directly.
  - **`utils::text::line_height(ui)` helper** replaces 9 occurrences of
    `calc_text_size("Mg" | "M" | "A")[1]` across 7 modules
    (`app_window`, `confirm_dialog`, `nav_panel::submenu`, `timeline`,
    `status_bar`, `notifications`, `property_inspector`). Maps to
    `igGetTextLineHeight()` — a direct `ImGuiContext::FontSize` read
    instead of a glyph-walk.
  - **`code_editor` gutter scratch buffer**: `format!("{}", line_idx + 1)`
    fired once per visible row, every frame. Replaced with a pre-allocated
    `String` buffer (`gutter_buf`, capacity 12) reused via `clear()` +
    `write!()` — zero allocations after the first repaint, regardless of
    how many lines are in view.
  - **`code_editor` child-window id cached as `Arc<str>`**: the historic
    `format!("##ce_{}", self.id)` inside `render` allocated a fresh
    `String` every frame. Now built once in `new()`, render does an
    atomic refcount bump.
  - **`timeline` + `property_inspector`**: track / category headers used
    `format!("{} {}", arrow, name)` for every visible row, every frame.
    Replaced with two separate `draw.add_text` calls offset by the
    measured arrow glyph width — no joined string, no heap alloc.

### Added

- **`config.rs` schema / `config.ron` values pattern completed for the last
  two modules** (session 038). `app_window` and `hex_viewer` were the only
  modules still hard-coding default values inside `Default::default()`.
  Both now load defaults from a shipped ron file:
  - `hex_viewer/config.ron` carries the 22 non-color fields; the 18
    theme-driven `color_*` fields stay `#[serde(skip)]` and are populated
    from `HexViewerColors::default()` after ron parse.
  - `app_window/config/default.ron` carries the full `AppConfig` schema;
    `font` and `window_icon` (which hold `Arc<[u8]>` / RGBA pixel buffers)
    are `#[serde(skip)]` and fall back to type-default. The `chrome` block
    is inlined because ron 0.8 has no `include`.
  - `app_window/config/titlebar_main.ron`, `titlebar_tool.ron`,
    `titlebar_dialog.ron` carry the three titlebar presets.
  - `TitlebarConfig::default/tool/dialog` and `AppConfig::default` now do
    `ron::from_str(include_str!(...))`; `splash/tool/dialog/main` window
    presets remain Rust-side methods because they're combinations, not
    values. Round-trip + drift-detection tests added in
    `app_window/config/mod.rs::tests`.
  - `ExtraButton` is excluded from ron (it holds `&'static str` for
    zero-allocation dispatch); `TitlebarConfig::extras` is
    `#[serde(skip, default)]`. Hosts add extras through the builder API.
- **`serde` + `ron` config serialization across all modules** (session
  036). Every config struct now derives `serde::Serialize, serde::Deserialize`.
  Each module ships a `config.ron` file (embedded via `include_str!`) that
  holds the canonical default values; `Default::default()` loads from it via
  `ron::from_str`. This enables round-trip save/restore of any widget config
  with zero hand-written code: `ron::to_string(&cfg)` / `ron::from_str(&s)`.

### Changed

- **Config schemas separated from logic** (session 036). Non-config code
  extracted from `config.rs` into dedicated submodules to keep each file
  under 500 lines:
  - `disasm_view`: `provider.rs` (FlowKind, Instruction, DisasmDataProvider,
    InstructionEntry, VecDisasmProvider), `arrows.rs` (BranchArrow,
    MAX_ARROW_DEPTH, compute_arrows, compute_arrows_clipped).
  - `hex_viewer`: `provider.rs` (HexDataProvider, VecDataProvider,
    ColorRegion, ByteCategory), `nav_history.rs` (NavHistory), `undo.rs`
    (UndoEntry, UndoStack).
  - `code_editor`: `syntax_colors.rs` (EditorTheme, SyntaxColors),
    `font_setup.rs` (CODE_EDITOR_FONT_PTR, install helpers).
  - `tab_control`: `types.rs` (TabId, TabStatus, Badge, CloseGlyph, TabStyle,
    TabAction), `colors.rs` (TabColors), `strings.rs` (TabStrings).
  - `nav_panel`: `buttons.rs` (SubMenuItem, NavButton, NavItem),
    `enums.rs` (DockPosition, ButtonStyle, ActiveStyle).
  - `notifications`: `enums.rs` (Severity, Placement, Duration,
    AnimationKind), `notification.rs` (Notification, NotificationAction).
  All existing public re-exports are preserved — downstream code unchanged.

- **`tab_control::TabStrings` fields changed from `&'static str` to
  `String`** (session 036). Required for RON deserialization. Call sites
  updated to pass `&field` where `&str` is expected.

- **`FpsMode` now derives `Copy`** (session 037). All three variants (`Auto`,
  `Fixed(u32)`, `Unlimited`) are inherently `Copy`-able. Two stale `.clone()`
  calls in `builders.rs` and `enums.rs` replaced with deref-copy `*fps_mode`.

- **`frame_latency` dead parameter removed** (session 037). `gpu/setup.rs`
  internal helper always returned `2` regardless of the `PowerMode` argument.
  Removed the parameter; call site updated. Comment explains the rationale.

- **Zero-allocation hot-path in `disasm_view::tokens`** (session 038).
  `classify_operand_token` no longer calls `to_ascii_lowercase()` (which heap-
  allocates a `String` per token on every render frame). Size keywords now
  tested via `eq_ignore_ascii_case`; the register check uses a 16-byte stack
  buffer for the case-fold instead of a heap-allocated `String`.

- **`format_bytes` in `hex_viewer::search` rewritten** (session 038).
  `HexSpaced`, `HexCompact`, `CArray`, and `RustArray` arms replaced the
  `map → collect::<Vec<_>>() → join` chain with direct `write!` into a
  pre-allocated `String`, eliminating the intermediate `Vec<String>`.

- **`disasm_view` ImGui IDs fully cached** (session 038). The child-window
  ID (`##dv_child_*`) is now built once in `DisasmView::new` and stored as
  `child_id` alongside the other cached IDs, removing the last per-frame
  `format!` call in `render()`. The now-redundant private `id: String` field
  is removed from the struct.

- **`col32` centralised in `crate::utils::color`** (session 038). Three
  independent copies of the helper (one in each draw module) collapsed into
  a single `pub(crate) fn col32` re-export of `rgba_f32`.

- **Popup-anchor helpers moved to `crate::utils::popup`** (session 038).
  `anchor_next_popup_at / _topleft / _centred` were duplicated verbatim in
  both `hex_viewer::popup` and `disasm_view::popup`; both modules now import
  from the shared location.

- **`disasm_view::popup` search hint is a `const`** (session 038). The label
  `"Search bytes (min 5 bytes; ?? wildcard):"` is stored as `SEARCH_HINT:
  &str` instead of being formatted anew on every frame while the popup is
  open.

- **`arrows.rs` lane array uses `std::array::from_fn`** (session 038).
  `vec![Vec::new(); MAX_ARROW_DEPTH]` → `std::array::from_fn(|_| Vec::new())`
  for the fixed-size 6-slot depth-lane array, replacing `N` heap allocations
  for the `Vec` wrapper with a single stack allocation.

- **`app_window` module split** (session 036). `mod.rs` (679 → 227 lines)
  and `config/mod.rs` (748 → 313 lines) extracted into dedicated submodules
  to stay under the 500-line rule:
  - `startup.rs` — GPU + ImGui init (`resumed` body).
  - `dispatch.rs` — per-frame event dispatch (`window_event` + `about_to_wait`).
  - `config/fonts.rs` — `FontChoice`, `FontLayer`, `GlyphRanges`.
  - `config/builders.rs` — all `impl AppConfig { pub fn with_* }` builder methods.
  All public re-exports unchanged.

- **`disasm_view` bookmarks** (session 033). Up to 64 addresses, 7
  public methods (`is_bookmarked` / `add_bookmark` / `remove_bookmark`
  / `toggle_bookmark` / `bookmarks` / `bookmark_count` /
  `clear_bookmarks`), `Ctrl+B` hotkey, gutter glyph, state-aware
  context-menu entry. Default glyph: MDI `BOOKMARK_CHECK_OUTLINE`
  (with `\u{25CB}` ring fallback when `icons_available = false`).
  +8 unit tests.
- **`disasm_view` watchpoint** (session 035). `Instruction::has_watchpoint`
  + `DisasmDataProvider::toggle_watchpoint` (default no-op false).
  Single `RW` glyph in the gutter; one "Toggle Watchpoint"
  context-menu entry. Hosts that distinguish read-only vs write-only
  data breakpoints sort that on the engine side and report the union
  back through `has_watchpoint()`.
- **`disasm_view` colour-coded context menu** (session 035). Each
  entry tinted by action class: navigation (address blue), follow
  (call green), function nav (jump amber), breakpoint (red),
  watchpoint (orange), bookmark (accent), settings (default). Same
  pattern landed in `hex_viewer` (nav entries on `color_offset`).
- **`disasm_view` two-pass current-line marker** (session 035).
  Translucent `current_line_bg` fill (alpha 0.09) + 1-px
  `current_line_border` outline (red, alpha 0.90) — replaces the
  prior solid amber bar that was too saturated.
- **`disasm_view` gutter split** (session 035). Margin column now
  reserves a 3-px inset on each edge with a 2-px centre gap; left
  half = bookmark glyph, right half = `RW` glyph or breakpoint
  number. `cols.address` widened (130 → 150 px), `cols.comment`
  shrunk (120 → 100 px), `cols.margin` (14 → 26 px).
- **`disasm_view` host-toolbar helpers** (session 032). Five
  selectors (`select_current_ip`, `select_first_breakpoint`,
  `select_next_breakpoint`, `select_prev_breakpoint`, `cursor_address`)
  + `can_nav_back / can_nav_forward` predicates so hosts can render
  toolbar buttons without scanning the provider themselves.
- **`tab_control::TabItem::text_color()`** (session 032). Per-tab
  RGB override for the tab title text — wins over the palette;
  defaults `None` (use palette colour).
- **`tab_control` body-frame model** (session 035). Active tab's
  `render_content()` now runs inside an outer + inner rectangle
  pair: outer drawn with `colors.frame_bg` directly on the parent's
  draw list, inner is a real borderless `child_window` filled with
  `colors.body_bg` and clipping host widgets to the inset. New
  config fields: `body_inset_enabled`, `body_inset` ([4.0, 4.0]
  default), `body_inset_border` (opt-in active-pane outline,
  default off), `body_inset_border_thickness` (1.5 px). New palette
  fields: `frame_bg`, `frame_border`. `body_bg` default flipped to
  slightly lighter than `strip_bg` so the inset gap registers as
  a visible frame.
- **`virtual_table::TableConfig::show_headers`** (between sessions
  031 and 032). Opt-out for the header strip; default `true`.

### Changed

- **`hex_viewer` config gains `pub icons_available: bool`** (default
  `true`, session 035). Settings popup uses MDI `wrench-cog`
  glyph (`U+F1B91`) when set, falls back to `\u{2026}` ellipsis
  otherwise. Drift with `disasm_view::DisasmViewConfig::icons_available`
  resolved.
- **`disasm_view::DisasmViewConfig::icons_available` default flipped
  to `true`** (session 035, was `false`). Bookmark gutter glyph and
  Settings popup icon now render their MDI variants by default —
  every in-tree consumer ships the MDI atlas.
- **`tab_control` config field rename**: `content_padding_enabled`
  → `body_inset_enabled`, `content_padding` → `body_inset`,
  `TabColors::content_bg` → `body_bg`. Diagram-aligned naming
  (outer / inner / pad).
- **`tab_control::TabColors::frame_bg` separated from `strip_bg`**
  (session 035). Recolouring the body-frame gap no longer
  recolours the tab strip itself.
- **`tab_control` snap-scroll smoother on tab activation** (session
  035). `SMOOTH_SCROLL_COEF` bumped 14.0 → 28.0 (faster ease, no
  hard snap). Hard-snap lines on `set_active` removed.
- **Performance**: `disasm_view` per-row layout pre-pass switched to
  `.len()` on ASCII mnemonic / operand strings (was `chars().count()`);
  per-row comment formatting split into two `add_text` calls (no
  `format!` allocation); `hex_viewer` address gutter formats into a
  thread-local scratch `String` (~3000 alloc/sec saved); `child_id` /
  `splitter_id` cached on `HexViewer` struct.

### Fixed

- **`disasm_view` watchpoint glyph colour** (session 035). The label
  was tinted with `bp_color()` regardless of label kind; now uses
  `operand_memory` for watchpoints and `bp_color()` for breakpoint
  digits.
- **`disasm_view` + `hex_viewer` Settings glyph fallback** (session
  035). The MDI `wrench-cog` (`U+F1B91`) is now gated by
  `icons_available` with a `\u{2026}` ellipsis fallback so the entry
  never renders as `?` on hosts without the MDI atlas.
- **`hex_viewer` Ctrl+A select-all anchor** (session 035). Cursor
  now moves to the end of the selection so a subsequent
  `shift+arrow` re-anchors at the new cursor instead of silently
  shrinking the selection.
- **`tab_control` body-frame flash on degenerate size** (session
  035). When the inner rectangle would degenerate (`<= 1 px`), the
  outer `frame_bg` rect is also skipped, falling through to a plain
  `render_content()` — eliminates the frame-coloured flash around
  user content.
- **`disasm_view` arrow underflow on shrinking provider** (session
  035). `first_row` is now clamped to `provider.instruction_count()`
  before `compute_arrows_clipped` consumes the visible window —
  protects from underflow when the provider shrinks between frames.
- **`hex_viewer` context-menu entries close popup on activation**
  (session 035). `Goto`, `Search`, `Step back`, `Step forward`, and
  `Settings` now call `close_current_popup()` on click, mirroring
  the `disasm_view` pattern.

### Tests

- **670 lib tests passed** (was 631 at 0.10.0 release; +39 net
  across sessions 032-035): bookmark suite (8), content_bg /
  body_bg invariants, host-toolbar helpers, watchpoint round-trip
  + breakpoint independence, body_inset_border defaults,
  `SMOOTH_SCROLL_COEF` pin, `HexSearchMode::Hex` round-trip,
  `icons_available` defaults, Ctrl+A cursor placement, `first_row`
  clamp arithmetic, `format_bytes` coverage of all `CopyFormat`
  variants. 0 clippy warnings under `-D warnings --all-targets`.

## [0.10.0] - 2026-04-30

This is a **major BREAKING release** consolidating the work of 14
sessions (017–031). The crate is now a single host framework
(`app_window`, was 3), with a unified theme system that every built-in
widget participates in, and a quality bar of 0 clippy warnings + 631
green library tests + 0 unsafe blocks added in this cycle.

### Highlights

- **Single host framework.** `app_window` v1, `borderless_window` and
  `app_window_v2` collapsed to one (session 026 — net −4 464 LoC).
- **Unified theme system.** All 16 colour-bearing widgets expose
  `Theme::*_colors()` / `*_config()` accessors and / or `with_theme()`
  builders (sessions 022, 023, 027, 031). No widget reads
  `crate::theme::*` legacy constants in its render path anymore.
- **Major audit waves.** 8 critical bugs closed (sparse-provider
  `binary_search`, fn-start clamp, swap-chain `Suboptimal`, click
  bleed-through, u64-wrap collision, span_color O(N²), code_editor
  multibyte fold-badge, Myers 40 GB cap) + ~25 mediums fixed in
  sessions 029–030.
- **`disasm_view` feature blast.** 8 user-driven feature batches
  (byte search, function nav, follow-at-cursor, origin breadcrumb,
  address copy, dynamic columns, settings popup, branch arrows
  clipped) — session 029.
- **`tab_control` widget added** as a polished replacement for
  `page_control` (32 unit-tests, drag-and-drop, hover-preview,
  pinned-prefix invariant) — session 024.
- **`force_graph` Phase D shipped** (time-travel, minimap, SVG/DOT/
  Mermaid export) — session 018.
- **`proc_mon` widget removed** — process enumeration moved to a
  separate user-owned crate, this lib is now exclusively UI mods
  (session 028 — net −1 744 LoC).

### BREAKING changes

(Aggregated across 017–031.)

- **`app_window` v1 + `borderless_window` removed** (session 026).
  Migration: `crate::borderless_window::*` → `crate::app_window::*`,
  `BorderlessConfig` → `AppConfig`, `BorderlessApp` → `AppHandler`.
- **`app_window_v2` renamed to `app_window`**, `*V2` suffixes dropped
  on 26 types (session 026).
- **`AppConfig.render_mode: RenderMode`** replaces 4 historical fields
  (`fps_mode`, `unfocused_fps`, `event_driven`, `idle_pulse`) — session
  019.
- **`PowerMode::Auto` removed** — was a duplicate alias for
  `HighPerformance`; migrate to `PowerMode::HighPerformance` or
  `PowerMode::default()` (session 023).
- **`HexViewerConfig.config` privatised** — use `config()` /
  `config_mut()` accessors (session 025).
- **`nav_panel` Cow migration** — string fields now `Cow<'static, str>`
  for zero-copy default labels (session 018).
- **`knowledge_graph` legacy alias removed** — use `force_graph`
  directly (session 019).
- **`DisasmDataProvider::refresh()` removed** — orphan trait method
  with no callers (session 028).
- **`proc_mon` widget removed entirely** — `proc_mon` Cargo feature
  is gone, `dep:proc_enum` and `dep:serde` no longer pulled
  transitively (session 028).
- **`app_window::chrome::whole_window_resize`** now returns
  `TitlebarResult` (was `(Option<ResizeEdge>, TitlebarAction)` tuple)
  — session 030.
- **`TitlebarColors` orphan fields removed** (`bg_erase`, `drag_hint`,
  `bg_inactive`, `title_inactive`) — defined in all 7 themes for years
  with zero in-tree consumers; matching `TITLE_INACTIVE_BG` constants
  in 5 theme modules also removed (session 030).
- **`SyntaxColors` gained four new fields** (`breakpoint`,
  `gutter_separator`, `cursor`, `whitespace_marker`) — session 031.
  Manual constructors must be updated; `SyntaxColors::dark_default()`
  / Monokai / OneDark / SolarizedDark / SolarizedLight / GithubLight
  factories all add the new fields. Two new presets added:
  `SyntaxColors::catppuccin()` / `nord()`. `EditorTheme::ALL` extended
  to 8 variants (added `Catppuccin` + `Nord`).
- **`tab_control::LABEL_COLOR` const removed** (was internal but
  some consumers grepped for it) — replaced with
  `cfg.colors.text_muted` lookups.

### Theme integration ecosystem

The unified theme contract now covers every built-in widget that
paints colour surfaces. New accessors (session 031):

| Accessor | Returns | Pattern |
|---|---|---|
| `Theme::code_editor_colors()` | `SyntaxColors` | crate-theme → `EditorTheme` map |
| `Theme::diff_viewer_config()` | `DiffViewerConfig` | tokens → `with_theme()` |
| `Theme::force_graph_colors()` | `GraphColors` | tokens → `from_theme()` |
| `Theme::node_graph_colors()` | `NgColors` | tokens → `from_theme()` |
| `Theme::timeline_config()` | `TimelineConfig` | tokens → `with_theme()` |
| `Theme::toolbar_config()` | `ToolbarConfig` | tokens → `with_theme()` |
| `Theme::inspector_config()` | `InspectorConfig` | tokens → `with_theme()` |

(Joining the existing `titlebar()` / `nav()` / `dialog()` /
`notifications()` / `statusbar_colors()` / `statusbar()` /
`hex_viewer_colors()` / `disasm_view_colors()` / `tab_colors()` from
sessions 022–028.)

Each colour-bearing widget gains a matching builder
(`XxxConfig::with_theme(theme)` or `XxxColors::from_theme(theme)`),
so a host that switches `Theme::Dark → Theme::Nord` propagates the
change across every chrome surface in one pass.

### Performance + safety wins (sessions 029–031)

- `disasm_view::do_search` sparse-provider correctness — `partition_point`
  on `(byte_offset, global_idx)` pairs (session 029).
- `disasm_view::find_function_start` bogus clamp removed (session 029).
- `app_window` `Suboptimal` swap chain reconfigures (was painting
  stale frame, session 029).
- `notifications` click bleed-through latch + `next_id` u64-wrap
  collision fix (session 029).
- `timeline::span_color` hoisted `data_time_range` (was O(spans²) in
  `ColorMode::ByDuration`, session 030).
- `force_graph` search-highlight allocation eliminated (saves N allocs
  per frame, session 030).
- `code_editor` fold-badge multibyte fix — `chars().count()` instead
  of `len()` (session 030).
- `diff_viewer` Myers memory hard cap `MAX_DIFF_INPUT_LINES = 20_000`
  — historic `max_d = 50_000` could allocate ~40 GB on 100k-line files
  (session 030).
- `diff_viewer` unsafe `from_raw_parts` replaced with safe split
  borrows (session 030).

### Misc

- `SyntaxColors::catppuccin()` + `nord()` factories added — Mocha
  pastels and Polar-Night frost-blues for the editor.
- `NotificationColors::catppuccin()` + `nord()` factories added —
  no longer fall back to Monokai/Midnight (whose hue families clashed
  visually).

[0.10.0]: https://github.com/Platon7788/Dear-imgui-custom-mods/releases/tag/v0.10.0

## [0.9.0] — earlier

(Earlier sessions: see commit history for detail. The crate has
been on master only since session 016; pre-master tags are not
maintained.)

### Session 031 — theme integration pass for 8 widgets (2026-04-30)

The largest deferred items from session 030 — eight widgets carrying
widget-local color state with no `Theme` accessor — are now wired into
the unified theme system. Every built-in widget that paints colour
surfaces can now flip palettes in lockstep with `Theme::Dark` / `Light`
/ `Midnight` / `Solarized` / `Monokai` / `Catppuccin` / `Nord`.

**Library tests: 624 → 631 (+7). Clippy: 0 warnings under `-D warnings`.**

#### `code_editor`

- **Removed the last legacy `theme::*` constants from the render path.**
  Previously `mod.rs` reached into `crate::theme::{DANGER, SEPARATOR,
  TEXT_PRIMARY, TEXT_MUTED}` for the breakpoint marker, gutter divider,
  primary cursor, and whitespace markers — these read as hard-pinned
  `Theme::Dark` colours regardless of the active theme. All five sites
  now use new typed `SyntaxColors` fields.
- New `SyntaxColors` fields: `breakpoint`, `gutter_separator`, `cursor`,
  `whitespace_marker`. Filled in for all six existing presets
  (DarkDefault / Monokai / OneDark / SolarizedDark / SolarizedLight /
  GithubLight) and two new ones — `SyntaxColors::catppuccin()` /
  `nord()`. `EditorTheme::ALL` extended to 8 variants; `display_name` /
  `colors()` updated.
- New `EditorTheme::from_crate_theme(crate::theme::Theme) -> EditorTheme`
  maps the host theme to the closest editor preset (`Dark→DarkDefault`,
  `Light→GithubLight`, `Midnight→OneDark`, `Solarized→SolarizedDark`,
  `Monokai→Monokai`, `Catppuccin→Catppuccin`, `Nord→Nord`).
- New `EditorConfig::set_crate_theme(theme)` /
  `EditorConfig::with_crate_theme(theme)` — the editor now switches
  syntax palettes with one call from a host that only knows about the
  crate-wide `Theme`.
- New `Theme::code_editor_colors() -> SyntaxColors` accessor.

#### `diff_viewer`

- New `DiffViewerConfig::apply_theme(crate::theme::Theme)` /
  `with_theme(theme)` — the 14 `color_*` fields are derived from
  `nav.bg` / `nav.icon_*` / `theme.success()` / `danger()` / `warning()`
  / `accent()`, so an added line reads as `theme.success()` and removed
  as `theme.danger()` on every theme. Layout fields untouched.
- New `Theme::diff_viewer_config()` accessor — returns a
  fully-themed `DiffViewerConfig`.

#### `tab_control`

- Removed `LABEL_COLOR` module-level const (used by 3 sites) — replaced
  with `cfg.colors.text_muted` so empty placeholder, no-tabs strip and
  close-confirm popup track the active theme.
- Hardcoded RGBA in close-confirm popup
  (`[0.88, 0.90, 0.92, 1.0]` text, `[0.70, 0.22, 0.22, 1.0]` /
  `[0.82, 0.30, 0.30, 1.0]` / `[0.60, 0.18, 0.18, 1.0]` button stack)
  replaced with `cfg.colors.text` / `text_muted` references and the
  graduated `crate::utils::popup::danger_button` helper. The 3-call
  `push_style_color(StyleColor::Button{,Hovered,Active})` boilerplate
  is gone — a future tweak to the destructive-button hue family
  propagates to the popup automatically.
- Empty-placeholder hint text uses `cfg.colors.text_muted` at α 0.7
  (was hardcoded grey).

#### `force_graph`

- New `GraphColors::from_theme(crate::theme::Theme) -> GraphColors`
  factory — synthesizes the 11-field palette from `nav` + `accent` +
  `window_bg()` so the canvas, node fills, edges, label text and box
  selection all sit in the same visual family as the rest of the
  chrome. `node_default` darkens the accent, `node_hover` is the
  accent itself, `node_selected` lifts it brighter, edges read as
  `nav.icon_default` (default) / `accent` (drag).
- `ViewerConfig::with_theme(theme)` builder — sets `theme` and clears
  `colors_override` so the renderer derives the palette via
  `from_theme` automatically.
- **`render::main` palette resolution rewired.** Was falling back to
  `GraphColors::default()` (Dark-pinned) when `colors_override` was
  `None`; now uses `GraphColors::from_theme(config.theme)` so the
  graph respects the active theme by default. Hosts still get
  full override via `colors_override`.
- New `Theme::force_graph_colors()` accessor.

#### `node_graph`

- New `NgColors::from_theme(crate::theme::Theme) -> NgColors` factory
  — synthesizes the 22-field palette (canvas + grid + node body /
  header / border, pin default + hover, wires default / hover / drag,
  selection rect, minimap surfaces, collapse button) from the crate
  theme. The historic `NgColors` type uses `[u8; 3]` — the factory
  rounds + clamps the float palette tokens for that on-the-wire
  format.
- New `NodeGraphConfig::apply_theme(theme)` / `with_theme(theme)`
  builders.
- New `Theme::node_graph_colors()` accessor.

#### `timeline`

- New `TimelineConfig::apply_theme(theme)` / `with_theme(theme)` —
  the 12 `color_*` fields (bg, alt-stripe, ruler bg+text, track
  label / separator, span text, selection / hover, marker, tooltip
  bg + text) sync with the theme. The per-span hue rotation
  (`span_palette`) stays theme-independent so flame-chart hues read
  identically across themes.
- New `Theme::timeline_config()` accessor.

#### `toolbar`

- New `ToolbarConfig::apply_theme(theme)` / `with_theme(theme)` —
  every `color_*` field maps to `nav.{bg,icon_*,btn_hover,btn_active,
  separator}` + `theme.accent()`. A horizontal toolbar now reads as
  the same chrome surface as the vertical [`crate::nav_panel`].
- New `Theme::toolbar_config()` accessor.

#### `property_inspector`

- New `InspectorConfig::apply_theme(theme)` / `with_theme(theme)` —
  bg / alt-row / key / value / readonly / category-header surfaces
  derive from `nav` + `statusbar` + semantic tokens.
- New `Theme::inspector_config()` accessor.

#### Theme accessor API (overall)

The set of `Theme::*_colors()` / `*_config()` accessors is now:

| Accessor | Type | Source module | Pattern |
|---|---|---|---|
| `titlebar()` | `TitlebarColors` | `palettes` | per-theme factory |
| `nav()` | `NavColors` | `palettes` | per-theme factory |
| `dialog()` | `DialogColors` | `palettes` | per-theme factory |
| `notifications()` | `NotificationColors` | `palettes` | named-preset constructor |
| `statusbar_colors()` | `StatusBarColors` | `palettes` | per-theme factory |
| `statusbar()` | `StatusBarConfig` | `status_bar` | per-theme factory |
| `hex_viewer_colors()` | `HexViewerColors` | `palettes` | tokens factory |
| `disasm_view_colors()` | `DisasmViewColors` | `palettes` | tokens factory |
| `tab_colors()` | `TabColors` | `tab_control` | derived from `nav + statusbar` |
| `code_editor_colors()` | `SyntaxColors` | `code_editor` | crate-theme→preset map |
| `diff_viewer_config()` | `DiffViewerConfig` | `diff_viewer` | `with_theme()` builder |
| `force_graph_colors()` | `GraphColors` | `force_graph::style` | `from_theme()` factory |
| `node_graph_colors()` | `NgColors` | `node_graph::config` | `from_theme()` factory |
| `timeline_config()` | `TimelineConfig` | `timeline` | `with_theme()` builder |
| `toolbar_config()` | `ToolbarConfig` | `toolbar` | `with_theme()` builder |
| `inspector_config()` | `InspectorConfig` | `property_inspector` | `with_theme()` builder |

#### Tests

- 7 new `theme::tests::{code_editor,diff_viewer,force_graph,
  node_graph,timeline,toolbar,inspector}_*_resolves_for_every_theme`
  pin per-theme invariants (non-zero alphas, key/value distinctness,
  `bg == window_bg` for force_graph canvas, `bg == nav.bg` for
  toolbar surface, etc.).

### Session 030 — full-codebase audit, deferred fixes, 7 more criticals (2026-04-30)

Autonomous overnight session. Six parallel `code-analyzer` agents
swept the remaining widgets (`code_editor`, `diff_viewer`,
`file_manager`, `nav_panel`, `virtual_table`, `virtual_tree`,
`timeline`, `toolbar`, `tab_control`, `status_bar`, `force_graph`,
`property_inspector`, `confirm_dialog`, `input`) plus the
foundation (`theme`, `utils`, `lib.rs`, `clipboard_backend`,
`frame_demand`, `icons`, `fonts`). 150+ findings; criticals +
high-impact mediums fixed in this commit.

**Library tests: 696 (held). Clippy: 0 warnings throughout.**

#### Critical fixes (7 new bugs)

- **`disasm_view::do_search`** — sparse provider correctness:
  rebuilt `(byte_offset, global_idx)` table using
  `partition_point` last-le / first-ge semantics. Previous
  `binary_search` on duplicate offsets (from `None` instructions)
  could map a hit into the wrong row.
- **`disasm_view::find_function_start`** — dropped bogus
  `(i + 1).min(count - 1)` clamp that returned the
  previous-function RET when the cursor was on the very last
  instruction.
- **`timeline::span_color`** — was calling `data_time_range()`
  per span in `ColorMode::ByDuration` (`O(spans × spans)` per
  frame). Hoisted the range into the render loop, plumbed it
  through `span_color(..., data_range)`. Tests updated.
- **`property_inspector::render`** — documented that the returned
  `Vec<PropertyChangedEvent>` is **always empty** today
  (read-only renderer; edit widgets unimplemented). API signature
  preserved for forward-compat. Stops users wiring change handlers
  that silently never fire.
- **`force_graph::render` search-highlight** — was calling
  `node.style.label.to_ascii_lowercase()` per node per frame.
  Pre-lowered query once outside the loop, added
  `contains_ignore_ascii_case` helper that scans without
  allocation. Saves N allocs per frame on graphs with active
  search.
- **`code_editor` fold-badge x-position** — used `line_str.len()`
  (bytes) when `chars().count()` was needed. Badge drifted by
  N bytes for any UTF-8 multibyte source line (Cyrillic, CJK,
  emoji). Same fix for `badge_w`.
- **`diff_viewer::diff_lines` Myers memory** — added hard cap
  `MAX_DIFF_INPUT_LINES = 20_000`; above the threshold falls
  back to a "delete-all-then-insert-all" coarse diff so the
  algorithm can't allocate gigabytes for the trace Vec on huge
  inputs. (Historic 50k cap on `max_d` could allocate ~40 GB
  on 100k-line files.)
- **`diff_viewer` unsafe slice trick** — replaced the
  `from_raw_parts` block in side-by-side render with plain
  `&self.left_lines` / `&self.right_lines` immutable borrows.
  The original SAFETY note ("not mutated during render") was
  correct but unenforceable across future refactors.

#### Deferred items from session 029 — done

- **`notifications`** — pre-formatted action button labels +
  close-button ID at `push()` time (was per-frame `format!` per
  visible toast per action). Threaded `est_h` from layout pass
  into `render_toast` so `estimate_height` runs once per toast
  per frame (was twice).
- **`notifications`** — added native `NotificationColors::catppuccin()`
  and `NotificationColors::nord()` palettes, wired them into
  `Theme::notifications()`. Catppuccin/Nord no longer fall back to
  Monokai/Midnight (whose hue families clashed visually).
- **`app_window::chrome::whole_window_resize`** — signature
  unified to `TitlebarResult` (was `(Option<ResizeEdge>,
  TitlebarAction)` tuple — caller in `gpu/mod.rs` immediately
  re-packed it into a `TitlebarResult` anyway). API drift gone.
- **`app_window`** — removed orphaned `TitlebarColors` fields
  `bg_erase`, `drag_hint`, `bg_inactive`, `title_inactive` —
  defined by all 7 themes for years, zero in-tree consumers.
  All 7 theme files updated; unused `TITLE_INACTIVE_BG`
  constants in 5 themes removed alongside.
- **`node_graph` viewer hooks** — documented `on_connect` /
  `on_disconnect` / `node_tooltip` / `input_tooltip` /
  `output_tooltip` as **NOT WIRED YET** with concrete pointers
  to `GraphAction::Connected` / `Disconnected` events as
  current-API equivalents. Implementation tracked as
  next-session deferred (requires `&mut dyn` viewer
  signature change + tooltip pass after hover-tracking
  block).

#### Audit findings deferred to next session

(150+ findings in total — only criticals + key mediums fixed in
this batch. Below is the prioritised follow-up list.)

- `code_editor`: legacy `crate::theme::{DANGER, SEPARATOR,
  TEXT_PRIMARY, TEXT_MUTED}` constants used in 8+ sites — break
  on Light/Solarized themes. Needs `SyntaxColors` extension +
  per-theme `code_editor_colors()` accessor.
- `diff_viewer`: zero `crate::theme` integration, hardcoded
  hover/accent colours, missing `MiniMap`. Needs
  `DiffViewerColors` palette + `Theme::diff_viewer_colors()`.
- `tab_control`: hardcoded RGBA in close-confirm popup.
- `timeline`, `toolbar`, `property_inspector`, `node_graph`,
  `force_graph`: each carries widget-local colour state with no
  `Theme` palette accessor.
- `virtual_table` ring-buffer head invariant fragility,
  scroll-snap centre math, sort clears selection.
- `virtual_tree` tree-line continuation depth limit (64),
  drag-drop reparent-only (no sibling reorder).
- `notifications` `estimate_height` line-count math is wrong
  for wrapped text; hard-coded pixel metrics break under DPI
  scaling; theme.rs is a 16-line stub.
- `force_graph` Barnes-Hut allocations per tick;
  collision pass O(N²); community placeholder file.
- `file_manager` Type-to-search ASCII-only; UTC mtime label
  with no zone suffix.
- `nav_panel` submenu hit-test ignores window occlusion.
- `app_window` per-frame `Vec<u32>` scratch in font rebuild;
  `pending_frames` u8 saturation; `subclass_proc` four-call
  `DefSubclassProc` redundancy.
- `disasm_view` tooltip path allocates 5+ `String`s per hover
  frame; `compute_arrows_clipped` O(N²) on `VecDisasmProvider`
  for very large buffers; `frame_comment_x` `<= 0.0` sentinel
  fragility.

### Session 029 — disasm_view feature blast + 4-module audit (2026-04-30)

Single-session sweep that landed 8 user-driven feature batches plus a
quality audit of `app_window`, `disasm_view`, `node_graph`, and
`notifications` (82 findings; criticals + key mediums fixed).
Library test count: **655 → 696** (+41), clippy: 0 warnings throughout.

#### `disasm_view` features

- **Per-byte category colouring** in the Bytes column — same 5-tier
  `ByteCategory` split (zero / control / printable / high / `0xFF`)
  hex_viewer uses, so the same buffer reads identically across both
  widgets. New `DisasmViewColors::byte_fg_color()`, new config flag
  `DisasmViewConfig::byte_category_colors` (default `true`).
- **Byte search** — wildcard hex pattern (`4D 5A ?? 00 89`),
  Ctrl+F to open, F3 / Shift+F3 to step matches, minimum 5 bytes.
  Cross-instruction matches supported (concatenated byte stream
  scanned by `crate::hex_viewer::search::find_pattern_masked`,
  promoted from `pub(super)` to `pub(crate)`). Search popup mirrors
  hex_view's geometry. New palette field
  `DisasmViewColors::search_match_bg`.
- **Function navigation** — `find_function_start` / `find_function_end`
  free helpers (RET-based heuristic), public methods
  `select_function`, `jump_to_function_start`, `jump_to_function_end`.
  Hotkeys: `Ctrl+Up`, `Ctrl+Down`, `Ctrl+L`. Context-menu entries.
- **Follow at cursor** — `follow_at_cursor()` tries `branch_target()`
  first, then scans operand `Number` tokens for resolvable
  addresses. Lazy `decode_range` retry for streaming providers (call
  targets outside the currently-decoded window now navigate
  correctly). Triggers: `Enter`, `Space`, double-click in
  Instruction column.
- **Address-column double-click copy** — Hand cursor + tooltip on
  hover, double-click copies via `format_address_literal`,
  flash-pill animation (mirrors `hex_viewer::address_flash`).
  hex_viewer's address copy promoted from single-click → double-click
  for parity (single-click felt accidental).
- **Origin breadcrumb** — soft "you came from here" highlight on the
  previous cursor row after navigation (Goto / Follow / function-jump
  / nav-back / search). Two-tier: faint background fill (alpha 0.30)
  + 3-px crisp left-edge stripe (alpha 0.90) — modern bookmark
  pattern, distinct from cursor (full-row solid) and current
  execution (warning hue). Stored as **address** so it survives
  provider mutation. Cleared on `Esc` or new navigation; preserved
  across single-click and arrow movement (user explicitly wants
  to scroll/click around without losing the breadcrumb).
- **Branch-arrow improvements** — new `compute_arrows_clipped()`
  scans cross-window so long jumps with off-screen endpoints stay
  visible (clamped to window edge with `clipped_from`/`clipped_to`
  flags; renderer suppresses arrowhead and stub at clipped ends).
  Pass-through arrows (source above + target below) preserved.
  Priority sort `(anchored, half-clipped, pass-through)` so
  `truncate(max_arrows)` drops least-informative first.
  `max_arrows` default bumped 64 → 256 (heavily-jumped functions
  no longer hit the cap).
- **Settings popup + reordered context menu** — Goto / Search /
  Copy / Follow / function nav / breakpoint / Settings.
- **Column widths** — Bytes 200, Instruction 300 (mnemonic 80 +
  operands 220), Comment dynamic (`frame_comment_w` per-frame:
  fills remaining window width down to a `cols.comment` floor).
- **Header label colour fix** — `DisasmViewColors::header` flipped
  from `fg_muted × 0.85` → `t.fg` (white in dark themes / bold
  dark in light) — matches the hex_view header treatment.

#### `disasm_view` audit fixes (this pass)

- **CRIT** `do_search` — sparse providers (lazy decode, gaps in
  `[0..count)`) produced wrong row mappings due to duplicate
  byte-offset entries when `provider.instruction(i) == None` and
  `binary_search` returning any of the duplicates. Rewrote to use
  `(byte_offset, global_idx)` pairs + `partition_point` for
  deterministic last-le / first-ge semantics. Skips `None`
  entries entirely.
- **CRIT** `find_function_start` — clamped result to `count - 1`
  which incorrectly returned the previous-function RET when the
  cursor was on the last instruction. Drop the clamp; `i + 1` is
  always `<= cur` by loop precondition.
- `select_function` now pushes nav history + sets origin
  breadcrumb (parity with `jump_to_function_*`).
- `follow_at_cursor` clones operand string only on the
  branch-target-miss fallback path (per-double-click alloc
  eliminated for the common jcc / call case).
- Drop dead `frame_counter` field + per-frame tick (orphaned —
  doc claimed "blink cursor" but no consumer).
- Drop dead `search_results: Vec<usize>` field (raw matcher
  output was never read after `do_search`; `search_match_starts`
  + `search_match_set` cover all consumers).
- Drop unnecessary `unsafe` block in byte-category render path —
  pass `byte_hex(b, uppercase)` (already `&'static str`)
  directly to `add_text`.

#### `app_window` audit fixes

- **CRIT** `Suboptimal` swap chain frame now reconfigures the
  surface immediately (same path as `Outdated` / `Lost`). Was
  painting the stale frame and requesting redraw → visible
  tearing on DPI / monitor switches, potential present-chain
  stall on some DX12 drivers.
- **MED** Cache `theme.titlebar()` in `GpuState::cached_titlebar`
  — was rebuilt from scratch every frame for the chrome render.
  Refreshed alongside `clear_color` in `refresh_clear_color()`
  on theme change.
- **MED** `confirm_close()` deduped to delegate to `exit()` —
  was a copy-paste twin with risk of divergence.
- **chrome/glyph.rs** (earlier in session) — replaced cascade
  overlay restore glyph with "doc-window" style (single rect +
  filled top titlebar). `bg` parameter dropped from
  `draw_restore` signature.

#### `notifications` audit fixes

- **CRIT** Click bleed-through — single click could emit
  `Clicked(id)` events for multiple stacked toasts in the same
  frame. Added `click_consumed` latch across the render loop so
  one click = one event, regardless of ImGui window stacking.
- **CRIT** `next_id` collision after `u64` wrap — push-loop now
  scans the live queue and skips occupied ids (also skips id 0
  as the "no id" sentinel).

#### `node_graph` audit fixes

- **MED** `arcs: [f32; 41]` literal coupled to `samples = 40` via
  silent invariant — bumping `samples` would OOB-panic. Replaced
  with `const SAMPLES: usize = 40;` + `[f32; SAMPLES + 1]` so
  the buffer size derives from the same source as the loop bound.
- **MED** `reset_viewport()` now cancels in-progress
  interactions (`node_drag`, `new_wire`, `rect_select`) before
  resetting zoom/offset — was leaving dangling screen-space
  state mid-drag, producing visual glitches until next click.

#### `hex_viewer` (during disasm session)

- Promoted `parse_hex_pattern_masked` and `find_pattern_masked`
  from `pub(super)` to `pub(crate)` so `disasm_view`'s search
  uses the exact same matcher (single source of truth).
- Address gutter copy: single-click → double-click (parity with
  `disasm_view`).
- Dropped trailing `:` from address gutter format string —
  divider already separates the address from hex content. Column
  width reduced from `digits + 2` to `digits + 1` (one trailing
  space for the divider gap). Two tests updated.

#### Theme palettes

- New `DisasmViewColors` fields: 5 `bytes_cat_*` (byte category
  tint), `search_match_bg` (semantic-green at 0.32 alpha).
- `DisasmViewColors::header` token changed from `fg_muted × 0.85`
  → `t.fg`.

#### Audit findings deferred (next session)

- `node_graph`: tooltip system tracked but not rendered;
  `on_connect`/`on_disconnect` hooks never invoked; frustum
  culling extends only to node body, not to wires/pins.
- `notifications`: per-frame `format!` allocations for action /
  close button labels; `estimate_height` line-count math wrong
  for wrapped text; hard-coded pixel metrics break under DPI
  scaling; `theme.rs` 16-line stub could be inlined.
- `app_window`: `bg_erase` palette field orphaned (no consumer
  after `draw_restore` change) — needs theme version bump
  before removal; `whole_window_resize` API drift (returns tuple
  vs sibling `TitlebarResult`).
- `disasm_view`: tooltip path allocates 5+ `String`s per hover
  frame; `compute_arrows_clipped` is `O(N²)` on
  `VecDisasmProvider` for very large buffers (provider
  responsibility — switch to indexed impl); `frame_comment_x`
  uses `<= 0.0` as uninitialised sentinel.

### Removed — `proc_mon` widget (2026-04-29, session 028)

The `proc_mon` widget — and its `proc_enum` sibling-workspace
dependency — has been removed. The crate is now exclusively a
collection of UI mods; process enumeration moved to a separate
project owned by the user.

**BREAKING — feature gate gone**:

- `proc_mon` Cargo feature deleted (was in the `full` meta-feature
  by default).
- `dep:serde` and `dep:proc_enum` optional dependencies removed —
  `serde` was used solely by `proc_mon::types`; nothing else in the
  crate touched it. The path dep on `../useful-lib/proc_enum` is
  gone.
- `dear_imgui_custom_mod::proc_mon` module removed (gated by the
  feature, so no breakage for callers who didn't enable it).

**Files deleted**:

- `src/proc_mon/` — 4 files (`mod.rs`, `config.rs`, `types.rs`,
  `ui.rs`).
- `examples/demo_proc_mon.rs` (426 LoC).
- `docs/proc_mon.md` (357 LoC).
- `[[example]] demo_proc_mon` block in `Cargo.toml`.
- README table row + tree entry + demo entry.

Historical changelog entries for `proc_mon` (sessions 023–026)
are kept in this file as a record of the past surface — they
describe state that no longer exists.

**Migration**: hosts that need process enumeration should use the
user's separate monitoring library directly. The previous
`MonitorColors` / `ColumnConfig` / `ProcessRow` types were tightly
coupled to `proc_enum`'s data model and aren't portable — re-build
on top of `virtual_table` against the new data source.

### `disasm_view` audit follow-up — refresh trait drop + 6 hygiene items (2026-04-29, session 028)

Post-overhaul audit pass on `src/disasm_view`. Found 0 critical /
8 smells / 6 polish items; **7 fixed, 6 deferred** with explicit
reasoning. No public API breaks beyond the trait method removal.

**Trait surface**:

- `DisasmDataProvider::refresh()` removed — the default-impl
  no-op had **zero callers** anywhere (no `render()` invocation,
  no host integration). Kept only as a stub from session 019;
  cleaned up now to keep the trait surface honest.

**Internal cleanups**:

- Dropped dead `_first_visible_row: usize` parameter from
  `draw_instruction_row` and `draw_arrows` — never read by either
  method (arrow indices are local to the visible slice; row Y
  comes from the caller).
- Dropped dead `x += cols.bytes;` post-increment after the bytes
  column in `draw_instruction_row` — `x` is only read once
  afterwards (for `instr_data_x`), then never again. Replaced with
  an explicit `bytes_end_x` calculation.
- `EDIT_CELL_BG` and `EDIT_CELL_BORDER` extracted as module-level
  constants — replaces 4 duplicate magic-number `[f32; 4]` literals
  for the warm-amber inline-edit highlight.
- `join_bytes_hex` helper added in `input.rs` — replaces 2
  `format!("{:02X}", b)` + `Vec::collect` + `.join(" ")` triple-
  allocation patterns (in `copy_selected` and the double-click
  pre-fill) with the same single-allocation `String::with_capacity`
  pattern `draw_instruction_row` already uses. Same treatment in
  `draw.rs`'s tooltip block.

**Behaviour changes**:

- Dynamic `comment_x` pre-pass now runs **unconditionally** (was
  gated on `show_comments`). The value is consumed by
  `draw_header` for "Instruction" centring and by `mouse_to_cell`
  to bound the Mnemonic hit-zone; only the comment-column draw
  branches stay gated on `show_comments`.
- **Mnemonic hit-zone gated** in `mouse_to_cell` — returns `None`
  for the mnemonic+operands range until the assembler round-trip
  is wired (`DisasmDataProvider::assemble` default-impl is no-op,
  so opening the editor here would be a UX leak: type + Enter,
  nothing happens). The `EditColumn::Mnemonic` variant and the
  `commit_edit` branch stay alive — flip the `None` back to
  `Some(...)` once `assemble` works.

**Deferred (with reasons)**:

- `BranchArrow.from_idx`/`to_idx` rename to `vis_*` — internal API
  surface, behavioural risk without testable benefit.
- Per-frame `Vec<&dyn Instruction>` allocation in arrow pre-pass —
  no profile evidence; ~50 elements × 60 fps is well within
  budget.
- `parse_address` dedup with `hex_viewer::parse_address` — the two
  use **different heuristics** (`disasm`: requires `a-f` letter to
  classify as hex; `hex_viewer`: uses `len > 4` as proxy).
  Behavioural merge requires explicit decision, not a mechanical
  extract.
- `BranchArrow` / `compute_arrows` / `MAX_ARROW_DEPTH`
  visibility → `pub(crate)` — SemVer-relevant, defer to the 0.10
  bump.
- Goto-popup uses `compact_popup_body` but context-menu does not
  — context menu sized by ImGui itself, intentional asymmetry.

### `disasm_view` polish — popup, column dividers, comment editing (2026-04-29, session 028)

Fourth widget in the popup-cohesion arc (`hex_viewer` → `nav_panel`
→ `disasm_view`). Brings the disassembly view's right-click menu
and Goto popup up to the same theming bar, adds the missing
column-divider chrome, and lights up an editable Comment column
via double-click — without disturbing the existing Bytes-edit
gesture.

**`utils::popup` (graduation)**

- `compact_popup_body(ui, body)` and `action_row(ui, body_w, primary_label)`
  graduated from `hex_viewer/popup.rs` and re-exported from
  `crate::utils::popup`. `hex_viewer` now consumes them; `disasm_view`
  picks them up immediately. Both helpers preserve the previous
  spacing constants (`ItemSpacing 6×4`, `FramePadding 6×3`,
  `ItemInnerSpacing 4×4`, action-button width 58, edge gap 2 px) so
  no visual regression in the existing dialogs.
- `crate::utils::popup` re-exports updated:
  `action_row`, `button_with_color`, `compact_popup_body`,
  `danger_button`, `selected_button`, `success_button`,
  `themed_popup_style`.

**`disasm_view` popup**

- Right-click context menu rebuilt as a themed popup (was raw
  `ui.selectable()` rows). Items now carry atlas-safe glyph icons:
  `»` Copy Address, `»` Copy N Instructions / Instruction · `Ctrl+C`,
  `→` Follow Branch · `Enter`, `●` Toggle Breakpoint · `F9`,
  `»` Goto Address... · `G`. The Copy item adapts its label to the
  selection size (`Copy 4 Instructions` vs `Copy Instruction`).
  Follow Branch greys out (`Alpha 0.40`) when the row has no branch
  target so the "nothing to follow" state is visible.
- Goto popup (`G`) now uses `themed_popup_style` + `compact_popup_body`
  + `action_row(.., "Go")` — visually identical to `hex_viewer`'s
  Goto dialog. Centred via `igSetNextWindowPos(.., Cond_Always, [0.5, 0.5])`
  on `component_center` so the popup always lands at the viewer's
  visual middle, no matter where the user pressed `G`.
- Right-click menu spawns at the click position via the same FFI
  with pivot `(0, 0)`. Both modal and context popups gate
  `OpenPopup` on a one-shot flag while keeping `BeginPopup` outside
  the conditional (the `hex_viewer` lesson — flashing-popup bug).

**`disasm_view` chrome**

- Vertical column dividers between Address / Bytes / Instruction /
  Comment, drawn in `colors.separator` at alpha `0.40` (matches the
  `hex_viewer` divider language). New `draw_column_dividers()`
  method runs in the foreground draw list after the arrows pass.
  Toggleable via the new `DisasmViewConfig::show_column_dividers`
  field (default `true`).
- Comment column shifted right by `+5 px` (`COMMENT_LEFT_PAD = 5.0`)
  for breathing room between the operands' last token and the
  comment text. Applied to both the header caption and per-row text.

**Comment editing (double-click)**

- `EditColumn` enum extended with a `Comment` variant alongside
  `Bytes` and the existing `Mnemonic`. Per-column hit-testing via
  the new `mouse_to_cell(ui, provider) -> Option<(usize, EditColumn)>`
  helper that walks the same X-layout as `draw_instruction_row`,
  so the address gutter / arrow lane / breakpoint margin all return
  `None` (those have non-edit affordances).
- `DisasmDataProvider::set_comment(addr, text) -> bool` — new trait
  method with a `false` default impl so existing implementors stay
  non-breaking. `VecDisasmProvider` implements it with trim-on-write
  and clear-on-empty semantics (whitespace-only `text` clears the
  comment). Returns `false` when the address isn't decoded.
- Double-click on a row routes through `mouse_to_cell` and pre-fills
  the edit buffer per column: bytes → space-separated hex pairs,
  mnemonic → `mnemonic operands`, comment → existing comment text
  (or empty). `commit_edit` dispatches to `write_bytes` / `assemble`
  / `set_comment` based on `EditColumn`.
- `InputTextFlags` now per-column: `Bytes` gets
  `CHARS_HEXADECIMAL | CHARS_UPPERCASE`; `Mnemonic` and `Comment`
  get free text. `AUTO_SELECT_ALL | ENTER_RETURNS_TRUE` apply
  to all three.
- 5 new unit tests:
  `set_comment_round_trip_via_vec_provider`,
  `set_comment_clears_on_empty_string`,
  `set_comment_trims_surrounding_whitespace`,
  `set_comment_returns_false_for_unknown_address`,
  `set_comment_default_trait_impl_is_noop`.
  (Hit-test math in `mouse_to_cell` requires an active ImGui frame
  — exercised via `examples/demo_disasm_view`, not unit-testable.)

### Polish session — chrome / nav / hex_viewer overhaul (2026-04-29, session 027)

A long late-evening polish pass touching the four widgets the user
spends the most cursor time on. Net result: cleaner hover language
across the chrome stack, friendlier popup geometry in `hex_viewer`,
and a `nav_panel` that finally settles on a single hover style.

**`nav_panel`**

- **BREAKING** — removed the `HoverStyle` enum and its `Flat` /
  `Zoom` variants. Hover behaviour is now hardcoded: icon glyph
  re-renders at `hover_zoom_scale` (default `1.20×`) on hover,
  no background fill. The cell-emboss / glyph-emboss earlier
  experiments are gone too. Set `with_hover_zoom_scale(1.0)` to
  effectively disable the magnification.
- New `ActiveStyle` enum: `Ring` (default — transparent orange
  ring around the icon, no fill, no indicator strip) and `Bar`
  (the historic filled-cell + indicator-strip look).
- New tunables: `active_ring_color: Option<[f32; 4]>` (default
  warm amber `[0.95, 0.62, 0.20, 1.0]`), `active_ring_thickness`,
  `active_ring_padding`. Builders: `with_active_style`,
  `with_active_ring_color`, `with_active_ring_thickness`,
  `with_active_ring_padding`, `without_active_ring_color`.
- New `with_hover_zoom_scale(f32)` builder, clamps `1.0..=3.0`.
- Module-level doc in `render.rs` explains the chrome-vs-content
  hover policy (chrome buttons get `btn_hover` rectangle, content
  buttons get zoom-only — deliberate, not an oversight).

**`hex_viewer`**

- Address gutter renamed: header reads `"Address"`, per-byte
  hover-tooltip says `Address: 0x…`, goto popup label says
  `Goto address`. Internal `show_offsets` field name kept (no
  breaking config change).
- **Click-to-copy on the address column.** Hover the gutter →
  cursor switches to `Hand`, themed tooltip reads
  `Click to copy: 0x…`. Left-click copies the row's address
  (formatted per `address_width` + `uppercase` config) to the
  clipboard, fades a translucent accent-coloured pill behind
  the address text for `~30` frames.
- ASCII column **right-anchored** to the child window's content
  edge (with a `1 ca` scrollbar gap) instead of floating right
  after the hex column. Falls back to the natural position on
  narrow windows. New `ascii_col_x(win_x)` helper centralises
  the math; both draw + hit-test go through it.
- Offset gutter padding **halved** — the address text was floating
  in too much whitespace before the column divider. The divider
  itself is now centred in the new 1-char gap.
- Header captions (`Address`, `00 01 02 …`, `ASCII`) **centred**
  inside their columns instead of left-aligned.
- Header text colour pinned to `fg` (was `fg_muted`) so captions
  read as bright white on dark themes (the project owner reported
  the muted shade as washed-out).
- gamma fix in `app_window` / `utils::color`: `srgb_to_linear` +
  `wgpu_clear_color(rgba, surface_format)` correctly handle the
  sRGB-encode round-trip when clearing an sRGB swap-chain
  surface (`*UnormSrgb`). Resolves the "fog" / washed-out look
  introduced when the root window picked up `NO_BACKGROUND`.
- 4 popups (`Ctrl+G` goto, `Ctrl+F` search, right-click context
  menu, Settings) now use the crate-wide `themed_popup_style`:
  generous padding, frame rounding, `add_text_with_font`-aware
  layout. Critical fix: `BeginPopup` now runs **every frame**
  instead of only the open-trigger frame (popups previously
  flashed for one frame and disappeared).
- Modal popups (Goto / Search / Settings) anchor at
  `component_center` with a `(0.5, 0.5)` pivot — they always
  spawn at the visual middle of the viewer regardless of where
  the trigger came from. Context menu still anchors at the click
  location.
- New public API: `request_goto()` / `request_search()` for
  host-side global hotkeys / menus / toolbars that want to fire
  a popup without depending on the viewer being focused.
- Right-click context menu: 4 entries with arrow / ellipsis icons
  (`»` Go to Address, `←` Step back, `→` Step forward,
  `…` Settings). Step-back / step-forward grey out at `0.40 α`
  when their nav stack is empty.
- Settings popup hosts BPR buttons (8/12/…/32, square 32-px
  cells), Display toggles (ASCII / inspector / offsets / column
  headers / column dividers / splitter), Format toggles
  (uppercase / category colours / dim zeros), and a right-anchored
  Close button. Locally tighter style overrides keep the popup
  compact despite the long checklist.
- Theme integration: `HexViewerColors` palette type with 18
  per-purpose tokens; `Theme::hex_viewer_colors()` accessor;
  per-theme `hex_viewer_colors()` factories for all 7 built-in
  themes; `HexViewerConfig::with_theme(theme)` /
  `apply_theme_colors(&palette)` builders.

**`app_window` chrome**

- Titlebar buttons gained a hover-zoom (`Buttons::hover_zoom_scale`,
  default `1.20`) — same macOS-Dock-style micro-magnification the
  nav panel uses. Min / max / restore / close all scale; extras
  (text-glyph buttons) scale via `add_text_with_font` so their
  rasterised glyph grows proportionally too.
- New `Buttons::show_hover_bg: bool` (default `false`) — disables
  the historic coloured rectangle behind a hovered button.
  Flip back to `true` to recover the old Vex0r-style red close
  hover.
- New builders: `Buttons::with_hover_zoom_scale(f32)` (clamps
  `1.0..=2.0`), `Buttons::with_hover_bg(bool)`.
- Close button glyph rewritten — historic circle-with-X →
  short-lived spoked-progress wheel → `"close"` text label →
  current bold standalone `×` (thickness `1.8`, arms `0.65×r`).
  The user iterated through every variant before settling.
- `glyph::draw_close` consolidated to that single bold-X
  implementation; orphan circle / progress / text helpers
  removed.
- `disasm_view::render` tooltip-passthrough fixed — mouse_pos
  is now gated on `is_window_hovered()` (same pattern as
  `hex_viewer`) so hover tooltips don't ghost through popups
  rendered on top of the disasm widget.

**`utils`**

- New `utils::popup` module (split off from `tooltip`) hosts the
  popup-styling + button-stack helpers:
  - `themed_popup_style(ui, body)` — pushes WindowPadding
    `[14, 12]`, ItemSpacing `[10, 8]`, FramePadding `[10, 6]`,
    WindowRounding / FrameRounding `5.0` for the duration of
    `body`. Wrap any `BeginPopup` body for consistent geometry.
  - `success_button(ui, label, size)` — green confirm button.
  - `danger_button(ui, label, size)` — red destructive button.
  - `button_with_color(ui, label, color, size)` — arbitrary
    base colour; auto-derives hover / active via `±0.06` per
    channel via internal `lift()` helper. **Bug fix:**
    `success_button` / `danger_button` now go through the same
    `lift()` derivation — earlier they used hand-rolled hover /
    active constants that drifted from the documented `±0.06`
    formula. Single source of truth via `with_button_stack`.
  - 6 unit tests covering `lift()` clamp behaviour, semantic
    colour invariants, hue distinctness.
- `utils::tooltip` keeps `themed_tooltip` only (the popup stuff
  moved out). Doc updated.

**Tests**

- `+14 lib tests`, total `651 passed / 0 failed`.
- New tests cover: utils popup `lift()` math + colour invariants
  (5 tests), `Buttons` builders + clamp (4 tests), hex_viewer
  `request_goto` / `request_search` / `address_flash` initial
  state / `component_center` initial state (4 tests), nav_panel
  `active_style` defaults + `hover_zoom_scale` clamp (1 carryover).

### BREAKING — Host framework consolidation (2026-04-29, sessions 026)

Three host-related modules collapsed into one. `app_window` v1 and the
standalone `borderless_window` module are gone; `app_window_v2` is
renamed back to `app_window` and is now the only window-host
framework. **Net −4464 LoC across two phases.**

**Removed**
- `dear_imgui_custom_mod::app_window` (the v1 API: `AppConfig::new`,
  `StartPosition`, etc.). The path is now occupied by the renamed v2.
- `dear_imgui_custom_mod::borderless_window` (whole module).
  `BorderlessConfig`, `ButtonConfig`, `CloseMode`, `TitleAlign`,
  `TitlebarState`, `WindowAction`, `ResizeEdge`, `render_titlebar`,
  `render_titlebar_overlay` — all gone. Borderless chrome lives
  inside `app_window` (private `chrome/` submodule).
- Cargo features `app_window` (the v1 one) and `borderless_window`.
- Examples `demo_app_window`, `demo_borderless`.
- `tests/builder_chains.rs::borderless_config_*`.

**Renamed (no behavioural change beyond the name)**
- Module: `app_window_v2` → `app_window`. Cargo feature, demo bin,
  docs file, examples bin all follow.
- Identifiers (26 total) lose their `V2` suffix:
  `AppWindowV2` → `AppWindow`, `AppConfigV2` → `AppConfig`,
  `AppHandlerV2` → `AppHandler`, `AppStateV2` → `AppState`,
  `AppProxyV2` → `AppProxy`, `RenderModeV2` → `RenderMode`,
  `PowerModeV2` → `PowerMode`, `FontChoiceV2` → `FontChoice`,
  `FontLayerV2` → `FontLayer`, `GlyphRangesV2` → `GlyphRanges`,
  `ExtraButtonV2` → `ExtraButton`, `WindowIconV2` → `WindowIcon`,
  `WindowKindV2` → `WindowKind`, `PositionV2` → `Position`,
  `BorderStyleV2` → `BorderStyle`, `FormStyleV2` → `FormStyle`,
  `CloseModeV2` → `CloseMode`, `TitleAlignV2` → `TitleAlign`,
  `FpsModeV2` → `FpsMode`, `TitlebarConfigV2` → `TitlebarConfig`,
  `TitlebarActionV2` → `TitlebarAction`, `TitlebarResultV2` →
  `TitlebarResult`, `TitlebarStateV2` → `TitlebarState`,
  `ButtonsV2` → `Buttons`, `ChromeV2` → `Chrome`,
  `ResizeEdgeV2` → `ResizeEdge`.

**Internal changes**
- `app_window/win32.rs` is now self-contained: the five Win32 helpers
  it used to delegate to `borderless_window::platform` (HWND extract,
  DWM dark mode, rounded corners, region update, Win11 detection)
  are inlined here. No `crate::borderless_window::*` references remain
  anywhere outside historical comments.
- `src/utils/clipboard.rs`: `vk_down`, `VK_A/C/F/G/Y/Z`,
  `c_key_down_physical` removed (no remaining users). The file now
  hosts only `set_clipboard` and the Windows layout-switching helpers
  used by `hex_viewer`'s edit mode (98 → 27 LoC).
- `src/input/keyboard.rs` gained a one-call helper
  `dispatch_window_event(context, platform, window, &WindowEvent)`
  that routes KeyboardInput / Ime through the layout-independent
  injection helpers and forwards to `WinitPlatform::handle_window_event`
  with the after-forward reinforce. Replaces ~30 lines of boilerplate
  per host. Used by every demo.
- Per-widget VK-edge-detection paths in `hex_viewer`, `disasm_view`,
  `virtual_table`, `virtual_tree` removed; all four widgets now use
  plain ImGui `is_key_pressed(...)` and rely on the host having
  installed the layout-independent dispatcher (which `app_window`
  does internally).

**Migration**

```rust
// before — old app_window v1
use dear_imgui_custom_mod::app_window::{
    AppConfig, AppWindow, StartPosition,
};
let cfg = AppConfig::new("My App", 1100.0, 700.0)
    .with_start_position(StartPosition::CenterScreen);

// after — current app_window (= former v2)
use dear_imgui_custom_mod::app_window::{AppConfig, AppWindow, Position};
let cfg = AppConfig::main("My App", 1100.0, 700.0)
    .with_position(Position::ScreenCenter);
```

```rust
// before — palette via borderless_window
use dear_imgui_custom_mod::borderless_window::TitlebarColors;

// after
use dear_imgui_custom_mod::theme::TitlebarColors;
```

For hosts that wire winit themselves (instead of using `AppWindow`):
add `dear_imgui_custom_mod::input::keyboard::dispatch_window_event(...)`
in the `WindowEvent::KeyboardInput` arm — this brings layout-
independent Ctrl+C / numpad text / IME commit support that
`app_window` enables for free.

### Changed — `hex_viewer` modernisation (2026-04-29, sessions 025+026)

- **BREAKING:** `HexViewer.config` field is now private. Use
  `viewer.config()` / `viewer.config_mut()` accessors.
- **BREAKING:** `set_cursor` now always clears any selection and pushes
  the previous position into the back-history (was: only on jumps
  larger than `bytes_per_row`). Matches the intuitive "go to address"
  semantics; `goto()` is built on top.
- **UX:** Edit mode is entered by **double-click** only. Single click
  on another byte commits any half-typed nibble (HxD-style upper-
  nibble replacement) and exits edit mode. Esc still discards. Drag-
  select is suppressed inside an active edit cell.
- **Fixed:** `Shift+Arrow` / `Shift+PageUp/Down` / `Shift+Home/End` now
  anchor `selection.start` at the previous cursor (selections used to
  always grow from offset 0).
- **Fixed:** `handle_ascii_input` rejects non-ASCII / control chars
  (used to write the first UTF-8 byte of multi-byte chars and silently
  corrupt the buffer).
- **Fixed:** `is_search_match` now binary-searches the sorted
  `search_results` (O(log N) per byte, was O(N)). Big win for
  permissive wildcard patterns.
- Module split — `mod.rs` 2045 LoC → six focused files
  (`mod.rs` 303, `config.rs` 611, `search.rs` 235, `input.rs` 533,
  `draw.rs` 585, `popup.rs` 87, `tests.rs` 409). Public API unchanged.
- 30 → 36 unit tests; 0 clippy warnings.

### Added — `disasm_view` demo polish

- Toolbar Back / Fwd / Clear-selection buttons mirror the existing
  `View::nav_back` / `nav_forward` / `clear_selection` API.
- "N selected" counter in the address line when multi-select is
  active. (No library changes — the `View` API gained these in
  commit 5499b5a; the demo finally exposes them.)

### Added — `tab_control` module (modern tab controller)
DevExpress XtraTabControl-inspired pure tab strip with contemporary touches:

- **3 visual styles** — `Pill` (default), `Underline` (Material), `Square` (classic).
- **Pinned tabs** — compact, non-scrolling left strip; `is_pinned()` per-tab opt-in.
  Pinned-prefix invariant maintained automatically across `add()` / `move_tab()` /
  per-frame `enforce_pinned_partition` (in-place `rotate_right`, no allocations).
- **Drag-and-drop reorder** within a group (pinned ↔ pinned, regular ↔ regular).
- **Hover preview popup** — Windows-taskbar-peek-style live thumbnail of the tab's
  content via `render_preview` (defaults to `render_content` re-render). Width-locked
  tooltip auto-grows vertically; never shows a scrollbar. Per-tab opt-out via
  `show_preview()`. Suppressed during drag and for the active tab.
- **Hover-activate** — `cfg.hover_activate_ms = Some(N)` for Edge / Win11-style
  auto-switch on dwell.
- **Keyboard shortcuts** — Left/Right step, Ctrl+Tab cycle (with wrap), Ctrl+1..9
  jump (Chrome convention: Ctrl+9 → last), Ctrl+T add, Ctrl+W close.
- **Status indicators** — Active / Inactive / Warning / Error / Dirty / None. Dot is
  three-way controllable: globally via `cfg.show_status_dot`, per-tab via
  `TabStatus::None`, per-tab color via `dot_color()`.
- **Dirty state** — cyan circle replaces the close button visually; close popup
  switches to a stronger "unsaved changes" confirmation text.
- **Customizable close glyph** — `Cross` / `CrossBold` / `SquareX` / `CircleX`,
  all rendered via the draw list (no font dependency).
- **Smooth scroll** with overflow `…` dropdown auto-closing on selection.
- **Animations** — open (grow), close (shrink), hover transitions.
- **Single-pass hit-test** — pre-pass fills `hit_scratch` once; both drawing
  and event handling read from the same scratch — no duplicate geometry.
- **Cached popup IDs** + **zero per-frame allocations** in steady state.
- **Nested `TabControl` trivially supported** — just embed in `render_content`.

API: `TabControl<T: TabItem>` with 14 public methods; `TabItem` trait with 14
methods (2 required, 12 with defaults). 32 unit tests covering pinned-prefix
invariant, pinned-aware `add`/`move_tab`, lifecycle hooks, status & color
overrides, popup-ID isolation, opt-out flags. See `docs/tab_control.md`.

### Removed — `page_control` module
The dashboard-with-tile-grid feature was removed. Use plain ImGui inside the
new `tab_control::TabItem::render_content` if you need a tile dashboard. The
demo, feature flag (`page_control`), example (`demo_page_control`), and the
internal `src/demo/` scaffolding (used only by `demo_page_control`) are all
gone. `tab_control` is the focused, modernized successor.

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
