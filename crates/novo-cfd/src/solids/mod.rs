//! Solids in the flow grid: conjugate heat transfer (lane CFD-C2).
//!
//! The grid of `mesh` knows, per cell, which material fills it and how much
//! (exact volume shares). A [`Medium`] turns that into what the flow core
//! needs:
//!
//! - **Flow**: a face between two cells is open by the smaller of their air
//!   shares, closed below [`MIN_OPEN`]. Closed faces carry no flow and act as
//!   no-slip walls; a cell with no open face drops out of the pressure
//!   system.
//! - **Heat**: every cell has a volumetric heat capacity and a conductivity
//!   weighted by the volume shares of its materials. Conduction between
//!   cells uses the harmonic mean of the two cell conductivities (series
//!   layers add resistances), which is exact for a layer boundary on a face
//!   and for any stack of layers normal to the heat flow.
//! - **Sources**: a device's loss is spread over its volume.
//!
//! Cells cut by a surface (partly solid) use the arithmetic share-weighted
//! conductivity: exact for heat flowing along the layers, an overestimate
//! across them. Meshes from `mesh` put planes on device and plate faces, so
//! cut cells are the exception, not the rule.
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate: devices heat
//! the plate, the rails and the walls, and the result shows their surface
//! temperatures. App side: lane CFD-F.

mod props;
mod setup;

pub use props::{SolidProps, props_for};
pub use setup::{
    DuctModel, FlowSetup, STEADY_WINDOW_S, SetupError, SolidsSettings, SteadyReport, Walls,
    wall_conductances,
};

use crate::flow::Mesh as FlowMesh;

/// Faces between cells with an air share below this are closed.
pub const MIN_OPEN: f64 = 0.1;

/// Per-cell content of a grid, ready for the flow core.
#[derive(Debug, Clone, PartialEq)]
pub struct Medium {
    /// Air share of each cell, 0 to 1.
    pub fluid_frac: Vec<f64>,
    /// Heat capacity of the solid part, J/m3K of the whole cell.
    pub c_solid: Vec<f64>,
    /// Conductivity of the solid part, W/mK weighted by its share.
    pub k_solid: Vec<f64>,
    /// Heating, W per m3 of the whole cell.
    pub heat_w_m3: Vec<f64>,
    /// Quadratic drag of a porous cell per axis, 1/m: the momentum sink is
    /// `-F |u| u` (zero in open air).
    pub drag: [Vec<f64>; 3],
}

impl Medium {
    /// All air.
    pub fn air(cells: usize) -> Medium {
        Medium {
            fluid_frac: vec![1.0; cells],
            c_solid: vec![0.0; cells],
            k_solid: vec![0.0; cells],
            heat_w_m3: vec![0.0; cells],
            drag: std::array::from_fn(|_| vec![0.0; cells]),
        }
    }

    /// Any porous cell.
    pub fn has_drag(&self) -> bool {
        self.drag.iter().flatten().any(|f| *f > 0.0)
    }

    pub fn cells(&self) -> usize {
        self.fluid_frac.len()
    }

    /// Volumetric heat capacity of cell `i`, J/m3K.
    pub fn cell_rho_cp(&self, i: usize, air_rho_cp: f64) -> f64 {
        self.c_solid[i] + self.fluid_frac[i] * air_rho_cp
    }

    /// Conductivity of cell `i`, W/mK.
    pub fn cell_k(&self, i: usize, air_k: f64) -> f64 {
        self.k_solid[i] + self.fluid_frac[i] * air_k
    }

    pub fn heat_w_m3(&self, i: usize) -> f64 {
        self.heat_w_m3[i]
    }

    /// Open share of the face between two cells.
    pub fn face_open(&self, lo: usize, hi: usize) -> f64 {
        let o = self.fluid_frac[lo].min(self.fluid_frac[hi]);
        if o < MIN_OPEN { 0.0 } else { o }
    }

    /// Sets the open share of every interior face in `afrac` (indexed like
    /// the velocity components of `mesh`); leaves the domain boundary alone.
    pub fn block_faces(&self, mesh: &FlowMesh, afrac: &mut [Vec<f64>; 3]) {
        for a in 0..3 {
            let d = mesh.dims(a);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[a] == 0 || ix[a] == mesh.n[a] {
                            continue;
                        }
                        let mut lo = ix;
                        lo[a] -= 1;
                        afrac[a][mesh.idx(a, ix)] = self.face_open(mesh.cell(lo), mesh.cell(ix));
                    }
                }
            }
        }
    }

    /// Total heating, W.
    pub fn total_heat_w(&self, mesh: &FlowMesh) -> f64 {
        let mut sum = 0.0;
        for k in 0..mesh.n[2] {
            for j in 0..mesh.n[1] {
                for i in 0..mesh.n[0] {
                    sum += self.heat_w_m3[mesh.cell([i, j, k])] * mesh.volume([i, j, k]);
                }
            }
        }
        sum
    }
}
