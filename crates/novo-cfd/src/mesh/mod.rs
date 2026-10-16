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
pub use classify::{BodyResolution, CellClass, Mesh, MixedCell};
pub use preview::MeshPreview;
pub use regions::{Region, domain_mm, regions, regions_with};

use novo_thermal::Scenario;

use crate::result::{Fidelity, Grid3};
use axis::{Span, axis_edges};

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
    /// Cells wanted across every solid body and opening (per axis). The
    /// budget is spent on these first and empty air gets the rest; when the
    /// bodies alone would not fit, the number is lowered until they do (see
    /// [`GridPlan::cells_across`]).
    pub min_cells_across: usize,
    /// Bodies thinner than this (mm) along an axis, such as the mounting
    /// plate, are one cell thick there, exactly aligned.
    pub thin_mm: f64,
    /// Give thin bodies (below `thin_mm`) their own aligned cell layer. Off:
    /// they are left to cut cells, which keeps the cells near the plate as
    /// wide as the rest (a 2.5 mm layer under 40 mm cells makes the
    /// pressure solve several times slower).
    pub resolve_thin: bool,
    /// The mounting plate is a volume slab of the grid (aligned, one cell
    /// thick). `false` leaves it out, for a solver that models it as a sheet
    /// on the rear face.
    pub plate_slab: bool,
}

/// Default of [`MeshSettings::thin_mm`].
pub const THIN_MM: f64 = 8.0;

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings {
            target_cells: Fidelity::Coarse.target_cells(),
            near_ratio: 0.35,
            growth: 1.3,
            max_ratio: 1.5,
            min_cells_across: 3,
            thin_mm: THIN_MM,
            resolve_thin: false,
            plate_slab: true,
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
        if self.min_cells_across == 0 {
            return Err(MeshError::Settings("min_cells_across must be at least 1"));
        }
        if self.thin_mm.is_nan() || self.thin_mm < 0.0 {
            return Err(MeshError::Settings("thin_mm must not be negative"));
        }
        if self.max_ratio < 1.1 {
            return Err(MeshError::Settings("max_ratio must be at least 1.1"));
        }
        Ok(())
    }
}

/// The mounting plate and DIN rails are sheets that may live in cut cells.
fn is_thin_sheet_kind(scn: &Scenario, material: u16) -> bool {
    if material == 1 {
        return true;
    }
    let idx = (material as usize).checked_sub(2 + scn.devices.len());
    idx.and_then(|i| scn.obstacles.get(i))
        .is_some_and(|o| o.kind == novo_thermal::ObstacleKind::Rail)
}

/// What the grid builder wants resolved, per axis: solid bodies and
/// openings (`need` is lowered for thin sheets).
fn spans(scn: &Scenario, regs: &[Region], need: usize, st: &MeshSettings) -> [Vec<Span>; 3] {
    let mut f: [Vec<Span>; 3] = Default::default();
    let first_obstacle = 2 + scn.devices.len();
    for r in regs {
        // Devices get the full requirement. Ducts and rails are minor
        // obstacles (porous or low), one cell fewer; the plate's edges are
        // planes of their own and it needs no cells along its length.
        let n = match r.material {
            1 => 1,
            m if m as usize >= first_obstacle => need.saturating_sub(1).max(1),
            _ => need,
        };
        let carve_out = is_thin_sheet_kind(scn, r.material);
        for a in 0..3 {
            let thin = r.hi[a] - r.lo[a] < st.thin_mm;
            f[a].push(match (thin, carve_out) {
                // plate and rails: a cut-cell sheet unless asked for
                (true, true) if !st.resolve_thin => Span {
                    lo: r.lo[a],
                    hi: r.hi[a],
                    need: 0,
                    sheet: false,
                },
                // any other thin body (a slim relay, a 6 mm terminal block)
                // keeps its faces as planes and at least one cell
                (true, _) => Span {
                    lo: r.lo[a],
                    hi: r.hi[a],
                    need: 1,
                    sheet: true,
                },
                _ => Span {
                    lo: r.lo[a],
                    hi: r.hi[a],
                    need: n,
                    sheet: false,
                },
            });
        }
    }
    let mut add = |a: usize, lo: f64, hi: f64, n: usize| {
        f[a].push(Span {
            lo,
            hi,
            need: n,
            sheet: false,
        });
    };
    let mut openings = |o: &novo_thermal::Opening| {
        for (a, pts) in regions::opening_spans(o).into_iter().enumerate() {
            if let Some((lo, hi)) = pts {
                add(a, lo, hi, need.min(2));
            }
        }
    };
    for v in &scn.vents {
        openings(&v.opening);
    }
    for fan in &scn.fans {
        openings(&fan.opening);
    }
    f
}

