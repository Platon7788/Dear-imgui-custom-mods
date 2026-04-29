//! Nord theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Arctic, north-bluish, *minimalist* palette by Sven Greb. Cool desaturated
//! greys (`polar-night` 0..3) for surfaces, ice-cool blues (`frost` 0..3) for
//! accents, and a small set of muted aurora colours for status (red, orange,
//! yellow, green, purple). Designed for long-form code reading; the contrast
//! is intentionally low compared to neon themes.
//!
//! Reference palette: <https://www.nordtheme.com/>

use super::palettes::{DisasmViewColors, DisasmViewTokens, HexViewerColors, HexViewerTokens};
use super::{DialogColors, NavColors, StatusBarColors, TitlebarColors};
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette — Nord ──────────────────────────────────────────────────────────

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Polar Night — surfaces, darkest → lightest.
const NORD0: u32 = 0x2e3440; // window bg
const NORD1: u32 = 0x3b4252; // child / titlebar
const NORD2: u32 = 0x434c5e; // input frame, separator
const NORD3: u32 = 0x4c566a; // borders, hover surfaces

// Snow Storm — foreground.
const NORD4: u32 = 0xd8dee9; // muted text
const NORD5: u32 = 0xe5e9f0; // primary text
#[allow(dead_code)]
const NORD6: u32 = 0xeceff4; // emphasis text

// Frost — cool blue accents (UI primary).
const NORD7: u32 = 0x8fbcbb; // teal
const NORD8: u32 = 0x88c0d0; // cyan-blue (typical accent)
#[allow(dead_code)]
const NORD9: u32 = 0x81a1c1; // soft blue
#[allow(dead_code)]
const NORD10: u32 = 0x5e81ac; // deep blue (reserved for future use)

// Aurora — status colours.
const NORD11: u32 = 0xbf616a; // red — error / destructive
const NORD12: u32 = 0xd08770; // orange — warning
const NORD13: u32 = 0xebcb8b; // yellow — minimize button
const NORD14: u32 = 0xa3be8c; // green — success / cancel
const NORD15: u32 = 0xb48ead; // purple — info / question

// Inactive titlebar.
const TITLE_INACTIVE_BG: u32 = 0x252a33;

// Primary accent — frost cyan-blue (matches Nord conventions).
const ACCENT: u32 = NORD8;
const ACCENT_HOVER: u32 = 0x9bcad9;
const ACCENT_ACTIVE: u32 = 0x6fa9c0;

// ─── Titlebar ────────────────────────────────────────────────────────────────

pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(NORD1, 1.0);
    let icon = hex(NORD4, 1.0);
    TitlebarColors {
        bg,
        separator: hex(NORD2, 1.0),
        title: hex(NORD5, 1.0),
        // Vex0r-style accents on the Nord aurora palette:
        // yellow / cyan-blue / red — desaturated to match Nord's low-contrast aesthetic.
        btn_minimize: hex(NORD13, 1.0),
        btn_maximize: hex(NORD8, 1.0),
        btn_close: hex(NORD11, 1.0),
        btn_hover_bg: hex(NORD3, 0.85),
        btn_close_hover_bg: hex(NORD11, 0.40),
        icon,
        bg_erase: bg,
        drag_hint: hex(NORD3, 0.35),
        bg_inactive: hex(TITLE_INACTIVE_BG, 1.0),
        title_inactive: hex(NORD3, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

pub fn nav_colors() -> NavColors {
    let bg = hex(NORD1, 1.0);
    let btn_hover = hex(NORD3, 1.0);
    // NORD3 separator (matches `statusbar_colors().separator`) — NORD2
    // sat too close to the NORD1 background to read as a divider.
    let sep = hex(NORD3, 1.0);
    let accent = hex(ACCENT, 1.0);
    let icon_active = hex(NORD5, 1.0);
    NavColors {
        bg,
        btn_hover,
        btn_active: btn_hover,
        indicator: accent,
        icon_default: hex(NORD4, 1.0),
        icon_active,
        separator: sep,
        badge_bg: hex(NORD11, 1.0),
        badge_text: [1.0, 1.0, 1.0, 1.0],
        submenu_bg: hex(NORD2, 1.0),
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(NORD4, 1.0),
    }
}

// ─── Confirm dialog ──────────────────────────────────────────────────────────

pub fn dialog_colors() -> DialogColors {
    let bg = hex(NORD1, 1.0);
    let bg_float = [
        (bg[0] + 0.04).min(1.0),
        (bg[1] + 0.04).min(1.0),
        (bg[2] + 0.04).min(1.0),
        1.0,
    ];
    DialogColors {
        overlay: [0.0, 0.0, 0.0, 0.55],
        bg: bg_float,
        border: hex(NORD3, 1.0),
        title: hex(NORD5, 1.0),
        message: hex(NORD4, 1.0),
        separator: hex(NORD2, 1.0),
        icon_warning: hex(NORD12, 1.0),
        icon_error: hex(NORD11, 1.0),
        icon_info: hex(NORD8, 1.0),
        icon_question: hex(NORD15, 1.0),
        // Restrained Nord red — softer than monokai's hot pink.
        btn_confirm: [0.65, 0.31, 0.36, 1.0],
        btn_confirm_hover: [0.74, 0.39, 0.44, 1.0],
        btn_confirm_active: [0.55, 0.24, 0.30, 1.0],
        btn_confirm_text: [1.0, 1.0, 1.0, 1.0],
        btn_cancel: [0.45, 0.62, 0.46, 1.0], // muted aurora green
        btn_cancel_hover: [0.52, 0.69, 0.53, 1.0],
        btn_cancel_active: [0.36, 0.54, 0.38, 1.0],
        btn_cancel_text: [1.0, 1.0, 1.0, 1.0],
    }
}

// ─── Status bar ──────────────────────────────────────────────────────────────

#[cfg(feature = "status_bar")]
pub fn statusbar_config() -> StatusBarConfig {
    StatusBarConfig {
        height: 22.0,
        item_padding: 10.0,
        separator_width: 1.0,
        show_separators: false,

        show_top_border: true,

        top_border_offset_left: 0.0,

        top_border_offset_right: 0.0,
        progress_width: 60.0,
        progress_height: 8.0,
        colors: statusbar_colors(),
    }
}

/// Status-bar colour subset for this theme.
pub fn statusbar_colors() -> StatusBarColors {
    StatusBarColors {
        bg: hex(NORD1, 1.0),
        text: hex(NORD5, 1.0),
        text_dim: hex(NORD4, 1.0),
        separator: hex(NORD3, 1.0),
        hover: hex(NORD3, 1.0),
        active: hex(NORD3, 1.0),
        success: hex(NORD14, 1.0),
        warning: hex(NORD12, 1.0),
        error: hex(NORD11, 1.0),
        info: hex(NORD8, 1.0),
    }
}

// ─── HexViewer ───────────────────────────────────────────────────────────────

/// Hex-viewer palette for the Nord theme — cool desaturated arctic colours.
pub fn hex_viewer_colors() -> HexViewerColors {
    HexViewerColors::from_tokens(&HexViewerTokens {
        fg: hex(NORD5, 1.0),
        fg_muted: hex(NORD4, 1.0),
        accent: hex(NORD8, 1.0),
        success: hex(NORD14, 1.0),
        warning: hex(NORD13, 1.0),
        danger: hex(NORD11, 1.0),
        // NORD15 — Nord's "purple" aurora colour.
        purple: hex(NORD15, 1.0),
    })
}

// ─── DisasmView ──────────────────────────────────────────────────────────────

/// Disassembly-view palette for the Nord theme — cool desaturated
/// arctic colours. NORD8 (cyan-blue) for address gutter, NORD14
/// (aurora green) for calls, NORD13 (yellow) for jumps, NORD11 (red)
/// for returns, NORD15 (purple) for stack ops, NORD12 (orange) for
/// syscall + memory, NORD7 (frost teal) for registers.
pub fn disasm_view_colors() -> DisasmViewColors {
    DisasmViewColors::from_tokens(&DisasmViewTokens {
        fg: hex(NORD5, 1.0),
        fg_muted: hex(NORD4, 1.0),
        accent: hex(NORD8, 1.0),
        success: hex(NORD14, 1.0),
        warning: hex(NORD13, 1.0),
        danger: hex(NORD11, 1.0),
        purple: hex(NORD15, 1.0),
        orange: hex(NORD12, 1.0),
        // NORD7 — frost teal, distinct from NORD8 cyan-blue accent.
        cyan: hex(NORD7, 1.0),
    })
}

// ─── ImGui style ─────────────────────────────────────────────────────────────

pub fn apply_imgui_style(style: &mut Style) {
    // Nord prefers slightly tighter rounding — minimalist, almost rectilinear.
    style.set_window_rounding(3.0);
    style.set_frame_rounding(2.0);
    style.set_child_rounding(2.0);
    style.set_popup_rounding(3.0);
    style.set_scrollbar_rounding(2.0);
    style.set_grab_rounding(2.0);
    style.set_tab_rounding(2.0);

    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_child_border_size(1.0);
    style.set_popup_border_size(1.0);
    style.set_scrollbar_size(12.0);
    style.set_grab_min_size(8.0);
    style.set_frame_padding([6.0, 4.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([4.0, 4.0]);

    style.set_color(StyleColor::WindowBg, hex(NORD0, 1.0));
    style.set_color(StyleColor::ChildBg, hex(NORD1, 0.0));
    style.set_color(StyleColor::PopupBg, hex(NORD1, 0.97));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.55));

    style.set_color(StyleColor::Text, hex(NORD5, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(NORD3, 1.0));

    style.set_color(StyleColor::Border, hex(NORD3, 0.85));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    style.set_color(StyleColor::FrameBg, hex(NORD2, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(NORD3, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(NORD3, 0.7));

    style.set_color(StyleColor::TitleBg, hex(NORD1, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(NORD1, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(NORD1, 0.75));

    style.set_color(StyleColor::MenuBarBg, hex(NORD1, 1.0));

    style.set_color(StyleColor::ScrollbarBg, hex(NORD0, 0.6));
    style.set_color(StyleColor::ScrollbarGrab, hex(NORD2, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(NORD3, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::Button, hex(ACCENT, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    style.set_color(StyleColor::Header, hex(NORD2, 0.85));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.7));

    style.set_color(StyleColor::Separator, hex(NORD2, 0.80));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.25));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.55));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.85));

    style.set_color(StyleColor::Tab, hex(NORD2, 0.85));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::TabSelected, hex(NORD3, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(NORD2, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(NORD3, 0.7));

    style.set_color(StyleColor::PlotLines, hex(NORD8, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(NORD15, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(NORD14, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(NORD13, 1.0));

    style.set_color(StyleColor::TableHeaderBg, hex(NORD1, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(NORD3, 0.90));
    style.set_color(StyleColor::TableBorderLight, hex(NORD2, 0.50));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(NORD5, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.85));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
