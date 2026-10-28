//! The bridge between the project model and the routing engine: read the
//! ducts on an enclosure's mounting plate. Engine duct ids are the raw
//! model ids of the rails.

use std::collections::{BTreeMap, HashMap};

use novo_core::{
    CabinetPlacement, DeviceId, Direction, PinRef, Project, Rail, RailKind, StructureId, SymbolRole,
};

use crate::{
    Anchor, Clearance, Duct, Facing, Job, Kind, Network, Plan, Point, Point3, Route, Rules,
    WireRules, plan_constrained,
};

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

/// Outer diameter of a wire of the given cross-section: the copper circle
/// plus about 0.8 mm of insulation each side (1.5 mm2 comes out at 3 mm).
/// Only steers duct fill. `None` for no size or a size that is not a
/// positive number. The one rule for the wire list, the cut list, the
/// checks and the Cabinet, so they fill a duct the same way.
pub fn wire_diameter_mm(cross_section_mm2: Option<f64>) -> Option<f64> {
    cross_section_mm2
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| 2.0 * (s / std::f64::consts::PI).sqrt() + 1.6)
}

/// The cross-section of a wire: its own, else its cable core's.
pub fn wire_section_mm2(project: &Project, wire: &novo_core::Wire) -> Option<f64> {
    wire.cross_section_mm2.or_else(|| {
        let (cable, core) = project.core_of_wire(wire.id)?;
        project.cable(cable)?.cores.get(core)?.cross_section_mm2
    })
}

/// The connections of a project's wires: every wire with a pin at both ends
/// (a wire ending on a junction or open is not a connection yet), the id
/// being the raw wire id. The diameter comes from [`wire_diameter_mm`] of
/// the wire's cross-section (its cable core's when the wire has none),
/// `None` when it has no size. The Cabinet, the checks and the wire list
/// route the same connections by using this.
pub fn wire_connections(project: &Project) -> Vec<Connection> {
    project
        .wires()
        .filter_map(|w| {
            Some(Connection {
                id: w.id.raw(),
                from: w.from.as_pin()?,
                to: w.to.as_pin()?,
                diameter_mm: wire_diameter_mm(wire_section_mm2(project, w)),
            })
        })
        .collect()
}

/// Property on a wire: the ducts (ids, comma separated) it should stay out of
/// when another way exists. The router treats them as expensive, never as
/// forbidden, so a plate with one duct still routes the wire through it.
pub const AVOID_DUCTS: &str = "route.avoid_ducts";

/// The duct ids a wire's `route.avoid_ducts` names.
pub fn avoided_ducts(properties: &novo_core::Properties) -> Vec<u64> {
    match properties.get(AVOID_DUCTS) {
        Some(novo_core::Value::Text(t)) => {
            t.split(',').filter_map(|n| n.trim().parse().ok()).collect()
        }
        _ => Vec::new(),
    }
}

/// Property on a wire: other wires it must keep clear of, `id:mm` pairs comma
/// separated. A hard clearance between the ducts the two wires use (edge to
/// edge); when no way satisfies it the wire is routed without it and the plan
/// says so ([`novo_routing::Problem::ClearanceUnmet`]).
pub const KEEP_CLEAR: &str = "route.keep_clear";

