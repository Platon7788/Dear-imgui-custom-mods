//! Per-widget palette-resolution tests for [`super::Theme`]. Each test
//! pins that a widget config / palette accessor stays in the same visual
//! family as the rest of the chrome stack. The core enum / semantic /
//! contrast tests live in the sibling [`super::tests`] module.

use super::{Theme, palettes};

#[cfg(feature = "tab_control")]
#[test]
fn tab_colors_track_nav_and_statusbar() {
    // `Theme::tab_colors()` must compose tab strip surfaces from
    // the nav + status-bar palettes for that theme — keeps the
    // chrome ecosystem coherent. Pin the wiring so an accidental
    // edit of either palette doesn't desync the tab strip.
    for &theme in Theme::ALL {
        let nav = theme.nav();
        let sb = theme.statusbar_colors();
        let tabs = theme.tab_colors();
        let to_u8 = |c: [f32; 4]| {
            [
                (c[0] * 255.0).round().clamp(0.0, 255.0) as u8,
                (c[1] * 255.0).round().clamp(0.0, 255.0) as u8,
                (c[2] * 255.0).round().clamp(0.0, 255.0) as u8,
            ]
        };
        assert_eq!(tabs.strip_bg, to_u8(nav.bg), "{theme:?}: strip_bg");
        // body_bg lifts strip_bg darker so the framed-content
        // visual works (gap = strip_bg, inner = body_bg).
        // Pin distinctness, not equality.
        assert_ne!(
            tabs.body_bg, tabs.strip_bg,
            "{theme:?}: body_bg must differ from strip_bg for visible frame",
        );
        assert_eq!(tabs.tab_hover, to_u8(nav.btn_hover), "{theme:?}: tab_hover");
        assert_eq!(tabs.text, to_u8(nav.icon_active), "{theme:?}: text");
        assert_eq!(
            tabs.text_muted,
            to_u8(nav.icon_default),
            "{theme:?}: text_muted"
        );
        assert_eq!(tabs.separator, to_u8(nav.separator), "{theme:?}: separator");
        assert_eq!(
            tabs.status_active,
            to_u8(sb.success),
            "{theme:?}: status_active"
        );
        assert_eq!(
            tabs.status_error,
            to_u8(sb.error),
            "{theme:?}: status_error"
        );
    }
}

#[test]
fn hex_viewer_colors_resolve_for_every_theme() {
    // Every theme must produce a non-default-zeroed
    // `HexViewerColors`. Pin a few invariants so a regression in
    // `HexViewerColors::from_tokens` (or a token mistype in a
    // theme module) surfaces as a fail with the offending theme
    // name rather than a silently broken visual.
    for &theme in Theme::ALL {
        let p = theme.hex_viewer_colors();
        assert!(p.offset[3] > 0.0, "{theme:?}: offset alpha = 0");
        assert!(
            p.cat_printable[3] > 0.0,
            "{theme:?}: cat_printable alpha = 0"
        );
        // Offset and category-printable are different hues
        // (accent vs success) — they must not collapse to the
        // same RGB or the visual hierarchy of the gutter
        // disappears.
        assert_ne!(
            [p.offset[0], p.offset[1], p.offset[2]],
            [p.cat_printable[0], p.cat_printable[1], p.cat_printable[2]],
            "{theme:?}: offset and cat_printable share a hue",
        );
    }
}

#[test]
fn disasm_view_colors_resolve_for_every_theme() {
    // Every theme must produce a non-default-zeroed
    // `DisasmViewColors`. Pin the per-flow uniqueness invariant —
    // call (success), jump (warning), return (danger), stack
    // (purple) and system (orange) MUST land on distinct hues so
    // the syntax highlighting actually reads.
    for &theme in Theme::ALL {
        let p = theme.disasm_view_colors();
        assert!(p.address[3] > 0.0, "{theme:?}: address alpha = 0");
        assert!(
            p.mnemonic_normal[3] > 0.0,
            "{theme:?}: mnemonic_normal alpha = 0"
        );
        assert!(!p.block_tints.is_empty(), "{theme:?}: empty block_tints");
        assert!(
            !p.breakpoint_colors.is_empty(),
            "{theme:?}: empty breakpoint_colors",
        );
        // Per-FlowKind mnemonic colours must be distinct so a jump
        // (yellow) does not visually collide with a call (green) or
        // return (red).
        let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
        assert_ne!(
            rgb(p.mnemonic_call),
            rgb(p.mnemonic_jump),
            "{theme:?}: call and jump share a hue",
        );
        assert_ne!(
            rgb(p.mnemonic_call),
            rgb(p.mnemonic_return),
            "{theme:?}: call and return share a hue",
        );
        assert_ne!(
            rgb(p.mnemonic_stack),
            rgb(p.mnemonic_system),
            "{theme:?}: stack and system share a hue",
        );
    }
}

