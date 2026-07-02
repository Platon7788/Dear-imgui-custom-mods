//! # CodeEditor
//!
//! Full-featured code editor widget for Dear ImGui with token-level syntax
//! highlighting, line numbers, cursor/selection, undo/redo, bracket matching,
//! find/replace, code folding, and draw-call-batched rendering.
//!
//! Built entirely on ImGui's DrawList API — no `InputTextMultiline`, giving
//! full control over rendering and input handling.
//!
//! ## Architecture
//!
//! ```text
//! code_editor/
//! ├── mod.rs          CodeEditor widget + render + input dispatch
//! ├── buffer.rs       TextBuffer (lines, cursor, selection, editing,
//! │                   line-ending + tab-style detection)
//! ├── config.rs       EditorConfig, SyntaxColors, Language, BuiltinFont
//! ├── token.rs        Token + TokenKind types
//! ├── tokenizer.rs    Tokenizer trait
//! ├── lang/           Per-language syntax definitions (Rust, TOML, RON,
//! │                   Rhai, JSON, YAML, XML, ASM, Hex, None)
//! ├── helpers.rs      Free functions — layout math, color parsing,
//! │                   clipboard/input FFI, bracket pairs, hash
//! ├── find_replace.rs FindScope + FindReplaceState (scoped search +
//! │                   lowercase cache)
//! ├── fold.rs         FoldRegion + detect_fold_regions
//! ├── wrap.rs         compute_wrap_points (word-wrap point finder)
//! └── undo.rs         UndoStack with grouping + alloc-free should_push
//! ```
//!
//! ## Key optimizations
//!
//! - **Draw call batching**: consecutive tokens of the same color are merged into
//!   a single `AddText` call, reducing draw calls by ~3–5×.
//! - **Token cache**: per-line tokenization is cached and only recomputed when the
//!   line content or block-comment state changes.
//! - **Viewport culling**: only visible lines are rendered.
//! - **Smooth scrolling**: animated scroll with exponential ease-out.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::code_editor::{CodeEditor, Language};
//!
//! let mut editor = CodeEditor::new("my_editor");
//! editor.set_language(Language::Rust);
//! editor.set_text("fn main() {\n    println!(\"Hello\");\n}");
//!
//! // In your render loop:
//! // editor.render(ui);
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
pub mod buffer;
pub mod config;
pub mod font_setup;
pub mod lang;
pub mod syntax_colors;
pub mod token;
pub mod tokenizer;
pub mod undo;

pub use config::{
    BuiltinFont, CODE_EDITOR_FONT_PTR, ContextMenuConfig, EditorConfig, EditorTheme,
    HACK_FONT_DATA, JETBRAINS_MONO_FONT_DATA, JETBRAINS_MONO_LIGATURES_FONT_DATA, Language,
    MDI_FONT_DATA, SyntaxColors, SyntaxDefinition, code_editor_font_ptr, install_code_editor_font,
    install_code_editor_font_ex, install_custom_code_editor_font, merge_mdi_icons,
};

mod find_replace;
mod fold;
mod helpers;
mod wrap;
use fold::{FoldRegion, detect_fold_regions};
use helpers::{
    calc_char_advance, closing_bracket, closing_quote, col_to_x, col32, digit_count, get_clipboard,
    hash_line, hex_auto_space_needed, is_closing_bracket, is_closing_quote, parse_hex_color,
    read_input_chars, set_clipboard, tab_stop_spaces, title_case, x_to_col,
};
use wrap::compute_wrap_points;

pub use find_replace::{FindReplaceState, FindScope};

use buffer::{CursorPos, LineEnding, Selection, TextBuffer};
use lang::tokenize_line;
use token::{Token, TokenKind};
use undo::{UndoEntry, UndoStack};

use crate::icons;
use dear_imgui_rs::{Key, MouseButton, StyleColor, StyleVar, Ui, WindowFlags};
use std::collections::HashSet;
use std::rc::Rc;

// ── Error/warning markers ────────────────────────────────────────────────────

/// Error/warning marker on a line.
#[derive(Debug, Clone)]
pub struct LineMarker {
    /// 0-based line number.
    pub line: usize,
    /// Message text (shown on hover).
    pub message: String,
    /// True = error (red), false = warning (yellow).
    pub is_error: bool,
}

/// Breakpoint on a line.
#[derive(Debug, Clone, Copy)]
pub struct Breakpoint {
    /// 0-based line number.
    pub line: usize,
    /// Whether this breakpoint is enabled.
    pub enabled: bool,
}

// ── Token cache ──────────────────────────────────────────────────────────────

/// Cached tokenization result for a single line.
struct CachedLineTokens {
    /// The line content when tokens were computed.
    content_hash: u64,
    /// Whether the line started inside a block comment.
    in_block_comment: bool,
    /// Computed tokens (Rc avoids cloning on every frame).
    tokens: Rc<Vec<Token>>,
}

// ── The CodeEditor widget ────────────────────────────────────────────────────

/// The CodeEditor widget.
pub struct CodeEditor {
    /// Pre-baked `##ce_<id>` ImGui child-window identifier — built once
    /// in `new()` instead of `format!`-allocating every render.
    /// `Arc<str>` so we can hand out a cheap refcount-bumped clone
    /// before the child-window closure (which itself takes `&mut self`)
    /// without borrowing `self` for the immutable handle.
    child_id: std::sync::Arc<str>,
    buffer: TextBuffer,
    config: EditorConfig,
    undo_stack: UndoStack,

