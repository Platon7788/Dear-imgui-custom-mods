//! Track — a named row of spans (e.g., one per thread or category).

use super::span::Span;

/// A track groups spans that belong to the same logical lane
/// (thread, CPU core, category, etc.).
#[derive(Debug, Clone)]
pub struct Track {
    /// Display name shown in the label sidebar.
    pub name: String,
    /// Spans on this track, sorted by start time.
    pub spans: Vec<Span>,
    /// Whether this track is collapsed (hide spans, show only header).
    pub collapsed: bool,
    /// Optional color override for the track header.
    pub color: Option<[f32; 4]>,
}

impl Track {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            spans: Vec::new(),
            collapsed: false,
            color: None,
        }
    }

    /// Add a span to this track. Keeps sorted by start time.
    pub fn add_span(&mut self, span: Span) {
        let idx = self.spans.partition_point(|s| s.start <= span.start);
        self.spans.insert(idx, span);
    }

    /// Maximum depth among all spans on this track.
    pub fn max_depth(&self) -> u32 {
        self.spans.iter().map(|s| s.depth).max().unwrap_or(0)
    }

    /// Total visual height for this track (number of depth rows).
    pub fn depth_rows(&self) -> u32 {
        self.max_depth() + 1
    }

    /// Time range covered by this track.
    pub fn time_range(&self) -> Option<(f64, f64)> {
        if self.spans.is_empty() {
            return None;
        }
        let start = self.spans.iter().map(|s| s.start).fold(f64::MAX, f64::min);
        let end = self.spans.iter().map(|s| s.end).fold(f64::MIN, f64::max);
        Some((start, end))
    }
}
