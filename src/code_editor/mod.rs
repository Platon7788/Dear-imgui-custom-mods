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
//! ├── mod.rs          CodeEditor widget + render + input
//! ├── buffer.rs       TextBuffer (lines, cursor, selection, editing)
//! ├── tokenizer.rs    Rust/TOML/RON syntax tokenizer
//! ├── config.rs       EditorConfig, SyntaxColors, Language
//! └── undo.rs         UndoStack with action grouping
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
pub mod lang;
pub mod token;
pub mod tokenizer;
pub mod undo;

pub use config::{
    code_editor_font_ptr, install_code_editor_font, install_code_editor_font_ex,
    install_custom_code_editor_font, merge_mdi_icons, BuiltinFont, ContextMenuConfig,
    EditorConfig, EditorTheme, Language, SyntaxColors, SyntaxDefinition,
    CODE_EDITOR_FONT_PTR, HACK_FONT_DATA, JETBRAINS_MONO_FONT_DATA,
    JETBRAINS_MONO_LIGATURES_FONT_DATA, MDI_FONT_DATA,
};

use buffer::{CursorPos, Selection, TextBuffer};
use lang::tokenize_line;
use token::{Token, TokenKind};
use undo::{UndoEntry, UndoStack};

use crate::utils::color::rgba_f32;

use std::collections::HashSet;
use std::rc::Rc;
use crate::icons;
use dear_imgui_rs::{Key, MouseButton, StyleColor, Ui, WindowFlags};

/// Pack an `[f32; 4]` RGBA color into u32 for DrawList.
#[inline]
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Parse a hex color literal into `[r, g, b, a]` (all 0.0–1.0).
///
/// Supports: `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `0xRRGGBB`, `0xAARRGGBB`.
fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    #[inline]
    fn byte(hex: &str, pos: usize) -> Option<f32> {
        u8::from_str_radix(&hex[pos..pos + 2], 16).ok().map(|v| v as f32 / 255.0)
    }
    if let Some(hex) = s.strip_prefix('#') {
        let all_hex = hex.chars().all(|c| c.is_ascii_hexdigit());
        return match (hex.len(), all_hex) {
            (3, true) => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()? as f32 / 255.0;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()? as f32 / 255.0;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()? as f32 / 255.0;
                Some([r, g, b, 1.0])
            }
            (6, true) => Some([byte(hex, 0)?, byte(hex, 2)?, byte(hex, 4)?, 1.0]),
            (8, true) => Some([byte(hex, 0)?, byte(hex, 2)?, byte(hex, 4)?, byte(hex, 6)?]),
            _ => None,
        };
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let all_hex = hex.chars().all(|c| c.is_ascii_hexdigit());
        return match (hex.len(), all_hex) {
            (6, true) => Some([byte(hex, 0)?, byte(hex, 2)?, byte(hex, 4)?, 1.0]),
            (8, true) => {
                // 0xAARRGGBB
                let a = byte(hex, 0)?;
                let r = byte(hex, 2)?;
                let g = byte(hex, 4)?;
                let b = byte(hex, 6)?;
                Some([r, g, b, a])
            }
            _ => None,
        };
    }
    None
}

/// Convert a string to Title Case (first char of each word uppercased).
fn title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut new_word = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            new_word = true;
            result.push(ch);
        } else if new_word {
            result.extend(ch.to_uppercase());
            new_word = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Measure the exact per-glyph advance width using `ImFont::CalcTextSizeA`
/// **directly on the font object**, bypassing the high-level `igCalcTextSize`
/// wrapper which applies `ceil(w + 0.99999)` rounding.
///
/// Ported from ImGuiColorTextEdit:
/// ```cpp
/// ImGui::GetFont()->CalcTextSizeA(ImGui::GetFontSize(), FLT_MAX, -1.0f, "#")
/// ```
///
/// Using `"#"` as the reference character is the ImGuiColorTextEdit convention.
/// For truly monospace fonts every glyph shares the same AdvanceX, so one
/// measurement per frame is sufficient.
///
/// # Safety
/// `igGetFont()` returns the currently-active ImGui font pointer which is
/// guaranteed valid for the lifetime of a frame.
fn calc_char_advance(font_size: f32) -> f32 {
    let text = b"#\0";
    // SAFETY: igGetFont valid for this frame; text is a valid null-terminated string.
    unsafe {
        let font = dear_imgui_rs::sys::igGetFont();
        let size = dear_imgui_rs::sys::ImFont_CalcTextSizeA(
            font,
            font_size,
            f32::MAX,
            -1.0,
            text.as_ptr() as *const std::os::raw::c_char,
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        size.x
    }
}

/// Convert a column index to pixel X offset, accounting for tab characters.
///
/// Walks the line's characters up to `col`, summing per-character widths:
/// regular characters use `char_advance`, tabs use `tab_size * char_advance`
/// (matching `draw_tokens_batched`).  For lines without tabs this reduces to
/// `col * char_advance`.
#[inline]
fn col_to_x(line: &str, col: usize, char_advance: f32, tab_size: u8) -> f32 {
    let mut x = 0.0f32;
    for (i, ch) in line.chars().enumerate() {
        if i == col { return x; }
        if ch == '\t' {
            x += char_advance * tab_size as f32;
        } else {
            x += char_advance;
        }
    }
    x
}

/// Convert a pixel X offset to a column index, accounting for tab characters.
///
/// Uses a **0.67-width** threshold (from ImGuiColorTextEdit): clicking the
/// left third of a character places the cursor *before* it; clicking the
/// right two-thirds places it *after*.
#[inline]
fn x_to_col(line: &str, x: f32, char_advance: f32, tab_size: u8) -> usize {
    let mut cur_x = 0.0f32;
    for (i, ch) in line.chars().enumerate() {
        let ch_w = if ch == '\t' {
            char_advance * tab_size as f32
        } else {
            char_advance
        };
        if x < cur_x + ch_w * 0.67 {
            return i;
        }
        cur_x += ch_w;
    }
    line.chars().count()
}

/// Set clipboard text via ImGui sys API.
fn set_clipboard(text: &str) {
    let c_str = std::ffi::CString::new(text).unwrap_or_default();
    // SAFETY: igSetClipboardText takes a null-terminated C string, which CString provides.
    unsafe { dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr()); }
}

/// Get clipboard text via ImGui sys API.
fn get_clipboard() -> Option<String> {
    // SAFETY: igGetClipboardText returns a pointer to ImGui's internal buffer.
    let ptr = unsafe { dear_imgui_rs::sys::igGetClipboardText() };
    if ptr.is_null() { return None; }
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    c_str.to_str().ok().map(|s| s.to_string())
}

/// Read input characters from ImGui's input queue (typed this frame).
fn read_input_chars() -> Vec<char> {
    // SAFETY: igGetIO returns a valid pointer to the global ImGuiIO struct.
    // InputQueueCharacters is an ImVector of ImWchar (u16) with typed chars this frame.
    unsafe {
        let io = &*dear_imgui_rs::sys::igGetIO_Nil();
        let data = io.InputQueueCharacters.Data;
        let size = io.InputQueueCharacters.Size;
        if data.is_null() || size <= 0 { return Vec::new(); }
        let slice = std::slice::from_raw_parts(data, size as usize);
        slice.iter().filter_map(|&wc| char::from_u32(wc)).collect()
    }
}

// ── Matching bracket pairs for auto-close ────────────────────────────────────

const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('{', '}'), ('[', ']')];
const QUOTE_PAIRS: &[(char, char)] = &[('"', '"'), ('\'', '\'')];

fn closing_bracket(ch: char) -> Option<char> {
    BRACKET_PAIRS.iter().find(|(o, _)| *o == ch).map(|(_, c)| *c)
}

fn closing_quote(ch: char) -> Option<char> {
    QUOTE_PAIRS.iter().find(|(o, _)| *o == ch).map(|(_, c)| *c)
}

fn is_closing_bracket(ch: char) -> bool {
    BRACKET_PAIRS.iter().any(|(_, c)| *c == ch)
}

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

