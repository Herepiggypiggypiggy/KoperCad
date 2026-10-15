//! The render mesh of a part and the steps that turn a triangle soup into one:
//! axis assignment, mounting side, simplification, smooth normals.

use std::collections::HashMap;

use super::build::StepStats;
use super::decimate::{cluster_to, simplify, weld};
use super::math::*;
use super::tess::Soup;

/// Why a part is drawn as a plain box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// The part has no 3D model.
    NoModel,
    /// The file is not a STEP file this reader understands.
    Unreadable,
    /// STEP, but without solid or surface geometry we read.
    NoGeometry,
    /// Too many faces for a quick preview.
    TooComplex,
    /// Reading took longer than the time allowed.
    TimedOut,
    Cancelled,
}

/// How the model's axes were matched to width, height and depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Matched to the catalog's width, height and depth.
    MatchedToCatalog,
    /// No catalog size: smallest extent is depth, largest is height. The
    /// part may be turned wrongly; a stored per-part rotation fixes it.
    Guessed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshOrigin {
    /// Built from width, height and depth because the part has no model.
    Box,
    /// A box because the model could not be used.
    Fallback(Fallback),
    Step {
        stats: StepStats,
        orientation: Orientation,
        /// Triangles after tessellation and welding, before simplification.
        before_simplify: u32,
    },
}

/// Axis-aligned bounds, millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn of(positions: &[[f32; 3]]) -> Aabb {
        let mut b = Aabb {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
        };
        for p in positions {
            for (k, &v) in p.iter().enumerate() {
                b.min[k] = b.min[k].min(v);
                b.max[k] = b.max[k].max(v);
            }
        }
        if positions.is_empty() {
            b = Aabb {
                min: [0.0; 3],
                max: [0.0; 3],
            };
        }
        b
    }

    pub fn size(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// One level of detail: indexed triangles in part space (x width, y height,
/// z depth; x and y centred, the mounting side at z = 0, the part growing
/// towards +z, the door). Millimetres. Triangles are counter-clockwise seen
/// from outside. Matches `MeshData` of `docs/design/3d-view.md` section 5.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Feature edges: creases over 30 degrees and open boundaries, for the
    /// line pass of the renderer. Indices into `positions`.
    pub edges: Vec<[u32; 2]>,
    pub bounds: Aabb,
}

impl MeshData {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Bytes this level takes in GPU buffers (positions, normals, indices,
    /// edge indices).
    pub fn gpu_bytes(&self) -> usize {
        self.positions.len() * 12
            + self.normals.len() * 12
            + self.indices.len() * 4
            + self.edges.len() * 8
    }

    /// The bounds as a 12-triangle box: the cheapest level of detail.
    pub fn bounding_box(&self) -> MeshData {
        let s = self.bounds.size();
        let mut m = box_data([f64::from(s[0]), f64::from(s[1]), f64::from(s[2])]);
        let shift = [
            (self.bounds.max[0] + self.bounds.min[0]) / 2.0,
            (self.bounds.max[1] + self.bounds.min[1]) / 2.0,
            self.bounds.min[2],
        ];
        for p in &mut m.positions {
            for k in 0..3 {
                p[k] += shift[k];
            }
        }
        m.bounds = Aabb::of(&m.positions);
        m
    }
}

/// What a part is drawn with: detailed to cheapest. `lods[0]` is at most
/// the quality budget (2000 triangles for a device at normal quality),
/// the next at most 300, and the last is always the 12-triangle box of the
/// model's bounds. Matches `PartGeometry` of the 3D design plus the origin.
#[derive(Debug, Clone, PartialEq)]
pub struct PartGeometry {
    pub lods: Vec<MeshData>,
    pub origin: MeshOrigin,
}

impl PartGeometry {
    /// The most detailed level.
    pub fn detail(&self) -> &MeshData {
        &self.lods[0]
    }

    /// Extents along x, y and z.
    pub fn size_mm(&self) -> [f32; 3] {
        self.lods[0].bounds.size()
    }

    pub fn is_box(&self) -> bool {
        matches!(self.origin, MeshOrigin::Box | MeshOrigin::Fallback(_))
    }

    /// Triangles per level, for choosing levels against a scene budget.
    pub fn lod_triangles(&self) -> Vec<u32> {
        self.lods
            .iter()
            .map(|l| l.triangle_count() as u32)
            .collect()
    }
}

/// How well a model fits the stored footprint of its part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fit {
    /// Every extent within [`FIT_TOLERANCE_MM`] of the stored size.
    Fits,
    /// The model is bigger or smaller than the stored size by this much
    /// (mm) in its worst axis: the scene flags the item instead of drawing
    /// outside the footprint.
    Mismatch { worst_mm: f32 },
}

