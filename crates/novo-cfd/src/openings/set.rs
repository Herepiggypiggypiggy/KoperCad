//! Patches resolved on a mesh, and the law at each open face.

use super::{OpenKind, OpenPatch};
use crate::flow::{Mesh, other_axes};

/// One boundary face (or part of it) that is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenFace {
    /// Normal axis and side (`upper` = the max side).
    pub axis: usize,
    pub upper: bool,
    /// Index into the velocity array of `axis`.
    pub vel: usize,
    /// The cell behind the face, and its index triple.
    pub cell: usize,
    pub ix: [usize; 3],
    /// Open area, m2, and the distance from the face to the cell centre, m.
    pub area: f64,
    pub dh: f64,
    pub patch: usize,
}

impl OpenFace {
    /// Sign that turns a velocity along the axis into the outward one.
    pub fn outward(&self) -> f64 {
        if self.upper { 1.0 } else { -1.0 }
    }
}

/// All patches of a run, resolved on one mesh.
#[derive(Debug, Clone)]
pub struct OpenSet {
    pub patches: Vec<OpenPatch>,
    pub faces: Vec<OpenFace>,
    /// Open area per patch, m2.
    pub patch_area: Vec<f64>,
    /// Open share of each velocity face of component `a` (1 inside the
    /// domain, 0 on a wall, the open share on an opening).
    pub afrac: [Vec<f64>; 3],
    /// Patch of each open velocity face, `usize::MAX` where none.
    pub patch_of: [Vec<usize>; 3],
}

impl OpenSet {
    /// Resolve `patches` on `mesh`. A boundary face already claimed by an
    /// earlier patch is not given to a later one.
    pub fn resolve(mesh: &Mesh, patches: &[OpenPatch]) -> OpenSet {
        let mut afrac: [Vec<f64>; 3] = std::array::from_fn(|c| vec![1.0; mesh.len(c)]);
        let mut patch_of: [Vec<usize>; 3] = std::array::from_fn(|c| vec![usize::MAX; mesh.len(c)]);
        // Boundary faces are closed until a patch opens them.
        for a in 0..3 {
            let d = mesh.dims(a);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[a] == 0 || ix[a] == mesh.n[a] {
                            afrac[a][mesh.idx(a, ix)] = 0.0;
                        }
                    }
                }
            }
        }
        let mut faces = Vec::new();
        let mut patch_area = vec![0.0; patches.len()];
        for (p, patch) in patches.iter().enumerate() {
            let face = patch.face as usize;
            let (a, upper) = (face / 2, face % 2 == 1);
            let (b, c) = other_axes(a);
            let overlap = |axis: usize, i: usize, lo: f64, hi: f64| -> f64 {
                let (e0, e1) = (mesh.edges[axis][i], mesh.edges[axis][i + 1]);
                ((e1.min(hi) - e0.max(lo)).max(0.0)) / (e1 - e0)
            };
            let face_ix = if upper { mesh.n[a] } else { 0 };
            let cell_a = if upper { mesh.n[a] - 1 } else { 0 };
            for kc in 0..mesh.n[c] {
                let sc = overlap(c, kc, patch.lo[1], patch.hi[1]);
                if sc <= 1e-9 {
                    continue;
                }
                for jb in 0..mesh.n[b] {
                    let sb = overlap(b, jb, patch.lo[0], patch.hi[0]);
                    if sb <= 1e-9 {
                        continue;
                    }
                    let mut ix = [0; 3];
                    ix[a] = face_ix;
                    ix[b] = jb;
                    ix[c] = kc;
                    let v = mesh.idx(a, ix);
                    if patch_of[a][v] != usize::MAX {
                        continue;
                    }
                    let mut cix = ix;
                    cix[a] = cell_a;
                    let share = sb * sc;
                    let area = mesh.area(a, cix) * share;
                    afrac[a][v] = share;
                    patch_of[a][v] = p;
                    patch_area[p] += area;
                    faces.push(OpenFace {
                        axis: a,
                        upper,
                        vel: v,
                        cell: mesh.cell(cix),
                        ix: cix,
                        area,
                        dh: 0.5 * mesh.dx[a][cell_a],
                        patch: p,
                    });
                }
            }
        }
        OpenSet {
            patches: patches.to_vec(),
            faces,
            patch_area,
            afrac,
            patch_of,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Resistance `r` and pressure jump `p_jump` (both kinematic: divided
    /// by the density `rho`) of the law `p_face = r u + p_jump` at a face
    /// whose recent speed is `speed` m/s and whose outward velocity is
    /// `u_out` (only its sign matters for the inflow term).
    pub fn law(&self, f: &OpenFace, speed: f64, u_out: f64, rho: f64) -> (f64, f64) {
        match self.patches[f.patch].kind {
            OpenKind::Free => (if u_out < 0.0 { 0.5 * speed } else { 0.0 }, 0.0),
            OpenKind::Loss { k } => (0.5 * k * speed, 0.0),
            OpenKind::Fan {
                intake,
                free_flow_m3s,
                max_pressure_pa,
            } => {
                let dp = max_pressure_pa / rho;
                let r = dp * self.patch_area[f.patch] / free_flow_m3s;
                (r, if intake { dp } else { -dp })
            }
        }
    }
}
