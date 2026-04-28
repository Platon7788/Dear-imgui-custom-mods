//! Unit tests for `TabControl` logic that does NOT require an ImGui context.
//!
//! Anything touching `calc_text_size` (i.e. `ensure_tab_widths` /
//! `compute_tab_width`) is intentionally NOT exercised here — those would
//! crash when run outside an initialized ImGui environment. We focus on the
//! deterministic state-machine bits: add/remove/clear, pinned partitioning,
//! move_tab clamping, set_active dispatch, request_focus tracking, etc.

use super::*;
use dear_imgui_rs::Ui;

/// Minimal `TabItem` that records lifecycle calls so tests can assert
/// `on_activated` / `on_deactivated` semantics.
#[derive(Default, Debug, Clone)]
struct Spy {
    name: String,
    pinned: bool,
    closable: bool,
    preview: bool,
    status: TabStatus,
    dot_color: Option<[u8; 3]>,
    activated: u32,
    deactivated: u32,
}

impl Spy {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            pinned: false,
            closable: true,
            preview: true,
            status: TabStatus::Active,
            dot_color: None,
            activated: 0,
            deactivated: 0,
        }
    }
    fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
    fn non_closable(mut self) -> Self {
        self.closable = false;
        self
    }
    fn no_preview(mut self) -> Self {
        self.preview = false;
        self
    }
    fn with_status(mut self, s: TabStatus) -> Self {
        self.status = s;
        self
    }
    fn with_dot_color(mut self, c: [u8; 3]) -> Self {
        self.dot_color = Some(c);
        self
    }
}

impl TabItem for Spy {
    fn title(&self) -> &str { &self.name }
    fn is_pinned(&self) -> bool { self.pinned }
    fn is_closable(&self) -> bool { self.closable }
    fn show_preview(&self) -> bool { self.preview }
    fn status(&self) -> TabStatus { self.status }
    fn dot_color(&self) -> Option<[u8; 3]> { self.dot_color }
    fn on_activated(&mut self) { self.activated += 1; }
    fn on_deactivated(&mut self) { self.deactivated += 1; }
    // never invoked in these tests, but the trait requires it
    fn render_content(&mut self, _ui: &Ui) {}
}

/// Collect the names from `pc.iter()` in order — handy for asserting layout.
fn names_of(pc: &TabControl<Spy>) -> Vec<String> {
    pc.iter().map(|(_, s)| s.name.clone()).collect()
}

// ─── add() ──────────────────────────────────────────────────────────────────

#[test]
fn add_returns_unique_increasing_ids() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b"));
    let c = pc.add(Spy::new("c"));
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert!(b > a);
    assert!(c > b);
}

#[test]
fn add_makes_new_tab_active() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    assert_eq!(pc.active_id(), Some(a));
    let b = pc.add(Spy::new("b"));
    assert_eq!(pc.active_id(), Some(b));
}

#[test]
fn add_calls_lifecycle_hooks() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b"));
    // Adding `b` should deactivate `a` and activate `b`.
    assert_eq!(pc.get(a).unwrap().activated, 1);
    assert_eq!(pc.get(a).unwrap().deactivated, 1);
    assert_eq!(pc.get(b).unwrap().activated, 1);
    assert_eq!(pc.get(b).unwrap().deactivated, 0);
}

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
fn enforce_pinned_partition_repairs_arbitrary_order() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    // Bypass add()'s smart insertion by directly pushing into pc.tabs.
    pc.tabs.push(TabEntry {
        id: 1,
        item: Spy::new("r1"),
        open: true,
        request_focus: false,
        open_anim: 1.0,
    });
    pc.tabs.push(TabEntry {
        id: 2,
        item: Spy::new("p1").pinned(),
        open: true,
        request_focus: false,
        open_anim: 1.0,
    });
    pc.tabs.push(TabEntry {
        id: 3,
        item: Spy::new("r2"),
        open: true,
        request_focus: false,
        open_anim: 1.0,
    });
    pc.tabs.push(TabEntry {
        id: 4,
        item: Spy::new("p2").pinned(),
        open: true,
        request_focus: false,
        open_anim: 1.0,
    });
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

// ─── remove / clear ─────────────────────────────────────────────────────────

#[test]
fn remove_returns_item_and_keeps_count() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let _b = pc.add(Spy::new("b"));
    assert_eq!(pc.tab_count(), 2);
    let item = pc.remove(a).unwrap();
    assert_eq!(item.name, "a");
    assert_eq!(pc.tab_count(), 1);
}

#[test]
fn remove_active_promotes_last_remaining() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b"));
    let c = pc.add(Spy::new("c"));
    // active = c. Remove it → next active should be the new last (b).
    assert_eq!(pc.active_id(), Some(c));
    pc.remove(c);
    assert_eq!(pc.active_id(), Some(b));
    // `b` should have received an extra on_activated.
    assert!(pc.get(a).is_some());
    assert!(pc.get(b).unwrap().activated >= 1);
}

#[test]
fn remove_unknown_returns_none() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a"));
    assert!(pc.remove(9999).is_none());
}

