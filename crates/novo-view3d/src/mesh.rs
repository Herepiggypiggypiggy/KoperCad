//! Geometry: indexed meshes in an item's local frame, the library the renderer
//! reads them from, and the generated shapes (boxes, rail and duct sections).

use crate::math::Aabb;
use crate::scene::Profile;
use glam::Vec3;
use std::collections::HashMap;
use std::sync::Arc;

/// Hash of the part id and source file hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GeometryKey(pub u64);

/// Indexed triangles in the item's local frame (origin: middle of the back
/// face, see the design doc, section 4).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Feature edges (creases over about 30 degrees), drawn as lines.
    pub edges: Vec<[u32; 2]>,
    pub bounds: Aabb,
}

impl MeshData {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// Merges axis-aligned boxes (min, max) into one mesh with flat normals
    /// and the 12 edges of each box.
    pub fn from_boxes(boxes: &[(Vec3, Vec3)]) -> MeshData {
        let mut m = MeshData {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            edges: Vec::new(),
            bounds: Aabb::EMPTY,
        };
        for &(lo, hi) in boxes {
            m.add_box(lo, hi);
        }
        m
    }

    fn add_box(&mut self, lo: Vec3, hi: Vec3) {
        self.bounds.extend(lo);
        self.bounds.extend(hi);
        // Face = (normal, four corners counter-clockwise seen from outside).
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            (
                Vec3::X,
                [
                    Vec3::new(hi.x, lo.y, lo.z),
                    Vec3::new(hi.x, hi.y, lo.z),
                    Vec3::new(hi.x, hi.y, hi.z),
                    Vec3::new(hi.x, lo.y, hi.z),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    Vec3::new(lo.x, hi.y, lo.z),
                    Vec3::new(lo.x, lo.y, lo.z),
                    Vec3::new(lo.x, lo.y, hi.z),
                    Vec3::new(lo.x, hi.y, hi.z),
                ],
            ),
            (
                Vec3::Y,
                [
                    Vec3::new(hi.x, hi.y, lo.z),
                    Vec3::new(lo.x, hi.y, lo.z),
                    Vec3::new(lo.x, hi.y, hi.z),
                    Vec3::new(hi.x, hi.y, hi.z),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(lo.x, lo.y, lo.z),
                    Vec3::new(hi.x, lo.y, lo.z),
                    Vec3::new(hi.x, lo.y, hi.z),
                    Vec3::new(lo.x, lo.y, hi.z),
                ],
            ),
            (
                Vec3::Z,
                [
                    Vec3::new(lo.x, lo.y, hi.z),
                    Vec3::new(hi.x, lo.y, hi.z),
                    Vec3::new(hi.x, hi.y, hi.z),
                    Vec3::new(lo.x, hi.y, hi.z),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(lo.x, hi.y, lo.z),
                    Vec3::new(hi.x, hi.y, lo.z),
                    Vec3::new(hi.x, lo.y, lo.z),
                    Vec3::new(lo.x, lo.y, lo.z),
                ],
            ),
        ];
        for (n, c) in faces {
            let base = self.positions.len() as u32;
            for p in c {
                self.positions.push(p.to_array());
                self.normals.push(n.to_array());
            }
            self.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        // Edges on the z faces' corners (the -Z face is the last quad, the +Z
        // face the fifth): bottom ring, top ring and four posts.
        let top = self.positions.len() as u32 - 6 * 4 + 4 * 4;
        let bottom = top + 4;
        for ring in [top, bottom] {
            for i in 0..4 {
                self.edges.push([ring + i, ring + (i + 1) % 4]);
            }
        }
        // Face +Z corner i sits above face -Z corner (3 - i... see mapping).
        // +Z: (lo,lo) (hi,lo) (hi,hi) (lo,hi). -Z: (lo,hi) (hi,hi) (hi,lo) (lo,lo).
        for (t, b) in [(0u32, 3u32), (1, 2), (2, 1), (3, 0)] {
            self.edges.push([top + t, bottom + b]);
        }
    }
}

/// Detail levels of one part, `lods[0]` the most detailed.
#[derive(Debug, Clone, PartialEq)]
pub struct PartGeometry {
    pub lods: Vec<MeshData>,
}

/// Geometry the renderer can draw by key. A missing key is drawn as a box.
#[derive(Debug, Clone, Default)]
pub struct MeshLibrary {
    parts: HashMap<GeometryKey, Arc<PartGeometry>>,
    revision: u64,
}

impl MeshLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: GeometryKey, geometry: PartGeometry) {
        self.parts.insert(key, Arc::new(geometry));
        self.revision += 1;
    }

    pub fn get(&self, key: GeometryKey) -> Option<&Arc<PartGeometry>> {
        self.parts.get(&key)
    }

    /// Bumped on every insert, so a view can tell the library changed.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// What a provider knows about a part right now.
