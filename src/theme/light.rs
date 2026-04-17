//! Light theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Designed to read well without washing everything out: frame / child
//! backgrounds are a noticeable step away from the window bg so input fields,
//! buttons and separators keep visible contrast (the previous Light palette
//! made borders and frame backgrounds almost invisible on a white surface).

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

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Surfaces: window bg is a soft warm grey, child / frame surfaces step down
// into progressively darker greys for clear layering.
const BG: u32 = 0xf1f2f5; // window background
const BG_CHILD: u32 = 0xe6e8ec; // child / popup background
const BG_POPUP: u32 = 0xe8ebef;
const BG_FRAME: u32 = 0xdadde3; // input / combobox / checkbox
const BG_FRAME_HOVER: u32 = 0xcfd3da;
const BG_FRAME_ACTIVE: u32 = 0xc4c9d1;

// Borders — deliberately dark enough to read against BG without being harsh.
const BORDER: u32 = 0xadb2bc;
const SEPARATOR: u32 = 0xbec2ca;

// Foreground text.
const FG: u32 = 0x1f232b;
const FG_MUTED: u32 = 0x5a606c;
const FG_DISABLED: u32 = 0x9096a2;

// Titlebar surfaces (a tiny step darker than the window bg so the bar reads
// as chrome rather than content).
const TITLE_BG: u32 = 0xe3e5ea;
const TITLE_INACTIVE_BG: u32 = 0xdbdde2;

// Accent — light-theme-optimised blue (saturated but not electric).
const ACCENT: u32 = 0x2e6fb8;
const ACCENT_HOVER: u32 = 0x3a82cf;
const ACCENT_ACTIVE: u32 = 0x255c9c;

// Secondary (for non-accented buttons / tabs).
const SECONDARY: u32 = 0xd3d6dc;
const SECONDARY_HOVER: u32 = 0xc2c6ce;
const SECONDARY_ACTIVE: u32 = 0xb3b8c1;

// Status / semantic colors.
const DANGER: u32 = 0xb43232;
const SUCCESS: u32 = 0x2a8a3c;
const WARNING: u32 = 0xa86a08;

// Status bar + nav badges stay constant across themes for meaning stability.
const STATUSBAR_BG: u32 = 0xd7dae0;
const NAV_BADGE_BG: u32 = 0xb43232;

// ─── Titlebar ────────────────────────────────────────────────────────────────

#[cfg(feature = "borderless_window")]
pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(TITLE_BG, 1.0);
    let icon = hex(FG_MUTED, 1.0);
    let icon_dark = hex(FG, 1.0);
    TitlebarColors {
        bg,
        separator: hex(BORDER, 1.0),
        title: icon_dark,
        btn_minimize: icon_dark,
        btn_maximize: icon_dark,
        btn_close: icon_dark,
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
        submenu_bg: hex(BG_CHILD, 1.0),
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(FG_MUTED, 1.0),
    }
}

// ─── Confirm dialog ─────────────────────────────────────────────────────────

#[cfg(feature = "confirm_dialog")]
pub fn dialog_colors() -> DialogColors {
    // Dialog background a notch darker than the window bg so the modal
    // reads as "above" rather than fused with the page.
    let bg = hex(BG_CHILD, 1.0);
    let bg_float = [
        (bg[0] - 0.03).max(0.0),
        (bg[1] - 0.03).max(0.0),
        (bg[2] - 0.03).max(0.0),
        1.0,
    ];
    let confirm_red = [0.70, 0.22, 0.22, 1.0];
    let cancel_green = [0.18, 0.52, 0.35, 1.0];
    DialogColors {
        overlay: [0.0, 0.0, 0.0, 0.40],
        bg: bg_float,
        border: hex(BORDER, 1.0),
        title: hex(FG, 1.0),
        message: hex(FG_MUTED, 1.0),
        separator: hex(BORDER, 1.0),
        icon_warning: hex(WARNING, 1.0),
        icon_error: hex(DANGER, 1.0),
        icon_info: hex(ACCENT, 1.0),
        icon_question: [0.52, 0.42, 0.72, 1.0],
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
        color_hover: hex(SECONDARY_HOVER, 0.60),
        color_active: hex(SECONDARY_HOVER, 0.90),
        color_success: hex(SUCCESS, 1.0),
        color_warning: hex(WARNING, 1.0),
        color_error: hex(DANGER, 1.0),
        color_info: hex(ACCENT, 1.0),
    }
}

// ─── ImGui style ─────────────────────────────────────────────────────────────

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
    style.set_frame_border_size(1.0); // visible borders on a light bg
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
    style.set_color(StyleColor::PopupBg, hex(BG_POPUP, 0.98));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.35));

    // ── Text ──
    style.set_color(StyleColor::Text, hex(FG, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(FG_DISABLED, 1.0));

    // ── Borders (darker than bg so they read clearly) ──
    style.set_color(StyleColor::Border, hex(BORDER, 0.85));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    // ── Frame ──
    style.set_color(StyleColor::FrameBg, hex(BG_FRAME, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(BG_FRAME_HOVER, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(BG_FRAME_ACTIVE, 1.0));

    // ── Title bar ──
    style.set_color(StyleColor::TitleBg, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(TITLE_BG, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(TITLE_BG, 0.75));

    // ── Menu bar ──
    style.set_color(StyleColor::MenuBarBg, hex(BG_CHILD, 1.0));

    // ── Scrollbar ──
    style.set_color(StyleColor::ScrollbarBg, hex(BG_CHILD, 0.7));
    style.set_color(StyleColor::ScrollbarGrab, hex(SECONDARY_HOVER, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(SECONDARY_ACTIVE, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    // ── Buttons ──
    style.set_color(StyleColor::Button, hex(ACCENT, 0.90));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    // ── Headers ──
    style.set_color(StyleColor::Header, hex(SECONDARY, 0.9));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.4));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.6));

    // ── Separator ──
    style.set_color(StyleColor::Separator, hex(SEPARATOR, 1.0));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    // ── Resize grip ──
    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.25));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.55));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.85));

    // ── Tabs ──
    style.set_color(StyleColor::Tab, hex(SECONDARY, 0.9));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::TabSelected, hex(BG, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(SECONDARY, 0.6));
    style.set_color(StyleColor::TabDimmedSelected, hex(SECONDARY_HOVER, 0.75));

    // ── Plot / Charts ──
    style.set_color(StyleColor::PlotLines, hex(ACCENT, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(DANGER, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(SUCCESS, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(WARNING, 1.0));

    // ── Tables ──
    style.set_color(StyleColor::TableHeaderBg, hex(BG_CHILD, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(BORDER, 1.0));
    style.set_color(StyleColor::TableBorderLight, hex(BORDER, 0.6));
    style.set_color(StyleColor::TableRowBg, hex(0xffffff, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0x000000, 0.04));

    // ── Misc ──
    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.30));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(FG, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.85));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