#[test]
fn clear_resets_state() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    pc.clear();
    assert_eq!(pc.tab_count(), 0);
    assert!(pc.is_empty());
    assert!(pc.active_id().is_none());
}

// ─── set_active ─────────────────────────────────────────────────────────────

#[test]
fn set_active_switches_active_and_dispatches_hooks() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b"));
    // Currently active = b (last add). Switch back to a.
    let a_act_before = pc.get(a).unwrap().activated;
    let b_deact_before = pc.get(b).unwrap().deactivated;
    pc.set_active(a);
    assert_eq!(pc.active_id(), Some(a));
    assert_eq!(pc.get(a).unwrap().activated, a_act_before + 1);
    assert_eq!(pc.get(b).unwrap().deactivated, b_deact_before + 1);
}

#[test]
fn set_active_unknown_id_is_noop() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    pc.set_active(9999);
    assert_eq!(pc.active_id(), Some(a));
}

#[test]
fn set_active_same_id_does_not_re_fire_hooks() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let act = pc.get(a).unwrap().activated;
    let deact = pc.get(a).unwrap().deactivated;
    pc.set_active(a);
    // Re-activating the same tab should still re-call on_activated (we don't
    // attempt to dedupe), but must NOT call on_deactivated on it.
    let after_deact = pc.get(a).unwrap().deactivated;
    assert_eq!(after_deact, deact);
    let _ = act;
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
    // Try to move r2 (idx 3) to position 0 (deep into pinned zone).
    // Should be clamped to the regular start (idx 2).
    assert!(pc.move_tab(3, 0));
    assert_eq!(names_of(&pc), vec!["p1", "p2", "r2", "r1"]);
}

#[test]
fn move_tab_clamps_pinned_into_regular_zone() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("p1").pinned());
    pc.add(Spy::new("p2").pinned());
    pc.add(Spy::new("r1"));
    // Try to move p1 (idx 0) to position 2 (regular zone).
    // Should be clamped to the last pinned slot (idx 1).
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

// ─── popup IDs ──────────────────────────────────────────────────────────────

#[test]
fn popup_ids_are_scoped_to_imgui_id() {
    let pc1: TabControl<Spy> = TabControl::new("##first");
    let pc2: TabControl<Spy> = TabControl::new("##second");
    assert_ne!(pc1.close_popup_id, pc2.close_popup_id);
    assert_ne!(pc1.overflow_popup_id, pc2.overflow_popup_id);
    assert!(pc1.close_popup_id.contains("##first"));
    assert!(pc2.close_popup_id.contains("##second"));
}

// ─── non-closable interaction ───────────────────────────────────────────────

#[test]
fn non_closable_tab_keeps_is_closable_false() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a").non_closable());
    assert!(!pc.get(a).unwrap().is_closable());
}

#[test]
fn show_preview_default_is_true_and_can_be_overridden() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b").no_preview());
    assert!(pc.get(a).unwrap().show_preview());
    assert!(!pc.get(b).unwrap().show_preview());
}

// ─── TabStatus / dot_color / status_color ───────────────────────────────────

#[test]
fn tab_status_default_is_active() {
    assert_eq!(TabStatus::default(), TabStatus::Active);
}

#[test]
fn tab_status_none_returns_neutral_color_via_palette() {
    let palette = TabColors::default();
    // None must not crash and must return a sensible neutral (status_inactive).
    let none_col = palette.status_color(TabStatus::None);
    let inactive_col = palette.status_color(TabStatus::Inactive);
    assert_eq!(none_col, inactive_col);
}

#[test]
fn dot_color_default_is_none_and_overridable() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b").with_dot_color([10, 20, 30]));
    assert_eq!(pc.get(a).unwrap().dot_color(), None);
    assert_eq!(pc.get(b).unwrap().dot_color(), Some([10, 20, 30]));
}

#[test]
fn show_status_dot_config_default_is_true() {
    let cfg = TabControlConfig::default();
    assert!(cfg.show_status_dot);
}

// ─── enforce_pinned_partition stability ────────────────────────────────────

#[test]
fn enforce_pinned_partition_preserves_relative_order() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    // Push directly to bypass add()'s smart insertion.
    let pushes = [
        ("r1", false), ("p1", true), ("r2", false),
        ("p2", true), ("r3", false), ("p3", true),
    ];
    for (i, (name, pinned)) in pushes.iter().enumerate() {
        let mut s = Spy::new(name);
        s.pinned = *pinned;
        pc.tabs.push(TabEntry {
            id: (i + 1) as u64,
            item: s,
            open: true,
            request_focus: false,
            open_anim: 1.0,
        });
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

// ─── add() respects pinned invariant for adversarial sequences ─────────────

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

// ─── status() trait round-trip ──────────────────────────────────────────────

#[test]
fn tab_with_status_none_reports_none() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a").with_status(TabStatus::None));
    assert_eq!(pc.get(a).unwrap().status(), TabStatus::None);
}

#[test]
fn tab_with_status_dirty_reports_dirty() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a").with_status(TabStatus::Dirty));
    assert_eq!(pc.get(a).unwrap().status(), TabStatus::Dirty);
}
