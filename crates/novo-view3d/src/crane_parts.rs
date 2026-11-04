//! Generic crane parts as meshes: flanged wheels, box girder with ribs, walkway with handrail,
//! runway beam and rail, rope drum, hook block with sheaves, forged hook, gear motor, gearbox,
//! trolley frame, end truck frame, buffer and control box. Every part is drawn from its real
//! dimensions in millimetres, so the picture has the proportions of the numbers; the details
//! (ribs, bolts, grooves) are typical of a workshop crane, not a maker's model.
//!
//! Each part is a [`Part`] with a [`GeometryKey`] that carries its kind and up to three sizes, so
//! the scene stays plain data and the view builds what a scene asks for from the scene alone
//! ([`ensure`]), as `device_shape` does for devices. Meshes are made once per key and shared.
//!
//! A part is modelled in its own *natural frame* (documented per variant). The mesh handed to the
//! renderer is moved so its box is centred in x and y and starts at z = 0 (the scene's local box);
//! [`item`] puts the part's natural origin at a world point, so callers never see the shift.

use crate::math::Aabb;
use crate::mesh::{GeometryKey, MeshData, MeshLibrary, PartGeometry};
use crate::scene::{Item, ItemId, ItemKind, Pose, Scene, Shape};
use glam::{Quat, Vec2, Vec3};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex, OnceLock};

/// Top byte of a crane part key. Maker keys are hashes, device shapes use `0xD5`.
const TAG: u64 = 0xC7;
const FIELD: u64 = 0xFFFF;

/// One crane part and its sizes, mm. The natural frame is given per variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// I-beam `h` high, `b` wide, `len` long. Along +z from 0, section centred in x and y.
    Beam { h: u16, b: u16, len: u16 },
    /// Crane rail, `len` long. Along +z from 0, foot bottom at y = 0, centred in x.
    Rail { len: u16 },
    /// Box girder `w` wide, `d` deep, `len` long, with outside stiffeners. Along +z from 0,
    /// bottom at y = 0, centred in x.
    BoxGirder { w: u16, d: u16, len: u16 },
    /// Walkway `w` wide and `len` long: deck top at y = 0, x from 0 (girder side) to `w`, along +z
    /// from 0, with toe plate, posts and two handrails on the open side.
    Walkway { w: u16, len: u16 },
    /// Flanged wheel of diameter `dia` and tread width `w`, axis z, centred on the origin.
    Wheel { dia: u16, w: u16 },
    /// Rope sheave of diameter `dia` and width `w`, axis z, centred on the origin.
    Sheave { dia: u16, w: u16 },
    /// Round bar of radius `r` along +z, 1000 long from the origin; stretch it with the item size.
    Bar { r: u16 },
    /// Rope drum of diameter `dia` and length `len`, axis z, centred on the origin.
    Drum { dia: u16, len: u16 },
    /// Hook block `w` (x) by `h` (y) by `d` (z): side plates, axles, crosshead, thrust bearing.
    /// Origin at the middle of the top, the block hangs down to y = -h. The sheaves are separate.
    Block { w: u16, h: u16, d: u16 },
    /// Forged hook with its shank and nut, `h` tall. Origin at the top of the shank, hangs down;
    /// the bowl opens towards +x.
    Hook { h: u16 },
    /// Induction motor of diameter `dia` and length `len`: fins, terminal box, fan cover. Axis z,
    /// origin at the middle of the drive end face, the motor runs to z = `len`.
    Motor { dia: u16, len: u16 },
    /// Gearbox `w` by `h` by `l`, centred on the origin, with an output boss on each side (x).
    Gearbox { w: u16, h: u16, l: u16 },
    /// Trolley frame `x` by `y` by `z`: two side members, two cross members and a deck.
    /// Origin at the middle of the bottom.
    Frame { x: u16, y: u16, z: u16 },
    /// End truck frame `len` along x: box beam above two axle boxes with cheeks, spaced `wheelbase`,
    /// for wheels of diameter `dia`. Origin on the rail top (the wheels' contact line), centred.
    Truck { len: u16, wheelbase: u16, dia: u16 },
    /// Buffer of diameter `dia`, `len` long: base plate, body and a pressure plate. Axis z from 0.
    Buffer { dia: u16, len: u16 },
    /// Control box `w` by `h` by `d`: door with frame and handle. Back at z = 0, bottom at y = 0.
    Panel { w: u16, h: u16, d: u16 },
}

impl Part {
    fn parts(self) -> (u64, [u16; 3]) {
        match self {
            Part::Beam { h, b, len } => (0, [h, b, len]),
            Part::Rail { len } => (1, [len, 0, 0]),
            Part::BoxGirder { w, d, len } => (2, [w, d, len]),
            Part::Walkway { w, len } => (3, [w, len, 0]),
            Part::Wheel { dia, w } => (4, [dia, w, 0]),
            Part::Sheave { dia, w } => (5, [dia, w, 0]),
            Part::Bar { r } => (6, [r, 0, 0]),
            Part::Drum { dia, len } => (7, [dia, len, 0]),
            Part::Block { w, h, d } => (8, [w, h, d]),
            Part::Hook { h } => (9, [h, 0, 0]),
            Part::Motor { dia, len } => (10, [dia, len, 0]),
            Part::Gearbox { w, h, l } => (11, [w, h, l]),
            Part::Frame { x, y, z } => (12, [x, y, z]),
            Part::Truck {
                len,
                wheelbase,
                dia,
            } => (13, [len, wheelbase, dia]),
            Part::Buffer { dia, len } => (14, [dia, len, 0]),
            Part::Panel { w, h, d } => (15, [w, h, d]),
        }
    }

    /// The key a scene item carries.
    pub fn key(self) -> GeometryKey {
        let (kind, [a, b, c]) = self.parts();
        GeometryKey(TAG << 56 | kind << 48 | (a as u64) << 32 | (b as u64) << 16 | c as u64)
    }

