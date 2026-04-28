//! Process enumeration via NT syscalls.
//!
//! Uses `NtQuerySystemInformation(SystemProcessInformation)` to get all processes,
//! `NtQueryInformationProcess(ProcessWow64Information)` for 32/64-bit detection,
//! and thread state analysis for Suspended detection.
//!
//! # Optimizations
//!
//! - WoW64 bits cached per PID (process bitness never changes).
//! - Reusable syscall buffer across ticks (avoids reallocation).
//! - Delta detection: direct `ProcStatus` compare vs previous tick — the
//!   only volatile field on the minimal `ProcessInfo`. Matches the
//!   `IMGUI_NXT` reference engine approach.
//! - `foldhash` (non-crypto) for all `u32`-keyed maps — faster than SipHash.
//!
//! # Safety
//!
//! All unsafe blocks perform direct NT syscalls via the `rsc-runtime`
//! crate (SysCalls / RSC). Buffer bounds are checked before pointer
//! dereferencing.

use crate::proc_mon::types::{ProcStatus, ProcessDelta, ProcessInfo};
use std::collections::HashMap;
use std::sync::Arc;

// Import rsc-runtime (Windows-only)
#[cfg(windows)]
use rsc_runtime::constants::PROCESS_QUERY_LIMITED_INFORMATION;
#[cfg(windows)]
use rsc_runtime::error::STATUS_INFO_LENGTH_MISMATCH;
#[cfg(windows)]
use rsc_runtime::syscalls::{
    NtClose, NtOpenProcess, NtQueryInformationProcess, NtQuerySystemInformation,
};
#[cfg(windows)]
use rsc_runtime::types::{CLIENT_ID, HANDLE, OBJECT_ATTRIBUTES, PVOID, ULONG, UNICODE_STRING};

// ─── Fast hasher alias ────────────────────────────────────────────────────────

/// `HashMap` with `foldhash::fast::FixedState` — non-cryptographic, high-quality,
/// ~5× faster than `std`'s SipHash on `u32` keys.
type FxMap<K, V> = HashMap<K, V, foldhash::fast::FixedState>;

#[inline]
fn fx_map_with_cap<K, V>(cap: usize) -> FxMap<K, V> {
    FxMap::with_capacity_and_hasher(cap, foldhash::fast::FixedState::default())
}

// ─── NT structures (not in syscalls crate) ───────────────────────────────────

/// System process information structure returned by NtQuerySystemInformation.
/// This is the layout for SystemProcessInformation (class 5).
#[repr(C)]
#[allow(non_snake_case)]
struct SYSTEM_PROCESS_INFORMATION {
    NextEntryOffset: u32,
    NumberOfThreads: u32,
    WorkingSetPrivateSize: i64,
    HardFaultCount: u32,
    NumberOfThreadsHighWatermark: u32,
    CycleTime: u64,
    CreateTime: i64,
    UserTime: i64,
    KernelTime: i64,
    ImageName: UNICODE_STRING,
    BasePriority: i32,
    UniqueProcessId: usize,
    InheritedFromUniqueProcessId: usize,
    HandleCount: u32,
    SessionId: u32,
    UniqueProcessKey: usize,
    PeakVirtualSize: usize,
    VirtualSize: usize,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
    PrivatePageCount: usize,
    ReadOperationCount: i64,
    WriteOperationCount: i64,
    OtherOperationCount: i64,
    ReadTransferCount: i64,
    WriteTransferCount: i64,
    OtherTransferCount: i64,
    // Followed by NumberOfThreads × SYSTEM_THREAD_INFORMATION
}

/// Thread information structure (follows SYSTEM_PROCESS_INFORMATION).
#[repr(C)]
#[allow(non_snake_case)]
struct SYSTEM_THREAD_INFORMATION {
    KernelTime: i64,
    UserTime: i64,
    CreateTime: i64,
    WaitTime: u32,
    StartAddress: usize,
    ClientId: CLIENT_ID,
    Priority: i32,
    BasePriority: i32,
    ContextSwitches: u32,
    ThreadState: u32,
    WaitReason: u32,
}

// ─── Constants ────────────────────────────────────────────────────────────────

const SYSTEM_PROCESS_INFORMATION_CLASS: u32 = 5;
const PROCESS_WOW64_INFORMATION: u32 = 26;

const THREAD_STATE_WAITING: u32 = 5;
const THREAD_WAIT_REASON_SUSPENDED: u32 = 5;

