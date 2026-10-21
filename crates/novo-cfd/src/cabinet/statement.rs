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
    if ENCLOSURE_VALIDATED {
        limits.insert(
            0,
            "Checked against IEC 60890 and the 2D engine on a sealed and a vented reference \
             cabinet (tests/validation_cabinet.rs); not checked against a measurement."
                .into(),
        );
    } else {
        limits.insert(
            0,
            "Enclosure temperatures are not yet validated. Reference study against the 2D engine \
             (sealed 50 W, vented 100 W): from 1500 to 6000 cells the mean air rise is 11 to 19 % \
             below it (the 3D mean is lower because the air stratifies; IEC 60890 sealed: 13 to \
             14 %) and the housing rise 5 to 11 % above it; at 20000 cells the air rise is 16 % \
             (sealed) and 31 % (vented) below and the housing 13 to 14 % above, and both are still \
             moving with refinement, so the result is not yet independent of the grid."
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

const RADIATION_MARK: &str = " Radiation: surface to surface.";

/// Marks a statement as one of a run with coupled radiation: the "radiation
/// is not modelled" remarks go, what the radiation model assumes comes in.
/// Safe to call twice.
pub fn mark_radiation(a: &mut AccuracyStatement) {
    if a.headline.ends_with(RADIATION_MARK) {
        return;
    }
    for l in &mut a.limits {
        if l.starts_with("Turbulence and radiation are not modelled") {
            *l = "Turbulence is not modelled: flow in a strongly forced cabinet is laminar.".into();
        }
        for note in [
            " radiation is not modelled, so surface temperatures of hot devices are slightly high.",
            " Radiation is not modelled, so surface temperatures of hot devices are slightly high.",
        ] {
            if let Some(i) = l.find(note) {
                l.replace_range(i..i + note.len(), "");
            }
        }
    }
    a.limits.push(
        "Radiation is grey and diffuse, surface to surface between devices, plate and walls \
         (air is transparent). Emissivities are planning values (devices from the scenario, \
         painted wall 0.9, bare plate 0.3); faces of cut cells count whole; a wall's surface \
         temperature uses a fixed film ratio. Checked against analytic view factors, not \
         against a measurement."
            .into(),
    );
    a.headline.push_str(RADIATION_MARK);
}

/// The accuracy statement of a result with a turbulence model: the generic
/// "turbulence is not modelled" limit is replaced by what the model does and
/// where it stops being valid.
pub fn accuracy_with_turbulence(fidelity: Fidelity, model: TurbModel) -> AccuracyStatement {
    let mut a = accuracy_for(fidelity);
    if model == TurbModel::Laminar {
        return a;
    }
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
            "Turbulence: {name} with wall functions. The wall functions assume attached flow, \
             and the heat flux of natural-convection walls is under-predicted when the first \
             cell is thicker than the boundary layer (coarse grids). Device surfaces use the \
             device film model, not the wall function. In a vented 100 W box at 6000 cells the \
             mean air rise changed by 3 % or less against laminar, the device surface rise by \
             under 1 % and the exhaust temperature at the top louvre by about 4 %, at 3400 \
             cells by up to 11 %. With a 120 m3/h intake fan in the same box the mean air rise was \
             11 % (SST) to 15 % (k-epsilon) lower than laminar. The device surface rise was 1 % \
             to 1.4 % lower and the exhaust temperature at the top louvre up to about 5 % lower. \
             In that fan case the 3D mean air rise is about 1.4 to 1.8 times the 2D engine's \
             (2.8 to 3.5 K over 3400 to 24000 cells against 1.93 K; the 2D engine gave 1.33 K \
             before it got a fan-jet mixing term tuned to the CFD of this one box). Both close \
             the heat balance and the exhaust flows agree within about 20 %. The most likely \
             cause is that the 2D engine averages over the depth and has no jet momentum, so it \
             leaves a cold pool below the device (lower fifth 0.8 to 1.3 K against 2.3 to 2.5 K \
             in 3D); a cooler stream short-circuiting from the fan to the top louvre may add to \
             it but is not shown. The gap next to the device remains (2D 2.9 K, 3D 9.6 K). The \
             device surface rise agrees within 4 % at 60 and 120 m3/h (9 % at 240 m3/h). \
             Neither engine is validated against measurement, so \
             read air temperatures of a fan case as an estimate. Radiation is not modelled, so surface temperatures of hot devices are \
             slightly high."
        );
    }
    a.headline.push_str(&format!(" Turbulence: {name}."));
    a
}
