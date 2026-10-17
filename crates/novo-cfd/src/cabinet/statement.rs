//! What the app may claim about a result, per level.
//!
//! The flow core is validated on closed boxes (see `accuracy` and
//! `docs/thermal-validation.md`). A cabinet result adds the wall films, the
//! solids, the ducts and the openings on top; those are validated for the
//! enclosure temperature only when [`ENCLOSURE_VALIDATED`] is set, which
//! happens together with the validation row that supports it. Until then
//! the app labels every result "not validated" and says why.

use crate::accuracy::flow_core_statement;
use crate::result::{AccuracyStatement, Fidelity};
use crate::turb::TurbModel;

/// Set when the cabinet against IEC 60890 and the 2D engine is within the
/// tolerance of `tests/validation_cabinet.rs` for the sealed and the vented
/// reference cabinet. Flip it in the PR that makes that test pass.
pub const ENCLOSURE_VALIDATED: bool = false;

/// The accuracy statement of a result at `fidelity`.
pub fn accuracy_for(fidelity: Fidelity) -> AccuracyStatement {
    let core = flow_core_statement();
    let level = match fidelity {
        Fidelity::Coarse => {
            "Coarse grid: shows where the heat goes; temperatures are a first look."
        }
        Fidelity::Medium => "Medium grid: use it to compare layouts.",
        _ => "Fine grid: the best this build offers.",
    };
    let mut limits = vec![
        "Device housing temperatures use the 2D engine's contact to the plate and surface film, \
         so on a grid of 1500 cells they agree with it to about 5 %. On finer grids the air next \
         to the device is resolved and comes out hotter than the 2D slice average: in the \
         sealed 50 W reference case the housing rise is 10 to 20 % above the 2D estimate from \
         6000 cells up and still creeps up with refinement (88 C at 6000 cells, about 92 C at \
         150000, against 82 C). Neither engine is checked \
         against a measurement."
            .into(),
        "Turbulence and radiation are not modelled: surface temperatures of hot devices are \
         slightly high without radiation, and flow in a strongly forced cabinet is laminar."
            .into(),
    ];
    limits.extend(
        core.limits
            .into_iter()
            .filter(|l| !l.starts_with("Open vents")),
    );
    if !ENCLOSURE_VALIDATED {
        limits.insert(
            0,
            "Enclosure temperatures are not yet validated against IEC 60890 and the 2D engine."
                .into(),
        );
    }
    AccuracyStatement {
        validated: ENCLOSURE_VALIDATED,
        headline: format!("{} {level}", fidelity.label()),
        benchmarks: core.benchmarks,
        limits,
    }
}

/// The accuracy statement of a result with a turbulence model: the generic
/// "turbulence is not modelled" limit is replaced by what the model does and
/// where it stops being valid.
pub fn accuracy_with_turbulence(fidelity: Fidelity, model: TurbModel) -> AccuracyStatement {
    accuracy_with(fidelity, model, false)
}

/// As [`accuracy_with_turbulence`], and with `radiation` the statement says
/// radiation is coupled (grey diffuse surfaces) instead of not modelled.
pub fn accuracy_with(fidelity: Fidelity, model: TurbModel, radiation: bool) -> AccuracyStatement {
    let mut a = accuracy_for(fidelity);
    if model != TurbModel::Laminar {
        turbulence_limit(&mut a, model);
    }
    if radiation {
        radiation_limit(&mut a, model != TurbModel::Laminar);
    }
    a
}

const RADIATION_LIMIT: &str = "Radiation: grey diffuse surface-to-surface exchange between devices, \
     rails, plate and walls with view factors by ray sampling (checked against analytic cube and \
     parallel-plate values); emissivities are planning values, not measured. Air is transparent. \
     Enclosure walls are boundary conditions that release what they absorb into the adjacent \
     air, so the wall's own heat capacity is not resolved. Not yet checked against a measurement.";

fn radiation_limit(a: &mut AccuracyStatement, turbulence: bool) {
    let key = if turbulence {
        "Turbulence:"
    } else {
        "Turbulence and radiation"
    };
    if let Some(l) = a.limits.iter_mut().find(|l| l.starts_with(key)) {
        if turbulence {
            *l = l.replace(
                "radiation is not modelled, so surface temperatures of hot devices are slightly high. ",
                "",
            );
            a.limits.push(RADIATION_LIMIT.into());
        } else {
            *l = "Turbulence is not modelled: flow in a strongly forced cabinet is laminar.".into();
            a.limits.insert(1, RADIATION_LIMIT.into());
        }
    }
    a.headline.push_str(" Radiation: surface to surface.");
}

fn turbulence_limit(a: &mut AccuracyStatement, model: TurbModel) {
    let name = match model {
        TurbModel::KOmegaSst => "k-omega SST",
        _ => "k-epsilon (cross check)",
    };
    if let Some(first) = a
        .limits
        .iter_mut()
        .find(|l| l.starts_with("Turbulence and radiation"))
    {
        *first = format!(
            "Turbulence: {name} with wall functions; radiation is not modelled, so surface \
             temperatures of hot devices are slightly high. The wall functions assume attached \
             flow, and the heat flux of natural-convection walls is under-predicted when the \
             first cell is thicker than the boundary layer (coarse grids)."
        );
    }
    a.headline.push_str(&format!(" Turbulence: {name}."));
}

/// Fixes the first note of a result ("Turbulence and radiation are not
/// modelled yet.") for a run with coupled radiation.
pub fn radiation_notes(notes: &mut [String], model: TurbModel) {
    let to = if model == TurbModel::Laminar {
        "Turbulence is not modelled yet. Radiation is coupled (surface to surface)."
    } else {
        "Radiation is coupled (surface to surface)."
    };
    for n in notes {
        *n = n.replace("Turbulence and radiation are not modelled yet.", to);
    }
}
