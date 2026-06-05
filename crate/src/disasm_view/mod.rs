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
//! ## Follow-at-cursor gesture (double-click on `call` / `jmp` / `Jcc`)
//!
//! Double-clicking the Instruction column (or pressing `Enter` /
//! `Space` while the row is selected) invokes
//! [`DisasmView::follow_at_cursor`]. The resolution order is:
//!
//! 1. The provider's [`provider::Instruction::branch_target`] —
//!    the **canonical** path. Hosts should set this on every
//!    branching row via [`provider::InstructionEntry::with_target`].
//! 2. Operand-string fallback — scans for the first
//!    [`tokens::TokenKind::Number`] that resolves to a known
//!    instruction. For `Call` / `Jump` rows the scanner skips
//!    numbers inside `[...]` (those are displacements, not
//!    targets) so `call qword ptr [rip+0x1234]` doesn't chase
//!    `0x1234` into nowhere.
//!
//! For streaming providers, `goto_address` calls
//! [`provider::DisasmDataProvider::decode_range`] once before
//! giving up, so a target landing in not-yet-decoded territory
//! still has a chance to resolve.
//!
//! Hosts that want a status-line hint when follow fails ("Cannot
//! follow: target 0x4011A0 not in provider") call
//! [`DisasmView::follow_at_cursor_diagnostic`] instead — it returns
//! a [`FollowOutcome`] enumerating the precise reason the gesture
//! did or didn't navigate (missing `.with_target(...)`, register-
//! indirect / symbolic operand, or a target outside the provider).
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

// ── Knowledge-layer re-exports (ADR-030) ────────────────────────────────────
//
// The catalogue / detector layer lives in the headless
// `disasm-knowledge` crate (sibling workspace `useful-lib/`). Re-export
// the 9 modules under their historic names so external consumers that
// import `dear_imgui_custom_mod::disasm_view::mnemonic::lookup` keep
// compiling without churn. New code can use either path —
// `disasm_knowledge::mnemonic::lookup` is the canonical one.
pub use disasm_knowledge::{HintTiers, HintVerbosity};
pub use disasm_knowledge::{
    abi, antidisasm, boundary, branch, compiler, hint_verbosity, idiom, mnemonic, operand,
};

// UI-only modules (rendering, layout, host data contract) stay here.
pub mod arrows;
pub mod config;
pub mod provider;

mod draw;
mod edit_state;
mod input;
mod popup;
mod tokens;

// `impl DisasmView` split across cohesive sibling files (session 043).
// The struct + its fields stay in this file; each module below holds
// one `impl DisasmView { ... }` block plus any free items in its theme.
mod api;
mod nav;
mod render;
mod search;
mod selection;

// Inline-edit state types live in `edit_state.rs`. Re-export at
// `pub(crate)` so `crate::disasm_view::EditColumn` / `super::EditState`
// keep resolving from the render / input / draw submodules and from the
// `EditState` field on `DisasmView`.
pub(crate) use edit_state::{EditColumn, EditState};

pub use arrows::{BranchArrow, MAX_ARROW_DEPTH, compute_arrows, compute_arrows_clipped};
pub use config::{ColumnWidths, DisasmColors, DisasmViewConfig};
pub use provider::{
    DisasmDataProvider, FlowKind, Instruction, InstructionEntry, VecDisasmProvider,
};

// Re-export the public navigation surface (function-boundary helpers +
// the follow-at-cursor outcome enum) that moved into `nav.rs`, so
// `disasm_view::FollowOutcome` / `find_function_start` keep resolving.
pub use nav::{FollowOutcome, find_function_end, find_function_start};

// Private free helper that moved into `nav.rs`. A private re-export binds
// the name `disasm_view::parse_address`, which the child `popup` module
// reaches via `super::parse_address` — no wider visibility is required.
// (`parse_operand_number` also lives in `nav.rs` but is only consumed by
// the test suite, which imports it directly via `super::super::nav::…`.)
use nav::parse_address;

