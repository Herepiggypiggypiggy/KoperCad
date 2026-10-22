//! EMC findings (novo-emc) as design-check problems.
//!
//! Opt-in with the project property `emc.check`: a project that never asked
//! for EMC checks gets none. Every problem carries its rule id (`EMC-01`),
//! the level of its method in the text and the first suggestion; the whole
//! picture (scores, assumptions, what is not checked) is in the report that
//! [`novo_emc::project::analyse_with`] returns.

use novo_core::{AnyId, CableId, DeviceId, Netlist, StructureId, WireId};
use novo_emc::{Severity as EmcSeverity, Subject};

use crate::{Context, Problem, ProblemKind, Severity};

fn any(subject: Subject) -> AnyId {
    match subject {
        Subject::Run(id) => AnyId::Wire(WireId::from(id)),
        Subject::Device(id) => AnyId::Device(DeviceId::from(id)),
        Subject::Cable(id) => AnyId::Cable(CableId::from(id)),
        Subject::Plate(id) => AnyId::Structure(StructureId::from(id)),
    }
}

pub(crate) fn run(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    if !novo_emc::project::is_enabled(cx.project) {
        return;
    }
    let report = novo_emc::project::analyse_with(cx.project, netlist);
    for f in &report.findings {
        let mut at: Vec<AnyId> = f.subjects.iter().copied().map(any).collect();
        // Drawn objects first: wires, then the rest, as the other checks do.
        at.sort_by_key(|id| !matches!(id, AnyId::Wire(_)));
        let hint = f.fixes.first().map(|x| format!(" Suggestion: {}", x.hint())).unwrap_or_default();
        out.push(Problem {
            kind: ProblemKind::Emc,
            severity: match f.severity {
                EmcSeverity::Error => Severity::Error,
                EmcSeverity::Warning => Severity::Warning,
                EmcSeverity::Info => Severity::Info,
            },
            at,
            page: None,
            position: None,
            message: format!("{} ({}, {}){hint}", f.message, f.rule, f.level.label().to_lowercase()),
            rule: Some(f.rule),
        });
    }
}
