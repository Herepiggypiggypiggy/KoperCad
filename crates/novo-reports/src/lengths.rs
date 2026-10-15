//! Cut lengths for the wire list: routed through the cabinet's ducts where
//! both ends are placed on one mounting plate, estimated otherwise.

use std::collections::HashMap;

use novo_core::{
    CabinetPlacement, Device, DeviceId, PinRef, Project, Rail, RailKind, StructureId, SymbolRole,
    Terminal,
};
use novo_layout::model::device_size;
use novo_routing::{Anchor, Duct, Facing, Job, Kind, Network, Point, Rules, plan};

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

/// The device a pin belongs to: the device itself, or the strip of a
/// terminal.
fn device_of(project: &Project, pin: PinRef) -> Option<DeviceId> {
    match &project.symbol(pin.symbol)?.role {
        SymbolRole::Device { device, .. } => Some(*device),
        SymbolRole::Terminal { terminals } => terminals
            .first()
            .and_then(|&t| project.get::<Terminal>(t))
            .map(|t| t.strip),
        _ => None,
    }
}

/// The centre of the device's footprint on its plate. The model does not
/// store where on a device each terminal sits, so every wire of a device
/// starts at its middle, with no preferred direction.
fn anchor(project: &Project, placement: &CabinetPlacement) -> Option<Anchor> {
    let device = project.get::<Device>(placement.device)?;
    let size = device_size(project, device);
    let (w, h) = if placement.rotated {
        (size.height, size.width)
    } else {
        (size.width, size.height)
    };
    Some(Anchor {
        pos: Point::new(placement.x + w / 2.0, placement.y + h / 2.0),
        facing: Facing::Any,
    })
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
/// between devices placed on the same plate are routed together per plate,
/// so duct fill steers later wires; the rest get [`estimate`]. A length is
/// rounded up to a whole millimetre and is never zero.
pub(crate) fn cut_lengths(project: &Project, connections: &[Connection]) -> Vec<CutLength> {
    let placements: HashMap<DeviceId, &CabinetPlacement> = project
        .all::<CabinetPlacement>()
        .map(|p| (p.device, p))
        .collect();

    let mut jobs: HashMap<StructureId, Vec<Job>> = HashMap::new();
    for (i, c) in connections.iter().enumerate() {
        let pair = device_of(project, c.from)
            .zip(device_of(project, c.to))
            .and_then(|(a, b)| placements.get(&a).zip(placements.get(&b)));
        let Some((a, b)) = pair else { continue };
        if a.structure != b.structure {
            continue;
        }
        if let Some((from, to)) = anchor(project, a).zip(anchor(project, b)) {
            jobs.entry(a.structure).or_default().push(Job {
                id: i as u64,
                from,
                to,
                diameter_mm: diameter_mm(c.cross_section_mm2),
            });
        }
    }

    let mut routed: HashMap<usize, CutLength> = HashMap::new();
    for (structure, jobs) in jobs {
        let ducts: Vec<Duct> = project
            .all::<Rail>()
            .filter(|r| r.structure == structure && r.kind == RailKind::Duct)
            .map(|r| Duct {
                id: r.id.raw(),
                a: Point::new(r.x, r.y),
                b: if r.vertical {
                    Point::new(r.x, r.y + r.length)
                } else {
                    Point::new(r.x + r.length, r.y)
                },
                width: r.width,
                height: r.height,
            })
            .collect();
        let network = Network::new(&ducts);
        for route in plan(&network, &jobs, &Rules::default()).routes {
            let basis = match route.kind {
                Kind::Ducted => Basis::Routed,
                Kind::FreeAir => Basis::FreeAir,
            };
            routed.insert(
                route.job as usize,
                CutLength {
                    mm: route.length_mm,
                    basis,
                },
            );
        }
    }

    connections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let found = routed.remove(&i).unwrap_or_else(|| CutLength {
                mm: estimate(project, c.from, c.to),
                basis: Basis::Estimate,
            });
            CutLength {
                mm: found.mm.ceil().max(1.0),
                ..found
            }
        })
        .collect()
}
