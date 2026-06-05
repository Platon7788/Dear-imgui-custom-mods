//! Focus-gated keyboard navigation.
//!
//! Arrow step (no wrap), `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle (wraparound),
//! `Ctrl+1..8` jump, `Ctrl+9` jump-to-last (Chrome convention), `Ctrl+T`
//! add-request and `Ctrl+W` close-active.

use dear_imgui_rs::{Key, Ui};

use super::super::types::*;
use super::super::{TabControl, TabItem};
use super::hittest::scroll_into_view;

pub(super) fn handle_keyboard<T: TabItem>(
    pc: &mut TabControl<T>,
    ui: &Ui,
    scroll_area_w: f32,
    action: &mut Option<TabAction>,
) {
    let io = ui.io();
    let ctrl = io.key_ctrl();
    let shift = io.key_shift();

    let prev = ui.is_key_pressed(Key::LeftArrow);
    let next = ui.is_key_pressed(Key::RightArrow);
    let ctrl_w = ctrl && ui.is_key_pressed(Key::W);
    let ctrl_t = ctrl && ui.is_key_pressed(Key::T);
    let ctrl_tab = ctrl && ui.is_key_pressed(Key::Tab);

    let mut nav_idx: Option<usize> = None;

    // Arrow navigation (no modifier)
    if !ctrl && (prev || next) {
        if let Some(active_id) = pc.active {
            if let Some(idx) = pc.tabs.iter().position(|t| t.id == active_id) {
                let new_idx = if prev {
                    idx.saturating_sub(1)
                } else {
                    (idx + 1).min(pc.tabs.len() - 1)
                };
                if new_idx != idx {
                    pc.tabs[idx].item.on_deactivated();
                    let new_id = pc.tabs[new_idx].id;
                    pc.active = Some(new_id);
                    pc.tabs[new_idx].item.on_activated();
                    *action = Some(TabAction::Activated(new_id));
                    nav_idx = Some(new_idx);
                }
            }
        } else {
            let id = pc.tabs[0].id;
            pc.active = Some(id);
            pc.tabs[0].item.on_activated();
            *action = Some(TabAction::Activated(id));
            nav_idx = Some(0);
        }
    }

    // Ctrl+Tab / Ctrl+Shift+Tab — cycle (with wraparound, like browsers)
    if ctrl_tab && !pc.tabs.is_empty() {
        let cur = pc
            .active
            .and_then(|id| pc.tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);
        let len = pc.tabs.len();
        let new_idx = if shift {
            (cur + len - 1) % len
        } else {
            (cur + 1) % len
        };
        if new_idx != cur {
            pc.tabs[cur].item.on_deactivated();
            let new_id = pc.tabs[new_idx].id;
            pc.active = Some(new_id);
            pc.tabs[new_idx].item.on_activated();
            *action = Some(TabAction::Activated(new_id));
            nav_idx = Some(new_idx);
        }
    }

    // Ctrl+1..9 — jump to nth tab (1-based; Ctrl+9 jumps to last)
    if ctrl {
        let digit_keys = [
            Key::Key1,
            Key::Key2,
            Key::Key3,
            Key::Key4,
            Key::Key5,
            Key::Key6,
            Key::Key7,
            Key::Key8,
            Key::Key9,
        ];
        for (i, key) in digit_keys.iter().enumerate() {
            if ui.is_key_pressed(*key) && !pc.tabs.is_empty() {
                let target = if i == 8 {
                    // Ctrl+9 → last (Chrome convention)
                    pc.tabs.len() - 1
                } else {
                    i.min(pc.tabs.len() - 1)
                };
                let cur = pc
                    .active
                    .and_then(|id| pc.tabs.iter().position(|t| t.id == id));
                if cur != Some(target) {
                    if let Some(c) = cur {
                        pc.tabs[c].item.on_deactivated();
                    }
                    let new_id = pc.tabs[target].id;
                    pc.active = Some(new_id);
                    pc.tabs[target].item.on_activated();
                    *action = Some(TabAction::Activated(new_id));
                    nav_idx = Some(target);
                }
                break;
            }
        }
    }

    if let Some(idx) = nav_idx {
        scroll_into_view(pc, idx, scroll_area_w);
    }

    // Ctrl+T — request a new tab
    if ctrl_t {
        *action = Some(TabAction::AddRequested);
    }

    // Ctrl+W — close active
    if ctrl_w && let Some(active_id) = pc.active {
        let can_close = pc
            .tabs
            .iter()
            .find(|t| t.id == active_id)
            .is_some_and(|t| pc.config.closable && t.item.is_closable());
        if can_close {
            if pc.config.confirm_close {
                pc.pending_close = Some(active_id);
                pc.pending_close_new = true;
            } else if pc.config.animate_close {
                pc.closing_tab = Some((active_id, 1.0));
            } else if let Some(t) = pc.tabs.iter_mut().find(|t| t.id == active_id) {
                t.open = false;
            }
        }
    }
}
