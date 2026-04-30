//! Crate-wide palette types — the colour tokens consumed by every
//! component that participates in the unified theme system.
//!
//! ## Why these live here
//!
//! Colour palettes are part of the **theme contract**, not implementation
//! detail of the consuming widget. Earlier they were defined inside each
//! component (`borderless_window::TitlebarColors`, `nav_panel::NavColors`,
//! `confirm_dialog::DialogColors`, `notifications::NotificationColors`),
//! which forced [`crate::theme`] — an *infrastructure* module — to import
//! from feature-gated *consumer* modules to construct its built-in
//! palettes. That is a textbook inverse dependency: the foundation
//! reaches up into the higher layers.
//!
//! After session 022 every palette **type** lives here. Consumer
//! modules `pub use` them back, and the theme modules build palettes
//! from a single import:
//!
//! ```ignore
//! use super::{TitlebarColors, NavColors, DialogColors, NotificationColors};
//! ```
//!
//! No `#[cfg(feature = …)]` walls, no upward references.
//!
//! ## What is *not* here yet
//!
//! [`crate::status_bar::StatusBarConfig`] still combines layout fields
//! (height, padding, separator widths) with colour fields. Splitting
//! those would touch the entire `status_bar` rendering path; the
//! refactor is scheduled separately. Until then,
//! [`crate::theme::Theme::statusbar`] remains the one cross-module
//! palette accessor that depends on a consumer module.

// ── TitlebarColors ──────────────────────────────────────────────────────────

/// A complete set of colours for the borderless titlebar, consumed by
/// [`crate::app_window`]'s chrome layer.
#[derive(Debug, Clone)]
pub struct TitlebarColors {
    /// Titlebar background.
    pub bg: [f32; 4],
    /// 1-px separator line below the titlebar.
    pub separator: [f32; 4],
    /// Title text color.
    pub title: [f32; 4],
    /// Minimize button icon color.
    pub btn_minimize: [f32; 4],
    /// Maximize / restore button icon color.
    pub btn_maximize: [f32; 4],
    /// Close button icon color.
    pub btn_close: [f32; 4],
    /// Hover background for minimize and maximize buttons.
    pub btn_hover_bg: [f32; 4],
    /// Hover background for the close button.
    pub btn_close_hover_bg: [f32; 4],
    /// Window icon color (if `BorderlessConfig::icon` is set).
    pub icon: [f32; 4],
    /// Titlebar background colour used to "erase" overlapping icon layers (restore icon).
    pub bg_erase: [f32; 4],
    /// Subtle hover tint over the drag-move zone.
    pub drag_hint: [f32; 4],
    /// Titlebar background when the window loses OS focus.
    pub bg_inactive: [f32; 4],
    /// Title text color when the window loses OS focus.
    pub title_inactive: [f32; 4],
}

// ── DialogColors ────────────────────────────────────────────────────────────

/// Complete colour set for the confirm dialog. Consumed by
/// [`crate::confirm_dialog`].
#[derive(Debug, Clone)]
pub struct DialogColors {
    /// Fullscreen dim overlay behind the dialog.
    pub overlay: [f32; 4],
    /// Dialog window background.
    pub bg: [f32; 4],
    /// Dialog border color.
    pub border: [f32; 4],
    /// Title / header text color.
    pub title: [f32; 4],
    /// Body message text color.
    pub message: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],

    /// Icon color for Warning type.
    pub icon_warning: [f32; 4],
    /// Icon color for Error type.
    pub icon_error: [f32; 4],
    /// Icon color for Info type.
    pub icon_info: [f32; 4],
    /// Icon color for Question type.
    pub icon_question: [f32; 4],

    /// Confirm (destructive) button background — red.
    pub btn_confirm: [f32; 4],
    /// Confirm button hover.
    pub btn_confirm_hover: [f32; 4],
    /// Confirm button active/press.
    pub btn_confirm_active: [f32; 4],
    /// Confirm button text.
    pub btn_confirm_text: [f32; 4],

    /// Cancel (safe) button background — green.
    pub btn_cancel: [f32; 4],
    /// Cancel button hover.
    pub btn_cancel_hover: [f32; 4],
    /// Cancel button active/press.
    pub btn_cancel_active: [f32; 4],
    /// Cancel button text.
    pub btn_cancel_text: [f32; 4],
}

// ── NavColors ───────────────────────────────────────────────────────────────

