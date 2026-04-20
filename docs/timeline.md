# Timeline

Zoomable horizontal timeline for Dear ImGui — profiler flame graph / icicle chart with tracks, spans, markers, and tooltips.

## Overview

`Timeline` displays nested call spans as colored bars across multiple tracks (one per thread or category). Designed for profiler data visualization with full pan/zoom, selection, and marker support.

## Features

- **Multiple tracks** — one per thread / CPU core / category
- **Nested spans** — depth-based vertical stacking (icicle chart or flame graph)
- **Pan and zoom** — mouse wheel zoom (to cursor), middle/right-click pan
- **Smooth zoom** — exponential ease-out interpolation
- **Time ruler** — adaptive tick marks with automatic unit selection (ns/us/ms/s)
- **Track labels** — sidebar with collapse/expand arrows
- **Track collapse** — hide spans, show only header
- **Vertical markers** — labeled vertical lines (e.g. frame boundaries)
- **Span selection** — click to select, double-click events
- **Hover tooltips** — span name, duration, category, source, depth
- **Fit to content** — zoom to show all data
- **4 color modes**: ByName (hash), ByDuration (heat), ByDepth (palette cycle), Explicit (per-span)
- **10-color span palette** — cycled by name hash or depth
- **Track striping** — alternating background for readability
- **Vertical scroll** — Shift+Wheel for tall track lists
- **Frustum culling** — off-screen tracks and spans are skipped

## Quick Start

```rust
use dear_imgui_custom_mod::timeline::{Timeline, Track, Span, Marker};

let mut tl = Timeline::new("##profiler");

// Add a track with spans
let mut track = Track::new("Main Thread");
track.add_span(Span::new(0, 0.0, 0.050, 0, "frame"));
track.add_span(Span::new(1, 0.0, 0.020, 1, "update"));
track.add_span(Span::new(2, 0.020, 0.050, 1, "render"));
tl.add_track(track);

// Add a frame marker
tl.add_marker(Marker::new(0.016, "16ms"));

// Fit view to data
let avail_w = 800.0; // content width in pixels
tl.fit_to_content(avail_w);

// In render loop:
let events = tl.render(ui);
for event in events {
    match event {
        TimelineEvent::SpanClicked { span_id } => { /* select span */ }
        TimelineEvent::SpanDoubleClicked { span_id } => { /* zoom to span */ }
        TimelineEvent::MarkerClicked { index } => { /* jump to marker */ }
        TimelineEvent::ViewChanged { start, end } => { /* sync other views */ }
    }
}
```

## Public API

### Construction

| Method | Description |
|--------|-------------|
| `new(id)` | Create a new timeline |

### Data Management

| Method | Description |
|--------|-------------|
| `add_track(track)` | Add a track |
| `track_mut(index) -> Option<&mut Track>` | Mutable access to a track |
| `tracks() -> &[Track]` | Read-only access to all tracks |
| `clear_tracks()` | Remove all tracks |
| `add_marker(marker)` | Add a vertical marker line |
| `clear_markers()` | Remove all markers |

### State

| Method | Description |
|--------|-------------|
| `selected_span() -> Option<u64>` | Currently selected span ID |
| `data_time_range() -> (f64, f64)` | Time range of all data |
| `fit_to_content(width)` | Zoom to fit all data in given pixel width |

### Rendering

| Method | Description |
|--------|-------------|
| `render(ui) -> Vec<TimelineEvent>` | Render the timeline. Returns events |

## Events

| Event | Description |
|-------|-------------|
| `SpanClicked { span_id }` | Span was clicked |
| `SpanDoubleClicked { span_id }` | Span was double-clicked |
| `MarkerClicked { index }` | Marker was clicked |
| `ViewChanged { start, end }` | View was panned or zoomed (visible time range in seconds) |

## Data Types

### Span

