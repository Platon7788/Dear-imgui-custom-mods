//! Monokai Pro theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Wimer Hazenberg's Monokai Pro palette — warm charcoal surfaces with
//! bright neon accents (pink, green, cyan, orange, yellow). Built for
//! contrast-rich syntax highlighting; equally readable for UI chrome.

#[cfg(feature = "borderless_window")]
use crate::borderless_window::TitlebarColors;
#[cfg(feature = "confirm_dialog")]
use crate::confirm_dialog::DialogColors;
#[cfg(feature = "nav_panel")]
use crate::nav_panel::NavColors;
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette — Monokai Pro ───────────────────────────────────────────────────

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Surfaces.
const BG: u32 = 0x2d2a2e; // window bg — warm charcoal
const BG_CHILD: u32 = 0x221f22; // child / panel
const BG_POPUP: u32 = 0x221f22;
const BG_FRAME: u32 = 0x403e41; // input / combobox
const BG_FRAME_HOVER: u32 = 0x4a484b;
const BG_FRAME_ACTIVE: u32 = 0x5b595c;

const BORDER: u32 = 0x5b595c;
const SEPARATOR: u32 = 0x4a484b;

// Foreground.
const FG: u32 = 0xfcfcfa;
const FG_MUTED: u32 = 0x939293;
const FG_DISABLED: u32 = 0x6c696d;

// Titlebar bg — one step darker than window to separate chrome.
const TITLE_BG: u32 = 0x221f22;
const TITLE_INACTIVE_BG: u32 = 0x1a171a;

// Monokai neon accents.
const RED: u32 = 0xff6188; // hot pink
#[allow(dead_code)]
const ORANGE: u32 = 0xfc9867;
const YELLOW: u32 = 0xffd866;
const GREEN: u32 = 0xa9dc76;
const CYAN: u32 = 0x78dce8;
const PURPLE: u32 = 0xab9df2;

// Primary accent — cyan (widely used in Monokai buttons / selections).
const ACCENT: u32 = CYAN;
const ACCENT_HOVER: u32 = 0x92e4ee;
const ACCENT_ACTIVE: u32 = 0x5bccdb;

// Status bar surface = titlebar bg (consistent chrome colour).
const STATUSBAR_BG: u32 = 0x1a171a;
const NAV_BADGE_BG: u32 = RED;

// ─── Titlebar ────────────────────────────────────────────────────────────────

#[cfg(feature = "borderless_window")]
pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(TITLE_BG, 1.0);
    let icon = hex(FG_MUTED, 1.0);
    let icon_light = hex(FG, 1.0);
    TitlebarColors {
        bg,
        separator: hex(BORDER, 1.0),
        title: icon,
        btn_minimize: icon_light,
        btn_maximize: icon_light,
        btn_close: icon_light,
        btn_hover_bg: hex(BG_FRAME_HOVER, 0.85),
        btn_close_hover_bg: hex(RED, 0.85),
        icon,
        bg_erase: bg,
        drag_hint: hex(BG_FRAME_HOVER, 0.35),
        bg_inactive: hex(TITLE_INACTIVE_BG, 1.0),
        title_inactive: hex(FG_DISABLED, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

#[cfg(feature = "nav_panel")]
pub fn nav_colors() -> NavColors {
    let bg = hex(STATUSBAR_BG, 1.0);
    let btn_hover = hex(BG_FRAME_HOVER, 1.0);
    let sep = hex(BORDER, 1.0);
    let accent = hex(ACCENT, 1.0);
    let icon_active = hex(FG, 1.0);
    NavColors {
        bg,
        btn_hover,
        btn_active: btn_hover,
        indicator: accent,
        icon_default: hex(FG_MUTED, 1.0),
        icon_active,
        separator: sep,
        badge_bg: hex(NAV_BADGE_BG, 1.0),
        badge_text: [1.0, 1.0, 1.0, 1.0],
        submenu_bg: hex(BG_CHILD, 1.0),
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(FG_MUTED, 1.0),
    }
}

// ─── Confirm dialog ──────────────────────────────────────────────────────────

#[cfg(feature = "confirm_dialog")]
pub fn dialog_colors() -> DialogColors {
    let bg = hex(BG_CHILD, 1.0);
    let bg_float = [
        (bg[0] + 0.04).min(1.0),
        (bg[1] + 0.04).min(1.0),
        (bg[2] + 0.04).min(1.0),
        1.0,
    ];
    let confirm_red = [0.70, 0.22, 0.22, 1.0];
    let cancel_green = [0.18, 0.52, 0.35, 1.0];
    DialogColors {
        overlay: [0.0, 0.0, 0.0, 0.60],
        bg: bg_float,
        border: hex(BORDER, 1.0),
        title: hex(FG, 1.0),
        message: hex(FG_MUTED, 1.0),
        separator: hex(SEPARATOR, 1.0),
        icon_warning: hex(YELLOW, 1.0),
        icon_error: hex(RED, 1.0),
        icon_info: hex(CYAN, 1.0),
        icon_question: hex(PURPLE, 1.0),
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
        highlight_hover: false,
        color_bg: hex(STATUSBAR_BG, 1.0),
        color_text: hex(FG, 1.0),
        color_text_dim: hex(FG_MUTED, 1.0),
        color_separator: hex(BORDER, 1.0),
        color_hover: hex(BG_FRAME_HOVER, 0.60),
        color_active: hex(BG_FRAME_HOVER, 0.90),
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

    style.set_color(StyleColor::WindowBg, hex(BG, 1.0));
    style.set_color(StyleColor::ChildBg, hex(BG_CHILD, 0.0));
    style.set_color(StyleColor::PopupBg, hex(BG_POPUP, 0.97));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.60));

    style.set_color(StyleColor::Text, hex(FG, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(FG_DISABLED, 1.0));

    style.set_color(StyleColor::Border, hex(BORDER, 0.85));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    style.set_color(StyleColor::FrameBg, hex(BG_FRAME, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(BG_FRAME_HOVER, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(BG_FRAME_ACTIVE, 1.0));

    style.set_color(StyleColor::TitleBg, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(TITLE_BG, 0.75));

    style.set_color(StyleColor::MenuBarBg, hex(BG_CHILD, 1.0));

    style.set_color(StyleColor::ScrollbarBg, hex(STATUSBAR_BG, 0.6));
    style.set_color(StyleColor::ScrollbarGrab, hex(BG_FRAME, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(BG_FRAME_HOVER, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::Button, hex(ACCENT, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    style.set_color(StyleColor::Header, hex(BG_FRAME, 0.85));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.7));

    style.set_color(StyleColor::Separator, hex(SEPARATOR, 0.80));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.25));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.55));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.85));

    style.set_color(StyleColor::Tab, hex(BG_FRAME, 0.85));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::TabSelected, hex(BG_FRAME_ACTIVE, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(BG_FRAME, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(BG_FRAME_HOVER, 0.7));

    style.set_color(StyleColor::PlotLines, hex(CYAN, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(RED, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(GREEN, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(YELLOW, 1.0));

    style.set_color(StyleColor::TableHeaderBg, hex(BG_CHILD, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(BORDER, 0.90));
    style.set_color(StyleColor::TableBorderLight, hex(BORDER, 0.50));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(FG, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.85));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
