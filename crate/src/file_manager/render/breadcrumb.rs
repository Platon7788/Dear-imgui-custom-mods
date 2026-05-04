//! Render the breadcrumb path bar (clickable segments + edit-mode text input).

use std::path::Path;

use dear_imgui_rs::{Key, MouseButton, StyleColor, Ui};

use crate::{icons, theme};

use crate::file_manager::actions::Action;
use crate::file_manager::util::BreadcrumbSegment;

/// Below this content-region width, breadcrumb collapses to 2 visible segments.
const BREADCRUMB_NARROW_WIDTH: f32 = 300.0;
/// Below this width, breadcrumb shows up to 3 segments. Above, all segments.
const BREADCRUMB_MEDIUM_WIDTH: f32 = 500.0;
/// Visible-segment count when content region is below [`BREADCRUMB_NARROW_WIDTH`].
const BREADCRUMB_VISIBLE_NARROW: usize = 2;
/// Visible-segment count when between narrow and medium width thresholds.
const BREADCRUMB_VISIBLE_MEDIUM: usize = 3;
/// Minimum free-space width to render the double-click "edit path" hot zone.
const BREADCRUMB_EDIT_HOTZONE_MIN: f32 = 20.0;

/// Render the breadcrumb path bar with two modes:
///
/// - **Browse mode** (default): clickable path segments separated by chevrons.
///   Clicking a segment navigates to that directory. Double-clicking the empty
///   area switches to edit mode.
/// - **Edit mode**: a text input where the user can type a path directly.
///   Enter navigates, Escape cancels.
pub(crate) fn render_breadcrumb_bar(
    ui: &Ui,
    path: &Path,
    segments: &[BreadcrumbSegment],
    editing: &mut bool,
    path_buf: &mut String,
) -> Option<Action> {
    // Edit-mode path text input.
    if *editing {
        let _bg = ui.push_style_color(StyleColor::FrameBg, theme::BG_FRAME);
        ui.text_colored(theme::WARNING, icons::FOLDER_OPEN);
        ui.same_line_with_spacing(0.0, 6.0);
        ui.set_next_item_width(ui.content_region_avail()[0]);
        let enter = ui
            .input_text("##pathbar", path_buf)
            .enter_returns_true(true)
            .build();

        if enter {
            *editing = false;
            return Some(Action::NavigateToInput(path_buf.clone()));
        }
        if ui.is_key_pressed(Key::Escape) {
            *editing = false;
            path_buf.clear();
            path_buf.push_str(&path.to_string_lossy());
        }
        return None;
    }

    // Breadcrumb mode: clickable path segments (segments cached on FileManager).
    let _bg = ui.push_style_color(StyleColor::ChildBg, theme::BG_FRAME);

    ui.text_colored(theme::WARNING, icons::FOLDER_OPEN);
    ui.same_line_with_spacing(0.0, 6.0);

    // Breadcrumb overflow: collapse early segments into "..." when narrow.
    let avail_w = ui.content_region_avail()[0];
    let max_visible = if avail_w < BREADCRUMB_NARROW_WIDTH {
        BREADCRUMB_VISIBLE_NARROW
    } else if avail_w < BREADCRUMB_MEDIUM_WIDTH {
        BREADCRUMB_VISIBLE_MEDIUM
    } else {
        segments.len()
    };
    let skip_count = segments.len().saturating_sub(max_visible);

    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            ui.same_line_with_spacing(0.0, 1.0);
            ui.text_colored(theme::TEXT_MUTED, icons::CHEVRON_RIGHT);
            ui.same_line_with_spacing(0.0, 1.0);
        }

        // Collapse early segments into "..." when overflow is needed.
        if i > 0 && i < skip_count {
            continue;
        }
        if i == 1 && skip_count > 1 {
            ui.text_colored(theme::TEXT_MUTED, "...");
            ui.same_line_with_spacing(0.0, 1.0);
            continue;
        }

        let _id = ui.push_id(i);
        // P0-2: navigate to the cached PathBuf — no string-splice reconstruction.
        // The cache was built via `Path::ancestors`, which preserves UNC prefixes
        // (`\\server\share\`) and Unix root (`/`) correctly.
        if ui.small_button(seg.label.as_str()) {
            return Some(Action::NavigateTo(seg.path.clone()));
        }
    }

    // Double-click on empty area → switch to edit mode
    ui.same_line();
    let avail = ui.content_region_avail()[0];
    if avail > BREADCRUMB_EDIT_HOTZONE_MIN {
        ui.invisible_button("##path_edit_zone", [avail, ui.text_line_height()]);
        if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
            *editing = true;
            path_buf.clear();
            path_buf.push_str(&path.to_string_lossy());
        }
    }

    None
}
