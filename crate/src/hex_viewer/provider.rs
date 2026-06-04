//! Data provider and byte category types for hex_viewer.

// ── Data Provider ───────────────────────────────────────────────────────────

/// Trait for abstracting the data source.
///
/// **Status:** the bundled [`HexViewer`](super::HexViewer) widget currently
/// owns its own `Vec<u8>` (see `set_data` / `data` / `data_mut`) and does
/// **not** read through this trait. The trait + [`VecDataProvider`] are
/// kept as a stable contract for downstream extensions: a future
/// `HexViewer::render_with(provider)` overload, or callers building their
/// own hex widgets from the same primitives. Existing implementors of
/// this trait remain valid; nothing here is deprecated, just not yet
/// wired into the default render path.
pub trait HexDataProvider {
    /// Total data length in bytes (may be `u64::MAX` for streaming).
    fn len(&self) -> u64;
    /// Whether the data source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Read bytes at `offset` into `buf`. Returns number of bytes actually read.
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize;
    /// Write bytes at `offset`. Returns `true` on success.
    /// Default: returns `false` (read-only).
    fn write(&mut self, _offset: u64, _data: &[u8]) -> bool {
        false
    }
    /// Whether the given offset is readable (e.g., mapped memory region).
    /// Default: `true` for any offset < len.
    fn is_readable(&self, offset: u64) -> bool {
        offset < self.len()
    }
    /// Whether the byte at `offset` has changed since last snapshot.
    /// Used for diff highlighting in live-memory scenarios.
    /// Default: `false`.
    fn is_changed(&self, _offset: u64) -> bool {
        false
    }
    /// Called every frame when auto-refresh is enabled.
    /// Use this to trigger page re-fetches, poll for changes, etc.
    fn refresh(&mut self) {}
}

/// Default in-memory data provider wrapping `Vec<u8>`.
pub struct VecDataProvider {
    data: Vec<u8>,
    /// Optional reference snapshot for diff highlighting.
    reference: Option<Vec<u8>>,
}

impl VecDataProvider {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            reference: None,
        }
    }
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            reference: None,
        }
    }
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
    pub fn set_data_slice(&mut self, data: &[u8]) {
        self.data = data.to_vec();
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
    pub fn set_reference(&mut self, r: &[u8]) {
        self.reference = Some(r.to_vec());
    }
    pub fn clear_reference(&mut self) {
        self.reference = None;
    }
}

impl HexDataProvider for VecDataProvider {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize {
        let off = offset as usize;
        if off >= self.data.len() {
            return 0;
        }
        let end = (off + buf.len()).min(self.data.len());
        let n = end - off;
        buf[..n].copy_from_slice(&self.data[off..end]);
        n
    }
    fn write(&mut self, offset: u64, data: &[u8]) -> bool {
        let off = offset as usize;
        if off + data.len() > self.data.len() {
            return false;
        }
        self.data[off..off + data.len()].copy_from_slice(data);
        true
    }
    fn is_changed(&self, offset: u64) -> bool {
        if let Some(ref r) = self.reference {
            let i = offset as usize;
            if i < self.data.len() && i < r.len() {
                return self.data[i] != r[i];
            }
        }
        false
    }
}

// ── ArcVecDataProvider ──────────────────────────────────────────────────────
//
// `Arc<Vec<u8>>` wrapper that implements [`HexDataProvider`] over the
// shared-ownership buffer used internally by [`crate::hex_viewer::HexViewer`].
// Owns the held `Arc` so `write()` can route through `Arc::make_mut`
// on its own clone — that keeps reads in the same frame consistent
// with writes (the inner `Vec` is mutated in-place once the wrapper
// becomes the sole owner via the COW step).
//
// The legacy [`crate::hex_viewer::HexViewer::render`] entry point
// constructs this with [`Self::from_arc`] (a normal `Arc::clone`).
// Writes mirror through `Arc::make_mut`, so the wrapper's view stays
// in sync with what the user has typed — immediate visual feedback
// during a hex / ASCII edit. The viewer's internal `self.data` is
// NOT mutated by this provider; the legacy edit path's fallback in
// `commit_pending_edit_with` / `handle_hex_input` still patches
// `self.data` via its own `Arc::make_mut` so the buffer the host
// sees through `data()` / `data_arc()` matches what the user typed.
//
// Hosts that want true write-through to an external backing store
// (debugger memory pane, ROM patcher) should implement
// `HexDataProvider` directly and call [`HexViewer::render_with_provider`].
pub struct ArcVecDataProvider {
    data: std::sync::Arc<Vec<u8>>,
}

impl ArcVecDataProvider {
    pub fn from_arc(data: std::sync::Arc<Vec<u8>>) -> Self {
        Self { data }
    }
}

impl HexDataProvider for ArcVecDataProvider {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }
    fn read(&self, offset: u64, buf: &mut [u8]) -> usize {
        let off = offset as usize;
        if off >= self.data.len() {
            return 0;
        }
        let end = (off + buf.len()).min(self.data.len());
        let n = end - off;
        buf[..n].copy_from_slice(&self.data[off..end]);
        n
    }
    fn write(&mut self, offset: u64, data: &[u8]) -> bool {
        let off = offset as usize;
        if off + data.len() > self.data.len() {
            return false;
        }
        // `Arc::make_mut` is COW: on the first in-frame write the
        // inner `Vec` gets cloned (because the viewer's `self.data`
        // still holds a parallel `Arc`); subsequent writes in the
        // same frame are zero-copy. The caller — the legacy edit
        // path in `input.rs` — also patches `self.data` via its
        // own `Arc::make_mut` so the *next* frame's wrapper starts
        // from the post-edit bytes again.
        let buf = std::sync::Arc::make_mut(&mut self.data);
        buf[off..off + data.len()].copy_from_slice(data);
        true
    }
}

// ── Color Region ────────────────────────────────────────────────────────────

/// Color region — maps a byte range to a color and label for struct overlays.
#[derive(Debug, Clone)]
pub struct ColorRegion {
    /// Start offset in the data buffer.
    pub offset: usize,
    /// Length in bytes.
    pub len: usize,
    /// RGBA color `[r, g, b, a]` in `0.0..=1.0`.
    pub color: [f32; 4],
    /// Human-readable label (e.g. field name).
    pub label: String,
}

impl ColorRegion {
    pub fn new(offset: usize, len: usize, color: [f32; 4], label: impl Into<String>) -> Self {
        Self {
            offset,
            len,
            color,
            label: label.into(),
        }
    }
}

// ── Byte Category ───────────────────────────────────────────────────────────

/// Semantic byte category for 5-tier coloring (like debugger hex views).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteCategory {
    /// `0x00` — null bytes.
    Zero,
    /// `0x01..=0x1F`, `0x7F` — control characters.
    Control,
    /// `0x20..=0x7E` — printable ASCII.
    Printable,
    /// `0x80..=0xFE` — high / extended bytes.
    High,
    /// `0xFF` — all-ones byte.
    Full,
}

impl ByteCategory {
    /// Classify a byte value.
    pub fn of(byte: u8) -> Self {
        match byte {
            0x00 => Self::Zero,
            0x01..=0x1F | 0x7F => Self::Control,
            0x20..=0x7E => Self::Printable,
            0xFF => Self::Full,
            _ => Self::High, // 0x80..=0xFE
        }
    }
}
