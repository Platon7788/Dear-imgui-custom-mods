//! Color-group query matching for the knowledge-graph renderer.
//!
//! Resolves a node's fill color by testing it against an ordered list of
//! [`ColorGroup`] entries. The first matching group wins; nodes that match no
//! group fall back to the caller's base color.

use super::super::config::{ColorGroup, ColorGroupQuery};
use super::super::style::{NodeKind, NodeStyle};

/// Resolve a node's color from the ordered `color_groups` list.
///
/// Iterates groups in order. Returns the color of the first group whose query
/// matches `style`. Returns `None` if no group matches (caller should fall
/// back to the active `ColorMode`).
pub(crate) fn resolve_group_color(
    style: &NodeStyle,
    color_groups: &[ColorGroup],
) -> Option<[f32; 4]> {
    for group in color_groups {
        if !group.enabled {
            continue;
        }
        if matches_query(style, &group.query) {
            return Some(group.color);
        }
    }
    None
}

/// Test whether a node's style matches a given [`ColorGroupQuery`].
fn matches_query(style: &NodeStyle, query: &ColorGroupQuery) -> bool {
    match query {
        ColorGroupQuery::Label(s) => contains_ignore_ascii_case(&style.label, s),
        ColorGroupQuery::Tag(s) => style
            .tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(s.as_str())),
        ColorGroupQuery::Kind(s) => kind_name_matches(style.kind, s),
        ColorGroupQuery::Regex(s) => {
            crate::utils::glob::glob_match(s, &style.label)
                || style.tags.iter().any(|t| crate::utils::glob::glob_match(s, t))
        }
        ColorGroupQuery::All => true,
    }
}

/// Check if `kind` matches the string name (case-insensitive, zero-alloc).
fn kind_name_matches(kind: NodeKind, name: &str) -> bool {
    match kind {
        NodeKind::Regular => name.eq_ignore_ascii_case("regular"),
        NodeKind::Tag => name.eq_ignore_ascii_case("tag"),
        NodeKind::Attachment => name.eq_ignore_ascii_case("attachment"),
        NodeKind::Unresolved => name.eq_ignore_ascii_case("unresolved"),
        NodeKind::Cluster => name.eq_ignore_ascii_case("cluster"),
        NodeKind::Custom(_) => name.eq_ignore_ascii_case("custom"),
    }
}

/// Case-insensitive ASCII substring check without heap allocation.
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    h.windows(n.len())
        .any(|w| w.eq_ignore_ascii_case(n))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::force_graph::style::NodeStyle;

    fn mk(label: &str) -> NodeStyle {
        NodeStyle::new(label)
    }

    #[test]
    fn label_match_case_insensitive() {
        let style = mk("Hello World");
        let g = ColorGroup::new("g", ColorGroupQuery::Label("hello".into()), [1.0; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([1.0; 4]));
    }

    #[test]
    fn tag_match() {
        let mut style = mk("node");
        style.tags = vec!["Work"];
        let g = ColorGroup::new("g", ColorGroupQuery::Tag("work".into()), [0.5; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([0.5; 4]));
    }

    #[test]
    fn disabled_group_skipped() {
        let style = mk("test");
        let mut g = ColorGroup::new("g", ColorGroupQuery::All, [1.0; 4]);
        g.enabled = false;
        assert_eq!(resolve_group_color(&style, &[g]), None);
    }

    #[test]
    fn first_match_wins() {
        let style = mk("alpha");
        let g1 = ColorGroup::new("g1", ColorGroupQuery::Label("alp".into()), [1.0, 0.0, 0.0, 1.0]);
        let g2 = ColorGroup::new("g2", ColorGroupQuery::All, [0.0, 1.0, 0.0, 1.0]);
        let result = resolve_group_color(&style, &[g1, g2]);
        assert_eq!(result, Some([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn kind_match() {
        let style = mk("tag_node").with_kind(NodeKind::Tag);
        let g = ColorGroup::new("tags", ColorGroupQuery::Kind("tag".into()), [0.8, 0.2, 0.2, 1.0]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([0.8, 0.2, 0.2, 1.0]));
    }

    #[test]
    fn no_match_returns_none() {
        let style = mk("zzz");
        let g = ColorGroup::new("g", ColorGroupQuery::Label("abc".into()), [1.0; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), None);
    }

    #[test]
    fn regex_glob_wildcard_star() {
        let style = mk("function_call");
        let g = ColorGroup::new("g", ColorGroupQuery::Regex("func*call".into()), [1.0; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([1.0; 4]));
        // Glob tests proper are in crate::utils::glob::tests.
    }

    #[test]
    fn regex_glob_question_mark() {
        let style = mk("node1");
        let g = ColorGroup::new("g", ColorGroupQuery::Regex("node?".into()), [1.0; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([1.0; 4]));
    }

    #[test]
    fn regex_glob_no_wildcard_falls_back_to_substring() {
        let style = mk("alpha_beta");
        let g = ColorGroup::new("g", ColorGroupQuery::Regex("beta".into()), [1.0; 4]);
        assert_eq!(resolve_group_color(&style, &[g]), Some([1.0; 4]));
    }
}
