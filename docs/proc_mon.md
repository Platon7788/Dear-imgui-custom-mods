# proc_mon

Windows-only process monitor with direct NT-syscall enumeration and a
virtualized `dear-imgui` table view. Mirrors the minimal 5-field
`ProcessInfo` from the `IMGUI_NXT` reference engine — list-only,
zero-overhead, production-ready.

## Overview

`proc_mon` ships two halves:

- [`ProcessEnumerator`] — calls `NtQuerySystemInformation`
  (`SystemProcessInformation`) once per tick, parses the returned linked
  list, and produces either a full snapshot (`enumerate()`) or an
  incremental [`ProcessDelta`] (`enumerate_delta()`). Uses a reusable
  syscall buffer, cached WoW64 bitness per PID, and `foldhash`-backed
  maps for all `u32`-keyed lookups. Delta detection is a direct
  `ProcStatus` compare — the only volatile field.
- [`ProcessMonitor`] — UI widget built on [`virtual_table`](virtual_table.md).
  Renders up to 10 000 processes at 30–60 FPS, handles selection, search,
  context-menu routing, and row highlighting via [`MonitorColors`].
- Shared [`ProcessInfo`] — five fields: `pid`, `name`, `bits`, `status`,
  `create_time`.

Gated behind the `proc_mon` feature (on by default via `full`). The
feature requires `virtual_table` + `syscalls` + `serde` and compiles
only on Windows.

## Features

- **Direct NT syscalls** via the local `syscalls` crate — no
  `kernel32`/`psapi` roundtrip, fewer dependencies.
- **Minimal 5-field `ProcessInfo`** matching `IMGUI_NXT`: `pid`, `name`,
  `bits` (32/64), `status` (Running / Suspended), `create_time`. Every
  other metric (memory / CPU% / threads / handles / I/O) is
  **intentionally absent** to keep the overhead profile identical to a
  headless service monitor.
- **Suspended detection** — walks the `SYSTEM_THREAD_INFORMATION` records
  that follow each process entry and checks if every thread is in
  `Waiting / WaitReason=Suspended`.
- **Zero-hash delta** — change detection is a single `ProcStatus`
  equality check per PID; no hashing, no false positives. Matches the
  exact mechanism used by the reference engine.
- **`foldhash` everywhere** — all `HashMap<u32, _>` use
  `foldhash::fast::FixedState` (~5× faster than `std`'s SipHash on
  `u32` keys). Same pattern as `virtual_table` / `virtual_tree`.
- **Reusable syscall buffer** — capped at 64 MiB, grown on demand. Bitness
  cache pruned every 15 ticks against the live PID list.
- **Stable ordering** — results sorted by `CreateTime` descending (newest
  first). Survives PID reuse — the same PID reappearing gets its real
  position back.
- **Virtualized rendering** — built-in [`virtual_table`](virtual_table.md)
  integration with a fixed sort; scrolls through 10 000 processes at 60 FPS.
- **Row highlighting** via [`MonitorColors`] — tint individual rows by
  PID, name, self-process, or suspended state. Resolved once per upsert
  and cached into `ProcessRow` so rendering is pure lookup.
- **Search filter** — case-insensitive substring match on process name +
  PID, using a pre-lowercased query and a reusable PID-scratch buffer (no
  per-frame allocation).
- **Context menu routing** — the widget emits
  [`MonitorEvent::ContextMenuRequested(pid)`] and clears the flag; the
  caller renders their own popup with arbitrary actions.
- **Embedded mode** — `render_contents(ui)` for use inside a parent panel
  (skipping the built-in `ui.window` wrapper); `render(ui, &mut show)`
  for a standalone pop-up.

## Quick Start

