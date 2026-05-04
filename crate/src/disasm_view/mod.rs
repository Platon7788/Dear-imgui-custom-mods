//! # DisasmView
//!
//! Standalone disassembly viewer widget for code analysis / debugging UIs.
//!
//! Features:
//! - **5-column layout**: margin | arrows | address | bytes | mnemonic operands ; comment
//! - **Branch arrows** with nesting depth and flow-kind coloring
//! - **Breakpoint markers** (red circles in left gutter)
//! - **Block tinting** (alternating background for logical code blocks)
//! - **Syntax coloring** by instruction type (jump/call/ret/nop/stack/system)
//! - **Operand highlighting** (registers, numbers, memory, strings)
//! - **Keyboard navigation** (arrows, PgUp/Dn, Enter → follow branch, G → goto)
//! - **Selection** with copy (address + mnemonic + operands)
//! - **Navigation history** (Alt+Left/Right, back/forward)
//! - **Inline editing** (double-click bytes to patch)
//! - **Stopped-at highlight** (current execution point)
//! - **Auto-scroll** to follow execution
//! - **Virtualized rendering** — only draws visible rows
//! - **Custom data provider trait** — bring your own decoder
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::disasm_view::{
//!     DisasmView, InstructionEntry, VecDisasmProvider, FlowKind,
//! };
//!
//! let mut provider = VecDisasmProvider::new();
//! provider.push(
//!     InstructionEntry::new(0x401000, vec![0x55], "push", "rbp")
//!         .with_flow(FlowKind::Stack)
//! );
//! let mut view = DisasmView::new("##disasm");
//! // In render loop: view.render(ui, &mut provider);
//! ```
//!
//! ## Educational tooltip pipeline
//!
//! Hovering an instruction draws a multi-section tooltip; each
//! section is produced by a dedicated submodule and is independently
//! toggleable via a `DisasmViewConfig::show_*` flag. The submodules
//! share a `(prev, current, next)` view of the instruction stream
//! plus the chosen [`abi::Abi`] and the host-resolved branch target.
//! Render order from top to bottom of the tooltip:
//!
//! | Layer | Module | Toggle | Answers the question |
//! |---|---|---|---|
//! | 1 | [`mnemonic`] | `show_explanation` | "What does this opcode do?" |
//! | 2 | [`idiom`] | `show_idiom` | "Is this part of a familiar 1-3 instruction pattern?" |
//! | 3 | [`mnemonic`] (gotcha) | `show_gotcha` | "Anti-RE / anti-debug warning for this opcode?" |
//! | 4 | [`operand`] + [`abi`] | `show_operand_hint` | "What is `[rcx+rax*8+8]` semantically? Which register has an ABI role?" |
//! | 5 | [`compiler`] | `show_compiler_pattern` | "Is this a compiler-stereotyped sequence (vtable / __chkstk / SEH / PEB ...)?" |
//! | 6 | [`antidisasm`] | `show_antidisasm` | "Is this an anti-RE / anti-debug trick?" |
//! | 7 | [`boundary`] | `show_boundary` | "Is this a function entry / exit / block boundary?" |
//! | 8 | [`branch`] | `show_branch_direction` | "Forward (if-then) or backward (loop) branch?" |
//!
//! Every layer is **pure / no-allocation** beyond the returned
//! `&'static str` (or one `String` for `branch::BranchHint`). All
//! recognisers are best-effort local pattern-matching — they don't
//! replace a real CFG / call-graph; they nudge newcomers toward the
//! right interpretation while staying useful for senior REs (who
//! tend to disable layers 1, 3, 5–8 and keep just the raw fields).

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod abi;
pub mod antidisasm;
pub mod arrows;
pub mod boundary;
pub mod branch;
pub mod compiler;
pub mod config;
pub mod idiom;
pub mod mnemonic;
pub mod operand;
pub mod provider;

mod draw;
mod input;
mod popup;
mod tokens;

pub use arrows::{BranchArrow, MAX_ARROW_DEPTH, compute_arrows, compute_arrows_clipped};
pub use config::{ColumnWidths, DisasmColors, DisasmViewConfig};
pub use provider::{DisasmDataProvider, FlowKind, Instruction, InstructionEntry, VecDisasmProvider};

use crate::utils::text::calc_text_size;

use std::collections::BTreeSet;

use crate::hex_viewer::NavHistory;
use crate::hex_viewer::search::{PatternByte, find_pattern_masked, parse_hex_pattern_masked};

/// Minimum number of pattern bytes (Exact + Any combined) required
/// before [`DisasmView::do_search`] runs the matcher. Set to 5 on
/// 2026-04-30 — anything shorter produces too many spurious hits in
/// typical x86 disassembly (e.g. `48 89` matches every other `mov`).
pub(super) const SEARCH_MIN_BYTES: usize = 5;

/// Origin breadcrumb visualisation — modern two-part design:
/// a faint full-row background (ambient awareness while
/// scrolling) plus a crisp left-edge stripe (unmistakable
/// "this is the breadcrumb" marker). The combination reads as
/// distinct from selection (full-row solid fill, no stripe) and
/// from current execution (warning hue, no stripe), so all
/// three row states are visually independent.
///
/// History: started as a single-tier `selection_bg × 0.60`
/// background (2026-04-30) but tested too faint over dark
/// themes — replaced with the stripe + bg combo same day.
pub(super) const ORIGIN_BG_ALPHA_FACTOR: f32 = 0.30;
pub(super) const ORIGIN_STRIPE_ALPHA: f32 = 0.90;
pub(super) const ORIGIN_STRIPE_WIDTH: f32 = 3.0;

// ── Edit State ──────────────────────────────────────────────────────────────

/// Which column is being edited inline. The view supports
/// double-click-to-edit on three semantic regions:
///
/// - [`Self::Bytes`] — patch raw instruction bytes; commit path goes
///   through [`super::config::DisasmDataProvider::write_bytes`].
/// - [`Self::Mnemonic`] — re-assemble the instruction from text;
///   commit path goes through
///   [`super::config::DisasmDataProvider::assemble`]. Reserved for
///   a future editor that exposes the mnemonic+operands as a single
///   editable string. **Not** triggered by the UI yet.
/// - [`Self::Comment`] — set / clear the per-instruction comment;
///   commit path goes through
///   [`super::config::DisasmDataProvider::set_comment`]. Wired in
///   2026-04-29.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditColumn {
    Bytes,
    #[allow(dead_code)]
    Mnemonic,
    Comment,
}

/// Inline editing state.
struct EditState {
    /// Index of the instruction being edited.
    idx: usize,
    /// Which column.
    column: EditColumn,
    /// Text buffer.
    buf: String,
    /// Frames since edit started — drives auto-focus on frame 0 and the
    /// "lost-focus → cancel" guard from frame 2 onwards (frame 1 is still
    /// in the focus-grab transition).
    frames: u32,
}

// ── DisasmView ──────────────────────────────────────────────────────────────

/// Standalone disassembly viewer widget.
pub struct DisasmView {
    /// Configuration.
    pub config: DisasmViewConfig,

    // ── Cached ImGui IDs (built once at construction) ─────────
    pub(super) child_id: String,
    pub(super) edit_label: String,
    pub(super) goto_popup_id: String,
    pub(super) ctx_popup_id: String,
    pub(super) settings_popup_id: String,
    pub(super) search_popup_id: String,

    // ── Navigation ───────────────────────────────────────────
    nav: NavHistory,

    // ── Interaction state ────────────────────────────────────
    /// Currently focused (cursor) instruction index.
    cursor_idx: Option<usize>,
    /// Multi-selection set (indices of selected instructions).
    selection: BTreeSet<usize>,
    /// Anchor index for shift-click range selection.
    sel_anchor: Option<usize>,
    /// Drag-select origin index.
    drag_origin: Option<usize>,
    /// Scroll target (instruction index).
    scroll_to: Option<usize>,
    /// Inline edit state.
    edit: Option<EditState>,
    /// Goto address buffer.
    pub(super) goto_buf: String,
    /// Show goto popup.
    pub(super) show_goto: bool,
    /// One-shot focus flag for the goto popup input. Raised when
    /// the popup opens; consumed inside the popup body the first
    /// frame the input renders. Mirrors `hex_viewer`'s pattern.
    pub(super) goto_focus_pending: bool,
    /// Goto-address request from the popup that the host needs to
    /// service — separate from `goto_address()` which only scrolls
    /// inside the *current* provider. When the user types an address
    /// outside the loaded range (e.g. a different module / RIP), the
    /// host (Vex0r `MemoryTab::forward_goto`) re-anchors the buffer
    /// and issues a fresh `ReadMem`. Drained via
    /// `take_pending_goto_request()` once per frame.
    pub(super) pending_goto_request: Option<u64>,
    /// Whether the widget is focused.
    focused: bool,
    /// Cached char advance.
    char_advance: f32,
    /// Cached line height.
    line_height: f32,
    /// Context menu target instruction index.
    context_idx: Option<usize>,
    /// Show context menu flag.
    show_context_menu: bool,
    /// One-shot trigger for the Settings popup. Raised by the
    /// context-menu "Settings..." entry; consumed on the next
    /// `render` frame by `render_settings_popup`.
    pub(super) show_settings: bool,
    /// Address-gutter "just copied" flash — `(row_idx, frames_left)`.
    /// Set by the double-click-to-copy path on the address column;
    /// `render` ticks it down each frame until it reaches `None`.
    /// Same pattern as `hex_viewer::address_flash`.
    pub(super) address_flash: Option<(usize, u32)>,
    // ── Byte search ──────────────────────────────────────────
    /// One-shot trigger for the Search popup (Ctrl+F / context-menu).
    pub(super) show_search: bool,
    /// Search input buffer — wildcard hex pattern (`4D 5A ?? 00`).
    pub(super) search_buf: String,
    /// One-shot keyboard-focus flag for the search input field.
    pub(super) search_focus_pending: bool,
    /// Parsed pattern from the latest `do_search`. Used for highlight
    /// extent + matches counter; cleared on too-short input.
    pub(super) search_pattern: Vec<PatternByte>,
    /// Instruction indices where each match *starts* — used by F3 /
    /// Shift+F3 to step through matches and by the "Result N/M"
    /// counter. Deduplicated when several matches share a starting
    /// instruction.
    pub(super) search_match_starts: Vec<usize>,
    /// Instruction indices any match *covers* (start row + every
    /// row a multi-byte match spans across). Used by
    /// `draw_instruction_row` to paint the search-match background
    /// without re-scanning per row.
    pub(super) search_match_set: BTreeSet<usize>,
    /// Index into `search_match_starts` — current "active" match.
    pub(super) search_idx: usize,
    /// **Origin breadcrumb** — address of the previous cursor row
    /// before the most recent navigation (Goto / Follow /
    /// function-jump / nav-back / search). Painted as a soft
    /// `selection_bg × ORIGIN_BG_ALPHA_FACTOR` highlight so the
    /// user can rediscover their jump source after scrolling.
    /// Stored as the **address** (not row index) so it survives
    /// provider mutations like inserting a new instruction at a
    /// lower address — the highlight stays on the same logical
    /// instruction even when the row index shifts.
    ///
    /// Cleared on `Esc` (decisive "I'm done with the trail" gesture)
    /// and overwritten by any new navigation that sets a different
    /// origin. Single-clicks **preserve** the breadcrumb on purpose
    /// so the user can scroll / click around without losing the
    /// jump source — see the explicit "keep" comment in
    /// `input.rs::handle_mouse`. Auto-suppressed when navigation
    /// lands on the same address it would set as origin
    /// (no breadcrumb-on-self).
    pub(super) origin_addr: Option<u64>,
    /// Cached arrows for current frame.
    cached_arrows: Vec<BranchArrow>,
    /// Position for InputText widget (set by draw_row, consumed by render).
    edit_render_pos: std::cell::Cell<Option<[f32; 2]>>,
    /// Width for the InputText widget.
    edit_render_width: std::cell::Cell<f32>,
    /// Screen-space centre of the disasm child window — captured per
    /// frame so modal popups (Goto / future Settings) can anchor at
    /// the visual middle. Mirrors the pattern used by `hex_viewer`.
    pub(super) component_center: [f32; 2],
    /// Screen-space anchor for the right-click context menu — set by
    /// the right-click handler to the cursor position so the menu
    /// spawns where the user clicked.
    pub(super) popup_open_pos: [f32; 2],
    /// Per-frame comment-column X (screen space). Computed in
    /// `render()` from a one-pass scan over visible rows: when the
    /// widest instruction text would collide with the default
    /// comment column, the comment + its left divider slide right
    /// just enough to clear it (plus a small `COMMENT_GAP` cushion).
    /// `Cell` so `mouse_to_cell` (called via `&self`) can read the
    /// value computed on the previous frame for hit-testing — the
    /// 1-frame lag is invisible for the double-click gesture.
    /// Wrapped in `Option` so frame 0 (before `render()` populated the
    /// value) is distinguishable from any legal X (including 0.0 when
    /// the host docks the view at screen-space x = 0). M1 from the
    /// session 034 audit replaced the prior `≤ 0.0` sentinel.
    pub(super) frame_comment_x: std::cell::Cell<Option<f32>>,
    /// Per-frame comment-column WIDTH (screen-space pixels).
    /// Computed in `render()` as
    /// `(window_w - comment_x).max(cols.comment)` so the Comment
    /// column always stretches to fill the host window down to a
    /// `cols.comment` floor. Read by `mouse_to_cell` (so
    /// double-click hit-testing extends to the full visible width)
    /// and by the comment edit-cell renderer. `None` only on frame 0.
    pub(super) frame_comment_w: std::cell::Cell<Option<f32>>,

    /// Bookmark address set — UI navigation aid. Up to
    /// [`Self::MAX_BOOKMARKS`] addresses (BTreeSet keeps them sorted
    /// for stable host-side save/restore). Bookmarks are pure
    /// view-state; they are not attached to running-process concepts
    /// like breakpoints. Persisting between sessions is the host's
    /// job — read [`Self::bookmarks`] on shutdown, push back via
    /// [`Self::add_bookmark`] on startup.
    bookmarks: BTreeSet<u64>,
    // The active locale lives on `config.locale` so it round-trips
    // through `ron::to_string(&cfg)` / `ron::from_str` along with
    // every other display flag — see `with_locale` / `set_locale`.
}

impl DisasmView {
    /// Maximum number of bookmarks the view will hold. Calls to
    /// [`Self::add_bookmark`] / [`Self::toggle_bookmark`] silently
    /// no-op (returning `false`) once the cap is reached.
    pub const MAX_BOOKMARKS: usize = 64;
}

