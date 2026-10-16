//! STEP B-rep entities to plain geometry: units, points, frames, curves,
//! surfaces, edges sampled into polylines, face boundaries.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::rc::Rc;
use std::sync::Arc;

use super::math::*;
use super::nurbs::SplineSurface;
use super::parse::{Entity, Param, StepFile};

/// A curve, in millimetres.
#[derive(Debug, Clone)]
pub enum Curve {
    Line,
    Circle(Frame, f64),
    Ellipse(Frame, f64, f64),
    Spline(Spline),
    Poly(Vec<V3>),
    Other,
}

/// A (rational) B-spline curve with the knots expanded.
#[derive(Debug, Clone)]
pub struct Spline {
    pub degree: usize,
    pub ctrl: Vec<V3>,
    pub weights: Option<Vec<f64>>,
    pub knots: Vec<f64>,
}

impl Spline {
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.ctrl.len()])
    }

    /// Point at parameter `t` (de Boor, in homogeneous coordinates).
    pub fn eval(&self, t: f64) -> V3 {
        let p = self.degree;
        let n = self.ctrl.len();
        let (lo, hi) = self.domain();
        let t = t.clamp(lo, hi);
        // Knot span k with knots[k] <= t < knots[k+1] (clamped at the end).
        let mut k = p;
        while k + 1 < n && self.knots[k + 1] <= t {
            k += 1;
        }
        let mut d: Vec<[f64; 4]> = (0..=p)
            .map(|j| {
                let i = k + j - p;
                let w = self.weights.as_ref().map_or(1.0, |w| w[i]);
                let c = self.ctrl[i];
                [c[0] * w, c[1] * w, c[2] * w, w]
            })
            .collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k + j - p;
                let den = self.knots[i + p + 1 - r] - self.knots[i];
                let a = if den.abs() < 1e-14 {
                    0.0
                } else {
                    (t - self.knots[i]) / den
                };
                let prev = d[j - 1];
                for (x, y) in d[j].iter_mut().zip(prev) {
                    *x = (1.0 - a) * y + a * *x;
                }
            }
        }
        let w = d[p][3];
        if w.abs() < 1e-14 {
            return [d[p][0], d[p][1], d[p][2]];
        }
        [d[p][0] / w, d[p][1] / w, d[p][2] / w]
    }
}

/// A surface, in millimetres.
#[derive(Debug, Clone, Copy)]
pub enum Surface {
    Plane(Frame),
    Cylinder(Frame, f64),
    /// Frame, radius at the origin, semi-angle in radians.
    Cone(Frame, f64, f64),
    Sphere(Frame, f64),
    /// Frame, major radius, minor radius.
    Torus(Frame, f64, f64),
    /// A B-spline surface; the data is in `Face::nurbs`.
    BSpline,
    Other,
}

/// One face: closed boundary polylines and the surface they lie on.
#[derive(Debug, Clone)]
pub struct Face {
    pub loops: Vec<Vec<V3>>,
    pub surface: Surface,
    /// The surface itself when `surface` is [`Surface::BSpline`].
    pub nurbs: Option<Arc<SplineSurface>>,
    pub same_sense: bool,
}

pub struct Reader<'a> {
    pub file: &'a StepFile<'a>,
    /// Millimetres per model unit.
    pub scale: f64,
    /// Chord tolerance for curved edges, in millimetres.
    pub tol: f64,
    edges: HashMap<u32, Rc<Vec<V3>>>,
}

fn nums(p: &Param) -> Vec<f64> {
    p.as_list()
        .map(|l| l.iter().filter_map(Param::as_f64).collect())
        .unwrap_or_default()
}

fn prefix_scale(p: &Param) -> f64 {
    match p {
        Param::Enum(e) => match e.as_str() {
            "MILLI" => 1.0,
            "CENTI" => 10.0,
            "DECI" => 100.0,
            "MICRO" => 1e-3,
            "KILO" => 1e6,
            _ => f64::NAN,
        },
        _ => 1000.0,
    }
}