/// Complete colour set for the navigation panel. Consumed by
/// [`crate::nav_panel`].
#[derive(Debug, Clone)]
pub struct NavColors {
    /// Panel background.
    pub bg: [f32; 4],
    /// Button hover background.
    pub btn_hover: [f32; 4],
    /// Active button background.
    pub btn_active: [f32; 4],
    /// Active indicator bar color (accent).
    pub indicator: [f32; 4],
    /// Default icon tint (monochrome fallback).
    pub icon_default: [f32; 4],
    /// Icon color when active.
    pub icon_active: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],
    /// Badge circle background.
    pub badge_bg: [f32; 4],
    /// Badge text color.
    pub badge_text: [f32; 4],
    /// Submenu flyout background.
    pub submenu_bg: [f32; 4],
    /// Submenu item hover.
    pub submenu_hover: [f32; 4],
    /// Submenu item text.
    pub submenu_text: [f32; 4],
    /// Submenu border.
    pub submenu_border: [f32; 4],
    /// Submenu separator.
    pub submenu_separator: [f32; 4],
    /// Toggle button icon color.
    pub toggle_icon: [f32; 4],
}

// ── StatusBarColors ─────────────────────────────────────────────────────────

/// Colour subset for the status bar — the *theme* part of
/// [`crate::status_bar::StatusBarConfig`]. The layout fields (height,
/// padding, separator widths, progress dimensions, hover-feedback flag)
/// stay on `StatusBarConfig` because they describe how `status_bar`
/// renders rather than what colours it uses.
///
/// Built-in themes expose this both directly via
/// [`crate::theme::Theme::statusbar_colors`] (always available, no
/// feature gate) and as part of the full
/// [`crate::theme::Theme::statusbar`] config (gated on the
/// `status_bar` feature because that returns `StatusBarConfig`).
#[derive(Debug, Clone, Copy)]
pub struct StatusBarColors {
    /// Bar background color.
    pub bg: [f32; 4],
    /// Default text color.
    pub text: [f32; 4],
    /// Dimmed/secondary text color.
    pub text_dim: [f32; 4],
    /// Separator line color.
    pub separator: [f32; 4],
    /// Hovered item background.
    pub hover: [f32; 4],
    /// Clicked item background.
    pub active: [f32; 4],

    /// Success indicator color (green dot).
    pub success: [f32; 4],
    /// Warning indicator color (yellow dot).
    pub warning: [f32; 4],
    /// Error indicator color (red dot).
    pub error: [f32; 4],
    /// Info indicator color (blue dot).
    pub info: [f32; 4],
}

impl Default for StatusBarColors {
    /// NxT-Dark default — pinned to `Theme::Dark.statusbar_colors()` so
    /// callers that construct a `StatusBarConfig::default()` without
    /// going through the theme system still get the **same** surface
    /// shade as `Theme::Dark.nav().bg` and the rest of the chrome
    /// stack. (Pre-2026-04-29 this default was stale-hardcoded and
    /// rendered noticeably darker than the matching nav/titlebar
    /// surfaces — see `theme::tests::default_matches_dark_theme`.)
    fn default() -> Self {
        Self {
            // Dark theme tokens, expressed as `[u8 / 255]`:
            //   bg        = STATUSBAR_BG (0x2B2F38)
            //   text      = FG           (0xE0E4EA)
            //   text_dim  = FG_MUTED     (0x8A92A1)
            //   separator = BORDER       (0x3F4654)
            //   hover     = SECONDARY_HOVER (0x48505E)
            //   active    = SECONDARY_HOVER
            //   success   = SUCCESS      (0x5FB870)
            //   warning   = WARNING      (0xD9A643)
            //   error     = DANGER       (0xE06060)
            //   info      = ACCENT       (0x5B9BD5)
            bg: [
                0x2B as f32 / 255.0,
                0x2F as f32 / 255.0,
                0x38 as f32 / 255.0,
                1.0,
            ],
            text: [
                0xE0 as f32 / 255.0,
                0xE4 as f32 / 255.0,
                0xEA as f32 / 255.0,
                1.0,
            ],
            text_dim: [
                0x8A as f32 / 255.0,
                0x92 as f32 / 255.0,
                0xA1 as f32 / 255.0,
                1.0,
            ],
            separator: [
                0x3F as f32 / 255.0,
                0x46 as f32 / 255.0,
                0x54 as f32 / 255.0,
                1.0,
            ],
            hover: [
                0x48 as f32 / 255.0,
                0x50 as f32 / 255.0,
                0x5E as f32 / 255.0,
                1.0,
            ],
            active: [
                0x48 as f32 / 255.0,
                0x50 as f32 / 255.0,
                0x5E as f32 / 255.0,
                1.0,
            ],

            success: [
                0x5F as f32 / 255.0,
                0xB8 as f32 / 255.0,
                0x70 as f32 / 255.0,
                1.0,
            ],
            warning: [
                0xD9 as f32 / 255.0,
                0xA6 as f32 / 255.0,
                0x43 as f32 / 255.0,
                1.0,
            ],
            error: [
                0xE0 as f32 / 255.0,
                0x60 as f32 / 255.0,
                0x60 as f32 / 255.0,
                1.0,
            ],
            info: [
                0x5B as f32 / 255.0,
                0x9B as f32 / 255.0,
                0xD5 as f32 / 255.0,
                1.0,
            ],
        }
    }
}

