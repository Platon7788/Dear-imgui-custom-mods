//! Dear ImGui context and renderer initialisation.

use std::sync::Arc;

use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use winit::window::Window;

use super::super::config::{AppConfigV2, FontChoiceV2, FontLayerV2, GlyphRangesV2};

// Codepoint ranges baked into the atlas, mirroring Dear ImGui's
// `ImFontGlyphRanges*` presets. We inline them so our public
// [`GlyphRangesV2`] enum is decoupled from upstream deprecations
// (ImGui 1.92+ favours dynamic on-demand loading; the classic preset
// ranges remain perfectly valid Unicode blocks).
//
// Each table is `[start, end, …, 0]` — the trailing zero terminates the
// list as Dear ImGui expects. Pairs are inclusive.
const RANGES_DEFAULT: &[u32] = &[0x0020, 0x00FF, 0];
const RANGES_CYRILLIC: &[u32] = &[
    0x0020, 0x00FF, 0x0400, 0x052F, 0x2DE0, 0x2DFF, 0xA640, 0xA69F, 0,
];
const RANGES_JAPANESE: &[u32] = &[
    0x0020, 0x00FF, 0x3000, 0x30FF, 0x31F0, 0x31FF, 0xFF00, 0xFFEF, 0,
];
const RANGES_CHINESE_SIMPLIFIED: &[u32] = &[
    0x0020, 0x00FF, 0x2000, 0x206F, 0x3000, 0x30FF, 0x31F0, 0x31FF, 0xFF00, 0xFFEF, 0x4E00, 0x9FAF,
    0,
];
const RANGES_CHINESE_TRADITIONAL: &[u32] = RANGES_CHINESE_SIMPLIFIED;
const RANGES_KOREAN: &[u32] = &[0x0020, 0x00FF, 0x3131, 0x3163, 0xAC00, 0xD7A3, 0];
const RANGES_THAI: &[u32] = &[0x0020, 0x00FF, 0x0E00, 0x0E7F, 0];
const RANGES_VIETNAMESE: &[u32] = &[
    0x0020, 0x00FF, 0x0102, 0x0103, 0x0110, 0x0111, 0x0128, 0x0129, 0x0168, 0x0169, 0x01A0, 0x01A1,
    0x01AF, 0x01B0, 0x1EA0, 0x1EF9, 0,
];

pub(crate) fn init_imgui(
    window: &Arc<Window>,
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    cfg: &AppConfigV2,
) -> (dear_imgui_rs::Context, WinitPlatform, WgpuRenderer) {
    let mut context = dear_imgui_rs::Context::create();
    let _ = context.set_ini_filename(None::<std::path::PathBuf>);

    // Wire the system clipboard so InputText copy/paste reaches the OS
    // buffer (Ctrl+C / Ctrl+V). Without this, the framework's UI feels
    // half-broken for anyone who tries to copy text *out* of the app.
    context.set_clipboard_backend(crate::clipboard_backend::SystemClipboardBackend);

    let mut platform = WinitPlatform::new(&mut context);
    platform.attach_window(window, HiDpiMode::Default, &mut context);

    let hidpi = (window.scale_factor() as f32).clamp(1.0, 3.0);
    let scaled = (cfg.font_size * hidpi).round();
    context.io_mut().set_font_global_scale(1.0 / hidpi);

    match &cfg.font {
        FontChoiceV2::Builtin(b) => {
            add_single_font(&mut context, b.data(), b.display_name(), scaled);
        }
        FontChoiceV2::Bytes(bytes) => {
            add_single_font(&mut context, bytes, "user", scaled);
        }
        FontChoiceV2::Stack(layers) => {
            add_font_stack(&mut context, layers, hidpi);
        }
    }

    if cfg.merge_mdi_icons {
        crate::fonts::merge_mdi_icons(&mut context, scaled);
    }

    cfg.theme.apply_imgui_style(context.style_mut());

    let renderer = WgpuRenderer::new(
        WgpuInitInfo {
            instance: Some(instance),
            adapter: Some(adapter),
            device,
            queue,
            num_frames_in_flight: 3,
            render_target_format: surface_format,
            depth_stencil_format: None,
            pipeline_multisample_state: wgpu::MultisampleState::default(),
        },
        &mut context,
    )
    .expect("WgpuRenderer::new");
    (context, platform, renderer)
}

