# proc_mon

Windows-only process monitor with direct NT-syscall enumeration and a
virtualized `dear-imgui` table view.

## Overview

`proc_mon` gives you everything needed to display and track live OS processes
with minimal overhead:

- [`ProcessEnumerator`] — calls `NtQuerySystemInformation`
  (`SystemProcessInformation`) once per tick, parses the returned linked list,
  and produces either a full snapshot (`enumerate()`) or an incremental
  [`ProcessDelta`] (`enumerate_delta()`) with only new / changed / removed
  processes. Uses a reusable syscall buffer, cached WoW64 bitness per PID,
  and `foldhash`-backed maps for all `u32`-keyed lookups.
- [`ProcessMonitor`] — UI widget built on [`virtual_table`](virtual_table.md).
  Renders up to 10 000 processes at 30–60 FPS, handles selection, search,
  context-menu routing, and pre-formats every volatile column into
  reusable `String`s so rendering is allocation-free on the hot path.
- Shared [`ProcessInfo`] — 19-field record with full metrics (memory, threads,
  handles, I/O bytes, kernel+user CPU time, optional `cpu_percent`).

Gated behind the `proc_mon` feature (on by default via `full`). The feature
requires `virtual_table` + `syscalls` + `serde` and compiles only on Windows.

## Features

- **Direct NT syscalls** via the local `syscalls` crate — no
  `kernel32`/`psapi` roundtrip, fewer dependencies.
- **Full process metrics** — PID, PPID, name, 32/64-bit, session ID, status
  (Running / Suspended), priority, working set, private bytes, virtual size,
  peak working set, thread count, handle count, I/O read/write bytes,
  kernel + user + cycle time, create time.
- **Optional CPU% tracking** — opt-in via
  [`ProcessEnumerator::set_cpu_tracking(true)`]. When disabled, the enumerator
  skips the wall-clock query, per-process `HashMap` lookups, and float math
  — matching the overhead profile of a list-only monitor. When enabled,
  `cpu_percent` is normalized across all logical cores: `Δ(kernel+user) /
  (Δwall × cores) × 100`, clamped to `[0, 100]`.
- **Suspended detection** — walks the `SYSTEM_THREAD_INFORMATION` records
  that follow each process entry and checks if every thread is in
  `Waiting / WaitReason=Suspended`.
- **Zero-hash delta** — change detection uses direct field comparison on a
  10-field [`SnapDiff`] struct with `PartialEq`, not `std::hash`. Cheaper,
  collision-free, and excludes monotonically-growing CPU counters so active
  processes don't spam upserts.
- **In-place row updates** — `ProcessMonitor::apply_delta` mutates existing
  `ProcessRow`s for known PIDs and only refreshes the volatile formatted
  strings (Memory, I/O, CPU%), avoiding drop/realloc + re-formatting of
  immutable columns (name, create time).
- **`foldhash` everywhere** — all `HashMap<u32, _>` use
  `foldhash::fast::FixedState` (~5× faster than `std`'s SipHash on `u32`
  keys). Same pattern as `virtual_table` / `virtual_tree`.
- **Reusable syscall buffer** — capped at 64 MiB, grown on demand. Bitness
  cache pruned every 15 ticks against the live PID list.
- **Stable ordering** — results sorted by `CreateTime` descending (newest
  first). Survives PID reuse — the same PID reappearing gets its real
  position back.
- **Virtualized rendering** — built-in [`virtual_table`](virtual_table.md)
  integration with a fixed sort; scrolls through 10 000 processes at 60 FPS.
- **Canonical column layout** — 18 columns, indexed 0..=17, hidden ones
  registered with `.visible(false)` so column indices stay stable regardless
  of `ColumnConfig`.
- **Search filter** — case-insensitive substring match on process name +
  PID, using a pre-lowercased query and a reusable PID-scratch buffer (no
  per-frame allocation).
- **Context menu routing** — the widget emits
  [`MonitorEvent::ContextMenuRequested(pid)`] and clears the flag; the caller
  renders their own popup with arbitrary actions.

## Quick Start

