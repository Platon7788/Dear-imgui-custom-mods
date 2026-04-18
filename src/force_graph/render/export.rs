//! SVG, DOT (Graphviz), and Mermaid export for the force-graph widget.
//!
//! All three functions are pure — they take a `&GraphData` snapshot and return
//! a UTF-8 string ready for writing to a file or copying to the clipboard.
//! Node world-space positions are baked into SVG output so the exported
//! diagram matches what the user sees.

use super::super::data::GraphData;

// ─── SVG ─────────────────────────────────────────────────────────────────────

/// Export the graph as a standalone SVG string.
///
/// Node positions from the physics simulation are preserved. Edges are drawn
/// as straight lines. Labels appear below each node circle.
pub(crate) fn export_svg(graph: &GraphData) -> String {
    if graph.node_count() == 0 {
        return r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"></svg>"#.into();
    }

    // Compute bounding box.
    let mut gmin = [f32::INFINITY; 2];
    let mut gmax = [f32::NEG_INFINITY; 2];
    for (_, node) in graph.nodes.iter() {
        gmin[0] = gmin[0].min(node.pos[0]);
        gmin[1] = gmin[1].min(node.pos[1]);
        gmax[0] = gmax[0].max(node.pos[0]);
        gmax[1] = gmax[1].max(node.pos[1]);
    }

    let pad = 40.0_f32;
    let vw = (gmax[0] - gmin[0] + pad * 2.0).max(200.0);
    let vh = (gmax[1] - gmin[1] + pad * 2.0).max(200.0);

    let proj = |p: [f32; 2]| -> [f32; 2] {
        [p[0] - gmin[0] + pad, p[1] - gmin[1] + pad]
    };

    let mut s = String::with_capacity(graph.node_count() * 128 + graph.edge_count() * 64);
    s.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{vw:.0}" height="{vh:.0}" viewBox="0 0 {vw:.0} {vh:.0}" style="background:#1a1a2e;font-family:sans-serif">"#
    ));

    // Edges.
    s.push_str(r##"<g stroke="#556" stroke-width="1" opacity="0.75">"##);
    for (_, edge) in graph.edges.iter() {
        let Some(na) = graph.nodes.get(edge.from) else { continue };
        let Some(nb) = graph.nodes.get(edge.to) else { continue };
        let [ax, ay] = proj(na.pos);
        let [bx, by] = proj(nb.pos);
        if edge.directed {
            s.push_str(&format!(
                r#"<line x1="{ax:.1}" y1="{ay:.1}" x2="{bx:.1}" y2="{by:.1}" marker-end="url(#arr)"/>"#
            ));
        } else {
            s.push_str(&format!(
                r#"<line x1="{ax:.1}" y1="{ay:.1}" x2="{bx:.1}" y2="{by:.1}"/>"#
            ));
        }
    }
    s.push_str("</g>");

    // Arrow marker definition (only if any directed edges exist).
    if graph.edges().any(|(_, e)| e.directed) {
        s.push_str(r##"<defs><marker id="arr" markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#888"/></marker></defs>"##);
    }

    // Nodes.
    for (_, node) in graph.nodes.iter() {
        let [cx, cy] = proj(node.pos);
        let r = node.style.radius.unwrap_or(8.0).max(3.0);
        let fill = node.style.color.map_or_else(
            || "#6ab0f5".into(),
            |[rv, g, b, _]| format!(
                "#{:02x}{:02x}{:02x}",
                (rv * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8
            ),
        );
        s.push_str(&format!(
            r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="{r:.1}" fill="{fill}" stroke="#fff" stroke-width="0.8"/>"##
        ));
        if !node.style.label.is_empty() {
            let label = xml_escape(&node.style.label);
            s.push_str(&format!(
                r##"<text x="{cx:.1}" y="{:.1}" font-size="9" text-anchor="middle" fill="#ccc">{label}</text>"##,
                cy + r + 10.0
            ));
        }
    }

    s.push_str("</svg>");
    s
}

// ─── DOT / Graphviz ───────────────────────────────────────────────────────────

/// Export the graph in Graphviz DOT format.
///
/// Nodes are emitted with their labels; edges carry a `weight` attribute.
/// Uses `digraph` when any edge is directed, `graph` otherwise.
pub(crate) fn export_dot(graph: &GraphData) -> String {
    let node_ids: Vec<_> = graph.nodes().map(|(id, _)| id).collect();
    let idx = move |id| node_ids.iter().position(|&x| x == id).unwrap_or(0);

    let directed = graph.edges().any(|(_, e)| e.directed);
    let kw    = if directed { "digraph" } else { "graph" };
    let arrow = if directed { "->" } else { "--" };

    let mut s = String::with_capacity(graph.node_count() * 48 + graph.edge_count() * 32);
    s.push_str(&format!("{kw} G {{\n"));
    s.push_str("  node [shape=circle fontname=sans style=filled fillcolor=\"#6ab0f5\"];\n");

    for (i, (id, style)) in graph.nodes().enumerate() {
        let _ = id; // index drives the node id
        let label = dot_escape(&style.label);
        s.push_str(&format!("  n{i} [label=\"{label}\"];\n"));
    }

    for (_, edge) in graph.edges() {
        let a = idx(edge.from);
        let b = idx(edge.to);
        s.push_str(&format!("  n{a} {arrow} n{b} [weight={:.2}];\n", edge.weight));
    }

    s.push('}');
    s
}

// ─── Mermaid ─────────────────────────────────────────────────────────────────

/// Export the graph in Mermaid flowchart syntax.
///
/// Compatible with the Mermaid live editor and most Markdown renderers that
/// support fenced `mermaid` code blocks.
pub(crate) fn export_mermaid(graph: &GraphData) -> String {
    let node_ids: Vec<_> = graph.nodes().map(|(id, _)| id).collect();
    let idx = move |id| node_ids.iter().position(|&x| x == id).unwrap_or(0);

    let directed = graph.edges().any(|(_, e)| e.directed);
    let arrow = if directed { "-->" } else { "---" };

    let mut s = String::with_capacity(graph.node_count() * 32 + graph.edge_count() * 24);
    s.push_str("flowchart LR\n");

    for (i, (_, style)) in graph.nodes().enumerate() {
        let label = mermaid_escape(&style.label);
        s.push_str(&format!("  n{i}[\"{label}\"]\n"));
    }

    for (_, edge) in graph.edges() {
        let a = idx(edge.from);
        let b = idx(edge.to);
        s.push_str(&format!("  n{a} {arrow} n{b}\n"));
    }

    s
}

// ─── Escape helpers ───────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '&'  => out.push_str("&amp;"),
            '<'  => out.push_str("&lt;"),
            '>'  => out.push_str("&gt;"),
            '"'  => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _    => out.push(c),
        }
    }
    out
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn mermaid_escape(s: &str) -> String {
    s.replace('"', "#quot;")
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::force_graph::data::GraphData;
    use crate::force_graph::style::{EdgeStyle, NodeStyle};

    #[test]
    fn svg_export_wraps_in_svg_tags() {
        let mut g = GraphData::new();
        g.add_node(NodeStyle::new("A"));
        let out = export_svg(&g);
        assert!(out.starts_with("<svg "), "expected <svg …>, got: {out}");
        assert!(out.ends_with("</svg>"), "expected </svg> at end");
    }

    #[test]
    fn svg_export_empty_graph_is_valid() {
        let g = GraphData::new();
        let out = export_svg(&g);
        assert!(out.contains("svg"));
    }

    #[test]
    fn dot_export_has_graph_keyword() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("X"));
        let b = g.add_node(NodeStyle::new("Y"));
        g.add_edge(a, b, EdgeStyle::new(), 1.0, false);
        let out = export_dot(&g);
        assert!(out.starts_with("graph G"), "expected undirected, got: {out}");
        assert!(out.contains("n0 -- n1"));
    }

    #[test]
    fn dot_export_directed_uses_digraph() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("A"));
        let b = g.add_node(NodeStyle::new("B"));
        g.add_edge(a, b, EdgeStyle::new(), 1.0, true);
        let out = export_dot(&g);
        assert!(out.starts_with("digraph G"), "expected digraph, got: {out}");
        assert!(out.contains("n0 -> n1"));
    }

    #[test]
    fn mermaid_export_has_flowchart_header() {
        let mut g = GraphData::new();
        let a = g.add_node(NodeStyle::new("Alpha"));
        let b = g.add_node(NodeStyle::new("Beta"));
        g.add_edge(a, b, EdgeStyle::new(), 0.5, false);
        let out = export_mermaid(&g);
        assert!(out.starts_with("flowchart LR"), "got: {out}");
        assert!(out.contains("n0[\"Alpha\"]"), "got: {out}");
        assert!(out.contains("n0 --- n1"), "got: {out}");
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("<A & B>"), "&lt;A &amp; B&gt;");
    }

    #[test]
    fn dot_escape_quotes() {
        assert_eq!(dot_escape(r#"say "hello""#), r#"say \"hello\""#);
    }
}
