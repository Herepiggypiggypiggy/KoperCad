//! Thermal radiation inside an enclosure from computed view factors (lane H2).
//!
//! Pure geometry and linear algebra, no solver state:
//!
//! 1. [`Polygon`]: a flat convex surface with a front side (the side that
//!    radiates). Boxes give their six faces through [`box_inside`] (the
//!    walls of an enclosure, looking in) and [`box_outside`] (a device,
//!    looking out).
//! 2. [`element_to_polygon`]: the exact view factor from a differential
//!    element to a flat polygon (Lambert's contour formula, the polygon is
//!    clipped to the half space the element can see). [`view_factor`]
//!    integrates it over the emitting polygon with Gauss-Legendre points, so
//!    it handles any orientation, including faces that touch along an edge.
//!    Closed forms for parallel and edge-sharing rectangles are kept as
//!    reference ([`parallel_rects`], [`perpendicular_rects`]) and checked
//!    against it in the tests.
//! 3. [`Enclosure`]: view factor matrix, reciprocity and summation enforced
//!    exactly for a closed set, and the grey diffuse radiosity system with
//!    mixed boundaries (known temperature, known net heat, or adiabatic
//!    re-radiating).
//!
//! Limits, stated on purpose: air is transparent; surfaces are grey and
//! diffuse; **no occlusion** between surfaces (a device hides another one
//! from the wall only in the 3D solver's ray tracing); emissivity is a
//! planning value per surface, not a measurement.
//!
//! Where the user finds it: Cabinet, Heat layer (radiation split) once the
//! 2D model takes its device-to-wall factors from here (the next H2 step).

use novo_layout::Rect;

pub type V3 = [f64; 3];

/// Stefan-Boltzmann constant, W/(m2 K4).
pub const SIGMA: f64 = 5.670_374_419e-8;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn unit(a: V3) -> V3 {
    scale(a, 1.0 / norm(a))
}

/// A flat convex polygon. The front side is the one the normal
/// (right-hand rule over the vertex order) points to.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    pub pts: Vec<V3>,
}

impl Polygon {
    /// Rectangle `origin + s u + t v`, `s, t` in `[0, 1]`; the normal is `u x v`.
    pub fn rect(origin: V3, u: V3, v: V3) -> Polygon {
        Polygon {
            pts: vec![
                origin,
                add(origin, u),
                add(add(origin, u), v),
                add(origin, v),
            ],
        }
    }

    pub fn normal(&self) -> V3 {
        let n = cross(sub(self.pts[1], self.pts[0]), sub(self.pts[2], self.pts[0]));
        unit(n)
    }

    pub fn area(&self) -> f64 {
        let mut s = [0.0; 3];
        for i in 0..self.pts.len() {
            let c = cross(self.pts[i], self.pts[(i + 1) % self.pts.len()]);
            s = add(s, c);
        }
        0.5 * norm(s)
    }

    pub fn centroid(&self) -> V3 {
        let n = self.pts.len() as f64;
        let mut c = [0.0; 3];
        for p in &self.pts {
            c = add(c, *p);
        }
        scale(c, 1.0 / n)
    }

    pub fn flipped(&self) -> Polygon {
        let mut pts = self.pts.clone();
        pts.reverse();
        Polygon { pts }
    }

    /// The same polygon with its front side towards `point`.
    pub fn facing(self, point: V3) -> Polygon {
        if dot(self.normal(), sub(point, self.centroid())) < 0.0 {
            self.flipped()
        } else {
            self
        }
    }
}

fn box_faces(min: V3, max: V3) -> [Polygon; 6] {
    let d = sub(max, min);
    let (x, y, z) = ([d[0], 0.0, 0.0], [0.0, d[1], 0.0], [0.0, 0.0, d[2]]);
    [
        Polygon::rect(min, y, z),                      // x min
        Polygon::rect([max[0], min[1], min[2]], y, z), // x max
        Polygon::rect(min, x, z),                      // y min
        Polygon::rect([min[0], max[1], min[2]], x, z), // y max
        Polygon::rect(min, x, y),                      // z min
        Polygon::rect([min[0], min[1], max[2]], x, y), // z max
    ]
}

