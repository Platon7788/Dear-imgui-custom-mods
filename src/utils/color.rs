//! Color helpers for ImGui draw list operations.
//!
//! Packs u8/f32 RGBA into the `u32` format expected by `ImDrawList`
//! (ABGR layout: alpha in bits 24-31, blue 16-23, green 8-15, red 0-7).

/// Pack RGBA (u8) into u32 matching ImColor32 (ABGR) layout.
#[inline]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Fully opaque RGB -> u32 (alpha = 0xFF).
#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    rgba(r, g, b, 0xFF)
}

/// Pack RGBA (f32 0.0-1.0, clamped) into u32.
#[inline]
pub fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> u32 {
    rgba(
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Pack `[f32; 4]` RGBA array into u32 (convenience for ImGui color arrays).
#[inline]
pub fn pack_color_f32(c: [f32; 4]) -> u32 {
    rgba_f32(c[0], c[1], c[2], c[3])
}

/// Pack `[u8; 3]` RGB array + alpha into u32.
#[inline]
pub const fn rgb_arr(c: [u8; 3], a: u8) -> u32 {
    rgba(c[0], c[1], c[2], a)
}

/// Linearly blend two RGBA colors. `t = 0.0` → `a`, `t = 1.0` → `b`.
#[inline]
pub fn blend_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Multiply the alpha component of `c` by `a`, leaving RGB unchanged.
#[inline]
pub fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
}

// ── sRGB ↔ linear gamma conversions ──────────────────────────────────────────
//
// `Theme::window_bg()` and the rest of our palette constants are
// **sRGB-encoded** floats in `0..=1` (per IEC 61966-2-1) — that is what
// ImGui's `StyleColor::*` consumers expect, and what the typical
// "0xRRGGBB / 255.0" hex literal yields.
//
// `wgpu::Color` for clear-pass operations is **linear-space**: when the
// swap-chain format is `*UnormSrgb`, the GPU writes the clear value
// directly into the framebuffer **without** any conversion. So a value
// of `0.184` (= sRGB hex `0x2F`) ends up encoded into a sRGB texel as
// `0.184` linear, which the display gamma function then expands to
// roughly sRGB hex `0x79` — the well-known "fog" / "washed-out" bug
// we ran into after enabling `WindowFlags::NO_BACKGROUND` on the
// `app_window` root window.
//
// Use [`srgb_to_linear`] when feeding sRGB floats into a `wgpu::Color`
// destined for a sRGB-formatted swap chain.

/// Convert a single sRGB-encoded channel value (`0..=1`) to its linear
/// equivalent, using the IEC 61966-2-1 piecewise transfer function.
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Apply [`srgb_to_linear`] componentwise to an `[r, g, b, a]` array,
/// leaving the alpha channel untouched (alpha is already linear in the
/// sRGB transfer function).
#[inline]
pub fn srgba_to_linear(c: [f32; 4]) -> [f32; 4] {
    [
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
        c[3],
    ]
}

/// Returns `true` for swap-chain formats that the GPU samples from the
/// framebuffer through a sRGB transfer (i.e. clear values are
/// interpreted as **linear** and need conversion from sRGB inputs).
#[inline]
pub fn is_srgb_surface_format(fmt: wgpu::TextureFormat) -> bool {
    use wgpu::TextureFormat as F;
    matches!(
        fmt,
        F::Bgra8UnormSrgb
            | F::Rgba8UnormSrgb
            | F::Bc1RgbaUnormSrgb
            | F::Bc2RgbaUnormSrgb
            | F::Bc3RgbaUnormSrgb
            | F::Bc7RgbaUnormSrgb
            | F::Etc2Rgb8UnormSrgb
            | F::Etc2Rgb8A1UnormSrgb
            | F::Etc2Rgba8UnormSrgb
    )
}

/// Build a [`wgpu::Color`] suitable for a clear-pass `LoadOp::Clear`,
/// taking the swap-chain format into account.
///
/// - sRGB-formatted surface (`*UnormSrgb`): the input sRGB floats are
///   converted to linear-space so the clear value, after the GPU's
///   sRGB encode, lands at the same byte values the ImGui style
///   would have painted via `StyleColor::WindowBg`.
/// - Non-sRGB / Unorm surface: pass-through — the input floats are
///   used directly because the framebuffer stores raw values.
///
/// Use this for every `wgpu::Color` derived from a theme palette
/// instead of hand-converting in each call site.
#[inline]
pub fn wgpu_clear_color(rgba: [f32; 4], fmt: wgpu::TextureFormat) -> wgpu::Color {
    let [r, g, b, a] = if is_srgb_surface_format(fmt) {
        srgba_to_linear(rgba)
    } else {
        rgba
    };
    wgpu::Color {
        r: r as f64,
        g: g as f64,
        b: b as f64,
        a: a as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_to_linear_endpoints() {
        // sRGB transfer is identity at 0 and 1.
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_to_linear_low_branch() {
        // Below 0.04045 → straight divide by 12.92 (linear toe).
        let v = 0.02;
        assert!((srgb_to_linear(v) - v / 12.92).abs() < 1e-7);
    }

    #[test]
    fn srgb_to_linear_high_branch() {
        // Above 0.04045 → power curve `((c + 0.055) / 1.055) ^ 2.4`.
        // Spot-check `0x2F` (= 47 / 255 ≈ 0.1843), the red channel of
        // `Theme::Dark::BG`. Expected linear value computed by hand:
        //   ((0.1843 + 0.055) / 1.055)^2.4 ≈ 0.02843
        let srgb = 0x2F as f32 / 255.0;
        let lin = srgb_to_linear(srgb);
        assert!(
            (lin - 0.028_43).abs() < 1e-4,
            "sRGB 0x2F → linear {lin}, expected ~0.02843"
        );
    }

    #[test]
    fn srgb_to_linear_clamps_out_of_range() {
        // Negative / over-range inputs must not panic.
        assert_eq!(srgb_to_linear(-1.0), 0.0);
        assert!((srgb_to_linear(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgba_to_linear_keeps_alpha() {
        // Alpha is already linear in the sRGB transfer — must pass through.
        let out = srgba_to_linear([0.5, 0.5, 0.5, 0.42]);
        assert_eq!(out[3], 0.42);
    }

    #[test]
    fn is_srgb_surface_format_identifies_srgb_swap_chains() {
        // Every flavour that wgpu surfaces expose for a normal framebuffer.
        assert!(is_srgb_surface_format(wgpu::TextureFormat::Bgra8UnormSrgb));
        assert!(is_srgb_surface_format(wgpu::TextureFormat::Rgba8UnormSrgb));
        // Linear / non-sRGB variants must be rejected.
        assert!(!is_srgb_surface_format(wgpu::TextureFormat::Bgra8Unorm));
        assert!(!is_srgb_surface_format(wgpu::TextureFormat::Rgba8Unorm));
        assert!(!is_srgb_surface_format(wgpu::TextureFormat::Rgba16Float));
    }

    #[test]
    fn wgpu_clear_color_converts_for_srgb_surface() {
        // sRGB surface → input is converted, clear value lands at the
        // *linear* value of the source sRGB channel.
        let color = wgpu_clear_color(
            [0x2F as f32 / 255.0, 0.0, 0.0, 1.0],
            wgpu::TextureFormat::Bgra8UnormSrgb,
        );
        assert!(
            (color.r - 0.028_43).abs() < 1e-4,
            "sRGB surface should convert: r = {}",
            color.r
        );
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn wgpu_clear_color_passes_through_for_unorm_surface() {
        // Non-sRGB surface → no conversion, raw value stored verbatim
        // (modulo `f32 → f64` widening which is exact for these values).
        let color = wgpu_clear_color([0.184, 0.5, 0.7, 1.0], wgpu::TextureFormat::Bgra8Unorm);
        assert!((color.r - 0.184_f32 as f64).abs() < 1e-7);
        assert!((color.g - 0.5).abs() < 1e-7);
        assert!((color.b - 0.7_f32 as f64).abs() < 1e-7);
        assert_eq!(color.a, 1.0);
    }
}
