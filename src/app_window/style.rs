//! ImGui style derivation from a [`TitlebarTheme`].

use crate::borderless_window::TitlebarTheme;
use dear_imgui_rs::StyleColor;

/// Apply a full Dear ImGui colour palette derived from the titlebar theme.
///
/// Called once at startup (or after a theme change) — not per frame.
pub(super) fn apply_imgui_style_for_theme(
    theme: &TitlebarTheme,
    s: &mut dear_imgui_rs::Style,
) {
    let c = theme.colors();
    let bg = c.bg;

    // Slightly lighter variants of the background colour.
    let bg1 = clamp_add(bg, 0.03);
    let bg2 = clamp_add(bg, 0.07);
    let bg3 = clamp_add(bg, 0.12);
    // Use maximize button colour as the accent.
    let acc = c.btn_maximize;
    let sep = c.separator;

    s.set_window_rounding(0.0);
    s.set_frame_rounding(3.0);
    s.set_scrollbar_rounding(4.0);
    s.set_window_border_size(0.0);

    s.set_color(StyleColor::WindowBg,    bg);
    s.set_color(StyleColor::ChildBg,     bg1);
    s.set_color(StyleColor::PopupBg, [
        (bg[0] + 0.03).min(1.0),
        (bg[1] + 0.03).min(1.0),
        (bg[2] + 0.05).min(1.0),
        0.97,
    ]);
    s.set_color(StyleColor::Border,          [sep[0], sep[1], sep[2], 0.60]);
    s.set_color(StyleColor::FrameBg,         bg2);
    s.set_color(StyleColor::FrameBgHovered,  bg3);
    s.set_color(StyleColor::FrameBgActive,   clamp_add(bg3, 0.04));
    s.set_color(StyleColor::Button,          bg2);
    s.set_color(StyleColor::ButtonHovered,   bg3);
    s.set_color(StyleColor::ButtonActive,    acc);
    s.set_color(StyleColor::Header,          bg2);
    s.set_color(StyleColor::HeaderHovered,   bg3);
    s.set_color(StyleColor::Separator,       [sep[0], sep[1], sep[2], 0.60]);
    s.set_color(StyleColor::Text,            c.title);
    s.set_color(StyleColor::TextDisabled, [
        c.title[0] * 0.55,
        c.title[1] * 0.55,
        c.title[2] * 0.55,
        1.0,
    ]);
    s.set_color(StyleColor::SliderGrab,        acc);
    s.set_color(StyleColor::SliderGrabActive,  clamp_add(acc, 0.1));
    s.set_color(StyleColor::CheckMark,         acc);
    s.set_color(StyleColor::ScrollbarBg, [
        (bg[0] - 0.03).max(0.0),
        (bg[1] - 0.03).max(0.0),
        (bg[2] - 0.02).max(0.0),
        0.60,
    ]);
    s.set_color(StyleColor::ScrollbarGrab,        bg2);
    s.set_color(StyleColor::ScrollbarGrabHovered, bg3);
    s.set_color(StyleColor::ScrollbarGrabActive,  acc);
    s.set_color(StyleColor::ResizeGrip,        [bg2[0], bg2[1], bg2[2], 0.40]);
    s.set_color(StyleColor::ResizeGripHovered, [acc[0], acc[1], acc[2], 0.80]);
    s.set_color(StyleColor::ResizeGripActive,  acc);
    s.set_color(StyleColor::Tab,                      bg1);
    s.set_color(StyleColor::TabHovered,               bg3);
    s.set_color(StyleColor::TabSelected,              bg2);
    s.set_color(StyleColor::TabDimmed,                bg1);
    s.set_color(StyleColor::TabDimmedSelected,        bg2);
    s.set_color(StyleColor::HeaderActive,             acc);
    s.set_color(StyleColor::TitleBg,           bg);
    s.set_color(StyleColor::TitleBgActive,     bg1);
    s.set_color(StyleColor::TextSelectedBg,    [acc[0], acc[1], acc[2], 0.40]);
    s.set_color(StyleColor::ModalWindowDimBg,  [0.0, 0.0, 0.0, 0.60]);
}

/// Add `delta` to the RGB channels of a colour, clamping to `[0, 1]`.
/// The source alpha channel is preserved.
#[inline]
fn clamp_add(c: [f32; 4], delta: f32) -> [f32; 4] {
    [
        (c[0] + delta).clamp(0.0, 1.0),
        (c[1] + delta).clamp(0.0, 1.0),
        (c[2] + delta).clamp(0.0, 1.0),
        c[3],
    ]
}