/// The design allows 2 mm between a model and its footprint.
pub const FIT_TOLERANCE_MM: f32 = 2.0;

/// Compares a model's extents with the part's stored width, height, depth.
pub fn fit_to_footprint(model: &PartGeometry, size_mm: [f64; 3]) -> Fit {
    let s = model.size_mm();
    let worst = (0..3)
        .map(|k| (f64::from(s[k]) - size_mm[k]).abs() as f32)
        .fold(0.0f32, f32::max);
    if worst <= FIT_TOLERANCE_MM {
        Fit::Fits
    } else {
        Fit::Mismatch { worst_mm: worst }
    }
}

/// Dihedral angle above which an edge is a feature edge (the design says
/// about 30 degrees; 35 leaves out the noise of simplified curved walls).
pub const EDGE_CREASE_DEG: f64 = 35.0;

/// Smallest edge a box part may have, in millimetres.
pub const MIN_BOX_MM: f64 = 1.0;

/// A plain box of width x height x depth (x, y, z), mounting side at z = 0.
pub fn box_data(size_mm: [f64; 3]) -> MeshData {
    let s = size_mm.map(|v| {
        if v.is_finite() {
            v.max(MIN_BOX_MM)
        } else {
            MIN_BOX_MM
        }
    });
    let (hx, hy, d) = ((s[0] / 2.0) as f32, (s[1] / 2.0) as f32, s[2] as f32);
    let lo = [-hx, -hy, 0.0];
    let hi = [hx, hy, d];
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    // For each face: axis, sign. The other two axes are (u, v) with
    // u x v pointing along +axis, so a counter-clockwise quad faces out.
    for axis in 0..3usize {
        for sign in [-1.0f32, 1.0] {
            let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
            let c = if sign > 0.0 { hi[axis] } else { lo[axis] };
            let base = positions.len() as u32;
            let corners = [
                (lo[ua], lo[va]),
                (hi[ua], lo[va]),
                (hi[ua], hi[va]),
                (lo[ua], hi[va]),
            ];
            for (u, v) in corners {
                let mut p = [0.0f32; 3];
                p[axis] = c;
                p[ua] = u;
                p[va] = v;
                positions.push(p);
                let mut n = [0.0f32; 3];
                n[axis] = sign;
                normals.push(n);
            }
            if sign > 0.0 {
                indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
            }
        }
    }
    let edges = mesh_edges(&positions, &indices, EDGE_CREASE_DEG);
    let bounds = Aabb::of(&positions);
    MeshData {
        positions,
        normals,
        indices,
        edges,
        bounds,
    }
}

/// A box part: one level of detail.
pub fn box_mesh(size_mm: [f64; 3]) -> PartGeometry {
    PartGeometry {
        lods: vec![box_data(size_mm)],
        origin: MeshOrigin::Box,
    }
}

/// Triangle budgets by part size and quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshBudget {
    pub max_triangles: u32,
}

impl MeshBudget {
    /// The fewest triangles a model mesh may be cut to: a box.
    pub const MIN: u32 = 12;

    /// Budget for a part with the given longest edge: terminals and relays
    /// are small and numerous, a gland plate or enclosure is rare.
    pub fn for_size(longest_mm: f64, quality: Quality) -> MeshBudget {
        // The design budget for a device is 2000 triangles at normal quality.
        let base = if longest_mm <= 25.0 {
            500
        } else if longest_mm <= 100.0 {
            1_200
        } else {
            2_000
        };
        let k = match quality {
            Quality::Draft => 0.4,
            Quality::Normal => 1.0,
            Quality::Detailed => 2.0,
        };
        MeshBudget {
            max_triangles: ((f64::from(base) * k) as u32).max(Self::MIN),
        }
    }
}

/// Triangles of the second level of detail.
pub const COARSE_TRIANGLES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    Draft,
    #[default]
    Normal,
    Detailed,
}

/// Chord tolerance for tessellation from the model's size.
pub fn chord_tolerance(longest_mm: f64) -> f64 {
    (longest_mm * 0.004).clamp(0.05, 0.5)
}

