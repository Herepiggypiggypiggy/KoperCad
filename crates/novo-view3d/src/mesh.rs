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

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Keeps the parts `keep` accepts. Returns how many it dropped; the
    /// revision moves only then.
    pub fn retain(&mut self, mut keep: impl FnMut(GeometryKey) -> bool) -> usize {
        let before = self.parts.len();
        self.parts.retain(|k, _| keep(*k));
        let dropped = before - self.parts.len();
        if dropped > 0 {
            self.revision += 1;
        }
        dropped
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

/// The boxes a rail or duct section is made of: length 1 along X (x from
/// -0.5 to 0.5), real millimetres in y and z. `width` and `height` apply to
/// `Channel` only. Also what a STEP export of the cabinet writes.
pub fn profile_boxes(profile: Profile, width: f32, height: f32) -> Vec<(Vec3, Vec3)> {
    let b = |x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32| {
        (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1))
    };
    let (x0, x1) = (-0.5, 0.5);
    match profile {
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
        // Unit length has no slots to show; `slotted_boxes` makes the real one.
        Profile::Channel | Profile::SlottedChannel => {
            let (w, h, t) = (width.max(6.0), height.max(6.0), DUCT_WALL_MM);
            vec![
                // Floor and two walls.
                b(x0, x1, -w * 0.5, w * 0.5, 0.0, t),
                b(x0, x1, -w * 0.5, -w * 0.5 + t, t, h),
                b(x0, x1, w * 0.5 - t, w * 0.5, t, h),
            ]
        }
    }
}

/// Wall and floor thickness of a duct, mm.
pub const DUCT_WALL_MM: f32 = 2.0;
/// Width of a slot in a duct wall, mm. Typical of commercial slotted duct
/// (about 5 mm slots between 4 mm fingers), not the figures of one maker.
pub const SLOT_WIDTH_MM: f32 = 5.0;
/// Width of a finger between two slots, mm.
pub const FINGER_WIDTH_MM: f32 = 4.0;
/// Pitch of the mounting slots in the duct floor, mm.
pub const MOUNT_PITCH_MM: f32 = 60.0;
/// Length and width of a floor mounting slot, mm.
pub const MOUNT_SLOT_MM: (f32, f32) = (12.0, 5.0);

/// The solid band at the foot of a slotted wall, mm: slots start above it, so
/// the wall keeps its stiffness. `None` when the duct is too low to slot.
fn slot_base(height: f32) -> Option<f32> {
    let base = ((height - DUCT_WALL_MM) * 0.18).clamp(2.0, 6.0);
    (height - DUCT_WALL_MM - base >= 8.0).then_some(base)
}

/// Number of fingers on a wall of length `len` that starts and ends with a
/// finger, and the width each gets so the slots keep their width.
fn finger_layout(len: f32) -> Option<(usize, f32)> {
    let n = ((len + SLOT_WIDTH_MM) / (FINGER_WIDTH_MM + SLOT_WIDTH_MM)).round() as usize;
    // Fewer than three fingers is a stub, not a slotted duct.
    if n < 3 {
        return None;
    }
    let finger = (len - (n - 1) as f32 * SLOT_WIDTH_MM) / n as f32;
    (finger >= 1.0).then_some((n, finger))
}

/// The boxes of a slotted duct of real length `len`: x from `-len / 2` to
/// `len / 2`, real millimetres in y and z. Both walls are cut into fingers
/// from the rim down to a solid base band, and the floor has a row of
/// mounting slots. Falls back to the solid section when the duct is too short
/// or too low to carry slots. Not a maker's drawing: the proportions are the
/// typical ones above.
pub fn slotted_boxes(len: f32, width: f32, height: f32) -> Vec<(Vec3, Vec3)> {
    let solid = || {
        profile_boxes(Profile::Channel, width, height)
            .into_iter()
            .map(|(a, b)| {
                (
                    Vec3::new(a.x * len, a.y, a.z),
                    Vec3::new(b.x * len, b.y, b.z),
                )
            })
            .collect::<Vec<_>>()
    };
    let (w, h, t) = (width.max(6.0), height.max(6.0), DUCT_WALL_MM);
    let (Some(base), Some((n, finger))) = (slot_base(h), finger_layout(len)) else {
        return solid();
    };
    let b = |x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32| {
        (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1))
    };
    let (x0, x1) = (-len * 0.5, len * 0.5);
    let mut boxes = Vec::new();

    // Floor: two side strips along the whole length, and bars across the
    // middle between the mounting slots.
    let (slot_len, slot_w) = MOUNT_SLOT_MM;
    let slot_w = slot_w.min((w - 2.0 * t) * 0.4);
    if slot_w >= 2.0 && len >= slot_len + 2.0 * t {
        let half = slot_w * 0.5;
        boxes.push(b(x0, x1, -w * 0.5, -half, 0.0, t));
        boxes.push(b(x0, x1, half, w * 0.5, 0.0, t));
        let count = ((len - slot_len) / MOUNT_PITCH_MM).floor() as usize + 1;
        let span = (count - 1) as f32 * MOUNT_PITCH_MM;
        let first = -span * 0.5;
        let mut edge = x0;
        for k in 0..count {
            let cx = first + k as f32 * MOUNT_PITCH_MM;
            boxes.push(b(edge, cx - slot_len * 0.5, -half, half, 0.0, t));
            edge = cx + slot_len * 0.5;
        }
        boxes.push(b(edge, x1, -half, half, 0.0, t));
    } else {
        boxes.push(b(x0, x1, -w * 0.5, w * 0.5, 0.0, t));
    }

    // Walls: a solid base band, then the fingers.
    for (y0, y1) in [(-w * 0.5, -w * 0.5 + t), (w * 0.5 - t, w * 0.5)] {
        boxes.push(b(x0, x1, y0, y1, t, t + base));
        for k in 0..n {
            let fx = x0 + k as f32 * (finger + SLOT_WIDTH_MM);
            boxes.push(b(fx, fx + finger, y0, y1, t + base, h));
        }
    }
    boxes
}