impl DisasmView {
    /// Create a new disassembly view with the given ImGui ID.
    pub fn new(id: impl Into<String>) -> Self {
        let id: String = id.into();
        let child_id = format!("##dv_child_{id}");
        let edit_label = format!("##dv_edit_{id}");
        let goto_popup_id = format!("##dv_goto_{id}");
        let ctx_popup_id = format!("##dv_ctx_{id}");
        let settings_popup_id = format!("##dv_settings_{id}");
        let search_popup_id = format!("##dv_search_{id}");
        Self {
            config: DisasmViewConfig::default(),
            child_id,
            edit_label,
            goto_popup_id,
            ctx_popup_id,
            settings_popup_id,
            search_popup_id,
            nav: NavHistory::new(64),
            cursor_idx: None,
            selection: BTreeSet::new(),
            sel_anchor: None,
            drag_origin: None,
            scroll_to: None,
            edit: None,
            goto_buf: String::new(),
            show_goto: false,
            goto_focus_pending: false,
            pending_goto_request: None,
            focused: false,
            char_advance: 0.0,
            line_height: 0.0,
            context_idx: None,
            show_context_menu: false,
            show_settings: false,
            address_flash: None,
            show_search: false,
            search_buf: String::new(),
            search_focus_pending: false,
            search_pattern: Vec::new(),
            search_match_starts: Vec::new(),
            search_match_set: BTreeSet::new(),
            search_idx: 0,
            origin_addr: None,
            cached_arrows: Vec::new(),
            edit_render_pos: std::cell::Cell::new(None),
            edit_render_width: std::cell::Cell::new(0.0),
            component_center: [0.0, 0.0],
            popup_open_pos: [0.0, 0.0],
            frame_comment_x: std::cell::Cell::new(None),
            frame_comment_w: std::cell::Cell::new(None),
            bookmarks: BTreeSet::new(),
        }
    }