fn bounds(pos: &[V3]) -> (V3, V3) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in pos {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

/// Which model axis becomes width, height and depth.
pub fn assign_axes(extents: V3, target: Option<[f64; 3]>) -> ([usize; 3], Orientation) {
    const PERMS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    if let Some(t) = target.filter(|t| t.iter().all(|v| v.is_finite() && *v > 0.0)) {
        let cost = |p: &[usize; 3]| -> f64 {
            (0..3)
                .map(|i| (extents[p[i]].max(1e-6) / t[i]).ln().abs())
                .sum()
        };
        let best = PERMS
            .iter()
            .min_by(|a, b| cost(a).total_cmp(&cost(b)))
            .copied()
            .unwrap_or([0, 1, 2]);
        return (best, Orientation::MatchedToCatalog);
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| extents[a].total_cmp(&extents[b]));
    // smallest = depth, middle = width, largest = height
    ([order[1], order[2], order[0]], Orientation::Guessed)
}

fn parity_odd(p: [usize; 3]) -> bool {
    let mut inv = 0;
    for i in 0..3 {
        for j in i + 1..3 {
            if p[i] > p[j] {
                inv += 1;
            }
        }
    }
    inv % 2 == 1
}

/// Turns model-space positions into part space: axes assigned, x and y
/// centred, back at z = 0. Triangles keep their winding (a mirroring
/// permutation is made proper by negating x).
pub fn to_part_space(pos: &mut [V3], perm: [usize; 3]) {
    let mirror = parity_odd(perm);
    for p in pos.iter_mut() {
        let q = [p[perm[0]], p[perm[1]], p[perm[2]]];
        *p = [if mirror { -q[0] } else { q[0] }, q[1], q[2]];
    }
    let (lo, hi) = bounds(pos);
    let shift = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]];
    for p in pos.iter_mut() {
        *p = sub(*p, shift);
    }
}

/// True when the +z end is the one with the larger flat face, which is
/// then taken to be the mounting side and turned to z = 0.
fn mounting_side_is_top(pos: &[V3], tris: &[[u32; 3]], depth: f64) -> bool {
    let band = depth * 0.04 + 1e-6;
    let (mut bottom, mut top) = (0.0f64, 0.0f64);
    for t in tris {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let area = len(n) / 2.0;
        if area < 1e-12 {
            continue;
        }
        let nz = n[2] / (2.0 * area);
        if nz.abs() < 0.95 {
            continue;
        }
        let z = (a[2] + b[2] + c[2]) / 3.0;
        if nz < 0.0 && z < band {
            bottom += area;
        } else if nz > 0.0 && z > depth - band {
            top += area;
        }
    }
    top > bottom * 1.25
}

/// Rotates 180 degrees about y: x -> -x, z -> depth - z. Winding is kept.
pub fn flip_mounting_side(pos: &mut [V3], depth: f64) {
    for p in pos.iter_mut() {
        *p = [-p[0], p[1], depth - p[2]];
    }
}

/// From a world-space soup to the levels of detail: weld, axes, mounting
/// side, then simplify to `budget` (detailed), to 300 (coarse, when the
/// detailed level is bigger than that) and the 12-triangle box.
pub fn finish(
    soup: Soup,
    target: Option<[f64; 3]>,
    budget: MeshBudget,
) -> (Vec<MeshData>, Orientation, u32) {
    let (lo, hi) = bounds(&soup.pos);
    let ext = sub(hi, lo);
    let eps = (len(ext) * 1e-5).max(1e-4);
    let (mut pos, mut tris) = weld(&soup.pos, &soup.tris, eps);
    let (perm, orientation) = assign_axes(ext, target);
    to_part_space(&mut pos, perm);
    let (_, hi) = bounds(&pos);
    if mounting_side_is_top(&pos, &tris, hi[2]) {
        flip_mounting_side(&mut pos, hi[2]);
    }
    let before = tris.len() as u32;
    let mut lods = Vec::new();
    reduce(&mut pos, &mut tris, budget.max_triangles as usize);
    lods.push(with_normals(&pos, &tris, 40.0));
    if tris.len() > COARSE_TRIANGLES {
        reduce(&mut pos, &mut tris, COARSE_TRIANGLES);
        lods.push(with_normals(&pos, &tris, 40.0));
    }
    let bbox = lods[0].bounding_box();
    lods.push(bbox);
    (lods, orientation, before)
}

