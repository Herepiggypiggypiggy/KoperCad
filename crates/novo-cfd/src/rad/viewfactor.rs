//! View factors by ray sampling.

use rayon::prelude::*;

use super::patches::{PatchKind, PatchSet, axis_of, edges, face_box_mm, sign_of};
use super::trace::Tracer;
use crate::mesh::Mesh;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewFactorSettings {
    /// Rays per patch. The sampling error of a factor `F` falls roughly as
    /// `1 / rays` thanks to the low-discrepancy sequence, but each patch
    /// still needs enough rays to see small neighbours.
    pub rays_per_patch: usize,
    /// Seed of the per-patch shifts; the result is deterministic for a seed.
    pub seed: u64,
    /// Enforce reciprocity and summation afterwards (leave on; the raw
    /// numbers are kept in [`ViewFactors::stats`] either way).
    pub enforce: bool,
}

impl Default for ViewFactorSettings {
    fn default() -> Self {
        ViewFactorSettings {
            rays_per_patch: 2000,
            seed: 0x4B4F_5045_5243_4144,
            enforce: true,
        }
    }
}

/// How good the raw sampling was, before the exact correction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SamplingStats {
    pub rays_per_patch: usize,
    /// Largest `|sum_j F_ij - 1|` of the raw rows.
    pub max_row_sum_error: f64,
    /// Largest `|A_i F_ij - A_j F_ji| / mean(A_i, A_j)` of the raw matrix.
    pub max_reciprocity_error: f64,
    /// Rays that ended on a face that belongs to no patch (zero unless the
    /// mesh and patch set disagree).
    pub lost_rays: usize,
}

/// Dense view factor matrix `F[i][j]`: the share of the diffuse radiation
/// leaving patch `i` that reaches patch `j` directly.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewFactors {
    n: usize,
    f: Vec<f64>,
    areas_m2: Vec<f64>,
    pub stats: SamplingStats,
}