// ── HexViewerColors ─────────────────────────────────────────────────────────

/// Complete palette for the [`crate::hex_viewer::HexViewer`] widget.
///
/// 18 colour tokens grouped by purpose:
/// - **5 byte categories** (`cat_zero`, `cat_control`, `cat_printable`,
///   `cat_high`, `cat_full`) — semantic byte-value tinting.
/// - **8 UI text** (`offset`, `hex`, `ascii`, `ascii_dot`, `header`,
///   `inspector_label`, `inspector_value`, `zero_legacy`) — gutter,
///   content, header row, data inspector.
/// - **5 highlight surfaces** (`selection_bg`, `cursor_bg`, `changed`,
///   `search_match`, `unreadable`) — interactive overlays.
///
/// Built-in themes expose this via
/// [`crate::theme::Theme::hex_viewer_colors`]; bare
/// `HexViewerConfig::default()` uses [`HexViewerColors::default`]
/// which mirrors `Theme::Dark.hex_viewer_colors()`.
#[derive(Debug, Clone, Copy)]
pub struct HexViewerColors {
    // ── Byte categories ─────────────────────────────────────────
    /// `0x00` byte.
    pub cat_zero: [f32; 4],
    /// `0x01..=0x1F` + `0x7F` (control chars).
    pub cat_control: [f32; 4],
    /// `0x20..=0x7E` (printable ASCII).
    pub cat_printable: [f32; 4],
    /// `0x80..=0xFE` (high / extended).
    pub cat_high: [f32; 4],
    /// `0xFF` byte.
    pub cat_full: [f32; 4],

    // ── UI text ─────────────────────────────────────────────────
    /// Address gutter colour — primary "where am I" cue.
    pub offset: [f32; 4],
    /// Default hex byte text colour (used when category tinting is off).
    pub hex: [f32; 4],
    /// Printable char in the ASCII column.
    pub ascii: [f32; 4],
    /// `.` placeholder for non-printable bytes in the ASCII column.
    pub ascii_dot: [f32; 4],
    /// Column-header row ("Offset / 00 01 02 ... / ASCII").
    pub header: [f32; 4],
    /// Inspector label colour (e.g. `u16=`).
    pub inspector_label: [f32; 4],
    /// Inspector value colour (the decoded number itself).
    pub inspector_value: [f32; 4],
    /// Legacy zero-byte colour (used when `category_colors == false` and
    /// `dim_zeros == true`).
    pub zero_legacy: [f32; 4],

    // ── Highlight surfaces ──────────────────────────────────────
    /// Background fill behind selected bytes.
    pub selection_bg: [f32; 4],
    /// Background fill behind the cursor byte.
    pub cursor_bg: [f32; 4],
    /// Foreground colour for bytes that differ from the reference snapshot.
    pub changed: [f32; 4],
    /// Background fill behind the bytes that match the active search.
    pub search_match: [f32; 4],
    /// Background fill for bytes the data provider reports as
    /// unreadable (gaps in a memory dump).
    pub unreadable: [f32; 4],
}

impl Default for HexViewerColors {
    /// Mirrors `Theme::Dark.hex_viewer_colors()` — see
    /// `theme::tests::hex_viewer_colors_default_matches_dark_theme`.
    fn default() -> Self {
        crate::theme::dark::hex_viewer_colors()
    }
}

/// Semantic token bundle each theme passes to [`HexViewerColors::from_tokens`].
/// Lets every per-theme palette be expressed in 9 lines instead of
/// reproducing all 18 hex_viewer fields by hand.
#[doc(hidden)]
pub struct HexViewerTokens {
    /// Primary content text — the colour the theme uses for `FG`.
    pub fg: [f32; 4],
    /// Muted text — `FG_MUTED`.
    pub fg_muted: [f32; 4],
    /// Theme accent — drives `offset` + `cursor_bg` (alpha-modulated).
    pub accent: [f32; 4],
    /// Semantic green (printable bytes, success).
    pub success: [f32; 4],
    /// Semantic amber (`0xFF` byte, search highlight).
    pub warning: [f32; 4],
    /// Semantic red (`changed`, `unreadable`).
    pub danger: [f32; 4],
    /// Purple-family hue for `0x80..0xFE` "high" bytes — pick a colour
    /// distinct from accent and success so the category really stands out.
    pub purple: [f32; 4],
}

