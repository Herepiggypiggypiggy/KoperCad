//! Mesh-level collision and clearance for cabinet parts (panel layout lane B).
//!
//! Pure geometry: triangle meshes in the 3D frame of `enclosure.rs` (`x`
//! across, `y` down, `z` from the back wall towards the door), millimetres.
//! The layout crate does not depend on the 3D view; callers convert a render
//! mesh (`novo-view3d::mesh::MeshData`) or a STEP tessellation into a
//! [`Mesh`] with plain vertex and index lists.
//!
//! Two triangles collide when they penetrate each other (separating axis
//! test over the 13 candidate axes); touching faces do not count, in line
//! with `Rect::overlaps`. Clearance is the smallest triangle-to-triangle
//! distance, found with a bounding volume hierarchy so large STEP meshes
//! stay cheap. A part without a mesh is a box from its width, height and
//! depth and is flagged as such (`Body::is_fallback`); results that involve
//! such a body say so through `approximate`.

type V3 = [f64; 3];

const EPS: f64 = 1e-9;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn dist(a: V3, b: V3) -> f64 {
    len(sub(a, b))
}

/// A triangle mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// Axis-aligned bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    fn empty() -> Aabb {
        Aabb {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    fn grow(&mut self, p: V3) {
        for (i, c) in p.iter().enumerate() {
            self.min[i] = self.min[i].min(*c);
            self.max[i] = self.max[i].max(*c);
        }
    }
    fn merge(&self, o: &Aabb) -> Aabb {
        let mut r = *self;
        r.grow(o.min);
        r.grow(o.max);
        r
    }
    /// True when the boxes overlap on every axis, each grown by `margin`
    /// (flat boxes need a small positive margin to overlap at all).
    fn overlaps(&self, o: &Aabb, margin: f64) -> bool {
        (0..3).all(|i| self.min[i] < o.max[i] + margin && o.min[i] < self.max[i] + margin)
    }
    /// Smallest distance between two boxes; 0 when they touch or overlap.
    pub fn gap(&self, o: &Aabb) -> f64 {
        let mut s = 0.0;
        for i in 0..3 {
            let d = (o.min[i] - self.max[i])
                .max(self.min[i] - o.max[i])
                .max(0.0);
            s += d * d;
        }
        s.sqrt()
    }
    fn centre(&self) -> V3 {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
            (self.min[2] + self.max[2]) / 2.0,
        ]
    }
}

impl Mesh {
    /// An axis-aligned box as 12 triangles.
    pub fn boxed(min: [f64; 3], max: [f64; 3]) -> Mesh {
        let mut vertices = Vec::with_capacity(8);
        for i in 0..8u32 {
            vertices.push([
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            ]);
        }
        let quads: [[u32; 4]; 6] = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
        ];
        let triangles = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        Mesh {
            vertices,
            triangles,
        }
    }

    /// True when every index points at a vertex and there is something to test.
    pub fn is_valid(&self) -> bool {
        !self.triangles.is_empty()
            && self
                .triangles
                .iter()
                .all(|t| t.iter().all(|&i| (i as usize) < self.vertices.len()))
    }

    pub fn bounds(&self) -> Option<Aabb> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut b = Aabb::empty();
        for v in &self.vertices {
            b.grow(*v);
        }
        Some(b)
    }

    pub fn translated(&self, d: [f64; 3]) -> Mesh {
        Mesh {
            vertices: self.vertices.iter().map(|v| add(*v, d)).collect(),
            triangles: self.triangles.clone(),
        }
    }

    /// Rotated about the vertical (`y`) axis through the point (`px`, `pz`)
    /// by `angle` radians, positive turning `x` towards `z`. This is what a
    /// hinged door does; the door sweep uses it.
    pub fn rotated_y(&self, px: f64, pz: f64, angle: f64) -> Mesh {
        let (s, c) = angle.sin_cos();
        Mesh {
            vertices: self
                .vertices
                .iter()
                .map(|v| {
                    let (x, z) = (v[0] - px, v[2] - pz);
                    [px + x * c - z * s, v[1], pz + x * s + z * c]
                })
                .collect(),
            triangles: self.triangles.clone(),
        }
    }

    fn tri(&self, i: usize) -> [V3; 3] {
        let t = self.triangles[i];
        [
            self.vertices[t[0] as usize],
            self.vertices[t[1] as usize],
            self.vertices[t[2] as usize],
        ]
    }
}

const LEAF: usize = 4;

#[derive(Debug, Clone)]
struct Node {
    bounds: Aabb,
    /// Leaf: range into `order`; inner: the two children.
    kind: NodeKind,
}

