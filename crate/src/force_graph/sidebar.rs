//! Sidebar panel — Filters / Color Groups / Display / Physics.
//!
//! The sidebar is a scrollable 220 px child window containing four collapsible
//! sections. It returns `true` when the mouse cursor is hovering over it so
//! that callers can suppress graph hit-testing while the sidebar is active.

use dear_imgui_rs::{TreeNodeFlags, Ui};

use crate::utils::clipboard::set_clipboard;

use super::config::{ColorGroup, ColorGroupQuery, ForceConfig, SidebarKind, ViewerConfig};
use super::data::GraphData;
use super::event::GraphEvent;
use super::filter::FilterState;

// ─── public entry point ────────────────────────────────────────────────────

/// Render the sidebar and return whether the mouse is hovering it.
///
/// Returns `true` when the cursor is over the sidebar — callers should
/// suppress graph hit-testing while this is the case.
///
/// Returns `false` immediately when `kind` is [`SidebarKind::None`].
pub(crate) fn render_sidebar(
    ui: &Ui,
    graph: &GraphData,
    config: &mut ViewerConfig,
    force_config: &mut ForceConfig,
    filter: &mut FilterState,
    events: &mut Vec<GraphEvent>,
    kind: &SidebarKind,
) -> bool {
    if matches!(kind, SidebarKind::None) {
        return false;
    }

    const SIDEBAR_W: f32 = 220.0;

    ui.same_line();

    let s = crate::i18n::force_graph::strings(config.locale);

    let mut hovered = false;
    ui.child_window("##sidebar")
        .size([SIDEBAR_W, 0.0])
        .build(ui, || {
            hovered = ui.is_window_hovered();

            let _w = ui.push_item_width(-1.0);

            render_filters(ui, filter, events, s);
            render_color_groups(ui, config, events, s);
            render_display(ui, config, events, s);
            render_physics(ui, force_config, events, s);
            render_export(ui, graph, s);
        });

    hovered
}

// ─── Section: Filters ─────────────────────────────────────────────────────

/// Render the "Filters" collapsing section.
fn render_filters(
    ui: &Ui,
    filter: &mut FilterState,
    events: &mut Vec<GraphEvent>,
    s: &crate::i18n::force_graph::Strings,
) {
    if !ui.collapsing_header(s.section_filters, TreeNodeFlags::DEFAULT_OPEN) {
        return;
    }

    // --- visibility toggles --------------------------------------------------
    if ui.checkbox(s.show_orphan_nodes, &mut filter.show_orphans) {
        events.push(GraphEvent::FilterChanged);
    }

    let mut hide_unresolved = filter.hide_unresolved;
    if ui.checkbox(s.hide_unresolved_links, &mut hide_unresolved) {
        filter.hide_unresolved = hide_unresolved;
        events.push(GraphEvent::FilterChanged);
    }

    let mut hide_tags = filter.hide_tags;
    if ui.checkbox(s.hide_tag_nodes, &mut hide_tags) {
        filter.hide_tags = hide_tags;
        events.push(GraphEvent::FilterChanged);
    }

    // --- search query --------------------------------------------------------
    ui.separator();
    ui.text(s.search_label);
    if ui.input_text("##search", &mut filter.search_query).build() {
        events.push(GraphEvent::SearchChanged(filter.search_query.clone()));
        events.push(GraphEvent::FilterChanged);
    }

    // --- depth slider (0 = all, 1-6 = hop limit) ----------------------------
    ui.text(s.depth_label);
    let mut depth_val: i32 = filter.depth.map_or(0, |d| d as i32);
    if ui.slider("##depth", 0_i32, 6_i32, &mut depth_val) {
        filter.depth = if depth_val == 0 {
            None
        } else {
            Some(depth_val as u32)
        };
        events.push(GraphEvent::FilterChanged);
    }

    // --- time-travel slider -------------------------------------------------
    ui.separator();
    ui.text(s.time_travel_label);
    let is_inf = filter.time_threshold.is_infinite();
    let mut t_val = if is_inf {
        1000.0_f32
    } else {
        filter.time_threshold
    };
    if ui.slider("##time", 0.0_f32, 1000.0_f32, &mut t_val) {
        filter.time_threshold = if t_val >= 999.9 { f32::INFINITY } else { t_val };
        events.push(GraphEvent::FilterChanged);
    }
    ui.same_line();
    if ui.small_button(s.btn_all) {
        filter.time_threshold = f32::INFINITY;
        events.push(GraphEvent::FilterChanged);
    }

    ui.spacing();
}

