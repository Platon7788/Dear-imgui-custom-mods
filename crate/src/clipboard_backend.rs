//! System-clipboard backend for the Dear ImGui context.
//!
//! Lives at the crate root so that [`app_window`](crate::app_window)
//! and any future host can install the exact same backend without
//! one feature pulling the other.
//!
//! Without this backend, every `InputText` ends up with a private paste
//! buffer that does not interact with the OS — an immediate UX
//! regression for end users (Ctrl+C from `InputText` would not reach
//! the system clipboard, Ctrl+V from outside would not appear inside).
//!
//! Both `get` and `set` go **directly to the Win32 API**, bypassing
//! `igSetClipboardText` / `igGetClipboardText`. Routing through ImGui's
//! setter from inside a backend `set` would re-enter the same callback
//! and be silently short-circuited by `dear_imgui_rs::ClipboardBorrowGuard`
//! — meaning **the OS clipboard never actually gets the text**.
//!
//! On non-Windows targets the backend is a no-op (returns `None` /
//! drops `set`); platform-native getters can be wired in later behind
//! an opt-in `arboard` feature.

use dear_imgui_rs::ClipboardBackend;

/// Default system clipboard backend — installed automatically on every
/// [`crate::app_window::AppWindow`] unless the user supplies their own.
///
/// Implementation (Windows): direct
/// `OpenClipboard + EmptyClipboard + GlobalAlloc(GHND) + GlobalLock +
/// memcpy UTF-16 + GlobalUnlock + SetClipboardData(CF_UNICODETEXT) +
/// CloseClipboard` for `set`; mirror sequence with `GetClipboardData`
/// for `get`. On `SetClipboardData` failure we **do** call
/// `GlobalFree` per MSDN guidance — leaving the handle leaked would
/// only persist until process exit, but a clean failure path is
/// trivial here. Other platforms: both methods are no-ops.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClipboardBackend;

impl ClipboardBackend for SystemClipboardBackend {
    fn get(&mut self) -> Option<String> {
        #[cfg(windows)]
        {
            win::read_clipboard_text()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    fn set(&mut self, value: &str) {
        #[cfg(windows)]
        {
            win::write_clipboard_text(value);
        }
        #[cfg(not(windows))]
        {
            let _ = value;
        }
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock};

    /// Standard `CF_UNICODETEXT` clipboard-format value. Inlined to avoid
    /// pulling in the full `Win32_System_Ole` feature for one constant.
    const CF_UNICODETEXT: u32 = 13;

    /// Read the system clipboard as a `String`.
    ///
    /// Returns `None` if the clipboard is empty, opening it fails (another
    /// process holds it), or the contents are not text.
    pub(super) fn read_clipboard_text() -> Option<String> {
        unsafe {
            // SAFETY: standard Windows API contract — `OpenClipboard(NULL)`
            // associates the calling task with the clipboard. We always
            // pair it with `CloseClipboard`.
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }

            let result = read_unicode_text_inner();

            CloseClipboard();
            result
        }
    }

    unsafe fn read_unicode_text_inner() -> Option<String> {
        // SAFETY: `GetClipboardData` returns a borrowed `HANDLE` valid until
        // `CloseClipboard`. We `GlobalLock` to obtain a stable pointer and
        // `GlobalUnlock` before returning, never escaping the locked memory.
        let handle: HANDLE = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if handle.is_null() {
            return None;
        }
        let ptr = unsafe { GlobalLock(handle as _) } as *const u16;
        if ptr.is_null() {
            return None;
        }

        // Find the NUL terminator.
        let mut len = 0usize;
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
            // Defensive cap: 16 Mi UTF-16 characters (~32 MiB). Anything
            // beyond is almost certainly a runaway process or driver bug.
            if len > 16 * 1024 * 1024 {
                unsafe { GlobalUnlock(handle as _) };
                return None;
            }
        }

        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        let owned = String::from_utf16_lossy(slice);
        unsafe { GlobalUnlock(handle as _) };
        Some(owned)
    }

    /// Write `text` to the system clipboard as `CF_UNICODETEXT`.
    ///
    /// Allocates a movable global with `GMEM_MOVEABLE | GMEM_ZEROINIT`
    /// (`GHND`), copies the UTF-16 NUL-terminated text into it, then
    /// transfers ownership of the handle to the OS via
    /// `SetClipboardData`. After that call the OS owns the handle — we
    /// do **not** free it. On *failure* (rare — clipboard busy, ACL
    /// denial, allocation failure post-OpenClipboard) we **do** call
    /// `GlobalFree` per MSDN guidance.
    ///
    /// Failures are silent — clipboard write is a best-effort UX nicety,
    /// not a state-mutating operation worth surfacing as an error to
    /// ImGui's hot path.
    pub(super) fn write_clipboard_text(text: &str) {
        // Encode as UTF-16 with explicit NUL terminator.
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes = wide.len() * std::mem::size_of::<u16>();

        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return;
            }
            // `EmptyClipboard` clears prior contents and assigns ownership
            // to our task — required before `SetClipboardData`.
            EmptyClipboard();

            // SAFETY: `GlobalAlloc(GHND, bytes)` returns a movable handle
            // backed by zero-initialised memory. `GlobalLock` returns a
            // stable pointer for the duration of the lock.
            let h_mem = GlobalAlloc(GHND, bytes);
            if h_mem.is_null() {
                CloseClipboard();
                return;
            }
            let dst = GlobalLock(h_mem) as *mut u16;
            if dst.is_null() {
                // Allocation succeeded but lock failed — free the handle.
                GlobalFree(h_mem);
                CloseClipboard();
                return;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
            GlobalUnlock(h_mem);

            // SetClipboardData transfers ownership; on success we MUST NOT
            // free the handle. On failure MSDN explicitly directs callers
            // to `GlobalFree` the local handle (the clipboard never took
            // ownership).
            if SetClipboardData(CF_UNICODETEXT, h_mem as HANDLE).is_null() {
                GlobalFree(h_mem);
            }
            CloseClipboard();
        }
    }
}