    /// The part a key stands for, if it is a crane part key.
    pub fn from_key(key: GeometryKey) -> Option<Part> {
        if key.0 >> 56 != TAG {
            return None;
        }
        let kind = (key.0 >> 48) & 0xFF;
        let a = ((key.0 >> 32) & FIELD) as u16;
        let b = ((key.0 >> 16) & FIELD) as u16;
        let c = (key.0 & FIELD) as u16;
        Some(match kind {
            0 => Part::Beam { h: a, b, len: c },
            1 => Part::Rail { len: a },
            2 => Part::BoxGirder { w: a, d: b, len: c },
            3 => Part::Walkway { w: a, len: b },
            4 => Part::Wheel { dia: a, w: b },
            5 => Part::Sheave { dia: a, w: b },
            6 => Part::Bar { r: a },
            7 => Part::Drum { dia: a, len: b },
            8 => Part::Block { w: a, h: b, d: c },
            9 => Part::Hook { h: a },
            10 => Part::Motor { dia: a, len: b },
            11 => Part::Gearbox { w: a, h: b, l: c },
            12 => Part::Frame { x: a, y: b, z: c },
            13 => Part::Truck {
                len: a,
                wheelbase: b,
                dia: c,
            },
            14 => Part::Buffer { dia: a, len: b },
            15 => Part::Panel { w: a, h: b, d: c },
            _ => return None,
        })
    }
}

/// Millimetres to the `u16` a key holds, held inside its range.
pub fn mm(v: f32) -> u16 {
    v.round().clamp(1.0, 65535.0) as u16
}

/// What a part is as the view needs it: its mesh in the scene's local frame, the size of that
/// frame's box and the shift from the natural frame into it.
#[derive(Debug)]
pub struct Entry {
    pub geometry: PartGeometry,
    pub size: Vec3,
    /// Local = natural + shift.
    pub shift: Vec3,
}

fn cache() -> &'static Mutex<HashMap<u64, Arc<Entry>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<Entry>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The entry for a part, made on first use.
pub fn entry(part: Part) -> Arc<Entry> {
    let key = part.key().0;
    if let Some(e) = cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return e;
    }
    let e = Arc::new(make(part));
    if let Ok(mut c) = cache().lock() {
        c.insert(key, e.clone());
    }
    e
}

/// A scene item for `part`, its natural origin at `anchor` turned by `rot`.
pub fn item(raw: u64, kind: ItemKind, part: Part, anchor: Vec3, rot: Quat, tint: [u8; 3]) -> Item {
    let e = entry(part);
    stretched(raw, kind, part, anchor, rot, tint, e.size)
}

/// As [`item`] with the local box set to `size`: the mesh is stretched to it (used for bars and
/// cylinders whose cross section does not change along the stretched axis).
pub fn stretched(
    raw: u64,
    kind: ItemKind,
    part: Part,
    anchor: Vec3,
    rot: Quat,
    tint: [u8; 3],
    size: Vec3,
) -> Item {
    let e = entry(part);
    let mut it = Item::new(
        ItemId::new(kind, raw),
        Shape::Part(part.key()),
        Pose {
            translation: anchor - rot * e.shift,
            rotation: rot,
        },
        size,
    );
    it.tint = Some(tint);
    it
}

/// Builds into `library` the crane parts `scene` asks for and it lacks. Returns whether it added any.
pub fn ensure(library: &mut MeshLibrary, scene: &Scene) -> bool {
    let mut added = false;
    for it in &scene.items {
        if let Shape::Part(k) = it.shape
            && library.get(k).is_none()
            && let Some(p) = Part::from_key(k)
        {
            library.insert(k, entry(p).geometry.clone());
            added = true;
        }
    }
    added
}

fn make(part: Part) -> Entry {
    let mut mb = Mb::new();
    build(part, &mut mb);
    let mut mesh = mb.0;
    let mut b = Aabb::EMPTY;
    for p in &mesh.positions {
        b.extend(Vec3::from_array(*p));
    }
    let centre = (b.min + b.max) * 0.5;
    let shift = Vec3::new(-centre.x, -centre.y, -b.min.z);
    for p in &mut mesh.positions {
        p[0] += shift.x;
        p[1] += shift.y;
        p[2] += shift.z;
    }
    let size = b.max - b.min;
    let lo = Vec3::new(-size.x * 0.5, -size.y * 0.5, 0.0);
    mesh.bounds = Aabb::new(lo, lo + size);
    // The far level is the plain box of the same extent.
    let mut plain = MeshData::from_boxes(&[(lo, lo + size)]);
    plain.bounds = mesh.bounds;
    Entry {
        geometry: PartGeometry {
            lods: vec![mesh, plain],
        },
        size,
        shift,
    }
}

// ---------------------------------------------------------------------------------------------
// Mesh building blocks

/// Which axis a shape runs along: maps a (u, v, w) frame to x, y, z by a cyclic permutation, so
/// the handedness (and the winding) is kept. Along Z, (u, v) is (x, y); along X it is (y, z);
/// along Y it is (z, x).
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

fn map(axis: Axis, u: f32, v: f32, w: f32) -> Vec3 {
    match axis {
        Axis::Z => Vec3::new(u, v, w),
        Axis::X => Vec3::new(w, u, v),
        Axis::Y => Vec3::new(v, w, u),
    }
}

struct Mb(MeshData);

impl Mb {
    fn new() -> Self {
        Mb(MeshData {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            edges: Vec::new(),
            bounds: Aabb::EMPTY,
        })
    }

    fn vert(&mut self, p: Vec3, n: Vec3) -> u32 {
        self.0.positions.push(p.to_array());
        self.0.normals.push(n.to_array());
        self.0.positions.len() as u32 - 1
    }

    fn tri_idx(&mut self, a: u32, b: u32, c: u32) {
        self.0.indices.extend([a, b, c]);
    }

    fn cube(&mut self, lo: Vec3, hi: Vec3) {
        self.0.add_box(lo, hi);
    }