#[test]
fn disasm_view_colors_default_matches_dark_theme() {
    // Bare `palettes::DisasmViewColors::default()` must mirror
    // `Theme::Dark.disasm_view_colors()` — that's the value
    // `DisasmViewConfig::default()` ends up with when the caller
    // never goes through the `Theme` system.
    let default = palettes::DisasmViewColors::default();
    let dark = Theme::Dark.disasm_view_colors();
    assert_eq!(default.mnemonic_normal, dark.mnemonic_normal);
    assert_eq!(default.mnemonic_call, dark.mnemonic_call);
    assert_eq!(default.address, dark.address);
    assert_eq!(default.selection_bg, dark.selection_bg);
    assert_eq!(default.breakpoint, dark.breakpoint);
    assert_eq!(default.block_tints.len(), dark.block_tints.len());
}

#[test]
fn hex_viewer_colors_default_matches_dark_theme() {
    // `HexViewerColors::default()` must be `Theme::Dark.hex_viewer_colors()`
    // exactly, so a bare `HexViewerConfig::default()` that
    // doesn't go through `Theme` still matches the rest of the
    // Dark chrome stack.
    let default = palettes::HexViewerColors::default();
    let dark = Theme::Dark.hex_viewer_colors();
    assert_eq!(default.offset, dark.offset);
    assert_eq!(default.cat_printable, dark.cat_printable);
    assert_eq!(default.hex, dark.hex);
    assert_eq!(default.cursor_bg, dark.cursor_bg);
    assert_eq!(default.changed, dark.changed);
}

#[test]
fn statusbar_colors_default_matches_dark_theme() {
    // `StatusBarColors::default()` is the value `StatusBarConfig::default()`
    // hands out when the caller hasn't gone through `Theme`. It must
    // mirror `Theme::Dark.statusbar_colors()` exactly, otherwise a
    // status bar built from defaults will read as a different shade
    // than every neighbour palette (nav, titlebar, dialog) in the
    // same Dark stack.
    let default = palettes::StatusBarColors::default();
    let dark = Theme::Dark.statusbar_colors();
    assert_eq!(default.bg, dark.bg, "bg drift");
    assert_eq!(default.text, dark.text, "text drift");
    assert_eq!(default.text_dim, dark.text_dim, "text_dim drift");
    assert_eq!(default.separator, dark.separator, "separator drift");
    assert_eq!(default.hover, dark.hover, "hover drift");
    assert_eq!(default.active, dark.active, "active drift");
}

#[cfg(feature = "code_editor")]
#[test]
fn code_editor_colors_resolve_for_every_theme() {
    // Maps each crate Theme to an EditorTheme preset; pin that the
    // cycle is total (no panic) and the resulting palette has
    // non-zero alpha on the foundational tokens. Catches a future
    // missing match arm in `EditorTheme::from_crate_theme`.
    for &theme in Theme::ALL {
        let p = theme.code_editor_colors();
        assert!(p.editor_bg[3] > 0.0, "{theme:?}: editor_bg alpha = 0");
        assert!(p.identifier[3] > 0.0, "{theme:?}: identifier alpha = 0");
        assert!(p.cursor[3] > 0.0, "{theme:?}: cursor alpha = 0");
    }
}

#[cfg(feature = "diff_viewer")]
#[test]
fn diff_viewer_config_resolves_for_every_theme() {
    // `Theme::diff_viewer_config()` must produce a non-zero
    // palette for every theme; added/removed text tokens must
    // share the hue family with `theme.success()` / `theme.danger()`
    // so visual semantics stay consistent.
    for &theme in Theme::ALL {
        let cfg = theme.diff_viewer_config();
        assert_eq!(
            cfg.color_added_text,
            theme.success(),
            "{theme:?}: added_text drift",
        );
        assert_eq!(
            cfg.color_removed_text,
            theme.danger(),
            "{theme:?}: removed_text drift",
        );
        assert!(cfg.color_text[3] > 0.0, "{theme:?}: text alpha = 0");
    }
}