```rust
use dear_imgui_custom_mod::proc_mon::{
    MonitorConfig, MonitorEvent, ProcessEnumerator, ProcessMonitor,
};
use std::time::{Duration, Instant};

// Persistent state — keep on your app struct.
let config = MonitorConfig::default();
let mut enumerator = ProcessEnumerator::new();
let mut monitor = ProcessMonitor::new(config.clone());
let mut last_tick = Instant::now();
let mut show = true;

// In your per-frame callback:
# fn frame(
#     ui: &dear_imgui_rs::Ui,
#     enumerator: &mut ProcessEnumerator,
#     monitor: &mut ProcessMonitor,
#     config: &MonitorConfig,
#     last_tick: &mut Instant,
#     show: &mut bool,
# ) {
if last_tick.elapsed() >= config.interval() {
    *last_tick = Instant::now();
    if let Ok(delta) = enumerator.enumerate_delta() {
        monitor.apply_delta(&delta);
    }
}

if let Some(event) = monitor.render(ui, show) {
    match event {
        MonitorEvent::RowSelected(pid)          => { /* single click */ }
        MonitorEvent::RowDoubleClicked(pid)     => { /* open details */ }
        MonitorEvent::ContextMenuRequested(pid) => {
            ui.open_popup("##proc_ctx");
        }
    }
}
# }
```

See `examples/demo_proc_mon.rs` for a complete end-to-end app including a
caller-drawn context menu with Kill / Copy PID / Details buttons.

## Configuration

```rust
use dear_imgui_custom_mod::proc_mon::{ColumnConfig, MonitorConfig, MonitorColors};

let config = MonitorConfig {
    columns: ColumnConfig {
        bits: true,
        status: true,
    },
    colors: MonitorColors::default()
        .with_self([0.20, 0.60, 0.35, 0.25])
        .with_name("chrome.exe", [0.25, 0.50, 0.85, 0.22]),
    interval_ms: 1000,
    max_processes: 10_000,
    show_search: true,
    window_title: "Process Monitor",
};

// Or use presets:
let minimal = MonitorConfig::minimal();      // Name + PID only
let full    = MonitorConfig::all_columns();  // Name + PID + Bits + Status
```

## Row highlighting

[`MonitorColors`] lets callers tint individual rows — useful for
identifying the host process, tracking a specific PID, or grouping
related executables without wrapping the widget in an outer UI.

```rust
use dear_imgui_custom_mod::proc_mon::{MonitorColors, MonitorConfig, ProcessMonitor};

let colors = MonitorColors::default()
    // Soft green for the current process (uses std::process::id()).
    .with_self([0.20, 0.60, 0.35, 0.25])
    // Case-insensitive name match for well-known tools.
    .with_name("chrome.exe",    [0.25, 0.50, 0.85, 0.22])
    .with_name("firefox.exe",   [0.90, 0.35, 0.15, 0.22])
    .with_name("svchost.exe",   [0.40, 0.40, 0.45, 0.16])
    // Explicit PID override — always wins.
    .with_pid(4, [0.70, 0.20, 0.20, 0.20]);                // System

let config = MonitorConfig {
    colors,
    ..MonitorConfig::default()
};
let mut monitor = ProcessMonitor::new(config);
```

Runtime updates:

```rust
monitor.colors_mut().add_pid(my_pid, [1.0, 0.9, 0.1, 0.30]);
monitor.refresh_colors();          // re-resolve overrides for existing rows

// Or swap the whole palette:
monitor.set_colors(MonitorColors::default().with_self([0.1, 0.7, 0.4, 0.25]));
```

**Resolution priority** (first non-`None` wins):

| # | Source | Notes |
|---|--------|-------|
| 1 | `by_pid[pid]` | Highest — explicit override |
| 2 | `by_name[name.to_lowercase()]` | Case-insensitive, O(1) lookup; skipped when map empty |
| 3 | `self_process` if `pid == std::process::id()` | Requires `Some(color)` |
| 4 | `suspended` if `status == Suspended` | Default palette has amber tint; set to `None` to disable |
| 5 | no highlight | Default row background |

**Zero-cost rendering.** `ProcessMonitor::apply_delta` /
`set_full_list` resolve the palette once per row at upsert time and
cache the result in `ProcessRow::color_override`. Rendering is a pure
`Option<[f32; 4]>` copy — no rule evaluation, no `to_lowercase` allocs,
no hash lookups per frame. Status flips (Running ↔ Suspended) re-resolve
automatically because `status` drives the upsert.