```rust
use dear_imgui_custom_mod::proc_mon::{
    MonitorConfig, MonitorEvent, ProcessEnumerator, ProcessMonitor,
};
use std::time::{Duration, Instant};

// Persistent state — keep on your app struct.
let config = MonitorConfig::default();
let mut enumerator = ProcessEnumerator::new();
// Opt-in only when the CPU% column is actually shown.
enumerator.set_cpu_tracking(config.columns.cpu_percent);
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
            // Open your own popup — the widget doesn't draw one.
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
use dear_imgui_custom_mod::proc_mon::{ColumnConfig, MonitorConfig};

let config = MonitorConfig {
    columns: ColumnConfig {
        bits: true,
        status: true,
        memory: true,        // opt-in
        cpu_percent: true,   // opt-in — also enable enumerator.set_cpu_tracking(true)
        threads: true,
        handles: true,
        ..ColumnConfig::default()
    },
    interval_ms: 1000,
    max_processes: 10_000,
    show_search: true,
    window_title: "Process Monitor",
};

// Or use presets:
let minimal = MonitorConfig::minimal();      // Name, PID, Bits, Status
let full    = MonitorConfig::all_columns();  // all 18 columns enabled
```

### Default column set

`ColumnConfig::default()` shows only four columns — **Process Name, PID,
Bits, Status** — matching a minimal monitoring UI. Working set, CPU%, and
everything else are **opt-in** to keep per-frame draw calls low. The
defaults are intentionally conservative: adding every column can double the
render cost on a 300-process box.

### CPU% tracking is opt-in

`ProcessEnumerator` starts with `track_cpu = false`. With tracking off:
- `filetime_now_100ns()` (one `SystemTime::now()` per tick) is skipped
- Per-process `HashMap` lookup for previous `cpu_time` is skipped
- Subtractions + float division + clamp are skipped
- `ProcessInfo::cpu_percent` stays `0.0`

Enable via `enumerator.set_cpu_tracking(true)`. Toggling automatically
resets the baseline so the first reading after enabling is `0.0`.

If the UI column is hidden, the `ProcessMonitor` also skips
`recompute_system_cpu` and the header "System CPU: X%" line.

## Columns

Canonical layout, indices `0..=17`. Hidden columns are registered but marked
`.visible(false)` — this keeps indices stable and the `cell_display_text`
match compact.

| # | Column | Default | Alignment | Width | Notes |
|---|--------|---------|-----------|-------|-------|
| 0 | Process Name | ✅ | Left | **stretch** | Absorbs leftover width |
| 1 | PID | ✅ | Center | 70 px | |
| 2 | Bits | ✅ | Center | 45 px | `x32` / `x64` |
| 3 | Status | ✅ | Center | 70 px | `Running` / `Suspended` |
| 4 | Memory | — | Right | 90 px | Working set |
| 5 | CPU % | — | Right | 65 px | Requires `set_cpu_tracking(true)` |
| 6 | PPID | — | Left | 70 px | |
| 7 | Session | — | Left | 70 px | Terminal-services session id |
| 8 | Priority | — | Left | 70 px | Base priority class |
| 9 | Threads | — | Left | 70 px | |
| 10 | Handles | — | Left | 70 px | |
| 11 | Private | — | Right | 90 px | Private bytes (`PrivatePageCount`) |
| 12 | VM Size | — | Right | 90 px | `VirtualSize` |
| 13 | Peak Mem | — | Right | 90 px | Peak working set |
| 14 | I/O Read | — | Right | 90 px | Read transfer bytes |
| 15 | I/O Write | — | Right | 90 px | Write transfer bytes |
| 16 | CPU Time | — | Left | 100 px | Kernel + user (human) |
| 17 | Created | — | Left | 120 px | NT FILETIME → date |

**Layout mechanics.** Process Name uses `.stretch(1.0)`; the other three
default-visible columns are fixed-width. On window resize, Name grows /
shrinks while PID/Bits/Status stay pinned to the right edge. Header hover /
active highlights are suppressed inside `ProcessMonitor::render` via
`push_style_color(HeaderHovered/Active, transparent)` — sortable is off, so
headers are informative-only (no button-like feedback).