    /// A box from its centre and half extents.
    fn bx(&mut self, c: Vec3, h: Vec3) {
        self.cube(c - h, c + h);
    }

    /// A polygon (counter-clockwise in u, v) extruded along `axis` from `w0` to `w1`, flat shaded.
    fn prism(&mut self, axis: Axis, poly: &[Vec2], w0: f32, w1: f32) {
        let mut poly = poly.to_vec();
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        let n = poly.len();
        let nrm = |u: f32, v: f32, w: f32| {
            // A direction along w, mapped.
            map(axis, u, v, w)
        };
        // Sides.
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let d = b - a;
            let out = Vec2::new(d.y, -d.x).normalize_or_zero();
            let normal = nrm(out.x, out.y, 0.0);
            let p = [
                map(axis, a.x, a.y, w0),
                map(axis, b.x, b.y, w0),
                map(axis, b.x, b.y, w1),
                map(axis, a.x, a.y, w1),
            ];
            let base = self.vert(p[0], normal);
            self.vert(p[1], normal);
            self.vert(p[2], normal);
            self.vert(p[3], normal);
            self.tri_idx(base, base + 1, base + 2);
            self.tri_idx(base, base + 2, base + 3);
        }
        // Caps: the w1 cap faces +w, the w0 cap faces -w.
        let tris = triangulate(&poly);
        for (w, sign) in [(w0, -1.0f32), (w1, 1.0)] {
            let normal = map(axis, 0.0, 0.0, sign);
            let ring: Vec<u32> = poly
                .iter()
                .map(|p| self.vert(map(axis, p.x, p.y, w), normal))
                .collect();
            for t in &tris {
                if sign > 0.0 {
                    self.tri_idx(ring[t[0]], ring[t[1]], ring[t[2]]);
                } else {
                    self.tri_idx(ring[t[0]], ring[t[2]], ring[t[1]]);
                }
            }
            for i in 0..n {
                self.0.edges.push([ring[i], ring[(i + 1) % n]]);
            }
        }
    }

    /// A smooth cylinder (or frustum) along `axis` about (u0, v0), radii `r0` at `w0` and `r1` at
    /// `w1`, with flat end caps when asked.
    #[allow(clippy::too_many_arguments)]
    fn frustum(
        &mut self,
        axis: Axis,
        u0: f32,
        v0: f32,
        r0: f32,
        r1: f32,
        w0: f32,
        w1: f32,
        segs: usize,
        caps: bool,
    ) {
        let (r0, r1, w0, w1) = if w1 < w0 {
            (r1, r0, w1, w0)
        } else {
            (r0, r1, w0, w1)
        };
        let slope = if (w1 - w0).abs() > 1e-6 {
            (r0 - r1) / (w1 - w0)
        } else {
            0.0
        };
        let mut rings = [Vec::new(), Vec::new()];
        for s in 0..=segs {
            let a = TAU * s as f32 / segs as f32;
            let (c, sn) = (a.cos(), a.sin());
            let normal = map(axis, c, sn, slope).normalize();
            for (k, (r, w)) in [(r0, w0), (r1, w1)].into_iter().enumerate() {
                let i = self.vert(map(axis, u0 + r * c, v0 + r * sn, w), normal);
                rings[k].push(i);
            }
        }
        for s in 0..segs {
            let (a0, a1) = (rings[0][s], rings[0][s + 1]);
            let (b0, b1) = (rings[1][s], rings[1][s + 1]);
            self.tri_idx(a0, a1, b1);
            self.tri_idx(a0, b1, b0);
        }
        if caps {
            for (w, r, sign) in [(w0, r0, -1.0f32), (w1, r1, 1.0)] {
                if r < 1e-3 {
                    continue;
                }
                let normal = map(axis, 0.0, 0.0, sign);
                let c = self.vert(map(axis, u0, v0, w), normal);
                let ring: Vec<u32> = (0..segs)
                    .map(|s| {
                        let a = TAU * s as f32 / segs as f32;
                        self.vert(map(axis, u0 + r * a.cos(), v0 + r * a.sin(), w), normal)
                    })
                    .collect();
                for s in 0..segs {
                    let (p, q) = (ring[s], ring[(s + 1) % segs]);
                    if sign > 0.0 {
                        self.tri_idx(c, p, q);
                    } else {
                        self.tri_idx(c, q, p);
                    }
                    self.0.edges.push([p, q]);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn cyl(&mut self, axis: Axis, u0: f32, v0: f32, r: f32, w0: f32, w1: f32, segs: usize) {
        self.frustum(axis, u0, v0, r, r, w0, w1, segs, true);
    }

    /// Revolves a profile of (radius, w) points about the axis through (u0, v0). The profile runs
    /// from the axis round the outside and back to the axis, so the outside is on its right when
    /// seen with r to the right and w up. Shading is smooth round the axis and crisp along it.
    fn revolve(&mut self, axis: Axis, u0: f32, v0: f32, profile: &[(f32, f32)], segs: usize) {
        for pair in profile.windows(2) {
            let ((r0, w0), (r1, w1)) = (pair[0], pair[1]);
            if (r0 - r1).abs() < 1e-6 && (w0 - w1).abs() < 1e-6 {
                continue;
            }
            // Outward normal in the (r, w) plane for a profile with the outside on its right.
            let (dr, dw) = (r1 - r0, w1 - w0);
            let (nr, nw) = (dw, -dr);
            let len = (nr * nr + nw * nw).sqrt();
            let (nr, nw) = (nr / len, nw / len);
            let mut ring = [Vec::new(), Vec::new()];
            for s in 0..=segs {
                let a = TAU * s as f32 / segs as f32;
                let (c, sn) = (a.cos(), a.sin());
                let normal = map(axis, nr * c, nr * sn, nw);
                for (k, (r, w)) in [(r0, w0), (r1, w1)].into_iter().enumerate() {
                    let i = self.vert(map(axis, u0 + r * c, v0 + r * sn, w), normal);
                    ring[k].push(i);
                }
            }
            for s in 0..segs {
                let (a0, a1) = (ring[0][s], ring[0][s + 1]);
                let (b0, b1) = (ring[1][s], ring[1][s + 1]);
                // Degenerate (pole) triangles are skipped: a ring at r = 0 collapses to a point.
                if r0 > 1e-4 {
                    self.tri_idx(a0, a1, b1);
                }
                if r1 > 1e-4 {
                    self.tri_idx(a0, b1, b0);
                }
            }
            // Outline rings at corners: only where the profile turns sharply.
            if r1 > 1e-4 && (r0 - r1).abs() > 1e-4 {
                for s in 0..segs {
                    self.0.edges.push([ring[1][s], ring[1][s + 1]]);
                }
            }
        }
    }

    /// A tube of circular section swept along a planar path in the xy plane: `(point, radius)`.
    fn tube_xy(&mut self, path: &[(Vec2, f32)], segs: usize) {
        let n = path.len();
        let mut rings: Vec<Vec<u32>> = Vec::new();
        for i in 0..n {
            let prev = path[i.saturating_sub(1)].0;
            let next = path[(i + 1).min(n - 1)].0;
            let t = (next - prev).normalize_or_zero();
            let nn = Vec2::new(-t.y, t.x);
            let (c, r) = path[i];
            let mut ring = Vec::new();
            for s in 0..=segs {
                let a = TAU * s as f32 / segs as f32;
                let dir = Vec3::new(nn.x * a.cos(), nn.y * a.cos(), a.sin());
                ring.push(self.vert(Vec3::new(c.x, c.y, 0.0) + dir * r, dir));
            }
            rings.push(ring);
        }
        for i in 0..n - 1 {
            for s in 0..segs {
                let (a0, a1) = (rings[i][s], rings[i][s + 1]);
                let (b0, b1) = (rings[i + 1][s], rings[i + 1][s + 1]);
                self.tri_idx(a0, a1, b1);
                self.tri_idx(a0, b1, b0);
            }
        }
    }
}

fn area(poly: &[Vec2]) -> f32 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5
}

/// Ear clipping of a simple counter-clockwise polygon: index triples.
fn triangulate(poly: &[Vec2]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::new();
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (i0, i1, i2) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[i0], poly[i1], poly[i2]);
            if cross(a, b, c) <= 1e-6 {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == i0 || j == i1 || j == i2 {
                    return false;
                }
                let p = poly[j];
                cross(a, b, p) > -1e-6 && cross(b, c, p) > -1e-6 && cross(c, a, p) > -1e-6
            });
            if inside {
                continue;
            }
            out.push([i0, i1, i2]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            // A degenerate polygon: fan out what is left rather than loop.
            break;
        }
    }
    if idx.len() >= 3 {
        for k in 1..idx.len() - 1 {
            out.push([idx[0], idx[k], idx[k + 1]]);
        }
    }
    out
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

