//! Overlays: coloured geometry the host draws on top of the cabinet, such as
//! the slice planes, iso shells and streamlines of the heat layer. They are
//! plain data in the scene frame (millimetres), never part of the `Scene`:
//! picking, marquee, labels and the camera fit ignore them.
//!
//! Opaque meshes are drawn first and write depth. Translucent meshes and
//! lines are drawn after them, in `order` (low first), with the depth test on
//! and depth writes off, so they blend with each other in that order.

use glam::Vec3;

/// A triangle mesh with a colour per vertex (sRGB bytes, straight alpha). It
/// is shaded flat from its own triangles, so no normals are needed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OverlayMesh {
    pub positions: Vec<Vec3>,
    pub colors: Vec<[u8; 4]>,
    /// Three indices per triangle.
    pub indices: Vec<u32>,
    pub translucent: bool,
    /// Draw order among translucent overlays: low first.
    pub order: i32,
}

/// Polylines drawn as screen-space ribbons of constant pixel width, for
/// streamlines and tracers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OverlayLines {
    pub positions: Vec<Vec3>,
    pub colors: Vec<[u8; 4]>,
    /// Index into `positions` where each polyline starts; empty means the
    /// whole list is one polyline.
    pub starts: Vec<u32>,
    pub width_px: f32,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    Mesh(OverlayMesh),
    Lines(OverlayLines),
}

impl Overlay {
    pub fn order(&self) -> i32 {
        match self {
            Overlay::Mesh(m) => m.order,
            Overlay::Lines(l) => l.order,
        }
    }
}

/// Upper bounds that keep one bad overlay from taking the GPU down.
pub const MAX_OVERLAY_VERTICES: usize = 4_000_000;

impl OverlayMesh {
    /// Whether the arrays agree: one colour per vertex, whole triangles, every
    /// index in range, every coordinate finite.
    pub fn is_valid(&self) -> bool {
        self.positions.len() == self.colors.len()
            && self.positions.len() <= MAX_OVERLAY_VERTICES
            && self.indices.len().is_multiple_of(3)
            && self
                .indices
                .iter()
                .all(|&i| (i as usize) < self.positions.len())
            && self.positions.iter().all(|p| p.is_finite())
    }
}

impl OverlayLines {
    pub fn is_valid(&self) -> bool {
        self.positions.len() == self.colors.len()
            && self.positions.len() <= MAX_OVERLAY_VERTICES
            && self.width_px.is_finite()
            && self
                .starts
                .iter()
                .all(|&s| (s as usize) <= self.positions.len())
            && self.positions.iter().all(|p| p.is_finite())
    }

    /// The segments `(a, b)` as vertex index pairs, polylines kept apart.
    pub fn segments(&self) -> Vec<(u32, u32)> {
        let n = self.positions.len() as u32;
        let mut starts: Vec<u32> = if self.starts.is_empty() {
            vec![0]
        } else {
            self.starts.clone()
        };
        starts.sort_unstable();
        starts.dedup();
        let mut out = Vec::new();
        for (k, &s) in starts.iter().enumerate() {
            let end = starts.get(k + 1).copied().unwrap_or(n).min(n);
            for i in s..end {
                if i + 1 < end {
                    out.push((i, i + 1));
                }
            }
        }
        out
    }
}

impl Overlay {
    pub fn is_valid(&self) -> bool {
        match self {
            Overlay::Mesh(m) => m.is_valid(),
            Overlay::Lines(l) => l.is_valid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(n: usize, starts: Vec<u32>) -> OverlayLines {
        OverlayLines {
            positions: (0..n).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect(),
            colors: vec![[255; 4]; n],
            starts,
            width_px: 2.0,
            order: 0,
        }
    }

    #[test]
    fn polylines_do_not_join_across_starts() {
        let l = lines(7, vec![0, 3]);
        assert_eq!(l.segments(), vec![(0, 1), (1, 2), (3, 4), (4, 5), (5, 6)]);
        assert_eq!(lines(3, vec![]).segments(), vec![(0, 1), (1, 2)]);
        assert!(lines(1, vec![]).segments().is_empty());
        assert!(lines(0, vec![]).segments().is_empty());
    }

    #[test]
    fn bad_arrays_are_rejected() {
        let mut m = OverlayMesh {
            positions: vec![Vec3::ZERO; 3],
            colors: vec![[0; 4]; 3],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        assert!(m.is_valid());
        m.indices = vec![0, 1, 3];
        assert!(!m.is_valid());
        m.indices = vec![0, 1];
        assert!(!m.is_valid());
        m.indices = vec![0, 1, 2];
        m.colors.pop();
        assert!(!m.is_valid());
        m.colors.push([0; 4]);
        m.positions[1] = Vec3::new(f32::NAN, 0.0, 0.0);
        assert!(!m.is_valid());
        let mut l = lines(2, vec![5]);
        assert!(!l.is_valid());
        l.starts = vec![0];
        l.width_px = f32::INFINITY;
        assert!(!l.is_valid());
    }
}