/// Prune dead PIDs from the bits cache every N ticks.
const CACHE_PRUNE_INTERVAL: u32 = 15;

/// Maximum allowed size for the syscall buffer (64 MiB).
/// Prevents unbounded memory growth if NtQuerySystemInformation
/// reports an unexpectedly large required size.
const SYS_BUF_MAX: usize = 64 * 1024 * 1024;

// ─── Error type ───────────────────────────────────────────────────────────────

/// Error type for process enumeration.
#[derive(Debug, Clone)]
pub enum Error {
    /// Syscall failed with NTSTATUS error code.
    SyscallFailed(i32),
    /// Buffer too large (exceeds SYS_BUF_MAX).
    BufferTooLarge(usize),
    /// Not supported on this platform (non-Windows).
    NotSupported,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyscallFailed(status) => {
                write!(f, "Syscall failed with status 0x{:08X}", *status as u32)
            }
            Self::BufferTooLarge(size) => write!(
                f,
                "Buffer too large: {} bytes (max {} MiB)",
                size,
                SYS_BUF_MAX / (1024 * 1024)
            ),
            Self::NotSupported => write!(f, "Process monitoring not supported on this platform"),
        }
    }
}

impl std::error::Error for Error {}

// ─── Monitor context (persists across ticks) ──────────────────────────────────

/// Internal state for the process enumerator.
struct MonitorCtx {
    /// Reusable syscall buffer.
    sys_buf: Vec<u8>,
    /// Cache: PID → bitness (32/64).
    bits_cache: FxMap<u32, u8>,
    /// Cache: PID → process name. Image names are immutable per-PID, so
    /// every snapshot of the same PID hands out an `Arc::clone` — no
    /// UTF-16 decode, no allocation past the first sighting.
    name_cache: FxMap<u32, Arc<str>>,
    /// Previous snapshot for delta calculation (PID → status). `ProcStatus`
    /// is the only volatile field on the minimal `ProcessInfo`; anything
    /// else (name, bits, create_time) is immutable per-PID.
    prev: FxMap<u32, ProcStatus>,
    /// Whether first tick (send full list).
    first_tick: bool,
    /// Tick counter (for periodic cache pruning).
    tick: u32,
    /// Reusable buffer for current tick PID→index lookup.
    current_pids_buf: FxMap<u32, usize>,
    /// Reusable buffer for delta upsert list.
    upsert_buf: Vec<ProcessInfo>,
    /// Reusable buffer for delta removed list.
    removed_buf: Vec<u32>,
}

