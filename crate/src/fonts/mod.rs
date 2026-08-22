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
use dear_imgui_rs::{Context, FontConfig, FontSource, sys::ImFont};

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

/// MDI glyph range: `[start, end, 0]` (null-terminated), covering the Material
/// Design Icons Private-Use Area.
///
/// Retained for backward compatibility. With Dear ImGui 1.92+ (dear-imgui-rs
/// 0.17) fonts load glyphs on demand, so merging the icon font no longer needs
/// an explicit inclusive range — [`merge_mdi_icons`] omits it.
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

// ── Raw-pointer bridge for dear-imgui-rs 0.17 ────────────────────────────────
//
// 0.17 replaced the raw `*mut ImFont` font model with an opaque, atlas-validated
// `FontId` that is deliberately `!Send + !Sync` and hides its inner pointer.
// This crate, however, stores a *process-global* raw `*mut ImFont` (see
// `crate::code_editor::CODE_EDITOR_FONT_PTR`, a `usize` atomic) and pushes it
// through the `igPushFont` / `ImFont_CalcTextSizeA` FFI, so it cannot use
// `FontId` directly. These two helpers add fonts through the safe
// `FontAtlas::add_font` API (which handles `FontConfig` cleanly) and then
// recover the raw pointer straight from the atlas font vector.

/// Raw `*mut ImFont` of the font most recently added to `ctx`'s atlas, or null
/// if the atlas holds no fonts.
fn last_font_ptr(ctx: &Context) -> *mut ImFont {
    let atlas = ctx.font_atlas().raw();
    // SAFETY: `atlas` is the live atlas owned by `ctx`; `Fonts` is a valid
    // `ImVector<*mut ImFont>` for the atlas lifetime and its entries are the
    // non-null `ImFont*` created by `add_font`. Merge-mode sources do not append
    // a new entry, so the last slot is always the base font just added.
    unsafe {
        let fonts = &(*atlas).Fonts;
        if fonts.Size > 0 {
            *fonts.Data.add((fonts.Size - 1) as usize)
        } else {
            std::ptr::null_mut()
        }
    }
}

/// Add a single, already-configured TTF font to `ctx`'s atlas and return its raw
/// `*mut ImFont`.
fn add_ttf_font(ctx: &Context, data: &[u8], size_pixels: f32, cfg: FontConfig) -> *mut ImFont {
    // SAFETY: `ttf_data_with_size` is `unsafe` only because Dear ImGui's TTF
    // parser has no guaranteed input-boundary check; the data is a valid TTF
    // blob that lives at least as long as this call (Dear ImGui copies it into
    // the atlas).
    let source = unsafe { FontSource::ttf_data_with_size(data, size_pixels) }.with_config(cfg);
    ctx.font_atlas().add_font(&[source]);
    last_font_ptr(ctx)
}

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
    let ptr = add_ttf_font(ctx, font.data(), size_pixels, cfg);

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
    let ptr = add_ttf_font(ctx, data, size_pixels, cfg);

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
    // No explicit glyph range: Dear ImGui 1.92+ loads glyphs on demand, so the
    // MDI PUA codepoints (U+F0000–U+F1FFF) are baked only when actually drawn.
    let source =
        unsafe { FontSource::ttf_data_with_size(MDI_FONT_DATA, size_pixels) }.with_config(mdi_cfg);
    ctx.font_atlas().add_font(&[source]);
}

/// Returns the raw `dear_imgui_rs::FontSource` for MDI icons at the given size
/// — useful when building a multi-source font in a single `add_font` call.
pub fn mdi_font_source(size_pixels: f32) -> dear_imgui_rs::FontSource<'static> {
    let config = FontConfig::new()
        .size_pixels(size_pixels)
        .merge_mode(true)
        .name("MDI Icons");
    // SAFETY: MDI_FONT_DATA is a valid 'static TTF blob embedded at build time.
    unsafe { FontSource::ttf_data_with_size(MDI_FONT_DATA, size_pixels) }.with_config(config)
}

// ─── Unicode coverage fallback (CJK, Hebrew, Arabic, box-drawing) ────────────

/// Load a font file from `C:\Windows\Fonts\<filename>` and leak the bytes so
/// the ImGui atlas can keep referencing them for the entire process lifetime
/// (font atlas needs `'static` data). Returns `None` if `WINDIR` is unset,
/// the file doesn't exist, or it can't be read.
///
/// Windows-only: always returns `None` on other platforms.
#[cfg(windows)]
fn load_windows_font(filename: &str) -> Option<&'static [u8]> {
    let windir = std::env::var_os("WINDIR")?;
    let path = std::path::PathBuf::from(windir)
        .join("Fonts")
        .join(filename);
    let bytes = std::fs::read(&path).ok()?;
    Some(Box::leak(bytes.into_boxed_slice()))
}

