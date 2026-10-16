//! Cell classification: material, fluid fraction and exact volume shares.

use novo_thermal::Scenario;

use super::regions::Region;
use super::{MeshError, MeshSettings, THIN_MM, plan_grid_report};
use crate::result::{Grid3, MaterialInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellClass {
    /// Entirely air.
    Fluid,
    /// Entirely one solid.
    Solid,
    /// Holds more than one material: a cut cell.
    Cut,
}

/// The exact content of a cell that is not pure: `(material id, share of
/// the cell volume)`, air (id 0) included, ids ascending, shares add up to 1.
#[derive(Debug, Clone, PartialEq)]
pub struct MixedCell {
    pub cell: usize,
    pub parts: Vec<(u16, f64)>,
}

/// A graded grid with its classification. Cell index `(k * ny + j) * nx + i`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub grid: Grid3,
    pub class: Vec<CellClass>,
    /// Material with the largest share in the cell (ties: lowest id).
    pub material: Vec<u16>,
    /// Air share of the cell volume, `[0, 1]`.
    pub fluid_fraction: Vec<f64>,
    /// Content of the cut cells, ascending cell index.
    pub mixed: Vec<MixedCell>,
    pub materials: Vec<MaterialInfo>,
    /// Volume per material id, m3 (index = id; ids of dropped items are 0).
    pub material_volume_m3: Vec<f64>,
    /// How many cells each solid body spans, per axis.
    pub bodies: Vec<BodyResolution>,
    /// The bodies alone needed more cells than the target (see
    /// `GridPlan::over_budget`).
    pub over_budget: bool,
}

/// Cells across one solid body, counted by cell centres inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyResolution {
    pub material: u16,
    pub name: String,
    pub cells_across: [usize; 3],
    /// Axes along which the body is a thin sheet (below `THIN_MM`).
    pub thin: [bool; 3],
}

impl BodyResolution {
    /// At least two cells across every axis that is not a thin sheet: what a
    /// temperature on the body's surface needs to mean something. A sheet
    /// (the mounting plate) lives in cut cells unless `resolve_thin` is on.
    pub fn is_resolved(&self) -> bool {
        (0..3).all(|a| self.thin[a] || self.cells_across[a] >= 2)
    }
}

impl Mesh {
    /// Mesh a scenario with the given settings.
    pub fn build(scn: &Scenario, st: &MeshSettings) -> Result<Mesh, MeshError> {
        let plan = plan_grid_report(scn, st)?;
        let (regs, materials) = super::regions_with(scn, st.plate_slab);
        let mut mesh = Mesh::classify(plan.grid, &regs, materials);
        mesh.over_budget = plan.over_budget;
        Ok(mesh)
    }

    /// Classify a given grid against solid regions (priority order).
    pub fn classify(grid: Grid3, regs: &[Region], materials: Vec<MaterialInfo>) -> Mesh {
        let n = grid.cells();
        let (nx, ny) = (grid.nx, grid.ny);
        let mut touched: Vec<(usize, usize)> = Vec::new();
        for (p, r) in regs.iter().enumerate() {
            let rng: [(usize, usize); 3] =
                std::array::from_fn(|a| cell_range(edges(&grid, a), r, a));
            for k in rng[2].0..=rng[2].1 {
                for j in rng[1].0..=rng[1].1 {
                    for i in rng[0].0..=rng[0].1 {
                        touched.push(((k * ny + j) * nx + i, p));
                    }
                }
            }
        }
        touched.sort_unstable();

        let mut class = vec![CellClass::Fluid; n];
        let mut material = vec![0u16; n];
        let mut fluid_fraction = vec![1.0f64; n];
        let mut mixed = Vec::new();
        let max_id = materials.iter().map(|m| m.id).max().unwrap_or(0) as usize;
        let mut solid_mm3 = vec![0.0f64; max_id + 1];

        let mut g = 0;
        while g < touched.len() {
            let cell = touched[g].0;
            let mut e = g;
            while e < touched.len() && touched[e].0 == cell {
                e += 1;
            }
            let (i, j, k) = (cell % nx, (cell / nx) % ny, cell / (nx * ny));
            let lo = [grid.x_edges_mm[i], grid.y_edges_mm[j], grid.z_edges_mm[k]];
            let hi = [
                grid.x_edges_mm[i + 1],
                grid.y_edges_mm[j + 1],
                grid.z_edges_mm[k + 1],
            ];
            let boxes: Vec<&Region> = touched[g..e].iter().map(|&(_, p)| &regs[p]).collect();
            let cell_mm3: f64 = (0..3).map(|a| hi[a] - lo[a]).product();
            let parts = resolve(lo, hi, &boxes);
            let dominant = parts.iter().fold(
                (0u16, -1.0f64),
                |b, &(id, v)| if v > b.1 { (id, v) } else { b },
            );
            material[cell] = dominant.0;
            for &(id, v) in &parts {
                if id != 0 {
                    solid_mm3[id as usize] += v;
                }
            }
            let air = parts.iter().find(|p| p.0 == 0).map_or(0.0, |p| p.1);
            fluid_fraction[cell] = air / cell_mm3;
            if parts.len() == 1 {
                class[cell] = if dominant.0 == 0 {
                    CellClass::Fluid
                } else {
                    CellClass::Solid
                };
            } else {
                class[cell] = CellClass::Cut;
                mixed.push(MixedCell {
                    cell,
                    parts: parts.iter().map(|&(id, v)| (id, v / cell_mm3)).collect(),
                });
            }
            g = e;
        }

        let mut vol = vec![0.0f64; max_id + 1];
        for (id, v) in solid_mm3.iter().enumerate() {
            vol[id] = v * 1e-9;
        }
        let mut air = 0.0;
        for k in 0..grid.nz {
            for j in 0..grid.ny {
                let dyz = (grid.y_edges_mm[j + 1] - grid.y_edges_mm[j])
                    * (grid.z_edges_mm[k + 1] - grid.z_edges_mm[k]);
                let mut row = 0.0;
                for i in 0..grid.nx {
                    let f = fluid_fraction[(k * ny + j) * nx + i];
                    row += f * (grid.x_edges_mm[i + 1] - grid.x_edges_mm[i]);
                }
                air += row * dyz;
            }
        }
        vol[0] = air * 1e-9;
        let bodies = bodies(&grid, regs, &materials);
        Mesh {
            grid,
            class,
            material,
            fluid_fraction,
            mixed,
            materials,
            material_volume_m3: vol,
            bodies,
            over_budget: false,
        }
    }

