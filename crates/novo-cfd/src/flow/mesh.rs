//! Geometry of the staggered (MAC) grid, in metres.
//!
//! Pressure and temperature live at cell centres. Velocity component `c`
//! lives on the faces normal to axis `c`: it has `n[c] + 1` samples along
//! axis `c` (at the cell edges) and `n[a]` along the other axes (at cell
//! centres). Axes are 0 = x, 1 = y (down), 2 = z.

use crate::result::Grid3;

#[derive(Debug, Clone)]
pub struct Mesh {
    pub n: [usize; 3],
    /// Cell edges per axis, m (`n + 1` values).
    pub edges: [Vec<f64>; 3],
    /// Cell centres per axis, m.
    pub centre: [Vec<f64>; 3],
    /// Cell widths per axis, m.
    pub dx: [Vec<f64>; 3],
}

impl Mesh {
    pub fn from_grid(g: &Grid3) -> Mesh {
        Mesh::from_edges_m([
            g.x_edges_mm.iter().map(|v| v * 1e-3).collect(),
            g.y_edges_mm.iter().map(|v| v * 1e-3).collect(),
            g.z_edges_mm.iter().map(|v| v * 1e-3).collect(),
        ])
    }

    pub fn from_edges_m(edges: [Vec<f64>; 3]) -> Mesh {
        let n = [edges[0].len() - 1, edges[1].len() - 1, edges[2].len() - 1];
        let centre =
            std::array::from_fn(|a| edges[a].windows(2).map(|w| 0.5 * (w[0] + w[1])).collect());
        let dx = std::array::from_fn(|a| edges[a].windows(2).map(|w| w[1] - w[0]).collect());
        Mesh {
            n,
            edges,
            centre,
            dx,
        }
    }

    /// Uniform mesh over `size_m`.
    pub fn uniform(n: [usize; 3], size_m: [f64; 3]) -> Mesh {
        Mesh::from_edges_m(std::array::from_fn(|a| {
            (0..=n[a])
                .map(|i| size_m[a] * i as f64 / n[a] as f64)
                .collect()
        }))
    }

    pub fn cells(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// Sample counts of velocity component `c`.
    pub fn dims(&self, c: usize) -> [usize; 3] {
        let mut d = self.n;
        d[c] += 1;
        d
    }

    pub fn len(&self, c: usize) -> usize {
        let d = self.dims(c);
        d[0] * d[1] * d[2]
    }

    pub fn idx(&self, c: usize, i: [usize; 3]) -> usize {
        let d = self.dims(c);
        (i[2] * d[1] + i[1]) * d[0] + i[0]
    }

    /// Cell-centred index.
    pub fn cell(&self, i: [usize; 3]) -> usize {
        (i[2] * self.n[1] + i[1]) * self.n[0] + i[0]
    }

    /// Inverse of [`Mesh::cell`].
    pub fn unindex(&self, id: usize) -> [usize; 3] {
        let (i, rest) = (id % self.n[0], id / self.n[0]);
        [i, rest % self.n[1], rest / self.n[1]]
    }

    pub fn volume(&self, i: [usize; 3]) -> f64 {
        self.dx[0][i[0]] * self.dx[1][i[1]] * self.dx[2][i[2]]
    }

    /// Area of the face of a cell normal to axis `a`.
    pub fn area(&self, a: usize, i: [usize; 3]) -> f64 {
        let (b, c) = other_axes(a);
        self.dx[b][i[b]] * self.dx[c][i[c]]
    }
}

pub fn other_axes(a: usize) -> (usize, usize) {
    match a {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}
