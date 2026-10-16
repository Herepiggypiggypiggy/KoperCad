//! Graded Cartesian meshing of a `novo_thermal::Scenario` (lane CFD-C1).
//!
//! Pure functions, no solver code. The grid is rectilinear with cells
//! graded towards the enclosure walls, device faces and opening edges
//! (geometric growth, bounded neighbour ratio). Solids that do not line up
//! with grid planes are handled "cut-cell lite": each cell keeps the exact
//! volume share of every material in it, so conduction and sources can be
//! weighted by fraction while the flow sees a fluid fraction.
//!
//! Guarantees (tests in `tests/mesh.rs`): deterministic; the material
//! volumes add up to the interior volume within 1e-9; every fraction lies
//! in `[0, 1]`; neighbouring cells differ in size by at most
//! [`MeshSettings::max_ratio`].
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate dialog shows
//! [`MeshPreview::summary`] before a run (app side is lane CFD-F).

mod axis;
mod classify;
mod preview;
mod regions;

pub use axis::Spacing;
pub use classify::{CellClass, Mesh, MixedCell};
pub use preview::MeshPreview;
pub use regions::{Region, domain_mm, regions};

use novo_thermal::Scenario;

use crate::result::{Fidelity, Grid3};
use axis::{AxisFeatures, axis_edges};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MeshError {
    #[error("the enclosure interior has no volume")]
    EmptyDomain,
    #[error("{0:?} has no 3D mesh")]
    NoMeshForFidelity(Fidelity),
    #[error("invalid mesh settings: {0}")]
    Settings(&'static str),
}

/// What to build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshSettings {
    /// Wanted number of cells; the result is within a few percent.
    pub target_cells: usize,
    /// Cell size at a plane relative to the coarse size.
    pub near_ratio: f64,
    /// Geometric growth away from a plane.
    pub growth: f64,
    /// Hard bound on the size ratio of neighbouring cells.
    pub max_ratio: f64,
}

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings {
            target_cells: Fidelity::Coarse.target_cells(),
            near_ratio: 0.35,
            growth: 1.2,
            max_ratio: 1.5,
        }
    }
}

impl MeshSettings {
    /// Preset of a progressive level (50k, 500k, 2M cells).
    pub fn for_fidelity(f: Fidelity) -> Result<MeshSettings, MeshError> {
        match f {
            Fidelity::Preview2d => Err(MeshError::NoMeshForFidelity(f)),
            _ => Ok(MeshSettings {
                target_cells: f.target_cells(),
                ..MeshSettings::default()
            }),
        }
    }

    fn validate(&self) -> Result<(), MeshError> {
        if self.target_cells < 8 {
            return Err(MeshError::Settings("target_cells must be at least 8"));
        }
        if !(self.near_ratio > 0.0 && self.near_ratio <= 1.0) {
            return Err(MeshError::Settings("near_ratio must be in (0, 1]"));
        }
        if !(self.growth >= 1.0 && self.growth <= self.max_ratio) {
            return Err(MeshError::Settings("growth must be in [1, max_ratio]"));
        }
        if self.max_ratio < 1.1 {
            return Err(MeshError::Settings("max_ratio must be at least 1.1"));
        }
        Ok(())
    }
}

/// Plane sources per axis from the scenario.
fn features(scn: &Scenario, regs: &[Region]) -> [AxisFeatures; 3] {
    let mut f: [AxisFeatures; 3] = Default::default();
    for r in regs {
        for (a, fa) in f.iter_mut().enumerate() {
            fa.boxes.push((r.lo[a], r.hi[a]));
        }
    }
    for v in &scn.vents {
        add_opening(&mut f, &v.opening);
    }
    for fan in &scn.fans {
        add_opening(&mut f, &fan.opening);
    }
    f
}

fn add_opening(f: &mut [AxisFeatures; 3], o: &novo_thermal::Opening) {
    for (a, pts) in regions::opening_planes(o).into_iter().enumerate() {
        f[a].points.extend(pts);
    }
}

fn edges_for(dom: [f64; 3], feats: &[AxisFeatures; 3], s: f64, st: &MeshSettings) -> [Vec<f64>; 3] {
    let sp = Spacing {
        far: s,
        near: (s * st.near_ratio).max(1e-3),
        growth: st.growth,
        max_ratio: st.max_ratio,
    };
    std::array::from_fn(|a| axis_edges(dom[a], &feats[a], &sp))
}

/// Only the grid, without classification: cheap, for previews.
pub fn plan_grid(scn: &Scenario, st: &MeshSettings) -> Result<Grid3, MeshError> {
    st.validate()?;
    let dom = domain_mm(scn);
    if !dom.iter().all(|d| *d > 0.0 && d.is_finite()) {
        return Err(MeshError::EmptyDomain);
    }
    let (regs, _) = regions(scn);
    let feats = features(scn, &regs);
    let count = |s: f64| -> usize {
        edges_for(dom, &feats, s, st)
            .iter()
            .map(|e| e.len() - 1)
            .product()
    };
    // Cell count falls as the coarse size grows: bisect on a log scale.
    let target = st.target_cells as f64;
    let (mut lo, mut hi) = (0.25_f64, dom.iter().cloned().fold(0.0, f64::max) * 2.0);
    let mut best = (f64::INFINITY, hi);
    for _ in 0..60 {
        let s = (lo * hi).sqrt();
        let n = count(s) as f64;
        let err = (n / target).ln().abs();
        if err < best.0 {
            best = (err, s);
        }
        if n > target {
            lo = s;
        } else {
            hi = s;
        }
        if err < 0.005 {
            break;
        }
    }
    // The count moves in steps (planes appear, cells split); look around
    // the bisection result for the closest step.
    let centre = best.1;
    for i in -40..=40 {
        let s = centre * (1.0 + 0.0015 * f64::from(i));
        let err = (count(s) as f64 / target).ln().abs();
        if err < best.0 {
            best = (err, s);
        }
    }
    let [x, y, z] = edges_for(dom, &feats, best.1, st);
    Ok(Grid3 {
        nx: x.len() - 1,
        ny: y.len() - 1,
        nz: z.len() - 1,
        x_edges_mm: x,
        y_edges_mm: y,
        z_edges_mm: z,
    })
}

/// Cell holding a point (mm), or `None` outside the grid. Used to move
/// fields between grid levels.
pub fn locate(grid: &Grid3, p_mm: [f64; 3]) -> Option<[usize; 3]> {
    let find = |e: &[f64], v: f64| -> Option<usize> {
        if !(v >= e[0] && v <= *e.last()?) {
            return None;
        }
        Some(
            e.partition_point(|&x| x <= v)
                .saturating_sub(1)
                .min(e.len() - 2),
        )
    };
    Some([
        find(&grid.x_edges_mm, p_mm[0])?,
        find(&grid.y_edges_mm, p_mm[1])?,
        find(&grid.z_edges_mm, p_mm[2])?,
    ])
}