impl<'a> Reader<'a> {
    pub fn new(file: &'a StepFile<'a>, tol: f64) -> Reader<'a> {
        let mut r = Reader {
            file,
            scale: 1.0,
            tol,
            edges: HashMap::new(),
        };
        r.scale = r.find_scale();
        r
    }

    /// Millimetres per unit of the first declared length unit; millimetres
    /// when none is declared.
    fn find_scale(&self) -> f64 {
        // The unit the geometry context lists; an inch declaration also
        // contains a millimetre unit of its own, so order alone misleads.
        for &id in self.file.ids_of("GLOBAL_UNIT_ASSIGNED_CONTEXT") {
            let Some(e) = self.file.entity(id) else {
                continue;
            };
            let Some(units) = e
                .part("GLOBAL_UNIT_ASSIGNED_CONTEXT")
                .and_then(|a| a.first())
            else {
                continue;
            };
            for u in units.refs() {
                if self.file.entity(u).is_some_and(|e| e.has("LENGTH_UNIT"))
                    && let Some(s) = self.unit_mm(u, 0)
                {
                    return s;
                }
            }
        }
        for &id in self.file.ids_of("LENGTH_UNIT") {
            if let Some(s) = self.unit_mm(id, 0) {
                return s;
            }
        }
        1.0
    }

    fn unit_mm(&self, id: u32, depth: u8) -> Option<f64> {
        if depth > 4 {
            return None;
        }
        let e = self.file.entity(id)?;
        if let Some(si) = e.part("SI_UNIT") {
            let k = prefix_scale(si.first()?);
            if k.is_nan() {
                return None;
            }
            return matches!(si.get(1), Some(Param::Enum(n)) if n == "METRE").then_some(k);
        }
        if let Some(cb) = e.part("CONVERSION_BASED_UNIT") {
            let m = self.file.entity(cb.get(1)?.as_ref()?)?;
            let a = m.args();
            let v = a.first()?.as_f64()?;
            let base = self.unit_mm(a.get(1)?.as_ref()?, depth + 1)?;
            return Some(v * base);
        }
        None
    }

    pub fn point(&self, id: u32) -> Option<V3> {
        let e = self.file.entity(id)?;
        let c = nums(e.args().get(1)?);
        (c.len() >= 3).then(|| [c[0] * self.scale, c[1] * self.scale, c[2] * self.scale])
    }

    pub fn dir(&self, id: u32) -> Option<V3> {
        let e = self.file.entity(id)?;
        let c = nums(e.args().get(1)?);
        (c.len() >= 3).then(|| [c[0], c[1], c[2]])
    }

    pub fn frame(&self, id: u32) -> Option<Frame> {
        let e = self.file.entity(id)?;
        let a = e.args();
        let o = self.point(a.get(1)?.as_ref()?)?;
        let z = a.get(2).and_then(Param::as_ref).and_then(|r| self.dir(r));
        let x = a.get(3).and_then(Param::as_ref).and_then(|r| self.dir(r));
        Some(Frame::from_axes(o, z.unwrap_or([0.0, 0.0, 1.0]), x))
    }

    pub fn curve(&self, id: u32) -> Curve {
        self.curve_at(id, 0)
    }

    fn curve_at(&self, id: u32, depth: u8) -> Curve {
        let Some(e) = self.file.entity(id) else {
            return Curve::Other;
        };
        if depth > 4 {
            return Curve::Other;
        }
        let a = e.args();
        match e.name() {
            "LINE" => Curve::Line,
            "CIRCLE" => match (
                a.get(1).and_then(Param::as_ref).and_then(|r| self.frame(r)),
                a.get(2).and_then(Param::as_f64),
            ) {
                (Some(f), Some(r)) => Curve::Circle(f, r * self.scale),
                _ => Curve::Other,
            },
            "ELLIPSE" => match (
                a.get(1).and_then(Param::as_ref).and_then(|r| self.frame(r)),
                a.get(2).and_then(Param::as_f64),
                a.get(3).and_then(Param::as_f64),
            ) {
                (Some(f), Some(p), Some(q)) => Curve::Ellipse(f, p * self.scale, q * self.scale),
                _ => Curve::Other,
            },
            "TRIMMED_CURVE" => a
                .get(1)
                .and_then(Param::as_ref)
                .map_or(Curve::Other, |r| self.curve_at(r, depth + 1)),
            "SURFACE_CURVE" | "SEAM_CURVE" | "INTERSECTION_CURVE" => a
                .get(1)
                .and_then(Param::as_ref)
                .map_or(Curve::Other, |r| self.curve_at(r, depth + 1)),
            "POLYLINE" => Curve::Poly(
                a.get(1)
                    .map(|p| p.refs())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| self.point(r))
                    .collect(),
            ),
            _ => self.spline(&e).map_or(Curve::Other, Curve::Spline),
        }
    }

    fn spline(&self, e: &Entity) -> Option<Spline> {
        // Simple entity: (name, degree, ctrl, form, closed, self, mults, knots, spec).
        // Complex entity: B_SPLINE_CURVE(degree, ctrl, ..) and
        // B_SPLINE_CURVE_WITH_KNOTS(mults, knots, spec) as separate parts.
        let (degree, ctrl, mults, knots) = if e.name() == "B_SPLINE_CURVE_WITH_KNOTS" {
            let a = e.args();
            (a.get(1)?, a.get(2)?, a.get(6)?, a.get(7)?)
        } else {
            let b = e.part("B_SPLINE_CURVE")?;
            let k = e.part("B_SPLINE_CURVE_WITH_KNOTS")?;
            (b.first()?, b.get(1)?, k.first()?, k.get(1)?)
        };
        let degree = degree.as_int()? as usize;
        let ctrl: Vec<V3> = ctrl
            .refs()
            .into_iter()
            .filter_map(|r| self.point(r))
            .collect();
        let mut expanded = Vec::new();
        for (m, k) in mults.as_list()?.iter().zip(knots.as_list()?) {
            for _ in 0..m.as_int()?.max(0) {
                expanded.push(k.as_f64()?);
            }
        }
        if degree == 0 || ctrl.len() <= degree || expanded.len() != ctrl.len() + degree + 1 {
            return None;
        }
        let weights = e
            .part("RATIONAL_B_SPLINE_CURVE")
            .and_then(|w| w.first())
            .map(nums)
            .filter(|w| w.len() == ctrl.len());
        Some(Spline {
            degree,
            ctrl,
            weights,
            knots: expanded,
        })
    }

    /// A `B_SPLINE_SURFACE_WITH_KNOTS` (simple or complex, rational or
    /// not). The other forms (uniform, quasi-uniform, Bezier) carry no
    /// knot lists and are left to the flat fallback.
    fn spline_surface(&self, id: u32) -> Option<SplineSurface> {
        let e = self.file.entity(id)?;
        let (degrees, rows, mults, knots) = if e.name() == "B_SPLINE_SURFACE_WITH_KNOTS" {
            let a = e.args();
            (
                [a.get(1)?, a.get(2)?],
                a.get(3)?,
                [a.get(8)?, a.get(9)?],
                [a.get(10)?, a.get(11)?],
            )
        } else {
            let b = e.part("B_SPLINE_SURFACE")?;
            let k = e.part("B_SPLINE_SURFACE_WITH_KNOTS")?;
            (
                [b.first()?, b.get(1)?],
                b.get(2)?,
                [k.first()?, k.get(1)?],
                [k.get(2)?, k.get(3)?],
            )
        };
        let (du, dv) = (degrees[0].as_int()?, degrees[1].as_int()?);
        let (du, dv) = (usize::try_from(du).ok()?, usize::try_from(dv).ok()?);
        let mut ctrl = Vec::new();
        let mut nv = 0;
        let rows = rows.as_list()?;
        for row in rows {
            let refs = row.refs();
            if nv != 0 && refs.len() != nv {
                return None;
            }
            nv = refs.len();
            for r in refs {
                let p = self.point(r)?;
                ctrl.push(p);
            }
        }
        let nu = rows.len();
        let expand = |m: &Param, k: &Param| -> Option<Vec<f64>> {
            let mut out = Vec::new();
            for (m, k) in m.as_list()?.iter().zip(k.as_list()?) {
                for _ in 0..m.as_int()?.clamp(0, 64) {
                    out.push(k.as_f64()?);
                }
            }
            Some(out)
        };
        let u_knots = expand(mults[0], knots[0])?;
        let v_knots = expand(mults[1], knots[1])?;
        // Weights are a grid of numbers shaped like the control net.
        let weights = e
            .part("RATIONAL_B_SPLINE_SURFACE")
            .and_then(|w| w.first())
            .and_then(Param::as_list)
            .map(|rows| rows.iter().flat_map(nums).collect::<Vec<f64>>())
            .filter(|w| w.len() == ctrl.len());
        SplineSurface::new((du, dv), (nu, nv), ctrl, weights, u_knots, v_knots)
    }

    pub fn surface(&self, id: u32) -> Surface {
        let Some(e) = self.file.entity(id) else {
            return Surface::Other;
        };
        if is_spline_surface(&e) {
            return Surface::BSpline;
        }
        let a = e.args();
        let fr = a.get(1).and_then(Param::as_ref).and_then(|r| self.frame(r));
        let num = |i: usize| a.get(i).and_then(Param::as_f64);
        let (Some(f), true) = (fr, !a.is_empty()) else {
            return Surface::Other;
        };
        match e.name() {
            "PLANE" => Surface::Plane(f),
            "CYLINDRICAL_SURFACE" => {
                num(2).map_or(Surface::Other, |r| Surface::Cylinder(f, r * self.scale))
            }
            "CONICAL_SURFACE" => match (num(2), num(3)) {
                (Some(r), Some(ang)) => Surface::Cone(f, r * self.scale, ang_rad(ang, self.file)),
                _ => Surface::Other,
            },
            "SPHERICAL_SURFACE" => {
                num(2).map_or(Surface::Other, |r| Surface::Sphere(f, r * self.scale))
            }
            "TOROIDAL_SURFACE" => match (num(2), num(3)) {
                (Some(r1), Some(r2)) => Surface::Torus(f, r1 * self.scale, r2 * self.scale),
                _ => Surface::Other,
            },
            _ => Surface::Other,
        }
    }

    /// Points of an `EDGE_CURVE` from its start vertex to its end vertex.
    pub fn edge_points(&mut self, id: u32) -> Option<Rc<Vec<V3>>> {
        if let Some(c) = self.edges.get(&id) {
            return Some(c.clone());
        }
        let pts = Rc::new(self.sample_edge(id)?);
        self.edges.insert(id, pts.clone());
        Some(pts)
    }

    fn vertex(&self, id: u32) -> Option<V3> {
        let e = self.file.entity(id)?;
        self.point(e.args().get(1)?.as_ref()?)
    }

    fn sample_edge(&self, id: u32) -> Option<Vec<V3>> {
        let e = self.file.entity(id)?;
        let a = e.args();
        let p1 = self.vertex(a.get(1)?.as_ref()?)?;
        let p2 = self.vertex(a.get(2)?.as_ref()?)?;
        let same = a.get(4).and_then(Param::as_bool).unwrap_or(true);
        let closed = dist(p1, p2) < 1e-6;
        let curve = self.curve(a.get(3)?.as_ref()?);
        let mid = |n: usize, f: &dyn Fn(usize) -> V3| -> Vec<V3> {
            let mut v = vec![p1];
            for i in 1..n {
                v.push(f(i));
            }
            v.push(p2);
            v
        };
        Some(match curve {
            Curve::Circle(f, r) => {
                let a1 = angle_in(&f, p1, 1.0, 1.0);
                let a2 = angle_in(&f, p2, 1.0, 1.0);
                let sweep = arc_sweep(a1, a2, same, closed);
                let n = arc_segments(sweep.abs(), r, self.tol);
                mid(n, &|i| {
                    let t = a1 + sweep * i as f64 / n as f64;
                    f.from_local([r * t.cos(), r * t.sin(), 0.0])
                })
            }
            Curve::Ellipse(f, ra, rb) => {
                let a1 = angle_in(&f, p1, ra, rb);
                let a2 = angle_in(&f, p2, ra, rb);
                let sweep = arc_sweep(a1, a2, same, closed);
                let n = arc_segments(sweep.abs(), ra.max(rb), self.tol);
                mid(n, &|i| {
                    let t = a1 + sweep * i as f64 / n as f64;
                    f.from_local([ra * t.cos(), rb * t.sin(), 0.0])
                })
            }
            Curve::Spline(s) => {
                let (lo, hi) = s.domain();
                let n = (s.ctrl.len() * 3).clamp(8, 200);
                let pts: Vec<V3> = (0..=n)
                    .map(|i| s.eval(lo + (hi - lo) * i as f64 / n as f64))
                    .collect();
                let near = |p: V3| {
                    (0..pts.len())
                        .min_by(|&i, &j| dist(pts[i], p).total_cmp(&dist(pts[j], p)))
                        .unwrap_or(0)
                };
                let (i1, i2) = (near(p1), near(p2));
                let mut out = vec![p1];
                if closed && i1 == i2 {
                    // A closed spline loop: walk the whole curve.
                    let it: Vec<usize> = if same {
                        (1..n).collect()
                    } else {
                        (1..n).rev().collect()
                    };
                    out.extend(it.into_iter().map(|i| pts[(i1 + i) % n]));
                } else if same == (i1 <= i2) {
                    let (s0, e0) = (i1.min(i2), i1.max(i2));
                    let inner: Vec<V3> = pts[s0 + 1..e0.max(s0 + 1)].to_vec();
                    if i1 <= i2 {
                        out.extend(inner);
                    } else {
                        out.extend(inner.into_iter().rev());
                    }
                }
                out.push(p2);
                out
            }
            Curve::Poly(v) if v.len() >= 2 => {
                let mut out = v;
                if !same {
                    out.reverse();
                }
                out
            }
            _ => vec![p1, p2],
        })
    }

    /// Loop polylines of a face boundary list.
    fn bound_points(&mut self, bound: u32) -> Option<Vec<V3>> {
        let e = self.file.entity(bound)?;
        let a = e.args();
        let lp = self.file.entity(a.get(1)?.as_ref()?)?;
        let outward = a.get(2).and_then(Param::as_bool).unwrap_or(true);
        let mut pts: Vec<V3> = Vec::new();
        match lp.name() {
            "EDGE_LOOP" => {
                for oe_id in lp.args().get(1)?.refs() {
                    let oe = self.file.entity(oe_id)?;
                    let oa = oe.args();
                    let ec = oa.get(3)?.as_ref()?;
                    let fwd = oa.get(4).and_then(Param::as_bool).unwrap_or(true);
                    let mut seg: Vec<V3> = self.edge_points(ec)?.as_ref().clone();
                    if !fwd {
                        seg.reverse();
                    }
                    // The next edge starts where this one ends.
                    seg.pop();
                    pts.extend(seg);
                }
            }
            "POLY_LOOP" => {
                for r in lp.args().get(1)?.refs() {
                    pts.push(self.point(r)?);
                }
            }
            _ => return None,
        }
        if !outward {
            pts.reverse();
        }
        pts.dedup_by(|a, b| dist(*a, *b) < 1e-9);
        if pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) < 1e-9 {
            pts.pop();
        }
        (pts.len() >= 3).then_some(pts)
    }

    pub fn face(&mut self, id: u32) -> Option<Face> {
        let e = self.file.entity(id)?;
        if !matches!(e.name(), "ADVANCED_FACE" | "FACE_SURFACE" | "FACE") {
            return None;
        }
        let a = e.args();
        let mut loops = Vec::new();
        for b in a.get(1)?.refs() {
            if let Some(l) = self.bound_points(b) {
                loops.push(l);
            }
        }
        if loops.is_empty() {
            return None;
        }
        let mut surface = a
            .get(2)
            .and_then(Param::as_ref)
            .map_or(Surface::Other, |r| self.surface(r));
        let mut nurbs = None;
        if matches!(surface, Surface::BSpline) {
            nurbs = a
                .get(2)
                .and_then(Param::as_ref)
                .and_then(|r| self.spline_surface(r))
                .map(Arc::new);
            if nurbs.is_none() {
                surface = Surface::Other;
            }
        }
        let same_sense = a.get(3).and_then(Param::as_bool).unwrap_or(true);
        Some(Face {
            loops,
            surface,
            nurbs,
            same_sense,
        })
    }
}

