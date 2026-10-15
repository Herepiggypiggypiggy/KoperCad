//! Cut lengths for the wire list: routed through the cabinet's ducts by
//! `novo-routing` where both ends are placed on one mounting plate,
//! estimated otherwise.

use novo_core::{PinRef, Project};
use novo_routing::model::route_connections;
use novo_routing::{Kind, Rules};

/// How a cut length was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// Through the ducts of the mounting plate both devices are placed on.
    Routed,
    /// Both devices are placed on one plate but no duct reaches both: a
    /// straight Manhattan line, which the routing engine flags as a problem.
    FreeAir,
    /// At least one device is not placed on a plate: see [`estimate`].
    Estimate,
}

impl Basis {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Basis::Routed => "routed",
            Basis::FreeAir => "free air",
            Basis::Estimate => "estimate",
        }
    }
}

/// Service loop per end, the same as the routing engine's default.
fn slack_mm() -> f64 {
    Rules::default().slack_mm
}

/// Shortest estimated wire between two pins, before the slack.
pub(crate) const ESTIMATE_MIN_MM: f64 = 100.0;

/// Wire between pins with no usable schematic distance (different pages),
/// before the slack.
pub(crate) const ESTIMATE_ACROSS_PAGES_MM: f64 = 500.0;

/// The estimate for a wire whose route is unknown: the Manhattan distance
/// between its two pins on the schematic page in millimetres (at least
/// [`ESTIMATE_MIN_MM`]), or [`ESTIMATE_ACROSS_PAGES_MM`] when the pins are
/// on different pages, plus the service loop at each end. The drawing is not
/// to cabinet scale, so this is a planning figure only, and the report
/// labels it "estimate".
fn estimate(project: &Project, from: PinRef, to: PinRef) -> f64 {
    let same_page = project
        .symbol(from.symbol)
        .zip(project.symbol(to.symbol))
        .is_some_and(|(a, b)| a.page == b.page);
    let base = match (
        same_page,
        project.endpoint_position(from.into()),
        project.endpoint_position(to.into()),
    ) {
        (true, Some(a), Some(b)) => ((a.x - b.x).abs() + (a.y - b.y).abs()).max(ESTIMATE_MIN_MM),
        _ => ESTIMATE_ACROSS_PAGES_MM,
    };
    base + 2.0 * slack_mm()
}

/// Outer diameter of a wire of the given cross-section: the copper circle
/// plus about 0.7 mm of insulation each side. Only steers duct fill.
fn diameter_mm(cross_section_mm2: Option<f64>) -> Option<f64> {
    cross_section_mm2
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| 2.0 * (s / std::f64::consts::PI).sqrt() + 1.4)
}

/// One connection to measure: its two pins and the wire's cross-section.
pub(crate) struct Connection {
    pub from: PinRef,
    pub to: PinRef,
    pub cross_section_mm2: Option<f64>,
}

/// A measured connection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CutLength {
    pub mm: f64,
    pub basis: Basis,
}

/// The cut length of every connection, in the given order. Connections
/// between devices placed on the same plate are routed by
/// [`route_connections`] (duct fill steers later wires); the rest get
/// [`estimate`]. A length is rounded up to a whole millimetre and is never
/// zero.
pub(crate) fn cut_lengths(project: &Project, connections: &[Connection]) -> Vec<CutLength> {
    let jobs: Vec<novo_routing::model::Connection> = connections
        .iter()
        .enumerate()
        .map(|(i, c)| novo_routing::model::Connection {
            id: i as u64,
            from: c.from,
            to: c.to,
            diameter_mm: diameter_mm(c.cross_section_mm2),
        })
        .collect();
    let routed = route_connections(project, &jobs, &Rules::default());
    connections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let found = match routed.routes.get(&(i as u64)) {
                Some(route) => CutLength {
                    mm: route.length_mm,
                    basis: match route.kind {
                        Kind::Ducted => Basis::Routed,
                        Kind::FreeAir => Basis::FreeAir,
                    },
                },
                None => CutLength {
                    mm: estimate(project, c.from, c.to),
                    basis: Basis::Estimate,
                },
            };
            CutLength {
                mm: found.mm.ceil().max(1.0),
                ..found
            }
        })
        .collect()
}
