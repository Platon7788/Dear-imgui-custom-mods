//! Unit tests for [`super::Graph`] — slab insert/remove, wire ops,
//! connection validation, and comments.

use super::*;

#[test]
fn insert_and_get() {
    let mut g = Graph::new();
    let id = g.insert_node("hello", [10.0, 20.0]);
    assert_eq!(g.node_count(), 1);
    let node = g.get_node(id).unwrap();
    assert_eq!(node.value, "hello");
    assert_eq!(node.pos, [10.0, 20.0]);
    assert!(node.open);
}

#[test]
fn remove_node_returns_value() {
    let mut g = Graph::new();
    let id = g.insert_node(42, [0.0, 0.0]);
    let val = g.remove_node(id);
    assert_eq!(val, Some(42));
    assert_eq!(g.node_count(), 0);
    assert!(g.get_node(id).is_none());
}

#[test]
fn remove_nonexistent() {
    let mut g: Graph<i32> = Graph::new();
    let id = NodeId(99);
    assert!(g.remove_node(id).is_none());
}

#[test]
fn slab_reuse() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let _b = g.insert_node("b", [1.0, 0.0]);
    g.remove_node(a);
    // Next insert should reuse slot 0
    let c = g.insert_node("c", [2.0, 0.0]);
    assert_eq!(c.index(), a.index());
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.get_node(c).unwrap().value, "c");
}

#[test]
fn connect_disconnect() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let out = OutPinId { node: a, output: 0 };
    let inp = InPinId { node: b, input: 0 };

    assert!(g.connect(out, inp));
    assert!(!g.connect(out, inp)); // duplicate
    assert_eq!(g.wire_count(), 1);
    assert!(g.has_wire(out, inp));

    assert!(g.disconnect(out, inp));
    assert_eq!(g.wire_count(), 0);
    assert!(!g.has_wire(out, inp));
}

#[test]
fn remove_node_removes_wires() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let c = g.insert_node("c", [200.0, 0.0]);
    g.connect(
        OutPinId { node: a, output: 0 },
        InPinId { node: b, input: 0 },
    );
    g.connect(
        OutPinId { node: b, output: 0 },
        InPinId { node: c, input: 0 },
    );
    assert_eq!(g.wire_count(), 2);
    g.remove_node(b);
    assert_eq!(g.wire_count(), 0);
}

/// Regression: after a node is removed, no surviving wire may reference it.
/// Guards against dangling wire endpoints (the slab slot can be reused by a
/// later insert, which would otherwise silently re-home stale wires).
#[test]
fn remove_node_leaves_no_dangling_endpoints() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let c = g.insert_node("c", [200.0, 0.0]);
    // a -> b, b -> c, a -> c
    g.connect(
        OutPinId { node: a, output: 0 },
        InPinId { node: b, input: 0 },
    );
    g.connect(
        OutPinId { node: b, output: 0 },
        InPinId { node: c, input: 0 },
    );
    g.connect(
        OutPinId { node: a, output: 1 },
        InPinId { node: c, input: 1 },
    );
    g.remove_node(b);
    // Only a -> c may survive; nothing touching `b`.
    for w in g.wires() {
        assert_ne!(
            w.out_pin.node, b,
            "wire output still references removed node"
        );
        assert_ne!(w.in_pin.node, b, "wire input still references removed node");
    }
    assert_eq!(g.wire_count(), 1);
}

#[test]
fn input_output_remotes() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let out = OutPinId { node: a, output: 0 };
    let inp = InPinId { node: b, input: 0 };
    g.connect(out, inp);

    assert_eq!(g.input_remotes(inp), vec![out]);
    assert_eq!(g.output_remotes(out), vec![inp]);
}

#[test]
fn drop_inputs_outputs() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let out0 = OutPinId { node: a, output: 0 };
    let out1 = OutPinId { node: a, output: 1 };
    let inp0 = InPinId { node: b, input: 0 };
    let inp1 = InPinId { node: b, input: 1 };
    g.connect(out0, inp0);
    g.connect(out1, inp1);
    assert_eq!(g.wire_count(), 2);

    g.drop_inputs(inp0);
    assert_eq!(g.wire_count(), 1);

    g.drop_outputs(out1);
    assert_eq!(g.wire_count(), 0);
}