// ---------------------------------------------------------------------------------------------
// The parts

fn build(part: Part, m: &mut Mb) {
    let f = |v: u16| v as f32;
    match part {
        Part::Beam { h, b, len } => i_beam(m, f(h), f(b), f(len)),
        Part::Rail { len } => rail(m, f(len)),
        Part::BoxGirder { w, d, len } => box_girder(m, f(w), f(d), f(len)),
        Part::Walkway { w, len } => walkway(m, f(w), f(len)),
        Part::Wheel { dia, w } => wheel(m, f(dia), f(w)),
        Part::Sheave { dia, w } => sheave(m, f(dia), f(w)),
        Part::Bar { r } => m.cyl(Axis::Z, 0.0, 0.0, f(r), 0.0, 1000.0, 10),
        Part::Drum { dia, len } => drum(m, f(dia), f(len)),
        Part::Block { w, h, d } => block(m, f(w), f(h), f(d)),
        Part::Hook { h } => hook(m, f(h)),
        Part::Motor { dia, len } => motor(m, f(dia), f(len)),
        Part::Gearbox { w, h, l } => gearbox(m, f(w), f(h), f(l)),
        Part::Frame { x, y, z } => frame(m, f(x), f(y), f(z)),
        Part::Truck {
            len,
            wheelbase,
            dia,
        } => truck(m, f(len), f(wheelbase), f(dia)),
        Part::Buffer { dia, len } => buffer(m, f(dia), f(len)),
        Part::Panel { w, h, d } => panel(m, f(w), f(h), f(d)),
    }
}

fn i_beam(m: &mut Mb, h: f32, b: f32, len: f32) {
    let (tf, tw) = ((h / 18.0).max(4.0), (h / 36.0).max(3.0));
    let (hb, hh, hw) = (b * 0.5, h * 0.5, tw * 0.5);
    let poly = [
        v2(-hb, -hh),
        v2(hb, -hh),
        v2(hb, -hh + tf),
        v2(hw, -hh + tf),
        v2(hw, hh - tf),
        v2(hb, hh - tf),
        v2(hb, hh),
        v2(-hb, hh),
        v2(-hb, hh - tf),
        v2(-hw, hh - tf),
        v2(-hw, -hh + tf),
        v2(-hb, -hh + tf),
    ];
    m.prism(Axis::Z, &poly, 0.0, len);
}

fn rail(m: &mut Mb, len: f32) {
    // A crane rail: foot, web and a head with chamfered corners; 70 mm high.
    let poly = [
        v2(-50.0, 0.0),
        v2(50.0, 0.0),
        v2(50.0, 12.0),
        v2(14.0, 18.0),
        v2(11.0, 42.0),
        v2(30.0, 48.0),
        v2(30.0, 66.0),
        v2(24.0, 70.0),
        v2(-24.0, 70.0),
        v2(-30.0, 66.0),
        v2(-30.0, 48.0),
        v2(-11.0, 42.0),
        v2(-14.0, 18.0),
        v2(-50.0, 12.0),
    ];
    m.prism(Axis::Z, &poly, 0.0, len);
    // Clips on the foot every 700 mm, from the first half pitch.
    let mut z = 350.0;
    while z < len - 100.0 {
        m.bx(Vec3::new(0.0, 14.0, z), Vec3::new(62.0, 14.0, 22.0));
        z += 700.0;
    }
}