```rust
pub struct Span {
    pub id: u64,                   // unique ID
    pub start: f64,                // start time (seconds)
    pub end: f64,                  // end time (seconds)
    pub depth: u32,                // nesting depth (0 = top-level)
    pub label: String,             // display text
    pub category: String,          // category for color hashing
    pub color: Option<[f32; 4]>,   // explicit color override
    pub source: Option<String>,    // source location string
}

Span::new(id, start, end, depth, label)
    .with_category("rendering")
    .with_color([1.0, 0.0, 0.0, 1.0])
    .with_source("renderer.rs:42")
```

### Track

```rust
pub struct Track {
    pub name: String,              // sidebar label
    pub spans: Vec<Span>,          // sorted by start time
    pub collapsed: bool,           // hide spans
    pub color: Option<[f32; 4]>,   // header color override
}

Track::new("Main Thread")
```

| Method | Description |
|--------|-------------|
| `add_span(span)` | Add span (auto-sorted by start time) |
| `max_depth() -> u32` | Maximum nesting depth |
| `depth_rows() -> u32` | Number of visual rows (`max_depth + 1`) |
| `time_range() -> Option<(f64, f64)>` | Time range of this track |

### Marker

```rust
pub struct Marker {
    pub time: f64,                 // position in seconds
    pub label: String,             // display text
    pub color: Option<[f32; 4]>,   // color override
}

Marker::new(0.016, "frame")
    .with_color([1.0, 1.0, 0.0, 1.0])
```

## Configuration

```rust
let cfg = &mut tl.config;

// Layout
cfg.row_height = 20.0;           // span bar height
cfg.row_gap = 1.0;               // gap between depth rows
cfg.ruler_height = 24.0;         // time ruler height
cfg.track_label_width = 120.0;   // sidebar label width
cfg.min_span_width = 2.0;        // minimum visible span width
cfg.track_header_height = 22.0;  // track header height

// Behavior
cfg.mode = TimelineMode::TopDown;   // or BottomUp (flame graph)
cfg.color_mode = ColorMode::ByName; // ByName, ByDuration, ByDepth, Explicit
cfg.show_ruler = true;
cfg.show_track_labels = true;
cfg.show_tooltip = true;
cfg.show_markers = true;
cfg.smooth_zoom = true;
cfg.smooth_zoom_speed = 12.0;
cfg.min_zoom = 1e-9;             // minimum pixels per second
cfg.max_zoom = 1e6;              // maximum pixels per second
```

### TimelineMode

| Mode | Description |
|------|-------------|
| `TopDown` | Parent spans at top, children below (icicle chart, default) |
| `BottomUp` | Aggregated stacks, hottest at top (flame graph) |

### ColorMode

| Mode | Description |
|------|-------------|
| `ByName` | Hash category name into palette (default) |
| `ByDuration` | Heat map: short=blue, long=red |
| `ByDepth` | Cycle palette by nesting depth |
| `Explicit` | Use each span's `color` field |

### Colors

| Field | Description |
|-------|-------------|
| `color_bg` | Background |
| `color_bg_alt` | Alternate track background (striping) |
| `color_ruler_bg` | Ruler background |
| `color_ruler_text` | Ruler text and tick marks |
| `color_track_label` | Track label text |
| `color_track_separator` | Track header separator line |
| `color_span_text` | Text on span bars |
| `color_selection` | Selected span outline |
| `color_hover` | Hovered span overlay |
| `color_marker` | Default marker line color |
| `color_tooltip_bg` | Tooltip background |
| `color_tooltip_text` | Tooltip text color |
| `span_palette` | Vec of 10 RGBA colors for span bars |

## Architecture

```
timeline/
  mod.rs      Timeline struct, rendering, viewport, pan/zoom, input
  config.rs   TimelineConfig, TimelineMode, ColorMode, TimeUnit
  span.rs     Span and Marker data types with builders
  track.rs    Track — named row of sorted spans
```