/// Simplifies to at most `target` triangles. Edge collapse stops on meshes
/// with cracks or non-manifold edges; the budget is a promise, so
/// clustering finishes the job when it fell short.
fn reduce(pos: &mut Vec<V3>, tris: &mut Vec<[u32; 3]>, target: usize) {
    simplify(pos, tris, target);
    if tris.len() > target + target / 8 {
        cluster_to(pos, tris, target);
    }
}

/// Indexed mesh with smooth normals except across creases sharper than
/// `crease_deg`; vertices on a crease are split. Feature edges are the
/// same creases and the open boundaries.
pub fn with_normals(pos: &[V3], tris: &[[u32; 3]], crease_deg: f64) -> MeshData {
    let cos_limit = crease_deg.to_radians().cos();
    let raw: Vec<V3> = tris
        .iter()
        .map(|t| {
            cross(
                sub(pos[t[1] as usize], pos[t[0] as usize]),
                sub(pos[t[2] as usize], pos[t[0] as usize]),
            )
        })
        .collect();
    let unit_n: Vec<V3> = raw.iter().map(|n| unit_or(*n, [0.0, 0.0, 1.0])).collect();
    let mut incident: Vec<Vec<u32>> = vec![Vec::new(); pos.len()];
    for (i, t) in tris.iter().enumerate() {
        for &v in t {
            incident[v as usize].push(i as u32);
        }
    }
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::with_capacity(tris.len() * 3);
    let mut seen: HashMap<(u32, [i16; 3]), u32> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for &v in t {
            let mut n = [0.0; 3];
            for &u in &incident[v as usize] {
                if dot(unit_n[i], unit_n[u as usize]) >= cos_limit {
                    n = add(n, raw[u as usize]);
                }
            }
            let n = unit_or(n, unit_n[i]);
            let key = (
                v,
                [
                    (n[0] * 1000.0).round() as i16,
                    (n[1] * 1000.0).round() as i16,
                    (n[2] * 1000.0).round() as i16,
                ],
            );
            let id = *seen.entry(key).or_insert_with(|| {
                let p = pos[v as usize];
                positions.push([p[0] as f32, p[1] as f32, p[2] as f32]);
                normals.push([n[0] as f32, n[1] as f32, n[2] as f32]);
                (positions.len() - 1) as u32
            });
            indices.push(id);
        }
    }
    let edges = mesh_edges(&positions, &indices, EDGE_CREASE_DEG);
    let bounds = Aabb::of(&positions);
    MeshData {
        positions,
        normals,
        indices,
        edges,
        bounds,
    }
}

