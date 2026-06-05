//! `crate::force_graph` localisation strings.

#![allow(missing_docs)]

use super::Locale;

/// All user-visible labels rendered by the force-graph sidebar
/// and the right-click context menu.
#[derive(Debug)]
pub struct Strings {
    // Section headers
    pub section_filters: &'static str,
    pub section_color_groups: &'static str,
    pub section_display: &'static str,
    pub section_export: &'static str,
    pub section_physics: &'static str,

    // Filters
    pub show_orphan_nodes: &'static str,
    pub hide_unresolved_links: &'static str,
    pub hide_tag_nodes: &'static str,
    pub search_label: &'static str,
    pub depth_label: &'static str,
    pub time_travel_label: &'static str,
    pub btn_all: &'static str,

    // Color Groups
    pub no_groups_defined: &'static str,
    pub add_group: &'static str,
    pub new_group_default_name: &'static str,
    pub query_label: &'static str,
    pub query_tag: &'static str,
    pub query_kind: &'static str,
    pub query_regex: &'static str,
    pub query_all: &'static str,

    // Display
    pub node_size: &'static str,
    pub edge_width: &'static str,
    pub text_fade: &'static str,
    pub hover_fade: &'static str,
    pub edge_curve: &'static str,
    pub toggle_arrows: &'static str,
    pub toggle_edge_labels: &'static str,
    pub toggle_background_grid: &'static str,
    pub toggle_glow_on_hover: &'static str,

    // Export
    pub copy_svg: &'static str,
    pub copy_dot: &'static str,
    pub copy_mermaid: &'static str,

    // Physics
    pub link_dist: &'static str,
    pub repulsion: &'static str,
    pub attraction: &'static str,
    pub center_pull: &'static str,
    pub decay: &'static str,
    pub gravity: &'static str,
    pub btn_pause_resume: &'static str,
    pub btn_reset_layout: &'static str,

    // Context menu
    pub menu_pin: &'static str,
    pub menu_unpin: &'static str,
    pub menu_select_neighbours: &'static str,
    pub menu_focus_here: &'static str,
    pub menu_clear_focus: &'static str,
    pub menu_activate: &'static str,
}

pub const EN: Strings = Strings {
    section_filters: "Filters",
    section_color_groups: "Color Groups",
    section_display: "Display",
    section_export: "Export",
    section_physics: "Physics",

    show_orphan_nodes: "Show orphan nodes",
    hide_unresolved_links: "Hide unresolved links",
    hide_tag_nodes: "Hide tag nodes",
    search_label: "Search:",
    depth_label: "Depth (0=all):",
    time_travel_label: "Time travel:",
    btn_all: "All",

    no_groups_defined: "No groups defined",
    add_group: "+ Add Group",
    new_group_default_name: "New group",
    query_label: "Query: label",
    query_tag: "Query: tag",
    query_kind: "Query: kind",
    query_regex: "Query: regex",
    query_all: "Query: all",

    node_size: "Node size:",
    edge_width: "Edge width:",
    text_fade: "Text fade:",
    hover_fade: "Hover fade:",
    edge_curve: "Edge curve:",
    toggle_arrows: "Arrows",
    toggle_edge_labels: "Edge labels",
    toggle_background_grid: "Background grid",
    toggle_glow_on_hover: "Glow on hover",

    copy_svg: "Copy SVG",
    copy_dot: "Copy DOT",
    copy_mermaid: "Copy Mermaid",

    link_dist: "Link dist:",
    repulsion: "Repulsion:",
    attraction: "Attraction:",
    center_pull: "Center pull:",
    decay: "Decay:",
    gravity: "Gravity:",
    btn_pause_resume: "Pause/Resume",
    btn_reset_layout: "Reset Layout",

    menu_pin: "Pin",
    menu_unpin: "Unpin",
    menu_select_neighbours: "Select neighbours",
    menu_focus_here: "Focus here",
    menu_clear_focus: "Clear focus",
    menu_activate: "Activate",
};

pub const RU: Strings = Strings {
    section_filters: "Фильтры",
    section_color_groups: "Цветовые группы",
    section_display: "Отображение",
    section_export: "Экспорт",
    section_physics: "Физика",

    show_orphan_nodes: "Сиротские узлы",
    hide_unresolved_links: "Скрыть нерасреш. ссылки",
    hide_tag_nodes: "Скрыть теги",
    search_label: "Поиск:",
    depth_label: "Глубина (0=все):",
    time_travel_label: "По времени:",
    btn_all: "Все",

    no_groups_defined: "Группы не заданы",
    add_group: "+ Добавить группу",
    new_group_default_name: "Новая группа",
    query_label: "Запрос: метка",
    query_tag: "Запрос: тег",
    query_kind: "Запрос: тип",
    query_regex: "Запрос: regex",
    query_all: "Запрос: все",

    node_size: "Размер узла:",
    edge_width: "Толщина ребра:",
    text_fade: "Затух. текста:",
    hover_fade: "Затух. при ховере:",
    edge_curve: "Изгиб ребра:",
    toggle_arrows: "Стрелки",
    toggle_edge_labels: "Подписи рёбер",
    toggle_background_grid: "Сетка фона",
    toggle_glow_on_hover: "Свечение при ховере",

    copy_svg: "Копировать SVG",
    copy_dot: "Копировать DOT",
    copy_mermaid: "Копировать Mermaid",

    link_dist: "Дист. связи:",
    repulsion: "Отталкивание:",
    attraction: "Притяжение:",
    center_pull: "К центру:",
    decay: "Затухание:",
    gravity: "Гравитация:",
    btn_pause_resume: "Пауза/Возобн.",
    btn_reset_layout: "Сбросить раскладку",

    menu_pin: "Закрепить",
    menu_unpin: "Открепить",
    menu_select_neighbours: "Выделить соседей",
    menu_focus_here: "Фокус здесь",
    menu_clear_focus: "Сбросить фокус",
    menu_activate: "Активировать",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
