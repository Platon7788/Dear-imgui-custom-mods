//! Unit tests for `TabControl` logic that does NOT require an ImGui context.
//!
//! Anything touching `calc_text_size` (i.e. `ensure_tab_widths` /
//! `compute_tab_width`) is intentionally NOT exercised here — those would
//! crash when run outside an initialized ImGui environment. We focus on the
//! deterministic state-machine bits: add/remove/clear, pinned partitioning,
//! move_tab clamping, set_active dispatch, request_focus tracking, scroll math,
//! config round-trips, and i18n.
//!
//! Split into themed sub-modules:
//! - [`lifecycle`] — add / remove / clear / set_active hooks + ids.
//! - [`pinned`] — pinned-prefix invariant, `enforce_pinned_partition`,
//!   `move_tab` clamping.
//! - [`config`] — config / palette defaults, popup-id scoping, pinned
//!   constants.
//! - [`scroll`] — `scroll_into_view` regular-vs-pinned offset math
//!   (regression coverage for the pinned over-count bug).
//! - [`i18n`] — locale guard tests (resolution, default, ron round-trip,
//!   optional field).

use super::*;
use dear_imgui_rs::Ui;

mod config;
mod i18n;
mod lifecycle;
mod pinned;
mod scroll;

/// Minimal `TabItem` that records lifecycle calls so tests can assert
/// `on_activated` / `on_deactivated` semantics.
#[derive(Default, Debug, Clone)]
pub(super) struct Spy {
    pub(super) name: String,
    pub(super) pinned: bool,
    pub(super) closable: bool,
    pub(super) preview: bool,
    pub(super) status: TabStatus,
    pub(super) dot_color: Option<[u8; 3]>,
    pub(super) text_color: Option<[u8; 3]>,
    pub(super) activated: u32,
    pub(super) deactivated: u32,
}

impl Spy {
    pub(super) fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            pinned: false,
            closable: true,
            preview: true,
            status: TabStatus::Active,
            dot_color: None,
            text_color: None,
            activated: 0,
            deactivated: 0,
        }
    }
    pub(super) fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
    pub(super) fn non_closable(mut self) -> Self {
        self.closable = false;
        self
    }
    pub(super) fn no_preview(mut self) -> Self {
        self.preview = false;
        self
    }
    pub(super) fn with_status(mut self, s: TabStatus) -> Self {
        self.status = s;
        self
    }
    pub(super) fn with_dot_color(mut self, c: [u8; 3]) -> Self {
        self.dot_color = Some(c);
        self
    }
    pub(super) fn with_text_color(mut self, c: [u8; 3]) -> Self {
        self.text_color = Some(c);
        self
    }
}

impl TabItem for Spy {
    fn title(&self) -> &str {
        &self.name
    }
    fn is_pinned(&self) -> bool {
        self.pinned
    }
    fn is_closable(&self) -> bool {
        self.closable
    }
    fn show_preview(&self) -> bool {
        self.preview
    }
    fn status(&self) -> TabStatus {
        self.status
    }
    fn dot_color(&self) -> Option<[u8; 3]> {
        self.dot_color
    }
    fn text_color(&self) -> Option<[u8; 3]> {
        self.text_color
    }
    fn on_activated(&mut self) {
        self.activated += 1;
    }
    fn on_deactivated(&mut self) {
        self.deactivated += 1;
    }
    // never invoked in these tests, but the trait requires it
    fn render_content(&mut self, _ui: &Ui) {}
}

/// Collect the names from `pc.iter()` in order — handy for asserting layout.
pub(super) fn names_of(pc: &TabControl<Spy>) -> Vec<String> {
    pc.iter().map(|(_, s)| s.name.clone()).collect()
}

/// Push a tab directly into `pc.tabs`, bypassing `add()`'s smart insertion.
/// Used by tests that deliberately construct an un-partitioned / hand-laid-out
/// list to exercise repair / scroll math.
pub(super) fn push_raw(pc: &mut TabControl<Spy>, id: TabId, item: Spy) {
    pc.tabs.push(TabEntry {
        id,
        item,
        open: true,
        request_focus: false,
        open_anim: 1.0,
    });
}
