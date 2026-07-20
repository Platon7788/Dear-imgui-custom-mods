//! Right-click reposition menu — re-dock the panel (Left / Right / Top).
//!
//! This is a **real ImGui popup** (`open_popup` / `begin_popup`), not a
//! hand-rolled window: ImGui owns positioning, focus, and close-on-click-
//! outside / Escape, and — crucially — the rows are genuine `menu_item`s, so
//! the per-entry tooltip hangs off [`Ui::is_item_hovered`] and behaves exactly
//! like every other tooltip in the crate (proper hover delay, correct
//! placement, correct z-order above the menu). It shares the crate-wide popup
//! chrome via [`crate::utils::popup`] and the tooltip styling via
//! [`crate::utils::themed_tooltip`], matching the `code_editor` / `disasm_view`
//! context menus.
//!
//! Selection is delivered as [`NavEvent::PositionChangeRequested`]; the host
//! applies it by rebuilding the config with the new [`DockPosition`] (the panel
//! never mutates its own immutable config).

use dear_imgui_rs::Ui;

use super::NavEvent;
use super::config::NavPanelConfig;
use super::enums::DockPosition;
use super::state::NavPanelState;

/// ImGui popup id for the reposition menu.
const POPUP_ID: &str = "##nav_reposition_menu";

/// Drive the reposition popup for one frame. Opens it (anchored at the stored
/// right-click position) when a request is pending, then renders its body
/// while ImGui keeps it open. A no-op in frames where the popup is closed and
/// no request is pending.
pub(super) fn render_reposition_menu(
    ui: &Ui,
    cfg: &NavPanelConfig,
    state: &mut NavPanelState,
    events: &mut Vec<NavEvent>,
) {
    // Open on the frame the right-click was captured, anchoring the popup at
    // the click position. `Always` pins it there for its lifetime; ImGui caches
    // the position afterwards, so anchoring once at open is enough.
    if let Some(anchor) = state.reposition_open_request.take() {
        crate::utils::popup::anchor_next_popup_topleft(anchor);
        ui.open_popup(POPUP_ID);
    }

    let s = crate::i18n::nav_panel::strings(cfg.locale);
    let entries = [
        (DockPosition::Left, s.dock_left, s.dock_left_hint),
        (DockPosition::Right, s.dock_right, s.dock_right_hint),
        (DockPosition::Top, s.dock_top, s.dock_top_hint),
    ];

    let mut open = false;
    crate::utils::popup::themed_popup_style(ui, || {
        let Some(_popup) = ui.begin_popup(POPUP_ID) else {
            return;
        };
        open = true;
        crate::utils::popup::compact_popup_body(ui, || {
            ui.text_disabled(s.position_title);
            ui.separator();

            for (dock, label, hint) in entries {
                let is_current = cfg.position == dock;
                // `selected = is_current` draws a checkmark on the active
                // dock, the standard "you are here" cue in a context menu.
                let clicked =
                    ui.menu_item_enabled_selected_with_shortcut(label, "", is_current, true);
                // Genuine ImGui item ⇒ `is_item_hovered` gives the tooltip the
                // normal hover delay / placement — no manual hit-testing.
                if ui.is_item_hovered() {
                    crate::utils::themed_tooltip(ui, || ui.text(hint));
                }
                if clicked {
                    events.push(NavEvent::PositionChangeRequested(dock));
                    ui.close_current_popup();
                }
            }
        });
    });

    // Mirror ImGui's open state so an `auto_hide` panel doesn't slide away
    // out from under an open menu (see `render`).
    state.reposition_menu_open = open;
}
