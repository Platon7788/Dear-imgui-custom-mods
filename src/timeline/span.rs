//! Span and Marker data types for the timeline.

/// A single profiling span (a timed interval on a track).
#[derive(Debug, Clone)]
pub struct Span {
    /// Unique span id (for selection / callbacks).
    pub id: u64,
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Nesting depth (0 = top-level).
    pub depth: u32,
    /// Display label.
    pub label: String,
    /// Category name (used for color hashing in ByName mode).
    pub category: String,
    /// Explicit color override (used when ColorMode::Explicit).
    pub color: Option<[f32; 4]>,
    /// Optional source location string.
    pub source: Option<String>,
}

impl Span {
    pub fn new(id: u64, start: f64, end: f64, depth: u32, label: impl Into<String>) -> Self {
        let lbl = label.into();
        // Ensure start <= end and values are finite
        let (s, e) = if start.is_finite() && end.is_finite() {
            if start <= end { (start, end) } else { (end, start) }
        } else {
            (0.0, 0.0)
        };
        let cat = lbl.clone();
        Self {
            id,
            start: s,
            end: e,
            depth,
            category: cat,
            label: lbl,
            color: None,
            source: None,
        }
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Builder: set category.
    pub fn with_category(mut self, cat: impl Into<String>) -> Self {
        self.category = cat.into();
        self
    }

    /// Builder: set explicit color.
    pub fn with_color(mut self, c: [f32; 4]) -> Self {
        self.color = Some(c);
        self
    }

    /// Builder: set source location.
    pub fn with_source(mut self, src: impl Into<String>) -> Self {
        self.source = Some(src.into());
        self
    }
}

/// A vertical marker line on the timeline (e.g., frame boundary, event).
#[derive(Debug, Clone)]
pub struct Marker {
    /// Time position in seconds.
    pub time: f64,
    /// Display label.
    pub label: String,
    /// Color override (default uses config.color_marker).
    pub color: Option<[f32; 4]>,
}

impl Marker {
    pub fn new(time: f64, label: impl Into<String>) -> Self {
        Self { time, label: label.into(), color: None }
    }

    pub fn with_color(mut self, c: [f32; 4]) -> Self {
        self.color = Some(c);
        self
    }
}
