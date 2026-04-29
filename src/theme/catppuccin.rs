//! Catppuccin Mocha theme — full stack (titlebar + nav + dialog + statusbar + ImGui style).
//!
//! Catppuccin is a community-driven pastel palette popularised through
//! dotfile / IDE configurations since 2022. Mocha is the darkest of the
//! four official flavours; it pairs warm dark surfaces (mantle / crust /
//! base) with soft pastel accents (mauve, sky, peach, green) for a
//! comfortable low-eye-strain workspace look.
//!
//! Reference palette: <https://github.com/catppuccin/catppuccin>

use super::palettes::{DisasmViewColors, DisasmViewTokens, HexViewerColors, HexViewerTokens};
use super::{DialogColors, NavColors, StatusBarColors, TitlebarColors};
#[cfg(feature = "status_bar")]
use crate::status_bar::StatusBarConfig;
use dear_imgui_rs::{Style, StyleColor};

// ─── Palette — Catppuccin Mocha ──────────────────────────────────────────────

const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        a,
    ]
}

// Surfaces (Mocha official names: crust < mantle < base < surface0/1/2 < overlay0/1/2 < subtext0/1 < text).
const CRUST: u32 = 0x11111b; // titlebar / status bar
const MANTLE: u32 = 0x181825; // child windows
const BASE: u32 = 0x1e1e2e; // main window bg
const SURFACE0: u32 = 0x313244; // input frames
const SURFACE1: u32 = 0x45475a; // hovered frames / borders
const SURFACE2: u32 = 0x585b70; // active frames

// Foreground.
const TEXT: u32 = 0xcdd6f4;
const SUBTEXT1: u32 = 0xbac2de;
const SUBTEXT0: u32 = 0xa6adc8;
const OVERLAY1: u32 = 0x7f849c; // disabled

// Pastel accents.
const RED: u32 = 0xf38ba8; // dialog error / destructive button
const PEACH: u32 = 0xfab387; // warning
const YELLOW: u32 = 0xf9e2af; // titlebar minimize
const GREEN: u32 = 0xa6e3a1; // success / confirm cancel
const TEAL: u32 = 0x94e2d5;
const SKY: u32 = 0x89dceb; // titlebar maximize / info
const SAPPHIRE: u32 = 0x74c7ec;
const BLUE: u32 = 0x89b4fa; // primary accent
const LAVENDER: u32 = 0xb4befe;
const MAUVE: u32 = 0xcba6f7; // question / dialog accent

// Active inactive variants (dim).
const TITLE_INACTIVE_BG: u32 = 0x0a0a12;

// Primary accent — official Mocha "blue" is the most-used UI accent.
const ACCENT: u32 = BLUE;
const ACCENT_HOVER: u32 = 0xa3c4fb; // 5% lighter
const ACCENT_ACTIVE: u32 = 0x6a9bf0; // 5% darker

// ─── Titlebar ────────────────────────────────────────────────────────────────

pub fn titlebar_colors() -> TitlebarColors {
    let bg = hex(CRUST, 1.0);
    let icon = hex(SUBTEXT0, 1.0);
    TitlebarColors {
        bg,
        separator: hex(SURFACE0, 1.0),
        title: hex(TEXT, 1.0),
        // Vex0r-style accents on the Catppuccin pastel palette:
        // yellow / sky / red — softer than Monokai's neon trio.
        btn_minimize: hex(YELLOW, 1.0),
        btn_maximize: hex(SKY, 1.0),
        btn_close: hex(RED, 1.0),
        btn_hover_bg: hex(SURFACE1, 0.85),
        btn_close_hover_bg: hex(RED, 0.40),
        icon,
        bg_erase: bg,
        drag_hint: hex(SURFACE1, 0.35),
        bg_inactive: hex(TITLE_INACTIVE_BG, 1.0),
        title_inactive: hex(OVERLAY1, 1.0),
    }
}

// ─── Nav panel ───────────────────────────────────────────────────────────────

pub fn nav_colors() -> NavColors {
    let bg = hex(CRUST, 1.0);
    let btn_hover = hex(SURFACE1, 1.0);
    // SURFACE1 separator (matches `statusbar_colors().separator`) —
    // SURFACE0 was a hair too dim against the CRUST background.
    let sep = hex(SURFACE1, 1.0);
    let accent = hex(ACCENT, 1.0);
    let icon_active = hex(TEXT, 1.0);
    NavColors {
        bg,
        btn_hover,
        btn_active: btn_hover,
        indicator: accent,
        icon_default: hex(SUBTEXT0, 1.0),
        icon_active,
        separator: sep,
        badge_bg: hex(RED, 1.0),
        badge_text: [1.0, 1.0, 1.0, 1.0],
        submenu_bg: hex(MANTLE, 1.0),
        submenu_hover: btn_hover,
        submenu_text: icon_active,
        submenu_border: sep,
        submenu_separator: sep,
        toggle_icon: hex(SUBTEXT0, 1.0),
    }
}