#[derive(Debug, Clone)]
enum NodeKind {
    Leaf { start: usize, end: usize },
    Inner { left: usize, right: usize },
}

/// A mesh with its bounding volume hierarchy.
#[derive(Debug, Clone)]
pub struct Body {
    mesh: Mesh,
    nodes: Vec<Node>,
    order: Vec<usize>,
    fallback: bool,
}

impl Body {
    /// A body from a real mesh; `None` when the mesh is empty or its
    /// indices are out of range.
    pub fn from_mesh(mesh: Mesh) -> Option<Body> {
        if !mesh.is_valid() {
            return None;
        }
        Some(Body::build(mesh, false))
    }

    /// The box stand-in for a part that has no mesh, flagged as a fallback.
    pub fn fallback_box(min: [f64; 3], max: [f64; 3]) -> Body {
        Body::build(Mesh::boxed(min, max), true)
    }

    /// The part's mesh if it has a usable one, else its box.
    pub fn from_mesh_or_box(mesh: Option<Mesh>, min: [f64; 3], max: [f64; 3]) -> Body {
        mesh.and_then(Body::from_mesh)
            .unwrap_or_else(|| Body::fallback_box(min, max))
    }

    fn build(mesh: Mesh, fallback: bool) -> Body {
        let mut order: Vec<usize> = (0..mesh.triangles.len()).collect();
        let boxes: Vec<Aabb> = (0..mesh.triangles.len())
            .map(|i| {
                let mut b = Aabb::empty();
                for p in mesh.tri(i) {
                    b.grow(p);
                }
                b
            })
            .collect();
        let mut nodes = Vec::new();
        let n = order.len();
        build_node(&boxes, &mut order, 0, n, &mut nodes);
        Body {
            mesh,
            nodes,
            order,
            fallback,
        }
    }

    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// True when this body is a box standing in for a missing mesh.
    pub fn is_fallback(&self) -> bool {
        self.fallback
    }

    pub fn bounds(&self) -> Aabb {
        self.nodes[0].bounds
    }

    /// A copy moved by `d`.
    pub fn translated(&self, d: [f64; 3]) -> Body {
        Body::build(self.mesh.translated(d), self.fallback)
    }

    /// A copy turned about the vertical axis through (`px`, `pz`).
    pub fn rotated_y(&self, px: f64, pz: f64, angle: f64) -> Body {
        Body::build(self.mesh.rotated_y(px, pz, angle), self.fallback)
    }
}

fn build_node(
    boxes: &[Aabb],
    order: &mut [usize],
    start: usize,
    end: usize,
    out: &mut Vec<Node>,
) -> usize {
    let mut bounds = Aabb::empty();
    for &i in &order[start..end] {
        bounds = bounds.merge(&boxes[i]);
    }
    let idx = out.len();
    if end - start <= LEAF {
        out.push(Node {
            bounds,
            kind: NodeKind::Leaf { start, end },
        });
        return idx;
    }
    out.push(Node {
        bounds,
        kind: NodeKind::Leaf { start, end },
    });
    let extent = sub(bounds.max, bounds.min);
    let axis = if extent[0] >= extent[1] && extent[0] >= extent[2] {
        0
    } else if extent[1] >= extent[2] {
        1
    } else {
        2
    };
    order[start..end].sort_by(|&a, &b| {
        boxes[a].centre()[axis]
            .partial_cmp(&boxes[b].centre()[axis])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mid = start + (end - start) / 2;
    let left = build_node(boxes, order, start, mid, out);
    let right = build_node(boxes, order, mid, end, out);
    out[idx].kind = NodeKind::Inner { left, right };
    idx
}

/// Two bodies that penetrate each other: either a pair of triangles that
/// cross, or one body lying inside the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contact {
    /// The first pair of crossing triangles; `None` when one body is
    /// inside the other without any surface crossing.
    pub triangles: Option<(usize, usize)>,
    /// One body lies inside the other (closed meshes only).
    pub contained: bool,
    /// One of the two bodies is a box fallback, so the contact is a
    /// worst-case envelope and not the real shape.
    pub approximate: bool,
}

/// How far apart two bodies are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clearance {
    /// Smallest distance in mm; 0 when they touch or penetrate.
    pub distance: f64,
    pub approximate: bool,
}

/// The first penetration of two bodies, or `None` when they are apart or
/// only touch. Meshes are taken as closed solids, so a body lying inside
/// another counts even though no surfaces cross.
pub fn collide(a: &Body, b: &Body) -> Option<Contact> {
    if let Some(c) = surfaces_cross(a, b) {
        return Some(c);
    }
    if !a.bounds().overlaps(&b.bounds(), 1e-6) {
        return None;
    }
    let approximate = a.fallback || b.fallback;
    if lies_inside(a, b) || lies_inside(b, a) {
        return Some(Contact {
            triangles: None,
            contained: true,
            approximate,
        });
    }
    None
}