impl HexViewerColors {
    /// Build a [`HexViewerColors`] from a small bundle of semantic
    /// tokens. Used by every per-theme `hex_viewer_colors()` so the
    /// 18-field palette stays consistent — only the seed colours
    /// change between themes.
    pub fn from_tokens(t: &HexViewerTokens) -> Self {
        // Helper: same RGB as `c`, but with alpha overridden.
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        Self {
            // Byte categories — distinct hues so users can scan dumps quickly.
            cat_zero: with_a(t.fg_muted, 0.45),
            cat_control: with_a(t.fg_muted, 0.70),
            cat_printable: t.success,
            cat_high: t.purple,
            cat_full: t.warning,

            // UI text.
            offset: t.accent,
            hex: t.fg,
            ascii: t.success,
            ascii_dot: with_a(t.fg_muted, 0.45),
            // Column header captions ("Offset", "00 01 ... 1F", "ASCII")
            // — pinned to `fg` (full-strength text colour) so they read
            // as bright white in dark themes and bold dark in light
            // themes. Earlier this token landed on `fg_muted`, which
            // made the header row visually closer to a comment than to
            // a label and the user reported it as washed-out.
            header: t.fg,
            inspector_label: t.fg_muted,
            inspector_value: t.fg,
            zero_legacy: with_a(t.fg_muted, 0.40),

            // Highlight surfaces — alpha-modulated theme tokens so the
            // overlays tint the underlying byte text instead of clobbering it.
            selection_bg: with_a(t.accent, 0.40),
            cursor_bg: with_a(t.accent, 0.45),
            changed: t.warning,
            search_match: with_a(t.warning, 0.35),
            unreadable: with_a(t.danger, 0.25),
        }
    }
}

// ── DisasmViewColors ────────────────────────────────────────────────────────

/// Instruction control-flow classification — re-exported here so the
/// palette factory below can build per-flow mnemonic / arrow colours
/// without pulling in the whole `disasm_view` module. Mirrors
/// [`crate::disasm_view::FlowKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisasmFlowKind {
    /// Normal sequential instruction (mov, add, lea, etc.).
    #[default]
    Normal,
    /// Unconditional/conditional jump (jmp, je, jne, etc.).
    Jump,
    /// Call instruction.
    Call,
    /// Return instruction (ret, iret).
    Return,
    /// NOP / INT3 / padding instruction.
    Nop,
    /// Stack manipulation (push, pop, sub rsp).
    Stack,
    /// System instruction (syscall, sysenter, int).
    System,
    /// Invalid / undecodable instruction.
    Invalid,
}

/// Complete colour set for the [`crate::disasm_view::DisasmView`] widget
/// — 26 individual fields covering mnemonic colouring (per
/// [`DisasmFlowKind`]), operand syntax tinting, address / bytes /
/// comment, branch arrows, alternating block tints, breakpoint markers,
/// the current-execution highlight, selection / hover / header /
/// separator surfaces.
///
/// Built-in themes expose this via
/// [`crate::theme::Theme::disasm_view_colors`]; bare
/// `DisasmViewConfig::default()` uses [`DisasmViewColors::default`]
/// which mirrors `Theme::Dark.disasm_view_colors()`. Re-exported from
/// `crate::disasm_view` as `DisasmColors` for backwards compatibility.
#[derive(Debug, Clone)]
pub struct DisasmViewColors {
    // ── Mnemonic colors by flow kind ────────────────────────
    /// Normal instruction mnemonic (mov, add, lea, etc.).
    pub mnemonic_normal: [f32; 4],
    /// Jump/branch mnemonic.
    pub mnemonic_jump: [f32; 4],
    /// Call mnemonic.
    pub mnemonic_call: [f32; 4],
    /// Return mnemonic.
    pub mnemonic_return: [f32; 4],
    /// NOP/INT3/padding.
    pub mnemonic_nop: [f32; 4],
    /// Stack operations (push, pop).
    pub mnemonic_stack: [f32; 4],
    /// System instructions (syscall, int).
    pub mnemonic_system: [f32; 4],
    /// Invalid instruction.
    pub mnemonic_invalid: [f32; 4],

    // ── Operand colors ──────────────────────────────────────
    /// Register names.
    pub operand_register: [f32; 4],
    /// Numeric constants / immediates.
    pub operand_number: [f32; 4],
    /// Memory dereference brackets and operators.
    pub operand_memory: [f32; 4],
    /// String operands.
    pub operand_string: [f32; 4],
    /// Default operand text.
    pub operand_default: [f32; 4],

