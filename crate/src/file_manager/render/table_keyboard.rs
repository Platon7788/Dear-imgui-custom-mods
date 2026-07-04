//! Keyboard navigation for the file table — arrow keys, Page Up/Down, Home/End,
//! Enter (open dir / confirm file), Backspace (parent directory).
//!
//! Split out of [`table`](super::table) to keep that file within the 500-line
//! budget (CLAUDE.md). Operates only on the selection/scroll state and the
//! entry slice, returning at most one deferred [`Action`].

use dear_imgui_rs::{Key, Ui};

use crate::file_manager::actions::Action;
use crate::file_manager::entry::FsEntry;

/// Approximate visible row count for PageUp/PageDown when the window height
/// isn't reliably available. `render_file_table` normally derives the page
/// size from `window_size().y / line_height`; this is the fallback.
const PAGE_SIZE_FALLBACK: usize = 20;

/// Handle keyboard navigation for the file table.
///
/// No-ops while any text input is active or the table window (and its
/// children) is unfocused. Mutates `selected_indices` and `scroll_to_index`
/// in place; returns a deferred [`Action`] for Enter (open dir / confirm file)
/// or Backspace (go to parent), otherwise `None`.
pub(super) fn handle_table_keyboard(
    ui: &Ui,
    entries: &[FsEntry],
    selected_indices: &mut Vec<usize>,
    scroll_to_index: &mut Option<usize>,
) -> Option<Action> {
    let mut action = None;

    if !ui.is_any_item_active()
        && ui.is_window_focused_with_flags(
            dear_imgui_rs::FocusedFlags::ROOT_WINDOW | dear_imgui_rs::FocusedFlags::CHILD_WINDOWS,
        )
    {
        if ui.is_key_pressed(Key::UpArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = current.saturating_sub(1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::DownArrow) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = (current + 1).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::Enter)
            && let Some(&idx) = selected_indices.first()
            && let Some(e) = entries.get(idx)
        {
            if e.is_dir {
                action = Some(Action::NavigateTo(e.path.clone()));
            } else {
                action = Some(Action::ConfirmSelection);
            }
        }
        if ui.is_key_pressed(Key::Backspace) {
            action = Some(Action::GoParent);
        }

        // Page Up / Page Down — derive page_size from the actual visible row
        // count. Falls back to the constant when window height is invalid.
        let row_h = ui.text_line_height_with_spacing().max(1.0);
        let win_h = ui.window_size()[1].max(0.0);
        let page_size = if win_h > 0.0 && row_h > 0.0 {
            ((win_h / row_h) as usize).max(1)
        } else {
            PAGE_SIZE_FALLBACK
        };
        if ui.is_key_pressed(Key::PageUp) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = current.saturating_sub(page_size);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        if ui.is_key_pressed(Key::PageDown) && !entries.is_empty() {
            let current = selected_indices.first().copied().unwrap_or(0);
            let new_idx = (current + page_size).min(entries.len() - 1);
            selected_indices.clear();
            selected_indices.push(new_idx);
            *scroll_to_index = Some(new_idx);
        }
        // Home / End
        if ui.is_key_pressed(Key::Home) && !entries.is_empty() {
            selected_indices.clear();
            selected_indices.push(0);
            *scroll_to_index = Some(0);
        }
        if ui.is_key_pressed(Key::End) && !entries.is_empty() {
            let last = entries.len() - 1;
            selected_indices.clear();
            selected_indices.push(last);
            *scroll_to_index = Some(last);
        }
    }

    action
}
