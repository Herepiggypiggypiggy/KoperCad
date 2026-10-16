//! Geometry the turbulence models need: cell sizes, fluid mask, wall faces
//! and the wall distance. Metres. Axes 0 = x, 1 = y (down), 2 = z.
//!
//! Velocity arrives on the staggered (MAC) layout of the flow core:
//! component `c` has `n[c] + 1` samples along axis `c` and `n[a]` along the
//! others.

use crate::result::Grid3;

/// What a domain face is for the turbulence fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceKind {
    /// No-slip wall: wall functions apply.
    Wall,
    /// Symmetry or open face: no wall treatment, zero gradient.
    Symmetry,
}

/// A wall face of a fluid cell (domain wall or the face of a solid cell).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFace {
    pub cell: usize,
    /// Axis of the wall normal.
    pub axis: usize,
    /// `false` when the wall lies on the low side of the cell.
    pub high: bool,
    /// Distance from the cell centre to the wall, m.
    pub dist: f64,
    pub area: f64,
}

#[derive(Debug, Clone)]
pub struct TurbMesh {
    pub n: [usize; 3],
    pub edges: [Vec<f64>; 3],
    pub centre: [Vec<f64>; 3],
    pub dx: [Vec<f64>; 3],
    /// `true` for cells that hold fluid.
    pub fluid: Vec<bool>,
    pub walls: Vec<WallFace>,
    /// Distance to the nearest wall, m (0 in solid cells).
    pub wall_dist: Vec<f64>,
    pub face_kind: [FaceKind; 6],
    /// Boundary faces of cells that are open (a vent or fan): no wall there.
    /// Per cell, indexed `2 * axis + high`.
    pub open: Vec<[bool; 6]>,
    /// Extent of the domain per axis, m.
    pub size: [f64; 3],
}

impl TurbMesh {
    pub fn new(grid: &Grid3, solid: Option<&[bool]>, face_kind: [FaceKind; 6]) -> TurbMesh {
        let edges: [Vec<f64>; 3] = [
            grid.x_edges_mm.iter().map(|v| v * 1e-3).collect(),
            grid.y_edges_mm.iter().map(|v| v * 1e-3).collect(),
            grid.z_edges_mm.iter().map(|v| v * 1e-3).collect(),
        ];
        let n = [grid.nx, grid.ny, grid.nz];
        let size = std::array::from_fn(|a| edges[a][n[a]] - edges[a][0]);
        let centre =
            std::array::from_fn(|a| edges[a].windows(2).map(|w| 0.5 * (w[0] + w[1])).collect());
        let dx = std::array::from_fn(|a| edges[a].windows(2).map(|w| w[1] - w[0]).collect());
        let cells = grid.cells();
        let fluid: Vec<bool> = match solid {
            Some(s) => {
                assert_eq!(s.len(), cells, "solid mask length");
                s.iter().map(|s| !s).collect()
            }
            None => vec![true; cells],
        };
        let mut mesh = TurbMesh {
            n,
            edges,
            centre,
            dx,
            fluid,
            walls: Vec::new(),
            wall_dist: vec![0.0; cells],
            face_kind,
            open: vec![[false; 6]; cells],
            size,
        };
        mesh.find_walls();
        mesh.compute_wall_distance();
        mesh
    }

    /// Centre of cell `(i, j, k)`, m.
    pub fn centre_of(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [self.centre[0][i], self.centre[1][j], self.centre[2][k]]
    }

    /// Kind of the domain face `high` side of cell `i` along `axis`; an
    /// open face is no wall.
    pub fn boundary_kind(&self, c: usize, axis: usize, high: bool) -> FaceKind {
        let f = 2 * axis + usize::from(high);
        if self.open[c][f] {
            FaceKind::Symmetry
        } else {
            self.face_kind[f]
        }
    }

