//! Cut lengths for the wire list: routed through the cabinet's ducts by
//! `novo-routing` where both ends are placed on one mounting plate,
//! estimated otherwise.

use novo_core::{PinRef, Project, StructureId};
use novo_routing::model::route_connections;
use novo_routing::{Kind, WireRules};

/// How a cut length was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// Through the ducts of the mounting plate both devices are placed on.
    Routed,
    /// Routed, but a device has no depth: the wire is measured from the plate,
    /// so it is too short by the rise to the terminals.
    RoutedFlat,
    /// Both devices are placed on one plate but no duct reaches both: a
    /// straight Manhattan line, which the routing engine flags as a problem.
    FreeAir,
    /// One end is on the door or another surface: the plate leg is routed,
    /// the leg on the surface is measured from the opening, and the distance
    /// between plate and door and the hinge loop are left out.
    Door,
    /// At least one device is not placed on a plate: see [`estimate`].
    Estimate,
}

impl Basis {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Basis::Routed => "routed",
            Basis::RoutedFlat => "routed, no depth",
            Basis::FreeAir => "free air",
            Basis::Door => "door, no gap",
            Basis::Estimate => "estimate",
        }
    }
}

/// The estimate for a wire whose route is unknown: the Manhattan distance
/// between its two pins on the schematic page in millimetres (at least
/// `wire.estimate_min_mm`), or `wire.estimate_across_pages_mm` when the pins are
/// on different pages, plus the service loop at each end. The drawing is not
/// to cabinet scale, so this is a planning figure only, and the report
/// labels it "estimate".
fn estimate(project: &Project, rules: &WireRules, from: PinRef, to: PinRef) -> f64 {
    let same_page = project
        .symbol(from.symbol)
        .zip(project.symbol(to.symbol))
        .is_some_and(|(a, b)| a.page == b.page);
    let base = match (
        same_page,
        project.endpoint_position(from.into()),
        project.endpoint_position(to.into()),
    ) {
        (true, Some(a), Some(b)) => {
            ((a.x - b.x).abs() + (a.y - b.y).abs()).max(rules.estimate_min_mm)
        }
        _ => rules.estimate_across_pages_mm,
    };
    base + 2.0 * rules.routing.slack_mm
}

/// One connection to measure: its two pins and the wire's cross-section.
pub(crate) struct Connection {
    pub from: PinRef,
    pub to: PinRef,
    pub cross_section_mm2: Option<f64>,
}

/// A measured connection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CutLength {
    pub mm: f64,
    pub basis: Basis,
    /// The ducts the wire lies in, in order (empty when not ducted).
    pub ducts: Vec<u64>,
}

/// The lengths of a set of connections and the plans they were routed in.
pub(crate) struct Measurement {
    pub lengths: Vec<CutLength>,
    /// Per enclosure the duct fill after routing all of its wires.
    pub plans: Vec<(StructureId, novo_routing::Plan)>,
}

/// The cut length of every connection, in the given order. Connections
/// between devices placed on the same plate are routed by
/// [`route_connections`] (duct fill steers later wires); the rest get
/// [`estimate`]. A length is rounded up to a whole millimetre and is never
/// zero.
pub(crate) fn cut_lengths(project: &Project, connections: &[Connection]) -> Vec<CutLength> {
    measure(project, connections).lengths
}

/// [`cut_lengths`] with the duct fill of each plate the wires were routed on.
pub(crate) fn measure(project: &Project, connections: &[Connection]) -> Measurement {
    let jobs: Vec<novo_routing::model::Connection> = connections
        .iter()
        .enumerate()
        .map(|(i, c)| novo_routing::model::Connection {
            id: i as u64,
            from: c.from,
            to: c.to,
            diameter_mm: novo_routing::model::wire_diameter_mm(c.cross_section_mm2),
        })
        .collect();
    let rules = WireRules::of(project);
    let routed = route_connections(project, &jobs, &rules.routing);
    let lengths = connections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let found = match routed.routes.get(&(i as u64)) {
                Some(route) => CutLength {
                    mm: route.length_mm,
                    basis: match route.kind {
                        // Through an opening to the door: a model, the gap and
                        // the hinge loop are not in the length.
                        _ if routed
                            .surface_legs
                            .get(&(i as u64))
                            .is_some_and(|leg| leg.opening.is_some()) =>
                        {
                            Basis::Door
                        }
                        Kind::Ducted if routed.unknown_height.contains(&(i as u64)) => {
                            Basis::RoutedFlat
                        }
                        Kind::Ducted => Basis::Routed,
                        Kind::FreeAir => Basis::FreeAir,
                    },
                    ducts: route.ducts.clone(),
                },
                None => CutLength {
                    mm: estimate(project, &rules, c.from, c.to),
                    basis: Basis::Estimate,
                    ducts: Vec::new(),
                },
            };
            CutLength {
                mm: found.mm.ceil().max(1.0),
                ..found
            }
        })
        .collect();
    Measurement {
        lengths,
        plans: routed.plans,
    }
}