#[cfg(not(windows))]
fn load_windows_font(_filename: &str) -> Option<&'static [u8]> {
    None
}

/// Merge an extra font into the **last** font added to the atlas with no
/// explicit glyph range — the Dear ImGui 1.92+ dynamic glyph loader picks up
/// codepoints on demand, so atlas memory grows only when a glyph is actually
/// rendered (e.g. a Chinese chat line arrives in the packet view).
///
/// Uses `merge_mode(true)` so the glyphs land in the existing UI font's glyph
/// table — text using the primary `ImFont*` automatically falls back to this
/// font for codepoints that the primary's cmap doesn't cover.
pub fn merge_fallback_font(ctx: &mut Context, size_pixels: f32, data: &'static [u8], name: &str) {
    let cfg = FontConfig::new()
        .size_pixels(size_pixels)
        .merge_mode(true)
        .oversample_h(1)
        .oversample_v(1)
        .pixel_snap_h(true)
        .name(name);
    let source = unsafe { FontSource::ttf_data_with_size(data, size_pixels) }.with_config(cfg);
    ctx.font_atlas().add_font(&[source]);
}

/// Merge a curated chain of fallback fonts into the **last** font added to
/// the atlas, expanding coverage to box-drawing, CJK (Simplified Chinese,
/// Traditional Chinese, Japanese, Korean), Hebrew and Arabic.
///
/// The bundled `Hack-Regular.ttf` is always merged first so box-drawing
/// glyphs (U+2500–U+257F) work even on a stripped Windows install where no
/// system fonts are reachable. Then a list of common Windows system fonts
/// is tried in priority order:
///
/// | File           | Coverage                          |
/// |----------------|-----------------------------------|
/// | seguisym.ttf   | Symbols, box-drawing, dingbats    |
/// | tahoma.ttf     | Hebrew, Arabic, broad symbol set  |
/// | msyh.ttc       | Microsoft YaHei — Simplified CN   |
/// | msjh.ttc       | Microsoft JhengHei — Traditional CN |
/// | YuGothM.ttc    | Yu Gothic Medium — Japanese (W10+)|
/// | meiryo.ttc     | Meiryo — Japanese (W7+)           |
/// | malgun.ttf     | Malgun Gothic — Korean            |
///
/// Each system font is loaded from `C:\Windows\Fonts\` on demand; missing
/// files are silently skipped, so the call is always safe regardless of
/// which language packs are installed.
///
/// Call **once** after installing the primary UI font, **before** the
/// renderer builds the atlas. Adds ~30 MB of leaked TTF data on a typical
/// Windows install (msyh.ttc alone is ~17 MB) — acceptable for a desktop
/// app that needs to display arbitrary localized strings from network
/// packets without garbling them as `?`.
pub fn install_unicode_fallbacks(ctx: &mut Context, size_pixels: f32) {
    // 1. Bundled Hack — covers box-drawing (U+2500–U+257F) and a few extra
    //    Latin / Cyrillic glyphs Inter may not have. Always available
    //    because Hack is embedded in this crate (~302 KB).
    merge_fallback_font(ctx, size_pixels, HACK_FONT_DATA, "Hack (fallback)");

    // 2. Windows system fonts — order matters for ambiguous codepoints:
    //    ImGui's merge mode picks the first font that has the glyph, so
    //    high-coverage broad fonts (Tahoma, Segoe UI Symbol) come before
    //    region-specific CJK fonts. Within CJK, Simplified Chinese is
    //    tried before Traditional to bias toward the more common script.
    let candidates: &[(&str, &str)] = &[
        ("seguisym.ttf", "Segoe UI Symbol"),
        ("tahoma.ttf", "Tahoma"),
        ("msyh.ttc", "Microsoft YaHei"),
        ("msjh.ttc", "Microsoft JhengHei"),
        ("YuGothM.ttc", "Yu Gothic"),
        ("meiryo.ttc", "Meiryo"),
        ("malgun.ttf", "Malgun Gothic"),
    ];

    for &(file, name) in candidates {
        if let Some(data) = load_windows_font(file) {
            merge_fallback_font(ctx, size_pixels, data, name);
        }
    }
}