/// Rebuild the ImGui font atlas at a new HiDPI scale.
///
/// Called from the framework's `ScaleFactorChanged` handler when the
/// user drags the window between monitors of differing DPI. Without
/// this, fonts stay rendered at the old physical pixel size and look
/// blurry / aliased on the new monitor until the app is restarted.
///
/// Steps:
/// 1. `clear_fonts` — drops every previously-added `ImFont`.
/// 2. Re-add fonts using the stored `cfg.font` and the new `hidpi`.
/// 3. `invalidate_device_objects` — flushes the wgpu renderer's
///    pipeline / texture cache so the next frame re-uploads the
///    rebuilt atlas. (`dear_imgui_wgpu` sets
///    `BackendFlags::RENDERER_HAS_TEXTURES` so the atlas itself
///    rebuilds on demand; only the renderer-side caches need
///    invalidation.)
/// 4. Update the global font scale on the IO so layout scales 1:1
///    with the new hidpi.
///
/// The next two frames are added to the pending budget so the rebuilt
/// atlas actually reaches the screen even in event-driven mode.
pub(crate) fn rebuild_fonts_for_scale(
    context: &mut dear_imgui_rs::Context,
    renderer: &mut WgpuRenderer,
    cfg: &AppConfigV2,
    new_hidpi: f32,
) {
    let new_hidpi = new_hidpi.clamp(1.0, 3.0);
    let scaled = (cfg.font_size * new_hidpi).round();

    // 1. Drop every pre-existing ImFont (not the texture data — that
    //    rebuilds on demand via RENDERER_HAS_TEXTURES).
    context.fonts().clear_fonts();

    // 2. Re-add fonts at the new scale.
    match &cfg.font {
        FontChoiceV2::Builtin(b) => {
            add_single_font(context, b.data(), b.display_name(), scaled);
        }
        FontChoiceV2::Bytes(bytes) => {
            add_single_font(context, bytes, "user", scaled);
        }
        FontChoiceV2::Stack(layers) => {
            add_font_stack(context, layers, new_hidpi);
        }
    }
    if cfg.merge_mdi_icons {
        crate::fonts::merge_mdi_icons(context, scaled);
    }

    // 3. Flush wgpu renderer cache. Texture manager is rebuilt lazily
    //    next frame, so this is cheap.
    let _ = renderer.invalidate_device_objects();

    // 4. Reciprocal global scale so widget metrics keep their logical
    //    sizes (they are multiplied by `display_framebuffer_scale`).
    context.io_mut().set_font_global_scale(1.0 / new_hidpi);
}

// ── Font helpers ─────────────────────────────────────────────────────────────

/// Add a single TTF as the base font.
fn add_single_font(
    context: &mut dear_imgui_rs::Context,
    bytes: &[u8],
    name: &str,
    scaled_size: f32,
) {
    context.fonts().add_font_from_memory_ttf(
        bytes,
        scaled_size,
        Some(
            &dear_imgui_rs::FontConfig::new()
                .size_pixels(scaled_size)
                .oversample_h(2)
                .name(name),
        ),
        None,
    );
}

/// Resolve a [`GlyphRangesV2`] into the borrowed (or owned) `&[u32]`
/// slice Dear ImGui expects — `[lo, hi, lo, hi, …, 0]` with a `0`
/// terminator. Caller passes a `Vec<u32>` reference for the `Custom`
/// variant to back the temporary slice.
///
/// Returning a borrow keeps callers allocation-free for the preset
/// variants (just hand back our `RANGES_*` `const`s), while `Custom`
/// fills the supplied `Vec` and returns a slice into it.
pub(super) fn resolve_glyph_ranges<'v>(
    ranges: &GlyphRangesV2,
    scratch: &'v mut Vec<u32>,
) -> &'v [u32] {
    match ranges {
        GlyphRangesV2::Default => RANGES_DEFAULT,
        GlyphRangesV2::Cyrillic => RANGES_CYRILLIC,
        GlyphRangesV2::Japanese => RANGES_JAPANESE,
        GlyphRangesV2::ChineseSimplified => RANGES_CHINESE_SIMPLIFIED,
        GlyphRangesV2::ChineseTraditional => RANGES_CHINESE_TRADITIONAL,
        GlyphRangesV2::Korean => RANGES_KOREAN,
        GlyphRangesV2::Thai => RANGES_THAI,
        GlyphRangesV2::Vietnamese => RANGES_VIETNAMESE,
        GlyphRangesV2::Custom(pairs) => {
            scratch.clear();
            scratch.extend(pairs.iter().flat_map(|[a, b]| [*a, *b]));
            scratch.push(0);
            scratch.as_slice()
        }
    }
}

