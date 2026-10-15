//! Cabinet checks: devices not placed, wires and ducts (novo-routing).

use std::collections::BTreeSet;

use novo_core::{AnyId, CabinetPlacement, DeviceKind, SymbolRole, WireId};
use novo_routing::model::{Connection, route_connections};
use novo_routing::{Problem as RoutingProblem, Rules};

use crate::{Context, Problem, ProblemKind};

fn started(cx: &Context<'_>) -> bool {
    cx.project.all::<CabinetPlacement>().next().is_some()
}

/// Devices drawn in the schematic but not on any plate, once the layout is
/// started. Terminal strips count: they sit on the plate too.
pub(crate) fn unplaced_devices(cx: &Context<'_>, out: &mut Vec<Problem>) {
    if !started(cx) {
        return;
    }
    let placed: BTreeSet<_> = cx
        .project
        .all::<CabinetPlacement>()
        .map(|p| p.device)
        .collect();
    for device in cx.project.devices() {
        let drawn = cx.by_device.contains_key(&device.id)
            || (device.kind == DeviceKind::TerminalStrip
                && cx.project.symbols().any(|s| {
                    matches!(&s.role, SymbolRole::Terminal { terminals }
                    if terminals.iter().any(|t| {
                        cx.project.terminal(*t).is_some_and(|t| t.strip == device.id)
                    }))
                }));
        if drawn && !placed.contains(&device.id) {
            out.push(Problem::new(
                ProblemKind::UnplacedDevice,
                vec![AnyId::Device(device.id)],
                format!("{} is not placed on a mounting plate", device.tag),
            ));
        }
    }
}

/// Routes the wires between placed devices and reports wires in free air and
/// ducts above their fill limit.
pub(crate) fn routing(cx: &Context<'_>, out: &mut Vec<Problem>) {
    if !started(cx) {
        return;
    }
    let project = cx.project;
    let connections: Vec<Connection> = project
        .wires()
        .filter_map(|w| {
            Some(Connection {
                id: w.id.raw(),
                from: w.from.as_pin()?,
                to: w.to.as_pin()?,
                // Conductor plus insulation: 1.5 mm² comes out at 3 mm.
                diameter_mm: w
                    .cross_section_mm2
                    .filter(|s| *s > 0.0)
                    .map(|s| 2.0 * (s / std::f64::consts::PI).sqrt() + 1.6),
            })
        })
        .collect();
    let routed = route_connections(project, &connections, &Rules::default());
    let wire = |job: u64| project.wires().find(|w| w.id.raw() == job).map(|w| w.id);
    for (_, plan) in &routed.plans {
        for problem in &plan.problems {
            match problem {
                RoutingProblem::FreeAir { job } => {
                    let Some(w) = wire(*job) else { continue };
                    out.push(Problem::new(
                        ProblemKind::WireWithoutDuct,
                        vec![AnyId::Wire(w)],
                        "No duct serves this wire: it would run in free air".to_owned(),
                    ));
                }
                RoutingProblem::OverfullDuct { ratio, jobs, .. } => {
                    let wires: Vec<WireId> = jobs.iter().filter_map(|j| wire(*j)).collect();
                    let mut at: Vec<AnyId> = wires.into_iter().map(AnyId::Wire).collect();
                    if at.is_empty() {
                        continue;
                    }
                    at.sort();
                    out.push(Problem::new(
                        ProblemKind::OverfullDuct,
                        at,
                        format!(
                            "A duct carries {}% of its fill limit",
                            (ratio * 100.0).round()
                        ),
                    ));
                }
                _ => {}
            }
        }
    }
}
