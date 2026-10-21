//! Helpers on a [`CfdSnapshot`]: slices for the plate and the 3D planes,
//! the 2D view of the device front plane, 3D streamlines and iso-surfaces
//! (spec `docs/design/thermal-visuals.md` sections 8 and 10).
//!
//! Frame of the snapshot: millimetres in the interior, `x` right, `y` down,
//! `z` from the mounting plate towards the door. Values sit at cell centres
//! and are blended trilinearly; solid cells are the ones whose material id
//! is not 0.

use novo_cfd::CfdSnapshot;

use crate::field::{Grid, Scalar, Vector};

/// The axis a slice plane is perpendicular to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// A slice plane: perpendicular to `axis`, `offset_mm` along it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub axis: Axis,
    pub offset_mm: f64,
}

/// The part of the plane to cut out, in the plane's own `(u, v)` frame:
/// `u` and `v` are the two other axes in the order x, y, z (so a plate-side
/// slice is `u = x`, `v = y`; a vertical one `u = z`, `v = y`; a horizontal
/// one `u = x`, `v = z`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub u0: f64,
    pub v0: f64,
    pub w: f64,
    pub h: f64,
}

/// A plane resampled onto a regular 2D grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub plane: Plane,
    pub window: Window,
    /// Degrees C.
    pub temperature: Scalar,
    /// Speed in m/s.
    pub speed: Scalar,
    /// The in-plane part of the velocity, m/s (`vx`, `vy` are along `u`, `v`).
    pub flow: Vector,
    /// True where the plane cuts a solid.
    pub solid: Vec<bool>,
}

impl Slice {
    /// Is the point (in the window frame, mm) inside a solid.
    pub fn is_solid(&self, u: f64, v: f64) -> bool {
        let g = self.temperature.grid;
        let (c, r) = g.cell_at(u - self.window.u0, v - self.window.v0);
        self.solid[r * g.cols + c]
    }
}

fn edges(snap: &CfdSnapshot, axis: Axis) -> &[f64] {
    match axis {
        Axis::X => &snap.grid.x_edges_mm,
        Axis::Y => &snap.grid.y_edges_mm,
        Axis::Z => &snap.grid.z_edges_mm,
    }
}

/// The whole plane of the snapshot as a window.
pub fn whole_window(snap: &CfdSnapshot, axis: Axis) -> Window {
    let (a, b) = match axis {
        Axis::Z => (Axis::X, Axis::Y),
        Axis::X => (Axis::Z, Axis::Y),
        Axis::Y => (Axis::X, Axis::Z),
    };
    let span = |ax: Axis| {
        let e = edges(snap, ax);
        (e[0], e[e.len() - 1])
    };
    let (ua, ub) = (span(a), span(b));
    Window {
        u0: ua.0,
        v0: ub.0,
        w: ua.1 - ua.0,
        h: ub.1 - ub.0,
    }
}

/// Cell index range around `x` and the blend between the two cell centres.
fn locate(edges: &[f64], x: f64) -> (usize, usize, f64) {
    let n = edges.len() - 1;
    let centre = |i: usize| 0.5 * (edges[i] + edges[i + 1]);
    if x <= centre(0) {
        return (0, 0, 0.0);
    }
    if x >= centre(n - 1) {
        return (n - 1, n - 1, 0.0);
    }
    // First cell whose centre lies beyond x.
    let (mut lo, mut hi) = (0, n - 1);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if centre(mid) <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (c0, c1) = (centre(lo), centre(hi));
    (lo, hi, (x - c0) / (c1 - c0))
}

/// The cell holding a point, clamped to the grid.
fn cell_of(edges: &[f64], x: f64) -> usize {
    let n = edges.len() - 1;
    edges[1..n].partition_point(|e| *e <= x)
}

pub(crate) struct Taps {
    pub(crate) idx: [usize; 8],
    pub(crate) w: [f32; 8],
}