fn box_girder(m: &mut Mb, w: f32, d: f32, len: f32) {
    let t = 14.0;
    // Webs and plates: the box proper, with the top and bottom plates a little proud of the webs.
    m.cube(Vec3::new(-w * 0.5, 0.0, 0.0), Vec3::new(w * 0.5, t, len));
    m.cube(Vec3::new(-w * 0.5, d - t, 0.0), Vec3::new(w * 0.5, d, len));
    let web = 10.0;
    for s in [-1.0f32, 1.0] {
        let x = s * (w * 0.5 - 28.0 - web * 0.5);
        m.bx(
            Vec3::new(x, d * 0.5, len * 0.5),
            Vec3::new(web * 0.5, d * 0.5 - t, len * 0.5),
        );
    }
    // Outside stiffeners (flat bars) every 1500 mm, as on a welded box girder.
    let n = ((len / 1500.0).floor() as usize).max(1);
    for k in 0..=n {
        let z = len * k as f32 / n as f32;
        let z = z.clamp(20.0, len - 20.0);
        for s in [-1.0f32, 1.0] {
            let x = s * (w * 0.5 - 28.0 + 8.0);
            m.bx(
                Vec3::new(x, d * 0.5, z),
                Vec3::new(14.0, d * 0.5 - t - 20.0, 4.0),
            );
        }
    }
    // End plates.
    for z in [0.0, len - 12.0] {
        m.cube(
            Vec3::new(-w * 0.5 + 28.0, t, z),
            Vec3::new(w * 0.5 - 28.0, d - t, z + 12.0),
        );
    }
}

fn walkway(m: &mut Mb, w: f32, len: f32) {
    // Deck: grating drawn as a deck plate with a row of cross bars standing proud.
    m.cube(Vec3::new(0.0, -30.0, 0.0), Vec3::new(w, 0.0, len));
    let mut z = 60.0;
    while z < len - 30.0 {
        m.cube(
            Vec3::new(8.0, 0.0, z - 3.0),
            Vec3::new(w - 6.0, 4.0, z + 3.0),
        );
        z += 60.0;
    }
    // Toe plate along the outer edge.
    m.cube(Vec3::new(w - 6.0, 0.0, 0.0), Vec3::new(w, 100.0, len));
    // Brackets under the deck every 1500 mm: a triangular gusset.
    let n = ((len / 1500.0).floor() as usize).max(1);
    for k in 0..=n {
        let z = (len * k as f32 / n as f32).clamp(20.0, len - 20.0);
        let tri = [
            v2(0.0, -30.0),
            v2(w * 0.9, -30.0),
            v2(0.0, -30.0 - w * 0.55),
        ];
        m.prism(Axis::Z, &tri, z - 5.0, z + 5.0);
    }
    // Handrail: posts of 33.7 mm tube every 1500 mm, a top rail at 1100 and a mid rail at 550.
    let r = 16.85;
    for k in 0..=n {
        let z = (len * k as f32 / n as f32).clamp(r, len - r);
        m.cyl(Axis::Y, z, w - 20.0, r, 0.0, 1100.0, 10);
    }
    for h in [1100.0 - r, 550.0] {
        m.cyl(Axis::Z, w - 20.0, h, r, 0.0, len, 10);
    }
}

fn wheel(m: &mut Mb, dia: f32, w: f32) {
    let (r, h) = (dia * 0.5, w * 0.5);
    let fl = (dia * 0.045).max(8.0); // flange height
    let ft = (w * 0.2).max(6.0); // flange thickness
    let hub = r * 0.32;
    let web = r * 0.92;
    // Tread with a flange on each side, a web and a hub, the outside on the left of the profile.
    let profile = [
        (0.0, -h),
        (hub, -h),
        (hub, -h * 0.55),
        (web, -h + ft),
        (r + fl, -h + ft * 0.5),
        (r + fl, -h + ft),
        (r, -h + ft * 1.6),
        (r, h - ft * 1.6),
        (r + fl, h - ft),
        (r + fl, h - ft * 0.5),
        (web, h - ft),
        (hub, h * 0.55),
        (hub, h),
        (0.0, h),
    ];
    // Rim and flanges are bright enough to read as a wheel; a pad pattern shows the turning.
    m.revolve(Axis::Z, 0.0, 0.0, &profile, 40);
    for k in 0..6 {
        let a = TAU * k as f32 / 6.0;
        let rr = (hub + web) * 0.5;
        m.cyl(
            Axis::Z,
            rr * a.cos(),
            rr * a.sin(),
            r * 0.075,
            h - ft * 0.9,
            h + 2.0,
            8,
        );
    }
    // Axle stub.
    m.cyl(
        Axis::Z,
        0.0,
        0.0,
        hub * 0.55,
        h,
        h + (w * 0.4).max(20.0),
        16,
    );
}

fn sheave(m: &mut Mb, dia: f32, w: f32) {
    let (r, h) = (dia * 0.5, w * 0.5);
    let groove = (w * 0.28).max(4.0);
    let profile = [
        (0.0, -h),
        (r * 0.18, -h),
        (r * 0.18, -h * 0.35),
        (r * 0.82, -h * 0.5),
        (r - groove, -h),
        (r, -h * 0.35),
        (r - groove * 0.9, 0.0),
        (r, h * 0.35),
        (r - groove, h),
        (r * 0.82, h * 0.5),
        (r * 0.18, h * 0.35),
        (r * 0.18, h),
        (0.0, h),
    ];
    m.revolve(Axis::Z, 0.0, 0.0, &profile, 36);
    // A pad pattern so the turning shows.
    for k in 0..5 {
        let a = TAU * k as f32 / 5.0;
        let rr = r * 0.5;
        m.cyl(
            Axis::Z,
            rr * a.cos(),
            rr * a.sin(),
            r * 0.07,
            h - 2.0,
            h + 2.0,
            8,
        );
    }
}