#[test]
fn nodes_iter() {
    let mut g = Graph::new();
    g.insert_node("a", [0.0, 0.0]);
    g.insert_node("b", [1.0, 0.0]);
    g.insert_node("c", [2.0, 0.0]);
    let ids: Vec<_> = g.nodes().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn node_ids_collect() {
    let mut g = Graph::new();
    let a = g.insert_node(1, [0.0, 0.0]);
    let _b = g.insert_node(2, [0.0, 0.0]);
    g.remove_node(a);
    let ids = g.node_ids();
    assert_eq!(ids.len(), 1);
}

#[test]
fn clear_graph() {
    let mut g = Graph::new();
    g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    g.connect(
        OutPinId {
            node: NodeId(0),
            output: 0,
        },
        InPinId { node: b, input: 0 },
    );
    g.clear();
    assert_eq!(g.node_count(), 0);
    assert_eq!(g.wire_count(), 0);
}

#[test]
fn get_node_mut() {
    let mut g = Graph::new();
    let id = g.insert_node("old", [0.0, 0.0]);
    g.get_node_mut(id).unwrap().value = "new";
    assert_eq!(g.get_node(id).unwrap().value, "new");
}

#[test]
fn double_remove() {
    let mut g = Graph::new();
    let id = g.insert_node(1, [0.0, 0.0]);
    assert!(g.remove_node(id).is_some());
    assert!(g.remove_node(id).is_none());
    assert_eq!(g.node_count(), 0);
}

// ── Connection validation (can_connect_basic) ─────────────────────────────

/// Regression: a self-loop (output → input on the *same* node) is rejected
/// even though the viewer default `can_connect` returns `true`.
#[test]
fn can_connect_basic_rejects_self_loop() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let out = OutPinId { node: a, output: 0 };
    let inp = InPinId { node: a, input: 0 };
    assert!(!g.can_connect_basic(out, inp), "self-loop must be rejected");
}

#[test]
fn can_connect_basic_allows_distinct_live_nodes() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let b = g.insert_node("b", [100.0, 0.0]);
    let out = OutPinId { node: a, output: 0 };
    let inp = InPinId { node: b, input: 0 };
    assert!(g.can_connect_basic(out, inp));
}

/// Regression: a connection naming a removed/dangling node is rejected.
#[test]
fn can_connect_basic_rejects_dangling_endpoint() {
    let mut g = Graph::new();
    let a = g.insert_node("a", [0.0, 0.0]);
    let ghost = NodeId(999);
    let out = OutPinId { node: a, output: 0 };
    let inp = InPinId {
        node: ghost,
        input: 0,
    };
    assert!(!g.can_connect_basic(out, inp));

    let out2 = OutPinId {
        node: ghost,
        output: 0,
    };
    let inp2 = InPinId { node: a, input: 0 };
    assert!(!g.can_connect_basic(out2, inp2));
}

// ── Comments ─────────────────────────────────────────────────────────

fn sample_comment(text: &str) -> Comment {
    Comment {
        pos: [10.0, 20.0],
        size: [200.0, 120.0],
        text: text.to_string(),
        color: [0x5b, 0x9b, 0xd5],
    }
}

#[test]
fn add_comment_returns_index() {
    let mut g: Graph<i32> = Graph::new();
    assert!(g.comments().is_empty());
    let a = g.add_comment(sample_comment("a"));
    let b = g.add_comment(sample_comment("b"));
    assert_eq!(a, 0);
    assert_eq!(b, 1);
    assert_eq!(g.comments().len(), 2);
    assert_eq!(g.comments()[0].text, "a");
    assert_eq!(g.comments()[1].text, "b");
}

#[test]
fn remove_comment_shifts_indices() {
    let mut g: Graph<i32> = Graph::new();
    g.add_comment(sample_comment("a"));
    g.add_comment(sample_comment("b"));
    g.add_comment(sample_comment("c"));
    g.remove_comment(1); // remove "b"
    assert_eq!(g.comments().len(), 2);
    assert_eq!(g.comments()[0].text, "a");
    assert_eq!(g.comments()[1].text, "c");
}

#[test]
fn remove_comment_out_of_range_is_noop() {
    let mut g: Graph<i32> = Graph::new();
    g.add_comment(sample_comment("a"));
    g.remove_comment(99); // out of range
    assert_eq!(g.comments().len(), 1);
}

#[test]
fn comments_mut_bulk_edit() {
    let mut g: Graph<i32> = Graph::new();
    g.add_comment(sample_comment("a"));
    g.comments_mut().push(sample_comment("loaded"));
    g.comments_mut()[0].text = "edited".to_string();
    assert_eq!(g.comments().len(), 2);
    assert_eq!(g.comments()[0].text, "edited");
    assert_eq!(g.comments()[1].text, "loaded");
}

#[test]
fn clear_comments_empties_list() {
    let mut g: Graph<i32> = Graph::new();
    g.add_comment(sample_comment("a"));
    g.add_comment(sample_comment("b"));
    g.clear_comments();
    assert!(g.comments().is_empty());
}

#[test]
fn clear_also_clears_comments() {
    let mut g: Graph<i32> = Graph::new();
    g.insert_node(1, [0.0, 0.0]);
    g.add_comment(sample_comment("a"));
    g.clear();
    assert_eq!(g.node_count(), 0);
    assert!(g.comments().is_empty());
}

/// UTF-8 safety: multi-byte node labels round-trip through the slab unchanged.
#[test]
fn utf8_node_value_roundtrip() {
    let mut g = Graph::new();
    let id = g.insert_node("Узел — 日本語 🎛".to_string(), [0.0, 0.0]);
    assert_eq!(g.get_node(id).unwrap().value, "Узел — 日本語 🎛");
}
