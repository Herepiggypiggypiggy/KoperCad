//! Wire routing problems (novo-routing, docs/design/wire-routing.md §5): a
//! wire no duct reaches, a duct fuller than its limit, a duct without a
//! size, and wires whose size is guessed.

use std::collections::BTreeMap;

use novo_core::{AnyId, Rail, Structure, SymbolId};
use novo_routing::Problem as Routing;
use novo_routing::Rules;
use novo_routing::model::{Connection, route_connections};

use crate::{Context, Problem, ProblemKind};

/// Routes `connections` and returns what the routing found wrong.
///
/// The connections are the wire list's (one per pair of pins a wire joins).
/// They come from the caller because `check` has no wire list of its own;
/// connections that cannot be routed (device not placed, two plates) are
/// not problems here.
pub(crate) fn routing(
    cx: &Context<'_>,
    connections: &[Connection],
    rules: &Rules,
    out: &mut Vec<Problem>,
) {
    let project = cx.project;
    let routed = route_connections(project, connections, rules);
    let by_id: BTreeMap<u64, &Connection> = connections.iter().map(|c| (c.id, c)).collect();
    let label = |id: u64| {
        by_id.get(&id).map_or_else(
            || format!("Wire {id}"),
            |c| {
                format!(
                    "{} to {}",
                    cx.endpoint_label(c.from.into()),
                    cx.endpoint_label(c.to.into())
                )
            },
        )
    };
    let symbols = |ids: &[u64]| -> Vec<AnyId> {
        let mut found: Vec<SymbolId> = ids
            .iter()
            .filter_map(|id| by_id.get(id))
            .flat_map(|c| [c.from.symbol, c.to.symbol])
            .collect();
        found.sort();
        found.dedup();
        found.into_iter().map(AnyId::from).collect()
    };
    let duct_name = |duct: u64| {
        let rail = project.all::<Rail>().find(|r| r.id.0 == duct);
        match rail {
            Some(r) => {
                let place = project
                    .get::<Structure>(r.structure)
                    .map_or(String::new(), |s| format!(" in {}", s.tag));
                format!("Duct at {:.0}, {:.0} mm{place}", r.x, r.y)
            }
            None => format!("Duct {duct}"),
        }
    };
    let rail_id = |duct: u64| -> Vec<AnyId> {
        project
            .all::<Rail>()
            .find(|r| r.id.0 == duct)
            .map(|r| vec![AnyId::from(r.id)])
            .unwrap_or_default()
    };
    for (_, plan) in &routed.plans {
        let mut guessed = Vec::new();
        for problem in &plan.problems {
            match problem {
                Routing::FreeAir { job } => out.push(Problem::new(
                    ProblemKind::WireRouting,
                    symbols(&[*job]),
                    format!(
                        "{}: no duct reaches both ends, so the wire runs in free air",
                        label(*job)
                    ),
                )),
                Routing::OverfullDuct { duct, ratio, jobs } => {
                    let mut at = rail_id(*duct);
                    at.extend(symbols(jobs));
                    out.push(Problem::new(
                        ProblemKind::WireRouting,
                        at,
                        format!(
                            "{} is {:.0}% of its allowed fill with {} wires",
                            duct_name(*duct),
                            ratio * 100.0,
                            jobs.len()
                        ),
                    ));
                }
                Routing::UnsizedDuct { duct, jobs } => out.push(Problem::new(
                    ProblemKind::RoutingData,
                    rail_id(*duct),
                    format!(
                        "{} has no size, so the fill of its {} wires cannot be checked",
                        duct_name(*duct),
                        jobs.len()
                    ),
                )),
                Routing::GuessedWire { job } => guessed.push(*job),
            }
        }
        if !guessed.is_empty() {
            out.push(Problem::new(
                ProblemKind::RoutingData,
                symbols(&guessed),
                format!(
                    "{} without a known size count as {} mm in the duct fill",
                    crate::checks::plural(guessed.len(), "wire", "wires"),
                    novo_routing::DEFAULT_WIRE_DIAMETER_MM
                ),
            ));
        }
    }
}