## Columns

Canonical layout, indices `0..=3`. Hidden columns are registered but
marked `.visible(false)` — this keeps indices stable and the
`cell_display_text` match compact.

| # | Column | Default | Alignment | Width |
|---|--------|---------|-----------|-------|
| 0 | Process Name | ✅ | Left | **stretch** |
| 1 | PID | ✅ | Center | 70 px |
| 2 | Bits | ✅ | Center | 45 px (`x32` / `x64`) |
| 3 | Status | ✅ | Center | 70 px (`Running` / `Suspended`) |

**Layout mechanics.** Process Name uses `.stretch(1.0)`; the other
columns are fixed-width. On window resize, Name grows / shrinks while
PID / Bits / Status stay pinned to the right edge. Header hover /
active highlights are suppressed inside `ProcessMonitor::render_contents`
via `push_style_color(HeaderHovered / Active, transparent)` — sortable
is off, so headers are informative-only (no button-like feedback).

## Architecture

```
src/proc_mon/
  mod.rs        # Public re-exports, feature gate
  types.rs      # ProcessInfo (5 fields), ProcStatus, ProcessDelta,
                # ColumnConfig (2 flags), MonitorColors, MonitorEvent
  core.rs       # ProcessEnumerator, MonitorCtx, NtQuerySystemInformation
                # syscall, WoW64 query, status-based delta
  config.rs     # MonitorConfig + presets (default / minimal / all_columns)
  ui.rs         # ProcessMonitor, ProcessRow, canonical 4-column layout
```

### Delta pipeline

```
┌───────────────────────────────────────────────────────────────────────┐
│  enumerate_delta()                                                    │
│   ├─ query_all_processes()                                            │
│   │   ├─ NtQuerySystemInformation → reusable sys_buf (64 MiB cap)     │
│   │   ├─ walk SYSTEM_PROCESS_INFORMATION list                         │
│   │   ├─ per-process: UTF-16 name, bits_cache.entry(pid), suspended   │
│   │   │                 detection                                     │
│   │   └─ sort_by_key(Reverse(create_time))                            │
│   ├─ for each current: new PID or status flip → upsert                │
│   ├─ for each prev PID not in current            → removed            │
│   └─ commit_snapshot(current) → prev: FxMap<u32, ProcStatus>          │
└───────────────────────────────────────────────────────────────────────┘
```

### Optimization summary

| Area | Technique | Benefit |
|------|-----------|---------|
| PID maps | `foldhash::fast::FixedState` | ~5× faster lookups vs `std` |
| Bitness | `bits_cache` entry-or-insert, prune every 15 ticks | Expensive `NtOpenProcess` + query happens once per PID |
| Delta | Direct `ProcStatus` equality, no hashing | Zero false-positives, no allocations |
| Syscall | Reusable `sys_buf`, grown on demand, capped at 64 MiB | No per-tick realloc; defense-in-depth cap |
| Row rendering | No cached strings — three short integer formats per row | Whole `ProcessRow` fits in a couple of cache lines |
| Sorting | Fixed `CreateTime` desc, done once per delta | Stable PID-reuse ordering without per-frame sort |
| Search | Lowercased query cached, reusable PID scratch buffer | No `io::Cursor`, no per-frame alloc |

## API Reference

### `ProcessEnumerator`

| Method | Description |
|--------|-------------|
| `new()` / `default()` | Fresh enumerator. |
| `enumerate()` | Full snapshot as `Vec<ProcessInfo>`, sorted newest first. |
| `enumerate_delta()` | Incremental `ProcessDelta`; first call returns full list in `upsert`. |
| `clear_cache()` | Forget bits cache + prev snapshot (next tick returns full list). |

### `ProcessMonitor`