fn is_spline_surface(e: &Entity) -> bool {
    e.name() == "B_SPLINE_SURFACE_WITH_KNOTS" || e.has("B_SPLINE_SURFACE")
}

/// Angle values in files are radians unless the context says degrees;
/// every maker file written in degrees also declares PLANE_ANGLE_UNIT.
fn ang_rad(v: f64, f: &StepFile) -> f64 {
    let degrees = f.ids_of("PLANE_ANGLE_UNIT").iter().any(|&id| {
        f.entity(id).is_some_and(|e| {
            e.has("CONVERSION_BASED_UNIT")
                && e.part("CONVERSION_BASED_UNIT")
                    .and_then(|p| p.first())
                    .and_then(Param::as_text)
                    .is_some_and(|n| n.eq_ignore_ascii_case("DEGREE"))
        })
    });
    if degrees { v.to_radians() } else { v }
}

/// Parameter angle of `p` in the (elliptic) frame.
fn angle_in(f: &Frame, p: V3, ra: f64, rb: f64) -> f64 {
    let l = f.to_local(p);
    (l[1] / rb.max(1e-12)).atan2(l[0] / ra.max(1e-12))
}

/// Signed sweep from `a1` to `a2` following the edge direction.
fn arc_sweep(a1: f64, a2: f64, same: bool, closed: bool) -> f64 {
    if closed {
        return if same { TAU } else { -TAU };
    }
    if same {
        (a2 - a1).rem_euclid(TAU)
    } else {
        -(a1 - a2).rem_euclid(TAU)
    }
}