fn edges_for(dom: [f64; 3], spans: &[Vec<Span>; 3], s: f64, st: &MeshSettings) -> [Vec<f64>; 3] {
    let sp = Spacing {
        far: s,
        near: (s * st.near_ratio).max(1e-3),
        growth: st.growth,
        max_ratio: st.max_ratio,
    };
    std::array::from_fn(|a| axis_edges(dom[a], &spans[a], &sp))
}

/// A grid and how the budget was spent.
#[derive(Debug, Clone, PartialEq)]
pub struct GridPlan {
    pub grid: Grid3,
    /// Cells across every body that the grid guarantees (at most the
    /// requested `min_cells_across`; lower when the bodies alone would
    /// exceed the budget).
    pub cells_across: usize,
    /// Even one cell across each body needs more cells than the target: the
    /// grid is larger than asked.
    pub over_budget: bool,
}

/// Only the grid, without classification: cheap, for previews.
pub fn plan_grid(scn: &Scenario, st: &MeshSettings) -> Result<Grid3, MeshError> {
    plan_grid_report(scn, st).map(|p| p.grid)
}

/// The grid with the account of how the cell budget was spent.
///
/// The target is met by choosing the coarse size of empty air; the cells
/// across bodies and openings are the first claim on the budget.
pub fn plan_grid_report(scn: &Scenario, st: &MeshSettings) -> Result<GridPlan, MeshError> {
    st.validate()?;
    let dom = domain_mm(scn);
    if !dom.iter().all(|d| *d > 0.0 && d.is_finite()) {
        return Err(MeshError::EmptyDomain);
    }
    let (regs, _) = regions_with(scn, st.plate_slab);
    let target = st.target_cells as f64;
    let s_max = dom.iter().cloned().fold(0.0, f64::max) * 2.0;
    let count = |sp: &[Vec<Span>; 3], s: f64| -> usize {
        edges_for(dom, sp, s, st)
            .iter()
            .map(|e| e.len() - 1)
            .product()
    };
    // The bodies come first: the largest requirement whose cells (with
    // empty air as coarse as it can be) still fit.
    let mut need = st.min_cells_across;
    let mut sp = spans(scn, &regs, need, st);
    while need > 1 && count(&sp, s_max) as f64 > target {
        need -= 1;
        sp = spans(scn, &regs, need, st);
    }
    let floor = count(&sp, s_max);
    let over_budget = floor as f64 > target * 1.05;

    // Cell count falls as the coarse size grows: bisect on a log scale.
    let (mut lo, mut hi) = (0.25_f64, s_max);
    let mut best = (f64::INFINITY, hi);
    for _ in 0..60 {
        let s = (lo * hi).sqrt();
        let n = count(&sp, s) as f64;
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
        let s = (centre * (1.0 + 0.0015 * f64::from(i))).min(s_max);
        let err = (count(&sp, s) as f64 / target).ln().abs();
        if err < best.0 {
            best = (err, s);
        }
    }
    let [x, y, z] = edges_for(dom, &sp, best.1, st);
    Ok(GridPlan {
        grid: Grid3 {
            nx: x.len() - 1,
            ny: y.len() - 1,
            nz: z.len() - 1,
            x_edges_mm: x,
            y_edges_mm: y,
            z_edges_mm: z,
        },
        cells_across: need,
        over_budget,
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