#[derive(Debug, Clone)]
pub enum GeometryState {
    None,
    Pending,
    Ready(Arc<PartGeometry>),
    Failed(String),
}

/// Source of part geometry (generated shapes, STEP import). Never blocks.
pub trait GeometryProvider {
    fn geometry(&self, key: GeometryKey) -> GeometryState;
}

/// The unit box x, y in [-0.5, 0.5], z in [0, 1], scaled by the item size.
pub fn unit_box() -> MeshData {
    MeshData::from_boxes(&[(Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 1.0))])
}

/// A rail or duct section of length 1 along X, at real millimetres in y and
/// z. Scale X by the length. `width` and `height` apply to `Channel` only.
pub fn profile_mesh(profile: Profile, width: f32, height: f32) -> MeshData {
    let b = |x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32| {
        (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1))
    };
    let (x0, x1) = (-0.5, 0.5);
    let boxes = match profile {
        Profile::TopHat35x7_5 | Profile::TopHat35x15 => {
            let h = if profile == Profile::TopHat35x15 {
                15.0
            } else {
                7.5
            };
            let t = 1.0;
            vec![
                // Top web, two side walls, two outer flanges.
                b(x0, x1, -12.5, 12.5, h - t, h),
                b(x0, x1, -12.5, -12.5 + t, t, h - t),
                b(x0, x1, 12.5 - t, 12.5, t, h - t),
                b(x0, x1, -17.5, -12.5, 0.0, t),
                b(x0, x1, 12.5, 17.5, 0.0, t),
            ]
        }
        Profile::Channel => {
            let (w, h, t) = (width.max(6.0), height.max(6.0), 2.0);
            vec![
                // Floor and two walls.
                b(x0, x1, -w * 0.5, w * 0.5, 0.0, t),
                b(x0, x1, -w * 0.5, -w * 0.5 + t, t, h),
                b(x0, x1, w * 0.5 - t, w * 0.5, t, h),
            ]
        }
    };
    MeshData::from_boxes(&boxes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_has_twelve_triangles_and_twelve_edges() {
        let m = unit_box();
        assert_eq!(m.triangles(), 12);
        assert_eq!(m.edges.len(), 12);
        assert_eq!(m.positions.len(), 24);
    }

    #[test]
    fn box_edges_join_corners_one_edge_length_apart() {
        let m = MeshData::from_boxes(&[(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 3.0, 5.0))]);
        let mut lengths: Vec<f32> = m
            .edges
            .iter()
            .map(|&[a, b]| {
                (Vec3::from(m.positions[a as usize]) - Vec3::from(m.positions[b as usize])).length()
            })
            .collect();
        lengths.sort_by(|a, b| a.total_cmp(b));
        // Four edges each along x (2), y (3) and z (5).
        assert!(lengths[..4].iter().all(|l| (l - 2.0).abs() < 1e-5));
        assert!(lengths[4..8].iter().all(|l| (l - 3.0).abs() < 1e-5));
        assert!(lengths[8..].iter().all(|l| (l - 5.0).abs() < 1e-5));
    }

    #[test]
    fn rail_profile_is_35_wide_and_as_tall_as_named() {
        let m = profile_mesh(Profile::TopHat35x7_5, 0.0, 0.0);
        let s = m.bounds.size();
        assert!((s.y - 35.0).abs() < 1e-5 && (s.z - 7.5).abs() < 1e-5 && (s.x - 1.0).abs() < 1e-5);
        let tall = profile_mesh(Profile::TopHat35x15, 0.0, 0.0);
        assert!((tall.bounds.size().z - 15.0).abs() < 1e-5);
    }

    #[test]
    fn triangles_wind_counter_clockwise_seen_from_outside() {
        for m in [unit_box(), profile_mesh(Profile::TopHat35x15, 0.0, 0.0)] {
            for t in m.indices.chunks(3) {
                let p = |i: u32| Vec3::from(m.positions[i as usize]);
                let n = Vec3::from(m.normals[t[0] as usize]);
                let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                assert!(
                    face.dot(n) > 0.0,
                    "triangle {t:?} faces away from its normal"
                );
            }
        }
    }

    #[test]
    fn normals_are_unit_and_indices_in_range() {
        for m in [unit_box(), profile_mesh(Profile::Channel, 40.0, 60.0)] {
            let n = m.positions.len() as u32;
            assert!(m.indices.iter().all(|&i| i < n));
            assert!(
                m.normals
                    .iter()
                    .all(|n| (Vec3::from(*n).length() - 1.0).abs() < 1e-5)
            );
        }
    }
}
