//! Grey surface-to-surface radiation (lane CFD-D).
//!
//! The pieces, each pure and tested on its own:
//!
//! 1. [`PatchSet`]: the exposed faces of the solid cells of a [`Mesh`]
//!    (solid against fluid) and the faces of the enclosure walls that look
//!    into the air, grouped into a coarse set of patches (flat, one
//!    material, one normal, one emissivity).
//! 2. [`ViewFactors`]: Monte Carlo ray sampling on the fine grid with a
//!    deterministic low-discrepancy sequence. Rays are blocked by every
//!    solid cell, so devices shadow each other. Reciprocity
//!    `A_i F_ij = A_j F_ji` and the closed-enclosure summation
//!    `sum_j F_ij = 1` are enforced exactly afterwards by a symmetric
//!    scaling, so the energy balance holds to round-off.
//! 3. [`radiosity::solve`]: the grey diffuse radiosity system
//!    `J_i = e_i sigma T_i^4 + (1 - e_i) sum_j F_ij J_j`, solved by
//!    Gauss-Seidel, with the net flux of every patch and a linearised film
//!    coefficient for an implicit coupling with the conduction in the solids.
//! 4. [`HeatPaths`]: the split of a device's heat into convection,
//!    conduction and radiation.
//!
//! Air does not take part (cabinet air is transparent to thermal
//! radiation). Faces of cut cells are taken whole, like the first-order
//! Cartesian treatment of the flow: a cell counts as solid when its largest
//! material share is not air.
//!
//! Coupling with the flow solver: [`RadPlan`] (see `couple.rs`) gives the
//! energy equation the radiative heat as `s - g T` per cell and plate
//! column, so the slope is taken implicitly. Enabled per run with
//! `FlowSetup::enable_radiation`, or `CabinetSolver::with_radiation`.
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate dialog,
//! "Radiation" toggle ([`RadSettings::enabled`]) and the heat split per
//! device (convection / conduction / radiation) in the device card (app
//! side is lane CFD-F).

mod couple;
mod patches;
mod paths;
pub mod radiosity;
mod trace;
mod viewfactor;

pub use couple::{RadInputs, RadLoad, RadPlan, RadState, WallBc};
pub use patches::{Emissivity, FaceRef, Patch, PatchKind, PatchSet, PatchSettings, RearPlate};
pub use paths::HeatPaths;
pub use radiosity::{RadResult, SIGMA};
pub use viewfactor::{ViewFactorSettings, ViewFactors};

use crate::mesh::Mesh;

/// Everything the radiation model needs to know.
#[derive(Debug, Clone, PartialEq)]
pub struct RadSettings {
    /// Off: no patches are built and no flux is produced.
    pub enabled: bool,
    pub emissivity: Emissivity,
    pub patches: PatchSettings,
    pub view_factors: ViewFactorSettings,
}

impl Default for RadSettings {
    fn default() -> Self {
        RadSettings {
            enabled: true,
            emissivity: Emissivity::default(),
            patches: PatchSettings::default(),
            view_factors: ViewFactorSettings::default(),
        }
    }
}

/// Patches and view factors of one mesh. Build once per grid, then call
/// [`RadiationModel::solve`] every outer iteration with fresh temperatures.
#[derive(Debug, Clone)]
pub struct RadiationModel {
    pub patches: PatchSet,
    pub view_factors: ViewFactors,
}

impl RadiationModel {
    /// `None` when radiation is switched off or the mesh has no surface.
    pub fn build(mesh: &Mesh, settings: &RadSettings) -> Option<RadiationModel> {
        RadiationModel::build_with(mesh, settings, None)
    }

    /// As [`RadiationModel::build`], with the mounting plate as a surface.
    pub fn build_with(
        mesh: &Mesh,
        settings: &RadSettings,
        plate: Option<&RearPlate>,
    ) -> Option<RadiationModel> {
        if !settings.enabled {
            return None;
        }
        let patches =
            PatchSet::from_mesh_with(mesh, &settings.emissivity, &settings.patches, plate);
        if patches.len() < 2 {
            return None;
        }
        let view_factors = ViewFactors::sample(mesh, &patches, &settings.view_factors);
        Some(RadiationModel {
            patches,
            view_factors,
        })
    }

    /// Radiosity solution for patch temperatures in kelvin.
    pub fn solve(&self, temperature_k: &[f64]) -> RadResult {
        radiosity::solve(&self.patches, &self.view_factors, temperature_k)
    }
}
