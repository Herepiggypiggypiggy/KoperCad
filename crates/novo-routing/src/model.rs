//! The bridge between the project model and the routing engine: read the
//! ducts on an enclosure's mounting plate. Engine duct ids are the raw
//! model ids of the rails.

use std::collections::{BTreeMap, HashMap};

use novo_core::{
    CabinetPlacement, DeviceId, Direction, PinRef, Project, Rail, RailKind, StructureId, SymbolRole,
};

use crate::{Anchor, Duct, Facing, Job, Network, Plan, Point, Route, Rules, plan};

/// The ducts on the plate of an enclosure, in model id order. DIN rails are
/// not ducts and are left out. A duct without a cross-section keeps
/// width and height 0, which the engine reports as unsized.
pub fn ducts(project: &Project, structure: StructureId) -> Vec<Duct> {
    project
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
        .collect()
}

/// The routing network of an enclosure's ducts.
pub fn network(project: &Project, structure: StructureId) -> Network {
    Network::new(&ducts(project, structure))
}

/// One connection to route: the wire list's from and to pins. `id` is the
/// caller's own number and comes back on the route; `diameter_mm` is the
/// wire's outer diameter when known.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub id: u64,
    pub from: PinRef,
    pub to: PinRef,
    pub diameter_mm: Option<f64>,
}

/// The connections of a project's wires: every wire with a pin at both ends
/// (a wire ending on a junction or open is not a connection yet), the id
/// being the raw wire id. The diameter is the conductor plus insulation of
/// the wire's cross-section (1.5 mm2 comes out at 3 mm), `None` when the
/// wire has no size. The Cabinet, the checks and the wire list route the
/// same connections by using this.
pub fn wire_connections(project: &Project) -> Vec<Connection> {
    project
        .wires()
        .filter_map(|w| {
            Some(Connection {
                id: w.id.raw(),
                from: w.from.as_pin()?,
                to: w.to.as_pin()?,
                diameter_mm: w
                    .cross_section_mm2
                    .filter(|s| *s > 0.0)
                    .map(|s| 2.0 * (s / std::f64::consts::PI).sqrt() + 1.6),
            })
        })
        .collect()
}

/// The device a pin physically sits on: the device of a device symbol, or
/// the strip of a terminal symbol.
fn device_of_pin(project: &Project, pin: PinRef) -> Option<DeviceId> {
    let symbol = project.symbol(pin.symbol)?;
    match &symbol.role {
        SymbolRole::Device { device, .. } => Some(*device),
        SymbolRole::Terminal { .. } => {
            let terminal = project.terminal(symbol.terminal_of_pin(pin.pin)?)?;
            Some(terminal.strip)
        }
        _ => None,
    }
}

fn facing(direction: Option<Direction>) -> Facing {
    match direction {
        Some(Direction::Up) => Facing::Up,
        Some(Direction::Down) => Facing::Down,
        Some(Direction::Left) => Facing::Left,
        Some(Direction::Right) => Facing::Right,
        None => Facing::Any,
    }
}

/// Where a pin is on the mounting plate, or `None` when its device is not
/// placed on one. The model does not know where a terminal sits inside its
/// device, so the anchor is the middle of the footprint edge the pin leaves
/// through, from the pin's direction in the symbol library, not its page
/// direction (the centre when the pin has no direction or the device is
/// turned): good to within the device size, which is what a cut length
/// needs.
fn anchor_of(
    project: &Project,
    placements: &HashMap<DeviceId, &CabinetPlacement>,
    pin: PinRef,
) -> Option<(StructureId, Anchor)> {
    let device = project.device(device_of_pin(project, pin)?)?;
    let placed = placements.get(&device.id)?;
    let size = novo_layout::model::device_size(project, device);
    let (w, h) = if placed.rotated {
        (size.height, size.width)
    } else {
        (size.width, size.height)
    };
    let facing = if placed.rotated {
        Facing::Any
    } else {
        // The library's own direction (before the symbol is turned on the
        // page): a contactor drawn sideways is still mounted upright.
        facing(
            project
                .symbol(pin.symbol)?
                .pins
                .get(usize::from(pin.pin))?
                .direction,
        )
    };
    let (cx, cy) = (placed.x + w / 2.0, placed.y + h / 2.0);
    let pos = match facing {
        Facing::Up => Point::new(cx, placed.y),
        Facing::Down => Point::new(cx, placed.y + h),
        Facing::Left => Point::new(placed.x, cy),
        Facing::Right => Point::new(placed.x + w, cy),
        Facing::Any => Point::new(cx, cy),
    };
    Some((placed.structure, Anchor { pos, facing }))
}

/// The routes of a set of connections.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Routed {
    /// The route per connection id.
    pub routes: BTreeMap<u64, Route>,
    /// The plan per enclosure, with its duct fill and problems. Problem job
    /// ids are connection ids.
    pub plans: Vec<(StructureId, Plan)>,
    /// Connections that could not be routed: a device is not placed on a
    /// plate, the two ends are on different plates, or the id is used by
    /// more than one connection (ids must be unique; every use is listed).
    pub unrouted: Vec<u64>,
}

/// Routes connections through the ducts of the enclosures their devices are
/// placed in, one `plan` per enclosure.
pub fn route_connections(project: &Project, connections: &[Connection], rules: &Rules) -> Routed {
    let placements: HashMap<DeviceId, &CabinetPlacement> = project
        .all::<CabinetPlacement>()
        .map(|p| (p.device, p))
        .collect();
    let mut jobs: BTreeMap<StructureId, Vec<Job>> = BTreeMap::new();
    let mut out = Routed::default();
    // A repeated id could not be told apart in the result: none of them is routed.
    let mut count: HashMap<u64, usize> = HashMap::new();
    for c in connections {
        *count.entry(c.id).or_default() += 1;
    }
    for c in connections {
        if count[&c.id] > 1 {
            out.unrouted.push(c.id);
            continue;
        }
        let ends =
            anchor_of(project, &placements, c.from).zip(anchor_of(project, &placements, c.to));
        match ends {
            Some(((s1, from), (s2, to))) if s1 == s2 => jobs.entry(s1).or_default().push(Job {
                id: c.id,
                from,
                to,
                diameter_mm: c.diameter_mm,
            }),
            _ => out.unrouted.push(c.id),
        }
    }
    for (structure, jobs) in jobs {
        let p = plan(&network(project, structure), &jobs, rules);
        for r in &p.routes {
            out.routes.insert(r.job, r.clone());
        }
        out.plans.push((structure, p));
    }
    out
}
