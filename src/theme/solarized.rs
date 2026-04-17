//! Solarized Dark theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Based on Ethan Schoonover's Solarized Precision Colors palette — a
//! warm, low-contrast scheme designed for sustained reading. Surfaces use
//! the base03 / base02 teals; text sits at base1 for a softened white.
//! Accent is Solarized blue (#268bd2), semantic colors are the published
//! red / green / yellow.

#[cfg(feature = "borderless_window")]
use crate::borderless_window::TitlebarColors;
#[cfg(feature = "confirm_dialog")]
use crate::confirm_dialog::DialogColors;
#[cfg(feature = "nav_panel")]
use crate::nav_panel::NavColors;
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette — Solarized Precision Colors ────────────────────────────────────

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Solarized base tones (dark variant).
const BASE03: u32 = 0x002b36; // window bg
const BASE02: u32 = 0x073642; // child / highlight bg
const BASE01: u32 = 0x586e75; // comments / muted
const BASE00: u32 = 0x657b83; // body text
const BASE0: u32 = 0x839496; // default content
const BASE1: u32 = 0x93a1a1; // emphasized content

// Solarized accents.
const YELLOW: u32 = 0xb58900;
const ORANGE: u32 = 0xcb4b16;
const RED: u32 = 0xdc322f;
const BLUE: u32 = 0x268bd2;
const CYAN: u32 = 0x2aa198;
const GREEN: u32 = 0x859900;

// Derived frame + hover variants.
const BG_FRAME: u32 = 0x0d4655;
const BG_FRAME_HOVER: u32 = 0x135363;
const BG_FRAME_ACTIVE: u32 = 0x18607a;

// ─── Titlebar ────────────────────────────────────────────────────────────────