    /// Marks boundary faces as open: `(cell, axis, high)` of a vent or fan
    /// face. They stop being walls and leave the wall distance.
    pub fn mark_open(&mut self, faces: &[(usize, usize, bool)]) {
        for &(c, axis, high) in faces {
            self.open[c][2 * axis + usize::from(high)] = true;
        }
        self.find_walls();
        self.compute_wall_distance();
    }

    pub fn cells(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    pub fn cell(&self, i: [usize; 3]) -> usize {
        (i[2] * self.n[1] + i[1]) * self.n[0] + i[0]
    }

    pub fn ijk(&self, c: usize) -> [usize; 3] {
        let (nx, ny) = (self.n[0], self.n[1]);
        [c % nx, (c / nx) % ny, c / (nx * ny)]
    }

    pub fn volume(&self, i: [usize; 3]) -> f64 {
        self.dx[0][i[0]] * self.dx[1][i[1]] * self.dx[2][i[2]]
    }

    /// Area of the face of a cell normal to axis `a`.
    pub fn area(&self, a: usize, i: [usize; 3]) -> f64 {
        let (b, c) = match a {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        self.dx[b][i[b]] * self.dx[c][i[c]]
    }

    /// Sample counts of velocity component `c` (MAC layout).
    pub fn vel_dims(&self, c: usize) -> [usize; 3] {
        let mut d = self.n;
        d[c] += 1;
        d
    }

    pub fn vel_len(&self, c: usize) -> usize {
        let d = self.vel_dims(c);
        d[0] * d[1] * d[2]
    }

    pub fn vel_idx(&self, c: usize, i: [usize; 3]) -> usize {
        let d = self.vel_dims(c);
        (i[2] * d[1] + i[1]) * d[0] + i[0]
    }

    /// The neighbour of `i` along `axis` (`high` side or low side), if inside.
    pub fn neighbour(&self, i: [usize; 3], axis: usize, high: bool) -> Option<[usize; 3]> {
        let mut j = i;
        if high {
            if i[axis] + 1 >= self.n[axis] {
                return None;
            }
            j[axis] += 1;
        } else {
            if i[axis] == 0 {
                return None;
            }
            j[axis] -= 1;
        }
        Some(j)
    }

    /// Cell-centred velocity from the face velocities.
    pub fn cell_velocity(&self, vel: &[Vec<f64>; 3]) -> Vec<[f64; 3]> {
        let mut out = vec![[0.0; 3]; self.cells()];
        for k in 0..self.n[2] {
            for j in 0..self.n[1] {
                for i in 0..self.n[0] {
                    let ix = [i, j, k];
                    let c = self.cell(ix);
                    for a in 0..3 {
                        let mut hi = ix;
                        hi[a] += 1;
                        out[c][a] =
                            0.5 * (vel[a][self.vel_idx(a, ix)] + vel[a][self.vel_idx(a, hi)]);
                    }
                }
            }
        }
        out
    }

    fn find_walls(&mut self) {
        let mut walls = Vec::new();
        for c in 0..self.cells() {
            if !self.fluid[c] {
                continue;
            }
            let ix = self.ijk(c);
            for axis in 0..3 {
                for high in [false, true] {
                    let is_wall = match self.neighbour(ix, axis, high) {
                        Some(nb) => !self.fluid[self.cell(nb)],
                        None => self.boundary_kind(c, axis, high) == FaceKind::Wall,
                    };
                    if is_wall {
                        walls.push(WallFace {
                            cell: c,
                            axis,
                            high,
                            dist: 0.5 * self.dx[axis][ix[axis]],
                            area: self.area(axis, ix),
                        });
                    }
                }
            }
        }
        self.walls = walls;
    }

    /// Fast sweeping solution of `|grad d| = 1` seeded by the wall faces.
    /// Exact for planar walls aligned with the grid.
    fn compute_wall_distance(&mut self) {
        let cells = self.cells();
        let mut d = vec![f64::INFINITY; cells];
        for w in &self.walls {
            d[w.cell] = d[w.cell].min(w.dist);
        }
        // exact distance to the planar domain walls; the sweep below only
        // has to carry the solid cells
        let mut domain = vec![f64::INFINITY; cells];
        for c in 0..cells {
            let ix = self.ijk(c);
            for axis in 0..3 {
                for high in [false, true] {
                    if self.boundary_kind(c, axis, high) != FaceKind::Wall {
                        continue;
                    }
                    let dist = if high {
                        self.edges[axis][self.n[axis]] - self.centre[axis][ix[axis]]
                    } else {
                        self.centre[axis][ix[axis]] - self.edges[axis][0]
                    };
                    domain[c] = domain[c].min(dist);
                }
            }
        }
        let solid_seeds = self.walls.iter().any(|w| {
            let ix = self.ijk(w.cell);
            match self.neighbour(ix, w.axis, w.high) {
                Some(nb) => !self.fluid[self.cell(nb)],
                None => false,
            }
        });
        if !solid_seeds {
            for c in 0..cells {
                d[c] = if self.fluid[c] { domain[c] } else { 0.0 };
            }
        }
        if !d.iter().any(|v| v.is_finite()) {
            // no walls at all: a large distance keeps the models inert
            let big = self
                .edges
                .iter()
                .map(|e| e[e.len() - 1] - e[0])
                .fold(0.0, f64::max);
            for (c, v) in d.iter_mut().enumerate() {
                *v = if self.fluid[c] { big } else { 0.0 };
            }
            self.wall_dist = d;
            return;
        }
        let order = |n: usize, fwd: bool| -> Vec<usize> {
            if fwd {
                (0..n).collect()
            } else {
                (0..n).rev().collect()
            }
        };
        if solid_seeds {
            for _pass in 0..4 {
                let mut changed = false;
                for sweep in 0..8 {
                    let (fx, fy, fz) = (sweep & 1 == 0, sweep & 2 == 0, sweep & 4 == 0);
                    for k in order(self.n[2], fz) {
                        for j in order(self.n[1], fy) {
                            for i in order(self.n[0], fx) {
                                let ix = [i, j, k];
                                let c = self.cell(ix);
                                if !self.fluid[c] {
                                    continue;
                                }
                                let new = self.eikonal_update(&d, ix);
                                if new < d[c] - 1e-14 * new.abs().max(1e-12) {
                                    d[c] = new;
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        for (c, v) in d.iter_mut().enumerate() {
            *v = if self.fluid[c] { v.min(domain[c]) } else { 0.0 };
        }
        self.wall_dist = d;
    }

    fn eikonal_update(&self, d: &[f64], ix: [usize; 3]) -> f64 {
        // best fluid neighbour value and spacing per axis
        let mut terms: Vec<(f64, f64)> = Vec::with_capacity(3);
        for axis in 0..3 {
            let mut best: Option<(f64, f64)> = None;
            for high in [false, true] {
                if let Some(nb) = self.neighbour(ix, axis, high) {
                    let c = self.cell(nb);
                    if self.fluid[c] && d[c].is_finite() {
                        let h = (self.centre[axis][nb[axis]] - self.centre[axis][ix[axis]]).abs();
                        if best.is_none_or(|(a, _)| d[c] < a) {
                            best = Some((d[c], h));
                        }
                    }
                }
            }
            if let Some(b) = best {
                terms.push(b);
            }
        }
        terms.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut result = f64::INFINITY;
        for m in 1..=terms.len() {
            // solve sum_{t<m} ((x - a_t)/h_t)^2 = 1
            let (mut qa, mut qb, mut qc) = (0.0, 0.0, -1.0);
            for &(a, h) in &terms[..m] {
                let w = 1.0 / (h * h);
                qa += w;
                qb -= 2.0 * a * w;
                qc += a * a * w;
            }
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                break;
            }
            let x = (-qb + disc.sqrt()) / (2.0 * qa);
            // valid when x exceeds every value used
            if x > terms[m - 1].0 {
                result = x;
            } else {
                break;
            }
        }
        result
    }
}
