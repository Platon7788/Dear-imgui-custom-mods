//! Node label rendering with visibility and zoom-based culling.

/// Draw a node label at `screen_pos` if conditions are met.
///
/// The label is skipped when:
/// - `zoom` is below `min_zoom` (label would be too small to read), or
/// - `label` is empty.
///
/// The label is drawn slightly below the node center to avoid overlapping
/// the node circle.
pub(crate) fn draw_label(
    draw: &dear_imgui_rs::DrawListMut<'_>,
    label: &str,
    screen_pos: [f32; 2],
    color: u32,
    zoom: f32,
    min_zoom: f32,
) {
    if zoom < min_zoom || label.is_empty() {
        return;
    }
    // Center the label slightly below the node.
    draw.add_text([screen_pos[0], screen_pos[1] + 2.0], color, label);
}