/// Segments for an arc of `sweep` radians: chord deviation within `tol`,
/// at most 30 degrees per segment.
fn arc_segments(sweep: f64, r: f64, tol: f64) -> usize {
    let step = if r <= tol {
        PI / 6.0
    } else {
        (2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos()).clamp(0.05, PI / 6.0)
    };
    ((sweep / step).ceil() as usize).clamp(2, 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_interpolates_ends_and_midpoint() {
        // Quadratic Bezier (0,0)-(1,2)-(2,0): midpoint (1,1).
        let s = Spline {
            degree: 2,
            ctrl: vec![[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0]],
            weights: None,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        };
        assert_eq!(s.eval(0.0), [0.0, 0.0, 0.0]);
        assert_eq!(s.eval(1.0), [2.0, 0.0, 0.0]);
        let m = s.eval(0.5);
        assert!((m[0] - 1.0).abs() < 1e-12 && (m[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn arcs_follow_edge_direction() {
        assert!((arc_sweep(0.0, 1.0, true, false) - 1.0).abs() < 1e-12);
        assert!((arc_sweep(0.0, 1.0, false, false) + (TAU - 1.0)).abs() < 1e-12);
        assert_eq!(arc_sweep(0.3, 0.3, true, true), TAU);
        assert!(arc_segments(TAU, 0.5, 0.05) >= 12);
    }
}
