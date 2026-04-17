//! Dark theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! This is the native NxT palette lifted into the library so any application
//! that picks `Theme::Dark` (the default) gets a consistent,
//! fully-tuned dark look across every component.

#[cfg(feature = "borderless_window")]
use crate::borderless_window::TitlebarColors;
#[cfg(feature = "confirm_dialog")]
use crate::confirm_dialog::DialogColors;
#[cfg(feature = "nav_panel")]
use crate::nav_panel::NavColors;
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette ─────────────────────────────────────────────────────────────────

/// Hex `0xRRGGBB` + alpha → `[r, g, b, a]` with components in `0.0..=1.0`.
const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

const BG: u32 = 0x2f343e;
const BG_CHILD: u32 = 0x31353d;
const BG_POPUP: u32 = 0x31353d;
const BORDER: u32 = 0x3f4654;
const FG: u32 = 0xe0e4ea;
const FG_MUTED: u32 = 0x8a92a1;

const ACCENT: u32 = 0x5b9bd5;
const ACCENT_HOVER: u32 = 0x6baadf;
const ACCENT_ACTIVE: u32 = 0x4a8ac4;

const SECONDARY: u32 = 0x3b414d;
const SECONDARY_HOVER: u32 = 0x48505e;
const SECONDARY_ACTIVE: u32 = 0x555e6e;

const DANGER: u32 = 0xe06060;
const SUCCESS: u32 = 0x5fb870;
const WARNING: u32 = 0xd9a643;

const SCROLLBAR_BG: u32 = 0x2f343e;
const SCROLLBAR_GRAB: u32 = 0x48505e;
const SCROLLBAR_HOVER: u32 = 0x5a6375;

const TITLE_BG: u32 = 0x31353d;
const TAB_ACTIVE_BG: u32 = 0x3b414d;

const STATUSBAR_BG: u32 = 0x2b2f38;
const NAV_BADGE_BG: u32 = 0xc03030;

// ─── Titlebar ────────────────────────────────────────────────────────────────

/// Titlebar colors for this theme.
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
        btn_hover_bg: hex(SECONDARY_HOVER, 0.85),
        btn_close_hover_bg: hex(DANGER, 0.90),
        icon,
        bg_erase: bg,
        drag_hint: hex(SECONDARY_HOVER, 0.35),
        bg_inactive: hex(0x2a2d34, 1.0),
        title_inactive: hex(0x606672, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

/// Nav-panel colors for this theme.
#[cfg(feature = "nav_panel")]
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
        submenu_bg: bg,
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(FG_MUTED, 1.0),
    }
}

// ─── Confirm dialog ─────────────────────────────────────────────────────────

/// Confirm-dialog colors for this theme.
///
/// The dialog background is deliberately one step lighter than the titlebar
/// bg so the modal "floats" above the window content. Confirm / Cancel
/// buttons use semantic colors (red / green) so severity stays obvious.
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
        overlay: [0.0, 0.0, 0.0, 0.55],
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

/// Default status-bar config for this theme.
#[cfg(feature = "status_bar")]
pub fn statusbar_config() -> StatusBarConfig {
    StatusBarConfig {
        height: 22.0,
        item_padding: 10.0,
        separator_width: 1.0,
        show_separators: false,
        color_bg: hex(STATUSBAR_BG, 1.0),
        color_text: hex(FG, 1.0),
        color_text_dim: hex(FG_MUTED, 1.0),
        color_separator: hex(BORDER, 1.0),
        color_hover: hex(SECONDARY_HOVER, 0.60),
        color_active: hex(SECONDARY_HOVER, 0.90),
        color_success: hex(SUCCESS, 1.0),
        color_warning: hex(WARNING, 1.0),
        color_error: hex(DANGER, 1.0),
        color_info: hex(ACCENT, 1.0),
    }
}

// ─── ImGui style ─────────────────────────────────────────────────────────────

/// Apply the full Dear ImGui style for this theme.
///
/// Sets rounding + sizing + a complete color palette for every
/// `StyleColor` variant used by the built-in widgets. Call once at startup
/// (or any time after a theme change).
pub fn apply_imgui_style(style: &mut Style) {
    // ── Rounding ──
    style.set_window_rounding(4.0);
    style.set_frame_rounding(3.0);
    style.set_child_rounding(3.0);
    style.set_popup_rounding(4.0);
    style.set_scrollbar_rounding(3.0);
    style.set_grab_rounding(2.0);
    style.set_tab_rounding(3.0);

    // ── Sizing ──
    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_child_border_size(1.0);
    style.set_popup_border_size(1.0);
    style.set_scrollbar_size(12.0);
    style.set_grab_min_size(8.0);
    style.set_frame_padding([6.0, 4.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([4.0, 4.0]);

    // ── Backgrounds ──
    style.set_color(StyleColor::WindowBg, hex(BG, 1.0));
    style.set_color(StyleColor::ChildBg, hex(BG_CHILD, 0.0));
    style.set_color(StyleColor::PopupBg, hex(BG_POPUP, 0.96));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.50));

    // ── Text ──
    style.set_color(StyleColor::Text, hex(FG, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(FG_MUTED, 0.6));

    // ── Borders ──
    style.set_color(StyleColor::Border, hex(BORDER, 0.70));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    // ── Frame ──
    style.set_color(StyleColor::FrameBg, hex(SECONDARY, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(SECONDARY_HOVER, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(SECONDARY_ACTIVE, 1.0));

    // ── Title bar ──
    style.set_color(StyleColor::TitleBg, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(TITLE_BG, 0.75));

    // ── Menu bar ──
    style.set_color(StyleColor::MenuBarBg, hex(BG_CHILD, 1.0));

    // ── Scrollbar ──
    style.set_color(StyleColor::ScrollbarBg, hex(SCROLLBAR_BG, 0.5));
    style.set_color(StyleColor::ScrollbarGrab, hex(SCROLLBAR_GRAB, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(SCROLLBAR_HOVER, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    // ── Buttons ──
    style.set_color(StyleColor::Button, hex(ACCENT, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    // ── Headers ──
    style.set_color(StyleColor::Header, hex(SECONDARY, 0.8));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.7));

    // ── Separator ──
    style.set_color(StyleColor::Separator, hex(BORDER, 0.5));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    // ── Resize grip ──
    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.2));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.8));

    // ── Tabs ──
    style.set_color(StyleColor::Tab, hex(SECONDARY, 0.85));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::TabSelected, hex(TAB_ACTIVE_BG, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(SECONDARY, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(SECONDARY_HOVER, 0.7));

    // ── Plot / Charts ──
    style.set_color(StyleColor::PlotLines, hex(ACCENT, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(DANGER, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(SUCCESS, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(WARNING, 1.0));

    // ── Tables ──
    style.set_color(StyleColor::TableHeaderBg, hex(BG_CHILD, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(BORDER, 0.8));
    style.set_color(StyleColor::TableBorderLight, hex(BORDER, 0.4));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    // ── Misc ──
    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(FG, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.8));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
