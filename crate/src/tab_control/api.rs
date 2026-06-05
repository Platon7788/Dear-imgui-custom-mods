//! Tab-management API for [`TabControl`](super::TabControl).
//!
//! The mutation surface — add / remove / clear / reorder / activate / iterate.
//! All deterministic (no ImGui FFI), which is what the unit-test suite
//! exercises. The struct definition and rendering entry point live in
//! [`super`]; the per-frame renderer lives in [`super::render`].

use super::layout;
use super::types::TabId;
use super::{TabControl, TabEntry, TabItem};

impl<T: TabItem> TabControl<T> {
    // ── Tab management ──────────────────────────────────────────────────

    /// Add a tab and return its [`TabId`]. The new tab becomes active.
    ///
    /// Insertion respects the pinned/regular invariant: pinned tabs are
    /// inserted right after the last pinned (i.e. at the end of the pinned
    /// section), regular tabs are pushed to the end. This way pinned tabs
    /// always occupy a contiguous prefix of the internal list.
    ///
    /// Safe to call before any ImGui context exists (e.g. from
    /// `Default::default()`) — the auto-scroll-to-active is deferred to the
    /// next render so no text measurement happens here.
    pub fn add(&mut self, mut item: T) -> TabId {
        let id = self.next_id;
        self.next_id += 1;

        if let Some(old_id) = self.active
            && let Some(old) = self.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        item.on_activated();

        let open_anim = if self.config.animate_open { 0.0 } else { 1.0 };
        let entry = TabEntry {
            id,
            item,
            open: true,
            request_focus: false,
            open_anim,
        };

        // Maintain "pinned tabs occupy a contiguous prefix" invariant.
        if entry.item.is_pinned() {
            // Insert right after the last existing pinned tab.
            let insert_at = self
                .tabs
                .iter()
                .position(|t| !t.item.is_pinned())
                .unwrap_or(self.tabs.len());
            self.tabs.insert(insert_at, entry);
        } else {
            self.tabs.push(entry);
        }
        self.active = Some(id);
        self.invalidate_tab_layout_cache();
        self.pending_scroll_to_active = true;
        id
    }

    /// Remove a tab by ID, returning the item if found. The next-most-recent
    /// tab becomes active and receives `on_activated()`.
    pub fn remove(&mut self, id: TabId) -> Option<T> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let entry = self.tabs.remove(idx);
        if self.active == Some(id) {
            self.active = self.tabs.last().map(|t| t.id);
            if let Some(new_id) = self.active
                && let Some(new_entry) = self.tabs.iter_mut().find(|t| t.id == new_id)
            {
                new_entry.item.on_activated();
            }
        }
        // Clear any in-flight close-confirmation / close-animation
        // that targeted the removed id — otherwise the popup would
        // re-open with a stale "Unknown" name and the close
        // animation would tick down against a dead entry. M1+M2
        // from session 034 audit.
        if self.pending_close == Some(id) {
            self.pending_close = None;
            self.pending_close_new = false;
        }
        if let Some((closing_id, _)) = self.closing_tab
            && closing_id == id
        {
            self.closing_tab = None;
        }
        if self.context_tab == Some(id) {
            self.context_tab = None;
            self.open_context_menu = false;
        }
        self.invalidate_tab_layout_cache();
        Some(entry.item)
    }

    /// Remove all tabs. Calls `on_deactivated()` on the active tab if any.
    pub fn clear(&mut self) {
        if let Some(active_id) = self.active
            && let Some(entry) = self.tabs.iter_mut().find(|t| t.id == active_id)
        {
            entry.item.on_deactivated();
        }
        self.tabs.clear();
        self.active = None;
        self.pending_close = None;
        self.pending_close_new = false;
        self.closing_tab = None;
        self.context_tab = None;
        self.open_context_menu = false;
        self.scroll_offset = 0.0;
        self.scroll_target = 0.0;
        self.invalidate_tab_layout_cache();
    }

    /// Move a tab from index `from` to index `to`.
    ///
    /// Cross-group moves (pinned ↔ regular) are clamped to the source group:
    /// a pinned tab cannot escape the pinned prefix, and a regular tab
    /// cannot enter it. This preserves the pinned/regular invariant.
    /// Returns `true` if a move actually happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return false;
        }
        let pinned_count = self.tabs.iter().take_while(|t| t.item.is_pinned()).count();
        let from_is_pinned = from < pinned_count;
        // Clamp `to` into the source's group.
        let clamped_to = if from_is_pinned {
            to.min(pinned_count.saturating_sub(1))
        } else {
            to.max(pinned_count)
        };
        if clamped_to == from {
            return false;
        }
        let entry = self.tabs.remove(from);
        self.tabs.insert(clamped_to, entry);
        self.invalidate_tab_layout_cache();
        true
    }

    /// Re-establish the pinned/regular partition. Delegates to
    /// [`layout::enforce_pinned_partition`].
    pub(crate) fn enforce_pinned_partition(&mut self) {
        layout::enforce_pinned_partition(self);
    }

    /// Borrow a tab's item.
    pub fn get(&self, id: TabId) -> Option<&T> {
        self.tabs.iter().find(|t| t.id == id).map(|t| &t.item)
    }

    /// Mutably borrow a tab's item.
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut T> {
        self.tabs
            .iter_mut()
            .find(|t| t.id == id)
            .map(|t| &mut t.item)
    }

    /// Currently active tab ID (if any).
    #[must_use]
    pub fn active_id(&self) -> Option<TabId> {
        self.active
    }

    /// Programmatically activate a tab. Calls `on_deactivated()` on the
    /// previously active tab and `on_activated()` on the new one. The
    /// scroll-into-view side effect is deferred to the next render.
    ///
    /// **Idempotent contract**: calling `set_active(id)` on the
    /// already-active tab fires `on_activated()` again (no
    /// matching `on_deactivated()`), pinned by
    /// [`tests::set_active_same_id_does_not_re_fire_hooks`]
    /// observing the legacy behaviour. Hosts that count `on_activated`
    /// invocations (e.g. "open connection" semantics) should treat
    /// the same-id case as a no-op themselves before calling.
    pub fn set_active(&mut self, id: TabId) {
        if !self.tabs.iter().any(|t| t.id == id) {
            return;
        }
        if let Some(old_id) = self.active
            && old_id != id
            && let Some(old) = self.tabs.iter_mut().find(|t| t.id == old_id)
        {
            old.item.on_deactivated();
        }
        self.active = Some(id);
        if let Some(entry) = self.tabs.iter_mut().find(|t| t.id == id) {
            entry.item.on_activated();
        }
        self.pending_scroll_to_active = true;
    }

    /// Number of tabs currently in the control.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Whether there are zero tabs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Iterate `(TabId, &T)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (TabId, &T)> {
        self.tabs.iter().map(|t| (t.id, &t.item))
    }

    /// Iterate `(TabId, &mut T)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TabId, &mut T)> {
        self.tabs.iter_mut().map(|t| (t.id, &mut t.item))
    }

    /// Force the tab-width cache to be recomputed next frame. Call this if a
    /// tab's title, icon, or badge changes dynamically — the controller
    /// can't otherwise detect trait method return value changes.
    pub fn force_invalidate(&mut self) {
        self.invalidate_tab_layout_cache();
    }

    /// Request that `scroll_target` be adjusted so the active tab is visible
    /// on the next render. The actual computation runs inside `render()` so
    /// it never invokes ImGui text measurement before the context is ready.
    pub fn scroll_to_active(&mut self) {
        self.pending_scroll_to_active = true;
    }
}
