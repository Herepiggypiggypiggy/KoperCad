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
        "The share of a device's heat that leaves through the mounting plate is not resolved \
         (reads 0); device temperatures matched the 2D estimate in the sealed reference case."
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
