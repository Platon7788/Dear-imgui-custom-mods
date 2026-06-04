//! Unit tests for [`TreeArena`](super::TreeArena), split out of `arena/mod.rs`.

use super::*;

#[test]
fn empty_arena() {
    let arena: TreeArena<i32> = TreeArena::new();
    assert_eq!(arena.node_count(), 0);
    assert_eq!(arena.roots().len(), 0);
    assert_eq!(arena.slot_len(), 0);
}

#[test]
fn insert_root() {
    let mut arena = TreeArena::new();
    let id = arena.insert_root(42).unwrap();
    assert_eq!(arena.node_count(), 1);
    assert_eq!(arena.roots().len(), 1);
    assert_eq!(arena.get_data(id), Some(&42));
    assert_eq!(arena.depth(id), Some(0));
    assert!(arena.parent(id).is_none());
    assert!(arena.children(id).is_empty());
}

#[test]
fn insert_children_and_depth() {
    let mut arena = TreeArena::new();
    let root = arena.insert_root("root").unwrap();
    let child = arena.insert_child(root, "child").unwrap();
    let grandchild = arena.insert_child(child, "grandchild").unwrap();

    assert_eq!(arena.node_count(), 3);
    assert_eq!(arena.depth(root), Some(0));
    assert_eq!(arena.depth(child), Some(1));
    assert_eq!(arena.depth(grandchild), Some(2));
    assert_eq!(arena.parent(child), Some(root));
    assert_eq!(arena.parent(grandchild), Some(child));
    assert_eq!(arena.children(root).len(), 1);
    assert_eq!(arena.children(child).len(), 1);
}

#[test]
fn insert_child_at_position() {
    let mut arena = TreeArena::new();
    let root = arena.insert_root(0).unwrap();
    let a = arena.insert_child(root, 1).unwrap();
    let c = arena.insert_child(root, 3).unwrap();
    let b = arena.insert_child_at(root, 1, 2).unwrap(); // insert between a and c

    let children = arena.children(root);
    assert_eq!(children.len(), 3);
    assert_eq!(arena.get_data(children[0]), Some(&1));
    assert_eq!(arena.get_data(children[1]), Some(&2));
    assert_eq!(arena.get_data(children[2]), Some(&3));
    let _ = (a, b, c); // silence unused
}

#[test]
fn remove_leaf() {
    let mut arena = TreeArena::new();
    let root = arena.insert_root("root").unwrap();
    let child = arena.insert_child(root, "child").unwrap();
    assert_eq!(arena.node_count(), 2);

    let removed = arena.remove(child);
    assert_eq!(removed, Some("child"));
    assert_eq!(arena.node_count(), 1);
    assert!(arena.children(root).is_empty());
    assert!(arena.get_data(child).is_none()); // stale ID
}

#[test]
fn remove_subtree() {
    let mut arena = TreeArena::new();
    let root = arena.insert_root(0).unwrap();
    let a = arena.insert_child(root, 1).unwrap();
    let _b = arena.insert_child(a, 2).unwrap();
    let _c = arena.insert_child(a, 3).unwrap();
    assert_eq!(arena.node_count(), 4);

    arena.remove(a);
    assert_eq!(arena.node_count(), 1); // only root remains
    assert!(arena.children(root).is_empty());
}

#[test]
fn generational_id_safety() {
    let mut arena = TreeArena::new();
    let id1 = arena.insert_root(100).unwrap();
    arena.remove(id1);
    // Slot is reused but generation incremented
    let id2 = arena.insert_root(200).unwrap();
    assert_eq!(id1.index, id2.index); // same slot
    assert_ne!(id1.generation, id2.generation); // different generation
    assert!(arena.get_data(id1).is_none()); // stale
    assert_eq!(arena.get_data(id2), Some(&200)); // valid
}

#[test]
fn capacity_limit() {
    let mut arena = TreeArena::with_capacity(3);
    assert_eq!(arena.capacity(), 3);
    let _a = arena.insert_root(1).unwrap();
    let _b = arena.insert_root(2).unwrap();
    let _c = arena.insert_root(3).unwrap();
    assert!(arena.insert_root(4).is_none()); // at capacity
    assert_eq!(arena.node_count(), 3);
}

#[test]
fn evict_on_overflow() {
    let mut arena = TreeArena::with_capacity(3);
    arena.set_evict_on_overflow(true);
    let a = arena.insert_root(1).unwrap();
    let _b = arena.insert_root(2).unwrap();
    let _c = arena.insert_root(3).unwrap();
    // Inserting a 4th should evict oldest root (a)
    let _d = arena.insert_root(4).unwrap();
    assert_eq!(arena.node_count(), 3);
    assert!(arena.get_data(a).is_none()); // evicted
    assert_eq!(arena.roots().len(), 3);
}

