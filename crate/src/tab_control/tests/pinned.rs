//! Pinned-prefix invariant: `add` insertion, `enforce_pinned_partition`
//! repair, and `move_tab` group clamping.

use super::super::*;
use super::{Spy, names_of, push_raw};

// ─── pinned invariant ───────────────────────────────────────────────────────

#[test]
fn pinned_inserted_after_existing_pinned() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("r2"));
    pc.add(Spy::new("p2").pinned());
    // Pinned must occupy the contiguous prefix:  [p1, p2, r1, r2].
    assert_eq!(names_of(&pc), vec!["p1", "p2", "r1", "r2"]);
}

#[test]
fn pinned_invariant_after_mixed_adds() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("p2").pinned());
    pc.add(Spy::new("p3").pinned());
    pc.add(Spy::new("r2"));
    assert_eq!(names_of(&pc), vec!["p1", "p2", "p3", "r1", "r2"]);
}

#[test]
fn add_keeps_pinned_prefix_through_many_inserts() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    for i in 0..10 {
        let s = if i % 3 == 0 {
            Spy::new(&format!("p{i}")).pinned()
        } else {
            Spy::new(&format!("r{i}"))
        };
        pc.add(s);
        // After every add, the pinned prefix must hold.
        let mut seen_regular = false;
        for t in pc.iter() {
            if t.1.is_pinned() {
                assert!(!seen_regular, "broken pinned prefix at {:?}", names_of(&pc));
            } else {
                seen_regular = true;
            }
        }
    }
}

// ─── enforce_pinned_partition ───────────────────────────────────────────────

#[test]
fn enforce_pinned_partition_repairs_arbitrary_order() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    // Bypass add()'s smart insertion by directly pushing into pc.tabs.
    push_raw(&mut pc, 1, Spy::new("r1"));
    push_raw(&mut pc, 2, Spy::new("p1").pinned());
    push_raw(&mut pc, 3, Spy::new("r2"));
    push_raw(&mut pc, 4, Spy::new("p2").pinned());
    pc.next_id = 5;

    pc.enforce_pinned_partition();
    assert_eq!(names_of(&pc), vec!["p1", "p2", "r1", "r2"]);
}

#[test]
fn enforce_pinned_partition_noop_when_already_partitioned() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("p2").pinned());
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("r2"));
    let gen_before = pc.tab_gen;
    pc.enforce_pinned_partition();
    // No re-arrangement → tab_gen must NOT bump.
    assert_eq!(pc.tab_gen, gen_before);
}

#[test]
fn enforce_pinned_partition_preserves_relative_order() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    // Push directly to bypass add()'s smart insertion.
    let pushes = [
        ("r1", false),
        ("p1", true),
        ("r2", false),
        ("p2", true),
        ("r3", false),
        ("p3", true),
    ];
    for (i, (name, pinned)) in pushes.iter().enumerate() {
        let mut s = Spy::new(name);
        s.pinned = *pinned;
        push_raw(&mut pc, (i + 1) as u64, s);
    }
    pc.next_id = 7;
    pc.enforce_pinned_partition();
    assert_eq!(names_of(&pc), vec!["p1", "p2", "p3", "r1", "r2", "r3"]);
}

#[test]
fn enforce_pinned_partition_handles_all_pinned() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a").pinned());
    pc.add(Spy::new("b").pinned());
    pc.add(Spy::new("c").pinned());
    pc.enforce_pinned_partition();
    assert_eq!(names_of(&pc), vec!["a", "b", "c"]);
}

#[test]
fn enforce_pinned_partition_handles_all_regular() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    pc.add(Spy::new("c"));
    pc.enforce_pinned_partition();
    assert_eq!(names_of(&pc), vec!["a", "b", "c"]);
}

#[test]
fn enforce_pinned_partition_idempotent() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p").pinned());
    pc.add(Spy::new("r"));
    let order = names_of(&pc);
    for _ in 0..5 {
        pc.enforce_pinned_partition();
    }
    assert_eq!(names_of(&pc), order);
}

// ─── move_tab clamping ──────────────────────────────────────────────────────

#[test]
fn move_tab_within_regular_group() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("r2"));
    pc.add(Spy::new("r3"));
    assert!(pc.move_tab(0, 2));
    assert_eq!(names_of(&pc), vec!["r2", "r3", "r1"]);
}

#[test]
fn move_tab_clamps_regular_into_pinned_zone() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("p2").pinned());
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("r2"));
    // Try to move r2 (idx 3) to position 0 (deep into pinned zone). Should be
    // clamped to the regular start (idx 2).
    assert!(pc.move_tab(3, 0));
    assert_eq!(names_of(&pc), vec!["p1", "p2", "r2", "r1"]);
}

#[test]
fn move_tab_clamps_pinned_into_regular_zone() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("p2").pinned());
    pc.add(Spy::new("r1"));
    // Try to move p1 (idx 0) to position 2 (regular zone). Should be clamped
    // to the last pinned slot (idx 1).
    assert!(pc.move_tab(0, 2));
    assert_eq!(names_of(&pc), vec!["p2", "p1", "r1"]);
}

#[test]
fn move_tab_returns_false_for_invalid_indices() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    assert!(!pc.move_tab(5, 0));
    assert!(!pc.move_tab(0, 5));
    assert!(!pc.move_tab(0, 0));
}

#[test]
fn move_tab_single_pinned_cannot_escape() {
    // Regression: a lone pinned tab at idx 0 has `pinned_count == 1`, so the
    // clamp target `to.min(pinned_count - 1) == 0 == from` → no move, returns
    // false (rather than panicking on `saturating_sub`).
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("r1"));
    pc.add(Spy::new("r2"));
    assert!(!pc.move_tab(0, 2), "lone pinned tab has nowhere to move");
    assert_eq!(names_of(&pc), vec!["p1", "r1", "r2"]);
}