// ─── Confirm dialog ──────────────────────────────────────────────────────────

pub fn dialog_colors() -> DialogColors {
    let bg = hex(MANTLE, 1.0);
    let bg_float = [
        (bg[0] + 0.04).min(1.0),
        (bg[1] + 0.04).min(1.0),
        (bg[2] + 0.04).min(1.0),
        1.0,
    ];
    DialogColors {
        overlay: [0.0, 0.0, 0.0, 0.55],
        bg: bg_float,
        border: hex(SURFACE1, 1.0),
        title: hex(TEXT, 1.0),
        message: hex(SUBTEXT1, 1.0),
        separator: hex(SURFACE0, 1.0),
        icon_warning: hex(PEACH, 1.0),
        icon_error: hex(RED, 1.0),
        icon_info: hex(SKY, 1.0),
        icon_question: hex(MAUVE, 1.0),
        // Soft destructive — pastel pink (RED) + neutral surface tone.
        btn_confirm: [0.78, 0.40, 0.50, 1.0],
        btn_confirm_hover: [0.86, 0.48, 0.58, 1.0],
        btn_confirm_active: [0.70, 0.32, 0.42, 1.0],
        btn_confirm_text: [1.0, 1.0, 1.0, 1.0],
        btn_cancel: [0.32, 0.50, 0.40, 1.0], // muted pastel green
        btn_cancel_hover: [0.40, 0.58, 0.48, 1.0],
        btn_cancel_active: [0.24, 0.42, 0.32, 1.0],
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
        bg: hex(CRUST, 1.0),
        text: hex(TEXT, 1.0),
        text_dim: hex(SUBTEXT0, 1.0),
        separator: hex(SURFACE1, 1.0),
        hover: hex(SURFACE1, 1.0),
        active: hex(SURFACE1, 1.0),
        success: hex(GREEN, 1.0),
        warning: hex(PEACH, 1.0),
        error: hex(RED, 1.0),
        info: hex(SKY, 1.0),
    }
}

// ─── HexViewer ───────────────────────────────────────────────────────────────

/// Hex-viewer palette for the Catppuccin Mocha theme — soft pastel hues.
pub fn hex_viewer_colors() -> HexViewerColors {
    HexViewerColors::from_tokens(&HexViewerTokens {
        fg: hex(TEXT, 1.0),
        fg_muted: hex(SUBTEXT0, 1.0),
        accent: hex(BLUE, 1.0),
        success: hex(GREEN, 1.0),
        warning: hex(PEACH, 1.0),
        danger: hex(RED, 1.0),
        // Catppuccin's mauve — softer than the standard purple, fits
        // alongside the rest of the pastel palette.
        purple: hex(MAUVE, 1.0),
    })
}

// ─── DisasmView ──────────────────────────────────────────────────────────────

/// Disassembly-view palette for the Catppuccin Mocha theme — soft
/// pastel hues. BLUE for the address gutter, GREEN for calls, YELLOW
/// for jumps, RED for returns, MAUVE for stack ops, PEACH for
/// syscall + memory, SKY-toned cyan for registers.
pub fn disasm_view_colors() -> DisasmViewColors {
    DisasmViewColors::from_tokens(&DisasmViewTokens {
        fg: hex(TEXT, 1.0),
        fg_muted: hex(SUBTEXT0, 1.0),
        accent: hex(BLUE, 1.0),
        success: hex(GREEN, 1.0),
        warning: hex(YELLOW, 1.0),
        danger: hex(RED, 1.0),
        purple: hex(MAUVE, 1.0),
        orange: hex(PEACH, 1.0),
        // Use TEAL — distinct from BLUE accent and SKY (which are close
        // in hue). Reads as a bright pastel cyan for register text.
        cyan: hex(TEAL, 1.0),
    })
}

// ─── ImGui style ─────────────────────────────────────────────────────────────

