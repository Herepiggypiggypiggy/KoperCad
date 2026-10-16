//! Planning values of the solid materials in a cabinet.
//!
//! Every number is a stated assumption of typical bulk values, not a
//! measurement of the user's parts. Devices are the soft spot: a
//! contactor or drive is a shell of plastic or metal around air and
//! windings, so its conductivity and heat capacity are effective numbers.
//! The scenario's own heat capacity (`Device::heat_capacity_j_per_k`) wins
//! over the default.

use novo_thermal::{Device, Scenario};

use crate::result::MaterialKind;

/// Conductivity (W/mK) and volumetric heat capacity (J/m3K) of a solid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidProps {
    pub k: f64,
    pub rho_cp: f64,
}

const STEEL: SolidProps = SolidProps {
    k: 50.0,
    rho_cp: 7850.0 * 490.0,
};
/// Effective device: housing, windings and air inside.
const DEVICE_K: f64 = 2.0;
/// 500 J/K per dm2 of footprint, the 2D model's default capacity.
const DEVICE_CAP_J_PER_K_DM2: f64 = 500.0;
/// PVC wire duct (the slotted duct is a porous medium, see the duct model).
const DUCT: SolidProps = SolidProps {
    k: 0.17,
    rho_cp: 1400.0 * 1000.0,
};

/// Properties of material `kind` in `scn`; `volume_m3` is the volume the
/// mesh found for it (used to turn a device's capacity into J/m3K).
pub fn props_for(kind: MaterialKind, scn: &Scenario, volume_m3: f64) -> SolidProps {
    match kind {
        MaterialKind::Air => SolidProps {
            k: 0.026,
            rho_cp: 1.18 * 1005.0,
        },
        MaterialKind::Plate => SolidProps {
            k: scn.params.plate_k,
            rho_cp: STEEL.rho_cp,
        },
        MaterialKind::Wall | MaterialKind::Rail => STEEL,
        MaterialKind::Duct => DUCT,
        MaterialKind::Device(id) => {
            let dev = scn.devices.iter().find(|d| d.id == id);
            let cap = dev.map_or(0.0, capacity_j_per_k).max(1.0);
            SolidProps {
                k: DEVICE_K,
                rho_cp: cap / volume_m3.max(1e-9),
            }
        }
    }
}

/// Heat capacity of a device, J/K.
pub fn capacity_j_per_k(d: &Device) -> f64 {
    d.heat_capacity_j_per_k.unwrap_or_else(|| {
        let area_dm2 = d.footprint.w * d.footprint.h / 1.0e4;
        DEVICE_CAP_J_PER_K_DM2 * area_dm2
    })
}
