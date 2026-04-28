//! Render the favorites sidebar (Desktop, Documents, Downloads, custom bookmarks).

use std::path::Path;

use dear_imgui_rs::{StyleColor, Ui};

use crate::{icons, theme};

use super::style::icon_label;
use crate::file_manager::actions::Action;
use crate::file_manager::config::FmStrings;
use crate::file_manager::favorites::FavoritesPanel;

/// Each entry is a selectable row with an icon. The current directory is highlighted.
pub(crate) fn render_favorites_panel(
    ui: &Ui,
    favorites: &FavoritesPanel,
    current_path: &Path,
    strings: &FmStrings,
    buf: &mut String,
) -> Option<Action> {
    let mut action = None;

    ui.text_colored(theme::TEXT_SECONDARY, icons::STAR);
    ui.same_line_with_spacing(0.0, 4.0);
    ui.text_colored(theme::TEXT_SECONDARY, strings.favorites);
    ui.separator();

    for (i, fav) in favorites.entries.iter().enumerate() {
        let _id = ui.push_id(i);
        let is_current = fav.path == current_path;

        let label = icon_label(buf, fav.icon, &fav.label);

        // Guard must live until after `selectable_config().build()`.
        let _bg = is_current.then(|| ui.push_style_color(StyleColor::Header, theme::ACCENT_ACTIVE));

        if ui.selectable_config(label).selected(is_current).build() && fav.path.is_dir() {
            action = Some(Action::NavigateTo(fav.path.clone()));
        }
    }

    action
}