impl Default for MonitorCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorCtx {
    fn new() -> Self {
        Self {
            sys_buf: Vec::with_capacity(512 * 1024),
            bits_cache: fx_map_with_cap(512),
            name_cache: fx_map_with_cap(512),
            prev: fx_map_with_cap(512),
            first_tick: true,
            tick: 0,
            current_pids_buf: fx_map_with_cap(512),
            upsert_buf: Vec::with_capacity(64),
            removed_buf: Vec::with_capacity(64),
        }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Process enumerator with delta support.
///
/// Call [`enumerate()`](Self::enumerate) for a full snapshot,
/// or [`enumerate_delta()`](Self::enumerate_delta) for incremental updates.
pub struct ProcessEnumerator {
    ctx: MonitorCtx,
}

impl Default for ProcessEnumerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessEnumerator {
    /// Create a new process enumerator.
    pub fn new() -> Self {
        Self {
            ctx: MonitorCtx::new(),
        }
    }

    /// Enumerate all processes (full snapshot).
    ///
    /// Returns a `Vec<ProcessInfo>` sorted by CreateTime (newest first).
    #[cfg(windows)]
    pub fn enumerate(&mut self) -> Result<Vec<ProcessInfo>, Error> {
        self.ctx.tick = self.ctx.tick.wrapping_add(1);
        let current = self.query_all_processes()?;
        self.commit_snapshot(&current);
        self.ctx.first_tick = false;
        Ok(current)
    }

    /// Enumerate all processes (full snapshot) — non-Windows stub.
    #[cfg(not(windows))]
    pub fn enumerate(&mut self) -> Result<Vec<ProcessInfo>, Error> {
        Err(Error::NotSupported)
    }

    /// Enumerate processes with delta update.
    ///
    /// First call returns a delta with all processes in `upsert` (equivalent
    /// to a full snapshot). Subsequent calls return only new, changed, or
    /// removed processes — change = `status` flip Running ↔ Suspended.
    #[cfg(windows)]
    pub fn enumerate_delta(&mut self) -> Result<ProcessDelta, Error> {
        self.ctx.tick = self.ctx.tick.wrapping_add(1);
        let current = self.query_all_processes()?;

        if self.ctx.first_tick {
            self.ctx.first_tick = false;
            self.commit_snapshot(&current);
            let total = current.len();
            return Ok(ProcessDelta {
                upsert: current,
                removed: Vec::new(),
                total,
            });
        }

        // Build current PID index for removal detection.
        self.ctx.current_pids_buf.clear();
        for (i, p) in current.iter().enumerate() {
            self.ctx.current_pids_buf.insert(p.pid, i);
        }

        // Walk current: new PID or status flip → upsert.
        self.ctx.upsert_buf.clear();
        for p in &current {
            match self.ctx.prev.get(&p.pid) {
                None => self.ctx.upsert_buf.push(p.clone()),
                Some(prev_status) if *prev_status != p.status => {
                    self.ctx.upsert_buf.push(p.clone());
                }
                _ => {}
            }
        }

        // Find removed PIDs (in prev but not in current).
        self.ctx.removed_buf.clear();
        self.ctx.removed_buf.extend(
            self.ctx
                .prev
                .keys()
                .copied()
                .filter(|pid| !self.ctx.current_pids_buf.contains_key(pid)),
        );

        // Update prev snapshot.
        self.commit_snapshot(&current);

        let total = current.len();
        Ok(ProcessDelta {
            upsert: std::mem::take(&mut self.ctx.upsert_buf),
            removed: std::mem::take(&mut self.ctx.removed_buf),
            total,
        })
    }

    /// Enumerate processes with delta update — non-Windows stub.
    #[cfg(not(windows))]
    pub fn enumerate_delta(&mut self) -> Result<ProcessDelta, Error> {
        Err(Error::NotSupported)
    }

    /// Clear internal caches (e.g., after a long period of inactivity).
    pub fn clear_cache(&mut self) {
        self.ctx.bits_cache.clear();
        self.ctx.name_cache.clear();
        self.ctx.prev.clear();
        self.ctx.first_tick = true;
    }

    /// Replace `ctx.prev` with the status view of `current`. Reuses the
    /// existing allocation (clear + insert) — no drop+alloc.
    fn commit_snapshot(&mut self, current: &[ProcessInfo]) {
        self.ctx.prev.clear();
        for p in current {
            self.ctx.prev.insert(p.pid, p.status);
        }
    }
}

// ─── Process enumeration via syscalls (Windows-only) ──────────────────────────

#[cfg(windows)]
impl ProcessEnumerator {
    /// Query all processes via NtQuerySystemInformation.
    /// Returns sorted by CreateTime (newest first).
    fn query_all_processes(&mut self) -> Result<Vec<ProcessInfo>, Error> {
        // Declared before `unsafe` to keep safe allocations out of the unsafe scope.
        let mut result = Vec::with_capacity(512);
        let mut live_pids = Vec::with_capacity(512);

        // SAFETY: The block performs direct syscalls (NtQuerySystemInformation)
        // followed by a walk over the returned linked list. The kernel writes
        // exactly `return_length` bytes into `sys_buf` which we resize to match.
        // Every pointer dereference is bounds-checked against `return_length`.
        unsafe {
            // 1. Query required buffer size.
            let mut return_length: ULONG = 0;
            let status = NtQuerySystemInformation(
                SYSTEM_PROCESS_INFORMATION_CLASS,
                core::ptr::null_mut(),
                0,
                &mut return_length,
            );

            // NTSTATUS: sign bit = error (NT_SUCCESS ≡ status >= 0).
            if status != STATUS_INFO_LENGTH_MISMATCH.code() && status < 0 {
                return Err(Error::SyscallFailed(status));
            }

            // 2. Resize buffer (capped at SYS_BUF_MAX).
            let needed = (return_length as usize) + 0x10000;
            if needed > SYS_BUF_MAX {
                return Err(Error::BufferTooLarge(needed));
            }
            if self.ctx.sys_buf.len() < needed {
                self.ctx.sys_buf.resize(needed, 0);
            }

            // 3. Query actual data.
            let status = NtQuerySystemInformation(
                SYSTEM_PROCESS_INFORMATION_CLASS,
                self.ctx.sys_buf.as_mut_ptr() as PVOID,
                self.ctx.sys_buf.len() as ULONG,
                &mut return_length,
            );

            if status < 0 {
                return Err(Error::SyscallFailed(status));
            }

            // 4. Parse linked list. Use `return_length` (not `sys_buf.len()`)
            // as the upper bound — the buffer may be over-allocated.
            let buf_len = return_length as usize;
            let mut offset: usize = 0;

            loop {
                // Bounds check.
                if offset + core::mem::size_of::<SYSTEM_PROCESS_INFORMATION>() > buf_len {
                    break;
                }

                let spi =
                    &*(self.ctx.sys_buf.as_ptr().add(offset) as *const SYSTEM_PROCESS_INFORMATION);
                let pid = spi.UniqueProcessId as u32;
                live_pids.push(pid);

                // Process name — UTF-16 decoded ONCE per PID, cached as
                // `Arc<str>`. Subsequent ticks return `Arc::clone(...)` —
                // a single atomic refcount bump, no decode, no allocation.
                let name: Arc<str> = self
                    .ctx
                    .name_cache
                    .entry(pid)
                    .or_insert_with(|| {
                        if spi.ImageName.Buffer.is_null() || spi.ImageName.Length == 0 {
                            match pid {
                                0 => Arc::from("System Idle Process"),
                                _ => Arc::from("System"),
                            }
                        } else {
                            let len = (spi.ImageName.Length / 2) as usize;
                            let slice = core::slice::from_raw_parts(spi.ImageName.Buffer, len);
                            // `from_utf16_lossy` allocates a `String`; we then
                            // hand its buffer to `Arc::from(&str)`.
                            Arc::from(String::from_utf16_lossy(slice).as_str())
                        }
                    })
                    .clone();

                // Bitness (cached: never changes per-PID). When
                // `query_process_bits` returns `None` we fall back to 64 —
                // the cache stores the resolved fallback so future ticks
                // don't keep retrying a denied OpenProcess.
                let bits = *self
                    .ctx
                    .bits_cache
                    .entry(pid)
                    .or_insert_with(|| Self::query_process_bits(pid).unwrap_or(64));

                // Bytes remaining for the thread array (immediately after header).
                let remaining = buf_len
                    .saturating_sub(offset + core::mem::size_of::<SYSTEM_PROCESS_INFORMATION>());
                let suspended = Self::is_process_suspended(spi, remaining);

                result.push(ProcessInfo {
                    pid,
                    name,
                    bits,
                    status: if suspended {
                        ProcStatus::Suspended
                    } else {
                        ProcStatus::Running
                    },
                    create_time: spi.CreateTime,
                });

                if spi.NextEntryOffset == 0 {
                    break;
                }
                offset += spi.NextEntryOffset as usize;
            }
        }

        // Prune dead PIDs from caches periodically (safe — outside unsafe block).
        // Both `bits_cache` and `name_cache` follow the same per-PID
        // invariant — drop entries for PIDs that aren't live this tick.
        if self.ctx.tick.is_multiple_of(CACHE_PRUNE_INTERVAL) {
            live_pids.sort_unstable();
            let live = &live_pids;
            self.ctx
                .bits_cache
                .retain(|pid, _| live.binary_search(pid).is_ok());
            self.ctx
                .name_cache
                .retain(|pid, _| live.binary_search(pid).is_ok());
        }

        // Sort by CreateTime descending (newest first).
        result.sort_by_key(|p| std::cmp::Reverse(p.create_time));

        Ok(result)
    }

    /// Query WoW64 status for a single PID (expensive — called once per PID, then cached).
    ///
    /// Returns:
    /// - `Some(64)` for System processes (PID 0..=4) — always 64-bit.
    /// - `Some(32)` if `ProcessWow64Information` reports a non-null peb.
    /// - `Some(64)` if it reports null (= native process).
    /// - `None` when `OpenProcess` failed (permission denied, dead PID).
    ///   Callers typically substitute the architecture default.
    fn query_process_bits(pid: u32) -> Option<u8> {
        if pid <= 4 {
            return Some(64); // System processes are always 64-bit
        }

        // SAFETY: Standard NtOpenProcess + NtQueryInformationProcess pattern.
        // Handle is closed on every path. Casts are documented in the canonical
        // RSC signature notes.
        unsafe {
            let mut handle: HANDLE = core::ptr::null_mut();
            let mut client_id = CLIENT_ID {
                UniqueProcess: pid as usize as HANDLE,
                UniqueThread: core::ptr::null_mut(),
            };
            let mut oa: OBJECT_ATTRIBUTES = core::mem::zeroed();
            oa.Length = core::mem::size_of::<OBJECT_ATTRIBUTES>() as ULONG;

            let status = NtOpenProcess(
                &mut handle,
                PROCESS_QUERY_LIMITED_INFORMATION,
                &mut oa as *mut OBJECT_ATTRIBUTES as PVOID,
                &mut client_id,
            );

            if status < 0 || handle.is_null() {
                return None; // permission denied / dead PID
            }

            let mut is_wow64: usize = 0;
            let mut ret_len: ULONG = 0;
            let status = NtQueryInformationProcess(
                handle,
                PROCESS_WOW64_INFORMATION as usize as PVOID,
                &mut is_wow64 as *mut _ as PVOID,
                core::mem::size_of::<usize>() as ULONG,
                &mut ret_len,
            );

            NtClose(handle);

            if status < 0 {
                return None;
            }
            Some(if is_wow64 != 0 { 32 } else { 64 })
        }
    }

    /// Check if all threads are in Suspended state.
    ///
    /// `remaining_bytes` is the number of bytes after the `SYSTEM_PROCESS_INFORMATION`
    /// header that are valid in the buffer — used to bounds-check the thread array.
    fn is_process_suspended(spi: &SYSTEM_PROCESS_INFORMATION, remaining_bytes: usize) -> bool {
        let thread_count = spi.NumberOfThreads as usize;
        if thread_count == 0 {
            return false;
        }

        // Bounds check: ensure the thread array fits within the buffer.
        if thread_count * core::mem::size_of::<SYSTEM_THREAD_INFORMATION>() > remaining_bytes {
            return false;
        }

        // SAFETY: SYSTEM_THREAD_INFORMATION records follow immediately after
        // SYSTEM_PROCESS_INFORMATION. We iterate exactly thread_count times,
        // and the bounds check above guarantees all accesses are in-range.
        unsafe {
            let threads_ptr = (spi as *const SYSTEM_PROCESS_INFORMATION).add(1)
                as *const SYSTEM_THREAD_INFORMATION;

            for i in 0..thread_count {
                let thread = &*threads_ptr.add(i);
                if thread.ThreadState != THREAD_STATE_WAITING
                    || thread.WaitReason != THREAD_WAIT_REASON_SUSPENDED
                {
                    return false;
                }
            }
        }

        true
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(all(windows, test))]
mod tests {
    use super::*;

    // NOTE: the two syscall-hitting tests below require real NT syscall stubs
    // in the running process (normally present in a compiled binary, absent in
    // the cargo-test harness on some toolchains). Run explicitly with
    // `cargo test -- --ignored` to exercise them.

    #[test]
    #[ignore = "requires live NT syscall binding (run with --ignored)"]
    fn test_enumerate_processes() {
        let mut enumerator = ProcessEnumerator::new();
        let procs = enumerator
            .enumerate()
            .expect("Failed to enumerate processes");
        assert!(!procs.is_empty(), "Should have at least one process");

        // Check that System process exists (PID 4 on Windows).
        let system = procs.iter().find(|p| p.pid == 4);
        assert!(system.is_some(), "System process (PID 4) should exist");

        // Check sorting (newest first = descending create_time).
        for i in 1..procs.len() {
            assert!(
                procs[i - 1].create_time >= procs[i].create_time,
                "Processes should be sorted by create_time descending"
            );
        }
    }

    #[test]
    #[ignore = "requires live NT syscall binding (run with --ignored)"]
    fn test_delta_update() {
        let mut enumerator = ProcessEnumerator::new();

        // First delta = full list.
        let delta1 = enumerator.enumerate_delta().expect("Failed to get delta");
        assert!(
            !delta1.upsert.is_empty(),
            "First delta should have processes"
        );
        assert!(
            delta1.removed.is_empty(),
            "First delta should have no removed"
        );

        // Second delta = incremental — vast majority of processes unchanged.
        let delta2 = enumerator.enumerate_delta().expect("Failed to get delta");
        assert!(
            delta2.upsert.len() <= delta1.upsert.len(),
            "delta upsert should be a subset on steady state"
        );
    }
}