// ─── Section: Color Groups ─────────────────────────────────────────────────

/// Render the "Color Groups" collapsing section.
fn render_color_groups(
    ui: &Ui,
    config: &mut ViewerConfig,
    events: &mut Vec<GraphEvent>,
    s: &crate::i18n::force_graph::Strings,
) {
    if !ui.collapsing_header(s.section_color_groups, TreeNodeFlags::DEFAULT_OPEN) {
        return;
    }

    if config.color_groups.is_empty() {
        ui.text_disabled(s.no_groups_defined);
    } else {
        // We process mutations (delete, color change) after the display loop to
        // avoid conflicting borrows during iteration.
        let mut delete_idx: Option<usize> = None;
        let mut changed = false;
        let count = config.color_groups.len();

        for i in 0..count {
            let group = &mut config.color_groups[i];

            // --- enabled checkbox -------------------------------------------
            if ui.checkbox(format!("##en{i}"), &mut group.enabled) {
                changed = true;
            }
            ui.same_line();

            // --- compact color swatch + picker ------------------------------
            // NO_INPUTS hides the hex/RGB text fields; NO_LABEL hides the
            // label text so only the colored square is visible inline.
            if ui
                .color_edit4_config(format!("##col{i}"), &mut group.color)
                .flags(
                    dear_imgui_rs::ColorEditFlags::NO_INPUTS
                        | dear_imgui_rs::ColorEditFlags::NO_LABEL
                        | dear_imgui_rs::ColorEditFlags::ALPHA_BAR,
                )
                .build()
            {
                changed = true;
            }
            ui.same_line();

            // --- name label -------------------------------------------------
            ui.text(group.name.as_str());
            ui.same_line();

            // --- delete button ----------------------------------------------
            if ui.small_button(format!("x##{i}")) {
                delete_idx = Some(i);
                changed = true;
            }

            // --- query hint (indented second line) --------------------------
            ui.indent();
            ui.text_disabled(query_hint(&group.query, s));
            ui.unindent();
        }

        if let Some(idx) = delete_idx {
            config.color_groups.remove(idx);
        }

        if changed {
            events.push(GraphEvent::GroupChanged);
        }
    }

    // --- Add Group button ---------------------------------------------------
    ui.spacing();
    if ui.button(s.add_group) {
        config.color_groups.push(ColorGroup::new(
            s.new_group_default_name,
            ColorGroupQuery::All,
            [0.4, 0.7, 1.0, 1.0],
        ));
        events.push(GraphEvent::GroupChanged);
    }

    ui.spacing();
}

/// Return a short static hint describing a [`ColorGroupQuery`] variant.
fn query_hint(
    query: &ColorGroupQuery,
    s: &crate::i18n::force_graph::Strings,
) -> &'static str {
    match query {
        ColorGroupQuery::Label(_) => s.query_label,
        ColorGroupQuery::Tag(_) => s.query_tag,
        ColorGroupQuery::Kind(_) => s.query_kind,
        ColorGroupQuery::Regex(_) => s.query_regex,
        ColorGroupQuery::All => s.query_all,
    }
}

// ─── Section: Display ─────────────────────────────────────────────────────