pub(crate) fn taps(snap: &CfdSnapshot, p: [f64; 3]) -> Taps {
    let g = &snap.grid;
    let (i0, i1, tx) = locate(&g.x_edges_mm, p[0]);
    let (j0, j1, ty) = locate(&g.y_edges_mm, p[1]);
    let (k0, k1, tz) = locate(&g.z_edges_mm, p[2]);
    let (tx, ty, tz) = (tx as f32, ty as f32, tz as f32);
    let mut idx = [0; 8];
    let mut w = [0.0; 8];
    let mut n = 0;
    for (k, wk) in [(k0, 1.0 - tz), (k1, tz)] {
        for (j, wj) in [(j0, 1.0 - ty), (j1, ty)] {
            for (i, wi) in [(i0, 1.0 - tx), (i1, tx)] {
                idx[n] = g.index(i, j, k);
                w[n] = wi * wj * wk;
                n += 1;
            }
        }
    }
    Taps { idx, w }
}

/// Trilinear temperature at a point, degrees C.
pub fn temperature_at(snap: &CfdSnapshot, p: [f64; 3]) -> f32 {
    let t = taps(snap, p);
    (0..8).map(|n| snap.temperature_c[t.idx[n]] * t.w[n]).sum()
}

/// Trilinear velocity at a point, m/s.
pub fn velocity_at(snap: &CfdSnapshot, p: [f64; 3]) -> [f32; 3] {
    let t = taps(snap, p);
    let mut v = [0.0; 3];
    for n in 0..8 {
        for (c, out) in v.iter_mut().enumerate() {
            *out += snap.velocity[t.idx[n]][c] * t.w[n];
        }
    }
    v
}

/// Is the cell holding the point a solid.
pub fn solid_at(snap: &CfdSnapshot, p: [f64; 3]) -> bool {
    let g = &snap.grid;
    let i = cell_of(&g.x_edges_mm, p[0]);
    let j = cell_of(&g.y_edges_mm, p[1]);
    let k = cell_of(&g.z_edges_mm, p[2]);
    snap.material[g.index(i, j, k)] != 0
}

/// Is the point inside the interior.
pub fn inside(snap: &CfdSnapshot, p: [f64; 3]) -> bool {
    let within = |e: &[f64], x: f64| x >= e[0] && x <= e[e.len() - 1];
    within(&snap.grid.x_edges_mm, p[0])
        && within(&snap.grid.y_edges_mm, p[1])
        && within(&snap.grid.z_edges_mm, p[2])
}

/// Cuts `plane` out of the snapshot onto a grid with cells of about
/// `cell_mm`. A snapshot that is not consistent gives `None`.
pub fn slice(snap: &CfdSnapshot, plane: Plane, window: Window, cell_mm: f64) -> Option<Slice> {
    if !snap.is_consistent() || window.w <= 0.0 || window.h <= 0.0 || cell_mm <= 0.0 {
        return None;
    }
    let cols = ((window.w / cell_mm).ceil() as usize).max(1);
    let rows = ((window.h / cell_mm).ceil() as usize).max(1);
    let grid = Grid {
        cols,
        rows,
        cell_w: window.w / cols as f64,
        cell_h: window.h / rows as f64,
    };
    let point = |u: f64, v: f64| match plane.axis {
        Axis::Z => [u, v, plane.offset_mm],
        Axis::X => [plane.offset_mm, v, u],
        Axis::Y => [u, plane.offset_mm, v],
    };
    let n = cols * rows;
    let (mut t, mut s, mut fu, mut fv, mut solid) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for r in 0..rows {
        for c in 0..cols {
            let (du, dv) = grid.centre(c, r);
            let p = point(window.u0 + du, window.v0 + dv);
            let vel = velocity_at(snap, p);
            t.push(temperature_at(snap, p));
            s.push(vel[0].hypot(vel[1]).hypot(vel[2]));
            let (a, b) = match plane.axis {
                Axis::Z => (vel[0], vel[1]),
                Axis::X => (vel[2], vel[1]),
                Axis::Y => (vel[0], vel[2]),
            };
            fu.push(a);
            fv.push(b);
            solid.push(solid_at(snap, p));
        }
    }
    Some(Slice {
        plane,
        window,
        temperature: Scalar::try_new(grid, t).ok()?,
        speed: Scalar::try_new(grid, s).ok()?,
        flow: Vector::try_new(grid, fu, fv).ok()?,
        solid,
    })
}

