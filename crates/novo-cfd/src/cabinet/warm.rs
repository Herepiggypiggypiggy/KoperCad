//! Initial temperatures from the 2D estimate of `novo-thermal`.
//!
//! Starting at ambient makes the march spend most of its time heating the
//! air and the solids. The 2D engine is cheap and already close in the bulk
//! (calibrated against IEC 60890), so the 3D march only has to develop the
//! depth structure and the local plumes. Only the starting point comes from
//! it: the converged 3D state does not depend on it.

use novo_thermal::{Options, Scenario, simulate_steady_with};

use crate::flow::FlowSolver;
use crate::mesh::CellClass;
use crate::result::MaterialKind;
use crate::solids::FlowSetup;

/// Sets `flow.t` from the 2D steady state; leaves ambient when the 2D engine
/// cannot run the scenario. Returns whether it was used.
pub fn warm_start(setup: &FlowSetup, scn: &Scenario, flow: &mut FlowSolver) -> bool {
    let Ok(r) = simulate_steady_with(
        scn,
        Options {
            bands: false,
            advice: false,
        },
    ) else {
        return false;
    };
    let cm = &setup.classified;
    let m = &flow.mesh;
    for k in 0..m.n[2] {
        for j in 0..m.n[1] {
            for i in 0..m.n[0] {
                let c = m.cell([i, j, k]);
                let (x, y) = (m.centre[0][i] * 1e3, m.centre[1][j] * 1e3);
                let air = r.air_temp.sample(x, y);
                flow.t[c] = match cm.class[c] {
                    CellClass::Fluid | CellClass::Cut => air,
                    CellClass::Solid => {
                        let kind = cm
                            .materials
                            .iter()
                            .find(|mi| mi.id == cm.material[c])
                            .map(|mi| mi.kind);
                        match kind {
                            Some(MaterialKind::Device(id)) => r
                                .devices
                                .iter()
                                .find(|d| d.id == id)
                                .map_or(air, |d| d.surface_c),
                            Some(MaterialKind::Plate) => r.plate_temp.sample(x, y),
                            _ => air,
                        }
                    }
                };
            }
        }
    }
    true
}