/// Render the "Display" collapsing section.
fn render_display(
    ui: &Ui,
    config: &mut ViewerConfig,
    events: &mut Vec<GraphEvent>,
    s: &crate::i18n::force_graph::Strings,
) {
    if !ui.collapsing_header(s.section_display, TreeNodeFlags::DEFAULT_OPEN) {
        return;
    }

    let mut changed = false;

    // -- node size -----------------------------------------------------------
    ui.text(s.node_size);
    changed |= ui.slider(
        "##node_size",
        0.5_f32,
        4.0_f32,
        &mut config.node_size_multiplier,
    );

    // -- edge width ----------------------------------------------------------
    ui.text(s.edge_width);
    changed |= ui.slider(
        "##edge_w",
        0.5_f32,
        4.0_f32,
        &mut config.edge_thickness_multiplier,
    );

    // -- text fade threshold -------------------------------------------------
    ui.text(s.text_fade);
    changed |= ui.slider(
        "##text_fade",
        -3.0_f32,
        3.0_f32,
        &mut config.text_fade_threshold,
    );

    // -- hover fade ----------------------------------------------------------
    ui.text(s.hover_fade);
    changed |= ui.slider(
        "##hover_fade",
        0.0_f32,
        1.0_f32,
        &mut config.hover_fade_opacity,
    );

    // -- edge curve ----------------------------------------------------------
    ui.text(s.edge_curve);
    changed |= ui.slider("##edge_curve", 0.0_f32, 1.0_f32, &mut config.edge_curve);

    ui.separator();

    // -- boolean toggles -----------------------------------------------------
    changed |= ui.checkbox(s.toggle_arrows, &mut config.edge_arrow);
    changed |= ui.checkbox(s.toggle_edge_labels, &mut config.show_edge_labels);
    changed |= ui.checkbox(s.toggle_background_grid, &mut config.background_grid);
    changed |= ui.checkbox(s.toggle_glow_on_hover, &mut config.glow_on_hover);

    if changed {
        events.push(GraphEvent::FilterChanged);
    }

    ui.spacing();
}

// ─── Section: Export ──────────────────────────────────────────────────────

/// Render the "Export" collapsing section.
fn render_export(ui: &Ui, graph: &GraphData, s: &crate::i18n::force_graph::Strings) {
    if !ui.collapsing_header(s.section_export, TreeNodeFlags::empty()) {
        return;
    }

    if ui.button(s.copy_svg) {
        set_clipboard(&super::render::export::export_svg(graph));
    }
    ui.same_line();
    if ui.button(s.copy_dot) {
        set_clipboard(&super::render::export::export_dot(graph));
    }
    ui.same_line();
    if ui.button(s.copy_mermaid) {
        set_clipboard(&super::render::export::export_mermaid(graph));
    }

    ui.spacing();
}

// ─── Section: Physics ─────────────────────────────────────────────────────

/// Render the "Physics" collapsing section.
fn render_physics(
    ui: &Ui,
    force_config: &mut ForceConfig,
    events: &mut Vec<GraphEvent>,
    s: &crate::i18n::force_graph::Strings,
) {
    if !ui.collapsing_header(s.section_physics, TreeNodeFlags::DEFAULT_OPEN) {
        return;
    }

    ui.text(s.link_dist);
    ui.slider(
        "##link_dist",
        20.0_f32,
        400.0_f32,
        &mut force_config.link_distance,
    );

    ui.text(s.repulsion);
    ui.slider(
        "##repulsion",
        0.0_f32,
        300.0_f32,
        &mut force_config.repulsion,
    );

    ui.text(s.attraction);
    ui.slider(
        "##attraction",
        0.0_f32,
        0.2_f32,
        &mut force_config.attraction,
    );

    ui.text(s.center_pull);
    ui.slider(
        "##center_pull",
        0.0_f32,
        0.01_f32,
        &mut force_config.center_pull,
    );

    ui.text(s.decay);
    ui.slider(
        "##decay",
        0.0_f32,
        1.0_f32,
        &mut force_config.velocity_decay,
    );

    ui.text(s.gravity);
    ui.slider(
        "##gravity",
        0.0_f32,
        0.5_f32,
        &mut force_config.gravity_strength,
    );

    ui.separator();
    ui.spacing();

    // -- simulation controls -------------------------------------------------
    if ui.button(s.btn_pause_resume) {
        // GraphViewer::render intercepts this and corrects the bool to actual sim state.
        events.push(GraphEvent::SimulationToggled(false));
    }
    ui.same_line();
    if ui.button(s.btn_reset_layout) {
        events.push(GraphEvent::ResetLayout);
    }

    ui.spacing();
}