#[test]
fn expand_collapse() {
    let mut arena = TreeArena::new();
    let id = arena.insert_root(0).unwrap();
    assert!(!arena.is_expanded(id));
    arena.expand(id);
    assert!(arena.is_expanded(id));
    arena.collapse(id);
    assert!(!arena.is_expanded(id));
    arena.toggle(id);
    assert!(arena.is_expanded(id));
}

#[test]
fn expand_all_collapse_all() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    let c = arena.insert_child(r, 1).unwrap();
    arena.expand_all();
    assert!(arena.is_expanded(r));
    assert!(arena.is_expanded(c));
    arena.collapse_all();
    assert!(!arena.is_expanded(r));
    assert!(!arena.is_expanded(c));
}

#[test]
fn ensure_visible() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    let c = arena.insert_child(r, 1).unwrap();
    let gc = arena.insert_child(c, 2).unwrap();
    assert!(!arena.is_expanded(r));
    assert!(!arena.is_expanded(c));
    arena.ensure_visible(gc);
    assert!(arena.is_expanded(r));
    assert!(arena.is_expanded(c));
}

#[test]
fn move_node_reparent() {
    let mut arena = TreeArena::new();
    let r1 = arena.insert_root(1).unwrap();
    let r2 = arena.insert_root(2).unwrap();
    let child = arena.insert_child(r1, 10).unwrap();

    assert_eq!(arena.children(r1).len(), 1);
    assert!(arena.children(r2).is_empty());

    assert!(arena.move_node(child, Some(r2), 0));
    assert!(arena.children(r1).is_empty());
    assert_eq!(arena.children(r2).len(), 1);
    assert_eq!(arena.parent(child), Some(r2));
    assert_eq!(arena.depth(child), Some(1));
}

#[test]
fn move_node_to_root() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    let child = arena.insert_child(r, 1).unwrap();

    assert!(arena.move_node(child, None, 0));
    assert!(arena.parent(child).is_none());
    assert_eq!(arena.depth(child), Some(0));
    assert_eq!(arena.roots().len(), 2);
}

#[test]
fn move_node_into_own_subtree_fails() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    let c = arena.insert_child(r, 1).unwrap();
    let gc = arena.insert_child(c, 2).unwrap();

    assert!(!arena.move_node(r, Some(gc), 0)); // can't move parent into its own descendant
    let _ = gc;
}

#[test]
fn clear() {
    let mut arena: TreeArena<String> = TreeArena::new();
    arena.insert_root("a".into());
    arena.insert_root("b".into());
    arena.clear();
    assert_eq!(arena.node_count(), 0);
    assert!(arena.roots().is_empty());
}

#[test]
fn sort_children() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(0).unwrap();
    arena.insert_child(r, 30).unwrap();
    arena.insert_child(r, 10).unwrap();
    arena.insert_child(r, 20).unwrap();

    arena.sort_children(Some(r), &mut |a, b| a.cmp(b));
    let sorted: Vec<_> = arena
        .children(r)
        .iter()
        .filter_map(|&id| arena.get_data(id).copied())
        .collect();
    assert_eq!(sorted, vec![10, 20, 30]);
}

#[test]
fn iter_all_nodes() {
    let mut arena = TreeArena::new();
    let r = arena.insert_root(1).unwrap();
    arena.insert_child(r, 2).unwrap();
    arena.insert_child(r, 3).unwrap();

    let mut vals: Vec<_> = arena.iter().map(|(_, &v)| v).collect();
    vals.sort();
    assert_eq!(vals, vec![1, 2, 3]);
}

#[test]
fn insert_root_at_position() {
    let mut arena = TreeArena::new();
    arena.insert_root(1).unwrap();
    arena.insert_root(3).unwrap();
    arena.insert_root_at(1, 2).unwrap(); // between 1 and 3

    let root_vals: Vec<_> = arena
        .roots()
        .iter()
        .filter_map(|&id| arena.get_data(id).copied())
        .collect();
    assert_eq!(root_vals, vec![1, 2, 3]);
}

#[test]
fn set_capacity_runtime() {
    let mut arena = TreeArena::<i32>::new();
    arena.set_capacity(5);
    assert_eq!(arena.capacity(), 5);
    arena.set_capacity(0); // clamped to 1
    assert_eq!(arena.capacity(), 1);
}

#[test]
fn capacity_clamped_to_max() {
    let arena = TreeArena::<()>::with_capacity(usize::MAX);
    assert_eq!(arena.capacity(), MAX_TREE_NODES);
}