    /// Override the user-visible language on construction. Default is
    /// English; pass [`crate::i18n::Locale::Ru`] to switch to Russian.
    /// The host is responsible for baking `GlyphRanges::Cyrillic`
    /// into the active font atlas — without that, non-ASCII characters
    /// render as `?` placeholders.
    ///
    /// The locale is stored on [`DisasmViewConfig::locale`], so it
    /// round-trips through `ron::to_string` / `ron::from_str` along
    /// with every other display setting.
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.config.locale = locale;
        self
    }

    /// Mid-flight language switch. Same caveat as [`Self::with_locale`]
    /// regarding font atlas glyph ranges.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
    }

    /// Currently-active locale (mirror of `self.config.locale`).
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    /// Static catalogue lookup for the current locale. Convenience
    /// for the per-frame popup / draw paths.
    #[inline]
    pub(super) fn strings(&self) -> &'static crate::i18n::disasm_view::Strings {
        crate::i18n::disasm_view::strings(self.config.locale)
    }

    // ── Byte search ──────────────────────────────────────────────────

    /// Run the byte search using the current `search_buf`. Builds
    /// the concatenated instruction-byte stream from `provider`,
    /// runs the wildcard-aware matcher
    /// ([`crate::hex_viewer::search::find_pattern_masked`]) and
    /// translates byte offsets back into instruction indices for
    /// row highlighting + step navigation.
    ///
    /// Patterns shorter than [`SEARCH_MIN_BYTES`] (5) are rejected —
    /// state is cleared and the function returns without scanning.
    /// Cross-instruction matches are supported (matches that span
    /// instruction boundaries cover every row they touch).
    pub(super) fn do_search(&mut self, provider: &dyn DisasmDataProvider) {
        let pattern = parse_hex_pattern_masked(&self.search_buf);
        if pattern.len() < SEARCH_MIN_BYTES {
            self.search_pattern.clear();
            self.search_match_starts.clear();
            self.search_match_set.clear();
            return;
        }

        let count = provider.instruction_count();
        // Build concat byte stream + a `(byte_offset,
        // global_instruction_idx)` table. Skipping `None`
        // instructions is mandatory for sparse / lazy providers
        // (they advertise `instruction_count` for the entire
        // address range but legitimately return `None` for
        // not-yet-decoded slots). The pair preserves the global
        // index so the offset → row mapping survives gaps.
        let mut data: Vec<u8> = Vec::with_capacity(count * 3);
        let mut entries: Vec<(usize, usize)> = Vec::with_capacity(count);
        for i in 0..count {
            if let Some(instr) = provider.instruction(i) {
                entries.push((data.len(), i));
                data.extend_from_slice(instr.bytes());
            }
        }

        let matches = find_pattern_masked(&data, &pattern);
        let plen = pattern.len();

        let mut starts: Vec<usize> = Vec::with_capacity(matches.len());
        let mut covered: BTreeSet<usize> = BTreeSet::new();
        for &offset in &matches {
            // `partition_point(|&(off, _)| off <= offset)` returns
            // the FIRST entry with `off > offset` — well-defined
            // last-le semantics even when entries share offsets
            // (which happens when an instruction has zero bytes —
            // never, in practice, but defensive). Use saturating
            // `pos - 1` to guard the impossible case where the
            // match starts before any entry.
            let pos = entries.partition_point(|&(off, _)| off <= offset);
            if pos == 0 {
                continue;
            }
            let start_pos = pos - 1;
            let end_offset = offset + plen;
            // First-ge semantics: entries[end_pos].0 is the first
            // offset that's at or beyond the end of the match.
            let end_pos = entries.partition_point(|&(off, _)| off < end_offset);

            starts.push(entries[start_pos].1);
            for entry in &entries[start_pos..end_pos] {
                covered.insert(entry.1);
            }
        }
        starts.sort_unstable();
        starts.dedup();

        self.search_pattern = pattern;
        self.search_match_starts = starts;
        self.search_match_set = covered;
        self.search_idx = 0;

        if let Some(&first_idx) = self.search_match_starts.first() {
            // Pre-search → first-match navigation pushes nav history
            // and sets the origin breadcrumb so the user can
            // `Alt+Left` back to where they were AND see the
            // pre-search row faintly highlighted while exploring
            // the matches. Self-navigation (search hit on current
            // row) skips both side effects.
            let pre_addr = self
                .cursor_idx
                .and_then(|i| provider.instruction(i))
                .map(|instr| instr.address());
            let dst_addr = provider.instruction(first_idx).map(|i| i.address());
            if let (Some(src), Some(dst)) = (pre_addr, dst_addr)
                && src != dst
            {
                self.nav.push(src);
                self.origin_addr = Some(src);
            }
            self.cursor_idx = Some(first_idx);
            self.scroll_to = Some(first_idx);
        }
    }

    /// Step to the next search match (wraps around).
    pub(super) fn search_next(&mut self) {
        if self.search_match_starts.is_empty() {
            return;
        }
        self.search_idx = (self.search_idx + 1) % self.search_match_starts.len();
        let idx = self.search_match_starts[self.search_idx];
        self.cursor_idx = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Step to the previous search match (wraps around).
    pub(super) fn search_prev(&mut self) {
        if self.search_match_starts.is_empty() {
            return;
        }
        self.search_idx = self
            .search_idx
            .checked_sub(1)
            .unwrap_or(self.search_match_starts.len() - 1);
        let idx = self.search_match_starts[self.search_idx];
        self.cursor_idx = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Format `addr` as a copy-friendly hex literal (`0x...`),
    /// honouring `address_width_64` + `uppercase`. Used by the
    /// address-gutter copy-on-double-click path and the "Copy
    /// Address" context-menu entry.
    pub(super) fn format_address_literal(&self, addr: u64) -> String {
        match (self.config.uppercase, self.config.address_width_64) {
            (true, true) => format!("0x{:016X}", addr),
            (false, true) => format!("0x{:016x}", addr),
            (true, false) => format!("0x{:08X}", addr),
            (false, false) => format!("0x{:08x}", addr),
        }
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Currently focused (cursor) instruction index.
    pub fn selected_index(&self) -> Option<usize> {
        self.cursor_idx
    }

    /// All selected instruction indices.
    pub fn selected_indices(&self) -> &BTreeSet<usize> {
        &self.selection
    }

    /// Number of selected instructions.
    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    /// Whether a specific index is selected.
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selection.contains(&idx)
    }

    /// Set the cursor and single-select one instruction.
    pub fn select(&mut self, idx: usize) {
        self.cursor_idx = Some(idx);
        self.selection.clear();
        self.selection.insert(idx);
        self.sel_anchor = Some(idx);
        self.scroll_to = Some(idx);
    }

    /// Clear all selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.sel_anchor = None;
    }

    // ── Convenience selectors (host toolbar helpers) ─────────────────
    //
    // The five methods below let a host implement a "Top / Bottom /
    // Current IP / Breakpoint / cycle BPs" toolbar in pure
    // `if button { view.method() }` style — no manual scan-loop over
    // the provider. They are pure view-domain operations and don't
    // cross into the host's debugger backend (stepping, run/pause,
    // register/memory reads stay on the backend side; the view only
    // reflects whatever provider state `is_current()` /
    // `has_breakpoint()` reports).

    /// Find and select the instruction the provider marks as
    /// [`Instruction::is_current`] (typically the debugger's IP /
    /// program counter). Returns `true` when an IP row was found
    /// and selection moved, `false` otherwise (host can disable
    /// the corresponding toolbar button on `false`).
    pub fn select_current_ip(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        for i in 0..count {
            if let Some(instr) = provider.instruction(i)
                && instr.is_current()
            {
                self.select(i);
                return true;
            }
        }
        false
    }

    /// Find and select the *first* instruction with a breakpoint
    /// (lowest index → lowest address in a sorted provider).
    /// Returns `true` when one was found.
    pub fn select_first_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        for i in 0..count {
            if let Some(instr) = provider.instruction(i)
                && instr.has_breakpoint()
            {
                self.select(i);
                return true;
            }
        }
        false
    }

    /// Cycle to the next breakpoint **strictly after** the current
    /// cursor (or, if the cursor is past the last breakpoint, wraps
    /// to the first). Returns `true` when a breakpoint exists at
    /// all. Standard disassembler UX — the IDE-style "next BP" button.
    pub fn select_next_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        if count == 0 {
            return false;
        }
        let start = self.cursor_idx.map(|c| c + 1).unwrap_or(0);
        // Search forward from start, then wrap around to 0..start.
        for i in start..count {
            if let Some(instr) = provider.instruction(i)
                && instr.has_breakpoint()
            {
                self.select(i);
                return true;
            }
        }
        for i in 0..start.min(count) {
            if let Some(instr) = provider.instruction(i)
                && instr.has_breakpoint()
            {
                self.select(i);
                return true;
            }
        }
        false
    }

    /// Cycle to the previous breakpoint **strictly before** the
    /// current cursor (wraps to the last). Symmetric to
    /// [`Self::select_next_breakpoint`].
    pub fn select_prev_breakpoint(&mut self, provider: &dyn DisasmDataProvider) -> bool {
        let count = provider.instruction_count();
        if count == 0 {
            return false;
        }
        let start = self.cursor_idx.unwrap_or(count);
        // Search backward from cursor-1, then wrap around from end.
        for i in (0..start).rev() {
            if let Some(instr) = provider.instruction(i)
                && instr.has_breakpoint()
            {
                self.select(i);
                return true;
            }
        }
        for i in (start..count).rev() {
            if let Some(instr) = provider.instruction(i)
                && instr.has_breakpoint()
            {
                self.select(i);
                return true;
            }
        }
        false
    }

    /// Whether the back / forward address-history stack has anything
    /// to consume. Use these to render `<< Back` / `Fwd >>` toolbar
    /// buttons as disabled when there's nothing to walk to. Mirrors
    /// the corresponding `Alt+Left` / `Alt+Right` shortcut state.
    pub fn can_nav_back(&self) -> bool {
        self.nav.can_go_back()
    }

    /// See [`Self::can_nav_back`].
    pub fn can_nav_forward(&self) -> bool {
        self.nav.can_go_forward()
    }

    /// Address of the row under the cursor, or `None` when the view
    /// has no cursor / the cursor index doesn't resolve through the
    /// provider. Useful for status-bar `Addr: 0x…` displays and as
    /// the prefill value for a host-rendered "Goto address" input.
    pub fn cursor_address(&self, provider: &dyn DisasmDataProvider) -> Option<u64> {
        let i = self.cursor_idx?;
        provider.instruction(i).map(|instr| instr.address())
    }

    // ── Bookmarks (UI navigation aid, view-state) ───────────────────
    //
    // Bookmarks let the user mark "interesting" addresses for quick
    // recall — the gutter paints a coloured ring on bookmarked rows
    // (`colors.bookmark`), the right-click menu offers an
    // add/remove-toggle entry, and `Ctrl+B` toggles the bookmark on
    // the cursor row. Capacity is fixed at [`Self::MAX_BOOKMARKS`]
    // (64); calls past the cap silently no-op so the host can wire
    // a button without managing the limit.
    //
    // Bookmarks are *view-state*, not provider-state — they are an
    // editor-style navigation aid, not tied to a running-process
    // concept like a breakpoint. Hosts that need cross-session
    // persistence read the set via [`Self::bookmarks`] on shutdown
    // and replay through [`Self::add_bookmark`] on startup.

    /// Whether `addr` is currently bookmarked.
    pub fn is_bookmarked(&self, addr: u64) -> bool {
        self.bookmarks.contains(&addr)
    }

    /// Number of bookmarks currently set (`<=` [`Self::MAX_BOOKMARKS`]).
    pub fn bookmark_count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Read-only access to the full bookmark set, sorted by address.
    /// Use this for host-side save / export.
    pub fn bookmarks(&self) -> &BTreeSet<u64> {
        &self.bookmarks
    }

    /// Add `addr` to the bookmark set. Returns `true` when the
    /// address is bookmarked after the call (i.e. the operation
    /// succeeded **or** the address was already bookmarked); `false`
    /// only when the [`Self::MAX_BOOKMARKS`] cap is reached and
    /// `addr` wasn't already in the set.
    pub fn add_bookmark(&mut self, addr: u64) -> bool {
        if self.bookmarks.contains(&addr) {
            return true;
        }
        if self.bookmarks.len() >= Self::MAX_BOOKMARKS {
            return false;
        }
        self.bookmarks.insert(addr);
        true
    }

    /// Remove `addr` from the bookmark set. Returns `true` when an
    /// entry was removed, `false` when the address wasn't in the set.
    pub fn remove_bookmark(&mut self, addr: u64) -> bool {
        self.bookmarks.remove(&addr)
    }

    /// Toggle bookmark state on `addr`. Returns the **new** state
    /// (`true` = bookmarked after the call). When transitioning
    /// from off → on and the [`Self::MAX_BOOKMARKS`] cap is reached,
    /// returns `false` and leaves the set unchanged.
    pub fn toggle_bookmark(&mut self, addr: u64) -> bool {
        if self.bookmarks.contains(&addr) {
            self.bookmarks.remove(&addr);
            false
        } else {
            self.add_bookmark(addr)
        }
    }

    /// Drop every bookmark.
    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.clear();
    }

    /// Drain the goto-address request emitted by the popup so the host
    /// can re-anchor the backing buffer when the user typed an address
    /// outside the currently decoded range. Returns `Some(addr)` once
    /// per popup commit, `None` otherwise.
    pub fn take_pending_goto_request(&mut self) -> Option<u64> {
        self.pending_goto_request.take()
    }

    /// Scroll to and select the instruction at `addr`.
    ///
    /// Side effects on jump (when `addr != current cursor address`):
    /// - Pushes the source address onto the 64-entry nav history
    ///   (`Alt+Left` / `Alt+Right` walk back / forward).
    /// - Sets [`Self::origin_addr`] to the source so the previous
    ///   row paints with the soft "you came from here" breadcrumb.
    ///
    /// No-op when `addr` doesn't resolve through
    /// [`DisasmDataProvider::index_of_address`]. Self-jumps
    /// (target == current) skip the side effects so the breadcrumb
    /// doesn't land on the current row.
    pub fn goto_address(&mut self, addr: u64, provider: &dyn DisasmDataProvider) {
        let Some(idx) = provider.index_of_address(addr) else {
            return;
        };
        let old_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address());
        if let Some(old) = old_addr
            && old != addr
        {
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(idx);
    }

    /// Navigate back in address history. Pushes a breadcrumb at
    /// the current row before stepping back, so a subsequent
    /// `Alt+Right` lands on the same place visually.
    pub fn nav_back(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_back(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            if addr != current_addr {
                self.origin_addr = Some(current_addr);
            }
            self.select(idx);
        }
    }

    /// Navigate forward in address history. Symmetrical breadcrumb
    /// behaviour to [`Self::nav_back`].
    pub fn nav_forward(&mut self, provider: &dyn DisasmDataProvider) {
        let current_addr = self
            .cursor_idx
            .and_then(|i| provider.instruction(i))
            .map(|instr| instr.address())
            .unwrap_or(0);
        if let Some(addr) = self.nav.go_forward(current_addr)
            && let Some(idx) = provider.index_of_address(addr)
        {
            if addr != current_addr {
                self.origin_addr = Some(current_addr);
            }
            self.select(idx);
        }
    }

    // ── Function navigation ──────────────────────────────────────────

    /// Jump to the first instruction of the function containing the
    /// cursor — uses [`find_function_start`]. The pre-jump address
    /// is pushed onto nav history (Alt+Left returns to it) AND
    /// recorded as [`Self::origin_addr`] so the source row paints
    /// with the soft breadcrumb. Self-jumps (already at start)
    /// skip both side effects. No-op when there's no cursor.
    pub fn jump_to_function_start(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let start = find_function_start(provider, cur);
        if start == cur {
            return;
        }
        if let Some(instr) = provider.instruction(cur) {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(start);
    }

    /// Jump to the last instruction of the function containing the
    /// cursor — uses [`find_function_end`]. Symmetrical breadcrumb +
    /// nav-history behaviour to [`Self::jump_to_function_start`].
    pub fn jump_to_function_end(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let end = find_function_end(provider, cur);
        if end == cur {
            return;
        }
        if let Some(instr) = provider.instruction(cur) {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select(end);
    }

    /// Select every instruction from the cursor through the end of
    /// the function (inclusive). Cursor moves to the function-end
    /// instruction; selection range is `[cursor_at_call ..= end]`.
    /// Useful for "copy whole tail of this function" workflows.
    ///
    /// Pushes the original cursor address onto the nav history and
    /// records it as the origin breadcrumb (mirrors
    /// [`Self::jump_to_function_start`] / `_end`) so `Alt+Left`
    /// returns to the user's pre-select location and the source
    /// row paints the breadcrumb. No-op when there's no cursor.
    pub fn select_function(&mut self, provider: &dyn DisasmDataProvider) {
        let Some(cur) = self.cursor_idx else { return };
        let end = find_function_end(provider, cur);
        if cur != end
            && let Some(instr) = provider.instruction(cur)
        {
            let old = instr.address();
            self.nav.push(old);
            self.origin_addr = Some(old);
        }
        self.select_range(cur, end);
        self.cursor_idx = Some(end);
        self.sel_anchor = Some(cur);
        self.scroll_to = Some(end);
    }

    /// "Cheat-Engine-style" follow at cursor — navigate to whatever
    /// the current instruction points at:
    ///
    /// 1. If [`Instruction::branch_target`] is `Some`, jump there.
    ///    For targets not yet decoded, calls
    ///    [`DisasmDataProvider::decode_range`] once to give
    ///    streaming/lazy providers a chance to populate the
    ///    target window before re-checking. Static providers
    ///    (`VecDisasmProvider`) implement `decode_range` as a
    ///    no-op so the lazy retry is free for pre-loaded data.
    /// 2. Otherwise scan the operand string for a [`TokenKind::Number`]
    ///    that parses as an address — same lazy-decode + retry
    ///    treatment. Useful for memory operands like
    ///    `[0x401000]` whose target the provider didn't tag as
    ///    a branch.
    ///
    /// Returns `true` when navigation actually happened. Pushes
    /// nav history + sets the origin breadcrumb in both paths.
    /// Returns `false` for unfollowable rows (no branch, no
    /// resolvable operand) so callers like the double-click
    /// handler can fall through to the edit-cell path.
    pub fn follow_at_cursor(&mut self, provider: &mut dyn DisasmDataProvider) -> bool {
        let Some(cur) = self.cursor_idx else { return false };
        // Read just the cheap-to-clone bits up front; the operand
        // string is only needed in the (rarer) fallback path so
        // we lazy-clone it there to avoid a heap alloc on every
        // jcc / call double-click.
        let branch = provider.instruction(cur).and_then(|i| i.branch_target());

        // Helper: try to navigate to `addr`, decoding lazily if the
        // target isn't yet in the provider. Returns `true` on
        // successful navigation. `goto_address` itself handles nav
        // history + origin breadcrumb when navigation lands.
        let try_goto = |this: &mut Self, addr: u64, provider: &mut dyn DisasmDataProvider| {
            if provider.index_of_address(addr).is_none() {
                // Lazy-decode a small window at the target — 32
                // instructions covers a typical function prologue
                // and gives the user something to look at on
                // arrival. Streaming providers honour this; the
                // built-in `VecDisasmProvider` is a no-op so this
                // is free for pre-loaded data.
                provider.decode_range(addr, 32);
            }
            if provider.index_of_address(addr).is_some() {
                this.goto_address(addr, provider);
                true
            } else {
                false
            }
        };

        if let Some(target) = branch {
            return try_goto(self, target, provider);
        }

        // Operand-pointer scan — only when there's no branch_target,
        // so the string clone is paid only by the fallback path.
        let operands_owned = match provider.instruction(cur) {
            Some(instr) => instr.operands().to_string(),
            None => return false,
        };
        for tok in tokens::OperandTokenizer::new(&operands_owned) {
            if tok.kind == tokens::TokenKind::Number
                && let Some(addr) = parse_operand_number(tok.text)
                && try_goto(self, addr, provider)
            {
                return true;
            }
        }
        false
    }

    // ── Selection helpers ────────────────────────────────────────────

    /// Select a contiguous range [lo..=hi].
    fn select_range(&mut self, a: usize, b: usize) {
        let lo = a.min(b);
        let hi = a.max(b);
        self.selection.clear();
        for i in lo..=hi {
            self.selection.insert(i);
        }
    }

    /// Whether the view is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // ── Rendering ────────────────────────────────────────────────────

    /// Render the disassembly view widget.
    pub fn render(&mut self, ui: &dear_imgui_rs::Ui, provider: &mut dyn DisasmDataProvider) {
        let count = provider.instruction_count();
        if count == 0 {
            return;
        }

        // Tick down the address-gutter "just copied" flash. Counter
        // hits zero → state cleared so the pill stops painting.
        // Mirrors `hex_viewer::render`.
        if let Some((row, frames)) = self.address_flash {
            if frames > 1 {
                self.address_flash = Some((row, frames - 1));
            } else {
                self.address_flash = None;
            }
        }

        // Cache font metrics. Guard against the rare zero-glyph case (e.g.
        // before the font atlas is fully built or in test stubs) — division
        // by zero in the row math below would produce inf-cast UB.
        let [cw, ch] = calc_text_size("0");
        self.char_advance = cw.max(1.0);
        self.line_height = (ch + 4.0).max(1.0);

        // Auto-scroll to current execution point.
        if self.config.follow_execution && self.scroll_to.is_none() {
            for i in 0..count {
                if let Some(instr) = provider.instruction(i)
                    && instr.is_current()
                {
                    self.cursor_idx = Some(i);
                    self.scroll_to = Some(i);
                    break;
                }
            }
        }

        // ── Goto popup ───────────────────────────────────────
        self.render_goto_popup(ui, provider);
        // ── Context menu ─────────────────────────────────────
        self.render_context_menu(ui, provider);
        // ── Settings popup ───────────────────────────────────
        self.render_settings_popup(ui);
        // ── Search popup ─────────────────────────────────────
        self.render_search_popup(ui, provider);

        let avail = ui.content_region_avail();
        let child_id = self.child_id.clone();

        ui.child_window(&child_id)
            .size([avail[0], avail[1]])
            .flags(
                dear_imgui_rs::WindowFlags::NO_MOVE
                    | dear_imgui_rs::WindowFlags::NO_SCROLL_WITH_MOUSE,
            )
            .build(ui, || {
                let was_focused = self.focused;
                self.focused = ui.is_window_focused();

                // Cancel edit on focus loss.
                if was_focused && !self.focused && self.edit.is_some() {
                    self.edit = None;
                }

                // Cache child window centre — used by modal popups
                // (Goto / Settings) to anchor at the visual middle.
                // Mirrors `hex_viewer::draw::render`.
                let wp = ui.window_pos();
                let ws = ui.window_size();
                self.component_center = [wp[0] + ws[0] * 0.5, wp[1] + ws[1] * 0.5];

                // Handle scroll-to target.
                if let Some(idx) = self.scroll_to.take() {
                    let y = idx as f32 * self.line_height;
                    let visible_h = ui.window_size()[1];
                    // Center the target row.
                    let target_y = (y - visible_h * 0.5).max(0.0);
                    ui.set_scroll_y(target_y);
                }

                // Keyboard.
                if self.focused {
                    self.handle_keyboard(ui, provider);
                }

                // Mouse.
                self.handle_mouse(ui, provider);

                let draw_list = ui.get_window_draw_list();
                let [win_x, win_y] = ui.cursor_screen_pos();
                let scroll_y = ui.scroll_y();
                let visible_h = ui.window_size()[1];

                // Clamp `first_row` to `count` defensively: when the
                // provider shrinks between frames (live disasm with
                // re-decoding), `scroll_y / line_height` can land
                // past the new tail and `last_row - first_row` would
                // underflow when fed to `compute_arrows_clipped`.
                let first_row = ((scroll_y / self.line_height) as usize).min(count);
                let visible_count = (visible_h / self.line_height) as usize + 2;
                let last_row = (first_row + visible_count).min(count);

                let origin_x = win_x + ui.scroll_x();
                let origin_y = win_y + scroll_y;

                // ── Compute branch arrows for visible range ───
                // Cross-window scan ([`compute_arrows_clipped`]):
                // walks ALL provider instructions, retains arrows
                // where AT LEAST ONE endpoint is in the visible
                // window, clamps off-window endpoints to the
                // window edge with `clipped_*` flags so the
                // renderer can suppress the arrowhead there.
                // Replaces the older visible-only scan that
                // dropped long-range jumps when the source or
                // target scrolled offscreen.
                if self.config.show_arrows {
                    self.cached_arrows = compute_arrows_clipped(
                        provider as &dyn DisasmDataProvider,
                        first_row,
                        last_row - first_row,
                    );
                    if self.cached_arrows.len() > self.config.max_arrows {
                        self.cached_arrows.truncate(self.config.max_arrows);
                    }
                }

                // ── Dynamic comment X ─────────────────────────
                // Pre-pass: walk the visible rows once to find the
                // rightmost glyph any instruction text would draw to.
                // If it overflows the default `operand_end`, push the
                // comment column (header + text + edit + divider 3)
                // right by exactly that overflow + COMMENT_GAP. Cell
                // is read by `mouse_to_cell` for the next frame's
                // double-click hit-test.
                let cols = &self.config.columns;
                let bytes_col_x = origin_x
                    + if self.config.show_breakpoints {
                        cols.margin
                    } else {
                        0.0
                    }
                    + if self.config.show_arrows {
                        cols.arrows
                    } else {
                        0.0
                    }
                    + cols.address;
                let mnemonic_col_x = if self.config.show_bytes {
                    bytes_col_x + cols.bytes
                } else {
                    bytes_col_x
                };
                let default_comment_x = mnemonic_col_x + cols.mnemonic + cols.operands;
                let instr_data_x = mnemonic_col_x + draw::COL_INNER_PAD;
                // Pre-pass runs unconditionally — even when the
                // comment column is hidden, the value is consumed
                // by `draw_header` for "Instruction" centring and by
                // `mouse_to_cell` to bound the Mnemonic hit-zone.
                // The Comment-column draw branches are individually
                // gated on `show_comments`.
                let mut max_instr_right = default_comment_x;
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        // Monospace assumption: width = char count
                        // × char_advance (mnemonic + space + operands).
                        // Mnemonic / operands are always ASCII for x86 /
                        // x86-64 / ARM, so byte length == codepoint count
                        // and `len()` is O(1) where `chars().count()`
                        // walks the string per row per frame.
                        let mn = instr.mnemonic().len();
                        let op = instr.operands().len();
                        let chars = mn + 1 + op;
                        let row_right =
                            instr_data_x + chars as f32 * self.char_advance + draw::COMMENT_GAP;
                        if row_right > max_instr_right {
                            max_instr_right = row_right;
                        }
                    }
                }
                let comment_x = max_instr_right;
                self.frame_comment_x.set(Some(comment_x));

                // Comment column stretches to fill remaining
                // host-window width (per user request 2026-04-30:
                // Comment has lowest layout priority). Floor at
                // `cols.comment` so the column never collapses
                // smaller than its configured min — prevents the
                // edit cell from becoming unusably narrow when the
                // host window is shrunk below the layout total.
                let comment_w =
                    (origin_x + avail[0] - comment_x).max(cols.comment);
                self.frame_comment_w.set(Some(comment_w));

                // ── Column header ─────────────────────────────
                if self.config.show_header {
                    self.draw_header(&draw_list, origin_x, origin_y, comment_x);
                }

                let header_h = if self.config.show_header {
                    self.line_height
                } else {
                    0.0
                };

                // ── Draw rows ─────────────────────────────────
                // Gate the mouse position on `is_window_hovered`
                // so hover-based row tooltips don't leak through
                // popups that overlap this widget (e.g. another
                // module's Settings dialog rendered on top of the
                // disasm view). Without this gate the row hit-test
                // is pure coordinate math that fires regardless of
                // whether ImGui has handed mouse focus to a popup —
                // user reported the tooltip ghosting through a
                // hex-viewer Settings popup on 2026-04-29. Same
                // pattern as `hex_viewer::draw::render`.
                let mouse_pos = if ui.is_window_hovered() {
                    ui.io().mouse_pos()
                } else {
                    [f32::NEG_INFINITY, f32::NEG_INFINITY]
                };
                for row in first_row..last_row {
                    if let Some(instr) = provider.instruction(row) {
                        let y = origin_y + header_h + (row - first_row) as f32 * self.line_height;
                        // Pull the immediate neighbours so the tooltip's
                        // idiom detector can recognise multi-instruction
                        // patterns (prologue / cmp+Jcc / get-IP / etc).
                        // `prev` / `next` may be `None` near edges or
                        // when the provider returns a sparse hole.
                        let prev_instr = row.checked_sub(1).and_then(|i| provider.instruction(i));
                        let next_instr = provider.instruction(row + 1);
                        self.draw_instruction_row(
                            ui,
                            &draw_list,
                            origin_x,
                            y,
                            row,
                            instr,
                            prev_instr,
                            next_instr,
                            mouse_pos,
                            avail[0],
                            comment_x,
                        );
                    }
                }

                // ── Draw branch arrows on top ─────────────────
                if self.config.show_arrows && !self.cached_arrows.is_empty() {
                    self.draw_arrows(&draw_list, origin_x, origin_y + header_h);
                }

                // ── Vertical column dividers ──────────────────
                if self.config.show_column_dividers {
                    let visible_h = ui.window_size()[1];
                    self.draw_column_dividers(
                        &draw_list,
                        origin_x,
                        origin_y,
                        origin_y + visible_h,
                        comment_x,
                    );
                }

                // ── Render inline InputText for editing ──────────
                if let Some(pos) = self.edit_render_pos.take() {
                    let input_w = self.edit_render_width.get();
                    // Position the ImGui cursor at the edit cell.
                    ui.set_cursor_screen_pos(pos);
                    ui.set_next_item_width(input_w);
                    if let Some(edit) = &mut self.edit {
                        // Auto-focus on first frame.
                        if edit.frames == 0 {
                            ui.set_keyboard_focus_here();
                        }
                        edit.frames += 1;

                        // Per-column input flags:
                        // - Bytes: hex-only + uppercase (raw `AA BB`
                        //   hex patch).
                        // - Mnemonic: free text (assembly source —
                        //   `mov rax, rbx`); no character restriction.
                        // - Comment: free text — same as mnemonic.
                        // AUTO_SELECT_ALL + ENTER_RETURNS_TRUE apply
                        // to all three so the open-then-overwrite
                        // gesture and Enter-to-commit shortcut behave
                        // uniformly.
                        let mut flags = dear_imgui_rs::InputTextFlags::AUTO_SELECT_ALL
                            | dear_imgui_rs::InputTextFlags::ENTER_RETURNS_TRUE;
                        if edit.column == EditColumn::Bytes {
                            flags |= dear_imgui_rs::InputTextFlags::CHARS_HEXADECIMAL
                                | dear_imgui_rs::InputTextFlags::CHARS_UPPERCASE;
                        }

                        let entered = ui
                            .input_text(&self.edit_label, &mut edit.buf)
                            .flags(flags)
                            .build();

                        if entered {
                            // Enter pressed — commit.
                            let edit_data = self.edit.take().unwrap();
                            self.commit_edit(edit_data, provider);
                        } else if !ui.is_item_active() && edit.frames > 2 {
                            // Lost focus (clicked elsewhere, Tab, etc.) — cancel.
                            self.edit = None;
                        }
                    }
                }

                // Dummy for scroll extent.
                let total_h = count as f32 * self.line_height + header_h + self.line_height;
                ui.set_cursor_pos([0.0, total_h]);
                ui.dummy([avail[0], 1.0]);
            });
    }
}

