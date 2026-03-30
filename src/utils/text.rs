//! Text measurement helpers wrapping ImGui's CalcTextSize.

/// Calculate text size using the current ImGui font.
///
/// Equivalent to `ImGui::CalcTextSize()` in the C++ API.
/// Returns `[width, height]`.
pub fn calc_text_size(text: impl AsRef<str>) -> [f32; 2] {
    let text = text.as_ref();
    let text_start = text.as_ptr() as *const std::os::raw::c_char;
    let text_end = unsafe { text_start.add(text.len()) };
    let out = unsafe {
        dear_imgui_rs::sys::igCalcTextSize(text_start, text_end, false, -1.0)
    };
    [out.x, out.y]
}