/// The `z` of the plane just in front of the devices (towards the door):
/// the median over the devices of their frontmost solid cell, plus half a
/// cell. Without devices a quarter of the depth.
pub fn front_plane_z(snap: &CfdSnapshot) -> f64 {
    let g = &snap.grid;
    let mut fronts: Vec<usize> = Vec::new();
    let mut seen: Vec<u16> = Vec::new();
    for k in (0..g.nz).rev() {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let m = snap.material[g.index(i, j, k)];
                let is_device = snap
                    .materials
                    .iter()
                    .any(|mi| mi.id == m && matches!(mi.kind, novo_cfd::MaterialKind::Device(_)));
                if is_device && !seen.contains(&m) {
                    seen.push(m);
                    fronts.push(k);
                }
            }
        }
    }
    if fronts.is_empty() {
        let e = &g.z_edges_mm;
        return 0.25 * (e[g.nz] - e[0]) + e[0];
    }
    fronts.sort_unstable();
    let k = fronts[fronts.len() / 2];
    (g.z_edges_mm[k + 1] + 0.5 * (g.z_edges_mm[(k + 2).min(g.nz)] - g.z_edges_mm[k + 1]))
        .min(g.z_edges_mm[g.nz])
}

/// The 2D view the plate shows: the slice at the device front plane over
/// `window` (the plate rectangle in the interior frame).
pub fn as_2d(snap: &CfdSnapshot, window: Window, cell_mm: f64) -> Option<Slice> {
    slice(
        snap,
        Plane {
            axis: Axis::Z,
            offset_mm: front_plane_z(snap),
        },
        window,
        cell_mm,
    )
}

/// How a streamline is traced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trace {
    pub step_mm: f64,
    pub max_steps: usize,
    /// Slower air than this ends the line, m/s.
    pub min_speed: f64,
}

impl Default for Trace {
    fn default() -> Self {
        Trace {
            step_mm: 8.0,
            max_steps: 400,
            min_speed: 0.002,
        }
    }
}

/// Streamlines from `seeds` (mm): midpoint steps downstream until the line
/// leaves the interior, enters a solid, stalls or runs out of steps. A seed
/// in still air or in a solid gives a one point line.
pub fn streamlines(snap: &CfdSnapshot, seeds: &[[f64; 3]], trace: Trace) -> Vec<Vec<[f64; 3]>> {
    trace_field(snap, seeds, trace, |p| {
        let v = velocity_at(snap, p).map(f64::from);
        let s = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (s >= trace.min_speed).then(|| [v[0] / s, v[1] / s, v[2] / s])
    })
}

/// The tracer behind [`streamlines`] for any direction field: `dir` gives the
/// unit direction at a point, or `None` where the field is too weak to follow.
pub fn trace_field(
    snap: &CfdSnapshot,
    seeds: &[[f64; 3]],
    trace: Trace,
    dir: impl Fn([f64; 3]) -> Option<[f64; 3]>,
) -> Vec<Vec<[f64; 3]>> {
    if !snap.is_consistent() {
        return Vec::new();
    }
    seeds
        .iter()
        .map(|&seed| {
            let mut line = vec![seed];
            let mut p = seed;
            if !inside(snap, p) || solid_at(snap, p) {
                return line;
            }
            for _ in 0..trace.max_steps {
                let Some(d) = dir(p) else { break };
                let half = trace.step_mm * 0.5;
                let mid = [p[0] + d[0] * half, p[1] + d[1] * half, p[2] + d[2] * half];
                if !inside(snap, mid) {
                    break;
                }
                let Some(m) = dir(mid) else { break };
                let next = [
                    p[0] + m[0] * trace.step_mm,
                    p[1] + m[1] * trace.step_mm,
                    p[2] + m[2] * trace.step_mm,
                ];
                if !inside(snap, next) || solid_at(snap, next) {
                    break;
                }
                line.push(next);
                p = next;
            }
            line
        })
        .collect()
}