    // ── Address / bytes ─────────────────────────────────────
    /// Address column color.
    pub address: [f32; 4],
    /// Hex bytes column color — used as the fallback when
    /// per-byte category tinting is disabled.
    pub bytes: [f32; 4],
    /// Comment color.
    pub comment: [f32; 4],

    // ── Byte categories (mirrors `HexViewerColors` cat_*) ───
    // Per-byte semantic tint for the Bytes column. Picks the same
    // 5-tier ByteCategory split hex_viewer uses so the same buffer
    // reads identically across both widgets.
    /// `0x00` byte — null padding.
    pub bytes_cat_zero: [f32; 4],
    /// `0x01..=0x1F` + `0x7F` — control chars.
    pub bytes_cat_control: [f32; 4],
    /// `0x20..=0x7E` — printable ASCII.
    pub bytes_cat_printable: [f32; 4],
    /// `0x80..=0xFE` — high / extended.
    pub bytes_cat_high: [f32; 4],
    /// `0xFF` — all-ones byte.
    pub bytes_cat_full: [f32; 4],

    // ── Branch arrows ───────────────────────────────────────
    /// Jump arrow color.
    pub arrow_jump: [f32; 4],
    /// Call arrow color.
    pub arrow_call: [f32; 4],
    /// Return arrow color.
    pub arrow_return: [f32; 4],
    /// Default arrow color (other flow kinds).
    pub arrow_default: [f32; 4],

    // ── Block tinting ───────────────────────────────────────
    /// Background tint colors for alternating code blocks.
    pub block_tints: Vec<[f32; 4]>,

    // ── UI elements ─────────────────────────────────────────
    /// Breakpoint marker color (fallback when no number available).
    pub breakpoint: [f32; 4],
    /// Breakpoint gutter background.
    pub breakpoint_bg: [f32; 4],
    /// Numbered breakpoint colors (cycle through these).
    pub breakpoint_colors: Vec<[f32; 4]>,
    /// Current execution point (stopped-at) background.
    pub current_line_bg: [f32; 4],
    /// Selected row background.
    pub selection_bg: [f32; 4],
    /// Row hover highlight.
    pub hover_bg: [f32; 4],
    /// Background fill for instruction rows whose bytes match the
    /// active byte search. Sits between `current_line_bg` (warning
    /// hue, exec pointer) and `selection_bg` (accent hue, click)
    /// visually so the three highlights never read as the same
    /// state.
    pub search_match_bg: [f32; 4],
    /// Column header / separator color.
    pub header: [f32; 4],
    /// Separator line between columns.
    pub separator: [f32; 4],
}

impl Default for DisasmViewColors {
    /// Mirrors `Theme::Dark.disasm_view_colors()` — see
    /// `theme::tests::disasm_view_colors_default_matches_dark_theme`.
    fn default() -> Self {
        crate::theme::dark::disasm_view_colors()
    }
}

impl DisasmViewColors {
    /// Get mnemonic color for a given flow kind.
    pub fn mnemonic_color(&self, kind: DisasmFlowKind) -> [f32; 4] {
        match kind {
            DisasmFlowKind::Normal => self.mnemonic_normal,
            DisasmFlowKind::Jump => self.mnemonic_jump,
            DisasmFlowKind::Call => self.mnemonic_call,
            DisasmFlowKind::Return => self.mnemonic_return,
            DisasmFlowKind::Nop => self.mnemonic_nop,
            DisasmFlowKind::Stack => self.mnemonic_stack,
            DisasmFlowKind::System => self.mnemonic_system,
            DisasmFlowKind::Invalid => self.mnemonic_invalid,
        }
    }

    /// Get arrow color for a given flow kind.
    pub fn arrow_color(&self, kind: DisasmFlowKind) -> [f32; 4] {
        match kind {
            DisasmFlowKind::Jump => self.arrow_jump,
            DisasmFlowKind::Call => self.arrow_call,
            DisasmFlowKind::Return => self.arrow_return,
            _ => self.arrow_default,
        }
    }

    /// Get breakpoint color by number (1-based). Falls back to
    /// [`Self::breakpoint`] when `number == 0` or no
    /// numbered colours are configured.
    pub fn bp_color(&self, number: u32) -> [f32; 4] {
        if number == 0 || self.breakpoint_colors.is_empty() {
            return self.breakpoint;
        }
        self.breakpoint_colors[((number - 1) as usize) % self.breakpoint_colors.len()]
    }

