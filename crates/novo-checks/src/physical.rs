//! Cabinet checks: devices not placed, wires and ducts (novo-routing).

use std::collections::BTreeSet;

use novo_core::{AnyId, CabinetPlacement, DeviceId, DeviceKind, SymbolRole, WireId};
use novo_layout::model::load;
use novo_routing::Problem as RoutingProblem;
use novo_routing::model::route_connections;

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
    let connections = novo_routing::model::wire_connections(project);
    let routed = route_connections(
        project,
        &connections,
        &novo_routing::WireRules::of(project).routing,
    );
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

/// Collisions, devices off the plate, tight spacing and full rails on each
/// enclosure's mounting plate, from novo-layout's report.
pub(crate) fn layout(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let tag = |id: u64| {
        project
            .device(DeviceId::from(id))
            .map_or_else(|| "A device".to_owned(), |d| d.tag.to_string())
    };
    let pair = |a: u64, b: u64| {
        let mut at = vec![
            AnyId::Device(DeviceId::from(a)),
            AnyId::Device(DeviceId::from(b)),
        ];
        at.sort();
        at
    };
    for structure in project.structures().filter(|s| s.plate.is_some()) {
        let Some(layout) = load(project, structure.id) else {
            continue;
        };
        let report = layout.report();
        for (a, b) in report.collisions {
            out.push(Problem::new(
                ProblemKind::DeviceCollision,
                pair(a, b),
                format!(
                    "{} and {} overlap on the plate of {}",
                    tag(a),
                    tag(b),
                    structure.tag
                ),
            ));
        }
        for id in report.outside {
            out.push(Problem::new(
                ProblemKind::DeviceOutsidePlate,
                vec![AnyId::Device(DeviceId::from(id))],
                format!(
                    "{} leaves the usable area of the plate of {}",
                    tag(id),
                    structure.tag
                ),
            ));
        }
        for (a, b, gap) in report.too_close {
            out.push(Problem::new(
                ProblemKind::DevicesTooClose,
                pair(a, b),
                format!(
                    "{} and {} are {} mm apart, closer than their clearance",
                    tag(a),
                    tag(b),
                    gap.round()
                ),
            ));
        }
        for (rail, over) in report.rail_overflows {
            let devices: Vec<AnyId> = layout
                .placements
                .iter()
                .filter(|p| p.rail == Some(rail))
                .map(|p| AnyId::Device(DeviceId::from(p.id)))
                .collect();
            if devices.is_empty() {
                continue;
            }
            out.push(Problem::new(
                ProblemKind::RailOverflow,
                devices,
                format!(
                    "The devices on a rail of {} need {} mm more than it has",
                    structure.tag,
                    over.round()
                ),
            ));
        }
    }
}