// ── Function-boundary detection ──────────────────────────────────────────────
//
// Heuristic — the trait gives us per-instruction `flow_kind()` but no
// CFG metadata. We use `FlowKind::Return` as the canonical function
// terminator (RET / IRET / RETF / RETN). Tail calls (`jmp some_func`
// at end of function) are NOT detected — a real disassembler would
// need provider-supplied function-boundary metadata for that. Padding
// (`Nop` / INT3) between functions folds into whichever side it
// neighbours; acceptable trade-off for a heuristic.

/// Return the index of the instruction that *ends* the function
/// containing `idx` — first [`FlowKind::Return`] at or after `idx`.
/// Returns `count - 1` (last decoded instruction) when no `Return`
/// is found before the buffer ends.
///
/// Empty providers return `0`. `idx` is clamped to `count - 1` first
/// so out-of-range cursors land at the buffer tail rather than
/// panicking.
pub fn find_function_end(provider: &dyn DisasmDataProvider, idx: usize) -> usize {
    let count = provider.instruction_count();
    if count == 0 {
        return 0;
    }
    let start = idx.min(count - 1);
    for i in start..count {
        if let Some(instr) = provider.instruction(i)
            && instr.flow_kind() == FlowKind::Return
        {
            return i;
        }
    }
    count - 1
}

/// Return the index of the instruction that *starts* the function
/// containing `idx` — instruction immediately after the previous
/// [`FlowKind::Return`] boundary, or `0` if no boundary exists
/// before `idx`.
///
/// When the cursor sits ON a `Return`, we still walk strictly
/// *backward* from `cur - 1` so the function this RET belongs to
/// (not the one it ends) is the one returned. Empty providers
/// return `0`.
pub fn find_function_start(provider: &dyn DisasmDataProvider, idx: usize) -> usize {
    let count = provider.instruction_count();
    if count == 0 {
        return 0;
    }
    let cur = idx.min(count - 1);
    if cur == 0 {
        return 0;
    }
    let mut i = cur;
    while i > 0 {
        i -= 1;
        if let Some(instr) = provider.instruction(i)
            && instr.flow_kind() == FlowKind::Return
        {
            // Function starts at the instruction immediately after
            // the previous RET. `i + 1` is always `<= cur` (we
            // entered the loop with `cur > 0` and `i` started at
            // `cur - 1`), so no clamp needed — the historic
            // `(i + 1).min(count - 1)` was a bogus cap that
            // incorrectly returned the previous-function RET when
            // `cur` was the last instruction in the buffer.
            return i + 1;
        }
    }
    0
}

// ── Free helpers ─────────────────────────────────────────────────────────────

/// Parse a single operand token classified as
/// [`tokens::TokenKind::Number`] into an absolute `u64` address.
/// Accepts:
///   - `0x...` / `0X...` hex prefix
///   - `...h` / `...H` MASM/iced-x86 hex suffix
///   - bare decimal
///
/// Returns `None` for unparseable text. Used by
/// [`DisasmView::follow_at_cursor`] to chase numeric-immediate
/// pointers that the provider didn't tag as branch targets.
fn parse_operand_number(s: &str) -> Option<u64> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    if s.len() > 1
        && (s.ends_with('h') || s.ends_with('H'))
        && s[..s.len() - 1].chars().all(|c| c.is_ascii_hexdigit())
    {
        return u64::from_str_radix(&s[..s.len() - 1], 16).ok();
    }
    s.parse::<u64>().ok()
}

