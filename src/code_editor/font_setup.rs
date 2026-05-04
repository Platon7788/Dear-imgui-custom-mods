//! Font management for the code editor.

use std::sync::atomic::{AtomicUsize, Ordering};

// ── Font re-exports for backward compatibility ───────────────────────────────
//
// The bundled TTF blobs and `BuiltinFont` enum live in the top-level `fonts`
// module now (centralised font registry). These re-exports keep existing
// callers like `code_editor::{BuiltinFont, HACK_FONT_DATA, ...}` working.

pub use crate::fonts::{
    BuiltinFont, HACK_FONT_DATA, JETBRAINS_MONO_FONT_DATA, JETBRAINS_MONO_LIGATURES_FONT_DATA,
    MDI_FONT_DATA, merge_mdi_icons,
};

// ── Global code-editor font ───────────────────────────────────────────────────

/// Raw `*mut ImFont` pointer for the **dedicated code-editor font**, stored as
/// a `usize` so it is `Send + Sync`.
///
/// When non-zero, every [`CodeEditor`] instance will push this specific font
/// (via `igPushFont`) instead of the default ImGui font.  This lets the IDE
/// use a proportional UI font (e.g. Segoe UI) while keeping the code editor
/// on a monospace font (e.g. Consolas) — which is required for the column↔pixel
/// mapping to be accurate.
///
/// You can set this manually, or use [`install_code_editor_font`] for zero-config
/// setup with an embedded JetBrains Mono font.
pub static CODE_EDITOR_FONT_PTR: AtomicUsize = AtomicUsize::new(0);

/// Read the current code-editor font pointer (0 = use default font).
#[inline]
pub fn code_editor_font_ptr() -> *mut dear_imgui_rs::sys::ImFont {
    let v = CODE_EDITOR_FONT_PTR.load(Ordering::Relaxed);
    v as *mut dear_imgui_rs::sys::ImFont
}

/// Install the built-in JetBrains Mono font **with MDI icons** into the ImGui
/// font atlas and register it as the code-editor font.
///
/// This is the recommended one-call setup.  It:
/// 1. Adds JetBrains Mono NL as a new atlas font (becomes the default if it's
///    the first font added).
/// 2. Merges Material Design Icons into the same font so that icon constants
///    from [`crate::icons`] render correctly (context menus, find bar, etc.).
/// 3. Stores the resulting `ImFont*` in [`CODE_EDITOR_FONT_PTR`] for use by
///    every [`CodeEditor`](super::CodeEditor) instance.
///
/// Call **once** at startup, **before** the renderer builds the font atlas.
///
/// # Example
///
/// ```rust,ignore
/// use dear_imgui_custom_mod::code_editor::install_code_editor_font;
///
/// let mut context = dear_imgui_rs::Context::create();
/// install_code_editor_font(&mut context, 15.0 * hidpi);
/// // … create renderer (builds font atlas) …
/// // CodeEditor now uses JetBrains Mono + MDI icons automatically.
/// ```
pub fn install_code_editor_font(ctx: &mut dear_imgui_rs::Context, size_pixels: f32) {
    install_code_editor_font_ex(ctx, size_pixels, BuiltinFont::default());
}

/// Like [`install_code_editor_font`] but lets you choose which built-in font
/// variant to use.
pub fn install_code_editor_font_ex(
    ctx: &mut dear_imgui_rs::Context,
    size_pixels: f32,
    font: BuiltinFont,
) {
    let ptr = crate::fonts::install_monospace(ctx, font, size_pixels, true);
    if !ptr.is_null() {
        CODE_EDITOR_FONT_PTR.store(ptr as usize, Ordering::SeqCst);
    }
}

/// Install a custom TTF font from raw bytes and register it as the code-editor
/// font, with MDI icons merged in.
pub fn install_custom_code_editor_font(
    ctx: &mut dear_imgui_rs::Context,
    data: &[u8],
    size_pixels: f32,
    name: &str,
) {
    let ptr = crate::fonts::install_ui_font(ctx, data, size_pixels, name, true);
    if !ptr.is_null() {
        CODE_EDITOR_FONT_PTR.store(ptr as usize, Ordering::SeqCst);
    }
}