/// The `(other wire id, mm)` pairs a wire's `route.keep_clear` names.
pub fn keep_clear(properties: &novo_core::Properties) -> Vec<(u64, f64)> {
    match properties.get(KEEP_CLEAR) {
        Some(novo_core::Value::Text(t)) => t
            .split(',')
            .filter_map(|e| {
                let (id, mm) = e.split_once(':')?;
                let mm: f64 = mm.trim().parse().ok()?;
                (mm.is_finite() && mm > 0.0).then_some((id.trim().parse().ok()?, mm))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Routes every wire of the project with the project's wire rules
/// ([`WireRules`]): the one call a screen needs. Routes and unrouted ids are
/// wire ids.
pub fn route_project(project: &Project) -> Routed {
    route_connections(
        project,
        &wire_connections(project),
        &WireRules::of(project).routing,
    )
}

/// Whether a pin is outside the enclosure whose plate has the other end: a
/// potential feed (the supply arrives from outside the drawing), or a device
/// whose location differs from the enclosure's.
fn is_outside(project: &Project, plate: StructureId, pin: PinRef) -> bool {
    let Some(symbol) = project.symbol(pin.symbol) else {
        return false;
    };
    if matches!(symbol.role, SymbolRole::PotentialFeed { .. }) {
        return true;
    }
    let (Some(device), Some(structure)) = (
        device_of_pin(project, pin).and_then(|d| project.device(d)),
        project.get::<novo_core::Structure>(plate),
    ) else {
        return false;
    };
    // A tag without a location says nothing about where the device is: it is
    // not "outside".
    !device.tag.location.is_empty()
        && !structure.tag.location.is_empty()
        && device.tag.location != structure.tag.location
}

/// The device a pin physically sits on: the device of a device symbol, or
/// the strip of a terminal symbol.
pub fn device_of_pin(project: &Project, pin: PinRef) -> Option<DeviceId> {
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

/// How far a device stands off the plate: the depth of its first part that
/// has one, which is where the terminals' screws and wires are. `None` when
/// no part says.
fn device_height(project: &Project, device: &novo_core::Device) -> Option<f64> {
    device.parts.iter().find_map(|part| {
        let part = project.get::<novo_core::Part>(*part)?;
        let depth = match part.properties.get("depth_mm")? {
            novo_core::Value::Float(v) => *v,
            novo_core::Value::Int(v) => *v as f64,
            _ => return None,
        };
        (depth.is_finite() && depth > 0.0).then_some(depth)
    })
}

/// Where the part says terminal `marking` sits, in the schema frame
/// (back-left-bottom corner, millimetres), from the first part of the device
/// that has measured or estimated positions.
fn terminal_in_part(
    project: &Project,
    device: &novo_core::Device,
    marking: &str,
) -> Option<[f64; 3]> {
    device.parts.iter().find_map(|part| {
        let part = project.get::<novo_core::Part>(*part)?;
        novo_parts::terminal_position(&part.properties, marking)
    })
}

/// The edge of its device a pin leaves through: the library direction, else
/// read from where the pin sits among the other pins of its symbol (a pin on
/// the upper half of a symbol that is taller than wide leaves through the top,
/// and so on). `Any` for a symbol with one pin or pins in a point.
fn pin_side(symbol: &novo_core::SymbolInstance, ix: usize) -> Facing {
    let Some(pin) = symbol.pins.get(ix) else {
        return Facing::Any;
    };
    if pin.direction.is_some() {
        return facing(pin.direction);
    }
    if symbol.pins.len() < 2 {
        return Facing::Any;
    }
    let span = |f: fn(&novo_core::Pin) -> f64| {
        symbol
            .pins
            .iter()
            .map(f)
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
    };
    let ((x0, x1), (y0, y1)) = (span(|p| p.offset.x), span(|p| p.offset.y));
    let (w, h) = (x1 - x0, y1 - y0);
    let (at, mid, low, high) = if h >= w {
        (pin.offset.y, (y0 + y1) / 2.0, Facing::Up, Facing::Down)
    } else {
        (pin.offset.x, (x0 + x1) / 2.0, Facing::Left, Facing::Right)
    };
    if w.max(h) < 1e-6 || (at - mid).abs() < 1e-6 {
        Facing::Any
    } else if at < mid {
        low
    } else {
        high
    }
}

/// Order of markings along an edge: letters first (`A1`, `A2`), then numbers
/// as numbers (`1`, `3`, `5`, `13`).
fn marking_order(a: &str, b: &str) -> std::cmp::Ordering {
    let numeric = |m: &str| !m.is_empty() && m.chars().all(|c| c.is_ascii_digit());
    numeric(a)
        .cmp(&numeric(b))
        .then_with(|| novo_core::natural_cmp(a, b))
}

/// Where a pin without a position from its part is estimated to be: the
/// edge it leaves through and its slot among the pins of that edge, from
/// the markings in their order (`index` of `count`). `None` for pins that
/// are not on a device symbol.
fn edge_slot(project: &Project, device: DeviceId, pin: PinRef) -> Option<(Facing, usize, usize)> {
    let symbol = project.symbol(pin.symbol)?;
    let side = pin_side(symbol, usize::from(pin.pin));
    if side == Facing::Any {
        return Some((side, 0, 1));
    }
    let mine = &symbol.pins.get(usize::from(pin.pin))?.name;
    let mut names: Vec<&str> = Vec::new();
    for s in project.symbols_of_device(device) {
        for (ix, p) in s.pins.iter().enumerate() {
            if pin_side(s, ix) == side && !names.contains(&p.name.as_str()) {
                names.push(&p.name);
            }
        }
    }
    names.sort_by(|a, b| marking_order(a, b));
    let index = names.iter().position(|n| *n == mine.as_str())?;
    Some((side, index, names.len().max(1)))
}

/// A terminal of a strip: its slot along the strip's long side.
fn strip_slot(project: &Project, strip: DeviceId, pin: PinRef) -> Option<(usize, usize)> {
    let terminal = project.symbol(pin.symbol)?.terminal_of_pin(pin.pin)?;
    let all = project.terminals_of_strip(strip);
    let index = all.iter().position(|t| t.id == terminal)?;
    Some((index, all.len().max(1)))
}

/// Turns an edge a quarter turn clockwise, seen from the door.
fn turned(side: Facing) -> Facing {
    match side {
        Facing::Up => Facing::Right,
        Facing::Right => Facing::Down,
        Facing::Down => Facing::Left,
        Facing::Left => Facing::Up,
        Facing::Any => Facing::Any,
    }
}

/// Whether the part of a placed pin's device gives no position for it, so
/// the anchor is an estimate (see [`anchor_of`]).
fn is_estimated(project: &Project, pin: PinRef) -> bool {
    let Some(device) = device_of_pin(project, pin).and_then(|d| project.device(d)) else {
        return false;
    };
    let Some(marking) = project
        .symbol(pin.symbol)
        .and_then(|s| s.pins.get(usize::from(pin.pin)))
        .map(|p| p.name.as_str())
    else {
        return false;
    };
    terminal_in_part(project, device, marking).is_none()
}

/// Where a pin is on the mounting plate, or `None` when its device is not
/// placed on one. When the device's part has the position of the terminal
/// (the Parts window's "Detect connection points", or the maker's data) the
/// anchor is the terminal itself: x and y on the plate, turned with the
/// device, and the height off the plate from the part.
///
/// Without it the position is an estimate (see [`Routed::estimated`]): the pin
/// is put on the edge of the footprint it leaves through (the library's
/// direction, else read from where it sits in its symbol) at its slot among
/// the pins of that edge in marking order, so the wires of one device do not
/// all start from one point. A terminal strip spreads its terminals along
/// its long side in strip order. A pin with no side is the middle of the
/// footprint. Good to within the device size, which a cut length needs.
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
    let strip = matches!(
        project.symbol(pin.symbol)?.role,
        SymbolRole::Terminal { .. }
    );
    let slot = if strip {
        None
    } else {
        edge_slot(project, device.id, pin)
    };
    let library_side = if strip {
        facing(
            project
                .symbol(pin.symbol)?
                .pins
                .get(usize::from(pin.pin))?
                .direction,
        )
    } else {
        slot.map_or(Facing::Any, |(side, ..)| side)
    };
    // The library's own direction (before the symbol is turned on the
    // page): a contactor drawn sideways is still mounted upright. A turned
    // device (R) turns its edges with it.
    let facing = if placed.rotated {
        turned(library_side)
    } else {
        library_side
    };
    let z_mm = device_height(project, device);
    let (cx, cy) = (placed.x + w / 2.0, placed.y + h / 2.0);
    let marking = &project
        .symbol(pin.symbol)?
        .pins
        .get(usize::from(pin.pin))?
        .name;
    if let Some([px, py, pz]) = terminal_in_part(project, device, marking) {
        // The schema frame has y up from the bottom; the plate has y down.
        // Turned (R) the part goes clockwise seen from the door.
        let pos = if placed.rotated {
            Point::new(cx + py - size.height / 2.0, cy + px - size.width / 2.0)
        } else {
            Point::new(placed.x + px, placed.y + size.height - py)
        };
        return Some((
            placed.structure,
            Anchor {
                pos,
                facing: if placed.rotated { Facing::Any } else { facing },
                z_mm: Some(pz),
            },
        ));
    }
    // The fraction along an edge of the pin's slot: even steps with half a
    // step to each end.
    let along = |(index, count): (usize, usize)| (index as f64 + 0.5) / count as f64;
    let pos = if strip {
        let (index, count) = strip_slot(project, device.id, pin).unwrap_or((0, 1));
        let t = along((index, count));
        if w >= h {
            Point::new(placed.x + w * t, cy)
        } else {
            Point::new(cx, placed.y + h * t)
        }
    } else {
        let t = slot.map_or(0.5, |(_, i, n)| along((i, n)));
        match facing {
            Facing::Up => Point::new(placed.x + w * t, placed.y),
            Facing::Down => Point::new(placed.x + w * t, placed.y + h),
            Facing::Left => Point::new(placed.x, placed.y + h * t),
            Facing::Right => Point::new(placed.x + w, placed.y + h * t),
            Facing::Any => Point::new(cx, cy),
        }
    };
    Some((placed.structure, Anchor { pos, facing, z_mm }))
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
    /// A connection from a device on the door (or another surface) to the
    /// plate with no opening between them is listed here too.
    pub unrouted: Vec<u64>,
    /// Routed connections with an end whose device depth is unknown: their
    /// length is flat (as if the terminals sat on the plate) and too short
    /// by the rise to the duct. The parts need a `depth_mm`.
    pub unknown_height: Vec<u64>,
    /// Routed connections with an end whose terminal position is an
    /// estimate: its part gives no position for the terminal, so the end is
    /// put on the device's edge by the pin's side and marking order (see
    /// `anchor_of`). Good to within the device size; the cut list should
    /// say so, and "Detect connection points" in the Parts window or the
    /// maker's data makes it exact.
    pub estimated: Vec<u64>,
    /// Connections with an end on a surface other than the plate (the door,
    /// a side wall), by connection id: how they get there.
    pub surface_legs: BTreeMap<u64, SurfaceLeg>,
    /// Connections routed from a placed device to the plate's cabinet entry
    /// point (value: the plate) because their other end is outside the
    /// enclosure (a supply feed, a device in another location). Their route
    /// ends at the entry: the part outside is not in the length. The route
    /// carries the service loop at the placed terminal and one at the entry.
    pub entry_legs: BTreeMap<u64, StructureId>,
    /// Connections whose two ends sit on the plates of two different
    /// enclosures that both have an entry point (value: the two plates, the
    /// `from` end's first). Each end is routed to its own plate's entry
    /// point and the legs are in `cross_routes`. These connections are also
    /// in `unrouted`: the run between the two entries is not known, so no
    /// whole length is claimed.
    pub cross_legs: BTreeMap<u64, [StructureId; 2]>,
    /// The leg of a cross-cabinet connection inside one plate, by
    /// connection id and plate. Ends at that plate's entry point.
    pub cross_routes: BTreeMap<(u64, StructureId), Route>,
}

/// The part of a wire that runs on a surface other than the mounting plate.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceLeg {
    /// The surface the device sits on.
    pub surface: String,
    /// The wiring opening the wire passes through; `None` when both ends
    /// are on that surface.
    pub opening: Option<String>,
    /// Wire length on the surface (from the opening to the terminal, or
    /// between the two terminals), including the service loop at the
    /// terminal. This is a model: the distance between plate and door and
    /// the hinge loop are not in it.
    pub length_mm: f64,
}

/// The surface of a device's placement and what the structure offers
/// between surfaces, read once per enclosure.
struct Surfaces {
    off_plate: HashMap<u64, String>,
    openings: Vec<novo_layout::openings::Opening>,
}

fn surfaces_of(project: &Project, structure: StructureId) -> Surfaces {
    match novo_layout::model::load(project, structure) {
        Some(layout) => Surfaces {
            off_plate: layout.off_plate.into_iter().collect(),
            openings: layout.openings,
        },
        None => Surfaces {
            off_plate: HashMap::new(),
            openings: Vec::new(),
        },
    }
}

/// Routes connections through the ducts of the enclosures their devices are
/// placed in, one `plan` per enclosure.
///
/// A device on the door or another surface is not on the plate, so its
/// coordinates are not plate coordinates. A wire between two devices on the
/// same such surface is a straight run (`Kind::FreeAir`, no duct there). A
/// wire between the plate and such a surface leaves the plate through the
/// wiring opening that adds the least wire
/// ([`novo_layout::openings::best_passage`], an approximate choice: it ranks
/// by straight-line distance, while the plate leg runs through the ducts): the plate leg is routed through
/// the ducts to the opening and the rest is added to its length (see
/// [`SurfaceLeg`]).
pub fn route_connections(project: &Project, connections: &[Connection], rules: &Rules) -> Routed {
    let placements: HashMap<DeviceId, &CabinetPlacement> = project
        .all::<CabinetPlacement>()
        .map(|p| (p.device, p))
        .collect();
    let mut jobs: BTreeMap<StructureId, Vec<Job>> = BTreeMap::new();
    let mut surfaces: HashMap<StructureId, Surfaces> = HashMap::new();
    let mut out = Routed::default();
    // A repeated id could not be told apart in the result: none of them is routed.
    let mut count: HashMap<u64, usize> = HashMap::new();
    for c in connections {
        *count.entry(c.id).or_default() += 1;
    }
    // Extra length added to a route after the plan: (connection id, mm).
    let mut extra: Vec<(u64, f64)> = Vec::new();
    for c in connections {
        if count[&c.id] > 1 {
            out.unrouted.push(c.id);
            continue;
        }
        let (a, b) = (
            anchor_of(project, &placements, c.from),
            anchor_of(project, &placements, c.to),
        );
        // One end on a plate with an entry point, the other outside the
        // enclosure: route to the entry point.
        let entry_leg = match (&a, &b) {
            (Some((plate, inside)), None) => Some((*plate, *inside, c.to, true)),
            (None, Some((plate, inside))) => Some((*plate, *inside, c.from, false)),
            _ => None,
        };
        if let Some((plate, inside, far, inside_is_from)) = entry_leg
            && is_outside(project, plate, far)
            && let Some(point) = crate::entry::entry_point(project, plate)
        {
            if inside.z_mm.is_none() {
                out.unknown_height.push(c.id);
            }
            let at_entry = Anchor {
                pos: point,
                facing: Facing::Any,
                z_mm: Some(0.0),
            };
            let (job_from, job_to) = if inside_is_from {
                (inside, at_entry)
            } else {
                (at_entry, inside)
            };
            jobs.entry(plate).or_default().push(Job {
                id: c.id,
                from: job_from,
                to: job_to,
                diameter_mm: c.diameter_mm,
            });
            out.entry_legs.insert(c.id, plate);
            continue;
        }
        let est = |anchor: &Option<(StructureId, Anchor)>, pin: PinRef| {
            anchor.is_some() && is_estimated(project, pin)
        };
        if est(&a, c.from) || est(&b, c.to) {
            out.estimated.push(c.id);
        }
        let ends = a.zip(b);
        let Some(((s1, from), (s2, to))) = ends else {
            out.unrouted.push(c.id);
            continue;
        };
        if s1 != s2 {
            out.unrouted.push(c.id);
            let (e1, e2) = (
                crate::entry::entry_point(project, s1),
                crate::entry::entry_point(project, s2),
            );
            let mut off_plate = |plate: StructureId, pin: PinRef| {
                surfaces
                    .entry(plate)
                    .or_insert_with(|| surfaces_of(project, plate))
                    .off_plate
                    .contains_key(&device_of_pin(project, pin).map_or(0, |d| d.raw()))
            };
            if let (Some(e1), Some(e2)) = (e1, e2)
                && !off_plate(s1, c.from)
                && !off_plate(s2, c.to)
            {
                let at = |pos: Point| Anchor {
                    pos,
                    facing: Facing::Any,
                    z_mm: Some(0.0),
                };
                if from.z_mm.is_none() || to.z_mm.is_none() {
                    out.unknown_height.push(c.id);
                }
                jobs.entry(s1).or_default().push(Job {
                    id: c.id,
                    from,
                    to: at(e1),
                    diameter_mm: c.diameter_mm,
                });
                jobs.entry(s2).or_default().push(Job {
                    id: c.id,
                    from: at(e2),
                    to,
                    diameter_mm: c.diameter_mm,
                });
                out.cross_legs.insert(c.id, [s1, s2]);
            }
            continue;
        }
        let known = surfaces
            .entry(s1)
            .or_insert_with(|| surfaces_of(project, s1));
        let surface_of = |pin: PinRef| {
            device_of_pin(project, pin)
                .and_then(|d| known.off_plate.get(&d.raw()))
                .cloned()
        };
        let (sa, sb) = (surface_of(c.from), surface_of(c.to));
        match (sa, sb) {
            (None, None) => {
                if from.z_mm.is_none() || to.z_mm.is_none() {
                    out.unknown_height.push(c.id);
                }
                jobs.entry(s1).or_default().push(Job {
                    id: c.id,
                    from,
                    to,
                    diameter_mm: c.diameter_mm,
                });
            }
            (Some(a), Some(b)) if a == b => {
                let length = from.pos.manhattan(&to.pos) + 2.0 * rules.slack_mm;
                out.routes.insert(
                    c.id,
                    Route {
                        job: c.id,
                        kind: Kind::FreeAir,
                        points: vec![from.pos, to.pos],
                        points3: vec![
                            Point3::new(from.pos.x, from.pos.y, 0.0),
                            Point3::new(to.pos.x, to.pos.y, 0.0),
                        ],
                        ducts: Vec::new(),
                        detour_mm: 0.0,
                        rise_mm: 0.0,
                        bends: 0,
                        length_mm: length.max(1.0),
                    },
                );
                out.surface_legs.insert(
                    c.id,
                    SurfaceLeg {
                        surface: a,
                        opening: None,
                        length_mm: length,
                    },
                );
            }
            (Some(_), Some(_)) => out.unrouted.push(c.id),
            (on_a, on_b) => {
                // One end on the plate, the other on a surface.
                let from_is_plate = on_a.is_none();
                let surface = on_a.or(on_b).unwrap_or_default();
                let (plate_end, off_end) = if from_is_plate {
                    (from, to)
                } else {
                    (to, from)
                };
                let Some(passage) = novo_layout::openings::best_passage(
                    &known.openings,
                    novo_layout::surfaces::MAIN_PLATE,
                    &surface,
                    (plate_end.pos.x, plate_end.pos.y),
                    (off_end.pos.x, off_end.pos.y),
                ) else {
                    out.unrouted.push(c.id);
                    continue;
                };
                let at_opening = Anchor {
                    pos: Point::new(passage.enter.0, passage.enter.1),
                    facing: Facing::Any,
                    z_mm: Some(0.0),
                };
                if plate_end.z_mm.is_none() {
                    out.unknown_height.push(c.id);
                }
                let (job_from, job_to) = if from_is_plate {
                    (plate_end, at_opening)
                } else {
                    (at_opening, plate_end)
                };
                jobs.entry(s1).or_default().push(Job {
                    id: c.id,
                    from: job_from,
                    to: job_to,
                    diameter_mm: c.diameter_mm,
                });
                let leg = Point::new(passage.leave.0, passage.leave.1).manhattan(&off_end.pos)
                    + rules.slack_mm;
                extra.push((c.id, leg));
                out.surface_legs.insert(
                    c.id,
                    SurfaceLeg {
                        surface,
                        opening: Some(passage.opening.name.clone()),
                        length_mm: leg,
                    },
                );
            }
        }
    }
    // Wires asked to stay out of some ducts (`route.avoid_ducts`).
    let avoid: BTreeMap<u64, Vec<u64>> = project
        .wires()
        .filter_map(|w| {
            let ducts = avoided_ducts(&w.properties);
            (!ducts.is_empty()).then_some((w.id.raw(), ducts))
        })
        .collect();
    // Hard clearances (`route.keep_clear`).
    let keep: BTreeMap<u64, Vec<Clearance>> = project
        .wires()
        .filter_map(|w| {
            let list: Vec<Clearance> = keep_clear(&w.properties)
                .into_iter()
                .filter(|(other, _)| *other != w.id.raw())
                .map(|(other, mm)| Clearance { other, mm })
                .collect();
            (!list.is_empty()).then_some((w.id.raw(), list))
        })
        .collect();
    for (structure, jobs) in jobs {
        let p = plan_constrained(&network(project, structure), &jobs, rules, &avoid, &keep);
        for r in &p.routes {
            if out.cross_legs.contains_key(&r.job) {
                out.cross_routes.insert((r.job, structure), r.clone());
            } else {
                out.routes.insert(r.job, r.clone());
            }
        }
        out.plans.push((structure, p));
    }
    // Only a wire that got a route can have an estimated end.
    let routed_ids: std::collections::HashSet<u64> = out
        .routes
        .keys()
        .copied()
        .chain(out.cross_routes.keys().map(|(id, _)| *id))
        .collect();
    out.estimated.retain(|id| routed_ids.contains(id));
    for (id, mm) in extra {
        match out.routes.get_mut(&id) {
            Some(route) => route.length_mm += mm,
            // The plate leg gave no route: the wire is not routed, and no
            // opening or length is claimed for it.
            None => {
                out.surface_legs.remove(&id);
                out.unrouted.push(id);
            }
        }
    }
    out
}