    // ── Rendering state ──────────────────────────────────────────────
    scroll_x: f32,
    scroll_y: f32,
    /// Target scroll Y for smooth scrolling.
    target_scroll_y: f32,
    /// scroll_y we wrote to ImGui last frame — used to detect external scrollbar drags.
    last_set_scroll_y: f32,
    /// Computed character advance width (monospace).
    char_advance: f32,
    /// Computed line height.
    line_height: f32,
    /// Cached visible height of the editor window.
    visible_height: f32,
    /// Whether the editor is focused.
    focused: bool,
    /// Previous frame's focus state — used to detect focus transitions.
    was_focused: bool,
    /// Saved keyboard layout handle to restore when editor loses focus.
    #[cfg(target_os = "windows")]
    saved_input_locale: usize,
    /// Cursor blink timer.
    blink_timer: f32,
    /// Whether cursor is currently visible (blink state).
    cursor_visible: bool,

    // ── Token cache ──────────────────────────────────────────────────
    /// Per-line cached tokenization.
    token_cache: Vec<Option<CachedLineTokens>>,
    /// Per-line "starts in block comment" flags.
    block_comment_states: Vec<bool>,
    /// Edit version when block_comment_states was last computed.
    bc_version: u64,
    /// Earliest line that may have changed (for incremental bc recompute).
    bc_dirty_from: Option<usize>,

    // ── Markers ──────────────────────────────────────────────────────
    error_markers: Vec<LineMarker>,
    error_lines: HashSet<usize>,
    breakpoints: Vec<Breakpoint>,
    breakpoint_lines: HashSet<usize>,

    // ── Find/Replace ─────────────────────────────────────────────────
    find_replace: FindReplaceState,

    // ── Code folding ─────────────────────────────────────────────────
    fold_regions: Vec<FoldRegion>,
    /// Edit version when fold_regions were last computed.
    fold_version: u64,

    // ── Mouse state ──────────────────────────────────────────────────
    mouse_selecting: bool,
    last_click_time: f64,
    last_click_pos: CursorPos,
    click_count: u8,

    // ── Word wrap cache ──────────────────────────────────────────────
    /// Per-line wrap column offsets.  Empty vec = line fits in one row.
    wrap_cols: Vec<Vec<usize>>,
    /// Prefix-sum of visual rows: `wrap_row_offset[i]` = total visual
    /// rows for lines `0..i`.  Length = line_count + 1.
    wrap_row_offsets: Vec<usize>,
    /// The text width (in px) used when the wrap cache was last built.
    wrap_cached_width: f32,
    /// Edit version when the wrap cache was last built.
    wrap_cached_version: u64,

    // ── Per-frame scratch ─────────────────────────────────────────────
    /// Pre-allocated `String` re-used to format the gutter line number
    /// once per visible row. `clear()` + `write!` keeps capacity, so
    /// repeated renders are zero-alloc after the first frame — replaces
    /// the historic `format!("{}", line_idx + 1)` heap allocation that
    /// fired ≥ visible-rows times every frame.
    gutter_buf: String,
}

impl CodeEditor {
    /// Override the user-visible language. Default English; pass
    /// [`crate::i18n::Locale::Ru`] for Russian. The host must bake
    /// `GlyphRanges::Cyrillic` into the active font atlas — without
    /// that, Cyrillic characters render as `?`.
    ///
    /// The locale is stored on [`EditorConfig::locale`] so it
    /// round-trips through `ron::to_string` / `ron::from_str`.
    #[must_use]
    pub fn with_locale(mut self, locale: crate::i18n::Locale) -> Self {
        self.config.locale = locale;
        self
    }

    /// Mid-flight language switch.
    pub fn set_locale(&mut self, locale: crate::i18n::Locale) {
        self.config.locale = locale;
    }

    /// Currently-active locale.
    pub fn locale(&self) -> crate::i18n::Locale {
        self.config.locale
    }

    /// Create a new editor instance.
    pub fn new(id: &str) -> Self {
        let child_id: std::sync::Arc<str> = format!("##ce_{id}").into();
        Self {
            child_id,
            buffer: TextBuffer::default(),
            config: EditorConfig::default(),
            undo_stack: UndoStack::new(500),

            scroll_x: 0.0,
            scroll_y: 0.0,
            target_scroll_y: 0.0,
            last_set_scroll_y: 0.0,
            char_advance: 7.0,
            line_height: 16.0,
            visible_height: 300.0,
            focused: false,
            was_focused: false,
            #[cfg(target_os = "windows")]
            saved_input_locale: 0,
            blink_timer: 0.0,
            cursor_visible: true,

            token_cache: Vec::new(),
            block_comment_states: vec![false],
            bc_version: u64::MAX,
            bc_dirty_from: None,

            error_markers: Vec::new(),
            error_lines: HashSet::new(),
            breakpoints: Vec::new(),
            breakpoint_lines: HashSet::new(),

            find_replace: FindReplaceState::default(),

            fold_regions: Vec::new(),
            fold_version: u64::MAX,

            mouse_selecting: false,
            last_click_time: 0.0,
            last_click_pos: CursorPos::default(),
            click_count: 0,

            wrap_cols: Vec::new(),
            wrap_row_offsets: vec![0],
            wrap_cached_width: 0.0,
            wrap_cached_version: u64::MAX,

            // 12 covers every plausible line count (`u32::MAX` = 10 digits).
            gutter_buf: String::with_capacity(12),
        }
    }
}

// ── Method impls (split into sibling modules, CLAUDE.md 500-line rule) ──
mod api;
mod cache;
mod draw;
mod draw_lines;
mod find_chrome;
mod find_glue;
mod input;
mod input_mouse;
mod input_text;
mod layout;
mod render;

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests;