/// The six walls of a box, looking into it. Order: x-, x+, y-, y+, z-, z+.
pub fn box_inside(min: V3, max: V3) -> Vec<Polygon> {
    let c = scale(add(min, max), 0.5);
    box_faces(min, max)
        .into_iter()
        .map(|p| p.facing(c))
        .collect()
}

/// The six faces of a box, looking out of it (a device). Same order.
pub fn box_outside(min: V3, max: V3) -> Vec<Polygon> {
    let c = scale(add(min, max), 0.5);
    box_faces(min, max)
        .into_iter()
        .map(|p| p.facing(c).flipped())
        .collect()
}

/// The part of the polygon on the front side of the plane through `p` with
/// normal `n` (Sutherland-Hodgman against one plane).
fn clip_front(poly: &[V3], p: V3, n: V3) -> Vec<V3> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (da, db) = (dot(sub(a, p), n), dot(sub(b, p), n));
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            out.push(add(a, scale(sub(b, a), t)));
        }
    }
    out
}

/// Exact view factor from a differential element at `p` with unit normal
/// `n` to a flat polygon, without obstacles. Zero when the element faces
/// away from the polygon or lies behind its plane.
pub fn element_to_polygon(p: V3, n: V3, poly: &Polygon) -> f64 {
    // The element must be on the front side of the polygon.
    if dot(sub(p, poly.pts[0]), poly.normal()) <= 1e-12 {
        return 0.0;
    }
    let clipped = clip_front(&poly.pts, p, n);
    if clipped.len() < 3 {
        return 0.0;
    }
    let dirs: Vec<V3> = clipped
        .iter()
        .map(|&q| sub(q, p))
        .filter(|d| norm(*d) > 1e-12)
        .map(unit)
        .collect();
    if dirs.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..dirs.len() {
        let (a, b) = (dirs[i], dirs[(i + 1) % dirs.len()]);
        let c = cross(a, b);
        let s = norm(c);
        if s < 1e-14 {
            continue;
        }
        let angle = dot(a, b).clamp(-1.0, 1.0).acos();
        sum += angle * dot(scale(c, 1.0 / s), n);
    }
    (sum.abs() / std::f64::consts::TAU).min(1.0)
}

/// True when the open segment `a`-`b` passes through the inside of the box
/// (slab test on the box shrunk by a hair, so a segment that starts on a
/// face of the box does not count as blocked by it).
fn segment_hits_box(a: V3, b: V3, min: V3, max: V3) -> bool {
    const EPS: f64 = 1e-6;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for k in 0..3 {
        let (lo, hi) = (min[k] + EPS, max[k] - EPS);
        if lo >= hi {
            return false;
        }
        let d = b[k] - a[k];
        if d.abs() < 1e-15 {
            if a[k] <= lo || a[k] >= hi {
                return false;
            }
        } else {
            let (mut u, mut v) = ((lo - a[k]) / d, (hi - a[k]) / d);
            if u > v {
                std::mem::swap(&mut u, &mut v);
            }
            t0 = t0.max(u);
            t1 = t1.min(v);
            if t0 >= t1 {
                return false;
            }
        }
    }
    true
}

/// View factor `F(a -> b)` with boxes `(min, max)` in the way: the exact
/// unobstructed factor of each emitting point, times the share of
/// sight lines to a 4 x 4 grid on `b` that no box cuts. A model, accurate
/// to a few percent for shadows larger than the grid pitch.
pub fn view_factor_blocked(a: &Polygon, b: &Polygon, div: usize, blockers: &[(V3, V3)]) -> f64 {
    if blockers.is_empty() {
        return view_factor(a, b, div);
    }
    let targets: Vec<V3> = sample_points(b, 2).into_iter().map(|(p, _)| p).collect();
    let n = a.normal();
    sample_points(a, div.max(1))
        .into_iter()
        .map(|(p, w)| {
            let f = element_to_polygon(p, n, b);
            if f == 0.0 {
                return 0.0;
            }
            let seen = targets
                .iter()
                .filter(|&&q| {
                    !blockers
                        .iter()
                        .any(|&(lo, hi)| segment_hits_box(p, q, lo, hi))
                })
                .count();
            w * f * seen as f64 / targets.len() as f64
        })
        .sum()
}

