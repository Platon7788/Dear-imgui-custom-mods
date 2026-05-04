//! # fonts
//!
//! Centralised font registry for `dear-imgui-custom-mod`.
//!
//! Bundles three monospace TTFs (Hack, JetBrains Mono, JetBrains Mono NL) and
//! Material Design Icons (MDI v7.4) as `const &[u8]` blobs, and exposes a
//! handful of one-call helpers for installing them into a Dear ImGui atlas
//! with MDI icons merged in.
//!
//! ```rust,no_run
//! use dear_imgui_custom_mod::fonts::{install_monospace, install_ui_font, BuiltinFont};
//!
//! let mut ctx = dear_imgui_rs::Context::create();
//!
//! // One-call: install Hack (monospace) + MDI icons
//! let hack = install_monospace(&mut ctx, BuiltinFont::Hack, 15.0, true);
//!
//! // Or install a custom UI font (e.g. Inter) with MDI merged:
//! const INTER: &[u8] = b""; // include_bytes!("Inter.ttf");
//! let ui = install_ui_font(&mut ctx, INTER, 15.0, "Inter", true);
//! ```

#![allow(missing_docs)] // TODO: per-module doc-coverage pass — see CONTRIBUTING.md
use dear_imgui_rs::{Context, FontConfig, sys::ImFont};

// ─── Bundled font data ───────────────────────────────────────────────────────

/// JetBrains Mono NL Regular — a premium monospace font optimized for code.
/// No-ligature variant (NL) since Dear ImGui does not support ligatures.
/// License: SIL Open Font License 1.1. ~204 KB.
pub const JETBRAINS_MONO_FONT_DATA: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMonoNL-Regular.ttf");

/// JetBrains Mono Regular — same font **with** ligature tables.
/// The ligature data is ignored by ImGui but the glyph metrics differ slightly.
/// Prefer [`JETBRAINS_MONO_FONT_DATA`] (NL) for smaller binary size.
pub const JETBRAINS_MONO_LIGATURES_FONT_DATA: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

/// Hack Regular — a typeface designed for source code, highly legible at
/// common sizes. License: MIT + Bitstream Vera. ~302 KB.
pub const HACK_FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/Hack-Regular.ttf");

/// Material Design Icons (MDI v7.4) — 7447 icons in the Private Use Area
/// (U+F0000–U+F1FFF). Merge this via [`merge_mdi_icons`] or the `merge_mdi`
/// flag on the install helpers below. ~1.3 MB.
pub const MDI_FONT_DATA: &[u8] = include_bytes!("../../assets/materialdesignicons-webfont.ttf");

/// MDI glyph range for ImGui font merging: `[start, end, 0]` (null-terminated).
///
/// Pass this to `add_font_from_memory_ttf(..., Some(MDI_GLYPH_RANGES))` when
/// merging the icon font manually.
pub const MDI_GLYPH_RANGES: &[u32] = &[0xF0000, 0xF1FFF, 0];

// ─── Built-in monospace fonts ────────────────────────────────────────────────

/// Built-in monospace font choice for code editors / terminals / hex views.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFont {
    /// Hack — highly legible monospace font, excellent for code at all sizes.
    #[default]
    Hack,
    /// JetBrains Mono NL (No Ligatures) — smallest binary footprint.
    JetBrainsMonoNL,
    /// JetBrains Mono (with ligature tables — ignored by ImGui).
    JetBrainsMono,
}

impl BuiltinFont {
    /// All built-in font variants.
    pub const ALL: &'static [BuiltinFont] =
        &[Self::Hack, Self::JetBrainsMonoNL, Self::JetBrainsMono];

    /// Raw TTF bytes for this font variant.
    pub fn data(self) -> &'static [u8] {
        match self {
            Self::Hack => HACK_FONT_DATA,
            Self::JetBrainsMonoNL => JETBRAINS_MONO_FONT_DATA,
            Self::JetBrainsMono => JETBRAINS_MONO_LIGATURES_FONT_DATA,
        }
    }

    /// Human-readable name (used for the `FontConfig.name` field).
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Hack => "Hack",
            Self::JetBrainsMonoNL => "JetBrains Mono NL",
            Self::JetBrainsMono => "JetBrains Mono",
        }
    }
}

// ─── Installation helpers ────────────────────────────────────────────────────

/// Install one of the built-in monospace fonts into the Dear ImGui atlas and
/// (optionally) merge MDI icons into the same glyph table.
///
/// Call **once** at startup **before** the renderer builds the font atlas.
/// Returns the raw `ImFont*` pointer (0 if the font failed to add).
pub fn install_monospace(
    ctx: &mut Context,
    font: BuiltinFont,
    size_pixels: f32,
    merge_mdi: bool,
) -> *mut ImFont {
    let cfg = FontConfig::new()
        .size_pixels(size_pixels)
        .oversample_h(1)
        .oversample_v(1)
        .pixel_snap_h(true)
        .name(font.display_name());
    let ptr = ctx
        .fonts()
        .add_font_from_memory_ttf(font.data(), size_pixels, Some(&cfg), None)
        .map(|f| f.raw())
        .unwrap_or(std::ptr::null_mut());

    if merge_mdi {
        merge_mdi_icons(ctx, size_pixels);
    }
    ptr
}

/// Install a custom UI font from raw TTF bytes, optionally with MDI icons merged.
///
/// Returns the raw `ImFont*` pointer (0 if the font failed to add).
pub fn install_ui_font(
    ctx: &mut Context,
    data: &[u8],
    size_pixels: f32,
    name: &str,
    merge_mdi: bool,
) -> *mut ImFont {
    let cfg = FontConfig::new()
        .size_pixels(size_pixels)
        .oversample_h(1)
        .oversample_v(1)
        .pixel_snap_h(true)
        .name(name);
    let ptr = ctx
        .fonts()
        .add_font_from_memory_ttf(data, size_pixels, Some(&cfg), None)
        .map(|f| f.raw())
        .unwrap_or(std::ptr::null_mut());

    if merge_mdi {
        merge_mdi_icons(ctx, size_pixels);
    }
    ptr
}

/// Merge Material Design Icons into the **last** font added to the atlas.
///
/// Useful when the caller installed its own font and just wants the MDI icon
/// glyphs merged in afterwards (the MDI codepoints live in the Private Use
/// Area U+F0000–U+F1FFF so they never collide with Latin / CJK glyphs).
pub fn merge_mdi_icons(ctx: &mut Context, size_pixels: f32) {
    let mdi_cfg = FontConfig::new()
        .size_pixels(size_pixels)
        .merge_mode(true)
        .name("MDI Icons");
    ctx.fonts().add_font_from_memory_ttf(
        MDI_FONT_DATA,
        size_pixels,
        Some(&mdi_cfg),
        Some(MDI_GLYPH_RANGES),
    );
}

/// Returns the raw `dear_imgui_rs::FontSource` for MDI icons at the given size
/// — useful when building a multi-source font in a single `add_font` call.
pub fn mdi_font_source(size_pixels: f32) -> dear_imgui_rs::FontSource<'static> {
    dear_imgui_rs::FontSource::TtfData {
        data: MDI_FONT_DATA,
        size_pixels: Some(size_pixels),
        config: Some(
            FontConfig::new()
                .size_pixels(size_pixels)
                .merge_mode(true)
                .name("MDI Icons"),
        ),
    }
}