| Method | Description |
|--------|-------------|
| `new(config)` | Monitor UI with the given `MonitorConfig`. |
| `set_full_list(procs)` | Replace state with a full snapshot. |
| `apply_delta(delta)` | Upsert / remove based on `ProcessDelta`; in-place mutation for known PIDs. |
| `selected_pid()` | PID of the currently-selected row, if any. |
| `set_columns(cfg)` | Change visible columns (rebuilds the `VirtualTable`). |
| `colors()` / `colors_mut()` | Read / mutate the current highlight palette. |
| `set_colors(colors)` | Replace the palette and re-resolve every row in one call. |
| `refresh_colors()` | Re-resolve overrides after editing via `colors_mut()`. |
| `self_pid()` | PID of the host process (cached at construction). |
| `process(pid)` | Look up a tracked `ProcessRow` by PID. |
| `invalidate()` | Force re-sort (call after changing `search_buf`). |
| `render_contents(ui)` | Render header + search + table without opening a window. |
| `render(ui, &mut show)` | Draw the standalone window; returns `Option<MonitorEvent>`. |

### `MonitorColors`

| Field / Method | Description |
|----------------|-------------|
| `suspended: Option<[f32;4]>` | Fallback tint for `Suspended` rows (default amber, set `None` to disable). |
| `self_process: Option<[f32;4]>` | Tint for the host process (default `None`). |
| `by_name: HashMap<String, [f32;4]>` | Case-insensitive name → color. Keys are lowercased. |
| `by_pid: HashMap<u32, [f32;4]>` | PID → color. Highest resolution priority. |
| `with_suspended / with_self / with_name / with_pid` | Builder-style setters (consume self). |
| `add_name / add_pid / remove_name / remove_pid` | In-place mutation on `colors_mut()`. |
| `clear_all()` | Reset every field to `None` / empty. |
| `resolve(&info, self_pid) -> Option<[f32;4]>` | Public resolver — useful for custom renderers that want the same priority rules. |

### `MonitorEvent`

| Variant | Fires when |
|---------|------------|
| `RowSelected(pid)` | Single-click on a row. |
| `RowDoubleClicked(pid)` | Double-click on a row. |
| `ContextMenuRequested(pid)` | Right-click on a row — caller draws the popup. |

### `ProcessInfo` (5 fields)

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub bits: u8,             // 32 or 64
    pub status: ProcStatus,   // Running | Suspended
    pub create_time: i64,     // NT FILETIME — stable sort key
}
```

### `ColumnConfig`

```rust
pub struct ColumnConfig {
    pub bits: bool,   // default true
    pub status: bool, // default true
}
```

`Name` + `PID` are always visible.

## Performance notes

- **Build profiles matter.** `[profile.dev.package."*"] opt-level = 2`
  keeps dependency hot paths (wgpu, imgui) near release speed even in debug;
  the release profile uses LTO + single codegen unit + stripped symbols.
  See repo-level `Cargo.toml`.
- **Demo render loop caps at 30 FPS** via `ControlFlow::WaitUntil(now + 33ms)`.
  Monitoring UIs do not benefit from 60 Hz — halving the cap cuts CPU roughly
  in half for idle windows.
- **Overhead parity with `IMGUI_NXT`**: the enumerator now does only what
  the NxT reference engine does — one `NtQuerySystemInformation` per tick,
  one `ProcStatus` compare per tracked PID, no float math, no wall-clock
  queries. In release, a headless user (enumerator only, no UI) hits the
  same 0.2% CPU baseline as the NxT engine task.

## Platform support

Windows only. The module is `#[cfg(windows)]`; `enumerate()` /
`enumerate_delta()` return `Error::NotSupported` on other platforms. The
`syscalls` dependency is gated `[target.'cfg(windows)'.dependencies]` so
non-Windows builds compile without it.

## Safety

All raw pointer work lives in `core.rs::query_all_processes` and two
helper functions. Each `unsafe` block is preceded by a SAFETY comment
documenting:
- Bounds checks on `sys_buf` before dereferencing `SYSTEM_PROCESS_INFORMATION`
- Null-`Buffer` handling for `UNICODE_STRING` process names
- Handle lifetime for `NtOpenProcess` / `NtClose` in the WoW64 query
- Thread record iteration bounded by `NumberOfThreads`