/// A triangle in mm.
pub type Triangle = [[f32; 3]; 3];

/// The surface where the temperature equals `level`, as triangles
/// (marching tetrahedra over the lattice of cell centres, so no case table
/// and no ambiguous faces). Empty when the level is outside the field.
pub fn iso_surface(snap: &CfdSnapshot, level: f32) -> Vec<Triangle> {
    if !snap.is_consistent() || snap.grid.nx < 2 || snap.grid.ny < 2 || snap.grid.nz < 2 {
        return Vec::new();
    }
    let g = &snap.grid;
    // Six tetrahedra around the cube diagonal 0-7; corners by bits x|y<<1|z<<2.
    const TETS: [[usize; 4]; 6] = [
        [0, 1, 3, 7],
        [0, 3, 2, 7],
        [0, 2, 6, 7],
        [0, 6, 4, 7],
        [0, 4, 5, 7],
        [0, 5, 1, 7],
    ];
    let mut out = Vec::new();
    for k in 0..g.nz - 1 {
        for j in 0..g.ny - 1 {
            for i in 0..g.nx - 1 {
                let mut pos = [[0.0f32; 3]; 8];
                let mut val = [0.0f32; 8];
                for c in 0..8 {
                    let (ci, cj, ck) = (i + (c & 1), j + ((c >> 1) & 1), k + (c >> 2));
                    let q = g.centre_mm(ci, cj, ck);
                    pos[c] = [q[0] as f32, q[1] as f32, q[2] as f32];
                    val[c] = snap.temperature_c[g.index(ci, cj, ck)];
                }
                let (lo, hi) = val
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
                if level < lo || level > hi || lo == hi {
                    continue;
                }
                for tet in TETS {
                    tetra(&pos, &val, tet, level, &mut out);
                }
            }
        }
    }
    out
}

fn tetra(
    pos: &[[f32; 3]; 8],
    val: &[f32; 8],
    tet: [usize; 4],
    level: f32,
    out: &mut Vec<Triangle>,
) {
    let (inside, outside): (Vec<usize>, Vec<usize>) = tet.iter().partition(|c| val[**c] >= level);
    let cut = |a: usize, b: usize| -> [f32; 3] {
        let t = (level - val[a]) / (val[b] - val[a]);
        [
            pos[a][0] + (pos[b][0] - pos[a][0]) * t,
            pos[a][1] + (pos[b][1] - pos[a][1]) * t,
            pos[a][2] + (pos[b][2] - pos[a][2]) * t,
        ]
    };
    match (inside.len(), outside.len()) {
        (1, 3) => out.push([
            cut(inside[0], outside[0]),
            cut(inside[0], outside[1]),
            cut(inside[0], outside[2]),
        ]),
        (3, 1) => out.push([
            cut(outside[0], inside[0]),
            cut(outside[0], inside[1]),
            cut(outside[0], inside[2]),
        ]),
        (2, 2) => {
            let a = cut(inside[0], outside[0]);
            let b = cut(inside[0], outside[1]);
            let c = cut(inside[1], outside[1]);
            let d = cut(inside[1], outside[0]);
            out.push([a, b, c]);
            out.push([a, c, d]);
        }
        _ => {}
    }
}

/// Area of a triangle list, mm2 (for tests and a size readout).
pub fn area_mm2(tris: &[Triangle]) -> f64 {
    tris.iter()
        .map(|t| {
            let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
            let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
            let c = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            0.5 * f64::from((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt())
        })
        .sum()
}
