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
