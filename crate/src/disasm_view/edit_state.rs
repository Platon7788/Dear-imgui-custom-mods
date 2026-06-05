//! Inline-edit state types for `DisasmView`.
//!
//! Split out of `mod.rs` (audit session 043). The types + fields are
//! `pub(crate)` so the sibling render / input / draw submodules can
//! construct and read the active [`EditState`] (they live in different
//! submodules of `disasm_view` after the split, where a bare-private
//! field is not reachable), and so the `disasm_view::EditColumn`
//! re-export can carry crate-wide visibility.

/// Which column is being edited inline. The view supports
/// double-click-to-edit on three semantic regions:
///
/// - [`Self::Bytes`] — patch raw instruction bytes; commit path goes
///   through [`super::provider::DisasmDataProvider::write_bytes`].
/// - [`Self::Mnemonic`] — re-assemble the instruction from text;
///   commit path goes through
///   [`super::provider::DisasmDataProvider::assemble`]. Reserved for
///   a future editor that exposes the mnemonic+operands as a single
///   editable string. **Not** triggered by the UI yet.
/// - [`Self::Comment`] — set / clear the per-instruction comment;
///   commit path goes through
///   [`super::provider::DisasmDataProvider::set_comment`]. Wired in
///   2026-04-29.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditColumn {
    Bytes,
    #[allow(dead_code)]
    Mnemonic,
    Comment,
}

/// Inline editing state.
pub(crate) struct EditState {
    /// Index of the instruction being edited.
    pub(crate) idx: usize,
    /// Which column.
    pub(crate) column: EditColumn,
    /// Text buffer.
    pub(crate) buf: String,
    /// Frames since edit started — drives auto-focus on frame 0 and the
    /// "lost-focus → cancel" guard from frame 2 onwards (frame 1 is still
    /// in the focus-grab transition).
    pub(crate) frames: u32,
}
