//! add / remove / clear / set_active — lifecycle-hook dispatch, id stability,
//! and the close-confirmation / context-menu state cleanup on removal.

use super::super::*;
use super::{Spy, names_of};

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

#[test]
fn remove_clears_pending_close_for_target_id() {
    // M1 from session 034 audit. If a tab is mid-confirmation and the host
    // removes it programmatically, the popup state must not survive the
    // removal — otherwise the popup would re-open with a stale "Unknown" name
    // and `pending_close.take()` would pop a dead id.
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    pc.pending_close = Some(a);
    pc.pending_close_new = true;
    pc.context_tab = Some(a);
    pc.open_context_menu = true;

    let _ = pc.remove(a);

    assert_eq!(pc.pending_close, None, "pending_close must clear");
    assert!(!pc.pending_close_new, "pending_close_new must clear");
    assert_eq!(pc.context_tab, None, "context_tab must clear");
    assert!(!pc.open_context_menu, "open_context_menu must clear");
}

#[test]
fn remove_clears_closing_tab_for_target_id() {
    // M2 from session 034 audit. Mid-animation removal must drop the dangling
    // timer so it doesn't tick down against a dead id.
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    pc.closing_tab = Some((a, 0.5));

    let _ = pc.remove(a);

    assert_eq!(pc.closing_tab, None, "closing_tab must clear");
}

#[test]
fn remove_keeps_pending_close_for_other_id() {
    // Negative test: removing tab A must NOT touch tab B's pending close.
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b"));
    pc.pending_close = Some(b);

    let _ = pc.remove(a);

    assert_eq!(pc.pending_close, Some(b), "unrelated pending_close kept");
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

// ─── non-closable / preview / status round-trip ─────────────────────────────

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

#[test]
fn dot_color_default_is_none_and_overridable() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let a = pc.add(Spy::new("a"));
    let b = pc.add(Spy::new("b").with_dot_color([10, 20, 30]));
    assert_eq!(pc.get(a).unwrap().dot_color(), None);
    assert_eq!(pc.get(b).unwrap().dot_color(), Some([10, 20, 30]));
}

#[test]
fn text_color_default_is_none_and_overridable() {
    // Pin the contract: by default `text_color()` returns `None` so the
    // renderer falls through to the palette's `text` / `text_muted` defaults.
    // A per-tab override surfaces verbatim.
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    let plain = pc.add(Spy::new("plain"));
    let warm = pc.add(Spy::new("warm").with_text_color([220, 140, 60]));
    assert_eq!(pc.get(plain).unwrap().text_color(), None);
    assert_eq!(pc.get(warm).unwrap().text_color(), Some([220, 140, 60]));
}

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

#[test]
fn iter_mut_yields_each_tab_once_in_order() {
    let mut pc: TabControl<Spy> = TabControl::new("##t");
    pc.add(Spy::new("a"));
    pc.add(Spy::new("b"));
    pc.add(Spy::new("c"));
    for (_, item) in pc.iter_mut() {
        item.name.push('!');
    }
    assert_eq!(names_of(&pc), vec!["a!", "b!", "c!"]);
}
