//! Ray marching through the graded grid.

use super::patches::edges;
use crate::result::Grid3;

/// Where a ray ended: the cell and its face that was reached. For a solid
/// that is the face looking back at the ray; at the domain boundary it is
/// the boundary face of the last air cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hit {
    pub cell: usize,
    pub dir: u8,
}

pub(crate) struct Tracer<'a> {
    pub grid: &'a Grid3,
    pub opaque: &'a [bool],
}

impl Tracer<'_> {
    /// Marches from `origin` (mm, inside air cell `start`) along the unit
    /// vector `d` until a solid cell or the domain boundary.
    pub fn trace(&self, start: [usize; 3], origin: [f64; 3], d: [f64; 3]) -> Hit {
        let g = self.grid;
        let n = [g.nx, g.ny, g.nz];
        let mut idx = start;
        loop {
            let mut best = (f64::INFINITY, 3usize);
            for a in 0..3 {
                if d[a] == 0.0 {
                    continue;
                }
                let e = edges(g, a);
                let b = if d[a] > 0.0 { e[idx[a] + 1] } else { e[idx[a]] };
                let t = (b - origin[a]) / d[a];
                if t < best.0 {
                    best = (t, a);
                }
            }
            let a = best.1;
            if a == 3 {
                // No direction at all: cannot happen for a unit vector.
                return Hit {
                    cell: self.cell(idx),
                    dir: 0,
                };
            }
            let step: isize = if d[a] > 0.0 { 1 } else { -1 };
            let dir = (2 * a + usize::from(step > 0)) as u8;
            let next = idx[a] as isize + step;
            if next < 0 || next as usize >= n[a] {
                return Hit {
                    cell: self.cell(idx),
                    dir,
                };
            }
            idx[a] = next as usize;
            let c = self.cell(idx);
            if self.opaque[c] {
                // Face of the solid cell that looks back at the ray.
                return Hit {
                    cell: c,
                    dir: dir ^ 1,
                };
            }
        }
    }

    fn cell(&self, i: [usize; 3]) -> usize {
        (i[2] * self.grid.ny + i[1]) * self.grid.nx + i[0]
    }
}