    /// Number of cells of each class `(fluid, solid, cut)`.
    pub fn class_counts(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for k in &self.class {
            match k {
                CellClass::Fluid => c.0 += 1,
                CellClass::Solid => c.1 += 1,
                CellClass::Cut => c.2 += 1,
            }
        }
        c
    }

    /// Interior volume as the sum of material volumes, m3.
    pub fn total_volume_m3(&self) -> f64 {
        self.material_volume_m3.iter().sum()
    }
}

fn edges(g: &Grid3, a: usize) -> &[f64] {
    match a {
        0 => &g.x_edges_mm,
        1 => &g.y_edges_mm,
        _ => &g.z_edges_mm,
    }
}

/// First and last cell along an axis that the region overlaps.
fn cell_range(e: &[f64], r: &Region, a: usize) -> (usize, usize) {
    let n = e.len() - 1;
    let i0 = e
        .partition_point(|&x| x <= r.lo[a])
        .saturating_sub(1)
        .min(n - 1);
    let i1 = e
        .partition_point(|&x| x < r.hi[a])
        .saturating_sub(1)
        .clamp(i0, n - 1);
    (i0, i1)
}

/// Volume (mm3) of every material inside a cell; `boxes` in priority order,
/// earlier ones win overlaps. Exact: the cell is cut at every box face and
/// each piece goes to the first box holding it.
fn resolve(lo: [f64; 3], hi: [f64; 3], boxes: &[&Region]) -> Vec<(u16, f64)> {
    if let Some(b) = boxes
        .iter()
        .find(|b| (0..3).all(|a| b.lo[a] <= lo[a] && b.hi[a] >= hi[a]))
    {
        // The first box that covers the cell wins; boxes before it cannot
        // be ignored if they also touch the cell, so only shortcut when it
        // is first.
        if std::ptr::eq(*b, boxes[0]) {
            return vec![(b.material, (0..3).map(|a| hi[a] - lo[a]).product())];
        }
    }
    let cuts: [Vec<f64>; 3] = std::array::from_fn(|a| {
        let mut c = vec![lo[a], hi[a]];
        for b in boxes {
            for v in [b.lo[a], b.hi[a]] {
                if v > lo[a] && v < hi[a] {
                    c.push(v);
                }
            }
        }
        c.sort_by(|x, y| x.total_cmp(y));
        c.dedup();
        c
    });
    let mut acc: Vec<(u16, f64)> = Vec::new();
    for z in cuts[2].windows(2) {
        for y in cuts[1].windows(2) {
            for x in cuts[0].windows(2) {
                let c = [
                    0.5 * (x[0] + x[1]),
                    0.5 * (y[0] + y[1]),
                    0.5 * (z[0] + z[1]),
                ];
                let id = boxes
                    .iter()
                    .find(|b| (0..3).all(|a| c[a] > b.lo[a] && c[a] < b.hi[a]))
                    .map_or(0, |b| b.material);
                let v = (x[1] - x[0]) * (y[1] - y[0]) * (z[1] - z[0]);
                match acc.iter_mut().find(|p| p.0 == id) {
                    Some(p) => p.1 += v,
                    None => acc.push((id, v)),
                }
            }
        }
    }
    acc.sort_by_key(|p| p.0);
    acc
}

fn bodies(grid: &Grid3, regs: &[Region], materials: &[MaterialInfo]) -> Vec<BodyResolution> {
    regs.iter()
        .map(|r| {
            let across = |a: usize| {
                let e = edges(grid, a);
                e.windows(2)
                    .filter(|w| {
                        let c = 0.5 * (w[0] + w[1]);
                        c > r.lo[a] && c < r.hi[a]
                    })
                    .count()
            };
            BodyResolution {
                material: r.material,
                name: materials
                    .iter()
                    .find(|m| m.id == r.material)
                    .map_or_else(String::new, |m| m.name.clone()),
                cells_across: [across(0), across(1), across(2)],
                thin: std::array::from_fn(|a| r.hi[a] - r.lo[a] < THIN_MM),
            }
        })
        .collect()
}