fn drum(m: &mut Mb, dia: f32, len: f32) {
    let (r, h) = (dia * 0.5, len * 0.5);
    let fl = r * 0.18;
    let ft = (len * 0.025).max(8.0);
    // Core with a rope groove every 40 mm (cut as a shallow V) and a flange on each end.
    let mut p: Vec<(f32, f32)> = vec![(0.0, -h), (r + fl, -h), (r + fl, -h + ft), (r, -h + ft)];
    let pitch = 40.0f32;
    let n = (((len - 2.0 * ft) / pitch).floor() as usize).clamp(2, 24);
    let step = (len - 2.0 * ft) / n as f32;
    let gd = (r * 0.03).max(2.0);
    for k in 0..n {
        let z0 = -h + ft + step * k as f32;
        p.push((r - gd, z0 + step * 0.5));
        p.push((r, z0 + step));
    }
    p.extend([(r, h - ft), (r + fl, h - ft), (r + fl, h), (0.0, h)]);
    m.revolve(Axis::Z, 0.0, 0.0, &p, 40);
    // Shaft stubs and a pad ring on one flange so the turning shows.
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.22, h, h + r * 0.5, 16);
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.22, -h - r * 0.5, -h, 16);
    for k in 0..8 {
        let a = TAU * k as f32 / 8.0;
        let rr = r * 0.75;
        m.cyl(
            Axis::Z,
            rr * a.cos(),
            rr * a.sin(),
            r * 0.04,
            -h - 6.0,
            -h,
            8,
        );
    }
}

fn block(m: &mut Mb, w: f32, h: f32, d: f32) {
    let plate = (w * 0.073).max(8.0);
    let (hx, hz) = (w * 0.5, d * 0.5);
    // Side plates: an octagon in (z, y), hanging from y = 0.
    let bottom = -h * 0.846;
    let poly = [
        v2(-hz * 0.72, 0.0),
        v2(hz * 0.72, 0.0),
        v2(hz, -h * 0.115),
        v2(hz, -h * 0.58),
        v2(hz * 0.52, bottom),
        v2(-hz * 0.52, bottom),
        v2(-hz, -h * 0.58),
        v2(-hz, -h * 0.115),
    ];
    for s in [-1.0f32, 1.0] {
        let (x0, x1) = if s < 0.0 {
            (-hx, -hx + plate)
        } else {
            (hx - plate, hx)
        };
        // Plate outline in (z, y) extruded along x.
        m.prism(Axis::X, &poly.map(|p| v2(p.y, p.x)), x0, x1);
    }
    // Axles of the sheaves and the crosshead through both plates.
    let sheave_y = -h * 0.31;
    for z in [-hz * 0.62, hz * 0.62] {
        m.cyl(Axis::X, sheave_y, z, w * 0.09, -hx, hx, 14);
    }
    m.cube(
        Vec3::new(-hx, -h * 0.95, -hz * 0.4),
        Vec3::new(hx, -h * 0.82, hz * 0.4),
    );
    // Rope guard bars between the plates above the sheaves.
    m.cube(
        Vec3::new(-hx + plate, -h * 0.06, -hz * 0.9),
        Vec3::new(hx - plate, -h * 0.03, hz * 0.9),
    );
    // Thrust bearing under the crosshead.
    m.cyl(Axis::Y, 0.0, 0.0, w * 0.2, -h, -h * 0.95, 20);
}

fn hook(m: &mut Mb, h: f32) {
    let s = h / 330.0;
    // Shank, thread nut and the bearing collar.
    m.cyl(Axis::Y, 0.0, 0.0, 26.0 * s, -150.0 * s, 0.0, 16);
    m.cyl(Axis::Y, 0.0, 0.0, 38.0 * s, -62.0 * s, -24.0 * s, 6);
    // The forged body: a J swept from the shank round the bowl up to the tip.
    let mut path: Vec<(Vec2, f32)> = vec![
        (v2(0.0, -140.0 * s), 27.0 * s),
        (v2(0.0, -190.0 * s), 30.0 * s),
    ];
    let c = v2(95.0 * s, -190.0 * s);
    let rb = 95.0 * s;
    let steps = 14;
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        let a = PI + PI * 1.18 * t;
        let r = (31.0 + 9.0 * (PI * t).sin() - 14.0 * t * t) * s;
        path.push((c + Vec2::new(a.cos(), a.sin()) * rb, r));
    }
    m.tube_xy(&path, 12);
    // Safety latch across the throat.
    let tip = path.last().map_or(v2(0.0, 0.0), |p| p.0);
    m.cyl(
        Axis::Z,
        tip.x - 8.0 * s,
        tip.y - 22.0 * s,
        4.0 * s,
        -6.0 * s,
        6.0 * s,
        6,
    );
}

fn motor(m: &mut Mb, dia: f32, len: f32) {
    let r = dia * 0.5;
    // Drive end shield, finned stator, fan cover and the shaft stub.
    m.cyl(Axis::Z, 0.0, 0.0, r * 1.02, 0.0, len * 0.1, 24);
    let body0 = len * 0.1;
    let body1 = len * 0.78;
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.93, body0, body1, 24);
    let fins = 9usize;
    for k in 0..fins {
        let z0 = body0 + (body1 - body0) * (k as f32 + 0.3) / fins as f32;
        let z1 = body0 + (body1 - body0) * (k as f32 + 0.7) / fins as f32;
        m.cyl(Axis::Z, 0.0, 0.0, r, z0, z1, 24);
    }
    // Fan cover (brake and fan at the back).
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.97, body1, len * 0.9, 24);
    m.frustum(
        Axis::Z,
        0.0,
        0.0,
        r * 0.97,
        r * 0.8,
        len * 0.9,
        len,
        24,
        true,
    );
    // Shaft stub with a key.
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.2, -len * 0.14, 0.0, 12);
    // Terminal box on top, feet below.
    m.bx(
        Vec3::new(0.0, r * 1.05, len * 0.42),
        Vec3::new(r * 0.5, r * 0.18, len * 0.14),
    );
    m.cube(
        Vec3::new(-r * 0.9, -r * 1.06, len * 0.14),
        Vec3::new(r * 0.9, -r * 0.88, len * 0.7),
    );
}