## Architecture

```
src/proc_mon/
  mod.rs        # Public re-exports, feature gate
  types.rs      # ProcessInfo, ProcStatus, ProcessDelta, ColumnConfig,
                # MonitorEvent, format_* helpers
  core.rs       # ProcessEnumerator, SnapDiff, MonitorCtx,
                # NtQuerySystemInformation syscall, WoW64 query
  config.rs     # MonitorConfig + presets (default / minimal / all_columns)
  ui.rs         # ProcessMonitor, ProcessRow, column build + cell dispatch
```

### Delta pipeline

```
┌───────────────────────────────────────────────────────────────────────┐
│  enumerate_delta()                                                    │
│   ├─ query_all_processes()                                            │
│   │   ├─ NtQuerySystemInformation → reusable sys_buf (64 MiB cap)     │
│   │   ├─ walk SYSTEM_PROCESS_INFORMATION list                         │
│   │   ├─ per-process: UTF-16 name, bits_cache.entry(pid), suspended   │
│   │   │                 detection, optional CPU% delta                │
│   │   └─ sort_by_key(Reverse(create_time))                            │
│   ├─ for each current: SnapDiff::from_info vs prev  → upsert          │
│   ├─ for each prev PID not in current               → removed         │
│   └─ commit_snapshot(current) → prev: FxMap<u32, PrevState>           │
└───────────────────────────────────────────────────────────────────────┘
```

### Why `SnapDiff` and not a hash

`std`'s `DefaultHasher` (SipHash-2-4) is a cryptographic hash — strong, but
slow, and prone to false positives if you include ever-growing counters in
the hash input. Earlier iterations of `proc_mon` hashed `kernel_time` and
`user_time`, which tick upward monotonically on every active process →
every active process was always "changed" → delta optimization was
effectively disabled.

`SnapDiff` captures the 10 fields that *actually* change in practice:
`status`, `priority`, `working_set`, `private_bytes`, `virtual_size`,
`peak_working_set`, `thread_count`, `handle_count`, `io_read_bytes`,
`io_write_bytes`. CPU counters are excluded — if they meaningfully moved,
memory or I/O almost always moved with them, so CPU% is still refreshed in
practice without triggering false upserts on every idle kernel tick.

### Optimization summary

| Area | Technique | Benefit |
|------|-----------|---------|
| PID maps | `foldhash::fast::FixedState` | ~5× faster lookups vs `std` |
| Bitness | `bits_cache` entry-or-insert, prune every 15 ticks | Expensive `NtOpenProcess` + query happens once per PID |
| Delta | `SnapDiff: PartialEq`, no hashing | Zero false-positives, exclude CPU counters |
| Syscall | Reusable `sys_buf`, grown on demand, capped at 64 MiB | No per-tick realloc; defense-in-depth cap |
| Rendering | Pre-formatted `String`s for every volatile column | Zero alloc in `cell_display_text` |
| Upsert | In-place mutation for known PIDs | Immutable fields (name, create_time) formatted once |
| CPU% | Opt-in via `set_cpu_tracking` | List-only monitor = zero CPU math overhead |
| Sorting | Fixed `CreateTime` desc, done once per delta | Stable PID-reuse ordering without per-frame sort |
| Search | Lowercased query cached, reusable PID scratch buffer | No `io::Cursor`, no per-frame alloc |
| System CPU | Cached on apply_delta, not per-frame | Saves N float adds per render |

## API Reference

### `ProcessEnumerator`

| Method | Description |
|--------|-------------|
| `new()` / `default()` | Fresh enumerator, CPU tracking off. |
| `enumerate()` | Full snapshot as `Vec<ProcessInfo>`, sorted newest first. |
| `enumerate_delta()` | Incremental `ProcessDelta`; first call returns full list in `upsert`. |
| `set_cpu_tracking(bool)` | Toggle CPU% computation (resets baseline). |
| `cpu_tracking()` | Current tracking flag. |
| `logical_cores()` | Core count used for CPU% normalization. |
| `clear_cache()` | Forget bits cache + prev snapshot (next tick returns full list). |