    /// Per-byte foreground colour for the Bytes column — same
    /// 5-tier `ByteCategory` split that
    /// [`crate::hex_viewer::HexViewerConfig::byte_fg_color`] uses, so
    /// the same buffer reads identically across both widgets. Always
    /// returns a category-tinted colour; pass `colors.bytes`
    /// directly at the call site to opt out.
    pub fn byte_fg_color(&self, byte: u8) -> [f32; 4] {
        match byte {
            0x00 => self.bytes_cat_zero,
            0x01..=0x1F | 0x7F => self.bytes_cat_control,
            0x20..=0x7E => self.bytes_cat_printable,
            0xFF => self.bytes_cat_full,
            _ => self.bytes_cat_high, // 0x80..=0xFE
        }
    }

    /// Get block tint color for a given block index.
    pub fn block_tint(&self, block_index: usize) -> [f32; 4] {
        if self.block_tints.is_empty() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        self.block_tints[block_index % self.block_tints.len()]
    }

    /// Build a [`DisasmViewColors`] from a small bundle of semantic
    /// tokens. Used by every per-theme `disasm_view_colors()` so the
    /// 26-field palette stays consistent — only the seed colours
    /// change between themes.
    pub fn from_tokens(t: &DisasmViewTokens) -> Self {
        let with_a = |c: [f32; 4], a: f32| [c[0], c[1], c[2], a];
        Self {
            // ── Mnemonics — semantic per FlowKind ────────────────
            mnemonic_normal: t.fg,
            mnemonic_jump: t.warning,
            mnemonic_call: t.success,
            mnemonic_return: t.danger,
            mnemonic_nop: with_a(t.fg_muted, 0.60),
            mnemonic_stack: t.purple,
            mnemonic_system: t.orange,
            mnemonic_invalid: t.danger,

            // ── Operands ─────────────────────────────────────────
            operand_register: t.cyan,
            operand_number: t.success,
            operand_memory: t.orange,
            operand_string: with_a(t.warning, 0.85),
            operand_default: with_a(t.fg, 0.95),

            // ── Address / bytes / comment ────────────────────────
            address: with_a(t.accent, 0.85),
            bytes: with_a(t.fg_muted, 0.85),
            comment: with_a(t.success, 0.75),

            // ── Byte categories — mirror hex_viewer ──────────────
            // Same 5-tier split (`HexViewerColors::from_tokens`) so
            // the Bytes column reads identically in both widgets.
            bytes_cat_zero: with_a(t.fg_muted, 0.45),
            bytes_cat_control: with_a(t.fg_muted, 0.70),
            bytes_cat_printable: t.success,
            bytes_cat_high: t.purple,
            bytes_cat_full: t.warning,

            // ── Branch arrows — same family as mnemonics ─────────
            arrow_jump: with_a(t.warning, 0.90),
            arrow_call: with_a(t.success, 0.90),
            arrow_return: with_a(t.danger, 0.90),
            arrow_default: with_a(t.fg_muted, 0.70),

            // ── Block tints — derived from semantic hues so the
            //    rotation reads on both light and dark surfaces. Alpha
            //    deliberately tiny (≤ 0.10) — the tint should hint
            //    at block boundaries, not compete with the byte text.
            block_tints: vec![
                with_a(t.accent, 0.07),
                with_a(t.danger, 0.06),
                with_a(t.success, 0.06),
                with_a(t.warning, 0.06),
                with_a(t.purple, 0.06),
                with_a(t.cyan, 0.06),
            ],

            // ── UI ───────────────────────────────────────────────
            breakpoint: t.danger,
            breakpoint_bg: with_a(t.danger, 0.30),
            breakpoint_colors: vec![
                t.danger,
                t.cyan,
                t.warning,
                t.success,
                t.purple,
                t.orange,
                t.accent,
                with_a(t.danger, 0.85),
            ],
            current_line_bg: with_a(t.warning, 0.35),
            selection_bg: with_a(t.accent, 0.45),
            hover_bg: with_a(t.fg, 0.04),
            // Search-match row background — semantic-green (success)
            // hue at low alpha. Distinct from warning (exec) and
            // accent (selection) so the three row states never
            // collide visually.
            search_match_bg: with_a(t.success, 0.32),
            // Header row labels ("Address", "Bytes", "Instruction",
            // "Comment") — pinned to `fg` (full-strength text) so
            // they read as bright white in dark themes and bold dark
            // in light themes. Mirrors `HexViewerColors::header`,
            // which had the same washed-out problem before being
            // bumped from `fg_muted` to `fg`.
            header: t.fg,
            separator: with_a(t.fg_muted, 0.40),
        }
    }
}

