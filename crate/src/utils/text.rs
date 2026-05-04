//! Text measurement helpers wrapping ImGui's CalcTextSize.

use dear_imgui_rs::Ui;

/// Calculate text size using the current ImGui font.
///
/// Equivalent to `ImGui::CalcTextSize()` in the C++ API.
/// Returns `[width, height]`.
pub fn calc_text_size(text: impl AsRef<str>) -> [f32; 2] {
    let text = text.as_ref();
    let text_start = text.as_ptr() as *const std::os::raw::c_char;
    // SAFETY: text is a valid &str, so text_start + text.len() is within
    // the same allocation. igCalcTextSize only reads within [start, end).
    let text_end = unsafe { text_start.add(text.len()) };
    let out = unsafe { dear_imgui_rs::sys::igCalcTextSize(text_start, text_end, false, -1.0) };
    [out.x, out.y]
}

/// Single-line text height for the **current** ImGui font, no glyph
/// walk. Equivalent to `igGetTextLineHeight()` — a direct read from
/// `ImGuiContext::FontSize` — and the right replacement for the historic
/// `calc_text_size("Mg" | "M" | "A")[1]` pattern, which paid an
/// `igCalcTextSize` glyph-lookup per frame just to read a number that
/// already lives in the context.
///
/// Use for vertical centring and y-axis spacing in custom draw-list
/// layouts. The value updates automatically when the font scale changes,
/// so no cache invalidation is needed.
#[inline]
pub fn line_height(ui: &Ui) -> f32 {
    ui.text_line_height()
}