/// Fast non-cryptographic hash for line content comparison.
fn hash_line(s: &str) -> u64 {
    // FNV-1a 64-bit
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ── Find/Replace state ───────────────────────────────────────────────────────

/// Find/Replace panel state.
#[derive(Debug, Default)]
pub struct FindReplaceState {
    /// Whether the find panel is open.
    pub open: bool,
    /// Set to true the frame the panel is opened — used to auto-focus the input.
    pub just_opened: bool,
    /// Search query.
    pub query: String,
    /// Replacement text.
    pub replacement: String,
    /// Case-sensitive search.
    pub case_sensitive: bool,
    /// Whole-word search.
    pub whole_word: bool,
    /// Whether the replace field is visible.
    pub show_replace: bool,
    /// All match positions: (line, col_start, col_end) in chars.
    pub matches: Vec<(usize, usize, usize)>,
    /// Current match index (for cycling through matches).
    pub current_match: usize,
}

impl FindReplaceState {
    fn update_matches(&mut self, lines: &[String]) {
        self.matches.clear();
        if self.query.is_empty() { return; }

        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        for (line_idx, line) in lines.iter().enumerate() {
            let search_line = if self.case_sensitive {
                line.clone()
            } else {
                line.to_lowercase()
            };

            let mut start = 0;
            while let Some(pos) = search_line[start..].find(&query) {
                let byte_start = start + pos;
                let byte_end = byte_start + query.len();
                let col_start = buffer::byte_to_char(line, byte_start);
                let col_end = buffer::byte_to_char(line, byte_end);

                if self.whole_word {
                    let before_ok = byte_start == 0 ||
                        !line.as_bytes()[byte_start - 1].is_ascii_alphanumeric();
                    let after_ok = byte_end >= line.len() ||
                        !line.as_bytes()[byte_end].is_ascii_alphanumeric();
                    if before_ok && after_ok {
                        self.matches.push((line_idx, col_start, col_end));
                    }
                } else {
                    self.matches.push((line_idx, col_start, col_end));
                }

                start = byte_start + query.len().max(1);
            }
        }

        if self.current_match >= self.matches.len() {
            self.current_match = 0;
        }
    }
}

// ── Code folding ─────────────────────────────────────────────────────────────

/// A foldable region in the code.
#[derive(Debug, Clone)]
struct FoldRegion {
    /// Start line (the line with `fn`, `struct`, `impl`, `{`, etc.).
    start_line: usize,
    /// End line (the line with the closing `}`).
    end_line: usize,
    /// Whether this region is currently folded.
    folded: bool,
}

/// Detects fold regions by matching `{` / `}` and `// region:` / `// endregion`.
fn detect_fold_regions(lines: &[String]) -> Vec<FoldRegion> {
    let mut regions = Vec::new();
    let mut brace_stack: Vec<usize> = Vec::new();
    let mut region_stack: Vec<usize> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Comment-based region markers: `// region: Name` / `// endregion`
        if let Some(rest) = trimmed.strip_prefix("//") {
            let comment = rest.trim_start();
            if comment.starts_with("region:") || comment.starts_with("region ") {
                region_stack.push(i);
                continue;
            }
            if comment.starts_with("endregion") {
                if let Some(start) = region_stack.pop()
                    && i > start
                {
                    regions.push(FoldRegion {
                        start_line: start,
                        end_line: i,
                        folded: false,
                    });
                }
                continue;
            }
        }

        // Brace matching (simplified: doesn't handle strings/comments perfectly,
        // but good enough for Rust code structure)
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_stack.push(i),
                '}' => {
                    if let Some(start) = brace_stack.pop() {
                        // Only create fold region if it spans multiple lines
                        if i > start + 1 {
                            regions.push(FoldRegion {
                                start_line: start,
                                end_line: i,
                                folded: false,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    regions.sort_by_key(|r| r.start_line);
    regions
}

// ── The CodeEditor widget ────────────────────────────────────────────────────

/// The CodeEditor widget.
pub struct CodeEditor {
    id: String,
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
}

impl CodeEditor {
    /// Create a new editor instance.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
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
        }
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Set the entire text content (resets undo, cursor, selection).
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.undo_stack.clear();
        self.bc_version = u64::MAX; // force recompute
        self.bc_dirty_from = Some(0);
        self.fold_version = u64::MAX;
        self.token_cache.clear();
        self.find_replace.matches.clear();
        // Force word-wrap cache recomputation on the next render call.
        // Without this, update_wrap_cache() sees an unchanged version and
        // skips recalculation, leaving a stale single-line layout.
        self.wrap_cached_version = u64::MAX;
        self.wrap_cached_width = 0.0;
    }

    /// Get the entire text content.
    pub fn get_text(&self) -> String {
        self.buffer.text()
    }

    /// Whether the buffer has been modified since last `clear_modified()`.
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Mark buffer as clean (e.g., after save).
    pub fn clear_modified(&mut self) {
        self.buffer.clear_modified();
    }

    /// Set syntax language.
    pub fn set_language(&mut self, lang: Language) {
        self.config.language = lang;
        self.bc_version = u64::MAX;
        self.bc_dirty_from = Some(0);
        self.token_cache.clear();
    }

    /// Set read-only mode.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.config.read_only = read_only;
    }

    /// Whether the editor is read-only.
    pub fn is_read_only(&self) -> bool {
        self.config.read_only
    }

    /// Navigate to a specific line (0-based).
    pub fn goto_line(&mut self, line: usize) {
        self.buffer.goto_line(line);
        self.ensure_cursor_visible();
    }

    /// Set error/warning markers.
    pub fn set_error_markers(&mut self, markers: Vec<LineMarker>) {
        self.error_lines = markers.iter().map(|m| m.line).collect();
        self.error_markers = markers;
    }

    /// Set breakpoints.
    pub fn set_breakpoints(&mut self, bps: Vec<Breakpoint>) {
        self.breakpoint_lines = bps.iter().filter(|bp| bp.enabled).map(|bp| bp.line).collect();
        self.breakpoints = bps;
    }

    /// Get access to the editor configuration.
    pub fn config(&self) -> &EditorConfig {
        &self.config
    }

    /// Get mutable access to the editor configuration.
    pub fn config_mut(&mut self) -> &mut EditorConfig {
        &mut self.config
    }

    /// Current cursor position.
    pub fn cursor(&self) -> CursorPos {
        self.buffer.cursor()
    }

    /// Total line count.
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// Get the word (identifier) under the cursor, if any.
    pub fn word_at_cursor(&self) -> Option<String> {
        let pos = self.buffer.cursor();
        let lines = self.buffer.lines();
        let line = lines.get(pos.line)?;
        let chars: Vec<char> = line.chars().collect();
        if pos.col >= chars.len() {
            return None;
        }
        // Expand left
        let mut start = pos.col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        // Expand right
        let mut end = pos.col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end {
            return None;
        }
        Some(chars[start..end].iter().collect())
    }

    /// Insert text at the current cursor position.
    pub fn insert_text(&mut self, text: &str) {
        self.buffer.insert_text(text);
    }

    /// Delete `n` characters before the cursor (like pressing Backspace n times).
    pub fn delete_chars_before(&mut self, n: usize) {
        for _ in 0..n {
            self.buffer.backspace();
        }
    }

    /// Whether the editor is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Get selected text.
    pub fn selected_text(&self) -> String {
        self.buffer.selected_text()
    }

    /// Open the find panel.
    pub fn open_find(&mut self) {
        self.find_replace.open = true;
        self.find_replace.show_replace = false;
        self.find_replace.just_opened = true;
    }

    /// Open the find & replace panel.
    pub fn open_find_replace(&mut self) {
        self.find_replace.open = true;
        self.find_replace.show_replace = true;
        self.find_replace.just_opened = true;
    }

    /// Current text zoom factor (1.0 = default).
    pub fn text_scale(&self) -> f32 {
        self.config.font_size_scale
    }

    /// Set text zoom factor (clamped to 0.4–4.0).
    pub fn set_text_scale(&mut self, scale: f32) {
        self.config.font_size_scale = scale.clamp(0.4, 4.0);
    }

    /// Close the find panel.
    pub fn close_find(&mut self) {
        self.find_replace.open = false;
    }

    /// Toggle fold at a line.
    pub fn toggle_fold(&mut self, line: usize) {
        for region in &mut self.fold_regions {
            if region.start_line == line {
                region.folded = !region.folded;
                return;
            }
        }
    }

    // ── Render ───────────────────────────────────────────────────────

    /// Render the editor. Call this every frame.
    ///
    /// The editor fills the available content region.
    pub fn render(&mut self, ui: &Ui) {
        // ── Font scale ───────────────────────────────────────────────
        // Push a scaled version of the current font so that char_advance,
        // line_height, and all text rendering use the correct size.
        // SAFETY: igPushFont / igPopFont are paired and always balanced.
        let base_font_size = unsafe { dear_imgui_rs::sys::igGetFontSize() };
        let scaled_font_size = base_font_size * self.config.font_size_scale;
        unsafe {
            dear_imgui_rs::sys::igPushFont(
                code_editor_font_ptr(),
                scaled_font_size,
            );
        }

        // Measure char advance using ImFont::CalcTextSizeA directly — the same
        // API that AddText uses internally.  The high-level igCalcTextSize adds
        // ceil(+0.99999) rounding which inflates char_advance and causes cursor
        // / selection positions to drift away from rendered glyph positions.
        // Using CalcTextSizeA gives the raw floating-point advance.
        self.char_advance = calc_char_advance(scaled_font_size);
        self.line_height = unsafe { dear_imgui_rs::sys::igGetTextLineHeight() } + 2.0;

        // Recompute caches if text changed
        self.update_block_comment_states();
        self.update_fold_regions();
        self.ensure_token_cache_size();

        let fold_extra = if self.config.show_fold_indicators { 2.0 } else { 0.0 };
        let gutter_width = if self.config.show_line_numbers {
            let digits = digit_count(self.buffer.line_count());
            // Layout: | padding | line_numbers | [fold_icon] | gap | code
            (digits as f32 + 1.3 + fold_extra) * self.char_advance
        } else if self.config.show_fold_indicators {
            self.char_advance * 2.0 // minimal gutter for fold arrows only
        } else {
            self.char_advance * 0.5 // tiny left margin, no gutter content
        };

        // ── Find/Replace bar at the TOP (before the editor child window) ──
        if self.find_replace.open {
            self.render_find_replace_bar(ui);
        }

        let avail = ui.content_region_avail();
        let child_id = format!("##ce_{}", self.id);

        // Push style for the editor region (uses theme's editor_bg)
        let _bg_token = ui.push_style_color(
            StyleColor::ChildBg,
            self.config.colors.editor_bg,
        );

        // When word-wrap is active there is no horizontal content overflow,
        // so suppress the horizontal scrollbar entirely.  Keeping it visible
        // when wrapping would (a) waste vertical space and (b) shrink
        // inner_size[1], causing the last visible line to be clipped.
        let child_flags = if self.config.word_wrap {
            WindowFlags::NO_MOVE | WindowFlags::NO_SCROLL_WITH_MOUSE
        } else {
            WindowFlags::HORIZONTAL_SCROLLBAR | WindowFlags::NO_MOVE | WindowFlags::NO_SCROLL_WITH_MOUSE
        };

        ui.child_window(&child_id)
            .size(avail)
            .flags(child_flags)
            .build(ui, || {
            self.focused = ui.is_window_focused();

            // ── Keyboard layout switching on focus change ────────────
            self.handle_input_locale_switch();

            // Inner window size — the actual visible area of the child
            // window (accounts for scrollbar, border, padding).
            let inner_size = ui.window_size();
            self.visible_height = inner_size[1];

            // ── Word wrap cache ───────────────────────────────────────
            let text_area_w = inner_size[0] - gutter_width;
            self.update_wrap_cache(text_area_w);

            // ── Read ImGui scroll state first ───────────────────────
            // This is the source of truth — user may have dragged the
            // scrollbar or ImGui processed wheel events.
            let imgui_scroll_y = ui.scroll_y();
            self.scroll_x = ui.scroll_x();

            // Detect external scroll change (scrollbar drag) — if ImGui's
            // scroll differs from what we wrote last frame, the user moved
            // the scrollbar directly.  Adopt that position as the new target
            // so smooth-scroll doesn't fight the scrollbar.
            let external_scroll = (imgui_scroll_y - self.last_set_scroll_y).abs() > 0.5;
            self.scroll_y = imgui_scroll_y;
            if external_scroll {
                self.target_scroll_y = imgui_scroll_y;
            }

            // Update cursor blink
            let dt = ui.io().delta_time();
            self.update_blink(dt);

            // Smooth scrolling (modifies self.scroll_y toward target)
            self.update_smooth_scroll(dt);

            // Handle input (may call ensure_cursor_visible → self.scroll_y)
            if self.focused {
                self.handle_keyboard(ui);
            }
            self.handle_mouse(ui, gutter_width, inner_size);

            // Re-sync wrap cache after input — paste/Enter may have added
            // lines, so the pre-input cache is stale.  Then re-run
            // ensure_cursor_visible with the fresh row counts so the
            // scrollbar target covers the full document.
            self.update_wrap_cache(text_area_w);
            self.ensure_cursor_visible();

            // ── Sync scroll back to ImGui ───────────────────────────
            // Input handling may have updated self.scroll_y (e.g.
            // ensure_cursor_visible, smooth scroll, mouse wheel).
            // Apply it so ImGui's scrollbar and cursor_screen_pos
            // reflect the new state.
            ui.set_scroll_y(self.scroll_y);
            self.last_set_scroll_y = self.scroll_y;

            let draw_list = ui.get_window_draw_list();
            // cursor_screen_pos() includes the scroll offset (ImGui's
            // DC.CursorPos = Pos + Pad − Scroll).  So `win_x`/`win_y`
            // already have −scroll baked in — content drawn at
            // `win_y + line*h` lands at the correct screen position and
            // is automatically clipped by the child window.
            let [win_x, win_y] = ui.cursor_screen_pos();
            let scroll_y = ui.scroll_y();
            let scroll_x = ui.scroll_x();
            self.scroll_x = scroll_x;
            self.scroll_y = scroll_y;

            // Scroll-independent origin: the fixed top-left of the
            // content area in screen space.  Used for UI elements that
            // must NOT scroll (gutter X position).
            let origin_x = win_x + scroll_x;
            let origin_y = win_y + scroll_y;

            // first/last visible: in VISUAL ROW space when wrapping.
            let first_vrow = (scroll_y / self.line_height) as usize;
            let visible_count = (self.visible_height / self.line_height) as usize + 2;
            let last_vrow = first_vrow + visible_count;

            // Map visual rows back to buffer lines for the rendering loop.
            let (first_visible, _) = self.visual_row_to_line(first_vrow);
            let (last_vis_line, _) = self.visual_row_to_line(last_vrow);
            let last_visible = (last_vis_line + 1).min(self.buffer.line_count());

            // text_start_x: scrolls horizontally with content.
            // win_x already contains −scroll_x, so no extra subtraction.
            let text_start_x = win_x + gutter_width;
            let cursor_pos = self.buffer.cursor();
            let selection = self.buffer.selection();
            let matching_bracket = if self.config.bracket_matching {
                self.buffer.find_matching_bracket()
            } else {
                None
            };

            // ── Build visible line list (respecting folds) ──────────
            let visible_lines = self.build_visible_lines(first_visible, last_visible);
            let wrapping = self.config.word_wrap;

            // Pre-populate token cache for all visible lines so the draw
            // loop doesn't need &mut self (avoids per-line to_string() alloc).
            for &(line_idx, _) in &visible_lines {
                self.get_cached_tokens(line_idx);
            }

            // ── Draw lines (batched) ────────────────────────────────
            for &(line_idx, _screen_row) in &visible_lines {
                let line_str = self.buffer.line(line_idx);

                // How many visual sub-rows does this line occupy?
                let sub_row_count = if wrapping && line_idx < self.wrap_cols.len() {
                    self.wrap_cols[line_idx].len() + 1
                } else {
                    1
                };

                for sub_row in 0..sub_row_count {
                    let vrow = if wrapping {
                        self.visual_row_of(line_idx, if sub_row == 0 { 0 }
                            else { self.wrap_cols[line_idx][sub_row - 1] })
                    } else {
                        line_idx
                    };
                    let y = win_y + (vrow as f32) * self.line_height;

                    // Column range for this sub-row
                    let (col_start, col_end) = if wrapping {
                        self.sub_row_col_range(line_idx, sub_row)
                    } else {
                        (0, line_str.chars().count())
                    };

                    // ── Per-line decorations (only on first sub-row) ──
                    if sub_row == 0 {
                        // Current line highlight — drawn BEFORE selection so
                        // the selection overlay is visible on top.  Also skip
                        // when there is an active selection touching this line
                        // so the selection color isn't washed out.
                        let sel_on_line = selection.is_some_and(|s| {
                            let (a, b) = s.ordered();
                            line_idx >= a.line && line_idx <= b.line
                                && !(a.line == b.line && a.col == b.col)
                        });
                        if self.config.highlight_current_line
                            && line_idx == cursor_pos.line
                            && self.focused
                            && !sel_on_line
                        {
                            let num_rows = sub_row_count as f32;
                            draw_list.add_rect(
                                [origin_x, y],
                                [origin_x + inner_size[0], y + self.line_height * num_rows],
                                col32(self.config.colors.current_line_bg),
                            ).filled(true).build();
                        }

                        // Error marker background
                        if self.error_lines.contains(&line_idx) {
                            draw_list.add_rect(
                                [origin_x, y],
                                [origin_x + inner_size[0], y + self.line_height],
                                col32([0.80, 0.20, 0.20, 0.15]),
                            ).filled(true).build();
                        }

                        // Breakpoint marker in gutter
                        if self.breakpoint_lines.contains(&line_idx) {
                            let center = [origin_x + gutter_width * 0.2, y + self.line_height * 0.5];
                            let radius = self.line_height * 0.3;
                            draw_list.add_circle(center, radius, col32(crate::theme::DANGER))
                                .filled(true)
                                .build();
                        }

                        // Fold indicator in gutter
                        if self.config.show_fold_indicators {
                            self.draw_fold_indicator(&draw_list, line_idx, origin_x, gutter_width, y);
                        }

                        // Line number
                        if self.config.show_line_numbers {
                            let num_str = format!("{}", line_idx + 1);
                            let num_color = if line_idx == cursor_pos.line {
                                self.config.colors.line_number_active
                            } else {
                                self.config.colors.line_number
                            };
                            let right_pad = if self.config.show_fold_indicators { 2.5 } else { 0.5 };
                            let num_x = origin_x + gutter_width
                                - (num_str.len() as f32 + right_pad) * self.char_advance;
                            draw_list.add_text([num_x, y], col32(num_color), &num_str);
                        }
                    }

                    // ── Selection & find highlights (every sub-row) ──
                    // Drawn AFTER current-line-bg so the selection is on top.
                    if let Some(sel) = selection {
                        self.draw_selection(&draw_list, sel, line_idx, line_str,
                                           text_start_x, y, col_start, col_end);
                    }
                    for sel in self.buffer.extra_selections().iter().filter_map(|s| s.as_ref()) {
                        self.draw_selection(&draw_list, *sel, line_idx, line_str,
                                           text_start_x, y, col_start, col_end);
                    }
                    self.draw_find_matches(&draw_list, line_idx, line_str,
                                           text_start_x, y, col_start, col_end);

                    // Gutter separator line (every sub-row)
                    draw_list.add_line(
                        [origin_x + gutter_width - self.char_advance * 0.5, y],
                        [origin_x + gutter_width - self.char_advance * 0.5,
                         y + self.line_height],
                        col32(crate::theme::SEPARATOR),
                    ).build();

                    // ── Tokenized text ───────────────────────────────
                    if !wrapping || sub_row_count == 1 {
                        // No wrapping — draw full line as before.
                        let tokens = self.cached_tokens(line_idx);
                        self.draw_tokens_batched(
                            &draw_list, &tokens, line_str, text_start_x, y,
                        );
                    } else {
                        // Word wrap: draw only the columns for this sub-row.
                        let tokens = self.cached_tokens(line_idx);
                        self.draw_tokens_batched_range(
                            &draw_list, &tokens, line_str,
                            text_start_x, y, col_start, col_end,
                        );
                    }

                    if sub_row == 0 && self.config.show_color_swatches {
                        self.draw_hex_color_swatches(&draw_list, line_str, text_start_x, y);
                    }

                    // Bracket match highlight (check all sub-rows)
                    if let Some(match_pos) = matching_bracket {
                        let col_start_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
                        // Highlight the matched bracket
                        if match_pos.line == line_idx
                            && match_pos.col >= col_start && match_pos.col < col_end
                        {
                            let bx = text_start_x
                                + col_to_x(line_str, match_pos.col, self.char_advance, self.config.tab_size)
                                - col_start_x;
                            draw_list.add_rect(
                                [bx, y],
                                [bx + self.char_advance, y + self.line_height],
                                col32(self.config.colors.bracket_match_bg),
                            ).filled(true).build();
                        }
                        // Highlight the cursor bracket
                        if cursor_pos.line == line_idx
                            && cursor_pos.col >= col_start && cursor_pos.col < col_end
                        {
                            let bx = text_start_x
                                + col_to_x(line_str, cursor_pos.col, self.char_advance, self.config.tab_size)
                                - col_start_x;
                            draw_list.add_rect(
                                [bx, y],
                                [bx + self.char_advance, y + self.line_height],
                                col32(self.config.colors.bracket_match_bg),
                            ).filled(true).build();
                        }
                    }
                }
            }

            // ── Cursor (primary + extras) ──────────────────────────
            if self.focused && self.cursor_visible && !self.config.read_only {
                let cursor_vrow = self.visual_row_of(cursor_pos.line, cursor_pos.col);
                let (col_start, _) = if wrapping {
                    let (_, sub) = self.visual_row_to_line(cursor_vrow);
                    self.sub_row_col_range(cursor_pos.line, sub)
                } else {
                    (0usize, 0usize)
                };
                let cursor_line_str = self.buffer.line(cursor_pos.line);
                let cx = text_start_x
                    + col_to_x(cursor_line_str, cursor_pos.col, self.char_advance, self.config.tab_size)
                    - col_to_x(cursor_line_str, col_start, self.char_advance, self.config.tab_size)
                    - 1.0;
                let cy = win_y + cursor_vrow as f32 * self.line_height;
                draw_list.add_line(
                    [cx, cy],
                    [cx, cy + self.line_height],
                    col32(crate::theme::TEXT_PRIMARY),
                ).thickness(1.5).build();

                // Draw extra cursors
                for extra in self.buffer.extra_cursors() {
                    let ev = self.visual_row_of(extra.line, extra.col);
                    let extra_line_str = self.buffer.line(extra.line);
                    let extra_col_start = if wrapping {
                        let (_, esub) = self.visual_row_to_line(ev);
                        self.sub_row_col_range(extra.line, esub).0
                    } else { 0 };
                    let ex = text_start_x
                        + col_to_x(extra_line_str, extra.col, self.char_advance, self.config.tab_size)
                        - col_to_x(extra_line_str, extra_col_start, self.char_advance, self.config.tab_size)
                        - 1.0;
                    let ey = win_y + ev as f32 * self.line_height;
                    if ey >= origin_y - self.line_height && ey <= origin_y + inner_size[1] {
                        draw_list.add_line(
                            [ex, ey],
                            [ex, ey + self.line_height],
                            col32([0.6, 0.8, 1.0, 0.85]),
                        ).thickness(1.5).build();
                    }
                }
            }

            // ── Error marker tooltips ───────────────────────────────
            if ui.is_window_hovered() {
                let [_mx, my] = ui.io().mouse_pos();
                let hover_vrow = ((my - win_y) / self.line_height).max(0.0) as usize;
                let (hover_line, _) = self.visual_row_to_line(hover_vrow);
                for marker in &self.error_markers {
                    if marker.line == hover_line {
                        ui.tooltip_text(&marker.message);
                        break;
                    }
                }
            }

            // Set dummy size for scrolling.
            // The dummy must extend the content region so ImGui's scrollbar
            // covers the full document.  Height = all visual rows + a small
            // bottom margin so the last line is never clipped.
            let total_height =
                self.total_visual_rows() as f32 * self.line_height
                + self.line_height; // extra row of padding at bottom
            let total_width = if wrapping {
                inner_size[0]
            } else {
                let max_line_len = (first_visible..last_visible)
                    .map(|i| self.buffer.line(i).chars().count())
                    .max()
                    .unwrap_or(80);
                gutter_width + (max_line_len as f32 + 10.0) * self.char_advance
            };
            // Place cursor at the very end of the content area and emit
            // a 1px-tall dummy so ImGui registers the full scroll extent.
            ui.set_cursor_pos([0.0, total_height]);
            ui.dummy([total_width, 1.0]);

            // ── Right-click context menu ─────────────────────────────
            self.render_context_menu(ui);
        });

        // ── Pop the font pushed at the start of render() ─────────────
        // SAFETY: balances the igPushFont call at the top of this function.
        unsafe { dear_imgui_rs::sys::igPopFont(); }
    }

    // ── Input handling ───────────────────────────────────────────────

    fn handle_keyboard(&mut self, ui: &Ui) {
        let io = ui.io();
        let ctrl = io.key_ctrl();
        let shift = io.key_shift();
        let alt = io.key_alt();

        // ── Navigation keys ─────────────────────────────────────────

        // Helper macro for nav keys that do NOT collapse selection (Up, Down,
        // Home, End, word movement, doc start/end).
        macro_rules! nav_key {
            ($key:ident, $action:expr) => {
                if ui.is_key_pressed(Key::$key) {
                    let anchor = if shift {
                        Some(self.buffer.selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor))
                    } else {
                        None
                    };
                    if !shift { self.buffer.clear_selection(); }
                    $action;
                    if let Some(a) = anchor {
                        self.buffer.set_selection(a, self.buffer.cursor());
                    }
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            };
        }

        // Left/Right arrows: collapse selection to start/end when selection
        // is active and Shift is NOT held (standard editor behaviour).
        // Without this, pressing Left with a selection would clear_selection
        // then move_left, landing one char *before* the selection start.
        macro_rules! nav_lr {
            ($key:ident, $move_action:expr, $collapse_end:expr) => {
                if ui.is_key_pressed(Key::$key) {
                    if shift {
                        let anchor = self.buffer.selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor);
                        $move_action;
                        self.buffer.set_selection(anchor, self.buffer.cursor());
                    } else if let Some(sel) = self.buffer.selection().filter(|s| !s.is_empty()) {
                        // Collapse to the appropriate end of the selection
                        let (start, end) = sel.ordered();
                        let target = $collapse_end(start, end);
                        self.buffer.set_cursor_clear_sel(target);
                    } else {
                        self.buffer.clear_selection();
                        $move_action;
                    }
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            };
        }

        if ctrl {
            nav_lr!(LeftArrow, self.buffer.move_word_left(), |start: CursorPos, _end: CursorPos| start);
            nav_lr!(RightArrow, self.buffer.move_word_right(), |_start: CursorPos, end: CursorPos| end);
            nav_key!(Home, self.buffer.move_doc_start());
            nav_key!(End, self.buffer.move_doc_end());
        } else {
            nav_lr!(LeftArrow, self.buffer.move_left(), |start: CursorPos, _end: CursorPos| start);
            nav_lr!(RightArrow, self.buffer.move_right(), |_start: CursorPos, end: CursorPos| end);
            if !alt {
                nav_key!(UpArrow, self.buffer.move_up());
                nav_key!(DownArrow, self.buffer.move_down());
            }
            nav_key!(Home, self.buffer.move_home());
            nav_key!(End, self.buffer.move_end());
        }

        // PageUp / PageDown
        for (key, sign) in [(Key::PageUp, -1isize), (Key::PageDown, 1isize)] {
            if ui.is_key_pressed(key) {
                let lines = sign * self.visible_lines() as isize;
                let anchor = if shift {
                    Some(self.buffer.selection()
                        .map_or(self.buffer.cursor(), |s| s.anchor))
                } else {
                    None
                };
                if !shift { self.buffer.clear_selection(); }
                self.buffer.move_page(lines);
                if let Some(a) = anchor {
                    self.buffer.set_selection(a, self.buffer.cursor());
                }
                self.reset_blink();
                self.ensure_cursor_visible();
            }
        }

        // ── Ctrl shortcuts ──────────────────────────────────────────
        if ctrl && ui.is_key_pressed(Key::A) {
            self.buffer.select_all();
            return;
        }

        if ctrl && ui.is_key_pressed(Key::C) {
            let text = self.buffer.selected_text();
            if !text.is_empty() {
                set_clipboard(&text);
            }
            return;
        }

        if ctrl && ui.is_key_pressed(Key::X) && !self.config.read_only {
            let text = self.buffer.selected_text();
            if !text.is_empty() {
                set_clipboard(&text);
                self.snapshot_undo(true);
                self.buffer.backspace();
                self.invalidate_token_cache_from(self.buffer.cursor().line);
                self.reset_blink();
            }
            return;
        }

        if ctrl && ui.is_key_pressed(Key::V) && !self.config.read_only {
            if let Some(clip) = get_clipboard()
                && !clip.is_empty()
            {
                // Truncate pasted text to respect max_lines / max_line_length.
                let clip = self.truncate_paste(&clip);
                if !clip.is_empty() {
                    self.snapshot_undo(true);
                    let paste_line = self.buffer.cursor().line;
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_from(paste_line);
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            }
            return;
        }

        if ctrl && ui.is_key_pressed(Key::Z) && !self.config.read_only {
            self.undo();
            return;
        }

        if ctrl && ui.is_key_pressed(Key::Y) && !self.config.read_only {
            self.redo();
            return;
        }

        // ── Find/Replace shortcuts ──────────────────────────────────
        if ctrl && ui.is_key_pressed(Key::F) {
            // Pre-fill with selection if any
            let sel = self.buffer.selected_text();
            if !sel.is_empty() && !sel.contains('\n') {
                self.find_replace.query = sel;
            }
            self.find_replace.open = true;
            self.find_replace.show_replace = false;
            self.find_replace.just_opened = true;
            self.update_find_matches();
            return;
        }

        if ctrl && ui.is_key_pressed(Key::H) && !self.config.read_only {
            let sel = self.buffer.selected_text();
            if !sel.is_empty() && !sel.contains('\n') {
                self.find_replace.query = sel;
            }
            self.find_replace.open = true;
            self.find_replace.show_replace = true;
            self.find_replace.just_opened = true;
            self.update_find_matches();
            return;
        }

        // Escape closes find panel
        if ui.is_key_pressed(Key::Escape) && self.find_replace.open {
            self.find_replace.open = false;
            return;
        }

        // F3 / Ctrl+G: next match;  Shift+F3: previous match
        if shift && ui.is_key_pressed(Key::F3) {
            self.find_prev();
            return;
        }
        if ui.is_key_pressed(Key::F3) || (ctrl && ui.is_key_pressed(Key::G)) {
            self.find_next();
            return;
        }

        // ── Comment toggling (Ctrl+/) ───────────────────────────────
        if ctrl && ui.is_key_pressed(Key::Slash) && !self.config.read_only {
            self.snapshot_undo(true);
            let (start, end) = if let Some(sel) = self.buffer.selection() {
                let (s, e) = sel.ordered();
                (s.line, e.line)
            } else {
                let l = self.buffer.cursor().line;
                (l, l)
            };
            self.buffer.toggle_line_comment(start..end + 1);
            self.invalidate_token_cache_all();
            return;
        }

        // ── Line operations ─────────────────────────────────────────
        if !self.config.read_only {
            // Alt+Up: move line up
            if alt && ui.is_key_pressed(Key::UpArrow) {
                self.snapshot_undo(true);
                self.buffer.move_line_up();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Alt+Down: move line down
            if alt && ui.is_key_pressed(Key::DownArrow) {
                self.snapshot_undo(true);
                self.buffer.move_line_down();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+Shift+D: duplicate line
            if ctrl && shift && ui.is_key_pressed(Key::D) {
                self.snapshot_undo(true);
                self.buffer.duplicate_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+Shift+K: delete line
            if ctrl && shift && ui.is_key_pressed(Key::K) {
                self.snapshot_undo(true);
                self.buffer.delete_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                return;
            }

            // Ctrl+D: select next occurrence (add cursor)
            if ctrl && !shift && ui.is_key_pressed(Key::D) {
                // Get current word under cursor or selected text
                let needle = {
                    let sel_text = self.buffer.selected_text();
                    if sel_text.is_empty() {
                        // Select word under cursor first
                        self.buffer.select_word_at_cursor();
                        self.buffer.selected_text()
                    } else {
                        sel_text
                    }
                };

                if !needle.is_empty() {
                    // Find next occurrence after the last cursor
                    let all = self.buffer.all_cursors_sorted();
                    let search_from = all.last().copied()
                        .unwrap_or(self.buffer.cursor());
                    if let Some((start, end)) = self.buffer
                        .find_next_occurrence(&needle, search_from)
                    {
                        let sel = Selection { anchor: start, cursor: end };
                        self.buffer.add_cursor_with_selection(end, sel);
                    }
                }
                self.reset_blink();
                return;
            }

            // Escape: clear extra cursors (if any) before other Escape behavior
            if ui.is_key_pressed(Key::Escape) && self.buffer.has_extra_cursors() {
                self.buffer.clear_extra_cursors();
                return;
            }
        }

        // ── Editing keys ────────────────────────────────────────────
        if !self.config.read_only {
            if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::KeypadEnter) {
                // Enforce max_lines limit
                if self.config.max_lines > 0
                    && self.buffer.line_count() >= self.config.max_lines
                {
                    return;
                }
                self.snapshot_undo(true);
                let split_line = self.buffer.cursor().line;
                self.buffer.insert_newline(
                    self.config.auto_indent,
                    self.config.tab_size,
                );
                self.invalidate_token_cache_from(split_line);
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Backspace) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_backspace();
                    self.invalidate_token_cache_all();
                } else if ctrl {
                    self.buffer.delete_word_left();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                } else {
                    self.buffer.backspace();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                }
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Delete) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_delete();
                    self.invalidate_token_cache_all();
                } else if ctrl {
                    self.buffer.delete_word_right();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                } else {
                    self.buffer.delete();
                    self.invalidate_token_cache_from(self.buffer.cursor().line);
                }
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Tab) {
                if let Some(sel) = self.buffer.selection() {
                    let (start, end) = sel.ordered();
                    if start.line != end.line {
                        self.snapshot_undo(true);
                        if shift {
                            self.buffer.unindent_lines(
                                start.line..end.line + 1,
                                self.config.tab_size,
                            );
                        } else {
                            self.buffer.indent_lines(
                                start.line..end.line + 1,
                                self.config.tab_size,
                                self.config.insert_spaces,
                            );
                        }
                        self.invalidate_token_cache_from(start.line);
                        return;
                    }
                }
                // Single-line tab insert
                self.snapshot_undo(false);
                if self.config.insert_spaces {
                    let cur_col = self.buffer.cursor().col;
                    let tab = self.config.tab_size as usize;
                    let spaces = tab - (cur_col % tab);
                    self.buffer.insert_text(&" ".repeat(spaces));
                } else {
                    self.buffer.insert_char('\t');
                }
                self.invalidate_token_cache_at(self.buffer.cursor().line);
                self.reset_blink();
                return;
            }

            // ── Text input (typed characters) ───────────────────────
            let input_chars = read_input_chars();
            for raw_ch in input_chars {
                if raw_ch < ' ' || raw_ch == '\x7f' { continue; }

                // Enforce max_line_length limit
                if self.config.max_line_length > 0 {
                    let cur = self.buffer.cursor();
                    let line_len = self.buffer.line(cur.line).chars().count();
                    if line_len >= self.config.max_line_length {
                        continue;
                    }
                }

                // ── Hex input transforms ─────────────────────────────
                let ch = if self.config.hex_auto_uppercase
                    && raw_ch.is_ascii_hexdigit()
                {
                    raw_ch.to_ascii_uppercase()
                } else {
                    raw_ch
                };

                // ── Auto-skip: check BEFORE inserting ────────────────
                // If the typed character is a closing bracket or quote and
                // the character at the cursor is the same, just skip past
                // it instead of inserting a duplicate.
                let is_closing = is_closing_bracket(ch)
                    || QUOTE_PAIRS.iter().any(|(_, c)| *c == ch);
                if is_closing {
                    let line = self.buffer.line(self.buffer.cursor().line);
                    let col = self.buffer.cursor().col;
                    let next_ch = line.chars().nth(col);
                    if next_ch == Some(ch) {
                        self.buffer.move_right();
                        self.reset_blink();
                        continue; // skip normal insert + auto-close
                    }
                }

                // ── Normal insert ────────────────────────────────────
                self.snapshot_undo(false);
                if self.buffer.has_extra_cursors() {
                    self.buffer.multi_insert_char(ch);
                    self.invalidate_token_cache_all();
                } else {
                    self.buffer.insert_char(ch);
                    self.invalidate_token_cache_at(self.buffer.cursor().line);

                    // Auto-space: after 2 consecutive hex digits insert a space
                    if self.config.hex_auto_space && ch.is_ascii_hexdigit() {
                        let line_idx = self.buffer.cursor().line;
                        let line = self.buffer.line(line_idx).to_string();
                        let col = self.buffer.cursor().col;
                        let before: String = line.chars().take(col).collect();
                        let nibbles_before: usize = before
                            .chars()
                            .rev()
                            .take_while(|c| c.is_ascii_hexdigit())
                            .count();
                        if nibbles_before == 2 {
                            let next_is_space = line.chars().nth(col)
                                .is_none_or(|c| c == ' ' || c == '\t');
                            if next_is_space {
                                self.buffer.insert_char(' ');
                                self.invalidate_token_cache_at(line_idx);
                            }
                        }
                    }
                }

                // ── Auto-close brackets ──────────────────────────────
                if self.config.auto_close_brackets
                    && let Some(close) = closing_bracket(ch)
                {
                    self.buffer.insert_char(close);
                    self.buffer.move_left();
                }

                // ── Auto-close quotes ────────────────────────────────
                if self.config.auto_close_quotes
                    && let Some(close) = closing_quote(ch)
                {
                    let line = self.buffer.line(self.buffer.cursor().line);
                    let col = self.buffer.cursor().col;
                    // Don't auto-close if preceded by a backslash (escape)
                    let is_escaped = col >= 2
                        && line.chars().nth(col - 2) == Some('\\');
                    if !is_escaped {
                        self.buffer.insert_char(close);
                        self.buffer.move_left();
                    }
                }

                self.reset_blink();
            }
        }
    }

    fn handle_mouse(&mut self, ui: &Ui, gutter_width: f32, inner_size: [f32; 2]) {
        if !ui.is_window_hovered() { return; }

        let io = ui.io();
        let [mx, my] = io.mouse_pos();
        // cursor_screen_pos includes −scroll, so it's the scroll-adjusted
        // origin.  origin_* compensates back to the fixed window position.
        let [win_x, win_y] = ui.cursor_screen_pos();
        let scroll_x = ui.scroll_x();
        let scroll_y = ui.scroll_y();
        let origin_x = win_x + scroll_x;
        let origin_y = win_y + scroll_y;
        let text_x = win_x + gutter_width;

        // ── Ctrl+Scroll zoom ──────────────────────────────────────────────
        if io.key_ctrl() && io.mouse_wheel() != 0.0 {
            self.config.font_size_scale =
                (self.config.font_size_scale + io.mouse_wheel() * 0.1).clamp(0.4, 4.0);
        }

        // ── I-beam cursor ONLY inside the text content area ───────────────
        let content_max_x = origin_x + inner_size[0];
        let content_max_y = origin_y + inner_size[1];
        if mx >= origin_x + gutter_width && mx < content_max_x
            && my >= origin_y && my < content_max_y
        {
            // SAFETY: igSetMouseCursor is a standard ImGui call.
            unsafe {
                dear_imgui_rs::sys::igSetMouseCursor(
                    dear_imgui_rs::sys::ImGuiMouseCursor_TextInput,
                );
            }
        }

        // Convert mouse position to text position.
        // win_y already includes −scroll_y, so (my − win_y) / h gives
        // the visual row directly.
        let vrow = ((my - win_y) / self.line_height).max(0.0) as usize;
        let (line, sub_row) = self.visual_row_to_line(vrow);
        let line = line.min(self.buffer.line_count().saturating_sub(1));
        let line_content = self.buffer.line(line).to_string();

        // For wrapped sub-rows, map into the column range.
        let (col_start, col_end) = self.sub_row_col_range(line, sub_row);
        let sub_str: String = line_content.chars()
            .skip(col_start).take(col_end - col_start).collect();

        let raw_col = col_start + x_to_col(
            &sub_str,
            (mx - text_x).max(0.0),
            self.char_advance,
            self.config.tab_size,
        );
        let col = raw_col.min(line_content.chars().count());
        let click_pos = CursorPos::new(line, col);

        let time = ui.time();

        // Click in gutter area → toggle fold
        if self.config.show_fold_indicators
            && ui.is_mouse_clicked(MouseButton::Left)
            && mx < text_x
        {
            let has_fold = self.fold_regions.iter()
                .any(|r| r.start_line == line);
            if has_fold {
                self.toggle_fold(line);
                return;
            }
        }

        if ui.is_mouse_clicked(MouseButton::Left) {
            // Alt+Click: add extra cursor at click position
            if ui.io().key_alt() && !self.config.read_only {
                self.buffer.add_cursor(click_pos);
                self.reset_blink();
                return;
            }

            // Any non-Alt click clears extra cursors
            if self.buffer.has_extra_cursors() {
                self.buffer.clear_extra_cursors();
            }

            // Detect double/triple click
            if time - self.last_click_time < 0.4
                && self.last_click_pos == click_pos
            {
                self.click_count = (self.click_count + 1).min(3);
            } else {
                self.click_count = 1;
            }
            self.last_click_time = time;
            self.last_click_pos = click_pos;

            match self.click_count {
                1 => {
                    if ui.io().key_shift() {
                        let anchor = self.buffer.selection()
                            .map_or(self.buffer.cursor(), |s| s.anchor);
                        self.buffer.set_selection(anchor, click_pos);
                    } else {
                        self.buffer.set_cursor_clear_sel(click_pos);
                    }
                    self.mouse_selecting = true;
                }
                2 => {
                    self.buffer.set_cursor(click_pos);
                    self.buffer.select_word_at_cursor();
                }
                3 => {
                    self.buffer.set_cursor(click_pos);
                    self.buffer.select_line();
                }
                _ => {}
            }
            self.reset_blink();
        }

        if ui.is_mouse_dragging(MouseButton::Left) && self.mouse_selecting {
            let anchor = self.buffer.selection()
                .map_or(self.buffer.cursor(), |s| s.anchor);
            self.buffer.set_selection(anchor, click_pos);
        }

        if ui.is_mouse_released(MouseButton::Left) {
            self.mouse_selecting = false;
        }

        // ── Right-click → context menu ────────────────────────────────────
        if ui.is_mouse_clicked(MouseButton::Right) {
            // Move cursor to click position if nothing is selected
            if self.buffer.selection().is_none() {
                self.buffer.set_cursor_clear_sel(click_pos);
                self.reset_blink();
            }
            if self.config.context_menu.enabled {
                ui.open_popup("##editor_ctx");
            }
        }

        // Scroll with mouse wheel (smooth) — suppressed when Ctrl is held (zoom mode)
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 && !io.key_ctrl() {
            let delta = -wheel * self.config.scroll_speed * self.line_height;
            // Use total VISUAL rows (word-wrap aware) — not buffer.line_count().
            // With wrap on, one text line can produce N visual rows, so clamping
            // to line_count instead of total_visual_rows() caps wheel scroll at
            // a tiny fraction of the actual document height and the scrollbar
            // appears stuck. Keep a one-row bottom margin for consistency with
            // the total_height used by the dummy element (see render()).
            let max_scroll =
                (self.total_visual_rows() as f32 * self.line_height).max(0.0);
            if self.config.smooth_scrolling {
                self.target_scroll_y =
                    (self.target_scroll_y + delta).clamp(0.0, max_scroll);
            } else {
                self.scroll_y =
                    (self.scroll_y + delta).clamp(0.0, max_scroll);
                self.target_scroll_y = self.scroll_y;
                ui.set_scroll_y(self.scroll_y);
            }
        }
    }

    // ── Drawing helpers ─────────────────────────────────────────────

    /// Draw tokens using batched draw calls — consecutive tokens of the same
    /// color are merged into a single `AddText()` call.
    fn draw_tokens_batched(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        tokens: &[Token],
        line_str: &str,
        text_start_x: f32,
        y: f32,
    ) {
        if tokens.is_empty() { return; }

        let mut batch_start_x = text_start_x;
        let mut batch_color = self.token_color(tokens[0].kind);
        let mut batch_text = String::with_capacity(64);
        let mut x = text_start_x;

        for tok in tokens {
            let byte_end = (tok.start + tok.len).min(line_str.len());
            // Guard: skip any token whose byte range doesn't sit on UTF-8 char
            // boundaries (can happen with multi-byte chars in the fallback path).
            if !line_str.is_char_boundary(tok.start)
                || !line_str.is_char_boundary(byte_end)
            {
                // Advance x approximately so subsequent tokens stay aligned.
                x += tok.len as f32 * self.char_advance;
                continue;
            }
            let text = &line_str[tok.start..byte_end];
            let color = self.token_color(tok.kind);

            if tok.kind == TokenKind::Whitespace {
                // Flush current batch before whitespace
                if !batch_text.is_empty() {
                    draw_list.add_text(
                        [batch_start_x, y],
                        col32(batch_color),
                        &batch_text,
                    );
                    batch_text.clear();
                }

                if self.config.show_whitespace {
                    for ch in text.chars() {
                        let ch_w = if ch == '\t' {
                            self.char_advance * self.config.tab_size as f32
                        } else {
                            self.char_advance
                        };
                        if ch == ' ' {
                            let cx = x + ch_w * 0.5;
                            let cy = y + self.line_height * 0.5;
                            draw_list.add_circle(
                                [cx, cy], 1.0,
                                col32(crate::theme::TEXT_MUTED),
                            ).filled(true).build();
                        } else if ch == '\t' {
                            let arrow_y = y + self.line_height * 0.5;
                            draw_list.add_line(
                                [x + 2.0, arrow_y],
                                [x + ch_w - 2.0, arrow_y],
                                col32(crate::theme::TEXT_MUTED),
                            ).build();
                        }
                        x += ch_w;
                    }
                } else {
                    // Account for tab character width even when not drawing
                    // whitespace markers (tabs are wider than regular chars).
                    for ch in text.chars() {
                        if ch == '\t' {
                            x += self.char_advance * self.config.tab_size as f32;
                        } else {
                            x += self.char_advance;
                        }
                    }
                }

                batch_start_x = x;
                batch_color = color;
                continue;
            }

            // Same color → extend batch
            if color == batch_color && !batch_text.is_empty() {
                batch_text.push_str(text);
                x += text.chars().count() as f32 * self.char_advance;
                continue;
            }

            // Different color → flush and start new batch
            if !batch_text.is_empty() {
                draw_list.add_text(
                    [batch_start_x, y],
                    col32(batch_color),
                    &batch_text,
                );
            }

            batch_text.clear();
            batch_text.push_str(text);
            batch_start_x = x;
            batch_color = color;
            x += text.chars().count() as f32 * self.char_advance;
        }

        // Flush final batch
        if !batch_text.is_empty() {
            draw_list.add_text(
                [batch_start_x, y],
                col32(batch_color),
                &batch_text,
            );
        }
    }

    /// Draw tokens for a sub-range of columns (used by word wrap).
    ///
    /// Only characters in `col_start..col_end` are drawn, positioned
    /// starting at `text_start_x`.
    #[allow(clippy::too_many_arguments)]
    fn draw_tokens_batched_range(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        tokens: &[Token],
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        if tokens.is_empty() { return; }

        // Build a char→byte mapping for the range.
        let chars: Vec<(usize, char)> = line_str.char_indices().collect();
        let byte_start = chars.get(col_start).map_or(line_str.len(), |&(b, _)| b);
        let byte_end = chars.get(col_end).map_or(line_str.len(), |&(b, _)| b);

        let mut x = text_start_x;
        let mut batch_start_x = x;
        let mut batch_color = [0.0f32; 4];
        let mut batch_text = String::with_capacity(64);
        let mut first_batch = true;

        for tok in tokens {
            let tok_byte_end = (tok.start + tok.len).min(line_str.len());
            // Skip tokens entirely outside our column range.
            if tok_byte_end <= byte_start || tok.start >= byte_end { continue; }
            // Clip token to our range.
            let clip_start = tok.start.max(byte_start);
            let clip_end = tok_byte_end.min(byte_end);
            if !line_str.is_char_boundary(clip_start) || !line_str.is_char_boundary(clip_end) {
                continue;
            }
            let text = &line_str[clip_start..clip_end];
            let color = self.token_color(tok.kind);

            if tok.kind == TokenKind::Whitespace {
                if !batch_text.is_empty() {
                    draw_list.add_text([batch_start_x, y], col32(batch_color), &batch_text);
                    batch_text.clear();
                }
                for ch in text.chars() {
                    x += if ch == '\t' { self.char_advance * self.config.tab_size as f32 }
                         else { self.char_advance };
                }
                batch_start_x = x;
                first_batch = true;
                continue;
            }

            if !first_batch && color == batch_color {
                batch_text.push_str(text);
                x += text.chars().count() as f32 * self.char_advance;
                continue;
            }

            if !batch_text.is_empty() {
                draw_list.add_text([batch_start_x, y], col32(batch_color), &batch_text);
            }
            batch_text.clear();
            batch_text.push_str(text);
            batch_start_x = x;
            batch_color = color;
            first_batch = false;
            x += text.chars().count() as f32 * self.char_advance;
        }
        if !batch_text.is_empty() {
            draw_list.add_text([batch_start_x, y], col32(batch_color), &batch_text);
        }
    }

    /// Draw small colored swatches next to hex color literals on a single line.
    ///
    /// Recognises `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `0xRRGGBB`, `0xAARRGGBB`.
    fn draw_hex_color_swatches(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_str: &str,
        text_start_x: f32,
        y: f32,
    ) {
        let swatch = (self.line_height - 4.0).max(6.0);
        let sy_off = (self.line_height - swatch) / 2.0;
        let bytes = line_str.as_bytes();
        let len = bytes.len();
        let mut i = 0usize;

        while i < len {
            // Find start of a potential hex token
            let (tok_start, tok_end) = if bytes[i] == b'#'
                && i + 1 < len && bytes[i + 1].is_ascii_hexdigit()
            {
                let s = i;
                i += 1;
                while i < len && bytes[i].is_ascii_hexdigit() { i += 1; }
                (s, i)
            } else if i + 1 < len
                && bytes[i] == b'0'
                && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
            {
                let s = i;
                i += 2;
                while i < len && bytes[i].is_ascii_hexdigit() { i += 1; }
                (s, i)
            } else {
                // Advance safely over one Unicode scalar
                i += line_str[i..].chars().next().map_or(1, |c| c.len_utf8());
                continue;
            };

            // Safety: tok_start/tok_end are on ASCII boundaries
            if !line_str.is_char_boundary(tok_start)
                || !line_str.is_char_boundary(tok_end)
            {
                continue;
            }
            let token_text = &line_str[tok_start..tok_end];
            if let Some(color) = parse_hex_color(token_text) {
                // x position: chars up to end of token × char_advance + gap
                let char_end = line_str[..tok_end].chars().count();
                let sx = text_start_x
                    + col_to_x(line_str, char_end, self.char_advance, self.config.tab_size)
                    + 2.0;
                let sy = y + sy_off;

                // Filled swatch
                draw_list
                    .add_rect([sx, sy], [sx + swatch, sy + swatch], col32(color))
                    .filled(true)
                    .build();
                // Dark border
                draw_list
                    .add_rect([sx, sy], [sx + swatch, sy + swatch],
                              col32([0.0, 0.0, 0.0, 0.55]))
                    .filled(false)
                    .build();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_selection(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        sel: buffer::Selection,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        let (start, end) = sel.ordered();
        if line_idx < start.line || line_idx > end.line { return; }

        let line_chars = line_str.chars().count();
        let sel_start = if line_idx == start.line { start.col } else { 0 };
        let sel_end = if line_idx == end.line { end.col } else { line_chars + 1 };

        // Clip to the sub-row column range
        let vis_start = sel_start.max(col_start);
        let vis_end = sel_end.min(col_end);

        if vis_start >= vis_end { return; }

        // X positions are relative to col_start (the sub-row starts at text_start_x)
        let x1 = text_start_x + col_to_x(line_str, vis_start, self.char_advance, self.config.tab_size)
            - col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        let x2 = text_start_x + col_to_x(line_str, vis_end, self.char_advance, self.config.tab_size)
            - col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        let bg = self.config.colors.selection_bg;
        draw_list.add_rect(
            [x1, y],
            [x2, y + self.line_height],
            col32(bg),
        ).filled(true).build();
        // Thin border for extra visibility
        let border_color = [bg[0], bg[1], bg[2], (bg[3] + 0.25).min(1.0)];
        draw_list.add_rect(
            [x1, y],
            [x2, y + self.line_height],
            col32(border_color),
        ).build();
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_find_matches(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
        col_start: usize,
        col_end: usize,
    ) {
        if !self.find_replace.open { return; }
        let col_start_x = col_to_x(line_str, col_start, self.char_advance, self.config.tab_size);
        for (i, &(ml, cs, ce)) in self.find_replace.matches.iter().enumerate() {
            if ml != line_idx { continue; }
            // Clip match to sub-row range
            let vis_start = cs.max(col_start);
            let vis_end = ce.min(col_end);
            if vis_start >= vis_end { continue; }

            let x1 = text_start_x + col_to_x(line_str, vis_start, self.char_advance, self.config.tab_size) - col_start_x;
            let x2 = text_start_x + col_to_x(line_str, vis_end, self.char_advance, self.config.tab_size) - col_start_x;
            let color = if i == self.find_replace.current_match {
                self.config.colors.search_current_bg
            } else {
                self.config.colors.search_match_bg
            };
            draw_list.add_rect(
                [x1, y],
                [x2, y + self.line_height],
                col32(color),
            ).filled(true).build();
        }
    }

    fn draw_fold_indicator(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        win_x: f32,
        gutter_width: f32,
        y: f32,
    ) {
        let region = self.fold_regions.iter()
            .find(|r| r.start_line == line_idx);
        if let Some(region) = region {
            // Position the icon in the gutter, right before line numbers.
            // Place fold icon at right edge of gutter, between line numbers and code
            let icon_x = win_x + gutter_width - self.char_advance * 1.8;
            let icon_y = y;
            let color = col32([0.55, 0.58, 0.65, 0.9]);
            let color_hover = col32([0.75, 0.80, 0.90, 1.0]);

            // Use MDI chevron icons for crisp rendering at any size.
            let icon = if region.folded {
                icons::CHEVRON_RIGHT  // ▸ collapsed
            } else {
                icons::CHEVRON_DOWN   // ▾ expanded
            };

            // Highlight on hover (mouse in the fold icon area).
            let mouse_pos = unsafe { dear_imgui_rs::sys::igGetMousePos() };
            let in_fold_area = mouse_pos.x >= icon_x
                && mouse_pos.x < icon_x + self.char_advance * 1.5
                && mouse_pos.y >= y
                && mouse_pos.y < y + self.line_height;
            let c = if in_fold_area { color_hover } else { color };

            draw_list.add_text([icon_x, icon_y], c, icon);

            // Draw "... N lines" badge after the line text when folded.
            if region.folded {
                let hidden = region.end_line.saturating_sub(region.start_line);
                if hidden > 0 {
                    let badge = format!(" ... {hidden} lines ");
                    let line_str = self.buffer.line(line_idx);
                    let text_x = win_x + gutter_width + 4.0;
                    let badge_x = text_x + line_str.len() as f32 * self.char_advance;
                    let badge_y = y;
                    let badge_w = badge.len() as f32 * self.char_advance;

                    // Badge background
                    let bg = col32([0.20, 0.22, 0.28, 0.85]);
                    let border = col32([0.35, 0.38, 0.45, 0.6]);
                    draw_list.add_rect(
                        [badge_x, badge_y + 1.0],
                        [badge_x + badge_w, badge_y + self.line_height - 1.0],
                        bg,
                    ).filled(true).rounding(3.0).build();
                    draw_list.add_rect(
                        [badge_x, badge_y + 1.0],
                        [badge_x + badge_w, badge_y + self.line_height - 1.0],
                        border,
                    ).rounding(3.0).build();

                    // Badge text
                    let text_col = col32([0.60, 0.65, 0.72, 1.0]);
                    draw_list.add_text([badge_x, badge_y], text_col, &badge);
                }
            }
        }
    }

    fn token_color(&self, kind: TokenKind) -> [f32; 4] {
        match kind {
            TokenKind::Keyword => self.config.colors.keyword,
            TokenKind::TypeName => self.config.colors.type_name,
            TokenKind::Lifetime => self.config.colors.lifetime,
            TokenKind::String => self.config.colors.string,
            TokenKind::CharLit => self.config.colors.char_lit,
            TokenKind::Number => self.config.colors.number,
            TokenKind::Comment => self.config.colors.comment,
            TokenKind::Attribute => self.config.colors.attribute,
            TokenKind::MacroCall => self.config.colors.macro_call,
            TokenKind::Operator => self.config.colors.operator,
            TokenKind::Punctuation => self.config.colors.punctuation,
            TokenKind::Identifier => self.config.colors.identifier,
            TokenKind::Whitespace => self.config.colors.identifier,
            TokenKind::UserCodeMarker => self.config.colors.user_code_marker,
            TokenKind::HexNull => self.config.colors.hex_null,
            TokenKind::HexFF => self.config.colors.hex_ff,
            TokenKind::HexDefault => self.config.colors.hex_default,
            TokenKind::HexPrintable => self.config.colors.hex_printable,
        }
    }

    // ── Find/Replace ────────────────────────────────────────────────

    fn update_find_matches(&mut self) {
        self.find_replace.update_matches(self.buffer.lines());
    }

    fn find_next(&mut self) {
        if self.find_replace.matches.is_empty() { return; }
        self.find_replace.current_match =
            (self.find_replace.current_match + 1)
                % self.find_replace.matches.len();
        self.jump_to_current_match();
    }

    fn find_prev(&mut self) {
        if self.find_replace.matches.is_empty() { return; }
        if self.find_replace.current_match == 0 {
            self.find_replace.current_match =
                self.find_replace.matches.len() - 1;
        } else {
            self.find_replace.current_match -= 1;
        }
        self.jump_to_current_match();
    }

    fn jump_to_current_match(&mut self) {
        if let Some(&(line, col_start, col_end)) =
            self.find_replace.matches.get(self.find_replace.current_match)
        {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.ensure_cursor_visible();
        }
    }

    fn replace_current(&mut self) {
        if self.find_replace.matches.is_empty() || self.config.read_only {
            return;
        }
        self.snapshot_undo(true);
        if let Some(&(line, col_start, col_end)) =
            self.find_replace.matches.get(self.find_replace.current_match)
        {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.buffer.backspace();
            self.buffer.insert_text(&self.find_replace.replacement.clone());
            self.invalidate_token_cache_all();
            self.update_find_matches();
        }
    }

    fn replace_all(&mut self) {
        if self.find_replace.matches.is_empty() || self.config.read_only {
            return;
        }
        self.snapshot_undo(true);
        // Replace from bottom to top so positions don't shift
        let replacement = self.find_replace.replacement.clone();
        let mut matches = self.find_replace.matches.clone();
        matches.reverse();
        for (line, col_start, col_end) in matches {
            self.buffer.set_selection(
                CursorPos::new(line, col_start),
                CursorPos::new(line, col_end),
            );
            self.buffer.backspace();
            self.buffer.insert_text(&replacement);
        }
        self.invalidate_token_cache_all();
        self.update_find_matches();
    }

    fn render_context_menu(&mut self, ui: &Ui) {
        let Some(_popup) = ui.begin_popup("##editor_ctx") else { return };
        let has_sel = self.buffer.selection().is_some();
        let ro = self.config.read_only;
        let cm = self.config.context_menu.clone();

        // ── Clipboard ────────────────────────────────────────────────────────
        if cm.show_clipboard {
            if ui.menu_item_enabled_selected_with_shortcut(
                "Cut", "Ctrl+X", false, has_sel && !ro,
            ) {
                let text = self.buffer.selected_text();
                if !text.is_empty() {
                    set_clipboard(&text);
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.invalidate_token_cache_all();
                    self.reset_blink();
                }
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                "Copy", "Ctrl+C", false, has_sel,
            ) {
                let text = self.buffer.selected_text();
                if !text.is_empty() { set_clipboard(&text); }
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                "Paste", "Ctrl+V", false, !ro,
            ) {
                if let Some(clip) = get_clipboard()
                    && !clip.is_empty()
                {
                    self.snapshot_undo(true);
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_all();
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Select All ───────────────────────────────────────────────────────
        if cm.show_select_all {
            if ui.menu_item_with_shortcut("Select All", "Ctrl+A") {
                self.buffer.select_all();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Undo / Redo ──────────────────────────────────────────────────────
        if cm.show_undo_redo {
            if ui.menu_item_enabled_selected_with_shortcut(
                "Undo", "Ctrl+Z", false, !ro && self.undo_stack.can_undo(),
            ) {
                self.undo();
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                "Redo", "Ctrl+Y", false, !ro && self.undo_stack.can_redo(),
            ) {
                self.redo();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Code actions ─────────────────────────────────────────────────────
        if cm.show_code_actions {
            if ui.menu_item_enabled_selected_with_shortcut(
                "Toggle Comment", "Ctrl+/", false, !ro,
            ) {
                self.snapshot_undo(true);
                let (start, end) = if let Some(sel) = self.buffer.selection() {
                    let (s, e) = sel.ordered();
                    (s.line, e.line)
                } else {
                    let l = self.buffer.cursor().line;
                    (l, l)
                };
                self.buffer.toggle_line_comment(start..end + 1);
                self.invalidate_token_cache_all();
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                "Duplicate Line", "Ctrl+Shift+D", false, !ro,
            ) {
                self.snapshot_undo(true);
                self.buffer.duplicate_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                ui.close_current_popup();
            }
            if ui.menu_item_enabled_selected_with_shortcut(
                "Delete Line", "Ctrl+Shift+K", false, !ro,
            ) {
                self.snapshot_undo(true);
                self.buffer.delete_line();
                self.invalidate_token_cache_all();
                self.ensure_cursor_visible();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── Transform submenu ────────────────────────────────────────────────
        if cm.show_transform && !ro && has_sel {
            if let Some(_m) = ui.begin_menu("Transform") {
                if ui.menu_item("UPPERCASE") {
                    let t = self.buffer.selected_text().to_uppercase();
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item("lowercase") {
                    let t = self.buffer.selected_text().to_lowercase();
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item("Title Case") {
                    let t = title_case(&self.buffer.selected_text());
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
                if ui.menu_item("Trim Whitespace") {
                    let t = self.buffer.selected_text()
                        .lines()
                        .map(str::trim_end)
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.snapshot_undo(true);
                    self.buffer.backspace();
                    self.buffer.insert_text(&t);
                    self.invalidate_token_cache_all();
                    ui.close_current_popup();
                }
            }
            ui.separator();
        }

        // ── Find ─────────────────────────────────────────────────────────────
        if cm.show_find {
            if ui.menu_item_enabled_selected_with_shortcut("Find…", "Ctrl+F", false, true) {
                let sel = self.buffer.selected_text();
                if !sel.is_empty() && !sel.contains('\n') {
                    self.find_replace.query = sel;
                }
                self.find_replace.open = true;
                self.find_replace.show_replace = false;
                self.find_replace.just_opened = true;
                self.update_find_matches();
                ui.close_current_popup();
            }
            ui.separator();
        }

        // ── View submenu ─────────────────────────────────────────────────────
        if cm.show_view_toggles
            && let Some(_m) = ui.begin_menu("View")
        {
                macro_rules! toggle {
                    ($label:expr, $field:expr) => {
                        if ui.menu_item_enabled_selected_no_shortcut($label, $field, true) {
                            $field = !$field;
                        }
                    };
                }
                toggle!("Word Wrap",              self.config.word_wrap);
                toggle!("Line Numbers",           self.config.show_line_numbers);
                toggle!("Highlight Current Line", self.config.highlight_current_line);
                toggle!("Show Whitespace",        self.config.show_whitespace);
                toggle!("Color Swatches",         self.config.show_color_swatches);
                toggle!("Smooth Scrolling",       self.config.smooth_scrolling);
                toggle!("English on Focus",       self.config.force_english_on_focus);
        }

        // ── Language submenu ─────────────────────────────────────────────────
        if cm.show_language_selector
            && let Some(_m) = ui.begin_menu("Language")
        {
                for (lang, name) in [
                    (Language::Rust, "Rust"),
                    (Language::Rhai, "Rhai"),
                    (Language::Toml, "TOML"),
                    (Language::Ron,  "RON"),
                    (Language::Json, "JSON"),
                    (Language::Yaml, "YAML"),
                    (Language::Xml,  "XML / HTML"),
                    (Language::Asm,  "Assembly (x86)"),
                    (Language::Hex,  "Hex Bytes"),
                    (Language::None, "Plain Text"),
                ] {
                    let selected = self.config.language == lang;
                    if ui.menu_item_enabled_selected_no_shortcut(name, selected, true) {
                        self.config.language = lang;
                        self.invalidate_token_cache_all();
                    }
                }
                // Show custom language name (read-only — can't switch away via menu)
                if let Language::Custom(ref def) = self.config.language.clone() {
                    ui.separator();
                    ui.text_disabled(format!("Custom: {}", def.name()));
                }
        }

        // ── Theme submenu ─────────────────────────────────────────────────────
        if cm.show_theme_selector
            && let Some(_m) = ui.begin_menu("Theme")
        {
                for &theme in EditorTheme::ALL {
                    let selected = self.config.theme == theme;
                    if ui.menu_item_enabled_selected_no_shortcut(
                        theme.display_name(), selected, true,
                    ) {
                        self.config.set_theme(theme);
                        self.invalidate_token_cache_all();
                    }
                }
            ui.separator();
        }

        // ── Font size ±────────────────────────────────────────────────────────
        if cm.show_font_size {
            ui.text("Font scale:");
            ui.same_line();
            let dec_lbl = format!("{}##fsd", icons::FORMAT_FONT_SIZE_DECREASE);
            if ui.small_button(&dec_lbl) {
                self.config.font_size_scale =
                    (self.config.font_size_scale - 0.1).clamp(0.4, 4.0);
            }
            if ui.is_item_hovered() { ui.tooltip_text("Decrease font size"); }
            ui.same_line();
            ui.text(format!("{:.0}%", self.config.font_size_scale * 100.0));
            ui.same_line();
            let inc_lbl = format!("{}##fsi", icons::FORMAT_FONT_SIZE_INCREASE);
            if ui.small_button(&inc_lbl) {
                self.config.font_size_scale =
                    (self.config.font_size_scale + 0.1).clamp(0.4, 4.0);
            }
            if ui.is_item_hovered() { ui.tooltip_text("Increase font size"); }
            ui.separator();
        }

        // ── Cursor info ───────────────────────────────────────────────────────
        if cm.show_cursor_info {
            let cur = self.buffer.cursor();
            let total = self.buffer.line_count();
            ui.text_disabled(format!(
                "Ln {}, Col {}  /  {} lines",
                cur.line + 1, cur.col + 1, total
            ));
        }
    }

    fn render_find_replace_bar(&mut self, ui: &Ui) {
        let avail_w = ui.content_region_avail()[0];
        // Row height: search row + optional replace row + 2px separator
        let row_h  = self.line_height + 8.0;
        let bar_h  = if self.find_replace.show_replace && !self.config.read_only {
            row_h * 2.0 + 4.0
        } else {
            row_h
        };

        // Dark toolbar background
        let _bg = ui.push_style_color(StyleColor::ChildBg, [0.11, 0.13, 0.17, 1.0]);

        ui.child_window("##find_bar")
            .size([avail_w, bar_h])
            .build(ui, || {
            // ── Row 1: Find ──────────────────────────────────────────
            ui.spacing();

            // Auto-focus the input field the frame the bar opens
            if self.find_replace.just_opened {
                // SAFETY: igSetKeyboardFocusHere sets focus on the next item.
                unsafe { dear_imgui_rs::sys::igSetKeyboardFocusHere(0); }
                self.find_replace.just_opened = false;
            }

            // Search icon + input
            ui.text_disabled(icons::MAGNIFY);
            ui.same_line();
            let query_w = (avail_w * 0.38).clamp(140.0, 360.0);
            ui.set_next_item_width(query_w);
            let changed = ui
                .input_text("##find_query", &mut self.find_replace.query)
                .hint("Find…")
                .build();
            if changed {
                self.update_find_matches();
                self.find_replace.current_match = 0;
            }

            // Navigate with Enter / Shift+Enter in the search field
            if ui.is_item_focused() {
                let io = ui.io();
                if ui.is_key_pressed(Key::Enter) || ui.is_key_pressed(Key::DownArrow) {
                    self.find_next();
                }
                if (io.key_shift() && ui.is_key_pressed(Key::Enter))
                    || ui.is_key_pressed(Key::UpArrow)
                {
                    self.find_prev();
                }
            }

            ui.same_line();

            // Match counter  "3 / 17"  or "No matches" in red
            if self.find_replace.query.is_empty() {
                ui.text_disabled("…");
            } else if self.find_replace.matches.is_empty() {
                ui.text_colored([0.9, 0.35, 0.35, 1.0], "No matches");
            } else {
                ui.text_colored(
                    [0.55, 0.85, 0.55, 1.0],
                    format!(
                        "{} / {}",
                        self.find_replace.current_match + 1,
                        self.find_replace.matches.len()
                    ),
                );
            }

            ui.same_line();

            // Prev / Next buttons
            let prev_lbl = format!("{}##fp", icons::ARROW_UP_BOLD);
            if ui.small_button(&prev_lbl) {
                self.find_prev();
            }
            if ui.is_item_hovered() { ui.tooltip_text("Previous match  Shift+F3"); }
            ui.same_line();
            let next_lbl = format!("{}##fn", icons::ARROW_DOWN_BOLD);
            if ui.small_button(&next_lbl) {
                self.find_next();
            }
            if ui.is_item_hovered() { ui.tooltip_text("Next match  F3"); }

            ui.same_line();

            // ── Toggle: case-sensitive ───────────────────────────────
            let cs_col = if self.find_replace.case_sensitive {
                [0.24, 0.52, 0.88, 0.90]
            } else {
                [0.28, 0.30, 0.36, 0.70]
            };
            let _c = ui.push_style_color(StyleColor::Button, cs_col);
            let cs_lbl = format!("{}##fcs", icons::FORMAT_LETTER_CASE);
            if ui.small_button(&cs_lbl) {
                self.find_replace.case_sensitive = !self.find_replace.case_sensitive;
                self.update_find_matches();
            }
            drop(_c);
            if ui.is_item_hovered() { ui.tooltip_text("Case sensitive"); }

            ui.same_line();

            // ── Toggle: whole word ───────────────────────────────────
            let ww_col = if self.find_replace.whole_word {
                [0.24, 0.52, 0.88, 0.90]
            } else {
                [0.28, 0.30, 0.36, 0.70]
            };
            let _w = ui.push_style_color(StyleColor::Button, ww_col);
            let ww_lbl = format!("{}##fww", icons::FORMAT_LETTER_MATCHES);
            if ui.small_button(&ww_lbl) {
                self.find_replace.whole_word = !self.find_replace.whole_word;
                self.update_find_matches();
            }
            drop(_w);
            if ui.is_item_hovered() { ui.tooltip_text("Whole word"); }

            if !self.config.read_only {
                ui.same_line();
                // Toggle replace row
                let rep_lbl = format!("{}##frep", icons::FIND_REPLACE);
                if ui.small_button(&rep_lbl) {
                    self.find_replace.show_replace = !self.find_replace.show_replace;
                }
                if ui.is_item_hovered() { ui.tooltip_text("Toggle replace  Ctrl+H"); }
            }

            ui.same_line();

            // Close button
            let close_lbl = format!("{}##fc", icons::CLOSE_THICK);
            if ui.small_button(&close_lbl) {
                self.find_replace.open = false;
            }
            if ui.is_item_hovered() { ui.tooltip_text("Close  Esc"); }

            // ── Row 2: Replace (only in writable editors) ────────────
            if self.find_replace.show_replace && !self.config.read_only {
                ui.text_disabled(icons::FIND_REPLACE);
                ui.same_line();
                let rep_w = (avail_w * 0.38).clamp(140.0, 360.0);
                ui.set_next_item_width(rep_w);
                ui.input_text("##find_rep", &mut self.find_replace.replacement)
                    .hint("Replace with…")
                    .build();
                ui.same_line();
                if ui.small_button("Replace##r1") {
                    self.replace_current();
                }
                ui.same_line();
                if ui.small_button("All##ra") {
                    self.replace_all();
                }
            }
        });
    }

    // ── Token cache management ──────────────────────────────────────

    fn ensure_token_cache_size(&mut self) {
        let count = self.buffer.line_count();
        self.token_cache.resize_with(count, || None);
    }

    fn get_cached_tokens(&mut self, line_idx: usize) -> Rc<Vec<Token>> {
        let line_str = self.buffer.line(line_idx);
        let content_hash = hash_line(line_str);
        let in_bc = self.block_comment_states
            .get(line_idx)
            .copied()
            .unwrap_or(false);

        // Check cache hit — Rc::clone is just a refcount bump, no Vec copy.
        if let Some(Some(cached)) = self.token_cache.get(line_idx)
            && cached.content_hash == content_hash
            && cached.in_block_comment == in_bc
        {
            return Rc::clone(&cached.tokens);
        }

        // Cache miss — tokenize
        let (tokens, _ends_in_bc) =
            tokenize_line(line_str, &self.config.language, in_bc);
        let rc = Rc::new(tokens);

        // Store in cache
        if line_idx < self.token_cache.len() {
            self.token_cache[line_idx] = Some(CachedLineTokens {
                content_hash,
                in_block_comment: in_bc,
                tokens: Rc::clone(&rc),
            });
        }

        rc
    }

    /// Read-only token lookup — returns cached tokens or empty.
    /// Call `get_cached_tokens` first to ensure the cache is populated.
    fn cached_tokens(&self, line_idx: usize) -> Rc<Vec<Token>> {
        if let Some(Some(cached)) = self.token_cache.get(line_idx) {
            Rc::clone(&cached.tokens)
        } else {
            Rc::new(Vec::new())
        }
    }

    fn invalidate_token_cache_at(&mut self, line: usize) {
        if line < self.token_cache.len() {
            self.token_cache[line] = None;
        }
        // Mark bc state dirty from this line so incremental recompute starts here.
        self.bc_dirty_from = Some(
            self.bc_dirty_from.map_or(line, |old| old.min(line)),
        );
        self.bc_version = u64::MAX;
    }

    /// Invalidate token cache from `from_line` onward (for structural edits
    /// that insert/remove lines). Entries before `from_line` stay valid.
    fn invalidate_token_cache_from(&mut self, from_line: usize) {
        self.token_cache.truncate(from_line);
        self.bc_dirty_from = Some(
            self.bc_dirty_from.map_or(from_line, |old| old.min(from_line)),
        );
        self.bc_version = u64::MAX;
    }

    fn invalidate_token_cache_all(&mut self) {
        self.invalidate_token_cache_from(0);
    }

    // ── Undo/Redo ───────────────────────────────────────────────────

    fn snapshot_undo(&mut self, force: bool) {
        let entry = UndoEntry {
            text: self.buffer.text(),
            cursor: self.buffer.cursor(),
            version: self.buffer.edit_version(),
        };
        if force {
            self.undo_stack.force_snapshot(entry);
        } else {
            self.undo_stack.push(entry, false);
        }
    }

    fn current_undo_entry(&self) -> UndoEntry {
        UndoEntry {
            text: self.buffer.text(),
            cursor: self.buffer.cursor(),
            version: self.buffer.edit_version(),
        }
    }

    /// Perform undo.
    pub fn undo(&mut self) {
        let current = self.current_undo_entry();
        if let Some(entry) = self.undo_stack.undo(current) {
            self.buffer.set_text(&entry.text);
            self.buffer.set_cursor(entry.cursor);
            self.invalidate_token_cache_all();
            self.ensure_cursor_visible();
        }
    }

    /// Perform redo.
    pub fn redo(&mut self) {
        let current = self.current_undo_entry();
        if let Some(entry) = self.undo_stack.redo(current) {
            self.buffer.set_text(&entry.text);
            self.buffer.set_cursor(entry.cursor);
            self.invalidate_token_cache_all();
            self.ensure_cursor_visible();
        }
    }

    // ── Cursor blink ────────────────────────────────────────────────

    fn update_blink(&mut self, dt: f32) {
        if self.config.cursor_blink_rate <= 0.0 {
            self.cursor_visible = true;
            return;
        }
        self.blink_timer += dt;
        if self.blink_timer >= self.config.cursor_blink_rate {
            self.blink_timer -= self.config.cursor_blink_rate;
            self.cursor_visible = !self.cursor_visible;
        }
    }

    fn reset_blink(&mut self) {
        self.blink_timer = 0.0;
        self.cursor_visible = true;
    }

    // ── Word wrap ─────────────────────────────────────────────────────

    /// Recompute the word-wrap cache if the text changed or the
    /// available width changed.
    fn update_wrap_cache(&mut self, text_width: f32) {
        if !self.config.word_wrap {
            // Ensure offsets are identity when wrap is off.
            if !self.wrap_cols.is_empty() {
                self.wrap_cols.clear();
                self.wrap_row_offsets.clear();
                self.wrap_row_offsets.push(0);
                self.wrap_cached_version = u64::MAX;
            }
            return;
        }
        let version = self.buffer.edit_version();
        let width_changed = (text_width - self.wrap_cached_width).abs() > 0.5;
        if version == self.wrap_cached_version && !width_changed {
            return;
        }
        self.wrap_cached_version = version;
        self.wrap_cached_width = text_width;

        let line_count = self.buffer.line_count();
        self.wrap_cols.resize(line_count, Vec::new());
        self.wrap_row_offsets.resize(line_count + 1, 0);

        let mut cumulative = 0usize;
        for i in 0..line_count {
            self.wrap_row_offsets[i] = cumulative;
            let line = self.buffer.line(i);
            let wraps = compute_wrap_points(
                line, text_width, self.char_advance, self.config.tab_size,
            );
            let rows = wraps.len() + 1;
            self.wrap_cols[i] = wraps;
            cumulative += rows;
        }
        self.wrap_row_offsets[line_count] = cumulative;
    }

    /// Total number of visual rows (accounting for word wrap).
    fn total_visual_rows(&self) -> usize {
        if !self.config.word_wrap || self.wrap_row_offsets.len() <= 1 {
            return self.buffer.line_count();
        }
        *self.wrap_row_offsets.last().unwrap_or(&0)
    }

    /// Convert a buffer (line, col) to a visual row index.
    fn visual_row_of(&self, line: usize, col: usize) -> usize {
        if !self.config.word_wrap
            || line >= self.wrap_cols.len()
            || line >= self.wrap_row_offsets.len()
        {
            return line;
        }
        let base = self.wrap_row_offsets[line];
        let wraps = &self.wrap_cols[line];
        // Find which sub-row this col falls in.
        let sub = wraps.iter().position(|&wc| col < wc).unwrap_or(wraps.len());
        base + sub
    }

    /// Convert a visual row to (buffer_line, sub_row_index).
    fn visual_row_to_line(&self, vrow: usize) -> (usize, usize) {
        let line_count = self.buffer.line_count();
        // Fall back to identity when wrap is off or cache is stale/empty.
        if !self.config.word_wrap
            || self.wrap_row_offsets.len() < line_count + 1
        {
            return (vrow.min(line_count.saturating_sub(1)), 0);
        }
        // Binary search: find largest line whose offset <= vrow.
        let mut lo = 0usize;
        let mut hi = line_count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.wrap_row_offsets[mid + 1] <= vrow {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let line = lo.min(line_count.saturating_sub(1));
        let sub = vrow.saturating_sub(self.wrap_row_offsets[line]);
        (line, sub)
    }

    /// Get the column range for a sub-row of a line.
    fn sub_row_col_range(&self, line: usize, sub_row: usize) -> (usize, usize) {
        if !self.config.word_wrap || line >= self.wrap_cols.len() {
            return (0, self.buffer.line(line).chars().count());
        }
        let wraps = &self.wrap_cols[line];
        let start = if sub_row == 0 { 0 } else {
            wraps.get(sub_row - 1).copied().unwrap_or(0)
        };
        let end = wraps.get(sub_row).copied()
            .unwrap_or_else(|| self.buffer.line(line).chars().count());
        (start, end)
    }

    // ── Keyboard layout switching ──────────────────────────────────

    /// On focus gain: save current layout and switch to English (US).
    /// On focus loss: restore the previously saved layout.
    fn handle_input_locale_switch(&mut self) {
        let gained = self.focused && !self.was_focused;
        let lost   = !self.focused && self.was_focused;
        self.was_focused = self.focused;

        if !self.config.force_english_on_focus { return; }

        #[cfg(target_os = "windows")]
        {
            // English (US) keyboard layout identifier: 0x0409
            const EN_US: usize = 0x0409;

            unsafe extern "system" {
                fn GetKeyboardLayout(thread_id: u32) -> usize;
                fn ActivateKeyboardLayout(hkl: usize, flags: u32) -> usize;
            }

            if gained {
                let current = unsafe { GetKeyboardLayout(0) };
                self.saved_input_locale = current;
                // ActivateKeyboardLayout with 0 flags = KLF_SETFORPROCESS not set
                // — applies only to the current thread.
                unsafe { ActivateKeyboardLayout(EN_US, 0); }
            } else if lost && self.saved_input_locale != 0 {
                unsafe { ActivateKeyboardLayout(self.saved_input_locale, 0); }
                self.saved_input_locale = 0;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (gained, lost);
        }
    }

    // ── Smooth scrolling ────────────────────────────────────────────

    fn update_smooth_scroll(&mut self, dt: f32) {
        if !self.config.smooth_scrolling { return; }

        let diff = self.target_scroll_y - self.scroll_y;
        if diff.abs() < 0.5 {
            self.scroll_y = self.target_scroll_y;
            return;
        }

        // When the gap is large (rapid Enter / PgDn), snap harder so the
        // cursor never drifts off-screen.  For small gaps the original
        // smooth ease-out is used.
        let big_gap = diff.abs() > self.line_height * 3.0;
        let speed = if big_gap { 25.0_f32 } else { 12.0_f32 };
        let factor = 1.0 - (-speed * dt).exp();
        self.scroll_y += diff * factor;
    }

    // ── Scroll management ───────────────────────────────────────────

    fn visible_lines(&self) -> usize {
        if self.line_height > 0.0 {
            (self.visible_height / self.line_height) as usize
        } else {
            30
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let cursor = self.buffer.cursor();
        let vrow = self.visual_row_of(cursor.line, cursor.col);
        let cursor_y = vrow as f32 * self.line_height;

        // Vertical
        let target = if cursor_y < self.scroll_y {
            cursor_y
        } else if cursor_y + self.line_height
            > self.scroll_y + self.visible_height
        {
            cursor_y + self.line_height - self.visible_height
        } else {
            return;
        };

        if self.config.smooth_scrolling {
            self.target_scroll_y = target;
        } else {
            self.scroll_y = target;
            self.target_scroll_y = target;
        }
    }

    /// Truncate pasted text to respect `max_lines` and `max_line_length`.
    fn truncate_paste(&self, text: &str) -> String {
        let max_lines = self.config.max_lines;
        let max_len = self.config.max_line_length;

        let mut result = String::with_capacity(text.len());
        let current_lines = self.buffer.line_count();
        let mut added_newlines = 0usize;

        for (i, line) in text.split('\n').enumerate() {
            // Check line count budget
            if max_lines > 0 && i > 0
                && current_lines + added_newlines >= max_lines {
                    break;
                }
            if i > 0 {
                result.push('\n');
                added_newlines += 1;
            }
            // Truncate line length
            if max_len > 0 && line.chars().count() > max_len {
                result.extend(line.chars().take(max_len));
            } else {
                result.push_str(line);
            }
        }
        result
    }

    // ── Block comment state tracking ────────────────────────────────

    fn update_block_comment_states(&mut self) {
        let version = self.buffer.edit_version();
        if self.bc_version == version { return; }
        self.bc_version = version;

        let count = self.buffer.line_count();
        let start_from = self.bc_dirty_from.unwrap_or(0).min(count);
        self.bc_dirty_from = None;

        // Resize to match line count (preserves existing correct entries).
        self.block_comment_states.resize(count, false);

        // Determine the bc state entering `start_from`.
        let mut in_bc = if start_from == 0 {
            false
        } else {
            let prev_bc = self.block_comment_states[start_from - 1];
            let (_, still_in) = tokenize_line(
                self.buffer.line(start_from - 1),
                &self.config.language,
                prev_bc,
            );
            still_in
        };

        for i in start_from..count {
            self.block_comment_states[i] = in_bc;
            let (_, still_in) = tokenize_line(
                self.buffer.line(i),
                &self.config.language,
                in_bc,
            );
            in_bc = still_in;

            // Early exit: if the bc state entering the next line matches
            // what was already stored, all downstream states are correct.
            if i + 1 < count && self.block_comment_states[i + 1] == in_bc {
                break;
            }
        }
    }

    // ── Code folding ────────────────────────────────────────────────

    fn update_fold_regions(&mut self) {
        let version = self.buffer.edit_version();
        if self.fold_version == version { return; }
        self.fold_version = version;

        let new_regions = detect_fold_regions(self.buffer.lines());

        // Preserve fold state from existing regions — HashMap for O(1) lookup
        let was_folded: std::collections::HashMap<usize, bool> = self.fold_regions.iter()
            .map(|r| (r.start_line, r.folded))
            .collect();

        self.fold_regions = new_regions;

        for region in &mut self.fold_regions {
            if let Some(&folded) = was_folded.get(&region.start_line) {
                region.folded = folded;
            }
        }
    }

    /// Build list of (line_index, screen_row) for visible lines,
    /// skipping folded regions.
    fn build_visible_lines(
        &self,
        first_visible: usize,
        last_visible: usize,
    ) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(last_visible - first_visible);
        let mut line_idx = first_visible;
        while line_idx < last_visible && line_idx < self.buffer.line_count() {
            result.push((line_idx, line_idx));

            // Check if this line starts a folded region
            let folded_end = self.fold_regions.iter()
                .find(|r| r.start_line == line_idx && r.folded)
                .map(|r| r.end_line);

            if let Some(end) = folded_end {
                line_idx = end + 1;
            } else {
                line_idx += 1;
            }
        }
        result
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn digit_count(n: usize) -> usize {
    if n == 0 { return 1; }
    let mut count = 0;
    let mut v = n;
    while v > 0 {
        count += 1;
        v /= 10;
    }
    count
}

/// Compute column indices where a line should wrap.
///
/// Returns an empty vec if the line fits within `max_width`.
/// Each entry is the char-column where a new visual row begins.
/// Prefers breaking at the last space (word boundary); falls back to
/// a hard break at the column that exceeds the width.
fn compute_wrap_points(
    line: &str,
    max_width: f32,
    char_advance: f32,
    tab_size: u8,
) -> Vec<usize> {
    if max_width <= char_advance { return Vec::new(); }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut wraps = Vec::new();
    let mut x = 0.0f32;
    let mut last_space: Option<usize> = None;
    let mut row_start = 0usize;

    let char_w = |ch: char| -> f32 {
        if ch == '\t' { char_advance * tab_size as f32 } else { char_advance }
    };

    let mut col = 0usize;
    while col < len {
        let ch = chars[col];
        let w = char_w(ch);

        // Check BEFORE adding: will this character overflow the row?
        // Exception: first character of a row always goes on that row
        // (prevents infinite loop on very narrow widths).
        if x + w > max_width && col > row_start {
            // Prefer breaking at a word boundary (last space).
            let wrap_col = if let Some(sp) = last_space {
                if sp > row_start { sp } else { col }
            } else {
                col
            };
            wraps.push(wrap_col);

            // Reset x: re-measure from wrap_col up to (but not including)
            // the current col — those characters landed on the new row.
            x = 0.0;
            for &c in &chars[wrap_col..col] {
                x += char_w(c);
            }
            row_start = wrap_col;
            last_space = None;
            // Do NOT advance col — re-evaluate the current character
            // against the fresh row (handles lines wider than 2× max_width).
            continue;
        }

        x += w;

        if ch == ' ' || ch == '\t' {
            last_space = Some(col + 1); // wrap AFTER whitespace
        }

        col += 1;
    }
    wraps
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = CodeEditor::new("test");
        assert_eq!(editor.line_count(), 1);
        assert!(!editor.is_modified());
        assert!(!editor.is_read_only());
    }

    #[test]
    fn test_set_get_text() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("fn main() {\n    println!(\"hi\");\n}");
        assert_eq!(editor.line_count(), 3);
        let text = editor.get_text();
        assert!(text.contains("fn main()"));
        assert!(text.contains("println!"));
    }

    #[test]
    fn test_language() {
        let mut editor = CodeEditor::new("test");
        editor.set_language(Language::Toml);
        assert_eq!(editor.config().language, Language::Toml);
    }

    #[test]
    fn test_read_only() {
        let mut editor = CodeEditor::new("test");
        editor.set_read_only(true);
        assert!(editor.is_read_only());
    }

    #[test]
    fn test_goto_line() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("line1\nline2\nline3\nline4\nline5");
        editor.goto_line(3);
        assert_eq!(editor.cursor().line, 3);
    }

    #[test]
    fn test_modified_flag() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("hello");
        assert!(!editor.is_modified());
        editor.buffer.insert_char('x');
        assert!(editor.is_modified());
        editor.clear_modified();
        assert!(!editor.is_modified());
    }

    #[test]
    fn test_error_markers() {
        let mut editor = CodeEditor::new("test");
        editor.set_error_markers(vec![
            LineMarker {
                line: 5,
                message: "error here".into(),
                is_error: true,
            },
        ]);
        assert_eq!(editor.error_markers.len(), 1);
    }

    #[test]
    fn test_digit_count() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(999), 3);
        assert_eq!(digit_count(1000), 4);
    }

    #[test]
    fn test_block_comment_states() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("/* start\nmiddle\nend */ code");
        editor.update_block_comment_states();
        assert_eq!(editor.block_comment_states, vec![false, true, true]);
    }

    #[test]
    fn test_hash_line() {
        let h1 = hash_line("hello world");
        let h2 = hash_line("hello world");
        let h3 = hash_line("hello World");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_find_matches() {
        let mut state = FindReplaceState::default();
        state.query = "hello".to_string();
        let lines = vec![
            "hello world".to_string(),
            "say hello".to_string(),
            "nothing here".to_string(),
        ];
        state.update_matches(&lines);
        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.matches[0], (0, 0, 5));
        assert_eq!(state.matches[1], (1, 4, 9));
    }

    #[test]
    fn test_find_case_insensitive() {
        let mut state = FindReplaceState::default();
        state.query = "hello".to_string();
        state.case_sensitive = false;
        let lines = vec!["Hello HELLO hello".to_string()];
        state.update_matches(&lines);
        assert_eq!(state.matches.len(), 3);
    }

    #[test]
    fn test_fold_regions() {
        let lines = vec![
            "fn main() {".to_string(),
            "    let x = 1;".to_string(),
            "    if true {".to_string(),
            "        foo();".to_string(),
            "    }".to_string(),
            "}".to_string(),
        ];
        let regions = detect_fold_regions(&lines);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].start_line, 0);
        assert_eq!(regions[0].end_line, 5);
        assert_eq!(regions[1].start_line, 2);
        assert_eq!(regions[1].end_line, 4);
    }

    #[test]
    fn test_closing_bracket() {
        assert_eq!(closing_bracket('('), Some(')'));
        assert_eq!(closing_bracket('{'), Some('}'));
        assert_eq!(closing_bracket('['), Some(']'));
        assert_eq!(closing_bracket('a'), None);
    }

    #[test]
    fn test_duplicate_line() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("line1\nline2\nline3");
        editor.buffer.set_cursor(CursorPos::new(1, 0));
        editor.buffer.duplicate_line();
        assert_eq!(editor.line_count(), 4);
        assert_eq!(editor.buffer.line(1), "line2");
        assert_eq!(editor.buffer.line(2), "line2");
    }

    #[test]
    fn test_move_line_up() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("aaa\nbbb\nccc");
        editor.buffer.set_cursor(CursorPos::new(1, 0));
        editor.buffer.move_line_up();
        assert_eq!(editor.buffer.line(0), "bbb");
        assert_eq!(editor.buffer.line(1), "aaa");
    }

    #[test]
    fn test_move_line_down() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("aaa\nbbb\nccc");
        editor.buffer.set_cursor(CursorPos::new(1, 0));
        editor.buffer.move_line_down();
        assert_eq!(editor.buffer.line(1), "ccc");
        assert_eq!(editor.buffer.line(2), "bbb");
    }

    #[test]
    fn test_toggle_comment() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("fn main() {\n    let x = 1;\n}");
        editor.buffer.toggle_line_comment(1..2);
        assert_eq!(editor.buffer.line(1), "    // let x = 1;");
        // Toggle again to uncomment
        editor.buffer.toggle_line_comment(1..2);
        assert_eq!(editor.buffer.line(1), "    let x = 1;");
    }

    #[test]
    fn test_delete_line() {
        let mut editor = CodeEditor::new("test");
        editor.set_text("aaa\nbbb\nccc");
        editor.buffer.set_cursor(CursorPos::new(1, 0));
        editor.buffer.delete_line();
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.buffer.line(0), "aaa");
        assert_eq!(editor.buffer.line(1), "ccc");
    }

    // ── Property-based tests ─────────────────────────────────────────────

    use proptest::prelude::*;

    proptest! {
        /// `parse_hex_color` accepts arbitrary strings without panicking.
        /// Any Some(rgba) must be a 4-element array with values in [0,1].
        #[test]
        fn prop_parse_hex_color_never_panics(s in ".{0,16}") {
            if let Some(rgba) = parse_hex_color(&s) {
                for c in rgba {
                    prop_assert!((0.0..=1.0).contains(&c));
                }
            }
        }

        /// `#RRGGBB` strings (6 hex digits) must decode successfully and
        /// the decoded RGBA must have alpha == 1.0, with each channel
        /// matching the input byte.
        #[test]
        fn prop_parse_hex_color_6_digit_decodes(
            r in any::<u8>(), g in any::<u8>(), b in any::<u8>(),
        ) {
            let s = format!("#{r:02X}{g:02X}{b:02X}");
            let rgba = parse_hex_color(&s).expect("valid 6-digit hex must parse");
            prop_assert!((rgba[3] - 1.0).abs() < f32::EPSILON);
            prop_assert_eq!((rgba[0] * 255.0).round() as u8, r);
            prop_assert_eq!((rgba[1] * 255.0).round() as u8, g);
            prop_assert_eq!((rgba[2] * 255.0).round() as u8, b);
        }
    }
}
