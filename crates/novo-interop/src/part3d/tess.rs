//! Face tessellation: boundary loops to triangles.
//!
//! Curved faces (cylinder, cone, sphere, torus) are triangulated in the
//! surface's (u, v) parameter space and then refined by splitting interior
//! edges whose chord strays from the surface by more than the tolerance.
//! Faces on other surfaces (B-spline, swept) are triangulated flat from
//! their boundary and counted as approximated.

use std::collections::{HashMap, HashSet};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::brep::{Face, Surface};
use super::math::*;

/// Triangles with their own vertex list (positions in millimetres).
#[derive(Debug, Clone, Default)]
pub struct Soup {
    pub pos: Vec<V3>,
    pub tris: Vec<[u32; 3]>,
}

impl Soup {
    /// Appends `other` moved by `xf`; a mirroring transform flips winding.
    pub fn append(&mut self, other: &Soup, xf: &Xf) {
        let base = self.pos.len() as u32;
        self.pos.extend(other.pos.iter().map(|&p| xf.apply(p)));
        let det = dot(cross(xf.cols[0], xf.cols[1]), xf.cols[2]);
        self.tris.extend(other.tris.iter().map(|t| {
            if det < 0.0 {
                [base + t[0], base + t[2], base + t[1]]
            } else {
                [base + t[0], base + t[1], base + t[2]]
            }
        }));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Triangulated on its true surface.
    Exact,
    /// Triangulated flat from the boundary.
    Approx,
    /// Nothing could be made of it.
    Skipped,
}

pub fn tessellate(face: &Face, tol: f64) -> (Soup, Outcome) {
    if !matches!(face.surface, Surface::Other)
        && let Some(s) = uv_path(face, tol)
    {
        return (s, Outcome::Exact);
    }
    match flat_path(face) {
        Some(s) => (s, Outcome::Approx),
        None => (Soup::default(), Outcome::Skipped),
    }
}

// ---- surface parameterisation ------------------------------------------

fn eval(s: &Surface, u: f64, v: f64) -> V3 {
    match *s {
        Surface::Plane(f) => f.from_local([u, v, 0.0]),
        Surface::Cylinder(f, r) => f.from_local([r * u.cos(), r * u.sin(), v]),
        Surface::Cone(f, r, a) => {
            let rho = r + v * a.tan();
            f.from_local([rho * u.cos(), rho * u.sin(), v])
        }
        Surface::Sphere(f, r) => {
            f.from_local([r * v.cos() * u.cos(), r * v.cos() * u.sin(), r * v.sin()])
        }
        Surface::Torus(f, big, small) => {
            let rho = big + small * v.cos();
            f.from_local([rho * u.cos(), rho * u.sin(), small * v.sin()])
        }
        Surface::Other => [0.0; 3],
    }
}

/// Raw (u, v) of a point; angles in (-pi, pi].
fn project(s: &Surface, p: V3) -> [f64; 2] {
    match *s {
        Surface::Plane(f) => {
            let l = f.to_local(p);
            [l[0], l[1]]
        }
        Surface::Cylinder(f, _) | Surface::Cone(f, _, _) => {
            let l = f.to_local(p);
            [l[1].atan2(l[0]), l[2]]
        }
        Surface::Sphere(f, _) => {
            let l = f.to_local(p);
            [l[1].atan2(l[0]), l[2].atan2(l[0].hypot(l[1]))]
        }
        Surface::Torus(f, big, _) => {
            let l = f.to_local(p);
            let h = l[0].hypot(l[1]);
            [l[1].atan2(l[0]), l[2].atan2(h - big)]
        }
        Surface::Other => [0.0; 2],
    }
}

fn wrap(d: f64) -> f64 {
    let mut d = d % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    d
}

/// A loop in parameter space, continuous along the loop, with the total
/// u winding (0 or a multiple of 2 pi) including the closing step.
struct UvLoop {
    uv: Vec<[f64; 2]>,
    pos: Vec<V3>,
    winding: i32,
}

fn unwrap_loop(s: &Surface, pts: &[V3]) -> Option<UvLoop> {
    if matches!(s, Surface::Plane(_)) {
        return Some(UvLoop {
            uv: pts.iter().map(|&p| project(s, p)).collect(),
            pos: pts.to_vec(),
            winding: 0,
        });
    }
    let torus = matches!(s, Surface::Torus(..));
    let mut uv: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for &p in pts {
        let mut q = project(s, p);
        if let Some(prev) = uv.last() {
            q[0] = prev[0] + wrap(q[0] - prev[0]);
            if matches!(s, Surface::Sphere(..)) || torus {
                q[1] = prev[1] + wrap(q[1] - prev[1]);
            }
        }
        uv.push(q);
    }
    let first = uv[0];
    let last = *uv.last()?;
    let close_u = wrap(first[0] - last[0]);
    let total_u = (last[0] + close_u) - first[0];
    let winding = (total_u / TAU).round() as i32;
    if torus {
        let close_v = wrap(first[1] - last[1]);
        if ((last[1] + close_v - first[1]) / TAU).round() != 0.0 {
            return None;
        }
    }
    Some(UvLoop {
        uv,
        pos: pts.to_vec(),
        winding,
    })
}

fn area2(uv: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for i in 0..uv.len() {
        let p = uv[i];
        let q = uv[(i + 1) % uv.len()];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a
}

fn uv_path(face: &Face, tol: f64) -> Option<Soup> {
    let s = &face.surface;
    let mut loops: Vec<UvLoop> = Vec::new();
    for l in &face.loops {
        loops.push(unwrap_loop(s, l)?);
    }
    let periodic: Vec<usize> = (0..loops.len())
        .filter(|&i| loops[i].winding != 0)
        .collect();
    let mut soup = match (s, periodic.len()) {
        (_, 0) => polygon(s, loops, tol)?,
        (Surface::Cylinder(..) | Surface::Cone(..), 2) if loops.len() == 2 => {
            strip(loops, periodic)?
        }
        (Surface::Sphere(..), 1) => {
            let i = periodic[0];
            let mut l = loops.swap_remove(i);
            // The patch lies to the left of the loop seen from the face's
            // outward side: north for an eastward loop on an outward sphere.
            let eff = l.winding.signum() * if face.same_sense { 1 } else { -1 };
            let pole = if eff > 0 { FRAC_PI_2 } else { -FRAC_PI_2 };
            let (u0, v0) = (l.uv[0][0], l.uv[0][1]);
            let u1 = u0 + f64::from(l.winding) * TAU;
            for q in [[u1, v0], [u1, pole], [u0, pole]] {
                l.pos.push(eval(s, q[0], q[1]));
                l.uv.push(q);
            }
            l.winding = 0;
            let mut all = vec![l];
            all.extend(loops);
            polygon(s, all, tol)?
        }
        _ => return None,
    };
    // The loops run counter-clockwise around the outward normal with the
    // face on their left, so a boundary edge a -> b appears as a -> b in a
    // triangle that is wound outward. That is more reliable than the sign
    // of the surface normal (a spindle torus has the normal turned inside
    // out where it crosses its axis); the surface sense only decides when
    // no boundary edge survives to vote.
    let (agree, disagree) = orientation_votes(&face.loops, &soup);
    let flip = if agree + disagree > 0 {
        disagree > agree
    } else {
        !face.same_sense
    };
    if flip {
        for t in &mut soup.tris {
            t.swap(1, 2);
        }
    }
    Some(soup)
}

fn key(p: V3) -> [u64; 3] {
    [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
}

/// How many boundary edges run the same way in the triangles as in the
/// loops, and how many the opposite way.
fn orientation_votes(loops: &[Vec<V3>], soup: &Soup) -> (usize, usize) {
    let mut directed: HashSet<([u64; 3], [u64; 3])> = HashSet::new();
    for t in &soup.tris {
        for k in 0..3 {
            directed.insert((
                key(soup.pos[t[k] as usize]),
                key(soup.pos[t[(k + 1) % 3] as usize]),
            ));
        }
    }
    let (mut agree, mut disagree) = (0, 0);
    for l in loops {
        for i in 0..l.len() {
            let (a, b) = (key(l[i]), key(l[(i + 1) % l.len()]));
            if directed.contains(&(a, b)) {
                agree += 1;
            }
            if directed.contains(&(b, a)) {
                disagree += 1;
            }
        }
    }
    (agree, disagree)
}

/// Outer loop, holes and ear clipping in parameter space, then refinement.
fn polygon(s: &Surface, mut loops: Vec<UvLoop>, tol: f64) -> Option<Soup> {
    let outer = (0..loops.len()).max_by(|&a, &b| {
        area2(&loops[a].uv)
            .abs()
            .total_cmp(&area2(&loops[b].uv).abs())
    })?;
    let periodic = !matches!(s, Surface::Plane(_));
    let cu = loops[outer].uv.iter().map(|p| p[0]).sum::<f64>() / loops[outer].uv.len() as f64;
    let mut flat: Vec<f64> = Vec::new();
    let mut pos: Vec<V3> = Vec::new();
    let mut uvs: Vec<[f64; 2]> = Vec::new();
    let mut hole_starts: Vec<usize> = Vec::new();
    let mut fixed: HashSet<(u32, u32)> = HashSet::new();
    let order: Vec<usize> = std::iter::once(outer)
        .chain((0..loops.len()).filter(|&i| i != outer))
        .collect();
    for (k, &i) in order.iter().enumerate() {
        let l = &mut loops[i];
        if periodic && k > 0 {
            let m = l.uv.iter().map(|p| p[0]).sum::<f64>() / l.uv.len() as f64;
            let shift = ((cu - m) / TAU).round() * TAU;
            for p in &mut l.uv {
                p[0] += shift;
            }
        }
        let a = area2(&l.uv);
        let want_ccw = k == 0;
        if (a > 0.0) != want_ccw {
            l.uv.reverse();
            l.pos.reverse();
        }
        if k > 0 {
            hole_starts.push(uvs.len());
        }
        let base = uvs.len() as u32;
        let n = l.uv.len() as u32;
        for j in 0..n {
            fixed.insert((base + j, base + (j + 1) % n));
            fixed.insert((base + (j + 1) % n, base + j));
        }
        for (q, &p) in l.uv.iter().zip(&l.pos) {
            flat.push(q[0]);
            flat.push(q[1]);
            uvs.push(*q);
            pos.push(p);
        }
    }
    let idx = earcutr::earcut(&flat, &hole_starts, 2).ok()?;
    if idx.len() < 3 || idx.len() % 3 != 0 {
        return None;
    }
    let mut tris: Vec<[u32; 3]> = idx
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| {
            let (a, b, c) = (c[0], c[1], c[2]);
            let cr = (uvs[b][0] - uvs[a][0]) * (uvs[c][1] - uvs[a][1])
                - (uvs[b][1] - uvs[a][1]) * (uvs[c][0] - uvs[a][0]);
            if cr < 0.0 {
                [a as u32, c as u32, b as u32]
            } else {
                [a as u32, b as u32, c as u32]
            }
        })
        .collect();
    if !matches!(s, Surface::Plane(_)) {
        // Triangles with no area in parameter space (three corners on one
        // seam or boundary line) carry nothing and only confuse welding.
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for q in &uvs {
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let floor = 1e-10 * ((hi[0] - lo[0]) * (hi[1] - lo[1])).max(1e-12);
        tris.retain(|t| {
            let (a, b, c) = (uvs[t[0] as usize], uvs[t[1] as usize], uvs[t[2] as usize]);
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() > floor
        });
        refine(s, &mut uvs, &mut pos, &mut tris, &fixed, tol);
    }
    Some(Soup { pos, tris })
}

/// Sag-driven refinement: interior edges whose midpoint on the surface is
/// farther than `tol` from the chord are split; neighbours agree because
/// the decision depends only on the edge.
fn refine(
    s: &Surface,
    uv: &mut Vec<[f64; 2]>,
    pos: &mut Vec<V3>,
    tris: &mut Vec<[u32; 3]>,
    fixed: &HashSet<(u32, u32)>,
    tol: f64,
) {
    const MAX_TRIS: usize = 12_000;
    for _ in 0..6 {
        if tris.len() > MAX_TRIS {
            return;
        }
        let mut mids: HashMap<(u32, u32), Option<u32>> = HashMap::new();
        let mut any = false;
        for t in tris.iter() {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                if mids.contains_key(&key) {
                    continue;
                }
                let mut m = None;
                if !fixed.contains(&key) {
                    let (qa, qb) = (uv[a as usize], uv[b as usize]);
                    let mq = [(qa[0] + qb[0]) * 0.5, (qa[1] + qb[1]) * 0.5];
                    let mp = eval(s, mq[0], mq[1]);
                    let chord = lerp(pos[a as usize], pos[b as usize], 0.5);
                    if dist(mp, chord) > tol {
                        uv.push(mq);
                        pos.push(mp);
                        m = Some((uv.len() - 1) as u32);
                        any = true;
                    }
                }
                mids.insert(key, m);
            }
        }
        if !any {
            return;
        }
        let mut next = Vec::with_capacity(tris.len() * 2);
        for t in tris.iter() {
            let m = |k: usize| {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                mids[&(a.min(b), a.max(b))]
            };
            let (m0, m1, m2) = (m(0), m(1), m(2));
            let (v0, v1, v2) = (t[0], t[1], t[2]);
            match (m0, m1, m2) {
                (None, None, None) => next.push(*t),
                (Some(a), None, None) => next.extend([[v0, a, v2], [a, v1, v2]]),
                (None, Some(a), None) => next.extend([[v1, a, v0], [a, v2, v0]]),
                (None, None, Some(a)) => next.extend([[v2, a, v1], [a, v0, v1]]),
                (Some(a), Some(b), None) => {
                    next.extend([[a, v1, b], [v0, a, b], [v0, b, v2]]);
                }
                (None, Some(b), Some(c)) => {
                    next.extend([[b, v2, c], [v1, b, c], [v1, c, v0]]);
                }
                (Some(a), None, Some(c)) => {
                    next.extend([[c, v0, a], [v2, c, a], [v2, a, v1]]);
                }
                (Some(a), Some(b), Some(c)) => {
                    next.extend([[v0, a, c], [a, v1, b], [c, b, v2], [a, b, c]]);
                }
            }
        }
        *tris = next;
    }
}

/// Two loops around a full cylinder or cone, no seam: a strip between them.
fn strip(loops: Vec<UvLoop>, periodic: Vec<usize>) -> Option<Soup> {
    let mut chains: Vec<Vec<(f64, f64, V3)>> = Vec::new();
    for &i in &periodic {
        let l = &loops[i];
        let mut c: Vec<(f64, f64, V3)> =
            l.uv.iter()
                .zip(&l.pos)
                .map(|(q, &p)| (q[0], q[1], p))
                .collect();
        if l.winding < 0 {
            c.reverse();
        }
        if l.winding.abs() != 1 {
            return None;
        }
        // Increasing u, closed by repeating the start one turn later.
        let first = c[0];
        c.push((first.0 + TAU, first.1, first.2));
        if c.windows(2).any(|w| w[1].0 < w[0].0 - 1e-9) {
            return None;
        }
        chains.push(c);
    }
    let mean_v = |c: &Vec<(f64, f64, V3)>| c.iter().map(|p| p.1).sum::<f64>() / c.len() as f64;
    if mean_v(&chains[0]) > mean_v(&chains[1]) {
        chains.swap(0, 1);
    }
    let (lower, upper) = (&chains[0], &chains[1]);
    // Move the upper chain by whole turns so it starts within one turn
    // after the lower one.
    let shift = ((lower[0].0 - upper[0].0) / TAU).ceil() * TAU;
    let up: Vec<(f64, V3)> = upper.iter().map(|p| (p.0 + shift, p.2)).collect();
    let n = lower.len() - 1;
    let mut lo: Vec<(f64, V3)> = lower.iter().map(|p| (p.0, p.2)).collect();
    lo.extend(lower[1..].iter().map(|p| (p.0 + TAU, p.2)));
    let b0 = up[0].0;
    let k = (0..lo.len()).rev().find(|&i| lo[i].0 <= b0 + 1e-9)?;
    if k + n >= lo.len() {
        return None;
    }
    let mut pos: Vec<V3> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut index: HashMap<[u64; 3], u32> = HashMap::new();
    let mut vid = |p: V3, pos: &mut Vec<V3>| -> u32 {
        let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
        *index.entry(key).or_insert_with(|| {
            pos.push(p);
            (pos.len() - 1) as u32
        })
    };
    let (mut i, mut j) = (k, 0usize);
    let (iend, jend) = (k + n, up.len() - 1);
    while i < iend || j < jend {
        let ua = if i < iend { lo[i + 1].0 } else { f64::INFINITY };
        let ub = if j < jend { up[j + 1].0 } else { f64::INFINITY };
        let t = if ua <= ub {
            let t = [
                vid(lo[i].1, &mut pos),
                vid(lo[i + 1].1, &mut pos),
                vid(up[j].1, &mut pos),
            ];
            i += 1;
            t
        } else {
            let t = [
                vid(lo[i].1, &mut pos),
                vid(up[j + 1].1, &mut pos),
                vid(up[j].1, &mut pos),
            ];
            j += 1;
            t
        };
        tris.push(t);
    }
    Some(Soup { pos, tris })
}

/// Flat triangulation from the boundary, for surfaces we do not evaluate.
/// The outward direction comes from the loop direction (Newell normal).
fn flat_path(face: &Face) -> Option<Soup> {
    let newell = |l: &Vec<V3>| {
        let mut n = [0.0; 3];
        for i in 0..l.len() {
            n = add(n, cross(l[i], l[(i + 1) % l.len()]));
        }
        n
    };
    let outer = (0..face.loops.len())
        .max_by(|&a, &b| len(newell(&face.loops[a])).total_cmp(&len(newell(&face.loops[b]))))?;
    let n = newell(&face.loops[outer]);
    if len(n) < 1e-12 {
        return None;
    }
    let n = unit(n);
    let fr = Frame::from_axes([0.0; 3], n, None);
    let mut loops: Vec<UvLoop> = Vec::new();
    for l in &face.loops {
        let uv: Vec<[f64; 2]> = l
            .iter()
            .map(|&p| {
                let q = fr.to_local(p);
                [q[0], q[1]]
            })
            .collect();
        loops.push(UvLoop {
            uv,
            pos: l.clone(),
            winding: 0,
        });
    }
    polygon(&Surface::Plane(fr), loops, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane_face(loops: Vec<Vec<V3>>) -> Face {
        Face {
            loops,
            surface: Surface::Plane(Frame::IDENTITY),
            same_sense: true,
        }
    }

    fn area(s: &Soup) -> f64 {
        s.tris
            .iter()
            .map(|t| {
                len(cross(
                    sub(s.pos[t[1] as usize], s.pos[t[0] as usize]),
                    sub(s.pos[t[2] as usize], s.pos[t[0] as usize]),
                )) / 2.0
            })
            .sum()
    }

    fn normal_z(s: &Soup) -> f64 {
        s.tris
            .iter()
            .map(|t| {
                cross(
                    sub(s.pos[t[1] as usize], s.pos[t[0] as usize]),
                    sub(s.pos[t[2] as usize], s.pos[t[0] as usize]),
                )[2]
            })
            .sum()
    }

    #[test]
    fn square_with_a_hole() {
        let outer = vec![[0., 0., 0.], [10., 0., 0.], [10., 10., 0.], [0., 10., 0.]];
        // Listed counter-clockwise on purpose: the hole is forced clockwise.
        let hole = vec![[4., 4., 0.], [6., 4., 0.], [6., 6., 0.], [4., 6., 0.]];
        let (s, o) = tessellate(&plane_face(vec![outer, hole]), 0.05);
        assert_eq!(o, Outcome::Exact);
        assert!((area(&s) - 96.0).abs() < 1e-9);
        assert!(normal_z(&s) > 0.0);
    }

    #[test]
    fn the_loop_direction_decides_which_way_a_face_looks() {
        let outer = vec![[0., 0., 0.], [10., 0., 0.], [10., 10., 0.], [0., 10., 0.]];
        let mut f = plane_face(vec![outer.clone()]);
        // Counter-clockwise seen from +z: faces +z, whatever the surface says.
        f.same_sense = false;
        assert!(normal_z(&tessellate(&f, 0.05).0) > 0.0);
        let mut back = outer;
        back.reverse();
        assert!(normal_z(&tessellate(&plane_face(vec![back]), 0.05).0) < 0.0);
    }

    fn circle(r: f64, z: f64, n: usize) -> Vec<V3> {
        (0..n)
            .map(|i| {
                let t = TAU * i as f64 / n as f64;
                [r * t.cos(), r * t.sin(), z]
            })
            .collect()
    }

    #[test]
    fn cylinder_wall_from_two_circles_is_closed_and_outward() {
        let f = Face {
            loops: vec![circle(5.0, 0.0, 24), circle(5.0, 20.0, 24)],
            surface: Surface::Cylinder(Frame::IDENTITY, 5.0),
            same_sense: true,
        };
        let (s, o) = tessellate(&f, 0.05);
        assert_eq!(o, Outcome::Exact);
        assert_eq!(s.tris.len(), 48);
        // Polygon prism wall: 24 * chord * height.
        let chord = 2.0 * 5.0 * (PI / 24.0).sin();
        assert!((area(&s) - 24.0 * chord * 20.0).abs() < 1e-6);
        // Outward: every triangle normal points away from the axis.
        for t in &s.tris {
            let (a, b, c) = (
                s.pos[t[0] as usize],
                s.pos[t[1] as usize],
                s.pos[t[2] as usize],
            );
            let n = cross(sub(b, a), sub(c, a));
            let m = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, 0.0];
            assert!(dot(n, m) > 0.0);
        }
    }

    #[test]
    fn hemisphere_cap_bulges_to_the_surface() {
        // Latitude 0 circle of a sphere, face on the north side.
        let f = Face {
            loops: vec![circle(10.0, 0.0, 32)],
            surface: Surface::Sphere(Frame::IDENTITY, 10.0),
            same_sense: true,
        };
        let (s, o) = tessellate(&f, 0.05);
        assert_eq!(o, Outcome::Exact);
        let top = s.pos.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
        assert!((top - 10.0).abs() < 1e-6, "top {top}");
        assert!(s.pos.iter().all(|p| p[2] > -1e-9));
    }
}