/// Most triangles sampled for the containment test.
const CONTAIN_SAMPLES: usize = 256;

/// True when the centre of some triangle of `inner` lies strictly inside
/// the closed mesh `outer` (ray parity along a skew direction).
fn lies_inside(inner: &Body, outer: &Body) -> bool {
    let n = inner.mesh.triangles.len();
    let step = n.div_ceil(CONTAIN_SAMPLES).max(1);
    let ob = outer.bounds();
    (0..n).step_by(step).any(|i| {
        let t = inner.mesh.tri(i);
        let c = scale(add(add(t[0], t[1]), t[2]), 1.0 / 3.0);
        (0..3).all(|k| c[k] > ob.min[k] + 1e-7 && c[k] < ob.max[k] - 1e-7) && point_inside(outer, c)
    })
}

/// Crossings of a ray from `p` with the mesh are counted; odd means inside.
fn point_inside(body: &Body, p: V3) -> bool {
    // A direction no axis-aligned face or edge is likely to be parallel to.
    let dir = [0.5773, 0.3017, 0.7571];
    let mut hits = 0;
    for i in 0..body.mesh.triangles.len() {
        let t = body.mesh.tri(i);
        // Moller-Trumbore.
        let e1 = sub(t[1], t[0]);
        let e2 = sub(t[2], t[0]);
        let h = cross(dir, e2);
        let det = dot(e1, h);
        if det.abs() < 1e-12 {
            continue;
        }
        let f = 1.0 / det;
        let s = sub(p, t[0]);
        let u = f * dot(s, h);
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(s, e1);
        let v = f * dot(dir, q);
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        if f * dot(e2, q) > 1e-9 {
            hits += 1;
        }
    }
    hits % 2 == 1
}

fn surfaces_cross(a: &Body, b: &Body) -> Option<Contact> {
    let mut stack = vec![(0usize, 0usize)];
    while let Some((na, nb)) = stack.pop() {
        let (x, y) = (&a.nodes[na], &b.nodes[nb]);
        if !x.bounds.overlaps(&y.bounds, 1e-6) {
            continue;
        }
        match (&x.kind, &y.kind) {
            (NodeKind::Leaf { start: s1, end: e1 }, NodeKind::Leaf { start: s2, end: e2 }) => {
                for &ta in &a.order[*s1..*e1] {
                    for &tb in &b.order[*s2..*e2] {
                        if triangles_penetrate(a.mesh.tri(ta), b.mesh.tri(tb)) {
                            return Some(Contact {
                                triangles: Some((ta, tb)),
                                contained: false,
                                approximate: a.fallback || b.fallback,
                            });
                        }
                    }
                }
            }
            (NodeKind::Inner { left, right }, NodeKind::Leaf { .. }) => {
                stack.push((*left, nb));
                stack.push((*right, nb));
            }
            (NodeKind::Leaf { .. }, NodeKind::Inner { left, right }) => {
                stack.push((na, *left));
                stack.push((na, *right));
            }
            (
                NodeKind::Inner {
                    left: l1,
                    right: r1,
                },
                NodeKind::Inner {
                    left: l2,
                    right: r2,
                },
            ) => {
                for p in [*l1, *r1] {
                    for q in [*l2, *r2] {
                        stack.push((p, q));
                    }
                }
            }
        }
    }
    None
}

/// Smallest distance between two bodies (0 when they touch or penetrate).
pub fn clearance(a: &Body, b: &Body) -> Clearance {
    let approximate = a.fallback || b.fallback;
    if collide(a, b).is_some() {
        return Clearance {
            distance: 0.0,
            approximate,
        };
    }
    let mut best = f64::INFINITY;
    let mut stack = vec![(0usize, 0usize)];
    while let Some((na, nb)) = stack.pop() {
        let (x, y) = (&a.nodes[na], &b.nodes[nb]);
        if x.bounds.gap(&y.bounds) >= best {
            continue;
        }
        match (&x.kind, &y.kind) {
            (NodeKind::Leaf { start: s1, end: e1 }, NodeKind::Leaf { start: s2, end: e2 }) => {
                for &ta in &a.order[*s1..*e1] {
                    for &tb in &b.order[*s2..*e2] {
                        best = best.min(triangle_distance(a.mesh.tri(ta), b.mesh.tri(tb)));
                    }
                }
            }
            (NodeKind::Inner { left, right }, NodeKind::Leaf { .. }) => {
                stack.push((*left, nb));
                stack.push((*right, nb));
            }
            (NodeKind::Leaf { .. }, NodeKind::Inner { left, right }) => {
                stack.push((na, *left));
                stack.push((na, *right));
            }
            (
                NodeKind::Inner {
                    left: l1,
                    right: r1,
                },
                NodeKind::Inner {
                    left: l2,
                    right: r2,
                },
            ) => {
                for p in [*l1, *r1] {
                    for q in [*l2, *r2] {
                        stack.push((p, q));
                    }
                }
            }
        }
    }
    Clearance {
        distance: best,
        approximate,
    }
}

