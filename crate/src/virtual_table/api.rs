//! Public data/column/selection/editing API and export/import.
//!
//! Split out of `mod.rs` to keep files under 500 lines; extends
//! [`VirtualTable`](super::VirtualTable) via an `impl` block.

use super::*;

impl<T: VirtualTableRow> VirtualTable<T> {
    /// Number of rows currently stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// `true` when the table holds no rows.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get a row by logical index (0 = oldest).
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    /// Get a mutable reference to a row by logical index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }

    /// Remove all rows and reset selection/editing state.
    pub fn clear(&mut self) {
        self.data.clear();
        self.selected_rows.clear();
        self.selection_anchor = None;
        self.edit_state.deactivate();
    }

    /// Remove the row at logical index. O(n). Returns the removed item.
    /// Automatically adjusts selection indices and deactivates any active editor.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.edit_state.deactivate();
        // Remove the deleted row and shift indices above it down by 1.
        // In-place: collect indices that need decrement, then rebuild.
        // This avoids allocating a second IndexSet.
        self.selected_rows.remove(&index);
        // Drain + reinsert reuses the HashSet's allocated capacity (no reallocation).
        let indices: Vec<usize> = self.selected_rows.drain().collect();
        for r in indices {
            self.selected_rows.insert(if r > index { r - 1 } else { r });
        }
        // Adjust anchor
        if let Some(a) = self.selection_anchor {
            if a == index {
                self.selection_anchor = None;
            } else if a > index {
                self.selection_anchor = Some(a - 1);
            }
        }
        self.data.remove(index)
    }

    /// Direct read access to the underlying row storage.
    pub fn data(&self) -> &RingBuffer<T> {
        &self.data
    }

    /// Direct mutable access to the underlying row storage.
    pub fn data_mut(&mut self) -> &mut RingBuffer<T> {
        &mut self.data
    }

    // ─── Column access ──────────────────────────────────────────────

    /// Current column definitions, in display order.
    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    /// Mutable access to the column definitions, in display order.
    pub fn columns_mut(&mut self) -> &mut [ColumnDef] {
        &mut self.columns
    }

    // ─── Selection ──────────────────────────────────────────────────

    /// Returns an iterator over selected row indices.
    pub fn selected_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.selected_rows.iter().copied()
    }

    /// Number of selected rows.
    pub fn selected_count(&self) -> usize {
        self.selected_rows.len()
    }

    /// Returns `true` if the given row index is selected.
    pub fn is_selected(&self, idx: usize) -> bool {
        self.selected_rows.contains(&idx)
    }

    /// Returns the anchor (last explicitly clicked) row, or any selected row.
    /// For `Single` mode, returns the one selected row.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection_anchor
            .filter(|a| self.selected_rows.contains(a))
            .or_else(|| self.selected_rows.iter().next().copied())
    }

    /// Deselect all rows and clear the selection anchor.
    pub fn clear_selection(&mut self) {
        self.selected_rows.clear();
        self.selection_anchor = None;
    }

    /// Programmatically select a single row (clears previous selection) and
    /// scroll to it on the next frame.
    pub fn select_row(&mut self, idx: usize) {
        self.selected_rows.clear();
        self.selected_rows.insert(idx);
        self.selection_anchor = Some(idx);
        self.pending_scroll_to = Some(idx);
    }

    /// Request scroll to the given row index on the next frame.
    pub fn scroll_to_row(&mut self, idx: usize) {
        self.pending_scroll_to = Some(idx);
    }

    // ─── Editing ────────────────────────────────────────────────────

    /// `true` when a cell editor is currently active.
    pub fn is_editing(&self) -> bool {
        self.edit_state.active
    }

    /// Deactivate the current cell editor without committing its value.
    pub fn cancel_edit(&mut self) {
        self.edit_state.deactivate();
    }

    // ─── Export / Import ────────────────────────────────────────────

    /// Export selected rows (or all if none selected) to a `FlatExportData`.
    ///
    /// Requires `T: Exportable`. Only available when export is conceptually enabled.
    pub fn export_data(
        &self,
        scope: crate::utils::export::ExportScope,
    ) -> crate::utils::export::FlatExportData
    where
        T: crate::utils::export::Exportable,
    {
        let names = T::field_names();
        let columns: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let mut data = crate::utils::export::FlatExportData::new(columns);

        match scope {
            crate::utils::export::ExportScope::Selected => {
                for idx in self.selected_rows() {
                    if let Some(row) = self.data.get(idx) {
                        let vals: Vec<crate::utils::export::FieldValue> =
                            (0..T::field_count()).map(|c| row.field_value(c)).collect();
                        data.add_row(vals);
                    }
                }
            }
            crate::utils::export::ExportScope::All => {
                for row in self.data.iter() {
                    let vals: Vec<crate::utils::export::FieldValue> =
                        (0..T::field_count()).map(|c| row.field_value(c)).collect();
                    data.add_row(vals);
                }
            }
        }

        // If scope was Selected but nothing was selected, export all.
        if data.rows.is_empty() && scope == crate::utils::export::ExportScope::Selected {
            return self.export_data(crate::utils::export::ExportScope::All);
        }

        data
    }

    /// Export to string in the given format.
    pub fn export_string(
        &self,
        scope: crate::utils::export::ExportScope,
        format: crate::utils::export::ExportFormat,
    ) -> String
    where
        T: crate::utils::export::Exportable,
    {
        let data = self.export_data(scope);
        crate::utils::export::format_flat(&data, format)
    }

    /// Export to file. Format auto-detected from extension.
    pub fn export_to_file(
        &self,
        scope: crate::utils::export::ExportScope,
        path: &std::path::Path,
    ) -> std::io::Result<()>
    where
        T: crate::utils::export::Exportable,
    {
        let data = self.export_data(scope);
        crate::utils::export::export_flat_to_file(&data, path, None)
    }

    /// Import rows from file, appending to the table.
    pub fn import_from_file(&mut self, path: &std::path::Path) -> Option<usize>
    where
        T: crate::utils::export::Importable,
    {
        let data = crate::utils::export::import_flat_from_file(path)?;
        let mut count = 0;
        for row_vals in &data.rows {
            let fields: Vec<(&str, crate::utils::export::FieldValue)> = data
                .columns
                .iter()
                .zip(row_vals.iter())
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            if let Some(item) = T::from_fields(&fields) {
                self.push(item);
                count += 1;
            }
        }
        Some(count)
    }
}