#[cfg(feature = "force_graph")]
#[test]
fn force_graph_colors_resolve_for_every_theme() {
    // Background must be the same surface as `window_bg()` so the
    // graph canvas paints on the host window without a visible
    // edge seam.
    for &theme in Theme::ALL {
        let p = theme.force_graph_colors();
        assert_eq!(p.background, theme.window_bg(), "{theme:?}: bg drift");
        assert!(p.label_text[3] > 0.0, "{theme:?}: label_text alpha = 0");
        assert!(p.edge_default[3] > 0.0, "{theme:?}: edge_default alpha = 0");
    }
}

#[cfg(feature = "node_graph")]
#[test]
fn node_graph_colors_resolve_for_every_theme() {
    // Pin per-theme distinctness: `pin_default` (the connector
    // accent) must not collapse to `text_muted` — they need to
    // read as separate visual elements regardless of theme.
    for &theme in Theme::ALL {
        let p = theme.node_graph_colors();
        assert_ne!(
            p.pin_default, p.text_muted,
            "{theme:?}: pin_default == text_muted (would hide pins)",
        );
        assert_ne!(
            p.canvas_bg, p.node_bg,
            "{theme:?}: canvas == node (no node panel contrast)",
        );
    }
}

#[cfg(feature = "timeline")]
#[test]
fn timeline_config_resolves_for_every_theme() {
    for &theme in Theme::ALL {
        let cfg = theme.timeline_config();
        assert!(cfg.color_bg[3] > 0.0, "{theme:?}: bg alpha = 0");
        assert!(
            cfg.color_span_text[3] > 0.0,
            "{theme:?}: span_text alpha = 0"
        );
        // `span_palette` is theme-independent; pin that
        // `apply_theme` doesn't accidentally clear it.
        assert!(
            !cfg.span_palette.is_empty(),
            "{theme:?}: span_palette empty",
        );
    }
}

#[cfg(feature = "toolbar")]
#[test]
fn toolbar_config_resolves_for_every_theme() {
    // Toolbar `bg` must share the nav surface so a horizontal
    // toolbar reads as the same chrome strip as the vertical
    // nav panel of the same theme.
    for &theme in Theme::ALL {
        let cfg = theme.toolbar_config();
        assert_eq!(cfg.color_bg, theme.nav().bg, "{theme:?}: bg drift");
        assert!(cfg.color_text[3] > 0.0, "{theme:?}: text alpha = 0");
    }
}

#[cfg(feature = "property_inspector")]
#[test]
fn inspector_config_resolves_for_every_theme() {
    for &theme in Theme::ALL {
        let cfg = theme.inspector_config();
        assert!(cfg.color_value[3] > 0.0, "{theme:?}: value alpha = 0");
        assert_ne!(
            cfg.color_key, cfg.color_value,
            "{theme:?}: key and value share a hue",
        );
    }
}

#[cfg(feature = "status_bar")]
#[test]
fn nav_and_statusbar_palettes_are_in_sync() {
    // The two strip-style chrome surfaces (vertical nav, horizontal
    // status) should read as one cohesive ecosystem — they share
    // the same surface tokens (background, separator, primary /
    // muted text). This test pins that contract per theme so a
    // future tweak to one palette can't silently desync the other.
    for &theme in Theme::ALL {
        let nav = theme.nav();
        let sb = theme.statusbar_colors();
        assert_eq!(nav.bg, sb.bg, "{theme:?}: nav.bg ≠ statusbar.bg");
        assert_eq!(
            nav.separator, sb.separator,
            "{theme:?}: nav.separator ≠ statusbar.separator",
        );
        assert_eq!(
            nav.icon_active, sb.text,
            "{theme:?}: nav.icon_active ≠ statusbar.text",
        );
        assert_eq!(
            nav.icon_default, sb.text_dim,
            "{theme:?}: nav.icon_default ≠ statusbar.text_dim",
        );
    }
}