#[cfg(feature = "borderless_window")]
pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(BASE02, 1.0);
    let icon = hex(BASE00, 1.0);
    let icon_light = hex(BASE1, 1.0);
    TitlebarColors {
        bg,
        separator: hex(BASE01, 0.60),
        title: icon,
        btn_minimize: icon_light,
        btn_maximize: icon_light,
        btn_close: icon_light,
        btn_hover_bg: hex(BG_FRAME, 0.85),
        btn_close_hover_bg: hex(RED, 0.85),
        icon,
        bg_erase: bg,
        drag_hint: hex(BG_FRAME, 0.35),
        bg_inactive: hex(BASE03, 1.0),
        title_inactive: hex(BASE01, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

#[cfg(feature = "nav_panel")]
pub fn nav_colors() -> NavColors {
    let bg = hex(BASE03, 1.0);
    let btn_hover = hex(BG_FRAME, 1.0);
    let sep = hex(BASE01, 0.60);
    let accent = hex(BLUE, 1.0);
    let icon_active = hex(BASE1, 1.0);
    NavColors {
        bg,
        btn_hover,
        btn_active: btn_hover,
        indicator: accent,
        icon_default: hex(BASE00, 1.0),
        icon_active,
        separator: sep,
        badge_bg: hex(RED, 1.0),
        badge_text: [1.0, 1.0, 1.0, 1.0],
        submenu_bg: hex(BASE02, 1.0),
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(BASE00, 1.0),
    }
}

// ─── Confirm dialog ──────────────────────────────────────────────────────────

#[cfg(feature = "confirm_dialog")]
pub fn dialog_colors() -> DialogColors {
    let bg = hex(BASE02, 1.0);
    let bg_float = [
        (bg[0] + 0.03).min(1.0),
        (bg[1] + 0.03).min(1.0),
        (bg[2] + 0.03).min(1.0),
        1.0,
    ];
    let confirm_red = [0.70, 0.22, 0.22, 1.0];
    let cancel_green = [0.18, 0.52, 0.35, 1.0];
    DialogColors {
        overlay: [0.0, 0.0, 0.0, 0.55],
        bg: bg_float,
        border: hex(BASE01, 0.80),
        title: hex(BASE1, 1.0),
        message: hex(BASE0, 1.0),
        separator: hex(BASE01, 0.60),
        icon_warning: hex(YELLOW, 1.0),
        icon_error: hex(RED, 1.0),
        icon_info: hex(BLUE, 1.0),
        icon_question: [0.70, 0.62, 0.86, 1.0],
        btn_confirm: confirm_red,
        btn_confirm_hover: [0.82, 0.30, 0.30, 1.0],
        btn_confirm_active: [0.60, 0.18, 0.18, 1.0],
        btn_confirm_text: [1.0, 1.0, 1.0, 1.0],
        btn_cancel: cancel_green,
        btn_cancel_hover: [0.24, 0.58, 0.40, 1.0],
        btn_cancel_active: [0.14, 0.44, 0.28, 1.0],
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
        color_bg: hex(BASE03, 1.0),
        color_text: hex(BASE1, 1.0),
        color_text_dim: hex(BASE00, 1.0),
        color_separator: hex(BASE01, 0.60),
        color_hover: hex(BG_FRAME, 0.60),
        color_active: hex(BG_FRAME, 0.90),
        color_success: hex(GREEN, 1.0),
        color_warning: hex(YELLOW, 1.0),
        color_error: hex(RED, 1.0),
        color_info: hex(CYAN, 1.0),
    }
}

// ─── ImGui style ─────────────────────────────────────────────────────────────

pub fn apply_imgui_style(style: &mut Style) {
    style.set_window_rounding(4.0);
    style.set_frame_rounding(3.0);
    style.set_child_rounding(3.0);
    style.set_popup_rounding(4.0);
    style.set_scrollbar_rounding(3.0);
    style.set_grab_rounding(2.0);
    style.set_tab_rounding(3.0);

    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_child_border_size(1.0);
    style.set_popup_border_size(1.0);
    style.set_scrollbar_size(12.0);
    style.set_grab_min_size(8.0);
    style.set_frame_padding([6.0, 4.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([4.0, 4.0]);

    style.set_color(StyleColor::WindowBg, hex(BASE03, 1.0));
    style.set_color(StyleColor::ChildBg, hex(BASE02, 0.0));
    style.set_color(StyleColor::PopupBg, hex(BASE02, 0.97));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.55));

    style.set_color(StyleColor::Text, hex(BASE1, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(BASE01, 1.0));

    style.set_color(StyleColor::Border, hex(BASE01, 0.70));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    style.set_color(StyleColor::FrameBg, hex(BG_FRAME, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(BG_FRAME_HOVER, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(BG_FRAME_ACTIVE, 1.0));

    style.set_color(StyleColor::TitleBg, hex(BASE02, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(BASE02, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(BASE02, 0.75));

    style.set_color(StyleColor::MenuBarBg, hex(BASE02, 1.0));

    style.set_color(StyleColor::ScrollbarBg, hex(BASE03, 0.6));
    style.set_color(StyleColor::ScrollbarGrab, hex(BG_FRAME, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(BG_FRAME_HOVER, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(BLUE, 1.0));

    style.set_color(StyleColor::Button, hex(BLUE, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(CYAN, 0.95));
    style.set_color(StyleColor::ButtonActive, hex(BLUE, 1.0));

    style.set_color(StyleColor::Header, hex(BG_FRAME, 0.85));
    style.set_color(StyleColor::HeaderHovered, hex(BLUE, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(BLUE, 0.7));

    style.set_color(StyleColor::Separator, hex(BASE01, 0.60));
    style.set_color(StyleColor::SeparatorHovered, hex(BLUE, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(BLUE, 1.0));

    style.set_color(StyleColor::ResizeGrip, hex(BLUE, 0.2));
    style.set_color(StyleColor::ResizeGripHovered, hex(BLUE, 0.5));
    style.set_color(StyleColor::ResizeGripActive, hex(BLUE, 0.8));

    style.set_color(StyleColor::Tab, hex(BG_FRAME, 0.85));
    style.set_color(StyleColor::TabHovered, hex(BLUE, 0.55));
    style.set_color(StyleColor::TabSelected, hex(BG_FRAME_ACTIVE, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(BG_FRAME, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(BG_FRAME_HOVER, 0.7));

    style.set_color(StyleColor::PlotLines, hex(BLUE, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(ORANGE, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(GREEN, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(YELLOW, 1.0));

    style.set_color(StyleColor::TableHeaderBg, hex(BASE02, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(BASE01, 0.80));
    style.set_color(StyleColor::TableBorderLight, hex(BASE01, 0.40));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    style.set_color(StyleColor::TextSelectedBg, hex(BLUE, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(BLUE, 0.9));
    style.set_color(StyleColor::NavCursor, hex(BLUE, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(BASE1, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(BLUE, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(BLUE, 0.8));
    style.set_color(StyleColor::SliderGrabActive, hex(CYAN, 1.0));
}