// ── Locale bridge (ADR-030) ─────────────────────────────────────────────────
//
// The UI crate carries its own `crate::i18n::Locale` (lives alongside
// per-widget string catalogues for hex_viewer / code_editor / etc.); the
// headless knowledge crate carries `disasm_knowledge::Locale` with the
// same shape (En / Ru). Only the UI→knowledge direction is needed —
// every call site sits inside the UI crate and uses `cfg.locale.into()`
// to hand the knowledge-side function the matching variant. The
// reverse direction has no caller and is omitted; consumers that
// genuinely need it can implement it in their own crate trivially.
impl From<crate::i18n::Locale> for disasm_knowledge::Locale {
    fn from(l: crate::i18n::Locale) -> Self {
        match l {
            crate::i18n::Locale::En => disasm_knowledge::Locale::En,
            crate::i18n::Locale::Ru => disasm_knowledge::Locale::Ru,
        }
    }
}

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
    /// origin. Single-clicks **preserve** the breadcrumb on purpose so
    /// the user can scroll / click around without losing the jump
    /// source — see the explicit "keep" comment in
    /// `input.rs::handle_mouse`. Auto-suppressed when navigation lands
    /// on the address it would set as origin (no breadcrumb-on-self).
    pub(super) origin_addr: Option<u64>,
    /// Cached arrows for the visible window, keyed by
    /// [`Self::cached_arrows_key`]. While the user hasn't scrolled and
    /// the provider hasn't grown (and `config.max_arrows` is unchanged)
    /// the previous `Vec` is reused intact — without the cache a 10k-
    /// instruction provider re-walks the full list every frame just to
    /// populate the visible arrow slice (audit H3).
    cached_arrows: Vec<BranchArrow>,
    /// `(first_row, last_row, instruction_count, max_arrows)` — the
    /// inputs `compute_arrows_clipped` depends on. `None` means the
    /// cache is dirty and must be rebuilt next render.
    cached_arrows_key: Option<(usize, usize, usize, usize)>,
    /// Position for InputText widget (set by draw_row, consumed by render).
    edit_render_pos: std::cell::Cell<Option<[f32; 2]>>,
    /// Width for the InputText widget.
    edit_render_width: std::cell::Cell<f32>,
    /// Screen-space centre of the disasm child window — captured per
    /// frame so modal popups (Goto / future Settings) can anchor at
    /// the visual middle. Mirrors the pattern used by `hex_viewer`.
    pub(super) component_center: [f32; 2],
    /// Screen-space anchor for the right-click context menu — set by
    /// the right-click handler (`input::handle_mouse`) to the cursor
    /// position so the menu spawns where the user clicked. `None` means
    /// "not yet captured", so the popup falls back to ImGui's default
    /// cursor-based anchor instead of pinning to screen origin (the
    /// pre-`Option` `[0.0, 0.0]` bug — audit M7).
    pub(super) popup_open_pos: Option<[f32; 2]>,
    /// Per-frame comment-column X (screen space). Computed in
    /// `render()` from a one-pass scan over visible rows: when the
    /// widest instruction text would collide with the default
    /// comment column, the comment + its left divider slide right
    /// just enough to clear it (plus a small `COMMENT_GAP` cushion).
    /// `Cell` so `mouse_to_cell` (called via `&self`) can read the
    /// previous frame's value for hit-testing — the 1-frame lag is
    /// invisible for the double-click gesture. `Option` so frame 0
    /// (before `render()` ran) is distinguishable from any legal X
    /// including 0.0 (audit M1, session 034 — replaced a `≤ 0.0`
    /// sentinel).
    pub(super) frame_comment_x: std::cell::Cell<Option<f32>>,
    /// Per-frame comment-column WIDTH (screen-space pixels).
    /// Computed in `render()` as
    /// `(window_w - comment_x).max(cols.comment)` so the Comment column
    /// stretches to fill the host window down to a `cols.comment` floor.
    /// Read by `mouse_to_cell` (so double-click hit-testing extends to
    /// the full visible width) and the comment edit-cell renderer.
    /// `None` only on frame 0.
    pub(super) frame_comment_w: std::cell::Cell<Option<f32>>,

    /// Bookmark address set — pure view-state UI navigation aid (not
    /// tied to running-process concepts like breakpoints). Up to
    /// [`Self::MAX_BOOKMARKS`] addresses; the `BTreeSet` keeps them
    /// sorted for stable host-side save/restore. Cross-session
    /// persistence is the host's job — read [`Self::bookmarks`] on
    /// shutdown, replay via [`Self::add_bookmark`] on startup.
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
            cached_arrows_key: None,
            edit_render_pos: std::cell::Cell::new(None),
            edit_render_width: std::cell::Cell::new(0.0),
            component_center: [0.0, 0.0],
            popup_open_pos: None,
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
    #[must_use]
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    /// Builder: replace the entire configuration in one call (audit
    /// M2). Restoring a saved [`DisasmViewConfig`] is the canonical
    /// use case — fewer touch-points than mutating ~20 individual
    /// `config.*` fields by hand.
    #[must_use]
    pub fn with_config(mut self, cfg: DisasmViewConfig) -> Self {
        self.config = cfg;
        self.cached_arrows_key = None;
        self
    }

    /// Replace the entire configuration at runtime. Like
    /// [`Self::with_config`] but for `&mut self` flows.
    pub fn set_config(&mut self, cfg: DisasmViewConfig) {
        self.config = cfg;
        self.cached_arrows_key = None;
    }

    /// Read-only access to the underlying [`DisasmViewConfig`].
    /// Symmetric with the public `pub config` field — host code is
    /// free to use either; the accessor exists so future migration
    /// to a private field doesn't break callers.
    #[must_use]
    pub fn config(&self) -> &DisasmViewConfig {
        &self.config
    }

    /// Mutable access. Use to flip individual flags at runtime
    /// (toggle bytes column, change ABI, …). Any change to
    /// `show_arrows` / `max_arrows` invalidates the arrow cache on
    /// the next render via the in-render key compare; no manual
    /// invalidation needed.
    pub fn config_mut(&mut self) -> &mut DisasmViewConfig {
        &mut self.config
    }

    /// Static catalogue lookup for the current locale. Convenience
    /// for the per-frame popup / draw paths.
    #[inline]
    pub(super) fn strings(&self) -> &'static crate::i18n::disasm_view::Strings {
        crate::i18n::disasm_view::strings(self.config.locale)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