### `ProcessMonitor`

| Method | Description |
|--------|-------------|
| `new(config)` | Monitor UI with the given `MonitorConfig`. |
| `set_full_list(procs)` | Replace state with a full snapshot. |
| `apply_delta(delta)` | Upsert / remove based on `ProcessDelta`; in-place mutation for known PIDs. |
| `selected_pid()` | PID of the currently-selected row, if any. |
| `set_columns(cfg)` | Change visible columns (rebuilds the `VirtualTable`). |
| `invalidate()` | Force re-sort (call after changing `search_buf`). |
| `render(ui, &mut show)` | Draw the window; returns `Option<MonitorEvent>`. |

### `MonitorEvent`

| Variant | Fires when |
|---------|------------|
| `RowSelected(pid)` | Single-click on a row. |
| `RowDoubleClicked(pid)` | Double-click on a row. |
| `ContextMenuRequested(pid)` | Right-click on a row — caller draws the popup. |

### `ProcessInfo` (19 fields)

```rust
pub struct ProcessInfo {
    // Identity
    pub pid: u32,
    pub name: String,
    pub bits: u8,            // 32 or 64
    pub ppid: u32,
    pub session_id: u32,
    // State
    pub status: ProcStatus,  // Running | Suspended
    pub create_time: i64,    // NT FILETIME
    pub priority: i32,
    // Memory (bytes)
    pub working_set: usize,
    pub private_bytes: usize,
    pub virtual_size: usize,
    pub peak_working_set: usize,
    // CPU
    pub kernel_time: i64,    // 100-ns
    pub user_time: i64,      // 100-ns
    pub cycle_time: u64,
    // Threads & handles
    pub thread_count: u32,
    pub handle_count: u32,
    // I/O
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    // Derived
    pub cpu_percent: f32,    // 0..=100, only non-zero if enumerator tracking is on
}
```

### Formatting helpers

| Function | Output |
|----------|--------|
| `format_bytes(n, &mut buf)` | `"512 B"` / `"2.0 KB"` / `"5.0 MB"` / `"3.0 GB"` |
| `format_cpu_time(t_100ns, &mut buf)` | `"123ms"` / `"1.500s"` / `"1:30.500"` |
| `format_cpu_percent(pct, &mut buf)` | `"—"` (0) / `"0.3%"` (<10%) / `"42%"` (≥10%) |
| `format_create_time(t_filetime, &mut buf)` | Rough `"YYYY-DDD HH:MM:SS"` (chrono-free) |

## Performance notes

- **Build profiles matter.** `[profile.dev.package."*"] opt-level = 2`
  keeps dependency hot paths (wgpu, imgui) near release speed even in debug;
  the release profile uses LTO + single codegen unit + stripped symbols.
  See repo-level `Cargo.toml`.
- **Demo render loop caps at 30 FPS** via `ControlFlow::WaitUntil(now + 33ms)`.
  Monitoring UIs do not benefit from 60 Hz — halving the cap cuts CPU roughly
  in half for idle windows.
- **Headless / service-grade monitors** — if you don't need the UI, use
  `ProcessEnumerator` directly. With CPU tracking off, the per-tick cost is
  a single `NtQuerySystemInformation` plus ~300 field comparisons — matching
  the overhead of the reference engine monitor in `IMGUI_NXT`.

## Platform support

Windows only. The module is `#[cfg(windows)]`; `enumerate()` /
`enumerate_delta()` return `Error::NotSupported` on other platforms. The
`syscalls` dependency is gated `[target.'cfg(windows)'.dependencies]` so
non-Windows builds compile without it.

## Safety

All raw pointer work lives in `core.rs::query_all_processes` and two helper
functions. Each `unsafe` block is preceded by a SAFETY comment documenting:
- Bounds checks on `sys_buf` before dereferencing `SYSTEM_PROCESS_INFORMATION`
- Null-`Buffer` handling for `UNICODE_STRING` process names
- Handle lifetime for `NtOpenProcess` / `NtClose` in the WoW64 query
- Thread record iteration bounded by `NumberOfThreads`
