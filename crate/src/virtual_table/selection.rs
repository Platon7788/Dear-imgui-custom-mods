//! Selection gestures and identity-preserving external-data remapping.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    /// Last active row, distinct from the anchor of a Shift range.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row
    }

    /// Select every row in the rendered data source without scrolling.
    /// External hosts supply their current length; internal hosts use `len()`.
    pub fn select_all(&mut self, row_count: usize) {
        if self.config.selection_mode != SelectionMode::Multi {
            return;
        }
        self.selected_rows = (0..row_count).collect();
        self.selection_anchor = (row_count > 0).then_some(0);
        self.focused_row = self
            .focused_row
            .filter(|i| *i < row_count)
            .or(self.selection_anchor);
    }

    /// Remap row indices after an external data source changes order or evicts rows.
    /// `map` must resolve an old row to the new index of the same item, or `None`
    /// when it disappeared. Selection, range anchor, focus, context and pending
    /// scroll follow the item. Remapping itself never requests a scroll.
    pub fn remap_rows(&mut self, mut map: impl FnMut(usize) -> Option<usize>) {
        self.selection_scratch.clear();
        self.selection_scratch
            .extend(self.selected_rows.iter().filter_map(|i| map(*i)));
        std::mem::swap(&mut self.selected_rows, &mut self.selection_scratch);
        self.selection_anchor = self.selection_anchor.and_then(&mut map);
        self.focused_row = self.focused_row.and_then(&mut map);
        self.context_row = self.context_row.and_then(&mut map);
        self.pending_scroll_to = self.pending_scroll_to.and_then(&mut map);
        self.edit_state.deactivate();
    }

    pub(super) fn select_context_row(&mut self, idx: usize, row_count: usize) {
        if !self.is_selected(idx) {
            self.apply_selection(idx, row_count, false, false);
        }
        self.focused_row = Some(idx);
        self.context_row = Some(idx);
    }

    pub(super) fn pause_for_multi_selection(&mut self) {
        if self.config.pause_scroll_on_multi_select && self.selected_count() > 1 {
            self.config.auto_scroll = false;
        }
    }

    pub(super) fn apply_selection(
        &mut self,
        idx: usize,
        row_count: usize,
        ctrl: bool,
        shift: bool,
    ) {
        if idx >= row_count || self.config.selection_mode == SelectionMode::None {
            return;
        }
        if self.config.selection_mode == SelectionMode::Multi && shift {
            let anchor = self.selection_anchor.unwrap_or(idx).min(row_count - 1);
            if !ctrl {
                self.selected_rows.clear();
            }
            self.selected_rows.extend(anchor.min(idx)..=anchor.max(idx));
            self.selection_anchor = Some(anchor);
        } else if self.config.selection_mode == SelectionMode::Multi && ctrl {
            if !self.selected_rows.remove(&idx) {
                self.selected_rows.insert(idx);
            }
            self.selection_anchor = Some(idx);
        } else {
            self.selected_rows.clear();
            self.selected_rows.insert(idx);
            self.selection_anchor = Some(idx);
        }
        self.focused_row = Some(idx);
        self.pause_for_multi_selection();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Row;
    impl VirtualTableRow for Row {
        fn cell_value(&self, _: usize) -> CellValue {
            CellValue::Text(String::new())
        }
        fn set_cell_value(&mut self, _: usize, _: &CellValue) {}
    }
    fn table() -> VirtualTable<Row> {
        VirtualTable::new(
            "selection",
            vec![],
            1,
            TableConfig {
                selection_mode: SelectionMode::Multi,
                auto_scroll: true,
                pause_scroll_on_multi_select: true,
                ..Default::default()
            },
        )
    }
    #[test]
    fn range_focus_and_anchor_are_distinct_and_ctrl_shift_adds() {
        let mut t = table();
        t.apply_selection(2, 10, false, false);
        assert!(t.config.auto_scroll);
        t.apply_selection(5, 10, false, true);
        assert_eq!(t.selected_count(), 4);
        assert_eq!(t.selected_row(), Some(2));
        assert_eq!(t.focused_row(), Some(5));
        assert!(!t.config.auto_scroll);
        t.config.auto_scroll = true;
        t.apply_selection(8, 10, true, false);
        t.apply_selection(9, 10, true, true);
        assert_eq!(t.selected_count(), 6);
        t.apply_selection(8, 10, true, false);
        assert!(!t.is_selected(8));
    }
    #[test]
    fn remapping_preserves_packet_identity_and_drops_only_evicted_rows() {
        let mut t = table();
        t.apply_selection(1, 4, false, false);
        t.apply_selection(3, 4, true, false);
        t.context_row = Some(3);
        t.scroll_to_row(3);
        t.remap_rows(|i| match i {
            1 => Some(2),
            3 => Some(4),
            _ => None,
        });
        assert!(t.is_selected(2) && t.is_selected(4));
        assert_eq!(t.focused_row(), Some(4));
        assert_eq!(t.selection_anchor, Some(4));
        assert_eq!(t.context_row, Some(4));
        assert_eq!(t.pending_scroll_to, Some(4));
        t.remap_rows(|i| (i != 4).then_some(i));
        assert_eq!(t.selected_count(), 1);
        assert_eq!(t.focused_row(), None);
        assert_eq!(t.context_row, None);
    }

    #[test]
    fn right_click_preserves_range_and_selects_outside_row_without_modifiers() {
        let mut t = table();
        t.apply_selection(2, 10, false, false);
        t.apply_selection(5, 10, false, true);
        t.select_context_row(4, 10);
        assert_eq!(t.selected_count(), 4);
        assert_eq!(t.focused_row(), Some(4));
        assert_eq!(t.selection_anchor, Some(2));
        t.select_context_row(8, 10);
        assert_eq!(t.selected_count(), 1);
        assert!(t.is_selected(8));
    }
    #[test]
    fn old_config_without_pause_policy_still_loads() {
        let ron =
            include_str!("config.ron").replace("    pause_scroll_on_multi_select: false,", "");
        let cfg: TableConfig = ron::from_str(&ron).unwrap();
        assert!(!cfg.pause_scroll_on_multi_select);
    }
}