/// Semantic token bundle each theme passes to
/// [`DisasmViewColors::from_tokens`]. Lets every per-theme palette be
/// expressed in 9 lines instead of reproducing all 26 disasm_view
/// fields by hand.
#[doc(hidden)]
pub struct DisasmViewTokens {
    /// Primary content text — `FG` of the theme.
    pub fg: [f32; 4],
    /// Muted text — `FG_MUTED`.
    pub fg_muted: [f32; 4],
    /// Theme accent — drives `address` (alpha-modulated) + `selection_bg`.
    pub accent: [f32; 4],
    /// Semantic green — `Call` mnemonic, numeric operands, comments.
    pub success: [f32; 4],
    /// Semantic amber — `Jump` mnemonic, current-line bg, jump arrows.
    pub warning: [f32; 4],
    /// Semantic red — `Return` / `Invalid` mnemonics, breakpoint marker.
    pub danger: [f32; 4],
    /// Purple-family hue — `Stack` mnemonic (`push` / `pop`), distinct
    /// from accent / warning so stack ops stand out.
    pub purple: [f32; 4],
    /// Orange-family hue — `System` mnemonic + memory operand brackets
    /// (`[rsp+0x10]`); MUST contrast against `cyan` and `success`.
    pub orange: [f32; 4],
    /// Cyan-family hue — register operand colour (`rax`, `xmm0`, etc.).
    /// Must contrast against the theme accent so registers don't merge
    /// into the address gutter.
    pub cyan: [f32; 4],
}

// ── NotificationColors ──────────────────────────────────────────────────────

/// Complete colour set for the notification center. Consumed by
/// [`crate::notifications`].
///
/// Unlike the other palettes, this one ships with five named-preset
/// constructors (`dark()`, `light()`, `midnight()`, `solarized()`,
/// `monokai()`). The built-in [`crate::theme::Theme`] variants delegate
/// to these constructors so adding a new theme that wants the default
/// notification look is one method call.
#[derive(Debug, Clone)]
pub struct NotificationColors {
    /// Toast window background.
    pub bg: [f32; 4],
    /// Toast border.
    pub border: [f32; 4],
    /// Title text.
    pub title: [f32; 4],
    /// Body text (dimmer than title).
    pub body: [f32; 4],
    /// `×` close-button glyph — default.
    pub close: [f32; 4],
    /// `×` close-button glyph — hover.
    pub close_hover: [f32; 4],
    /// Progress-bar track (background).
    pub progress_bg: [f32; 4],

    // Severity accents — used for icon color + left accent strip + progress fill.
    /// Info severity — default blue.
    pub info: [f32; 4],
    /// Success severity — default green.
    pub success: [f32; 4],
    /// Warning severity — default amber.
    pub warning: [f32; 4],
    /// Error severity — default red.
    pub error: [f32; 4],
    /// Debug severity — default gray.
    pub debug: [f32; 4],

    /// Action-button background — default.
    pub btn_action: [f32; 4],
    /// Action-button background — hover.
    pub btn_action_hover: [f32; 4],
    /// Action-button background — active / pressed.
    pub btn_action_active: [f32; 4],
    /// Action-button text.
    pub btn_action_text: [f32; 4],
}

impl NotificationColors {
    /// NxT dark palette — matches `Theme::Dark`.
    pub fn dark() -> Self {
        Self {
            bg: [0.18, 0.20, 0.24, 0.96],
            border: [0.28, 0.31, 0.37, 1.0],
            title: [0.88, 0.90, 0.92, 1.0],
            body: [0.68, 0.71, 0.77, 1.0],
            close: [0.54, 0.57, 0.63, 1.0],
            close_hover: [0.95, 0.95, 0.95, 1.0],
            progress_bg: [0.25, 0.27, 0.32, 0.8],

            info: [0.36, 0.61, 0.84, 1.0],
            success: [0.37, 0.72, 0.44, 1.0],
            warning: [0.85, 0.65, 0.25, 1.0],
            error: [0.88, 0.37, 0.37, 1.0],
            debug: [0.55, 0.58, 0.64, 1.0],

            btn_action: [0.28, 0.31, 0.38, 1.0],
            btn_action_hover: [0.35, 0.40, 0.48, 1.0],
            btn_action_active: [0.22, 0.25, 0.31, 1.0],
            btn_action_text: [0.92, 0.94, 0.96, 1.0],
        }
    }

