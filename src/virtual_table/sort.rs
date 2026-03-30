//! Sort state — wraps Dear ImGui's TableSortSpecs.

/// Cached sort specification from Dear ImGui.
#[derive(Clone, Debug)]
pub(crate) struct SortSpec {
    pub column_index: usize,
    pub ascending: bool,
}

/// Manages sort state and applies sorting to data.
#[derive(Clone, Debug, Default)]
pub(crate) struct SortState {
    pub specs: Vec<SortSpec>,
}