/// Edges between triangles that meet at more than `crease_deg`, and edges
/// with only one (or more than two) triangles. Vertices are matched by
/// position, so a mesh whose vertices are split along creases still finds
/// its edges. Each edge is returned once.
pub fn mesh_edges(positions: &[[f32; 3]], indices: &[u32], crease_deg: f64) -> Vec<[u32; 2]> {
    let key = |p: [f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
    let mut weld: HashMap<[u32; 3], u32> = HashMap::new();
    let ids: Vec<u32> = positions
        .iter()
        .map(|&p| {
            let n = weld.len() as u32;
            *weld.entry(key(p)).or_insert(n)
        })
        .collect();
    let cos_limit = crease_deg.to_radians().cos();
    type EdgeUses = Vec<(usize, [u32; 2])>;
    let mut by_edge: HashMap<(u32, u32), EdgeUses> = HashMap::new();
    let mut normals: Vec<V3> = Vec::with_capacity(indices.len() / 3);
    for (ti, t) in indices.chunks_exact(3).enumerate() {
        let p = |i: usize| positions[t[i] as usize].map(f64::from);
        normals.push(unit_or(cross(sub(p(1), p(0)), sub(p(2), p(0))), [0.0; 3]));
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let (wa, wb) = (ids[a as usize], ids[b as usize]);
            if wa != wb {
                by_edge
                    .entry((wa.min(wb), wa.max(wb)))
                    .or_default()
                    .push((ti, [a, b]));
            }
        }
    }
    let mut out: Vec<[u32; 2]> = by_edge
        .values()
        .filter_map(|v| {
            let sharp = match v.len() {
                2 => dot(normals[v[0].0], normals[v[1].0]) < cos_limit,
                _ => true,
            };
            sharp.then_some(v[0].1)
        })
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_is_closed_outward_and_sized() {
        let g = box_mesh([10.0, 20.0, 30.0]);
        let m = g.detail();
        assert_eq!((m.positions.len(), m.triangle_count()), (24, 12));
        assert_eq!(g.size_mm(), [10.0, 20.0, 30.0]);
        assert_eq!(m.edges.len(), 12, "a box has twelve feature edges");
        let mut vol = 0.0f64;
        for t in m.indices.chunks_exact(3) {
            let p = |i: usize| m.positions[t[i] as usize].map(f64::from);
            let (a, b, c) = (p(0), p(1), p(2));
            // Signed volume of the tetrahedron to the origin.
            vol += dot(a, cross(b, c)) / 6.0;
            // Normal agrees with the stored one.
            let n = unit(cross(sub(b, a), sub(c, a)));
            let sn = m.normals[t[0] as usize].map(f64::from);
            assert!(dot(n, sn) > 0.99);
        }
        assert!((vol - 6000.0).abs() < 1e-3, "volume {vol}");
        assert!(m.positions.iter().all(|p| (0.0..=30.0).contains(&p[2])));
    }

    #[test]
    fn bad_sizes_become_a_one_millimetre_box() {
        let m = box_mesh([f64::NAN, -5.0, 0.0]);
        assert_eq!(m.size_mm(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn axes_follow_the_catalog_or_the_depth_guess() {
        // Model: x 40 (depth), y 10 (width), z 90 (height).
        let (p, o) = assign_axes([40.0, 10.0, 90.0], Some([10.0, 90.0, 40.0]));
        assert_eq!((p, o), ([1, 2, 0], Orientation::MatchedToCatalog));
        let (p, o) = assign_axes([40.0, 10.0, 90.0], None);
        // smallest (y) is depth, largest (z) is height, middle (x) is width.
        assert_eq!((p, o), ([0, 2, 1], Orientation::Guessed));
    }

    #[test]
    fn part_space_is_centred_with_the_back_at_zero() {
        let mut pos = vec![[10.0, 0.0, 5.0], [14.0, 20.0, 9.0], [10.0, 20.0, 9.0]];
        to_part_space(&mut pos, [0, 1, 2]);
        assert_eq!(pos[0], [-2.0, -10.0, 0.0]);
        assert_eq!(pos[1], [2.0, 10.0, 4.0]);
    }

    #[test]
    fn creases_split_normals_and_curves_stay_smooth() {
        // A cube's corners keep three normals each (24 vertices, 12 edges)...
        let m = box_data([2.0, 2.0, 2.0]);
        let pos: Vec<V3> = m.positions.iter().map(|p| p.map(f64::from)).collect();
        let tris: Vec<[u32; 3]> = m
            .indices
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        let (pos, tris) = weld(&pos, &tris, 1e-6);
        assert_eq!(pos.len(), 8);
        let cube = with_normals(&pos, &tris, 40.0);
        assert_eq!((cube.positions.len(), cube.edges.len()), (24, 12));
        // ...while a 24-gon prism wall is smooth: its side vertices are shared
        // and only the cap rims are edges.
        let n = 24usize;
        let mut p: Vec<V3> = Vec::new();
        for z in [0.0, 5.0] {
            for i in 0..n {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                p.push([3.0 * t.cos(), 3.0 * t.sin(), z]);
            }
        }
        let mut t: Vec<[u32; 3]> = Vec::new();
        for i in 0..n {
            let (a, b) = (i as u32, ((i + 1) % n) as u32);
            let (c, d) = (a + n as u32, b + n as u32);
            t.extend([[a, b, d], [a, d, c]]);
        }
        let wall = with_normals(&p, &t, 40.0);
        assert_eq!(wall.positions.len(), 2 * n);
        // Open top and bottom: the two rims are boundary edges; the smooth
        // wall has no creases.
        assert_eq!(wall.edges.len(), 2 * n);
    }

    #[test]
    fn a_model_fits_its_footprint_within_two_millimetres() {
        let g = box_mesh([10.0, 20.0, 30.0]);
        assert_eq!(fit_to_footprint(&g, [11.5, 20.0, 29.0]), Fit::Fits);
        assert_eq!(
            fit_to_footprint(&g, [10.0, 20.0, 35.0]),
            Fit::Mismatch { worst_mm: 5.0 }
        );
    }

    #[test]
    fn levels_end_in_the_box_of_the_bounds() {
        let g = box_mesh([10.0, 20.0, 30.0]);
        let b = g.detail().bounding_box();
        assert_eq!(b.triangle_count(), 12);
        assert_eq!(b.bounds, g.detail().bounds);
    }
}