/// A slotted duct of real length `len` as one mesh. See [`slotted_boxes`].
pub fn slotted_mesh(len: f32, width: f32, height: f32) -> MeshData {
    MeshData::from_boxes(&slotted_boxes(len, width, height))
}

/// Share of a slotted wall's area that is open, 0 to 1: the part of the wall
/// above the base band that is slot rather than finger, taken over the whole
/// wall. 0 for a duct too short or low to slot. A drawing figure only: the
/// heat model describes a duct by one blockage number and does not read this.
pub fn slotted_open_fraction(len: f32, height: f32) -> f32 {
    let h = height.max(6.0);
    let (Some(base), Some((n, _))) = (slot_base(h), finger_layout(len)) else {
        return 0.0;
    };
    (n - 1) as f32 * SLOT_WIDTH_MM * (h - DUCT_WALL_MM - base) / (len * h)
}

/// A rail or duct section of length 1 along X, at real millimetres in y and
/// z. Scale X by the length. `width` and `height` apply to `Channel` only.
pub fn profile_mesh(profile: Profile, width: f32, height: f32) -> MeshData {
    MeshData::from_boxes(&profile_boxes(profile, width, height))
}

#[cfg(test)]
mod tests {
    #[test]
    fn retain_drops_what_is_not_kept_and_moves_the_revision_only_then() {
        let mut lib = MeshLibrary::new();
        for k in [1, 2, 3] {
            lib.insert(
                GeometryKey(k),
                PartGeometry {
                    lods: vec![unit_box()],
                },
            );
        }
        let rev = lib.revision();
        assert_eq!(lib.retain(|_| true), 0);
        assert_eq!(lib.revision(), rev);
        assert_eq!(lib.retain(|k| k.0 != 2), 1);
        assert_eq!(lib.len(), 2);
        assert!(lib.get(GeometryKey(2)).is_none());
        assert!(lib.revision() > rev);
    }

    use super::*;

    fn tris(boxes: &[(Vec3, Vec3)]) -> usize {
        MeshData::from_boxes(boxes).triangles()
    }

    #[test]
    fn a_slotted_duct_has_the_extent_of_the_solid_one() {
        let a = MeshData::from_boxes(&slotted_boxes(600.0, 40.0, 60.0)).bounds;
        let b = MeshData::from_boxes(&profile_boxes(Profile::Channel, 40.0, 60.0)).bounds;
        assert!((a.size().y - b.size().y).abs() < 1e-4 && (a.size().z - b.size().z).abs() < 1e-4);
        assert!((a.size().x - 600.0).abs() < 1e-3, "{}", a.size().x);
        assert!((a.min.x + 300.0).abs() < 1e-3);
    }

    #[test]
    fn slots_keep_their_width_and_the_pitch_does_not_depend_on_the_length() {
        for len in [100.0_f32, 250.0, 600.0, 1000.0, 2000.0] {
            let (n, finger) = finger_layout(len).expect("long enough");
            let pitch = finger + SLOT_WIDTH_MM;
            assert!((pitch - 9.0).abs() < 1.0, "{len}: pitch {pitch}");
            let total = n as f32 * finger + (n - 1) as f32 * SLOT_WIDTH_MM;
            assert!(
                (total - len).abs() < 1e-3,
                "{len}: fingers and slots fill the length"
            );
        }
    }

    #[test]
    fn a_wall_starts_and_ends_with_a_finger() {
        let boxes = slotted_boxes(500.0, 40.0, 60.0);
        let xs: Vec<f32> = boxes
            .iter()
            .filter(|(a, b)| a.z > 7.0 && b.z > 59.0 && a.y < 0.0)
            .map(|(a, _)| a.x)
            .collect();
        assert!((xs.iter().cloned().fold(f32::MAX, f32::min) + 250.0).abs() < 1e-3);
    }

    #[test]
    fn a_short_or_low_duct_stays_solid() {
        let solid = profile_boxes(Profile::Channel, 40.0, 60.0).len();
        assert_eq!(slotted_boxes(12.0, 40.0, 60.0).len(), solid, "too short");
        assert_eq!(slotted_boxes(500.0, 40.0, 10.0).len(), solid, "too low");
        assert_eq!(slotted_open_fraction(12.0, 60.0), 0.0);
    }

    #[test]
    fn about_half_the_wall_of_a_tall_duct_is_open() {
        let f = slotted_open_fraction(1000.0, 60.0);
        assert!((0.35..0.6).contains(&f), "{f}");
        let low = slotted_open_fraction(1000.0, 25.0);
        assert!(
            low < f,
            "a low duct has less above its base band: {low} vs {f}"
        );
    }

    #[test]
    fn a_metre_of_duct_stays_cheap_to_draw() {
        let t = tris(&slotted_boxes(1000.0, 40.0, 60.0));
        assert!(t < 3500, "{t} triangles");
        let t2 = tris(&slotted_boxes(2000.0, 80.0, 80.0));
        assert!(t2 < 7000, "{t2} triangles");
    }

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