/// 4-point Gauss-Legendre nodes and weights on `[0, 1]`.
const GL: [(f64, f64); 4] = [
    (0.069_431_844_202_973_71, 0.173_927_422_568_726_93),
    (0.330_009_478_207_571_9, 0.326_072_577_431_273_07),
    (0.669_990_521_792_428_1, 0.326_072_577_431_273_07),
    (0.930_568_155_797_026_3, 0.173_927_422_568_726_93),
];

/// Quadrature points `(position, weight)` over a polygon, weights summing
/// to one. Only quads are subdivided: `div x div` cells with 16 points each;
/// other shapes use the centroid fan (rarely needed).
fn sample_points(poly: &Polygon, div: usize) -> Vec<(V3, f64)> {
    if poly.pts.len() != 4 {
        return vec![(poly.centroid(), 1.0)];
    }
    let o = poly.pts[0];
    let u = sub(poly.pts[1], o);
    let v = sub(poly.pts[3], o);
    let mut out = Vec::with_capacity(div * div * 16);
    let w_cell = 1.0 / (div * div) as f64;
    for i in 0..div {
        for j in 0..div {
            for &(s, ws) in &GL {
                for &(t, wt) in &GL {
                    let a = (i as f64 + s) / div as f64;
                    let b = (j as f64 + t) / div as f64;
                    out.push((add(o, add(scale(u, a), scale(v, b))), w_cell * ws * wt));
                }
            }
        }
    }
    out
}

/// View factor `F(a -> b)` of two flat polygons without obstacles: the share
/// of the diffuse radiation leaving `a` that reaches `b`. `div` is the
/// number of quadrature cells per side of `a` (4 gives about 1e-4 for
/// ordinary boxes; faces that touch along an edge converge slowest).
pub fn view_factor(a: &Polygon, b: &Polygon, div: usize) -> f64 {
    let n = a.normal();
    sample_points(a, div.max(1))
        .into_iter()
        .map(|(p, w)| w * element_to_polygon(p, n, b))
        .sum()
}

/// Closed form for two equal, aligned rectangles `a x b` in parallel planes
/// a distance `c` apart (Hottel; Howell catalogue C-11). Reference.
pub fn parallel_rects(a: f64, b: f64, c: f64) -> f64 {
    let (x, y) = (a / c, b / c);
    let (sx, sy) = ((1.0 + x * x).sqrt(), (1.0 + y * y).sqrt());
    let ln = (((1.0 + x * x) * (1.0 + y * y)) / (1.0 + x * x + y * y))
        .sqrt()
        .ln();
    2.0 / (std::f64::consts::PI * x * y)
        * (ln + x * sy * (x / sy).atan() + y * sx * (y / sx).atan() - x * x.atan() - y * y.atan())
}

/// Closed form for rectangles in perpendicular planes sharing an edge of
/// length `l`; the emitting one extends `w` and the receiving one `h` away
/// from the edge (Howell C-14). Reference.
pub fn perpendicular_rects(w: f64, h: f64, l: f64) -> f64 {
    let (hh, ww) = (h / l, w / l);
    let r = (hh * hh + ww * ww).sqrt();
    let pi = std::f64::consts::PI;
    let ln = ((1.0 + ww * ww) * (1.0 + hh * hh) / (1.0 + ww * ww + hh * hh)).ln()
        + ww * ww
            * (ww * ww * (1.0 + ww * ww + hh * hh) / ((1.0 + ww * ww) * (ww * ww + hh * hh))).ln()
        + hh * hh
            * (hh * hh * (1.0 + hh * hh + ww * ww) / ((1.0 + hh * hh) * (hh * hh + ww * ww))).ln();
    1.0 / (pi * ww)
        * (ww * (1.0 / ww).atan() + hh * (1.0 / hh).atan() - r * (1.0 / r).atan() + 0.25 * ln)
}

/// How a surface of an [`Enclosure`] is held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Boundary {
    /// Surface temperature, kelvin.
    Temperature(f64),
    /// Net heat the surface gives off by radiation, watts (negative: takes
    /// heat in). Zero is an adiabatic, re-radiating surface.
    Power(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub polygon: Polygon,
    pub emissivity: f64,
    pub boundary: Boundary,
}

/// A set of surfaces that see each other.
#[derive(Debug, Clone)]
pub struct Enclosure {
    pub surfaces: Vec<Surface>,
    n: usize,
    f: Vec<f64>,
}