impl ViewFactors {
    /// From a given matrix (row-major, `n * n`) and areas; no correction.
    pub fn from_matrix(areas_m2: Vec<f64>, f: Vec<f64>) -> ViewFactors {
        let n = areas_m2.len();
        assert_eq!(f.len(), n * n, "view factor matrix must be n x n");
        ViewFactors {
            n,
            f,
            areas_m2,
            stats: SamplingStats::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.f[i * self.n + j]
    }

    pub fn row(&self, i: usize) -> &[f64] {
        &self.f[i * self.n..(i + 1) * self.n]
    }

    pub fn areas_m2(&self) -> &[f64] {
        &self.areas_m2
    }

    /// Largest `|A_i F_ij - A_j F_ji| / mean(A_i, A_j)`.
    pub fn reciprocity_error(&self) -> f64 {
        let mut worst: f64 = 0.0;
        for i in 0..self.n {
            for j in (i + 1)..self.n {
                let a = self.areas_m2[i] * self.get(i, j);
                let b = self.areas_m2[j] * self.get(j, i);
                let mean = 0.5 * (self.areas_m2[i] + self.areas_m2[j]);
                worst = worst.max((a - b).abs() / mean);
            }
        }
        worst
    }

    /// Largest `|sum_j F_ij - 1|` (a closed enclosure sums to one).
    pub fn summation_error(&self) -> f64 {
        (0..self.n)
            .map(|i| (self.row(i).iter().sum::<f64>() - 1.0).abs())
            .fold(0.0, f64::max)
    }

    /// Sample the view factors of a patch set on its mesh.
    pub fn sample(mesh: &Mesh, patches: &PatchSet, st: &ViewFactorSettings) -> ViewFactors {
        let n = patches.len();
        let grid = &mesh.grid;
        let opaque: Vec<bool> = mesh.material.iter().map(|&m| m != 0).collect();
        let tracer = Tracer {
            grid,
            opaque: &opaque,
        };
        let rays = st.rays_per_patch.max(1);
        let areas = patches.areas_m2();

        // Raw hit counts per row, rays that went astray per row.
        let rows: Vec<(Vec<f64>, usize)> = (0..n)
            .into_par_iter()
            .map(|i| {
                let p = &patches.patches[i];
                let mut hits = vec![0.0f64; n];
                let mut lost = 0usize;
                let mut rng = SplitMix(st.seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
                let shift: [f64; 5] = std::array::from_fn(|_| rng.unit());
                let total = patches.cum_area_mm2[p.faces.end - 1];
                let cum = &patches.cum_area_mm2[p.faces.clone()];
                for s in 0..rays {
                    let h = |base: u32, k: usize| -> f64 {
                        (halton(s as u64 + 1, base) + shift[k]).fract()
                    };
                    // Direction (bases 2, 3), face (5), position (7, 11).
                    let (u1, u2) = (h(2, 0), h(3, 1));
                    let pick = h(5, 2) * total;
                    let fi = cum.partition_point(|&c| c <= pick).min(cum.len() - 1);
                    let face = patches.faces[p.faces.start + fi];
                    let (lo, hi) = face_box_mm(grid, face);
                    let a = axis_of(face.dir);
                    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
                    let mut origin = lo;
                    origin[b] = lo[b] + h(7, 3) * (hi[b] - lo[b]);
                    origin[c] = lo[c] + h(11, 4) * (hi[c] - lo[c]);
                    // Start cell: the air cell in front of the face.
                    let cell = face.cell as usize;
                    let mut idx = [
                        cell % grid.nx,
                        (cell / grid.nx) % grid.ny,
                        cell / (grid.nx * grid.ny),
                    ];
                    let sn = if p.kind == PatchKind::Solid {
                        sign_of(face.dir)
                    } else {
                        -sign_of(face.dir)
                    };
                    if p.kind == PatchKind::Solid {
                        idx[a] = (idx[a] as isize + sn) as usize;
                    }
                    // Keep the origin strictly inside its start cell on the
                    // tangential axes (a point on a cell edge is fine: the
                    // marcher only looks at the way forward).
                    for ax in [b, c] {
                        let e = edges(grid, ax);
                        origin[ax] = origin[ax].clamp(e[idx[ax]], e[idx[ax] + 1]);
                    }
                    let r = u1.sqrt();
                    let phi = std::f64::consts::TAU * u2;
                    let mut d = [0.0; 3];
                    d[a] = sn as f64 * (1.0 - u1).sqrt();
                    d[b] = r * phi.cos();
                    d[c] = r * phi.sin();
                    let hit = tracer.trace(idx, origin, d);
                    match patches.patch_of(hit.cell, hit.dir) {
                        Some(j) => hits[j] += 1.0,
                        None => lost += 1,
                    }
                }
                (hits, lost)
            })
            .collect();

        // G_ij = A_i * share of rays from i that hit j, in m2.
        let mut g = vec![0.0f64; n * n];
        let mut lost_rays = 0;
        let mut raw_row: f64 = 0.0;
        for (i, (hits, lost)) in rows.iter().enumerate() {
            lost_rays += lost;
            let mut sum = 0.0;
            for (j, h) in hits.iter().enumerate() {
                let share = h / rays as f64;
                g[i * n + j] = areas[i] * share;
                sum += share;
            }
            raw_row = raw_row.max((sum - 1.0).abs());
        }
        let mut raw_rec: f64 = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let mean = 0.5 * (areas[i] + areas[j]);
                raw_rec = raw_rec.max((g[i * n + j] - g[j * n + i]).abs() / mean);
            }
        }
        let stats = SamplingStats {
            rays_per_patch: rays,
            max_row_sum_error: raw_row,
            max_reciprocity_error: raw_rec,
            lost_rays,
        };

        // Symmetrise, then scale so that every row sums to one:
        // d_i * sum_j G_ij d_j = A_i keeps G symmetric (reciprocity).
        let mut sym = vec![0.0f64; n * n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    sym[i * n + j] = 0.5 * (g[i * n + j] + g[j * n + i]);
                }
            }
        }
        let mut d = vec![1.0f64; n];
        if st.enforce {
            for _ in 0..1000 {
                let s: Vec<f64> = (0..n)
                    .into_par_iter()
                    .map(|i| (0..n).map(|j| sym[i * n + j] * d[j]).sum())
                    .collect();
                let mut worst: f64 = 0.0;
                for i in 0..n {
                    if s[i] > 0.0 {
                        worst = worst.max((d[i] * s[i] / areas[i] - 1.0).abs());
                        d[i] = (d[i] * areas[i] / s[i]).sqrt();
                    }
                }
                if worst < 1e-13 {
                    break;
                }
            }
        }
        let mut f = vec![0.0f64; n * n];
        for i in 0..n {
            for j in 0..n {
                f[i * n + j] = if st.enforce {
                    d[i] * sym[i * n + j] * d[j] / areas[i]
                } else {
                    g[i * n + j] / areas[i]
                };
            }
        }
        ViewFactors {
            n,
            f,
            areas_m2: areas,
            stats,
        }
    }
}

/// Radical inverse of `n` in `base`.
fn halton(mut n: u64, base: u32) -> f64 {
    let (b, mut inv, mut r) = (u64::from(base), 1.0 / f64::from(base), 0.0);
    let step = inv;
    while n > 0 {
        r += (n % b) as f64 * inv;
        n /= b;
        inv *= step;
    }
    r
}

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}
