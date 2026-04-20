//! Free helper functions extracted from `mod.rs` during the module split.
//!
//! Everything here is either a pure computation (color parsing, layout math,
//! string tweaks) or a tiny FFI wrapper around `dear_imgui_rs::sys` (clipboard,
//! input-queue read, per-glyph advance measurement). None of these touch
//! `CodeEditor` state — they take their inputs by value / reference and
//! return results.
//!
//! Visibility is `pub(super)` so every sibling module (`input`, `render`, …)
//! can reach them without exposing anything in the crate's public API.

use crate::utils::color::rgba_f32;

// ── Color utilities ─────────────────────────────────────────────────────────

/// Pack an `[f32; 4]` RGBA color into u32 for DrawList.
#[inline]
pub(super) fn col32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Parse a hex color literal into `[r, g, b, a]` (all 0.0–1.0).
///
/// Supports: `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `0xRRGGBB`, `0xAARRGGBB`.
pub(super) fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    #[inline]
    fn byte(hex: &str, pos: usize) -> Option<f32> {
        u8::from_str_radix(&hex[pos..pos + 2], 16)
            .ok()
            .map(|v| v as f32 / 255.0)
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

// ── String utilities ────────────────────────────────────────────────────────

/// Convert a string to Title Case (first char of each word uppercased).
pub(super) fn title_case(s: &str) -> String {
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

// ── Text measurement ────────────────────────────────────────────────────────

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
pub(super) fn calc_char_advance(font_size: f32) -> f32 {
    let text = b"#\0";
    // SAFETY: igGetFont returns the currently-active ImGui font pointer which is
    // guaranteed valid for the lifetime of a frame; `text` is null-terminated.
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

// ── Column ↔ pixel conversion ───────────────────────────────────────────────

/// Convert a column index to pixel X offset, accounting for tab characters.
///
/// **Fast path** (most common): lines without tabs return
/// `col * char_advance` immediately, skipping the O(col) `chars()` scan.
/// `str::contains('\t')` uses `memchr` (SIMD) so the tab-absence check
/// is near-zero cost — orders of magnitude faster than decoding UTF-8
/// char-by-char. Hex-editor lines (no tabs by construction) take this
/// path every time.
///
/// **Slow path** (line contains tabs): walks characters up to `col`,
/// summing per-character widths — regular chars use `char_advance`,
/// tabs use `tab_size * char_advance` (matching `draw_tokens_batched`).
#[inline]
pub(super) fn col_to_x(line: &str, col: usize, char_advance: f32, tab_size: u8) -> f32 {
    if !line.contains('\t') {
        return col as f32 * char_advance;
    }
    let mut x = 0.0f32;
    for (i, ch) in line.chars().enumerate() {
        if i == col {
            return x;
        }
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
///
/// Tab-free fast path matches [`col_to_x`] — closed-form `x / char_advance`
/// via direct floor division with the 0.67 threshold applied.
#[inline]
pub(super) fn x_to_col(line: &str, x: f32, char_advance: f32, tab_size: u8) -> usize {
    if !line.contains('\t') && char_advance > 0.0 {
        // Closed-form path — no Unicode decode, no per-char loop.
        let max_col = line.chars().count();
        if x <= 0.0 {
            return 0;
        }
        let raw = ((x + char_advance * 0.33) / char_advance).floor() as usize;
        return raw.min(max_col);
    }
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

// ── Clipboard + input-queue ─────────────────────────────────────────────────

/// Set clipboard text via ImGui sys API.
pub(super) fn set_clipboard(text: &str) {
    let c_str = std::ffi::CString::new(text).unwrap_or_default();
    // SAFETY: igSetClipboardText takes a null-terminated C string, which CString provides.
    unsafe {
        dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
    }
}

/// Get clipboard text via ImGui sys API.
pub(super) fn get_clipboard() -> Option<String> {
    // SAFETY: `igGetClipboardText` returns a pointer to ImGui's internal
    // null-terminated UTF-8 clipboard buffer (or null if unavailable).
    // `CStr::from_ptr` requires null-termination — documented by ImGui.
    // We immediately copy to `String` so the returned value outlives any
    // subsequent ImGui call that might invalidate the underlying buffer.
    let ptr = unsafe { dear_imgui_rs::sys::igGetClipboardText() };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: ptr is non-null and points at a null-terminated C string
    // owned by ImGui; valid until the next clipboard API call.
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    c_str.to_str().ok().map(|s| s.to_string())
}

/// Read input characters from ImGui's input queue (typed this frame).
pub(super) fn read_input_chars() -> Vec<char> {
    // SAFETY: igGetIO returns a valid pointer to the global ImGuiIO struct.
    // InputQueueCharacters is an ImVector of ImWchar (u16) with typed chars this frame.
    unsafe {
        let io = &*dear_imgui_rs::sys::igGetIO_Nil();
        let data = io.InputQueueCharacters.Data;
        let size = io.InputQueueCharacters.Size;
        if data.is_null() || size <= 0 {
            return Vec::new();
        }
        let slice = std::slice::from_raw_parts(data, size as usize);
        slice.iter().filter_map(|&wc| char::from_u32(wc)).collect()
    }
}

// ── Bracket / quote auto-close pairs ────────────────────────────────────────

const BRACKET_PAIRS: &[(char, char)] = &[('(', ')'), ('{', '}'), ('[', ']')];
const QUOTE_PAIRS: &[(char, char)] = &[('"', '"'), ('\'', '\'')];

pub(super) fn closing_bracket(ch: char) -> Option<char> {
    BRACKET_PAIRS
        .iter()
        .find(|(o, _)| *o == ch)
        .map(|(_, c)| *c)
}

pub(super) fn closing_quote(ch: char) -> Option<char> {
    QUOTE_PAIRS.iter().find(|(o, _)| *o == ch).map(|(_, c)| *c)
}

pub(super) fn is_closing_bracket(ch: char) -> bool {
    BRACKET_PAIRS.iter().any(|(_, c)| *c == ch)
}

pub(super) fn is_closing_quote(ch: char) -> bool {
    QUOTE_PAIRS.iter().any(|(_, c)| *c == ch)
}

// ── Line-count digit width ──────────────────────────────────────────────────

/// Number of digits in `n` — used to size the line-number gutter.
pub(super) fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut n = n;
    let mut c = 0;
    while n > 0 {
        c += 1;
        n /= 10;
    }
    c
}

// ── Fast non-cryptographic string hash ──────────────────────────────────────

/// FNV-1a 64-bit — used for line-content change detection in the token cache.
pub(super) fn hash_line(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digit_count() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
        assert_eq!(digit_count(9999), 4);
    }
}
