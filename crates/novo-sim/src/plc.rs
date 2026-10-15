//! PLC channels under simulation (docs/design/plc.md, "Simulation"). There
//! is no PLC program: a digital output (DQ) is switched by hand in the live
//! view and puts the plant's DC positive potential on its pin; a digital
//! input (DI) shows whether its pin carries a potential.

use std::collections::BTreeSet;

use novo_core::{Endpoint, Netlist, PotentialId, PotentialKind, Project, SymbolId, SymbolInstance};

use crate::Solution;

fn name_of(symbol: &SymbolInstance) -> &str {
    symbol.symbol.0.rsplit('/').next().unwrap_or("")
}

/// A digital input channel symbol (`iec/plc/digital_input`).
pub fn is_plc_input(symbol: &SymbolInstance) -> bool {
    symbol.device().is_some() && symbol.symbol.0.starts_with("iec/plc/") && {
        name_of(symbol) == "digital_input"
    }
}

/// A digital output channel symbol (`iec/plc/digital_output`). A relay
/// output is a make contact and is switched like any other pole.
pub fn is_plc_output(symbol: &SymbolInstance) -> bool {
    symbol.device().is_some() && symbol.symbol.0.starts_with("iec/plc/") && {
        name_of(symbol) == "digital_output"
    }
}

/// The potential an output puts on its pin: the first DC positive potential
/// of the project, else the first signal potential.
fn supply(project: &Project) -> Option<PotentialId> {
    let find = |kind| project.potentials().find(|p| p.kind == kind).map(|p| p.id);
    find(PotentialKind::DcPositive).or_else(|| find(PotentialKind::Signal))
}

/// What the outputs in `on` drive: their pin, and the supply potential. Empty
/// when the project has no supply potential.
pub fn output_drives(project: &Project, on: &BTreeSet<SymbolId>) -> Vec<(Endpoint, PotentialId)> {
    let Some(potential) = supply(project) else {
        return Vec::new();
    };
    project
        .symbols()
        .filter(|s| on.contains(&s.id) && is_plc_output(s))
        .map(|s| (Endpoint::pin(s.id, 0), potential))
        .collect()
}

/// The inputs whose pin carries a potential.
pub fn inputs_seen(
    project: &Project,
    netlist: &Netlist,
    solution: &Solution,
) -> BTreeSet<SymbolId> {
    project
        .symbols()
        .filter(|s| is_plc_input(s))
        .filter(|s| {
            solution
                .of(netlist, Endpoint::pin(s.id, 0))
                .potential()
                .is_some()
        })
        .map(|s| s.id)
        .collect()
}