/// Solution of [`Enclosure::solve`].
#[derive(Debug, Clone, PartialEq)]
pub struct Exchange {
    /// Surface temperature, kelvin (given or solved).
    pub temperature_k: Vec<f64>,
    /// Net radiative heat given off, watts (negative: absorbed).
    pub power_w: Vec<f64>,
    pub radiosity_w_m2: Vec<f64>,
}

impl Enclosure {
    /// Computes the view factors. With `closed`, reciprocity and the
    /// summation rule are enforced exactly by a symmetric scaling (the
    /// quadrature error, about 1e-4, is spread instead of kept).
    pub fn new(surfaces: Vec<Surface>, closed: bool, div: usize) -> Enclosure {
        Enclosure::with_blockers(surfaces, &[], closed, div)
    }

    /// As [`Enclosure::new`] with boxes `(min, max)` that hide surfaces from
    /// each other (devices on the plate). With blockers the rows still sum
    /// to one for a closed set, because the blockers' own faces are
    /// surfaces of the set; pass those too.
    pub fn with_blockers(
        surfaces: Vec<Surface>,
        blockers: &[(V3, V3)],
        closed: bool,
        div: usize,
    ) -> Enclosure {
        let n = surfaces.len();
        let areas: Vec<f64> = surfaces.iter().map(|s| s.polygon.area()).collect();
        let mut g = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    g[i * n + j] = areas[i]
                        * view_factor_blocked(
                            &surfaces[i].polygon,
                            &surfaces[j].polygon,
                            div,
                            blockers,
                        );
                }
            }
        }
        // Symmetrise (reciprocity), then scale rows to one if closed.
        let mut sym = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                sym[i * n + j] = 0.5 * (g[i * n + j] + g[j * n + i]);
            }
        }
        let mut d = vec![1.0; n];
        if closed {
            for _ in 0..1000 {
                let mut worst: f64 = 0.0;
                for i in 0..n {
                    let s: f64 = (0..n).map(|j| sym[i * n + j] * d[j]).sum();
                    if s > 0.0 {
                        worst = worst.max((d[i] * s / areas[i] - 1.0).abs());
                        d[i] = (d[i] * areas[i] / s).sqrt();
                    }
                }
                if worst < 1e-13 {
                    break;
                }
            }
        }
        let f = (0..n * n)
            .map(|k| {
                let (i, j) = (k / n, k % n);
                d[i] * sym[k] * d[j] / areas[i]
            })
            .collect();
        Enclosure { surfaces, n, f }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `F(i -> j)`.
    pub fn factor(&self, i: usize, j: usize) -> f64 {
        self.f[i * self.n + j]
    }

    pub fn area(&self, i: usize) -> f64 {
        self.surfaces[i].polygon.area()
    }

    /// Largest `|A_i F_ij - A_j F_ji| / mean(A_i, A_j)`.
    pub fn reciprocity_error(&self) -> f64 {
        let mut worst: f64 = 0.0;
        for i in 0..self.n {
            for j in (i + 1)..self.n {
                let (ai, aj) = (self.area(i), self.area(j));
                let e = (ai * self.factor(i, j) - aj * self.factor(j, i)).abs();
                worst = worst.max(e / (0.5 * (ai + aj)));
            }
        }
        worst
    }

    /// Largest `|sum_j F_ij - 1|` (zero for a closed set after `new`).
    pub fn summation_error(&self) -> f64 {
        (0..self.n)
            .map(|i| ((0..self.n).map(|j| self.factor(i, j)).sum::<f64>() - 1.0).abs())
            .fold(0.0, f64::max)
    }

    /// The grey diffuse radiosity system. Returns `None` when the system is
    /// singular (for example every surface has a heat boundary and no
    /// temperature anchors the level, or an emissivity is zero).
    pub fn solve(&self) -> Option<Exchange> {
        let n = self.n;
        let mut a = vec![0.0; n * n];
        let mut rhs = vec![0.0; n];
        for (i, s) in self.surfaces.iter().enumerate() {
            let e = s.emissivity.clamp(1e-3, 1.0);
            for j in 0..n {
                let id = if i == j { 1.0 } else { 0.0 };
                match s.boundary {
                    Boundary::Temperature(_) => a[i * n + j] = id - (1.0 - e) * self.factor(i, j),
                    Boundary::Power(_) => a[i * n + j] = id - self.factor(i, j),
                }
            }
            rhs[i] = match s.boundary {
                Boundary::Temperature(t) => e * SIGMA * t.powi(4),
                Boundary::Power(w) => w / self.area(i),
            };
        }
        let j = solve_dense(a, rhs)?;
        let mut t = vec![0.0; n];
        let mut p = vec![0.0; n];
        for (i, s) in self.surfaces.iter().enumerate() {
            let e = s.emissivity.clamp(1e-3, 1.0);
            let g: f64 = (0..n).map(|k| self.factor(i, k) * j[k]).sum();
            let q = j[i] - g;
            p[i] = q * self.area(i);
            t[i] = match s.boundary {
                Boundary::Temperature(t0) => t0,
                Boundary::Power(_) => ((j[i] + (1.0 - e) / e * q) / SIGMA).max(0.0).powf(0.25),
            };
        }
        Some(Exchange {
            temperature_k: t,
            power_w: p,
            radiosity_w_m2: j,
        })
    }
}