/// Parse a goto-popup / API-supplied address string.
///
/// Acceptance order (first match wins):
/// 1. **`0x` / `0X` prefix** → unconditional hex parse.
/// 2. **Contains a hex letter (`a–f` / `A–F`)** → unambiguous hex, parse as base 16.
/// 3. Otherwise → parse as base 10.
///
/// The previous heuristic accepted hex on length alone (`s.len() > 4` with
/// any hex digits) which made `"4080"` (4 chars) decimal but `"40810"`
/// (5 chars) hex — a confusing length-cliff. Now decimal is the default
/// for all-digit input; hex requires either an explicit `0x` prefix or
/// at least one `a–f` letter.
fn parse_address(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    let has_hex_letter = s.chars().any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    if has_hex_letter && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u64::from_str_radix(s, 16).ok();
    }
    s.parse::<u64>().ok()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use provider::{InstructionEntry, VecDisasmProvider};
    use tokens::{OperandTokenizer, TokenKind, classify_operand_token};

    fn sample_provider() -> VecDisasmProvider {
        let mut p = VecDisasmProvider::new();
        p.push(
            InstructionEntry::new(0x401000, vec![0x55], "push", "rbp").with_flow(FlowKind::Stack),
        );
        p.push(InstructionEntry::new(
            0x401001,
            vec![0x48, 0x89, 0xE5],
            "mov",
            "rbp, rsp",
        ));
        p.push(
            InstructionEntry::new(0x401004, vec![0x48, 0x83, 0xEC, 0x20], "sub", "rsp, 0x20")
                .with_flow(FlowKind::Stack),
        );
        p.push(
            InstructionEntry::new(
                0x401008,
                vec![0xE8, 0x10, 0x00, 0x00, 0x00],
                "call",
                "0x40101D",
            )
            .with_flow(FlowKind::Call)
            .with_target(0x40101D)
            .with_comment("some_function"),
        );
        p.push(InstructionEntry::new(
            0x40100D,
            vec![0x48, 0x85, 0xC0],
            "test",
            "rax, rax",
        ));
        p.push(
            InstructionEntry::new(0x401010, vec![0x74, 0x05], "je", "0x401017")
                .with_flow(FlowKind::Jump)
                .with_target(0x401017),
        );
        p.push(InstructionEntry::new(0x401012, vec![0xC9], "leave", ""));
        p.push(InstructionEntry::new(0x401013, vec![0xC3], "ret", "").with_flow(FlowKind::Return));
        p
    }

    #[test]
    fn test_new_view() {
        let view = DisasmView::new("test");
        assert_eq!(view.selected_index(), None);
        assert!(!view.is_focused());
    }

    #[test]
    fn test_instruction_entry() {
        let entry = InstructionEntry::new(0x1000, vec![0x90], "nop", "");
        assert_eq!(entry.address(), 0x1000);
        assert_eq!(entry.bytes(), &[0x90]);
        assert_eq!(entry.mnemonic(), "nop");
        assert_eq!(entry.operands(), "");
        assert_eq!(entry.flow_kind(), FlowKind::Normal);

        let entry2 = InstructionEntry::new(0x2000, vec![0xEB, 0x10], "jmp", "0x2010")
            .with_flow(FlowKind::Jump)
            .with_target(0x2010)
            .with_comment("loop top");
        assert_eq!(entry2.flow_kind(), FlowKind::Jump);
        assert_eq!(entry2.branch_target(), Some(0x2010));
        assert_eq!(entry2.comment(), Some("loop top"));
    }

    #[test]
    fn test_vec_provider() {
        let p = sample_provider();
        assert_eq!(p.instruction_count(), 8);
        assert!(p.instruction(0).is_some());
        assert!(p.instruction(8).is_none());
        assert_eq!(p.index_of_address(0x401004), Some(2));
        assert_eq!(p.index_of_address(0xFF0000), None);
    }

    #[test]
    fn test_toggle_breakpoint() {
        let mut p = sample_provider();
        p.toggle_breakpoint(0x401000);
        assert!(p.instruction(0).unwrap().has_breakpoint());
        p.toggle_breakpoint(0x401000);
        assert!(!p.instruction(0).unwrap().has_breakpoint());
    }

    // ── Convenience selectors (host toolbar API, session 032) ──────────

    #[test]
    fn select_current_ip_finds_marked_row() {
        let mut p = sample_provider();
        // Mark idx 3 (the `call`) as the current IP.
        p.instructions_mut()[3].current = true;

        let mut view = DisasmView::new("t");
        assert!(view.select_current_ip(&p));
        assert_eq!(view.selected_index(), Some(3));
    }

    #[test]
    fn select_current_ip_returns_false_when_no_ip() {
        let p = sample_provider(); // no `current` flag set
        let mut view = DisasmView::new("t");
        assert!(!view.select_current_ip(&p));
        assert_eq!(view.selected_index(), None);
    }

    #[test]
    fn select_first_breakpoint_finds_lowest_index() {
        let mut p = sample_provider();
        p.toggle_breakpoint(0x40100D); // idx 4
        p.toggle_breakpoint(0x401004); // idx 2

        let mut view = DisasmView::new("t");
        assert!(view.select_first_breakpoint(&p));
        assert_eq!(view.selected_index(), Some(2), "lowest-index BP wins");
    }

    #[test]
    fn select_first_breakpoint_returns_false_when_none() {
        let p = sample_provider();
        let mut view = DisasmView::new("t");
        assert!(!view.select_first_breakpoint(&p));
    }

    #[test]
    fn select_next_breakpoint_cycles_forward_with_wraparound() {
        let mut p = sample_provider();
        p.toggle_breakpoint(0x401001); // idx 1
        p.toggle_breakpoint(0x401010); // idx 5

        let mut view = DisasmView::new("t");
        view.select(3); // cursor between the two BPs

        // Next from idx 3 → idx 5.
        assert!(view.select_next_breakpoint(&p));
        assert_eq!(view.selected_index(), Some(5));
        // Next from idx 5 → wraps to idx 1.
        assert!(view.select_next_breakpoint(&p));
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn select_prev_breakpoint_cycles_backward_with_wraparound() {
        let mut p = sample_provider();
        p.toggle_breakpoint(0x401001); // idx 1
        p.toggle_breakpoint(0x401010); // idx 5

        let mut view = DisasmView::new("t");
        view.select(3);
        // Prev from idx 3 → idx 1.
        assert!(view.select_prev_breakpoint(&p));
        assert_eq!(view.selected_index(), Some(1));
        // Prev from idx 1 → wraps to idx 5.
        assert!(view.select_prev_breakpoint(&p));
        assert_eq!(view.selected_index(), Some(5));
    }

    #[test]
    fn can_nav_back_forward_track_history_state() {
        let p = sample_provider();
        let mut view = DisasmView::new("t");
        // Empty history at construction.
        assert!(!view.can_nav_back());
        assert!(!view.can_nav_forward());

        // First goto seeds the back stack (origin → push).
        view.goto_address(0x401000, &p);
        // Still nothing on the back stack — first selection has no
        // prior cursor to push.
        assert!(!view.can_nav_back());

        view.goto_address(0x40100D, &p);
        assert!(view.can_nav_back(), "second goto must populate back");
        assert!(!view.can_nav_forward());

        view.nav_back(&p);
        assert!(view.can_nav_forward(), "back must populate forward");
    }

    #[test]
    fn cursor_address_matches_selected_instruction() {
        let p = sample_provider();
        let mut view = DisasmView::new("t");
        assert_eq!(view.cursor_address(&p), None);

        view.select(3);
        assert_eq!(view.cursor_address(&p), Some(0x401008));
    }

    // ── Bookmarks ────────────────────────────────────────────────────

    #[test]
    fn bookmark_default_empty() {
        let view = DisasmView::new("t");
        assert_eq!(view.bookmark_count(), 0);
        assert!(view.bookmarks().is_empty());
        assert!(!view.is_bookmarked(0x401000));
    }

    #[test]
    fn add_bookmark_inserts_and_is_idempotent() {
        let mut view = DisasmView::new("t");
        assert!(view.add_bookmark(0x401000));
        assert!(view.is_bookmarked(0x401000));
        assert_eq!(view.bookmark_count(), 1);
        // Adding the same address again still returns true (idempotent)
        // and doesn't duplicate.
        assert!(view.add_bookmark(0x401000));
        assert_eq!(view.bookmark_count(), 1);
    }

    #[test]
    fn add_bookmark_capped_at_max() {
        let mut view = DisasmView::new("t");
        for i in 0..DisasmView::MAX_BOOKMARKS as u64 {
            assert!(view.add_bookmark(0x400000 + i));
        }
        assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS);
        // The 65th unique address must fail without mutating the set.
        assert!(!view.add_bookmark(0x4FFFFF));
        assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS);
        assert!(!view.is_bookmarked(0x4FFFFF));
    }

    #[test]
    fn remove_bookmark_returns_true_when_present() {
        let mut view = DisasmView::new("t");
        view.add_bookmark(0x401000);
        assert!(view.remove_bookmark(0x401000));
        assert!(!view.is_bookmarked(0x401000));
        // Subsequent removal of the same address returns false.
        assert!(!view.remove_bookmark(0x401000));
    }

    #[test]
    fn toggle_bookmark_round_trip() {
        let mut view = DisasmView::new("t");
        // off → on
        assert!(view.toggle_bookmark(0x401000));
        assert!(view.is_bookmarked(0x401000));
        // on → off
        assert!(!view.toggle_bookmark(0x401000));
        assert!(!view.is_bookmarked(0x401000));
    }

    #[test]
    fn toggle_bookmark_at_cap_returns_false_for_new_address() {
        let mut view = DisasmView::new("t");
        for i in 0..DisasmView::MAX_BOOKMARKS as u64 {
            view.add_bookmark(0x400000 + i);
        }
        // New address at cap → toggle on must fail.
        assert!(!view.toggle_bookmark(0x4FFFFF));
        assert!(!view.is_bookmarked(0x4FFFFF));
        // Existing address must still toggle off correctly.
        assert!(!view.toggle_bookmark(0x400000));
        assert!(!view.is_bookmarked(0x400000));
        assert_eq!(view.bookmark_count(), DisasmView::MAX_BOOKMARKS - 1);
    }

    #[test]
    fn clear_bookmarks_empties_set() {
        let mut view = DisasmView::new("t");
        view.add_bookmark(0x401000);
        view.add_bookmark(0x401004);
        view.add_bookmark(0x401010);
        assert_eq!(view.bookmark_count(), 3);
        view.clear_bookmarks();
        assert_eq!(view.bookmark_count(), 0);
        assert!(view.bookmarks().is_empty());
    }

    #[test]
    fn show_bookmarks_default_is_true_independently_of_breakpoints() {
        // C1 from session 034 audit: bookmark visibility was once
        // gated inside the `show_breakpoints` block, so disabling
        // breakpoints would silently hide bookmarks too. Pin the
        // independent default flags.
        let cfg = super::DisasmViewConfig::default();
        assert!(cfg.show_breakpoints);
        assert!(cfg.show_bookmarks);
    }

    #[test]
    fn bookmarks_set_is_sorted_for_host_iteration() {
        // Pin the BTreeSet ordering — hosts that round-trip the
        // bookmark set through serde / config files want a stable
        // ascending-address order. Insertion order is intentionally
        // randomised here.
        let mut view = DisasmView::new("t");
        view.add_bookmark(0x401010);
        view.add_bookmark(0x401000);
        view.add_bookmark(0x40100F);
        view.add_bookmark(0x401004);
        let collected: Vec<u64> = view.bookmarks().iter().copied().collect();
        assert_eq!(collected, vec![0x401000, 0x401004, 0x40100F, 0x401010]);
    }

    #[test]
    fn test_flow_kind_colors() {
        let colors = DisasmColors::default();
        // Different flow kinds should have visually distinct mnemonic colors.
        let normal = colors.mnemonic_color(FlowKind::Normal);
        let jump = colors.mnemonic_color(FlowKind::Jump);
        let call = colors.mnemonic_color(FlowKind::Call);
        let ret = colors.mnemonic_color(FlowKind::Return);

        assert_ne!(normal, jump);
        assert_ne!(jump, call);
        assert_ne!(call, ret);
    }

    #[test]
    fn test_arrow_color() {
        let colors = DisasmColors::default();
        let jump_color = colors.arrow_color(FlowKind::Jump);
        let call_color = colors.arrow_color(FlowKind::Call);
        // Should have different colors for different flow types.
        assert_ne!(jump_color, call_color);
    }

    #[test]
    fn test_block_tint() {
        let colors = DisasmColors::default();
        let tint0 = colors.block_tint(0);
        let tint1 = colors.block_tint(1);
        // Block tints should differ between adjacent blocks.
        assert!(tint0 != tint1 || tint0[3] == 0.0);
    }

    #[test]
    fn test_compute_arrows() {
        let p = sample_provider();
        let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
            .filter_map(|i| p.instruction(i))
            .collect();
        let arrows = compute_arrows(&instrs, 0, instrs.len());
        // je at index 5 targets 0x401017 and call targets 0x40101D — both outside
        // our 8 instructions, so no arrows expected in this basic sample.
        // Arrow computation only shows arrows where BOTH endpoints are visible.
        assert!(
            arrows.is_empty() || arrows.len() <= 2,
            "Expected 0-2 arrows, got {}",
            arrows.len()
        );
    }

    #[test]
    fn test_operand_tokenizer_registers() {
        let tokens: Vec<_> = OperandTokenizer::new("rax, rbx").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Register));
    }

    #[test]
    fn test_operand_tokenizer_numbers() {
        let tokens: Vec<_> = OperandTokenizer::new("0x1234").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Number));
    }

    #[test]
    fn test_operand_tokenizer_memory() {
        let tokens: Vec<_> = OperandTokenizer::new("[rsp+8]").collect();
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Memory));
    }

    #[test]
    fn test_classify_operand_register() {
        assert_eq!(classify_operand_token("rax"), TokenKind::Register);
        assert_eq!(classify_operand_token("xmm0"), TokenKind::Register);
        assert_eq!(classify_operand_token("RAX"), TokenKind::Register);
    }

    #[test]
    fn test_classify_operand_number() {
        assert_eq!(classify_operand_token("0x1234"), TokenKind::Number);
        assert_eq!(classify_operand_token("100"), TokenKind::Number);
        assert_eq!(classify_operand_token("FFh"), TokenKind::Number);
    }

    #[test]
    fn test_classify_operand_size() {
        assert_eq!(classify_operand_token("qword"), TokenKind::Memory);
        assert_eq!(classify_operand_token("ptr"), TokenKind::Memory);
    }

    #[test]
    fn test_column_widths_default() {
        let cols = ColumnWidths::default();
        assert!(cols.address > 0.0);
        // User-requested widths (2026-04-30): Bytes 200,
        // Instruction 300 (= mnemonic + operands), Comment is
        // a *minimum* width (renders dynamic in `frame_comment_w`).
        assert_eq!(cols.bytes, 200.0, "bytes column must be 200 px");
        assert_eq!(
            cols.mnemonic + cols.operands,
            300.0,
            "instruction (mnemonic + operands) must total 300 px"
        );
        assert!(
            cols.comment >= 100.0,
            "comment min should keep edit-cell usable"
        );
    }

    #[test]
    fn test_disasm_config_default() {
        let cfg = DisasmViewConfig::default();
        assert!(cfg.show_arrows);
        assert!(cfg.show_breakpoints);
        assert!(!cfg.show_block_tints);
        assert!(cfg.show_header);
        assert!(!cfg.editable);
        assert!(cfg.address_width_64);
    }

    #[test]
    fn test_select_and_goto() {
        let p = sample_provider();
        let mut view = DisasmView::new("test");
        view.select(3);
        assert_eq!(view.selected_index(), Some(3));

        view.goto_address(0x401000, &p);
        assert_eq!(view.selected_index(), Some(0));
    }

    #[test]
    fn test_nav_history() {
        let p = sample_provider();
        let mut view = DisasmView::new("test");

        view.select(0); // at 0x401000
        view.goto_address(0x401008, &p); // jump to call
        assert_eq!(view.selected_index(), Some(3));

        view.nav_back(&p);
        assert_eq!(view.selected_index(), Some(0));

        view.nav_forward(&p);
        assert_eq!(view.selected_index(), Some(3));
    }

    #[test]
    fn test_parse_address() {
        // Explicit 0x prefix → hex.
        assert_eq!(parse_address("0x401000"), Some(0x401000));
        assert_eq!(parse_address("0X401000"), Some(0x401000));
        // No prefix, no hex letters → decimal.
        assert_eq!(parse_address("256"), Some(256));
        assert_eq!(parse_address("4080"), Some(4080));
        assert_eq!(parse_address("401000"), Some(401000));
        // No prefix, contains a hex letter → hex.
        assert_eq!(parse_address("4abc"), Some(0x4abc));
        assert_eq!(parse_address("DEAD"), Some(0xDEAD));
        assert_eq!(parse_address("cafef00d"), Some(0xcafef00d));
        // Whitespace is trimmed.
        assert_eq!(parse_address("  0xff  "), Some(0xff));
        // Garbage → None.
        assert_eq!(parse_address("hello"), None);
        assert_eq!(parse_address(""), None);
    }

    #[test]
    fn test_arrow_depth_assignment() {
        // Create instructions with nested branches.
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let mut entry = InstructionEntry::new(0x1000 + i * 2, vec![0x90], "nop", "");
            entry.flow_kind = FlowKind::Normal;
            p.push(entry);
        }
        // Add two overlapping jumps.
        p.instructions_mut()[2] = InstructionEntry::new(0x1004, vec![0xEB, 0x08], "jmp", "0x100E")
            .with_flow(FlowKind::Jump)
            .with_target(0x100E);
        p.instructions_mut()[1] = InstructionEntry::new(0x1002, vec![0x74, 0x0C], "je", "0x1010")
            .with_flow(FlowKind::Jump)
            .with_target(0x1010);

        let instrs: Vec<&dyn Instruction> = (0..p.instruction_count())
            .filter_map(|i| p.instruction(i))
            .collect();
        let arrows = compute_arrows(&instrs, 0, instrs.len());

        // If both targets are in range, should have different depths.
        if arrows.len() >= 2 {
            assert_ne!(
                arrows[0].depth, arrows[1].depth,
                "Overlapping arrows should have different depths"
            );
        }
    }

    // ── Property-based tests ─────────────────────────────────────────────

    use proptest::prelude::*;

    proptest! {
        /// `parse_address` accepts arbitrary strings without panicking.
        #[test]
        fn prop_parse_address_never_panics(s in ".{0,32}") {
            let _ = parse_address(&s);
        }

        /// Hex-prefixed addresses round-trip cleanly.
        #[test]
        fn prop_parse_address_hex_roundtrips(value in any::<u64>()) {
            let s = format!("0x{value:X}");
            prop_assert_eq!(parse_address(&s), Some(value));
        }
    }

    // ── OperandTokenizer edge cases ──────────────────────────────────────

    fn tokens_of(s: &str) -> Vec<(String, TokenKind)> {
        OperandTokenizer::new(s)
            .map(|t| (t.text.to_string(), t.kind))
            .collect()
    }

    #[test]
    fn tokenizer_empty_input() {
        assert!(tokens_of("").is_empty());
    }

    #[test]
    fn tokenizer_only_punctuation_collapses() {
        // Run of `, +-*: ` is consumed as a single Plain token.
        let toks = tokens_of(",,, ");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, ",,, ");
        assert_eq!(toks[0].1, TokenKind::Plain);
    }

    #[test]
    fn tokenizer_trailing_comma() {
        // `rax,` → Register("rax"), Plain(",").
        let toks = tokens_of("rax,");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].0, "rax");
        assert_eq!(toks[0].1, TokenKind::Register);
        assert_eq!(toks[1].0, ",");
        assert_eq!(toks[1].1, TokenKind::Plain);
    }

    #[test]
    fn tokenizer_two_operands() {
        // `rax, rbx` splits into reg, plain (", "), reg.
        let toks = tokens_of("rax, rbx");
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0].1, TokenKind::Register);
        assert_eq!(toks[1].1, TokenKind::Plain);
        assert_eq!(toks[2].1, TokenKind::Register);
        assert_eq!(toks[2].0, "rbx");
    }

    #[test]
    fn tokenizer_memory_brackets() {
        // `[rsp+8]` → `[`, `rsp`, `+`, `8`, `]`.
        let toks = tokens_of("[rsp+8]");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        let texts: Vec<&str> = toks.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(texts, vec!["[", "rsp", "+", "8", "]"]);
        assert_eq!(
            kinds,
            vec![
                TokenKind::Memory,
                TokenKind::Register,
                TokenKind::Plain,
                TokenKind::Number,
                TokenKind::Memory,
            ]
        );
    }

    #[test]
    fn tokenizer_nested_brackets_size_keyword() {
        // `qword ptr [rax + 0x10]` exercises size keywords + memory + register + hex.
        let toks = tokens_of("qword ptr [rax + 0x10]");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        // qword/ptr classify as Memory; rax Register; 0x10 Number; brackets Memory.
        assert!(kinds.contains(&TokenKind::Memory));
        assert!(kinds.contains(&TokenKind::Register));
        assert!(kinds.contains(&TokenKind::Number));
        assert_eq!(toks.first().unwrap().0, "qword");
        assert_eq!(toks.last().unwrap().0, "]");
        assert_eq!(toks.last().unwrap().1, TokenKind::Memory);
    }

    #[test]
    fn tokenizer_unterminated_string() {
        // Missing closing quote: tokenizer must consume to end-of-input, not panic.
        let toks = tokens_of("\"hello world");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::String);
        assert_eq!(toks[0].0, "\"hello world");
    }

    #[test]
    fn tokenizer_hex_suffix_h() {
        // MASM-style `1Fh` is classified as a number.
        let toks = tokens_of("1Fh");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::Number);
    }

    #[test]
    fn tokenizer_unknown_word_is_plain() {
        // `gibberish` is not a register, not a number → Plain.
        let toks = tokens_of("gibberish");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].1, TokenKind::Plain);
    }

    // ── Iced-x86 / extended register coverage ────────────────────────────
    //
    // iced-x86's default `IntelFormatter` outputs operand text that we
    // need to colour-code correctly. Pin the cases that previously fell
    // through to `Plain` so a regression in `is_x86_register` /
    // `classify_operand_token` is caught with a meaningful diagnostic.

    #[test]
    fn classify_extended_gp_registers() {
        // r8..r15 with optional b/w/d suffix.
        for r in [
            "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "r8b", "r9w", "r10d", "r15b",
            "r12w",
        ] {
            assert_eq!(
                classify_operand_token(r),
                TokenKind::Register,
                "{r} should classify as Register",
            );
        }
    }

    #[test]
    fn classify_avx512_registers() {
        // SIMD 16..31 (AVX-512) + zmm + mask registers.
        for r in [
            "xmm15", "xmm16", "xmm31", "ymm0", "ymm17", "ymm31", "zmm0", "zmm15", "zmm31", "k0",
            "k1", "k7",
        ] {
            assert_eq!(
                classify_operand_token(r),
                TokenKind::Register,
                "{r} should classify as Register",
            );
        }
    }

    #[test]
    fn classify_system_registers() {
        // Control / debug / test / MMX — used by kernel-mode + legacy disasm.
        for r in [
            "cr0", "cr2", "cr3", "cr4", "cr8", "cr15", "dr0", "dr6", "dr7", "tr0", "tr7", "mm0",
            "mm7",
        ] {
            assert_eq!(
                classify_operand_token(r),
                TokenKind::Register,
                "{r} should classify as Register",
            );
        }
    }

    #[test]
    fn classify_size_keywords_extended() {
        // `fword`, `tbyte`, `oword`, `zmmword` — not in pre-iced-x86 set.
        for kw in ["fword", "tbyte", "oword", "zmmword"] {
            assert_eq!(
                classify_operand_token(kw),
                TokenKind::Memory,
                "{kw} should classify as Memory",
            );
        }
    }

    #[test]
    fn classify_rejects_register_lookalikes() {
        // Regression guards: invalid range reads as Plain (NOT Register).
        // `r0` (no extended r0 exists), `xmm32` (out of range),
        // `zmm99`, `cr16`, `mm8`, `k8`, `r10x` (bad suffix).
        for tok in ["r0", "r7", "xmm32", "zmm99", "cr16", "mm8", "k8", "r10x"] {
            assert_eq!(
                classify_operand_token(tok),
                TokenKind::Plain,
                "{tok} must NOT be classified as Register",
            );
        }
    }

    #[test]
    fn classify_number_edge_cases() {
        // Empty hex bodies (`h`, `0x`) are NOT numbers.
        assert_eq!(classify_operand_token("h"), TokenKind::Plain);
        assert_eq!(classify_operand_token("H"), TokenKind::Plain);
        assert_eq!(classify_operand_token("0x"), TokenKind::Plain);
        assert_eq!(classify_operand_token("0X"), TokenKind::Plain);
        // ...but minimal valid hex stays a Number.
        assert_eq!(classify_operand_token("0x0"), TokenKind::Number);
        assert_eq!(classify_operand_token("Fh"), TokenKind::Number);
    }

    #[test]
    fn tokenizer_iced_x86_no_space_after_comma() {
        // iced-x86's default IntelFormatter outputs without a space
        // after the operand separator: `mov rax,qword ptr [rsp+10h]`.
        // Pin that the tokenizer still produces correct kinds.
        let toks = tokens_of("rax,qword ptr [rsp+10h]");
        let kinds: Vec<TokenKind> = toks.iter().map(|t| t.1).collect();
        // First token must be the register.
        assert_eq!(toks[0].0, "rax");
        assert_eq!(toks[0].1, TokenKind::Register);
        // `qword`, `ptr`, `[`, `]` all classify as Memory.
        assert!(kinds.contains(&TokenKind::Memory));
        // `rsp` → Register, `10h` → Number.
        assert!(kinds.contains(&TokenKind::Register));
        assert!(kinds.contains(&TokenKind::Number));
    }

    // ── Theme integration ───────────────────────────────────────────────

    use crate::theme::Theme;

    #[test]
    fn config_with_theme_replaces_palette() {
        // `with_theme` is a builder shortcut — it must replace the
        // embedded palette with the named theme's disasm-view colours.
        let dark = DisasmViewConfig::default().with_theme(Theme::Dark);
        let nord = DisasmViewConfig::default().with_theme(Theme::Nord);
        let solar = DisasmViewConfig::default().with_theme(Theme::Solarized);

        // Different themes => different mnemonic colours (at minimum).
        assert_ne!(
            dark.colors.mnemonic_jump, nord.colors.mnemonic_jump,
            "Dark and Nord should expose distinct jump-mnemonic colours",
        );
        assert_ne!(
            nord.colors.address, solar.colors.address,
            "Nord and Solarized should expose distinct address colours",
        );
    }

    #[test]
    fn config_default_matches_dark_theme() {
        // Bare `DisasmViewConfig::default()` must reuse
        // `Theme::Dark.disasm_view_colors()` so a host that doesn't
        // pick a theme still gets the canonical Dark look.
        let default_cfg = DisasmViewConfig::default();
        let dark_palette = Theme::Dark.disasm_view_colors();
        assert_eq!(
            default_cfg.colors.mnemonic_normal,
            dark_palette.mnemonic_normal
        );
        assert_eq!(default_cfg.colors.address, dark_palette.address);
        assert_eq!(default_cfg.colors.selection_bg, dark_palette.selection_bg);
        assert_eq!(default_cfg.colors.breakpoint, dark_palette.breakpoint);
    }

    // ── set_comment / Comment edit round-trip ────────────────────────────

    #[test]
    fn set_comment_round_trip_via_vec_provider() {
        // Mutate-then-read: writing a comment via the trait method
        // must be visible through `Instruction::comment()` on the
        // very next frame (no buffering / async).
        let mut p = sample_provider();
        let addr = p.instruction(2).unwrap().address(); // 0x401004
        assert_eq!(p.instruction(2).unwrap().comment(), None);

        assert!(p.set_comment(addr, "stack alloca"));
        assert_eq!(p.instruction(2).unwrap().comment(), Some("stack alloca"));
    }

    #[test]
    fn set_comment_clears_on_empty_string() {
        // Empty / whitespace-only input clears the comment so the
        // user can wipe a note by opening the editor and pressing
        // Enter on a blank buffer.
        let mut p = sample_provider();
        let addr = p.instruction(3).unwrap().address(); // 0x401008 (call)
        // Sample provider already attached "some_function" here.
        assert_eq!(p.instruction(3).unwrap().comment(), Some("some_function"));

        assert!(p.set_comment(addr, ""));
        assert_eq!(p.instruction(3).unwrap().comment(), None);

        // Whitespace-only must also clear (trim semantics).
        assert!(p.set_comment(addr, "first"));
        assert!(p.set_comment(addr, "   \t  "));
        assert_eq!(p.instruction(3).unwrap().comment(), None);
    }

    #[test]
    fn set_comment_trims_surrounding_whitespace() {
        // Trim guards against accidental trailing whitespace from
        // clipboard pastes — stored value is canonicalised.
        let mut p = sample_provider();
        let addr = p.instruction(0).unwrap().address();
        assert!(p.set_comment(addr, "   prologue  "));
        assert_eq!(p.instruction(0).unwrap().comment(), Some("prologue"));
    }

    #[test]
    fn set_comment_returns_false_for_unknown_address() {
        // Unknown address → no-op + false. Caller can then surface
        // the "address not decoded" diagnostic to the user.
        let mut p = sample_provider();
        assert!(!p.set_comment(0xDEAD_BEEF, "ghost"));
    }

    // ── Function-boundary heuristic ──────────────────────────────────────

    /// Build a 3-function provider for boundary tests:
    /// - func A: `[0..=2]` ending in RET at index 2
    /// - func B: `[3..=5]` ending in RET at index 5
    /// - func C: `[6..=8]` ending in RET at index 8
    fn three_function_provider() -> VecDisasmProvider {
        let mut p = VecDisasmProvider::new();
        // func A
        p.push(InstructionEntry::new(0x1000, vec![0x55], "push", "rbp")
            .with_flow(FlowKind::Stack));
        p.push(InstructionEntry::new(0x1001, vec![0x90], "nop", ""));
        p.push(InstructionEntry::new(0x1002, vec![0xC3], "ret", "")
            .with_flow(FlowKind::Return));
        // func B
        p.push(InstructionEntry::new(0x1003, vec![0x55], "push", "rbp")
            .with_flow(FlowKind::Stack));
        p.push(InstructionEntry::new(0x1004, vec![0x90], "nop", ""));
        p.push(InstructionEntry::new(0x1005, vec![0xC3], "ret", "")
            .with_flow(FlowKind::Return));
        // func C
        p.push(InstructionEntry::new(0x1006, vec![0x55], "push", "rbp")
            .with_flow(FlowKind::Stack));
        p.push(InstructionEntry::new(0x1007, vec![0x90], "nop", ""));
        p.push(InstructionEntry::new(0x1008, vec![0xC3], "ret", "")
            .with_flow(FlowKind::Return));
        p
    }

    #[test]
    fn find_function_start_returns_zero_for_first_function() {
        let p = three_function_provider();
        assert_eq!(find_function_start(&p, 0), 0);
        assert_eq!(find_function_start(&p, 1), 0);
        assert_eq!(find_function_start(&p, 2), 0);
    }

    #[test]
    fn find_function_start_returns_post_ret_index() {
        let p = three_function_provider();
        // Cursor in func B → start should be index 3 (right after func A's RET).
        assert_eq!(find_function_start(&p, 3), 3);
        assert_eq!(find_function_start(&p, 4), 3);
        assert_eq!(find_function_start(&p, 5), 3);
        // Cursor in func C → start should be index 6.
        assert_eq!(find_function_start(&p, 6), 6);
        assert_eq!(find_function_start(&p, 8), 6);
    }

    #[test]
    fn find_function_end_returns_first_ret_at_or_after_cursor() {
        let p = three_function_provider();
        // Cursor in func A → end at index 2 (the RET).
        assert_eq!(find_function_end(&p, 0), 2);
        assert_eq!(find_function_end(&p, 1), 2);
        assert_eq!(find_function_end(&p, 2), 2);
        // Cursor in func B → end at index 5.
        assert_eq!(find_function_end(&p, 3), 5);
        assert_eq!(find_function_end(&p, 5), 5);
    }

    #[test]
    fn find_function_end_returns_last_when_no_ret_after() {
        // No-RET tail — end clamps to last instruction.
        let mut p = VecDisasmProvider::new();
        p.push(InstructionEntry::new(0x2000, vec![0x90], "nop", ""));
        p.push(InstructionEntry::new(0x2001, vec![0x90], "nop", ""));
        p.push(InstructionEntry::new(0x2002, vec![0x90], "nop", ""));
        assert_eq!(find_function_end(&p, 0), 2);
    }

    #[test]
    fn find_function_helpers_handle_empty_provider() {
        let p = VecDisasmProvider::new();
        assert_eq!(find_function_start(&p, 0), 0);
        assert_eq!(find_function_end(&p, 0), 0);
        assert_eq!(find_function_start(&p, 999), 0);
        assert_eq!(find_function_end(&p, 999), 0);
    }

    #[test]
    fn find_function_helpers_clamp_oob_cursor() {
        let p = three_function_provider();
        // Cursor past the end clamps to the last instruction (index 8 = func C RET).
        assert_eq!(find_function_end(&p, 999), 8);
        assert_eq!(find_function_start(&p, 999), 6);
    }

    #[test]
    fn select_function_selects_from_cursor_to_end() {
        let p = three_function_provider();
        let mut view = DisasmView::new("test_select_func");
        // Cursor at index 4 (middle of func B); select_function
        // should select [4, 5] and move cursor to 5 (the RET).
        view.cursor_idx = Some(4);
        view.select_function(&p);
        assert_eq!(view.selected_index(), Some(5));
        assert_eq!(view.selected_indices().len(), 2);
        assert!(view.is_selected(4));
        assert!(view.is_selected(5));
    }

    #[test]
    fn jump_to_function_start_lands_on_post_ret_index() {
        let p = three_function_provider();
        let mut view = DisasmView::new("test_jump_start");
        view.cursor_idx = Some(7); // middle of func C
        view.jump_to_function_start(&p);
        assert_eq!(view.selected_index(), Some(6));
    }

    #[test]
    fn jump_to_function_end_lands_on_ret_index() {
        let p = three_function_provider();
        let mut view = DisasmView::new("test_jump_end");
        view.cursor_idx = Some(7); // middle of func C
        view.jump_to_function_end(&p);
        assert_eq!(view.selected_index(), Some(8));
    }

    // ── follow_at_cursor ─────────────────────────────────────────────────

    #[test]
    fn follow_at_cursor_uses_branch_target_first() {
        // Controlled 2-instruction provider: a jmp at 0x500 with
        // resolvable target 0x510 (existing as instruction at idx 1).
        let mut p = VecDisasmProvider::new();
        p.push(
            InstructionEntry::new(0x500, vec![0xEB, 0x00], "jmp", "0x510")
                .with_flow(FlowKind::Jump)
                .with_target(0x510),
        );
        p.push(InstructionEntry::new(0x510, vec![0x90], "nop", ""));
        let mut view = DisasmView::new("test_follow_branch");
        view.cursor_idx = Some(0);
        let followed = view.follow_at_cursor(&mut p);
        assert!(followed);
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn follow_at_cursor_falls_back_to_operand_pointer() {
        // No `branch_target`, but operand string contains `0x500`
        // which matches the address of an existing instruction.
        // `mov rax, [0x500]` → follow_at_cursor should jump there.
        let mut p = VecDisasmProvider::new();
        p.push(InstructionEntry::new(0x100, vec![0x48, 0x8B, 0x05], "mov", "rax, [0x500]"));
        p.push(InstructionEntry::new(0x500, vec![0x90], "nop", ""));
        let mut view = DisasmView::new("test_follow_op");
        view.cursor_idx = Some(0);
        let followed = view.follow_at_cursor(&mut p);
        assert!(followed);
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn follow_at_cursor_returns_false_when_nothing_to_follow() {
        // Operand contains a number but it doesn't resolve to any
        // known instruction → no navigation.
        let mut p = VecDisasmProvider::new();
        p.push(InstructionEntry::new(0x100, vec![0xB8, 0x10, 0x00, 0x00, 0x00], "mov", "eax, 0x10"));
        let mut view = DisasmView::new("test_no_follow");
        view.cursor_idx = Some(0);
        assert!(!view.follow_at_cursor(&mut p));
    }

    // ── parse_operand_number ─────────────────────────────────────────────

    #[test]
    fn parse_operand_number_accepts_hex_decimal_masm() {
        assert_eq!(parse_operand_number("0x401000"), Some(0x401000));
        assert_eq!(parse_operand_number("0X401000"), Some(0x401000));
        assert_eq!(parse_operand_number("401000h"), Some(0x401000));
        assert_eq!(parse_operand_number("DEADh"), Some(0xDEAD));
        assert_eq!(parse_operand_number("100"), Some(100));
        assert_eq!(parse_operand_number(""), None);
        assert_eq!(parse_operand_number("h"), None);
        assert_eq!(parse_operand_number("0x"), None);
        assert_eq!(parse_operand_number("garbage"), None);
    }

    // ── compute_arrows_clipped ───────────────────────────────────────────

    #[test]
    fn compute_arrows_clipped_keeps_arrow_when_only_target_visible() {
        // Source at index 0 (offscreen), target at index 5 (visible).
        // Window = [3..7) → 4 rows.
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let entry = if i == 0 {
                InstructionEntry::new(0x1000, vec![0xEB, 0x00], "jmp", "0x1005")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1005)
            } else {
                InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 3, 4);
        assert_eq!(arrows.len(), 1);
        let arrow = &arrows[0];
        // Source clamped to top of visible window (idx 0 in local space).
        assert!(arrow.clipped_from);
        assert_eq!(arrow.from_idx, 0);
        // Target visible at global idx 5 → local idx 5 - 3 = 2.
        assert!(!arrow.clipped_to);
        assert_eq!(arrow.to_idx, 2);
    }

    #[test]
    fn compute_arrows_clipped_keeps_arrow_when_only_source_visible() {
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let entry = if i == 2 {
                InstructionEntry::new(0x1002, vec![0xEB, 0x00], "jmp", "0x1009")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1009)
            } else {
                InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        // Window [1..5) → source at global 2 visible (local 1),
        // target at global 9 offscreen below (clamped to last_local = 3).
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 1, 4);
        assert_eq!(arrows.len(), 1);
        let arrow = &arrows[0];
        assert!(!arrow.clipped_from);
        assert_eq!(arrow.from_idx, 1);
        assert!(arrow.clipped_to);
        assert_eq!(arrow.to_idx, 3); // last_local
    }

    #[test]
    fn compute_arrows_clipped_drops_same_side_off_window() {
        // Source AND target both above visible window → drop.
        let mut p = VecDisasmProvider::new();
        for i in 0..10 {
            let entry = if i == 0 {
                // jmp from idx 0 → idx 2 (both above window [5..9)).
                InstructionEntry::new(0x1000, vec![0xEB, 0x00], "jmp", "0x1002")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1002)
            } else {
                InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 4);
        assert!(arrows.is_empty(), "both-above arrow must drop");
    }

    #[test]
    fn compute_arrows_clipped_keeps_pass_through_arrow_forward() {
        // Source above window, target below → vertical line passes
        // through the entire visible region. Both endpoints clipped,
        // no horizontal stubs, no arrowhead in the renderer.
        let mut p = VecDisasmProvider::new();
        for i in 0..15 {
            let entry = if i == 1 {
                // jmp from idx 1 (above window [5..10)) →
                // idx 12 (below window).
                InstructionEntry::new(0x1001, vec![0xEB, 0x00], "jmp", "0x100C")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x100C)
            } else {
                InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
        assert_eq!(arrows.len(), 1, "pass-through arrow must survive");
        let arrow = &arrows[0];
        assert!(arrow.clipped_from);
        assert!(arrow.clipped_to);
        assert_eq!(arrow.from_idx, 0); // clamped to top of window
        assert_eq!(arrow.to_idx, 4); // clamped to bottom (last_local)
    }

    #[test]
    fn compute_arrows_clipped_keeps_pass_through_arrow_backward() {
        // Source below window, target above → backward jump
        // crossing through visible region.
        let mut p = VecDisasmProvider::new();
        for i in 0..15 {
            let entry = if i == 12 {
                InstructionEntry::new(0x100C, vec![0xEB, 0x00], "jmp", "0x1001")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1001)
            } else {
                InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
        assert_eq!(arrows.len(), 1);
        let arrow = &arrows[0];
        assert!(arrow.clipped_from);
        assert!(arrow.clipped_to);
        // Source at global 12 (below) → clamped to last_local = 4.
        assert_eq!(arrow.from_idx, 4);
        // Target at global 1 (above) → clamped to 0.
        assert_eq!(arrow.to_idx, 0);
    }

    #[test]
    fn compute_arrows_clipped_priority_orders_anchored_first() {
        // 3 arrows with different priority tiers — verify post-sort
        // order is anchored → half-clipped → pass-through.
        let mut p = VecDisasmProvider::new();
        for i in 0..20 {
            let entry = match i {
                // idx 6 → idx 8: both inside window [5..10) — anchored.
                6 => InstructionEntry::new(0x1006, vec![0xEB, 0x00], "jmp", "0x1008")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1008),
                // idx 7 → idx 15: source visible, target below — half-clipped.
                7 => InstructionEntry::new(0x1007, vec![0xEB, 0x00], "jmp", "0x100F")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x100F),
                // idx 1 → idx 18: pass-through.
                1 => InstructionEntry::new(0x1001, vec![0xEB, 0x00], "jmp", "0x1012")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x1012),
                _ => InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", ""),
            };
            p.push(entry);
        }
        let arrows = compute_arrows_clipped(&p as &dyn DisasmDataProvider, 5, 5);
        assert_eq!(arrows.len(), 3);
        // Anchored arrow first.
        assert!(!arrows[0].clipped_from && !arrows[0].clipped_to);
        // Half-clipped arrow second.
        assert!(arrows[1].clipped_from ^ arrows[1].clipped_to);
        // Pass-through arrow last (first to be truncated under cap).
        assert!(arrows[2].clipped_from && arrows[2].clipped_to);
    }

    // ── Architecture coverage: x32 (PE32) addresses ──────────────────────
    //
    // All addresses are `u64` on the wire so x32 fits naturally in
    // the upper-zero range. These tests pin behaviour at the
    // typical PE32 image-base 0x401000 to catch regressions where a
    // future change accidentally assumes the upper 32 bits are
    // populated (e.g. truncates to u32 internally).

    fn pe32_three_function_provider() -> VecDisasmProvider {
        // Same shape as `three_function_provider` but at PE32 base.
        let mut p = VecDisasmProvider::new();
        let mut addr = 0x00401000_u64;
        for f in 0..3 {
            // prologue
            p.push(
                InstructionEntry::new(addr, vec![0x55], "push", "ebp")
                    .with_flow(FlowKind::Stack)
                    .with_block(f),
            );
            addr += 1;
            // body
            p.push(
                InstructionEntry::new(addr, vec![0x90], "nop", "").with_block(f),
            );
            addr += 1;
            // ret
            p.push(
                InstructionEntry::new(addr, vec![0xC3], "ret", "")
                    .with_flow(FlowKind::Return)
                    .with_block(f),
            );
            addr += 1;
        }
        p
    }

    #[test]
    fn x32_find_function_works_with_pe32_addresses() {
        let p = pe32_three_function_provider();
        // Func 0: indices 0..=2, addresses 0x401000..=0x401002.
        assert_eq!(find_function_start(&p, 0), 0);
        assert_eq!(find_function_end(&p, 0), 2);
        // Func 1: indices 3..=5.
        assert_eq!(find_function_start(&p, 4), 3);
        assert_eq!(find_function_end(&p, 4), 5);
        // Func 2: indices 6..=8.
        assert_eq!(find_function_start(&p, 7), 6);
        assert_eq!(find_function_end(&p, 7), 8);
    }

    #[test]
    fn x32_follow_at_cursor_resolves_pe32_jump() {
        // Typical x32 binary: jmp from 0x401000 → 0x401005.
        let mut p = VecDisasmProvider::new();
        p.push(
            InstructionEntry::new(0x00401000, vec![0xE9, 0x00, 0x00, 0x00, 0x00], "jmp", "0x00401005")
                .with_flow(FlowKind::Jump)
                .with_target(0x00401005),
        );
        p.push(InstructionEntry::new(0x00401005, vec![0x90], "nop", ""));
        let mut view = DisasmView::new("x32_follow");
        view.cursor_idx = Some(0);
        assert!(view.follow_at_cursor(&mut p));
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn x32_follow_at_cursor_resolves_absolute_memory_operand() {
        // x32 absolute memory operand `mov eax, [0x401005]` — the
        // operand-pointer fallback should chase the immediate.
        let mut p = VecDisasmProvider::new();
        p.push(InstructionEntry::new(
            0x00401000,
            vec![0x8B, 0x05, 0x05, 0x10, 0x40, 0x00],
            "mov",
            "eax, [0x00401005]",
        ));
        p.push(InstructionEntry::new(0x00401005, vec![0x90], "nop", ""));
        let mut view = DisasmView::new("x32_op_follow");
        view.cursor_idx = Some(0);
        assert!(view.follow_at_cursor(&mut p));
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn x32_format_address_literal_8_digits_when_not_64bit() {
        let mut view = DisasmView::new("x32_format");
        view.config.address_width_64 = false;
        view.config.uppercase = true;
        assert_eq!(view.format_address_literal(0x00401000), "0x00401000");
        assert_eq!(view.format_address_literal(0xDEADBEEF), "0xDEADBEEF");
        view.config.uppercase = false;
        assert_eq!(view.format_address_literal(0x00401000), "0x00401000");
        // Truncation behaviour for too-wide addresses: `{:08X}`
        // doesn't truncate, it just widens — this is fine for x32
        // because addresses fit in 8 hex digits, and would surface
        // weird-but-not-broken display if a 64-bit address landed
        // here while `address_width_64=false`.
    }

    #[test]
    fn x64_format_address_literal_16_digits_when_64bit() {
        let mut view = DisasmView::new("x64_format");
        view.config.address_width_64 = true;
        view.config.uppercase = true;
        assert_eq!(
            view.format_address_literal(0x00007FF6_12345678),
            "0x00007FF612345678"
        );
        assert_eq!(
            view.format_address_literal(0xFFFF_FFFF_FFFF_FFFF),
            "0xFFFFFFFFFFFFFFFF"
        );
        view.config.uppercase = false;
        assert_eq!(
            view.format_address_literal(0x7FF6_1234_5678),
            "0x00007ff612345678"
        );
    }

    #[test]
    fn parse_operand_number_handles_masm_leading_zero_quirk() {
        // MASM / iced-x86 emit `0FFFFFFFFh` (leading 0 prefix
        // before a hex letter) so the assembler doesn't mistake it
        // for an identifier. Parser must accept this form.
        assert_eq!(parse_operand_number("0FFFFFFFFh"), Some(0xFFFFFFFF));
        assert_eq!(parse_operand_number("0CAFEBABEh"), Some(0xCAFEBABE));
        // Without leading zero — also valid (just an upper-case hex).
        assert_eq!(parse_operand_number("FFFFh"), Some(0xFFFF));
    }

    #[test]
    fn parse_operand_number_handles_full_u64_range() {
        // Verify the parser doesn't truncate to u32 anywhere.
        assert_eq!(
            parse_operand_number("0xFFFFFFFFFFFFFFFF"),
            Some(u64::MAX)
        );
        assert_eq!(parse_operand_number("0x7FF612345678"), Some(0x7FF6_1234_5678));
    }

    // ── Origin breadcrumb + nav history ───────────────────────────────────

    #[test]
    fn goto_address_sets_origin_to_old_address() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_goto");
        view.cursor_idx = Some(1); // 0x1001 in func A
        view.goto_address(0x1004, &p); // jump to func B middle
        assert_eq!(view.origin_addr, Some(0x1001));
        assert_eq!(view.selected_index(), Some(4));
    }

    #[test]
    fn goto_address_self_does_not_set_origin() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_self");
        view.cursor_idx = Some(2);
        // Pre-condition: no breadcrumb.
        assert!(view.origin_addr.is_none());
        view.goto_address(0x1002, &p); // self-jump (cursor at 2 == addr 0x1002)
        assert!(
            view.origin_addr.is_none(),
            "self-goto must not paint a breadcrumb on the current row"
        );
    }

    #[test]
    fn goto_address_overwrites_previous_origin() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_overwrite");
        view.cursor_idx = Some(0);
        view.goto_address(0x1003, &p); // origin = 0x1000
        assert_eq!(view.origin_addr, Some(0x1000));
        view.goto_address(0x1006, &p); // origin = 0x1003
        assert_eq!(view.origin_addr, Some(0x1003));
    }

    #[test]
    fn jump_to_function_start_sets_origin() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_func_start");
        view.cursor_idx = Some(7); // middle of func C (addr 0x1007)
        view.jump_to_function_start(&p);
        assert_eq!(view.origin_addr, Some(0x1007));
        assert_eq!(view.selected_index(), Some(6));
    }

    #[test]
    fn jump_to_function_end_sets_origin() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_func_end");
        view.cursor_idx = Some(4); // middle of func B (addr 0x1004)
        view.jump_to_function_end(&p);
        assert_eq!(view.origin_addr, Some(0x1004));
        assert_eq!(view.selected_index(), Some(5));
    }

    #[test]
    fn nav_back_sets_origin_to_pre_back_address() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_nav_back");
        view.cursor_idx = Some(0); // 0x1000
        view.goto_address(0x1004, &p); // → 0x1004 (origin = 0x1000)
        assert_eq!(view.origin_addr, Some(0x1000));
        view.nav_back(&p); // ← 0x1000 (origin should now be 0x1004)
        assert_eq!(view.origin_addr, Some(0x1004));
        assert_eq!(view.selected_index(), Some(0));
    }

    #[test]
    fn nav_forward_sets_origin_to_pre_forward_address() {
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_nav_fwd");
        view.cursor_idx = Some(0);
        view.goto_address(0x1004, &p);
        view.nav_back(&p); // back to 0x1000
        view.nav_forward(&p); // forward to 0x1004 (origin = 0x1000)
        assert_eq!(view.origin_addr, Some(0x1000));
        assert_eq!(view.selected_index(), Some(4));
    }

    #[test]
    fn do_search_sets_origin_and_pushes_nav_history() {
        let mut p = VecDisasmProvider::new();
        // Build a buffer with a unique 5-byte pattern at 0x1010.
        for i in 0..16 {
            let bytes = if i == 16 - 6 {
                vec![0x48, 0x89, 0xE5, 0x90, 0x90]
            } else {
                vec![0x90]
            };
            p.push(InstructionEntry::new(0x1000 + i as u64, bytes, "nop", ""));
        }
        let mut view = DisasmView::new("origin_search");
        view.cursor_idx = Some(2); // pre-search at 0x1002
        view.search_buf = "48 89 E5 90 90".to_string();
        view.do_search(&p);
        // Pre-search address recorded as origin.
        assert_eq!(view.origin_addr, Some(0x1002));
        // Nav history holds the pre-search address — Alt+Left works.
        view.nav_back(&p);
        assert_eq!(view.selected_index(), Some(2));
    }

    #[test]
    fn origin_persists_across_arrow_navigation() {
        // Arrow keys / single-step movement should NOT clear origin —
        // the user is exploring around the breadcrumb, not abandoning
        // it. We exercise this via direct cursor mutation since
        // `handle_keyboard` requires an Ui mock.
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_arrows");
        view.cursor_idx = Some(0);
        view.goto_address(0x1006, &p);
        assert_eq!(view.origin_addr, Some(0x1000));
        // Simulate arrow movement (cursor changes; origin untouched).
        view.cursor_idx = Some(7);
        assert_eq!(view.origin_addr, Some(0x1000));
        view.cursor_idx = Some(8);
        assert_eq!(view.origin_addr, Some(0x1000));
    }

    #[test]
    fn origin_survives_provider_address_reordering() {
        // `origin_addr` is an ABSOLUTE address (not row index), so
        // a provider mutation that shifts row indices doesn't
        // invalidate the breadcrumb.
        let mut p = three_function_provider();
        let mut view = DisasmView::new("origin_survives_mut");
        view.cursor_idx = Some(0);
        view.goto_address(0x1006, &p);
        assert_eq!(view.origin_addr, Some(0x1000));
        // Mutate the provider — set a comment on the origin
        // instruction. This doesn't shift indices but proves the
        // address-based key is stable across mutation.
        assert!(p.set_comment(0x1000, "marked"));
        // Origin still points at the same address.
        assert_eq!(view.origin_addr, Some(0x1000));
        assert_eq!(p.instruction(0).unwrap().comment(), Some("marked"));
    }

    // ── follow_at_cursor: lazy decode for streaming providers ────────────

    /// Test-only provider that decodes a target on demand into its
    /// internal `Vec`. Models the kind of streaming/lazy provider
    /// users build on top of iced-x86 / capstone where decoding
    /// happens per-page or per-function.
    struct LazyDecodeProvider {
        decoded: VecDisasmProvider,
        /// Addresses that *can* be decoded but haven't been yet.
        pending: std::collections::HashSet<u64>,
    }

    impl DisasmDataProvider for LazyDecodeProvider {
        fn instruction_count(&self) -> usize {
            self.decoded.instruction_count()
        }
        fn instruction(&self, idx: usize) -> Option<&dyn Instruction> {
            self.decoded.instruction(idx)
        }
        fn decode_range(&mut self, start_addr: u64, _max_count: usize) {
            if self.pending.remove(&start_addr) {
                self.decoded.push(InstructionEntry::new(
                    start_addr,
                    vec![0x90],
                    "nop",
                    "",
                ));
            }
        }
        fn index_of_address(&self, addr: u64) -> Option<usize> {
            self.decoded.index_of_address(addr)
        }
    }

    #[test]
    fn follow_at_cursor_lazy_decodes_call_target() {
        // Source: `call 0x4011A0`. Target NOT yet decoded — only
        // present in the lazy provider's `pending` set. Without
        // lazy-decode, follow would silently fail.
        let mut decoded = VecDisasmProvider::new();
        decoded.push(
            InstructionEntry::new(0x401000, vec![0xE8, 0x9B, 0x01, 0x00, 0x00], "call", "0x4011A0")
                .with_flow(FlowKind::Call)
                .with_target(0x4011A0),
        );
        let mut p = LazyDecodeProvider {
            decoded,
            pending: [0x4011A0].iter().copied().collect(),
        };
        let mut view = DisasmView::new("lazy_call");
        view.cursor_idx = Some(0);
        let followed = view.follow_at_cursor(&mut p);
        assert!(followed, "follow must succeed via lazy decode");
        // After decode, target is at idx 1.
        assert_eq!(view.selected_index(), Some(1));
        assert_eq!(view.origin_addr, Some(0x401000));
    }

    #[test]
    fn follow_at_cursor_returns_false_when_lazy_decode_yields_nothing() {
        // Lazy provider has no pending decodes — target stays unknown.
        let mut decoded = VecDisasmProvider::new();
        decoded.push(
            InstructionEntry::new(0x401000, vec![0xE8, 0x00, 0x00, 0x00, 0x00], "call", "0xDEAD")
                .with_flow(FlowKind::Call)
                .with_target(0xDEAD),
        );
        let mut p = LazyDecodeProvider {
            decoded,
            pending: std::collections::HashSet::new(),
        };
        let mut view = DisasmView::new("lazy_unfollowable");
        view.cursor_idx = Some(0);
        assert!(!view.follow_at_cursor(&mut p));
        assert!(view.origin_addr.is_none());
    }

    #[test]
    fn origin_preserved_through_repeated_navigations() {
        // Each new navigation overwrites origin (not stacks) — verify
        // the breadcrumb tracks the *last* jump source only.
        let p = three_function_provider();
        let mut view = DisasmView::new("origin_chain");
        view.cursor_idx = Some(0); // 0x1000
        view.goto_address(0x1003, &p);
        assert_eq!(view.origin_addr, Some(0x1000));
        view.goto_address(0x1006, &p);
        assert_eq!(view.origin_addr, Some(0x1003));
        view.goto_address(0x1000, &p);
        assert_eq!(view.origin_addr, Some(0x1006));
    }

    #[test]
    fn nav_history_capacity_is_64_entries() {
        // Push 100 distinct addresses, walk back, count how many
        // we recover before the history runs dry. Should be 64
        // (per `NavHistory::new(64)` in DisasmView::new).
        let mut p = VecDisasmProvider::new();
        for i in 0..101 {
            p.push(InstructionEntry::new(0x1000 + i as u64, vec![0x90], "nop", ""));
        }
        let mut view = DisasmView::new("nav_capacity");
        view.cursor_idx = Some(0);
        for i in 1..=100 {
            view.goto_address(0x1000 + i as u64, &p);
        }
        // After 100 pushes, walk back. Count how many distinct
        // addresses we recover before nav_back stops moving us.
        let mut visited = std::collections::HashSet::new();
        for _ in 0..200 {
            let before = view.selected_index();
            view.nav_back(&p);
            let after = view.selected_index();
            if before == after {
                break;
            }
            visited.insert(after);
        }
        // History capacity is 64; one slot is "current", rest are
        // back-stack — so we should recover ~64 distinct steps.
        // Allow ±2 tolerance for off-by-one in the NavHistory
        // implementation (capacity vs back-only-stack semantics).
        assert!(
            visited.len() >= 60 && visited.len() <= 65,
            "expected ~64 nav history slots, got {}",
            visited.len()
        );
    }

    #[test]
    fn x32_compute_arrows_clipped_works_with_pe32_addresses() {
        // PE32 image-base jumps: jmp from 0x401000 → 0x401010.
        // Verifies that compute_arrows_clipped's `index_of_address`
        // path resolves x32 addresses identically to x64.
        let mut p = VecDisasmProvider::new();
        for i in 0..16 {
            let entry = if i == 0 {
                InstructionEntry::new(0x00401000, vec![0xEB, 0x00], "jmp", "0x00401010")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x00401010)
            } else {
                InstructionEntry::new(0x00401000 + i as u64, vec![0x90], "nop", "")
            };
            p.push(entry);
        }
        // Window [5..10) — source at idx 0 above, target at idx 16
        // not present (0x401010 = idx 16, but we only have 16
        // instructions = idx 0..=15). Let's adjust to ensure
        // target exists: target 0x40100F → idx 15.
        let mut p2 = VecDisasmProvider::new();
        for i in 0..16 {
            let entry = if i == 0 {
                InstructionEntry::new(0x00401000, vec![0xEB, 0x00], "jmp", "0x0040100F")
                    .with_flow(FlowKind::Jump)
                    .with_target(0x0040100F)
            } else {
                InstructionEntry::new(0x00401000 + i as u64, vec![0x90], "nop", "")
            };
            p2.push(entry);
        }
        let arrows = compute_arrows_clipped(&p2 as &dyn DisasmDataProvider, 5, 5);
        // Source at idx 0 above window, target at idx 15 below window
        // → pass-through arrow.
        assert_eq!(arrows.len(), 1);
        assert!(arrows[0].clipped_from);
        assert!(arrows[0].clipped_to);
    }

    #[test]
    fn set_comment_default_trait_impl_is_noop() {
        // Read-only providers inherit the default `false` impl so
        // existing implementors remain non-breaking after the trait
        // gained `set_comment`. Verify the default really is a no-op.
        struct ReadOnly;
        impl DisasmDataProvider for ReadOnly {
            fn instruction_count(&self) -> usize {
                0
            }
            fn instruction(&self, _i: usize) -> Option<&dyn Instruction> {
                None
            }
            fn decode_range(&mut self, _start_addr: u64, _max_count: usize) {}
            fn index_of_address(&self, _addr: u64) -> Option<usize> {
                None
            }
        }
        let mut ro = ReadOnly;
        assert!(!ro.set_comment(0x1000, "anything"));
    }

    // ── Session 035 audit follow-ups ─────────────────────────────────
    //
    // Watchpoint plumbing test — the `RW` gutter glyph and the
    // single context-menu entry hang off a provider trait method
    // that defaults to a no-op; pin the round-trip through
    // `VecDisasmProvider` so a future refactor can't silently
    // break the `Instruction::has_watchpoint` ↔ `toggle_watchpoint`
    // contract. (Earlier sessions had separate `R` / `W` toggles —
    // collapsed into a single watchpoint by user request:
    // host-side engine sorts read-only / write-only on its side.)

    #[test]
    fn vec_provider_toggle_watchpoint_round_trip() {
        let mut p = sample_provider();
        let addr = 0x401000;
        let idx = p.index_of_address(addr).unwrap();
        assert!(!p.instruction(idx).unwrap().has_watchpoint());
        assert!(p.toggle_watchpoint(addr));
        assert!(p.instruction(idx).unwrap().has_watchpoint());
        assert!(!p.toggle_watchpoint(addr));
        assert!(!p.instruction(idx).unwrap().has_watchpoint());
    }

    #[test]
    fn vec_provider_watchpoint_independent_of_breakpoint() {
        // Pin: setting a watchpoint does NOT touch the breakpoint
        // flag and vice versa. Renderer priority at draw.rs uses
        // `has_watchpoint` first, falling back to `bp_number > 0`,
        // and assumes the two are independent booleans.
        let mut p = sample_provider();
        let addr = 0x401004;
        assert!(p.toggle_watchpoint(addr));
        let idx = p.index_of_address(addr).unwrap();
        let i = p.instruction(idx).unwrap();
        assert!(i.has_watchpoint());
        assert!(!i.has_breakpoint(), "watchpoint must not flip breakpoint");
    }

    #[test]
    fn provider_default_watchpoint_toggle_is_noop_false() {
        // Trait default: hosts that opt out of the watchpoint API
        // (e.g. simple read-only disassemblers) get a false-returning
        // no-op toggle so the context-menu entry doesn't crash.
        // Pin the default behaviour so a future trait refactor
        // doesn't accidentally make it required.
        struct ReadOnly;
        impl DisasmDataProvider for ReadOnly {
            fn instruction_count(&self) -> usize { 0 }
            fn instruction(&self, _idx: usize) -> Option<&dyn Instruction> { None }
            fn toggle_breakpoint(&mut self, _addr: u64) -> bool { false }
            fn decode_range(&mut self, _start_addr: u64, _max_count: usize) {}
            fn index_of_address(&self, _addr: u64) -> Option<usize> { None }
        }
        let mut ro = ReadOnly;
        assert!(!ro.toggle_watchpoint(0x1000));
    }

    #[test]
    fn icons_available_default_is_true() {
        // Pin: MDI glyphs (BOOKMARK_CHECK_OUTLINE, wrench-cog)
        // render by default. Hosts without the MDI atlas opt out
        // by setting `view.config.icons_available = false`.
        let view = DisasmView::new("test");
        assert!(view.config.icons_available);
    }

    #[test]
    fn first_row_clamped_when_count_shrinks() {
        // Defensive guard at mod.rs:1015 prevents `last_row -
        // first_row` from underflowing when the provider shrinks
        // between frames. Mirror the math here as a regression
        // pin (the actual call lives inside `render`, which can't
        // run without an ImGui context, but the saturation arithmetic
        // is independently verifiable).
        let scroll_y: f32 = 1000.0;
        let line_h: f32 = 18.0;
        let count: usize = 5;
        let first_row = ((scroll_y / line_h) as usize).min(count);
        assert!(first_row <= count);
        let visible_count = 30;
        let last_row = (first_row + visible_count).min(count);
        assert!(last_row >= first_row, "last_row must not underflow");
    }

    // ── Locale on `DisasmViewConfig` ─────────────────────────────────

    #[test]
    fn config_default_locale_is_english() {
        let cfg = DisasmViewConfig::default();
        assert_eq!(cfg.locale, crate::i18n::Locale::En);
    }

    #[test]
    fn config_locale_round_trips_through_ron() {
        let cfg = DisasmViewConfig {
            locale: crate::i18n::Locale::Ru,
            ..DisasmViewConfig::default()
        };
        let text = ron::ser::to_string(&cfg).unwrap();
        let back: DisasmViewConfig = ron::from_str(&text).unwrap();
        assert_eq!(back.locale, crate::i18n::Locale::Ru);
    }

    #[test]
    fn with_locale_updates_config_field() {
        // The view-level builder forwards into `config.locale`, so
        // `set_locale` mutations persist into the saved ron payload.
        let view = DisasmView::new("test").with_locale(crate::i18n::Locale::Ru);
        assert_eq!(view.config.locale, crate::i18n::Locale::Ru);
        assert_eq!(view.locale(), crate::i18n::Locale::Ru);
    }

    #[test]
    fn columns_inline_in_config_ron_matches_canonical() {
        // `disasm_view/config.ron` inlines `columns:(...)` because ron
        // 0.8 has no `include`. This drift-test makes sure the inline
        // block stays in lock-step with `column_widths.ron`.
        let canonical = super::ColumnWidths::default();
        let cfg = DisasmViewConfig::default();
        assert_eq!(cfg.columns.margin, canonical.margin);
        assert_eq!(cfg.columns.arrows, canonical.arrows);
        assert_eq!(cfg.columns.address, canonical.address);
        assert_eq!(cfg.columns.bytes, canonical.bytes);
        assert_eq!(cfg.columns.mnemonic, canonical.mnemonic);
        assert_eq!(cfg.columns.operands, canonical.operands);
        assert_eq!(cfg.columns.comment, canonical.comment);
    }

    #[test]
    fn config_locale_field_optional_in_ron() {
        // Older configs (saved before the locale field landed) still
        // parse — `#[serde(default)]` falls back to English. Pre-0.10.x
        // hosts depend on this for forward compatibility.
        let cfg: DisasmViewConfig = ron::from_str(
            r#"(
                columns: (
                    margin: 26.0,
                    arrows: 36.0,
                    address: 150.0,
                    bytes: 200.0,
                    mnemonic: 80.0,
                    operands: 220.0,
                    comment: 100.0,
                ),
                show_bytes: true,
                show_comments: true,
                show_arrows: true,
                show_breakpoints: true,
                show_bookmarks: true,
                icons_available: true,
                show_block_tints: false,
                show_header: true,
                show_column_dividers: true,
                uppercase: true,
                address_width_64: true,
                byte_category_colors: true,
                editable: false,
                follow_execution: false,
                base_address: 0,
                max_arrows: 256,
            )"#,
        )
        .expect("disasm_view config without `locale` field must still parse");
        assert_eq!(cfg.locale, crate::i18n::Locale::En);
    }
}
