//! Resize-edge detection and OS cursor mapping.

use winit::window::CursorIcon;

/// Edge / corner under the cursor, for drag-resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdgeV2 {
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
pub fn cursor_for_edge(edge: Option<ResizeEdgeV2>) -> CursorIcon {
    match edge {
        None => CursorIcon::Default,
        Some(ResizeEdgeV2::North) => CursorIcon::NResize,
        Some(ResizeEdgeV2::South) => CursorIcon::SResize,
        Some(ResizeEdgeV2::East) => CursorIcon::EResize,
        Some(ResizeEdgeV2::West) => CursorIcon::WResize,
        Some(ResizeEdgeV2::NorthEast) => CursorIcon::NeResize,
        Some(ResizeEdgeV2::NorthWest) => CursorIcon::NwResize,
        Some(ResizeEdgeV2::SouthEast) => CursorIcon::SeResize,
        Some(ResizeEdgeV2::SouthWest) => CursorIcon::SwResize,
    }
}

/// winit `ResizeDirection` for the given edge.
pub fn resize_direction(edge: ResizeEdgeV2) -> winit::window::ResizeDirection {
    use winit::window::ResizeDirection as R;
    match edge {
        ResizeEdgeV2::North => R::North,
        ResizeEdgeV2::South => R::South,
        ResizeEdgeV2::East => R::East,
        ResizeEdgeV2::West => R::West,
        ResizeEdgeV2::NorthEast => R::NorthEast,
        ResizeEdgeV2::NorthWest => R::NorthWest,
        ResizeEdgeV2::SouthEast => R::SouthEast,
        ResizeEdgeV2::SouthWest => R::SouthWest,
    }
}

/// Map a local cursor position `(lx, ly)` inside a `w × h` window to a
/// resize edge if it falls within `rz` pixels of any edge. Returns `None`
/// for the inside.
pub fn edge_at(lx: f32, ly: f32, w: f32, h: f32, rz: f32) -> Option<ResizeEdgeV2> {
    if lx < 0.0 || lx >= w || ly < 0.0 || ly >= h {
        return None;
    }
    let l = lx < rz;
    let r = lx > w - rz;
    let t = ly < rz;
    let b = ly > h - rz;
    match (t, b, l, r) {
        (true, _, true, _) => Some(ResizeEdgeV2::NorthWest),
        (true, _, _, true) => Some(ResizeEdgeV2::NorthEast),
        (_, true, true, _) => Some(ResizeEdgeV2::SouthWest),
        (_, true, _, true) => Some(ResizeEdgeV2::SouthEast),
        (true, _, _, _) => Some(ResizeEdgeV2::North),
        (_, true, _, _) => Some(ResizeEdgeV2::South),
        (_, _, true, _) => Some(ResizeEdgeV2::West),
        (_, _, _, true) => Some(ResizeEdgeV2::East),
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
            Some(ResizeEdgeV2::NorthWest)
        );
        // Top-right corner.
        assert_eq!(
            edge_at(95.0, 2.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::NorthEast)
        );
        // Bottom-left corner.
        assert_eq!(
            edge_at(2.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::SouthWest)
        );
        // Bottom-right corner.
        assert_eq!(
            edge_at(95.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::SouthEast)
        );
    }

    #[test]
    fn straight_edges() {
        assert_eq!(
            edge_at(50.0, 2.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::North)
        );
        assert_eq!(
            edge_at(50.0, 95.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::South)
        );
        assert_eq!(
            edge_at(2.0, 50.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::West)
        );
        assert_eq!(
            edge_at(95.0, 50.0, 100.0, 100.0, 6.0),
            Some(ResizeEdgeV2::East)
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
            ResizeEdgeV2::North,
            ResizeEdgeV2::South,
            ResizeEdgeV2::East,
            ResizeEdgeV2::West,
            ResizeEdgeV2::NorthEast,
            ResizeEdgeV2::NorthWest,
            ResizeEdgeV2::SouthEast,
            ResizeEdgeV2::SouthWest,
        ] {
            let _ = cursor_for_edge(Some(edge));
            let _ = resize_direction(edge);
        }
        assert_eq!(cursor_for_edge(None), CursorIcon::Default);
    }
}
