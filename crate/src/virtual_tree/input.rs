//! Selection, keyboard navigation, and clipboard copy.
//!
//! Part of [`VirtualTree`](super::VirtualTree); split out of `mod.rs`
//! to keep files under 500 lines. See `mod.rs` for the struct.

use super::*;

impl<T: VirtualTreeNode> VirtualTree<T> {
    // ─── Internal: selection ────────────────────────────────────────

    pub(super) fn handle_selection(&mut self, ui: &Ui, flat_idx: usize) {
        let node_id = match self.flat_view.rows.get(flat_idx) {
            Some(r) => r.node_id,
            None => return,
        };

        match self.config.table.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_nodes.clear();
                self.selected_nodes.insert(node_id);
                self.anchor = Some(node_id);
            }
            SelectionMode::Multi => {
                let io = ui.io();
                let ctrl = io.key_ctrl();
                let shift = io.key_shift();

                if ctrl {
                    if !self.selected_nodes.remove(&node_id) {
                        self.selected_nodes.insert(node_id);
                    }
                    self.anchor = Some(node_id);
                } else if shift {
                    // Resolve the anchor node to its current flat-view row.
                    // If the anchor has scrolled out of the flat view (e.g. its
                    // branch was collapsed), fall back to a single-row select.
                    let anchor_idx = self
                        .anchor
                        .and_then(|n| self.flat_view.index_of(n))
                        .unwrap_or(flat_idx);
                    let (start, end) = if flat_idx < anchor_idx {
                        (flat_idx, anchor_idx)
                    } else {
                        (anchor_idx, flat_idx)
                    };
                    self.selected_nodes.clear();
                    for i in start..=end {
                        if let Some(r) = self.flat_view.rows.get(i) {
                            self.selected_nodes.insert(r.node_id);
                        }
                    }
                    // Shift extends the range; the anchor itself is unchanged.
                } else {
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node_id);
                    self.anchor = Some(node_id);
                }
            }
        }
    }

    // ─── Internal: keyboard ─────────────────────────────────────────

    pub(super) fn handle_keyboard(&mut self, ui: &Ui) {
        if !ui.is_window_focused() {
            return;
        }

        // Resolve the stable anchor node to its current flat-view row once.
        // `None` means "no live anchor" (never selected, or anchor scrolled
        // out of the flat view) → arrow keys seed selection at row 0.
        let anchor_idx = self.anchor.and_then(|n| self.flat_view.index_of(n));

        if ui.is_key_pressed(Key::DownArrow) {
            if let Some(anchor) = anchor_idx {
                let next = (anchor + 1).min(self.flat_view.len().saturating_sub(1));
                self.select_flat_row(next);
            } else if !self.flat_view.rows.is_empty() {
                self.select_flat_row(0);
            }
        }

        if ui.is_key_pressed(Key::UpArrow) {
            if let Some(anchor) = anchor_idx {
                let prev = anchor.saturating_sub(1);
                self.select_flat_row(prev);
            } else if !self.flat_view.rows.is_empty() {
                self.select_flat_row(0);
            }
        }

        if ui.is_key_pressed(Key::RightArrow)
            && let Some(anchor) = anchor_idx
            && let Some(row) = self.flat_view.rows.get(anchor)
        {
            let node_id = row.node_id;
            if !row.is_leaf && !row.is_expanded {
                self.pending_toggle = Some(node_id);
            } else if row.is_expanded && anchor + 1 < self.flat_view.len() {
                self.select_flat_row(anchor + 1);
            }
        }

        if ui.is_key_pressed(Key::LeftArrow)
            && let Some(anchor) = anchor_idx
            && let Some(row) = self.flat_view.rows.get(anchor)
        {
            let node_id = row.node_id;
            if !row.is_leaf && row.is_expanded {
                self.pending_toggle = Some(node_id);
            } else if let Some(parent_id) = self.arena.parent(node_id)
                && let Some(parent_flat) = self.flat_view.index_of(parent_id)
            {
                self.select_flat_row(parent_flat);
            }
        }

        // Delete
        if ui.is_key_pressed(Key::Delete) && !self.selected_nodes.is_empty() {
            // Collect to avoid borrow conflict
            let to_remove: Vec<NodeId> = self.selected_nodes.iter().copied().collect();
            for id in to_remove {
                self.arena.remove(id);
            }
            self.selected_nodes.clear();
            self.anchor = None;
            self.edit_state.deactivate();
            self.flat_view.mark_dirty();
        }

        // Ctrl+A
        if ui.io().key_ctrl() && ui.is_key_pressed(Key::A) {
            self.selected_nodes.clear();
            for row in &self.flat_view.rows {
                self.selected_nodes.insert(row.node_id);
            }
        }

        // F2
        if ui.is_key_pressed(Key::F2)
            && self.config.table.edit_trigger == EditTrigger::F2Key
            && let Some(anchor) = anchor_idx
        {
            for c in 0..self.columns.len() {
                if !matches!(
                    editor_kind(&self.columns[c].editor),
                    EditorKind::None
                        | EditorKind::Checkbox
                        | EditorKind::ComboBox
                        | EditorKind::Button
                        | EditorKind::ProgressBar
                        | EditorKind::ColorEdit
                        | EditorKind::Custom
                ) {
                    self.try_activate_edit(anchor, c);
                    break;
                }
            }
        }
    }

    pub(super) fn select_flat_row(&mut self, flat_idx: usize) {
        if let Some(row) = self.flat_view.rows.get(flat_idx) {
            let node_id = row.node_id;
            self.selected_nodes.clear();
            self.selected_nodes.insert(node_id);
            self.anchor = Some(node_id);
        }
    }

    /// Build tab-separated text from selected nodes for clipboard copy.
    ///
    /// **Copy rules:**
    /// - If a parent node is selected, its entire subtree is copied (parent + all
    ///   children) with depth indentation — **even when the parent is collapsed**.
    /// - If only leaf/child nodes are selected, only those rows are copied.
    /// - Mixed selection: each selected subtree-root pulls in its descendants;
    ///   a selected node whose ancestor is also selected is not emitted twice.
    ///
    /// Emission order: selection-roots visible in the flat view come first in
    /// display order; collapsed-away selection-roots follow in arena order.
    pub(super) fn build_copy_text(&self) -> String {
        let col_count = self.columns.len();
        let mut out = String::new();
        let mut cell_buf = String::new();

        // A "selection-root" is a selected node with no selected ancestor — it
        // owns the subtree to emit. Roots are mutually disjoint, so each subtree
        // is emitted exactly once with no need for a visited set.
        let mut roots: Vec<NodeId> = self
            .selected_nodes
            .iter()
            .copied()
            .filter(|&nid| !self.is_ancestor_selected(nid, &self.selected_nodes))
            .collect();

        // Deterministic order: by flat-view row when visible (collapsed roots,
        // index_of == None → sorted last), tie-broken by arena slot index.
        roots.sort_by_key(|&nid| {
            (
                self.flat_view.index_of(nid).unwrap_or(usize::MAX),
                nid.index,
            )
        });

        for nid in roots {
            // Start indentation at the node's real depth so a copied subtree
            // keeps its hierarchy regardless of expand state.
            let depth = self.arena.depth(nid).unwrap_or(0) as usize;
            self.copy_subtree(nid, depth, col_count, &mut out, &mut cell_buf);
        }

        out
    }

    /// Copy a subtree (iterative DFS — safe at any depth).
    pub(super) fn copy_subtree(
        &self,
        nid: NodeId,
        depth: usize,
        col_count: usize,
        out: &mut String,
        cell_buf: &mut String,
    ) {
        let mut stack: Vec<(NodeId, usize)> = vec![(nid, depth)];
        while let Some((current, d)) = stack.pop() {
            let Some(slot) = self.arena.get(current) else {
                continue;
            };

            for _ in 0..d {
                out.push_str("  ");
            }
            for ci in 0..col_count {
                if ci > 0 {
                    out.push('\t');
                }
                cell_buf.clear();
                slot.data.cell_display_text(ci, cell_buf);
                out.push_str(cell_buf);
            }
            out.push('\n');

            // Push children in reverse so first child is processed first.
            for &child_id in slot.children.iter().rev() {
                stack.push((child_id, d + 1));
            }
        }
    }
}
