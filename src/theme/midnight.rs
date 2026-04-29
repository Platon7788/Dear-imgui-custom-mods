//! Midnight theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Near-black OLED-friendly palette inspired by Tokyo Night / VS Code Dark+.
//! Maximum contrast between chrome and content: window bg is almost true
//! black, child/frame surfaces step up in very small increments so deep
//! hierarchies stay readable.

use super::palettes::{DisasmViewColors, DisasmViewTokens, HexViewerColors, HexViewerTokens};
use super::{DialogColors, NavColors, StatusBarColors, TitlebarColors};
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette ─────────────────────────────────────────────────────────────────

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Surfaces — graduated near-blacks.
const BG: u32 = 0x0e0f13;
const BG_CHILD: u32 = 0x151722;
const BG_POPUP: u32 = 0x151722;
const BG_FRAME: u32 = 0x1f2230;
const BG_FRAME_HOVER: u32 = 0x262a3b;
const BG_FRAME_ACTIVE: u32 = 0x2d3246;

const BORDER: u32 = 0x2a2e3e;
const SEPARATOR: u32 = 0x25283a;

// Foreground.
const FG: u32 = 0xd8dae0;
const FG_MUTED: u32 = 0x6f7280;
const FG_DISABLED: u32 = 0x4a4d59;

// Titlebar surfaces (slightly darker than window bg).
const TITLE_BG: u32 = 0x0a0b0f;
const TITLE_INACTIVE_BG: u32 = 0x070809;

// Accent — Tokyo Night blue.
const ACCENT: u32 = 0x7aa2f7;
const ACCENT_HOVER: u32 = 0x89b4ff;
const ACCENT_ACTIVE: u32 = 0x6691e8;

// Secondary (non-accented widgets).
const SECONDARY: u32 = 0x25283a;
const SECONDARY_HOVER: u32 = 0x323649;
#[allow(dead_code)]
const SECONDARY_ACTIVE: u32 = 0x3d4257;

// Semantic colors.
const DANGER: u32 = 0xf7768e;
const SUCCESS: u32 = 0x9ece6a;
const WARNING: u32 = 0xe0af68;

const STATUSBAR_BG: u32 = 0x08090c;
const NAV_BADGE_BG: u32 = 0xf7768e;
const TAB_ACTIVE_BG: u32 = 0x1f2230;

// ─── Titlebar ────────────────────────────────────────────────────────────────

pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(TITLE_BG, 1.0);
    let icon = hex(FG_MUTED, 1.0);
    TitlebarColors {
        bg,
        separator: hex(BORDER, 1.0),
        title: icon,
        // Vex0r-style accent palette: amber/cyan/red.
        btn_minimize: hex(0xfcbf00, 1.0),
        btn_maximize: hex(0x4fc3f7, 1.0),
        btn_close: hex(0xef5350, 1.0),
        btn_hover_bg: hex(SECONDARY_HOVER, 0.85),
        btn_close_hover_bg: hex(DANGER, 0.85),
        icon,
        bg_erase: bg,
        drag_hint: hex(SECONDARY_HOVER, 0.35),
        bg_inactive: hex(TITLE_INACTIVE_BG, 1.0),
        title_inactive: hex(FG_DISABLED, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

pub fn nav_colors() -> NavColors {
    let bg = hex(STATUSBAR_BG, 1.0);
    let btn_hover = hex(SECONDARY_HOVER, 1.0);
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
        overlay: [0.0, 0.0, 0.0, 0.65],
        bg: bg_float,
        border: hex(BORDER, 1.0),
        title: hex(FG, 1.0),
        message: hex(FG_MUTED, 1.0),
        separator: hex(BORDER, 1.0),
        icon_warning: hex(WARNING, 1.0),
        icon_error: hex(DANGER, 1.0),
        icon_info: hex(ACCENT, 1.0),
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
        bg: hex(STATUSBAR_BG, 1.0),
        text: hex(FG, 1.0),
        text_dim: hex(FG_MUTED, 1.0),
        separator: hex(BORDER, 1.0),
        hover: hex(SECONDARY_HOVER, 1.0),
        active: hex(SECONDARY_HOVER, 1.0),
        success: hex(SUCCESS, 1.0),
        warning: hex(WARNING, 1.0),
        error: hex(DANGER, 1.0),
        info: hex(ACCENT, 1.0),
    }
}

// ─── HexViewer ───────────────────────────────────────────────────────────────

/// Hex-viewer palette for the Midnight (Tokyo Night-inspired) theme.
pub fn hex_viewer_colors() -> HexViewerColors {
    HexViewerColors::from_tokens(&HexViewerTokens {
        fg: hex(FG, 1.0),
        fg_muted: hex(FG_MUTED, 1.0),
        accent: hex(ACCENT, 1.0),
        success: hex(SUCCESS, 1.0),
        warning: hex(WARNING, 1.0),
        danger: hex(DANGER, 1.0),
        purple: hex(0x9d7cd8, 1.0),
    })
}

// ─── DisasmView ──────────────────────────────────────────────────────────────

/// Disassembly-view palette for the Midnight (Tokyo Night-inspired) theme.
/// Tokyo-Night-style hues — saturated jewel tones over near-black surfaces.
pub fn disasm_view_colors() -> DisasmViewColors {
    DisasmViewColors::from_tokens(&DisasmViewTokens {
        fg: hex(FG, 1.0),
        fg_muted: hex(FG_MUTED, 1.0),
        accent: hex(ACCENT, 1.0),
        success: hex(SUCCESS, 1.0),
        warning: hex(WARNING, 1.0),
        danger: hex(DANGER, 1.0),
        purple: hex(0x9d7cd8, 1.0),
        // Tokyo Night canonical orange + cyan.
        orange: hex(0xff9e64, 1.0),
        cyan: hex(0x7dcfff, 1.0),
    })
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
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.65));

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

    style.set_color(StyleColor::ScrollbarBg, hex(TITLE_BG, 0.6));
    style.set_color(StyleColor::ScrollbarGrab, hex(SECONDARY, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(SECONDARY_HOVER, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::Button, hex(ACCENT, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    style.set_color(StyleColor::Header, hex(SECONDARY, 0.85));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.7));

    style.set_color(StyleColor::Separator, hex(SEPARATOR, 0.85));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.2));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.8));

    style.set_color(StyleColor::Tab, hex(SECONDARY, 0.85));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::TabSelected, hex(TAB_ACTIVE_BG, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(SECONDARY, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(SECONDARY_HOVER, 0.7));

    style.set_color(StyleColor::PlotLines, hex(ACCENT, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(DANGER, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(SUCCESS, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(WARNING, 1.0));

    style.set_color(StyleColor::TableHeaderBg, hex(BG_CHILD, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(BORDER, 0.9));
    style.set_color(StyleColor::TableBorderLight, hex(BORDER, 0.5));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(FG, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.8));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