fn gearbox(m: &mut Mb, w: f32, h: f32, l: f32) {
    // A housing with chamfered corners, an output boss on each side and an oil plug.
    let (hx, hy, hz) = (w * 0.5, h * 0.5, l * 0.5);
    let c = (w.min(h) * 0.12).min(hz * 0.5);
    let poly = [
        v2(-hx + c, -hy),
        v2(hx - c, -hy),
        v2(hx, -hy + c),
        v2(hx, hy - c),
        v2(hx - c, hy),
        v2(-hx + c, hy),
        v2(-hx, hy - c),
        v2(-hx, -hy + c),
    ];
    m.prism(Axis::Z, &poly, -hz, hz);
    for s in [-1.0f32, 1.0] {
        m.cyl(Axis::X, 0.0, 0.0, h * 0.2, s * hx, s * (hx + w * 0.12), 20);
    }
    m.cyl(Axis::Y, hz * 0.4, 0.0, h * 0.05, hy, hy + h * 0.06, 8);
    m.cube(
        Vec3::new(-hx * 0.5, -hy - h * 0.1, -hz * 1.1),
        Vec3::new(hx * 0.5, -hy, hz * 1.1),
    );
}

fn frame(m: &mut Mb, x: f32, y: f32, z: f32) {
    // Natural frame: centred in x and z, bottom at y = 0.
    let (hx, hz) = (x * 0.5, z * 0.5);
    let beam = (y * 0.36).max(30.0);
    // Two side members along z, two cross members along x, a deck plate on top.
    let side_w = (x * 0.09).max(30.0);
    for s in [-1.0f32, 1.0] {
        m.cube(
            Vec3::new(s * hx * 0.86 - side_w * 0.5, 0.0, -hz),
            Vec3::new(s * hx * 0.86 + side_w * 0.5, beam, hz),
        );
    }
    for s in [-1.0f32, 1.0] {
        m.cube(
            Vec3::new(-hx, 0.0, s * hz * 0.82 - beam * 0.4),
            Vec3::new(hx, beam, s * hz * 0.82 + beam * 0.4),
        );
    }
    m.cube(Vec3::new(-hx, beam, -hz), Vec3::new(hx, beam + 12.0, hz));
    // A guard rail on the deck's edge.
    let rail_h = y;
    m.cube(
        Vec3::new(-hx, beam + 12.0, hz - 6.0),
        Vec3::new(hx, rail_h, hz),
    );
}

fn truck(m: &mut Mb, len: f32, wheelbase: f32, dia: f32) {
    // Natural frame: rail top at y = 0, so the wheel centres are at y = dia / 2.
    let r = dia * 0.5;
    let beam_lo = r + 90.0;
    let beam_hi = r + 330.0;
    let hz = 120.0;
    m.cube(
        Vec3::new(-len * 0.5, beam_lo, -hz),
        Vec3::new(len * 0.5, beam_hi, hz),
    );
    for k in 0..2 {
        let x = (k as f32 - 0.5) * wheelbase;
        // Two cheeks round the wheel, an axle box on the outside.
        let cheek = [
            v2(x - r * 0.4, r - 70.0),
            v2(x + r * 0.4, r - 70.0),
            v2(x + r * 0.55, r - 10.0),
            v2(x + r * 0.55, beam_lo + 6.0),
            v2(x - r * 0.55, beam_lo + 6.0),
            v2(x - r * 0.55, r - 10.0),
        ];
        for s in [-1.0f32, 1.0] {
            let (z0, z1) = if s < 0.0 {
                (-78.0, -62.0)
            } else {
                (62.0, 78.0)
            };
            m.prism(Axis::Z, &cheek, z0, z1);
        }
        for s in [-1.0f32, 1.0] {
            m.bx(
                Vec3::new(x, r, s * 94.0),
                Vec3::new(r * 0.36, r * 0.36, 17.0),
            );
        }
    }
    // End plates carrying the buffers.
    for s in [-1.0f32, 1.0] {
        m.cube(
            Vec3::new(s * len * 0.5 - 8.0, beam_lo - 10.0, -hz - 20.0),
            Vec3::new(s * len * 0.5 + 8.0, beam_hi + 10.0, hz + 20.0),
        );
    }
}

fn buffer(m: &mut Mb, dia: f32, len: f32) {
    let r = dia * 0.5;
    m.cube(
        Vec3::new(-r * 1.3, -r * 1.3, 0.0),
        Vec3::new(r * 1.3, r * 1.3, len * 0.12),
    );
    m.cyl(Axis::Z, 0.0, 0.0, r * 0.8, len * 0.12, len * 0.8, 20);
    m.cyl(Axis::Z, 0.0, 0.0, r, len * 0.8, len, 24);
}

