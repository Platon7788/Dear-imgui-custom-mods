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

pub mod buffer;
pub mod config;
pub mod tokenizer;
pub mod undo;

pub use config::{EditorConfig, Language, SyntaxColors};

use buffer::{CursorPos, Selection, TextBuffer};
use tokenizer::{Token, TokenKind, tokenize_line};
use undo::{UndoEntry, UndoStack};

use crate::utils::color::rgba_f32;
use crate::utils::text::calc_text_size;

use dear_imgui_rs::{Key, MouseButton, StyleColor, Ui, WindowFlags};

/// Pack an `[f32; 4]` RGBA color into u32 for DrawList.
#[inline]
fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
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
        slice.iter().filter_map(|&wc| char::from_u32(wc as u32)).collect()
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
    /// Computed tokens.
    tokens: Vec<Token>,
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
                if let Some(start) = region_stack.pop() {
                    if i > start {
                        regions.push(FoldRegion {
                            start_line: start,
                            end_line: i,
                            folded: false,
                        });
                    }
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
    /// Computed character advance width (monospace).
    char_advance: f32,
    /// Computed line height.
    line_height: f32,
    /// Cached visible height of the editor window.
    visible_height: f32,
    /// Whether the editor is focused.
    focused: bool,
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

    // ── Markers ──────────────────────────────────────────────────────
    error_markers: Vec<LineMarker>,
    breakpoints: Vec<Breakpoint>,

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

    // ── Font scale (Ctrl+Scroll zoom) ────────────────────────────────
    /// Current text zoom factor (1.0 = default, 0.4–4.0 range).
    text_scale: f32,
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
            char_advance: 7.0,
            line_height: 16.0,
            visible_height: 300.0,
            focused: false,
            blink_timer: 0.0,
            cursor_visible: true,

            token_cache: Vec::new(),
            block_comment_states: vec![false],
            bc_version: u64::MAX,

            error_markers: Vec::new(),
            breakpoints: Vec::new(),

            find_replace: FindReplaceState::default(),

            fold_regions: Vec::new(),
            fold_version: u64::MAX,

            mouse_selecting: false,
            last_click_time: 0.0,
            last_click_pos: CursorPos::default(),
            click_count: 0,

            text_scale: 1.0,
        }
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Set the entire text content (resets undo, cursor, selection).
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.undo_stack.clear();
        self.bc_version = u64::MAX; // force recompute
        self.fold_version = u64::MAX;
        self.token_cache.clear();
        self.find_replace.matches.clear();
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
        self.error_markers = markers;
    }

    /// Set breakpoints.
    pub fn set_breakpoints(&mut self, bps: Vec<Breakpoint>) {
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
        if pos.col >= line.len() {
            return None;
        }
        // Expand left
        let mut start = pos.col;
        while start > 0 {
            let ch = line.as_bytes()[start - 1] as char;
            if ch.is_alphanumeric() || ch == '_' {
                start -= 1;
            } else {
                break;
            }
        }
        // Expand right
        let mut end = pos.col;
        while end < line.len() {
            let ch = line.as_bytes()[end] as char;
            if ch.is_alphanumeric() || ch == '_' {
                end += 1;
            } else {
                break;
            }
        }
        if start == end {
            return None;
        }
        Some(line[start..end].to_string())
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
        self.text_scale
    }

    /// Set text zoom factor (clamped to 0.4–4.0).
    pub fn set_text_scale(&mut self, scale: f32) {
        self.text_scale = scale.clamp(0.4, 4.0);
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
        unsafe {
            dear_imgui_rs::sys::igPushFont(
                std::ptr::null_mut(),
                base_font_size * self.text_scale,
            );
        }

        // Measure monospace character size (uses the now-active scaled font)
        let [cw, ch] = calc_text_size("X");
        self.char_advance = cw;
        self.line_height = ch + 2.0;

        // Recompute caches if text changed
        self.update_block_comment_states();
        self.update_fold_regions();
        self.ensure_token_cache_size();

        let gutter_width = if self.config.show_line_numbers {
            let digits = digit_count(self.buffer.line_count());
            // Extra space for fold indicators
            (digits as f32 + 3.0) * self.char_advance
        } else {
            self.char_advance * 2.0 // minimal gutter for fold arrows
        };

        // ── Find/Replace bar at the TOP (before the editor child window) ──
        if self.find_replace.open {
            self.render_find_replace_bar(ui);
        }

        let avail = ui.content_region_avail();
        let child_id = format!("##ce_{}", self.id);

        // Push style for the editor region
        let _bg_token = ui.push_style_color(
            StyleColor::ChildBg,
            self.config.colors.gutter_bg,
        );

        ui.child_window(&child_id)
            .size(avail)
            .flags(WindowFlags::HORIZONTAL_SCROLLBAR | WindowFlags::NO_MOVE)
            .build(ui, || {
            self.focused = ui.is_window_focused();
            self.visible_height = ui.window_size()[1];

            // Capture the inner content area (excludes scrollbar regions) for
            // accurate cursor hit-testing inside handle_mouse.
            let content_avail = ui.content_region_avail();

            // Update cursor blink
            let dt = ui.io().delta_time();
            self.update_blink(dt);

            // Smooth scrolling
            self.update_smooth_scroll(dt);

            // Handle input
            if self.focused {
                self.handle_keyboard(ui);
            }
            // Mouse is handled whenever the window is hovered — this also covers
            // the frame on which the window *gains* focus via a click, so the very
            // first click both focuses the editor AND positions the cursor correctly
            // instead of leaving it at (0, 0).
            self.handle_mouse(ui, gutter_width, content_avail);

            let draw_list = ui.get_window_draw_list();
            let [win_x, win_y] = ui.cursor_screen_pos();
            let scroll_y = ui.scroll_y();
            let scroll_x = ui.scroll_x();
            self.scroll_x = scroll_x;
            self.scroll_y = scroll_y;

            let first_visible = (scroll_y / self.line_height) as usize;
            let visible_count = (self.visible_height / self.line_height) as usize + 2;
            let last_visible = (first_visible + visible_count)
                .min(self.buffer.line_count());

            let text_start_x = win_x + gutter_width - scroll_x;
            let cursor_pos = self.buffer.cursor();
            let selection = self.buffer.selection();
            let matching_bracket = if self.config.bracket_matching {
                self.buffer.find_matching_bracket()
            } else {
                None
            };

            // ── Build visible line list (respecting folds) ──────────
            let visible_lines = self.build_visible_lines(first_visible, last_visible);

            // ── Draw lines (batched) ────────────────────────────────
            for &(line_idx, screen_row) in &visible_lines {
                let y = win_y + (screen_row as f32) * self.line_height - scroll_y;
                let line_str_owned = self.buffer.line(line_idx).to_string();
                let line_str = line_str_owned.as_str();

                // Current line highlight
                if self.config.highlight_current_line
                    && line_idx == cursor_pos.line
                    && self.focused
                {
                    draw_list.add_rect(
                        [win_x, y],
                        [win_x + avail[0] + scroll_x, y + self.line_height],
                        col32(self.config.colors.current_line_bg),
                    ).filled(true).build();
                }

                // Selection highlight (primary)
                if let Some(sel) = selection {
                    self.draw_selection(&draw_list, sel, line_idx, line_str,
                                       text_start_x, y);
                }

                // Extra cursor selections
                for extra_sel in self.buffer.extra_selections() {
                    if let Some(sel) = extra_sel {
                        self.draw_selection(&draw_list, *sel, line_idx, line_str,
                                           text_start_x, y);
                    }
                }

                // Find/Replace match highlights
                self.draw_find_matches(&draw_list, line_idx, text_start_x, y);

                // Error marker background
                if self.error_markers.iter().any(|m| m.line == line_idx) {
                    draw_list.add_rect(
                        [win_x, y],
                        [win_x + avail[0] + scroll_x, y + self.line_height],
                        col32([0.80, 0.20, 0.20, 0.15]),
                    ).filled(true).build();
                }

                // Breakpoint marker in gutter
                if self.breakpoints.iter().any(|bp| bp.line == line_idx && bp.enabled) {
                    let center = [win_x + gutter_width * 0.2, y + self.line_height * 0.5];
                    let radius = self.line_height * 0.3;
                    draw_list.add_circle(center, radius, col32(crate::theme::DANGER))
                        .filled(true)
                        .build();
                }

                // Fold indicator in gutter
                self.draw_fold_indicator(&draw_list, line_idx, win_x, gutter_width, y);

                // Line number
                if self.config.show_line_numbers {
                    let num_str = format!("{}", line_idx + 1);
                    let num_color = if line_idx == cursor_pos.line {
                        self.config.colors.line_number_active
                    } else {
                        self.config.colors.line_number
                    };
                    let num_x = win_x + gutter_width
                        - (num_str.len() as f32 + 1.5) * self.char_advance;
                    draw_list.add_text([num_x, y], col32(num_color), &num_str);
                }

                // Gutter separator line
                draw_list.add_line(
                    [win_x + gutter_width - self.char_advance * 0.5, y],
                    [win_x + gutter_width - self.char_advance * 0.5,
                     y + self.line_height],
                    col32(crate::theme::SEPARATOR),
                ).build();

                // ── Tokenized text (batched draw calls) ─────────────
                let tokens = self.get_cached_tokens(line_idx);
                self.draw_tokens_batched(
                    &draw_list, &tokens, line_str, text_start_x, y,
                );

                // Bracket match highlight
                if let Some(match_pos) = matching_bracket {
                    if match_pos.line == line_idx {
                        let bx = text_start_x
                            + match_pos.col as f32 * self.char_advance;
                        draw_list.add_rect(
                            [bx, y],
                            [bx + self.char_advance, y + self.line_height],
                            col32(self.config.colors.bracket_match_bg),
                        ).filled(true).build();
                    }
                    if cursor_pos.line == line_idx {
                        let bx = text_start_x
                            + cursor_pos.col as f32 * self.char_advance;
                        draw_list.add_rect(
                            [bx, y],
                            [bx + self.char_advance, y + self.line_height],
                            col32(self.config.colors.bracket_match_bg),
                        ).filled(true).build();
                    }
                }
            }

            // ── Cursor (primary + extras) ──────────────────────────
            if self.focused && self.cursor_visible && !self.config.read_only {
                let cx = text_start_x
                    + cursor_pos.col as f32 * self.char_advance;
                let cy = win_y
                    + cursor_pos.line as f32 * self.line_height - scroll_y;
                draw_list.add_line(
                    [cx, cy],
                    [cx, cy + self.line_height],
                    col32(crate::theme::TEXT_PRIMARY),
                ).thickness(1.5).build();

                // Draw extra cursors
                for extra in self.buffer.extra_cursors() {
                    let ex = text_start_x + extra.col as f32 * self.char_advance;
                    let ey = win_y + extra.line as f32 * self.line_height - scroll_y;
                    if ey >= win_y - self.line_height && ey <= win_y + avail[1] {
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
                let hover_line =
                    ((my - win_y + scroll_y) / self.line_height) as usize;
                for marker in &self.error_markers {
                    if marker.line == hover_line {
                        ui.tooltip_text(&marker.message);
                        break;
                    }
                }
            }

            // ── Minimap ─────────────────────────────────────────────
            if self.config.show_minimap {
                let minimap_w = 80.0_f32;
                let minimap_x = win_x + avail[0] - minimap_w + scroll_x;
                let minimap_y = win_y;
                let minimap_h = avail[1].min(self.visible_height);
                let line_count = self.buffer.line_count();
                let minimap_line_h = if line_count > 0 {
                    (minimap_h / line_count as f32).min(2.0).max(0.5)
                } else {
                    1.0
                };
                let minimap_total_h = line_count as f32 * minimap_line_h;
                let minimap_scale = if minimap_total_h > minimap_h {
                    minimap_h / minimap_total_h
                } else {
                    1.0
                };

                // Background
                draw_list.add_rect(
                    [minimap_x, minimap_y],
                    [minimap_x + minimap_w, minimap_y + minimap_h],
                    col32([0.078, 0.078, 0.098, 0.784]),
                ).filled(true).build();

                // Viewport indicator
                let vp_start = first_visible as f32 * minimap_line_h * minimap_scale;
                let vp_end = last_visible as f32 * minimap_line_h * minimap_scale;
                draw_list.add_rect(
                    [minimap_x, minimap_y + vp_start],
                    [minimap_x + minimap_w, minimap_y + vp_end.min(minimap_h)],
                    col32([0.235, 0.314, 0.471, 0.392]),
                ).filled(true).build();

                // Draw lines as colored bars
                let char_w = minimap_w / 80.0; // scale to ~80 cols
                for i in 0..line_count {
                    let y = minimap_y + i as f32 * minimap_line_h * minimap_scale;
                    if y > minimap_y + minimap_h { break; }
                    let line = self.buffer.line(i);
                    let trimmed = line.trim_start();
                    if trimmed.is_empty() { continue; }
                    let indent = (line.len() - trimmed.len()) as f32;
                    let len = trimmed.len().min(80) as f32;
                    let color: [f32; 4] = if trimmed.starts_with("//") {
                        [0.235, 0.431, 0.235, 0.706]  // comment green
                    } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                        [0.392, 0.627, 0.863, 0.784] // function blue
                    } else if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ")
                        || trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ")
                    {
                        [0.784, 0.667, 0.392, 0.784] // type yellow
                    } else {
                        [0.549, 0.549, 0.588, 0.588] // default gray
                    };
                    let x0 = minimap_x + indent * char_w;
                    let x1 = (minimap_x + (indent + len) * char_w).min(minimap_x + minimap_w);
                    draw_list.add_rect(
                        [x0, y],
                        [x1, y + (minimap_line_h * minimap_scale).max(1.0)],
                        col32(color),
                    ).filled(true).build();

                    // Error markers on minimap
                    if self.error_markers.iter().any(|m| m.line == i) {
                        draw_list.add_rect(
                            [minimap_x, y],
                            [minimap_x + 3.0, y + (minimap_line_h * minimap_scale).max(1.0)],
                            col32([1.0, 0.314, 0.314, 0.863]),
                        ).filled(true).build();
                    }
                }

                // Click on minimap to scroll
                if ui.is_window_hovered() {
                    let [mx_pos, my_pos] = ui.io().mouse_pos();
                    if mx_pos >= minimap_x && mx_pos <= minimap_x + minimap_w
                        && my_pos >= minimap_y && my_pos <= minimap_y + minimap_h
                        && ui.is_mouse_clicked(dear_imgui_rs::MouseButton::Left)
                    {
                        let click_frac = (my_pos - minimap_y) / minimap_h;
                        let target_line = (click_frac * line_count as f32) as usize;
                        self.buffer.set_cursor(CursorPos { line: target_line, col: 0 });
                    }
                }
            }

            // Set dummy size for scrolling
            let total_height =
                self.buffer.line_count() as f32 * self.line_height;
            let max_line_len = (first_visible..last_visible)
                .map(|i| self.buffer.line(i).chars().count())
                .max()
                .unwrap_or(80);
            let total_width =
                gutter_width + (max_line_len as f32 + 10.0) * self.char_advance;
            ui.set_cursor_pos([0.0, total_height]);
            ui.dummy([total_width, 0.0]);

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

        if ctrl {
            nav_key!(LeftArrow, self.buffer.move_word_left());
            nav_key!(RightArrow, self.buffer.move_word_right());
            nav_key!(Home, self.buffer.move_doc_start());
            nav_key!(End, self.buffer.move_doc_end());
        } else {
            nav_key!(LeftArrow, self.buffer.move_left());
            nav_key!(RightArrow, self.buffer.move_right());
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
                self.invalidate_token_cache_all();
                self.reset_blink();
            }
            return;
        }

        if ctrl && ui.is_key_pressed(Key::V) && !self.config.read_only {
            if let Some(clip) = get_clipboard() {
                if !clip.is_empty() {
                    self.snapshot_undo(true);
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_all();
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
                self.snapshot_undo(true);
                self.buffer.insert_newline(
                    self.config.auto_indent,
                    self.config.tab_size,
                );
                self.invalidate_token_cache_all();
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Backspace) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_backspace();
                } else if ctrl {
                    self.buffer.delete_word_left();
                } else {
                    self.buffer.backspace();
                }
                self.invalidate_token_cache_all();
                self.reset_blink();
                self.ensure_cursor_visible();
                return;
            }

            if ui.is_key_pressed(Key::Delete) {
                self.snapshot_undo(self.buffer.has_extra_cursors() || ctrl);
                if self.buffer.has_extra_cursors() && !ctrl {
                    self.buffer.multi_delete();
                } else if ctrl {
                    self.buffer.delete_word_right();
                } else {
                    self.buffer.delete();
                }
                self.invalidate_token_cache_all();
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
                        self.invalidate_token_cache_all();
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
                self.invalidate_token_cache_all();
                self.reset_blink();
                return;
            }

            // ── Text input (typed characters) ───────────────────────
            let input_chars = read_input_chars();
            for ch in input_chars {
                if ch >= ' ' && ch != '\x7f' {
                    self.snapshot_undo(false);
                    if self.buffer.has_extra_cursors() {
                        self.buffer.multi_insert_char(ch);
                        self.invalidate_token_cache_all();
                    } else {
                        self.buffer.insert_char(ch);
                        self.invalidate_token_cache_at(self.buffer.cursor().line);
                    }

                    // Auto-close brackets
                    if self.config.auto_close_brackets {
                        if let Some(close) = closing_bracket(ch) {
                            self.buffer.insert_char(close);
                            self.buffer.move_left();
                        }
                    }

                    // Auto-close quotes (only if not already inside a string)
                    if self.config.auto_close_quotes {
                        if let Some(close) = closing_quote(ch) {
                            // Simple heuristic: don't auto-close if preceded by `\`
                            let line = self.buffer.line(self.buffer.cursor().line);
                            let col = self.buffer.cursor().col;
                            let is_escaped = col >= 2
                                && line.chars().nth(col - 2) == Some('\\');
                            if !is_escaped {
                                self.buffer.insert_char(close);
                                self.buffer.move_left();
                            }
                        }
                    }

                    // Auto-skip closing bracket if typed and already next char
                    if is_closing_bracket(ch) {
                        let line = self.buffer.line(self.buffer.cursor().line);
                        let col = self.buffer.cursor().col;
                        if col < line.chars().count() {
                            let next_ch = line.chars().nth(col);
                            if next_ch == Some(ch) {
                                // Delete the auto-inserted one (we just typed the
                                // closing bracket, and the next char is also the
                                // same closing bracket from auto-close)
                                self.buffer.delete();
                            }
                        }
                    }

                    self.reset_blink();
                }
            }
        }
    }

    fn handle_mouse(&mut self, ui: &Ui, gutter_width: f32, content_avail: [f32; 2]) {
        if !ui.is_window_hovered() { return; }

        let io = ui.io();
        let [mx, my] = io.mouse_pos();
        let [win_x, win_y] = ui.cursor_screen_pos();
        let text_x = win_x + gutter_width;

        // ── Ctrl+Scroll zoom ──────────────────────────────────────────────
        if io.key_ctrl() && io.mouse_wheel() != 0.0 {
            self.text_scale = (self.text_scale + io.mouse_wheel() * 0.1).clamp(0.4, 4.0);
        }

        // ── I-beam cursor ONLY inside the text content area ───────────────
        // content_avail excludes scrollbar width/height, so this won't fire
        // when the pointer is hovering the vertical or horizontal scrollbar.
        let content_max_x = win_x + content_avail[0];
        let content_max_y = win_y + content_avail[1];
        if mx >= text_x && mx < content_max_x && my >= win_y && my < content_max_y {
            // SAFETY: igSetMouseCursor is a standard ImGui call.
            unsafe {
                dear_imgui_rs::sys::igSetMouseCursor(
                    dear_imgui_rs::sys::ImGuiMouseCursor_TextInput,
                );
            }
        }

        // Convert mouse position to text position.
        // Use floor() (no +0.5 rounding) so clicking anywhere on character N
        // places the cursor BEFORE character N — the standard "between chars" feel.
        let line = ((my - win_y + self.scroll_y) / self.line_height)
            .max(0.0) as usize;
        let line = line.min(self.buffer.line_count().saturating_sub(1));
        let col = ((mx - text_x + self.scroll_x) / self.char_advance)
            .max(0.0) as usize;
        let max_col = self.buffer.line(line).chars().count();
        let col = col.min(max_col);
        let click_pos = CursorPos::new(line, col);

        let time = ui.time();

        // Click in gutter area → toggle fold
        if ui.is_mouse_clicked(MouseButton::Left) && mx < text_x {
            // Check if this line has a fold region
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
            ui.open_popup("##editor_ctx");
        }

        // Scroll with mouse wheel (smooth) — suppressed when Ctrl is held (zoom mode)
        let wheel = ui.io().mouse_wheel();
        if wheel != 0.0 && !io.key_ctrl() {
            let delta = -wheel * self.config.scroll_speed * self.line_height;
            let max_scroll =
                (self.buffer.line_count() as f32 * self.line_height).max(0.0);
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
            let text = &line_str[tok.start..tok.start + tok.len];
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
                        if ch == ' ' {
                            let cx = x + self.char_advance * 0.5;
                            let cy = y + self.line_height * 0.5;
                            draw_list.add_circle(
                                [cx, cy], 1.0,
                                col32(crate::theme::TEXT_MUTED),
                            ).filled(true).build();
                        } else if ch == '\t' {
                            // Draw tab arrow
                            let arrow_y = y + self.line_height * 0.5;
                            let tab_w = self.char_advance
                                * self.config.tab_size as f32;
                            draw_list.add_line(
                                [x + 2.0, arrow_y],
                                [x + tab_w - 2.0, arrow_y],
                                col32(crate::theme::TEXT_MUTED),
                            ).build();
                        }
                        x += self.char_advance;
                    }
                } else {
                    x += text.chars().count() as f32 * self.char_advance;
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

    fn draw_selection(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        sel: buffer::Selection,
        line_idx: usize,
        line_str: &str,
        text_start_x: f32,
        y: f32,
    ) {
        let (start, end) = sel.ordered();
        if line_idx < start.line || line_idx > end.line { return; }

        let line_chars = line_str.chars().count();
        let sel_start = if line_idx == start.line { start.col } else { 0 };
        let sel_end = if line_idx == end.line { end.col } else { line_chars + 1 };

        if sel_start >= sel_end { return; }

        let x1 = text_start_x + sel_start as f32 * self.char_advance;
        let x2 = text_start_x + sel_end as f32 * self.char_advance;
        draw_list.add_rect(
            [x1, y],
            [x2, y + self.line_height],
            col32(self.config.colors.selection_bg),
        ).filled(true).build();
    }

    fn draw_find_matches(
        &self,
        draw_list: &dear_imgui_rs::DrawListMut<'_>,
        line_idx: usize,
        text_start_x: f32,
        y: f32,
    ) {
        if !self.find_replace.open { return; }
        for (i, &(ml, cs, ce)) in self.find_replace.matches.iter().enumerate() {
            if ml != line_idx { continue; }
            let x1 = text_start_x + cs as f32 * self.char_advance;
            let x2 = text_start_x + ce as f32 * self.char_advance;
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
            let cx = win_x + gutter_width - self.char_advance * 1.2;
            let cy = y + self.line_height * 0.5;
            let size = self.line_height * 0.25;
            let color = col32(crate::theme::TEXT_MUTED);

            if region.folded {
                // Right-pointing triangle ▶
                draw_list.add_triangle(
                    [cx - size * 0.5, cy - size],
                    [cx - size * 0.5, cy + size],
                    [cx + size, cy],
                    color,
                ).filled(true).build();
            } else {
                // Down-pointing triangle ▼
                draw_list.add_triangle(
                    [cx - size, cy - size * 0.5],
                    [cx + size, cy - size * 0.5],
                    [cx, cy + size],
                    color,
                ).filled(true).build();
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

        // ── Edit operations ──────────────────────────────────────────────────
        if ui.menu_item_enabled_selected_with_shortcut("Cut", "Ctrl+X", false, has_sel && !ro) {
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
        if ui.menu_item_enabled_selected_with_shortcut("Copy", "Ctrl+C", false, has_sel) {
            let text = self.buffer.selected_text();
            if !text.is_empty() {
                set_clipboard(&text);
            }
            ui.close_current_popup();
        }
        if ui.menu_item_enabled_selected_with_shortcut("Paste", "Ctrl+V", false, !ro) {
            if let Some(clip) = get_clipboard() {
                if !clip.is_empty() {
                    self.snapshot_undo(true);
                    self.buffer.insert_text(&clip);
                    self.invalidate_token_cache_all();
                    self.reset_blink();
                    self.ensure_cursor_visible();
                }
            }
            ui.close_current_popup();
        }

        ui.separator();

        // ── Selection ────────────────────────────────────────────────────────
        if ui.menu_item_with_shortcut("Select All", "Ctrl+A") {
            self.buffer.select_all();
            ui.close_current_popup();
        }

        ui.separator();

        // ── Undo / Redo ──────────────────────────────────────────────────────
        if ui.menu_item_enabled_selected_with_shortcut(
            "Undo", "Ctrl+Z", false, !ro && self.undo_stack.can_undo()
        ) {
            self.undo();
            ui.close_current_popup();
        }
        if ui.menu_item_enabled_selected_with_shortcut(
            "Redo", "Ctrl+Y", false, !ro && self.undo_stack.can_redo()
        ) {
            self.redo();
            ui.close_current_popup();
        }

        ui.separator();

        // ── Code helpers ─────────────────────────────────────────────────────
        if ui.menu_item_enabled_selected_with_shortcut(
            "Toggle Comment", "Ctrl+/", false, !ro
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
            "Find…", "Ctrl+F", false, true
        ) {
            let sel = self.buffer.selected_text();
            if !sel.is_empty() && !sel.contains('\n') {
                self.find_replace.query = sel;
            }
            self.find_replace.open = true;
            self.find_replace.show_replace = false;
            self.update_find_matches();
            ui.close_current_popup();
        }

        ui.separator();

        // ── View toggles ─────────────────────────────────────────────────────
        if ui.menu_item_enabled_selected_no_shortcut(
            "Minimap", self.config.show_minimap, true
        ) {
            self.config.show_minimap = !self.config.show_minimap;
        }
        if ui.menu_item_enabled_selected_no_shortcut(
            "Word Wrap", self.config.word_wrap, true
        ) {
            self.config.word_wrap = !self.config.word_wrap;
        }
        if ui.menu_item_enabled_selected_no_shortcut(
            "Line Numbers", self.config.show_line_numbers, true
        ) {
            self.config.show_line_numbers = !self.config.show_line_numbers;
        }
        if ui.menu_item_enabled_selected_no_shortcut(
            "Highlight Current Line", self.config.highlight_current_line, true
        ) {
            self.config.highlight_current_line = !self.config.highlight_current_line;
        }

        ui.separator();

        // ── Line info ────────────────────────────────────────────────────────
        let cur = self.buffer.cursor();
        let total = self.buffer.line_count();
        ui.text_disabled(format!("Ln {}, Col {}  /  {} lines", cur.line + 1, cur.col + 1, total));
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

            // Search input
            let query_w = (avail_w * 0.38).max(140.0).min(360.0);
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
                    &format!(
                        "{} / {}",
                        self.find_replace.current_match + 1,
                        self.find_replace.matches.len()
                    ),
                );
            }

            ui.same_line();

            // Prev / Next buttons
            if ui.small_button("◀##fp") {
                self.find_prev();
            }
            if ui.is_item_hovered() { ui.tooltip_text("Previous match  Shift+F3"); }
            ui.same_line();
            if ui.small_button("▶##fn") {
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
            if ui.small_button("Aa") {
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
            if ui.small_button("W") {
                self.find_replace.whole_word = !self.find_replace.whole_word;
                self.update_find_matches();
            }
            drop(_w);
            if ui.is_item_hovered() { ui.tooltip_text("Whole word"); }

            if !self.config.read_only {
                ui.same_line();
                // Toggle replace row
                let rep_lbl = if self.find_replace.show_replace { "▲ Rep" } else { "▼ Rep" };
                if ui.small_button(rep_lbl) {
                    self.find_replace.show_replace = !self.find_replace.show_replace;
                }
                if ui.is_item_hovered() { ui.tooltip_text("Toggle replace  Ctrl+H"); }
            }

            ui.same_line();

            // Close button
            if ui.small_button("✕##fc") {
                self.find_replace.open = false;
            }
            if ui.is_item_hovered() { ui.tooltip_text("Close  Esc"); }

            // ── Row 2: Replace (only in writable editors) ────────────
            if self.find_replace.show_replace && !self.config.read_only {
                let rep_w = (avail_w * 0.38).max(140.0).min(360.0);
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
        if self.token_cache.len() > count {
            self.token_cache.truncate(count);
        }
    }

    fn get_cached_tokens(&mut self, line_idx: usize) -> Vec<Token> {
        let line_str = self.buffer.line(line_idx);
        let content_hash = hash_line(line_str);
        let in_bc = self.block_comment_states
            .get(line_idx)
            .copied()
            .unwrap_or(false);

        // Check cache hit
        if let Some(Some(cached)) = self.token_cache.get(line_idx) {
            if cached.content_hash == content_hash
                && cached.in_block_comment == in_bc
            {
                return cached.tokens.clone();
            }
        }

        // Cache miss — tokenize
        let (tokens, _ends_in_bc) =
            tokenize_line(line_str, self.config.language, in_bc);

        // Store in cache
        if line_idx < self.token_cache.len() {
            self.token_cache[line_idx] = Some(CachedLineTokens {
                content_hash,
                in_block_comment: in_bc,
                tokens: tokens.clone(),
            });
        }

        tokens
    }

    fn invalidate_token_cache_at(&mut self, line: usize) {
        if line < self.token_cache.len() {
            self.token_cache[line] = None;
        }
    }

    fn invalidate_token_cache_all(&mut self) {
        self.token_cache.clear();
        self.bc_version = u64::MAX;
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

    // ── Smooth scrolling ────────────────────────────────────────────

    fn update_smooth_scroll(&mut self, dt: f32) {
        if !self.config.smooth_scrolling { return; }

        let diff = self.target_scroll_y - self.scroll_y;
        if diff.abs() < 0.5 {
            self.scroll_y = self.target_scroll_y;
            return;
        }

        // Exponential ease-out: lerp toward target at ~12× per second
        let speed = 12.0 * dt;
        self.scroll_y += diff * speed.min(1.0);
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
        let cursor_y = cursor.line as f32 * self.line_height;

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

    // ── Block comment state tracking ────────────────────────────────

    fn update_block_comment_states(&mut self) {
        let version = self.buffer.edit_version();
        if self.bc_version == version { return; }
        self.bc_version = version;

        let count = self.buffer.line_count();
        self.block_comment_states.clear();
        self.block_comment_states.reserve(count);

        let mut in_bc = false;
        for i in 0..count {
            self.block_comment_states.push(in_bc);
            let (_, still_in) = tokenize_line(
                self.buffer.line(i),
                self.config.language,
                in_bc,
            );
            in_bc = still_in;
        }
    }

    // ── Code folding ────────────────────────────────────────────────

    fn update_fold_regions(&mut self) {
        let version = self.buffer.edit_version();
        if self.fold_version == version { return; }
        self.fold_version = version;

        let new_regions = detect_fold_regions(self.buffer.lines());

        // Preserve fold state from existing regions
        let was_folded: Vec<(usize, bool)> = self.fold_regions.iter()
            .map(|r| (r.start_line, r.folded))
            .collect();

        self.fold_regions = new_regions;

        for region in &mut self.fold_regions {
            if let Some((_, folded)) = was_folded.iter()
                .find(|(sl, _)| *sl == region.start_line)
            {
                region.folded = *folded;
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
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
}