/// True when the bodies are at least `required` mm apart. Touching bodies
/// are only apart when `required` is 0.
pub fn keeps_clearance(a: &Body, b: &Body, required: f64) -> bool {
    clearance(a, b).distance + EPS >= required
}

fn project(tri: &[V3; 3], axis: V3) -> (f64, f64) {
    let p = tri.map(|v| dot(v, axis));
    (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]))
}

/// Penetration test for two triangles; touching does not count.
fn triangles_penetrate(t1: [V3; 3], t2: [V3; 3]) -> bool {
    let e1 = [sub(t1[1], t1[0]), sub(t1[2], t1[1]), sub(t1[0], t1[2])];
    let e2 = [sub(t2[1], t2[0]), sub(t2[2], t2[1]), sub(t2[0], t2[2])];
    let n1 = cross(e1[0], e1[1]);
    let n2 = cross(e2[0], e2[1]);
    let mut axes: Vec<V3> = vec![n1, n2];
    for a in &e1 {
        for b in &e2 {
            axes.push(cross(*a, *b));
        }
    }
    // Coplanar triangles: the cross products above degenerate, the in-plane
    // edge normals separate them instead.
    for a in &e1 {
        axes.push(cross(n1, *a));
    }
    for b in &e2 {
        axes.push(cross(n2, *b));
    }
    let scale_len = e1
        .iter()
        .chain(e2.iter())
        .map(|e| len(*e))
        .fold(0.0, f64::max)
        .max(1.0);
    let mut tested = 0;
    for axis in axes {
        let l = len(axis);
        if l < EPS * scale_len * scale_len {
            continue;
        }
        let axis = scale(axis, 1.0 / l);
        let (a0, a1) = project(&t1, axis);
        let (b0, b1) = project(&t2, axis);
        // Both flat on this axis at the same level: coplanar. Faces of
        // solids that lie in one plane touch, they do not penetrate.
        if a1 - a0 < 1e-7 && b1 - b0 < 1e-7 && (a0 - b0).abs() < 1e-7 {
            return false;
        }
        // Overlap must be real; touching within a hair is a separation.
        if a1 <= b0 + 1e-7 || b1 <= a0 + 1e-7 {
            return false;
        }
        tested += 1;
    }
    tested > 0
}

fn closest_on_segment(p: V3, a: V3, b: V3) -> V3 {
    let ab = sub(b, a);
    let d = dot(ab, ab);
    if d < EPS {
        return a;
    }
    let t = (dot(sub(p, a), ab) / d).clamp(0.0, 1.0);
    add(a, scale(ab, t))
}

fn point_triangle_distance(p: V3, t: &[V3; 3]) -> f64 {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let nl = len(n);
    let mut best = f64::INFINITY;
    for i in 0..3 {
        best = best.min(dist(p, closest_on_segment(p, t[i], t[(i + 1) % 3])));
    }
    if nl > EPS {
        let n = scale(n, 1.0 / nl);
        let d = dot(sub(p, t[0]), n);
        let q = sub(p, scale(n, d));
        // Inside test by edge sides.
        let inside = (0..3).all(|i| {
            let e = sub(t[(i + 1) % 3], t[i]);
            dot(cross(e, sub(q, t[i])), n) >= -1e-9
        });
        if inside {
            best = best.min(d.abs());
        }
    }
    best
}

fn segment_distance(p1: V3, q1: V3, p2: V3, q2: V3) -> f64 {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (s, t);
    if a <= EPS && e <= EPS {
        return dist(p1, p2);
    }
    if a <= EPS {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = dot(d1, r);
        if e <= EPS {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > EPS {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    dist(add(p1, scale(d1, s)), add(p2, scale(d2, t)))
}

/// Distance between two triangles that do not penetrate each other.
fn triangle_distance(t1: [V3; 3], t2: [V3; 3]) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..3 {
        for j in 0..3 {
            best = best.min(segment_distance(
                t1[i],
                t1[(i + 1) % 3],
                t2[j],
                t2[(j + 1) % 3],
            ));
        }
        best = best.min(point_triangle_distance(t1[i], &t2));
        best = best.min(point_triangle_distance(t2[i], &t1));
    }
    best
}
