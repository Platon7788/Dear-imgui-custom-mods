//! Sidebar panel — "Фильтры / Группировка / Отображение / Силы".
//!
//! Phase B and C will add the actual control widgets.
//! Phase A: placeholder collapsing headers only.

use dear_imgui_rs::Ui;

use super::config::{ForceConfig, SidebarKind, ViewerConfig};
use super::event::GraphEvent;
use super::filter::FilterState;

/// Render the sidebar and return whether the mouse is hovering it.
///
/// Returns `true` when the cursor is over the sidebar — callers should
/// suppress graph hit-testing while this is the case.
///
/// Returns `false` immediately when `kind` is [`SidebarKind::None`].
pub(crate) fn render_sidebar(
    ui: &Ui,
    config: &mut ViewerConfig,
    force_config: &mut ForceConfig,
    filter: &mut FilterState,
    events: &mut Vec<GraphEvent>,
    kind: &SidebarKind,
) -> bool {
    if matches!(kind, SidebarKind::None) {
        return false;
    }

    // Sidebar width
    let sidebar_w = 200.0_f32;
    ui.same_line();

    // Suppress unused-variable warnings — these fields will be used in Phase B/C.
    let _ = (config, force_config, filter, events, sidebar_w);

    // Phase B/C will add proper controls here. For Phase A, just a placeholder.
    let _token = ui.push_item_width(sidebar_w);
    ui.text("Sidebar (Phase B)");

    ui.is_window_hovered()
}