fn panel(m: &mut Mb, w: f32, h: f32, d: f32) {
    // Body, a door frame standing proud, a hinge side and a handle.
    m.cube(
        Vec3::new(-w * 0.5, 0.0, 0.0),
        Vec3::new(w * 0.5, h, d * 0.9),
    );
    let t = 6.0;
    let (fw, fh) = (w * 0.5 - 12.0, h - 24.0);
    for (c, hx, hy) in [
        (Vec3::new(0.0, 12.0 + t * 0.5, d * 0.95), fw),
        (Vec3::new(0.0, h - 12.0 - t * 0.5, d * 0.95), fw),
    ]
    .map(|(c, hx)| (c, hx, t * 0.5))
    {
        m.bx(c, Vec3::new(hx, hy, d * 0.05));
    }
    for s in [-1.0f32, 1.0] {
        m.bx(
            Vec3::new(s * (fw - t * 0.5), h * 0.5, d * 0.95),
            Vec3::new(t * 0.5, fh * 0.5, d * 0.05),
        );
    }
    m.bx(
        Vec3::new(w * 0.32, h * 0.5, d),
        Vec3::new(8.0, h * 0.06, d * 0.04),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<Part> {
        vec![
            Part::Beam {
                h: 400,
                b: 300,
                len: 3000,
            },
            Part::Rail { len: 3000 },
            Part::BoxGirder {
                w: 560,
                d: 800,
                len: 10000,
            },
            Part::Walkway { w: 700, len: 10000 },
            Part::Wheel { dia: 400, w: 100 },
            Part::Wheel { dia: 200, w: 70 },
            Part::Sheave { dia: 250, w: 70 },
            Part::Bar { r: 12 },
            Part::Drum { dia: 500, len: 900 },
            Part::Block {
                w: 300,
                h: 520,
                d: 420,
            },
            Part::Hook { h: 330 },
            Part::Motor { dia: 250, len: 500 },
            Part::Gearbox {
                w: 300,
                h: 300,
                l: 250,
            },
            Part::Frame {
                x: 1250,
                y: 500,
                z: 1100,
            },
            Part::Truck {
                len: 2400,
                wheelbase: 1800,
                dia: 400,
            },
            Part::Buffer { dia: 100, len: 160 },
            Part::Panel {
                w: 400,
                h: 500,
                d: 200,
            },
        ]
    }

    #[test]
    fn keys_round_trip_and_do_not_clash_with_device_shapes() {
        for p in samples() {
            assert_eq!(Part::from_key(p.key()), Some(p), "{p:?}");
            assert!(crate::device_shape::spec(p.key()).is_none());
        }
        assert_eq!(Part::from_key(GeometryKey(42)), None);
    }

    #[test]
    fn every_part_has_triangles_a_centred_local_box_and_a_far_level() {
        for p in samples() {
            let e = entry(p);
            let lod = &e.geometry.lods[0];
            assert!(lod.triangles() > 10, "{p:?} has no mesh");
            assert_eq!(e.geometry.lods.len(), 2);
            let b = lod.bounds;
            assert!((b.min.x + b.max.x).abs() < 1e-2, "{p:?} not centred in x");
            assert!((b.min.y + b.max.y).abs() < 1e-2, "{p:?} not centred in y");
            assert!(b.min.z.abs() < 1e-2, "{p:?} does not start at z = 0");
            for q in &lod.positions {
                assert!(
                    q.iter().all(|v| v.is_finite()),
                    "{p:?} has a non-finite vertex"
                );
            }
            // The mesh really fills its box.
            let mut got = Aabb::EMPTY;
            for q in &lod.positions {
                got.extend(Vec3::from_array(*q));
            }
            assert!((got.max - b.max).length() < 1e-2 && (got.min - b.min).length() < 1e-2);
        }
    }

    #[test]
    fn faces_agree_with_their_normals_so_no_part_is_inside_out() {
        for p in samples() {
            let m = &entry(p).geometry.lods[0];
            let mut bad = 0;
            let mut total = 0;
            for t in m.indices.chunks(3) {
                let [a, b, c] =
                    [t[0], t[1], t[2]].map(|i| Vec3::from_array(m.positions[i as usize]));
                let face = (b - a).cross(c - a);
                if face.length() < 1e-4 {
                    continue;
                }
                let n = (Vec3::from_array(m.normals[t[0] as usize])
                    + Vec3::from_array(m.normals[t[1] as usize])
                    + Vec3::from_array(m.normals[t[2] as usize]))
                    / 3.0;
                total += 1;
                if face.normalize().dot(n.normalize_or_zero()) < 0.0 {
                    bad += 1;
                }
            }
            assert!(total > 0);
            assert_eq!(
                bad, 0,
                "{p:?}: {bad} of {total} faces point against their normal"
            );
        }
    }

    #[test]
    fn indices_stay_inside_the_vertex_lists() {
        for p in samples() {
            let m = &entry(p).geometry.lods[0];
            assert_eq!(m.positions.len(), m.normals.len());
            assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
            assert!(
                m.edges
                    .iter()
                    .all(|e| e.iter().all(|&i| (i as usize) < m.positions.len()))
            );
        }
    }

    #[test]
    fn an_item_puts_the_natural_origin_at_the_anchor() {
        // The block hangs from its origin at the middle of the top: its world box ends there.
        let part = Part::Block {
            w: 300,
            h: 520,
            d: 420,
        };
        let anchor = Vec3::new(1000.0, 4000.0, 2000.0);
        let scene = Scene::new(crate::scene::PlateFrame {
            width: 1.0,
            height: 1.0,
        });
        let it = item(1, ItemKind::Other, part, anchor, Quat::IDENTITY, [1, 2, 3]);
        let b = scene.world_bounds(&it);
        assert!((b.max.y - 4000.0).abs() < 1e-2, "{b:?}");
        assert!((b.min.y - 3480.0).abs() < 1e-2);
        assert!(((b.min.x + b.max.x) * 0.5 - 1000.0).abs() < 1e-2);
        assert!(((b.min.z + b.max.z) * 0.5 - 2000.0).abs() < 1e-2);
    }

    #[test]
    fn the_library_gets_only_what_a_scene_asks_for_and_lacks() {
        let mut scene = Scene::new(crate::scene::PlateFrame {
            width: 1.0,
            height: 1.0,
        });
        scene.push(item(
            1,
            ItemKind::Other,
            Part::Wheel { dia: 400, w: 100 },
            Vec3::ZERO,
            Quat::IDENTITY,
            [1, 2, 3],
        ));
        let mut lib = MeshLibrary::new();
        assert!(ensure(&mut lib, &scene));
        assert_eq!(lib.len(), 1);
        assert!(!ensure(&mut lib, &scene));
    }
}