    /// Light palette — matches `Theme::Light`.
    pub fn light() -> Self {
        Self {
            bg: [0.98, 0.98, 0.99, 0.98],
            border: [0.78, 0.80, 0.84, 1.0],
            title: [0.12, 0.14, 0.18, 1.0],
            body: [0.36, 0.39, 0.44, 1.0],
            close: [0.50, 0.54, 0.60, 1.0],
            close_hover: [0.10, 0.12, 0.16, 1.0],
            progress_bg: [0.88, 0.89, 0.92, 0.8],

            info: [0.18, 0.48, 0.76, 1.0],
            success: [0.18, 0.60, 0.32, 1.0],
            warning: [0.82, 0.55, 0.16, 1.0],
            error: [0.80, 0.22, 0.22, 1.0],
            debug: [0.46, 0.49, 0.55, 1.0],

            btn_action: [0.86, 0.88, 0.92, 1.0],
            btn_action_hover: [0.78, 0.82, 0.88, 1.0],
            btn_action_active: [0.70, 0.74, 0.82, 1.0],
            btn_action_text: [0.14, 0.16, 0.20, 1.0],
        }
    }

    /// Midnight palette — Tokyo Night accent, OLED-friendly.
    pub fn midnight() -> Self {
        Self {
            bg: [0.06, 0.07, 0.10, 0.97],
            border: [0.18, 0.20, 0.28, 1.0],
            title: [0.86, 0.88, 0.94, 1.0],
            body: [0.58, 0.62, 0.72, 1.0],
            close: [0.46, 0.49, 0.58, 1.0],
            close_hover: [0.92, 0.94, 0.98, 1.0],
            progress_bg: [0.12, 0.14, 0.20, 0.8],

            info: [0.50, 0.72, 0.96, 1.0],
            success: [0.58, 0.82, 0.62, 1.0],
            warning: [0.95, 0.78, 0.42, 1.0],
            error: [0.94, 0.46, 0.52, 1.0],
            debug: [0.48, 0.52, 0.62, 1.0],

            btn_action: [0.14, 0.17, 0.24, 1.0],
            btn_action_hover: [0.20, 0.24, 0.32, 1.0],
            btn_action_active: [0.10, 0.12, 0.18, 1.0],
            btn_action_text: [0.88, 0.90, 0.96, 1.0],
        }
    }

    /// Solarized-dark palette.
    pub fn solarized() -> Self {
        Self {
            bg: [0.0, 0.17, 0.21, 0.97],
            border: [0.03, 0.21, 0.26, 1.0],
            title: [0.93, 0.91, 0.84, 1.0],
            body: [0.51, 0.58, 0.59, 1.0],
            close: [0.40, 0.48, 0.51, 1.0],
            close_hover: [0.93, 0.91, 0.84, 1.0],
            progress_bg: [0.03, 0.21, 0.26, 0.8],

            info: [0.15, 0.55, 0.82, 1.0],   // blue
            success: [0.52, 0.60, 0.0, 1.0], // green
            warning: [0.71, 0.54, 0.0, 1.0], // yellow
            error: [0.86, 0.20, 0.18, 1.0],  // red
            debug: [0.40, 0.48, 0.51, 1.0],  // base01

            btn_action: [0.03, 0.21, 0.26, 1.0],
            btn_action_hover: [0.06, 0.28, 0.33, 1.0],
            btn_action_active: [0.0, 0.17, 0.21, 1.0],
            btn_action_text: [0.93, 0.91, 0.84, 1.0],
        }
    }

    /// Monokai-Pro palette — warm charcoal with neon accents.
    pub fn monokai() -> Self {
        Self {
            bg: [0.16, 0.16, 0.16, 0.97],
            border: [0.26, 0.24, 0.23, 1.0],
            title: [0.98, 0.96, 0.90, 1.0],
            body: [0.64, 0.62, 0.58, 1.0],
            close: [0.50, 0.48, 0.44, 1.0],
            close_hover: [1.0, 0.98, 0.92, 1.0],
            progress_bg: [0.22, 0.20, 0.19, 0.8],

            info: [0.47, 0.78, 0.91, 1.0],    // cyan
            success: [0.67, 0.82, 0.40, 1.0], // green
            warning: [1.0, 0.76, 0.31, 1.0],  // yellow/orange
            error: [1.0, 0.40, 0.44, 1.0],    // red
            // Debug = neutral grey across every theme so the docs claim
            // ("Gray — developer-only diagnostic") holds true universally.
            // Earlier Monokai used purple here, which surprised callers
            // expecting a low-saturation tone.
            debug: [0.62, 0.60, 0.56, 1.0], // warm grey (matches FG_MUTED tone)

            btn_action: [0.26, 0.24, 0.23, 1.0],
            btn_action_hover: [0.34, 0.32, 0.30, 1.0],
            btn_action_active: [0.20, 0.18, 0.17, 1.0],
            btn_action_text: [0.98, 0.96, 0.90, 1.0],
        }
    }
}