pub fn apply_imgui_style(style: &mut Style) {
    style.set_window_rounding(6.0); // Catppuccin tends to softer corners
    style.set_frame_rounding(4.0);
    style.set_child_rounding(4.0);
    style.set_popup_rounding(6.0);
    style.set_scrollbar_rounding(4.0);
    style.set_grab_rounding(3.0);
    style.set_tab_rounding(4.0);

    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_child_border_size(1.0);
    style.set_popup_border_size(1.0);
    style.set_scrollbar_size(12.0);
    style.set_grab_min_size(8.0);
    style.set_frame_padding([6.0, 4.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([4.0, 4.0]);

    style.set_color(StyleColor::WindowBg, hex(BASE, 1.0));
    style.set_color(StyleColor::ChildBg, hex(MANTLE, 0.0));
    style.set_color(StyleColor::PopupBg, hex(MANTLE, 0.97));
    style.set_color(StyleColor::ModalWindowDimBg, hex(0x000000, 0.55));

    style.set_color(StyleColor::Text, hex(TEXT, 1.0));
    style.set_color(StyleColor::TextDisabled, hex(OVERLAY1, 1.0));

    style.set_color(StyleColor::Border, hex(SURFACE1, 0.85));
    style.set_color(StyleColor::BorderShadow, hex(0x000000, 0.0));

    style.set_color(StyleColor::FrameBg, hex(SURFACE0, 1.0));
    style.set_color(StyleColor::FrameBgHovered, hex(SURFACE1, 1.0));
    style.set_color(StyleColor::FrameBgActive, hex(SURFACE2, 1.0));

    style.set_color(StyleColor::TitleBg, hex(CRUST, 1.0));
    style.set_color(StyleColor::TitleBgActive, hex(CRUST, 1.0));
    style.set_color(StyleColor::TitleBgCollapsed, hex(CRUST, 0.75));

    style.set_color(StyleColor::MenuBarBg, hex(MANTLE, 1.0));

    style.set_color(StyleColor::ScrollbarBg, hex(CRUST, 0.6));
    style.set_color(StyleColor::ScrollbarGrab, hex(SURFACE0, 0.9));
    style.set_color(StyleColor::ScrollbarGrabHovered, hex(SURFACE1, 1.0));
    style.set_color(StyleColor::ScrollbarGrabActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::Button, hex(ACCENT, 0.85));
    style.set_color(StyleColor::ButtonHovered, hex(ACCENT_HOVER, 1.0));
    style.set_color(StyleColor::ButtonActive, hex(ACCENT_ACTIVE, 1.0));

    style.set_color(StyleColor::Header, hex(SURFACE0, 0.85));
    style.set_color(StyleColor::HeaderHovered, hex(ACCENT, 0.5));
    style.set_color(StyleColor::HeaderActive, hex(ACCENT, 0.7));

    style.set_color(StyleColor::Separator, hex(SURFACE0, 0.80));
    style.set_color(StyleColor::SeparatorHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::SeparatorActive, hex(ACCENT, 1.0));

    style.set_color(StyleColor::ResizeGrip, hex(ACCENT, 0.25));
    style.set_color(StyleColor::ResizeGripHovered, hex(ACCENT, 0.55));
    style.set_color(StyleColor::ResizeGripActive, hex(ACCENT, 0.85));

    style.set_color(StyleColor::Tab, hex(SURFACE0, 0.85));
    style.set_color(StyleColor::TabHovered, hex(ACCENT, 0.6));
    style.set_color(StyleColor::TabSelected, hex(SURFACE2, 1.0));
    style.set_color(StyleColor::TabDimmed, hex(SURFACE0, 0.5));
    style.set_color(StyleColor::TabDimmedSelected, hex(SURFACE1, 0.7));

    style.set_color(StyleColor::PlotLines, hex(SAPPHIRE, 1.0));
    style.set_color(StyleColor::PlotLinesHovered, hex(LAVENDER, 1.0));
    style.set_color(StyleColor::PlotHistogram, hex(GREEN, 1.0));
    style.set_color(StyleColor::PlotHistogramHovered, hex(TEAL, 1.0));

    style.set_color(StyleColor::TableHeaderBg, hex(MANTLE, 1.0));
    style.set_color(StyleColor::TableBorderStrong, hex(SURFACE1, 0.90));
    style.set_color(StyleColor::TableBorderLight, hex(SURFACE0, 0.50));
    style.set_color(StyleColor::TableRowBg, hex(0x000000, 0.0));
    style.set_color(StyleColor::TableRowBgAlt, hex(0xffffff, 0.02));

    style.set_color(StyleColor::TextSelectedBg, hex(ACCENT, 0.35));
    style.set_color(StyleColor::DragDropTarget, hex(ACCENT, 0.9));
    style.set_color(StyleColor::NavCursor, hex(ACCENT, 1.0));
    style.set_color(StyleColor::NavWindowingHighlight, hex(TEXT, 0.7));
    style.set_color(StyleColor::NavWindowingDimBg, hex(0x000000, 0.2));
    style.set_color(StyleColor::CheckMark, hex(ACCENT, 1.0));
    style.set_color(StyleColor::SliderGrab, hex(ACCENT, 0.85));
    style.set_color(StyleColor::SliderGrabActive, hex(ACCENT_HOVER, 1.0));
}