/// Bake a [`FontChoiceV2::Stack`] into the atlas: first layer is the base,
/// subsequent layers are merged on top so their glyphs (icons / CJK /
/// math) overlay the base font's metrics.
fn add_font_stack(context: &mut dear_imgui_rs::Context, layers: &[FontLayerV2], hidpi: f32) {
    if layers.is_empty() {
        // Empty stack — fall back to default builtin so the atlas isn't empty.
        let fallback = crate::fonts::BuiltinFont::Hack;
        let scaled = (15.0 * hidpi).round();
        add_single_font(context, fallback.data(), fallback.display_name(), scaled);
        return;
    }
    for (idx, layer) in layers.iter().enumerate() {
        let scaled = (layer.size * hidpi).round();
        let merge = idx > 0 && layer.merge;
        let mut scratch: Vec<u32> = Vec::new();
        let ranges_slice = resolve_glyph_ranges(&layer.glyph_ranges, &mut scratch);
        let ranges_ptr: Option<&[u32]> = Some(ranges_slice);
        let cfg = dear_imgui_rs::FontConfig::new()
            .size_pixels(scaled)
            .oversample_h(2)
            .merge_mode(merge)
            .name(if merge { "stack-merge" } else { "stack-base" });
        context
            .fonts()
            .add_font_from_memory_ttf(&layer.bytes, scaled, Some(&cfg), ranges_ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_default_ends_with_zero_terminator() {
        // Every preset must end with a `0` terminator so Dear ImGui knows
        // where the range list stops.
        for preset in [
            RANGES_DEFAULT,
            RANGES_CYRILLIC,
            RANGES_JAPANESE,
            RANGES_CHINESE_SIMPLIFIED,
            RANGES_CHINESE_TRADITIONAL,
            RANGES_KOREAN,
            RANGES_THAI,
            RANGES_VIETNAMESE,
        ] {
            assert!(!preset.is_empty(), "preset must be non-empty");
            assert_eq!(*preset.last().unwrap(), 0, "preset must end with 0");
        }
    }

    #[test]
    fn preset_lengths_are_pairs_plus_one() {
        // Each preset is `[lo,hi, lo,hi, …, 0]` — len = pairs*2 + 1, so odd.
        for preset in [
            RANGES_DEFAULT,
            RANGES_CYRILLIC,
            RANGES_JAPANESE,
            RANGES_CHINESE_SIMPLIFIED,
            RANGES_KOREAN,
            RANGES_THAI,
            RANGES_VIETNAMESE,
        ] {
            assert_eq!(preset.len() % 2, 1, "preset len must be odd");
        }
    }

    #[test]
    fn resolve_custom_ranges_appends_zero_terminator() {
        let custom = GlyphRangesV2::Custom(vec![[0xF0001, 0xF1FFF], [0x4E00, 0x9FAF]]);
        let mut scratch = Vec::new();
        let s = resolve_glyph_ranges(&custom, &mut scratch);
        assert_eq!(s, &[0xF0001, 0xF1FFF, 0x4E00, 0x9FAF, 0]);
    }

    #[test]
    fn resolve_default_borrows_static() {
        let mut scratch = Vec::new();
        let s = resolve_glyph_ranges(&GlyphRangesV2::Default, &mut scratch);
        assert_eq!(s, RANGES_DEFAULT);
        assert!(scratch.is_empty(), "preset path should not touch scratch");
    }

    #[test]
    fn resolve_empty_custom_only_terminator() {
        let custom = GlyphRangesV2::Custom(Vec::new());
        let mut scratch = Vec::new();
        let s = resolve_glyph_ranges(&custom, &mut scratch);
        assert_eq!(s, &[0u32]);
    }

    #[test]
    fn resolve_overwrites_scratch_on_reuse() {
        let mut scratch = vec![1, 2, 3]; // garbage from prior call
        let custom = GlyphRangesV2::Custom(vec![[0xA, 0xB]]);
        let s = resolve_glyph_ranges(&custom, &mut scratch);
        assert_eq!(s, &[0xA, 0xB, 0]); // garbage gone
    }
}