/// Gaussian elimination with partial pivoting.
fn solve_dense(mut a: Vec<f64>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|&x, &y| a[x * n + c].abs().total_cmp(&a[y * n + c].abs()))?;
        if a[piv * n + c].abs() < 1e-12 {
            return None;
        }
        if piv != c {
            for k in 0..n {
                a.swap(c * n + k, piv * n + k);
            }
            b.swap(c, piv);
        }
        for r in (c + 1)..n {
            let m = a[r * n + c] / a[c * n + c];
            if m != 0.0 {
                for k in c..n {
                    a[r * n + k] -= m * a[c * n + k];
                }
                b[r] -= m * b[c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = ((r + 1)..n).map(|k| a[r * n + k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r * n + r];
    }
    Some(x)
}

/// Net radiation between a small convex body (area `a1`, emissivity `e1`)
/// and the enclosing surface (area `a2`, emissivity `e2`), both isothermal:
/// the textbook two-surface result, `q = sigma (T1^4 - T2^4) A1 /
/// (1/e1 + A1/A2 (1/e2 - 1))`. Exact for concentric shapes; reference.
pub fn two_surface_power(a1: f64, e1: f64, t1: f64, a2: f64, e2: f64, t2: f64) -> f64 {
    SIGMA * (t1.powi(4) - t2.powi(4)) * a1 / (1.0 / e1 + a1 / a2 * (1.0 / e2 - 1.0))
}

/// `Rect` helper for callers that hold 2D footprints: a footprint rectangle
/// of the cabinet plate as a box from the plate up to `height_mm`, in metres.
pub fn footprint_box(r: &Rect, plate_z_mm: f64, height_mm: f64) -> (V3, V3) {
    (
        [r.x / 1000.0, r.y / 1000.0, plate_z_mm / 1000.0],
        [
            (r.x + r.w) / 1000.0,
            (r.y + r.h) / 1000.0,
            (plate_z_mm + height_mm) / 1000.0,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(l: f64) -> Vec<Polygon> {
        box_inside([0.0; 3], [l, l, l])
    }

    #[test]
    fn parallel_unit_squares_match_the_closed_form() {
        let a = Polygon::rect([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let b = Polygon::rect([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).flipped();
        let f = view_factor(&a, &b, 4);
        assert!((f - parallel_rects(1.0, 1.0, 1.0)).abs() < 2e-4, "{f}");
        assert!((f - 0.199_825).abs() < 3e-4, "{f}");
    }

    #[test]
    fn perpendicular_unit_squares_match_the_closed_form() {
        // Floor z=0 looking up, wall x=0 looking in (+x): share the y edge.
        let floor = Polygon::rect([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let wall = Polygon::rect([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let (floor, wall) = (floor.facing([0.5, 0.5, 1.0]), wall.facing([1.0, 0.5, 0.5]));
        let f = view_factor(&floor, &wall, 6);
        assert!((f - perpendicular_rects(1.0, 1.0, 1.0)).abs() < 5e-4, "{f}");
        assert!((f - 0.200_04).abs() < 6e-4, "{f}");
    }

    #[test]
    fn off_size_parallel_and_perpendicular_agree_with_closed_forms() {
        let (a, b, c) = (0.8, 0.5, 0.3);
        let bot = Polygon::rect([0.0; 3], [a, 0.0, 0.0], [0.0, b, 0.0]).facing([0.0, 0.0, 1.0]);
        let top = Polygon::rect([0.0, 0.0, c], [a, 0.0, 0.0], [0.0, b, 0.0]).facing([0.0; 3]);
        let f = view_factor(&bot, &top, 4);
        assert!((f - parallel_rects(a, b, c)).abs() < 5e-4, "{f}");
        // Floor (a along x, b along y) to the wall x = 0 (b along y, c up).
        let wall = Polygon::rect([0.0; 3], [0.0, b, 0.0], [0.0, 0.0, c]).facing([1.0, 0.0, 0.0]);
        let f = view_factor(&bot, &wall, 8);
        // Emitter extends a from the edge, receiver c; shared edge b.
        assert!((f - perpendicular_rects(a, c, b)).abs() < 1e-3, "{f}");
    }

    #[test]
    fn a_cube_sees_one_fifth_of_itself_in_every_face() {
        let surfaces: Vec<Surface> = cube(1.0)
            .into_iter()
            .map(|polygon| Surface {
                polygon,
                emissivity: 1.0,
                boundary: Boundary::Temperature(300.0),
            })
            .collect();
        let enc = Enclosure::new(surfaces, true, 4);
        for i in 0..6 {
            for j in 0..6 {
                let want = if i == j { 0.0 } else { 0.2 };
                assert!((enc.factor(i, j) - want).abs() < 6e-4, "{i} {j}");
            }
        }
        assert!(enc.reciprocity_error() < 1e-12);
        assert!(enc.summation_error() < 1e-12);
    }

    #[test]
    fn an_empty_box_is_closed_and_reciprocal() {
        let surfaces: Vec<Surface> = box_inside([0.0; 3], [0.8, 0.3, 1.9])
            .into_iter()
            .map(|polygon| Surface {
                polygon,
                emissivity: 0.9,
                boundary: Boundary::Temperature(300.0),
            })
            .collect();
        let enc = Enclosure::new(surfaces, true, 4);
        assert!(enc.reciprocity_error() < 1e-12);
        assert!(enc.summation_error() < 1e-12);
        // The raw (unscaled) sampling is close to closed already.
        let raw: f64 = (0..6)
            .map(|j| view_factor(&enc.surfaces[0].polygon, &enc.surfaces[j].polygon, 4))
            .sum();
        assert!((raw - 1.0).abs() < 2e-3, "{raw}");
    }

    #[test]
    fn a_small_body_in_a_box_follows_the_two_surface_formula() {
        let mut surfaces = Vec::new();
        for p in box_inside([0.0; 3], [1.0, 1.0, 1.0]) {
            surfaces.push(Surface {
                polygon: p,
                emissivity: 0.8,
                boundary: Boundary::Temperature(300.0),
            });
        }
        let body = box_outside([0.45; 3], [0.55; 3]);
        let a1: f64 = body.iter().map(Polygon::area).sum();
        for p in body {
            surfaces.push(Surface {
                polygon: p,
                emissivity: 0.9,
                boundary: Boundary::Temperature(350.0),
            });
        }
        let enc = Enclosure::new(surfaces, true, 4);
        let x = enc.solve().unwrap();
        let q: f64 = x.power_w[6..].iter().sum();
        let want = two_surface_power(a1, 0.9, 350.0, 6.0, 0.8, 300.0);
        assert!((q / want - 1.0).abs() < 0.02, "{q} vs {want}");
        // Energy balance of the closed set.
        let sum: f64 = x.power_w.iter().sum();
        assert!(sum.abs() < 1e-9 * q.abs(), "{sum}");
    }

    #[test]
    fn parallel_plates_close_together_follow_the_infinite_plate_formula() {
        // Two 1 m plates 5 mm apart, ringed by a cold black room: the middle
        // behaves as infinite plates.
        let (e1, e2, t1, t2) = (0.3, 0.9, 400.0, 300.0);
        let gap = 0.005;
        let hot = Polygon::rect([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).facing([0.5, 0.5, 1.0]);
        let cold = Polygon::rect([0.0, 0.0, gap], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
            .facing([0.5, 0.5, 0.0]);
        let f = view_factor(&hot, &cold, 6);
        assert!(f > 0.97, "{f}");
        // Two-surface exchange with F = f (rest of the view is a black room).
        let enc = Enclosure::new(
            vec![
                Surface {
                    polygon: hot,
                    emissivity: e1,
                    boundary: Boundary::Temperature(t1),
                },
                Surface {
                    polygon: cold,
                    emissivity: e2,
                    boundary: Boundary::Temperature(t2),
                },
            ],
            false,
            6,
        );
        let x = enc.solve().unwrap();
        let want = SIGMA * (t1.powi(4) - t2.powi(4)) / (1.0 / e1 + 1.0 / e2 - 1.0);
        assert!(
            (x.power_w[0] / want - 1.0).abs() < 0.04,
            "{} vs {want}",
            x.power_w[0]
        );
    }

    #[test]
    fn black_adiabatic_sides_sit_at_the_fourth_power_mean() {
        let temps = [400.0, 300.0];
        let mut surfaces = Vec::new();
        for (i, p) in cube(1.0).into_iter().enumerate() {
            let boundary = match i {
                4 => Boundary::Temperature(temps[0]), // floor
                5 => Boundary::Temperature(temps[1]), // lid
                _ => Boundary::Power(0.0),
            };
            surfaces.push(Surface {
                polygon: p,
                emissivity: 1.0,
                boundary,
            });
        }
        let x = Enclosure::new(surfaces, true, 4).solve().unwrap();
        let want = ((temps[0].powi(4) + temps[1].powi(4)) / 2.0).powf(0.25);
        for i in 0..4 {
            assert!(
                (x.temperature_k[i] - want).abs() < 0.1,
                "{}",
                x.temperature_k[i]
            );
        }
        assert!((x.power_w[4] + x.power_w[5]).abs() < 1e-6);
    }

    #[test]
    fn a_blocker_between_two_plates_hides_them_from_each_other() {
        let lo = Polygon::rect([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).facing([0.5, 0.5, 1.0]);
        let hi = Polygon::rect([0.0, 0.0, 0.4], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
            .facing([0.5, 0.5, 0.0]);
        let open = view_factor_blocked(&lo, &hi, 4, &[]);
        let full = view_factor_blocked(&lo, &hi, 4, &[([-1.0, -1.0, 0.1], [2.0, 2.0, 0.2])]);
        assert!(open > 0.3 && full < 1e-9, "{open} {full}");
        // A slab over the left half hides about the left half.
        let half = view_factor_blocked(&lo, &hi, 4, &[([-1.0, -1.0, 0.1], [0.5, 2.0, 0.2])]);
        assert!(half > 0.3 * open && half < 0.7 * open, "{half} vs {open}");
    }

    #[test]
    fn a_device_on_the_plate_sees_less_wall_than_it_does_alone() {
        // Device A (hot) next to a tall neighbour B: part of A's side view of
        // the wall is hidden by B.
        let (a_lo, a_hi) = ([0.2, 0.4, 0.0], [0.3, 0.6, 0.1]);
        let (b_lo, b_hi) = ([0.32, 0.5, 0.0], [0.5, 0.8, 0.3]);
        let room = box_inside([0.0; 3], [1.0, 1.0, 0.5]);
        let wall = &room[1]; // x+ wall
        let side = &box_outside(a_lo, a_hi)[1]; // A's x+ face
        let free = view_factor(side, wall, 4);
        let hidden = view_factor_blocked(side, wall, 4, &[(b_lo, b_hi)]);
        assert!(
            hidden < 0.8 * free && hidden > 0.1 * free,
            "{hidden} {free}"
        );
    }

    #[test]
    fn low_emissivity_walls_reflect_and_shrink_the_exchange() {
        let run = |e_wall: f64| {
            let mut s: Vec<Surface> = box_inside([0.0; 3], [1.0; 3])
                .into_iter()
                .map(|polygon| Surface {
                    polygon,
                    emissivity: e_wall,
                    boundary: Boundary::Temperature(300.0),
                })
                .collect();
            for p in box_outside([0.4; 3], [0.6; 3]) {
                s.push(Surface {
                    polygon: p,
                    emissivity: 0.9,
                    boundary: Boundary::Temperature(340.0),
                });
            }
            let x = Enclosure::new(s, true, 3).solve().unwrap();
            x.power_w[6..].iter().sum::<f64>()
        };
        // A reflective wall sends the body's own radiation back to it.
        assert!(run(0.1) < run(0.9));
        assert!(run(0.9) > 0.0);
    }
}
