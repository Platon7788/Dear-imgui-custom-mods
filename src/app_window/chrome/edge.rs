//! Resize-edge detection and OS cursor mapping.

use winit::window::CursorIcon;

/// Edge / corner under the cursor, for drag-resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// OS cursor for the given hover edge.
pub fn cursor_for_edge(edge: Option<ResizeEdge>) -> CursorIcon {
    match edge {
        None => CursorIcon::Default,
        Some(ResizeEdge::North) => CursorIcon::NResize,
        Some(ResizeEdge::South) => CursorIcon::SResize,
        Some(ResizeEdge::East) => CursorIcon::EResize,
        Some(ResizeEdge::West) => CursorIcon::WResize,
        Some(ResizeEdge::NorthEast) => CursorIcon::NeResize,
        Some(ResizeEdge::NorthWest) => CursorIcon::NwResize,
        Some(ResizeEdge::SouthEast) => CursorIcon::SeResize,
        Some(ResizeEdge::SouthWest) => CursorIcon::SwResize,
    }
}

/// winit `ResizeDirection` for the given edge.
pub fn resize_direction(edge: ResizeEdge) -> winit::window::ResizeDirection {
    use winit::window::ResizeDirection as R;
    match edge {
        ResizeEdge::North => R::North,
        ResizeEdge::South => R::South,
        ResizeEdge::East => R::East,
        ResizeEdge::West => R::West,
        ResizeEdge::NorthEast => R::NorthEast,
        ResizeEdge::NorthWest => R::NorthWest,
        ResizeEdge::SouthEast => R::SouthEast,
        ResizeEdge::SouthWest => R::SouthWest,
    }
}

/// Map a local cursor position `(lx, ly)` inside a `w × h` window to a
/// resize edge if it falls within `rz` pixels of any edge. Returns `None`
/// for the inside.
pub fn edge_at(lx: f32, ly: f32, w: f32, h: f32, rz: f32) -> Option<ResizeEdge> {
    if lx < 0.0 || lx >= w || ly < 0.0 || ly >= h {
        return None;
    }
    let l = lx < rz;
    let r = lx > w - rz;
    let t = ly < rz;
    let b = ly > h - rz;
    match (t, b, l, r) {
        (true, _, true, _) => Some(ResizeEdge::NorthWest),
        (true, _, _, true) => Some(ResizeEdge::NorthEast),
        (_, true, true, _) => Some(ResizeEdge::SouthWest),
        (_, true, _, true) => Some(ResizeEdge::SouthEast),
        (true, _, _, _) => Some(ResizeEdge::North),
        (_, true, _, _) => Some(ResizeEdge::South),
        (_, _, true, _) => Some(ResizeEdge::West),
        (_, _, _, true) => Some(ResizeEdge::East),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_returns_none() {
        assert_eq!(edge_at(-1.0, 5.0, 100.0, 100.0, 6.0), None);
        assert_eq!(edge_at(5.0, -1.0, 100.0, 100.0, 6.0), None);
        assert_eq!(edge_at(100.0, 5.0, 100.0, 100.0, 6.0), None);
        assert_eq!(edge_at(5.0, 100.0, 100.0, 100.0, 6.0), None);
    }

    #[test]
    fn corners_take_priority() {
        // Top-left corner.
        assert_eq!(
            edge_at(2.0, 2.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::NorthWest)
        );
        // Top-right corner.
        assert_eq!(
            edge_at(95.0, 2.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::NorthEast)
        );
        // Bottom-left corner.
        assert_eq!(
            edge_at(2.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::SouthWest)
        );
        // Bottom-right corner.
        assert_eq!(
            edge_at(95.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::SouthEast)
        );
    }

    #[test]
    fn straight_edges() {
        assert_eq!(
            edge_at(50.0, 2.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::North)
        );
        assert_eq!(
            edge_at(50.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::South)
        );
        assert_eq!(
            edge_at(2.0, 50.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::West)
        );
        assert_eq!(
            edge_at(95.0, 50.0, 100.0, 100.0, 6.0),
            Some(ResizeEdge::East)
        );
    }

    #[test]
    fn interior_returns_none() {
        assert_eq!(edge_at(50.0, 50.0, 100.0, 100.0, 6.0), None);
    }

    #[test]
    fn cursor_for_edge_covers_all_variants() {
        // Smoke test — every edge yields some cursor (no panic).
        for edge in [
            ResizeEdge::North,
            ResizeEdge::South,
            ResizeEdge::East,
            ResizeEdge::West,
            ResizeEdge::NorthEast,
            ResizeEdge::NorthWest,
            ResizeEdge::SouthEast,
            ResizeEdge::SouthWest,
        ] {
            let _ = cursor_for_edge(Some(edge));
            let _ = resize_direction(edge);
        }
        assert_eq!(cursor_for_edge(None), CursorIcon::Default);
    }
}
