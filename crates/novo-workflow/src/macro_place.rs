//! Placing a macro (docs/design/macros.md): copies a block definition loaded
//! from a macro file into the project when it is not there yet, places it
//! on a page and settles the wire numbers, as one undo step in the caller's
//! transaction. [`preview`] runs the same placement on a copy of the project
//! and reports what it would do, so the ghost can say it before anything
//! changes.
//!
//! Tags need no work here: `Tx::place_block` gives each device the next
//! free number for its letter code. Wire numbers in a macro are a
//! convenience of the source project, so they are cleared by default
//! ([`WireNumbers::Clear`]); the project has no wire numbering engine yet,
//! so cleared wires stay unnumbered until one numbers them. A macro can
//! keep its numbers ([`WireNumbers::Keep`]); a number that another wire
//! already carries is cleared and reported as a clash, never kept twice.

use std::collections::BTreeSet;

use novo_core::{
    AnyId, BlockDefinition, BlockDefinitionId, BlockInstanceId, DeviceId, DeviceTag, ModelError,
    PageId, PartId, Point, Project, Record, Result, Tx, Value, Wire, WireId,
};

/// What happens to the wire numbers stored in a macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireNumbers {
    /// Placed wires carry no number.
    #[default]
    Clear,
    /// Placed wires keep the macro's numbers, except where the project
    /// already uses the number.
    Keep,
}

/// A part a macro's device carried when it was saved. Part ids belong to
/// one project, so the macro keeps the part itself (manufacturer, order
/// number and the text fields) beside the id of the device in the
/// definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRef {
    /// The device's id in the definition.
    pub device: DeviceId,
    pub manufacturer: String,
    pub order_number: String,
    pub type_number: String,
    pub description: String,
}

/// The parts of the devices in a captured `definition`, read from the
/// `project` it was captured from (a definition's device ids are those of
/// the drawing, and its devices carry no parts).
pub fn captured_parts(project: &Project, definition: &BlockDefinition) -> Vec<PartRef> {
    let mut out = Vec::new();
    for member in &definition.members {
        let Record::Device(template) = member else {
            continue;
        };
        let Some(device) = project.device(template.id) else {
            continue;
        };
        for &part in &device.parts {
            if let Some(part) = project.part(part) {
                out.push(PartRef {
                    device: template.id,
                    manufacturer: part.manufacturer.clone(),
                    order_number: part.order_number.clone(),
                    type_number: part.type_number.clone(),
                    description: part.description.clone(),
                });
            }
        }
    }
    out
}

/// The JSON a macro file keeps under `parts`.
pub fn parts_to_json(parts: &[PartRef]) -> serde_json::Value {
    parts
        .iter()
        .map(|p| {
            serde_json::json!({
                "device": p.device.raw(),
                "manufacturer": p.manufacturer,
                "order_number": p.order_number,
                "type_number": p.type_number,
                "description": p.description,
            })
        })
        .collect()
}

/// Reads `parts` back. Entries that are not understood are skipped, so a
/// later version's extra fields never break a placement.
pub fn parts_from_json(value: &serde_json::Value) -> Vec<PartRef> {
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let device = entry.get("device")?.as_u64()?;
            let order_number = text(entry, "order_number");
            (!order_number.is_empty()).then(|| PartRef {
                device: DeviceId::from(device),
                manufacturer: text(entry, "manufacturer"),
                order_number,
                type_number: text(entry, "type_number"),
                description: text(entry, "description"),
            })
        })
        .collect()
}

/// How to place.
#[derive(Debug, Clone, Default)]
pub struct PlaceOptions {
    /// Parts to assign to the placed devices (empty: assign none).
    pub parts: Vec<PartRef>,
    /// Parameter values; unset ones take the definition's defaults.
    pub values: Vec<(String, Value)>,
    pub wire_numbers: WireNumbers,
}

/// What a placement did or would do.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Tags the placed devices took, in tag order.
    pub tags: Vec<DeviceTag>,
    /// Wire numbers the placed wires carry.
    pub kept_wire_numbers: Vec<String>,
    /// Parts assigned to the placed devices.
    pub parts_assigned: usize,
    /// Number of wires whose macro number was dropped.
    pub cleared_wires: usize,
    /// Macro wire numbers dropped because another wire already has them.
    pub clashes: Vec<String>,
    /// The project already had this definition, so it was not copied again.
    pub reused_definition: bool,
}

/// The result of a placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub definition: BlockDefinitionId,
    pub instance: BlockInstanceId,
    pub report: Report,
}

/// Places `definition` (as read from a macro file) on `page` with its
/// origin at `origin`. Use within `Document::transact` for one undo step.
///
/// A definition of the same name that is already in the project is reused
/// when its parameters and members are equal. A different one under the same
/// name is refused with `DuplicateBlock`: updating from the library is a
/// separate step.
pub fn place_macro(
    tx: &mut Tx<'_>,
    definition: BlockDefinition,
    page: PageId,
    origin: Point,
    options: &PlaceOptions,
) -> Result<Placed> {
    let existing = tx
        .project()
        .block_definition_by_name(&definition.name)
        .map(|d| {
            (
                d.id,
                d.parameters == definition.parameters && d.members == definition.members,
            )
        });
    let (id, reused_definition) = match existing {
        Some((id, true)) => (id, true),
        Some((_, false)) => return Err(ModelError::DuplicateBlock(definition.name)),
        None => (tx.adopt_block_definition(definition)?, false),
    };
    let instance = tx.place_block(id, page, origin, options.values.clone())?;
    let mut report = Report {
        reused_definition,
        ..Report::default()
    };
    settle_wire_numbers(tx, instance, options.wire_numbers, &mut report)?;
    report.parts_assigned = assign_parts(tx, instance, &options.parts)?;
    report.tags = placed_tags(tx.project(), instance);
    Ok(Placed {
        definition: id,
        instance,
        report,
    })
}

/// What [`place_macro`] would do, computed on a copy of the project; the
/// project is not changed. It clones the project, and the report does not
/// depend on `origin`: compute it once when a ghost starts and again only
/// when the options or parameter values change, not on every pointer move.
pub fn preview(
    project: &Project,
    definition: &BlockDefinition,
    page: PageId,
    origin: Point,
    options: &PlaceOptions,
) -> Result<Report> {
    let mut copy = project.clone();
    let (placed, _) = copy.transact("Preview macro", |tx| {
        place_macro(tx, definition.clone(), page, origin, options)
    })?;
    Ok(placed.report)
}

/// Assigns the macro's parts to the placed devices, adding a part to the
/// project when it has none with that manufacturer and order number.
fn assign_parts(tx: &mut Tx<'_>, instance: BlockInstanceId, parts: &[PartRef]) -> Result<usize> {
    let mut assigned = 0;
    for part in parts {
        let device = tx.project().block_instance(instance).and_then(|i| {
            i.members.iter().find_map(|m| match (m.template, m.actual) {
                (AnyId::Device(t), AnyId::Device(a)) if t == part.device => Some(a),
                _ => None,
            })
        });
        let Some(device) = device else { continue };
        let id = match find_part(tx.project(), part) {
            Some(id) => id,
            None => {
                let id = tx.add_part(part.manufacturer.clone(), part.order_number.clone())?;
                tx.update::<novo_core::Part>(id, |p| {
                    p.type_number = part.type_number.clone();
                    p.description = part.description.clone();
                })?;
                id
            }
        };
        tx.assign_part(device, id)?;
        assigned += 1;
    }
    Ok(assigned)
}

fn find_part(project: &Project, part: &PartRef) -> Option<PartId> {
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    project
        .parts()
        .find(|p| {
            same(&p.manufacturer, &part.manufacturer) && same(&p.order_number, &part.order_number)
        })
        .map(|p| p.id)
}

fn placed_wires(project: &Project, instance: BlockInstanceId) -> Vec<WireId> {
    project
        .block_instance(instance)
        .map(|i| {
            i.members
                .iter()
                .filter_map(|m| match m.actual {
                    AnyId::Wire(w) => Some(w),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn placed_tags(project: &Project, instance: BlockInstanceId) -> Vec<DeviceTag> {
    let mut tags: Vec<DeviceTag> = project
        .block_instance(instance)
        .map(|i| {
            i.members
                .iter()
                .filter_map(|m| match m.actual {
                    AnyId::Device(d) => project.device(d).map(|d| d.tag.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    tags.sort();
    tags
}

fn settle_wire_numbers(
    tx: &mut Tx<'_>,
    instance: BlockInstanceId,
    mode: WireNumbers,
    report: &mut Report,
) -> Result<()> {
    let wires = placed_wires(tx.project(), instance);
    let own: BTreeSet<WireId> = wires.iter().copied().collect();
    let used: BTreeSet<String> = tx
        .project()
        .wires()
        .filter(|w| !own.contains(&w.id))
        .filter_map(|w| w.number.clone())
        .collect();
    for wire in wires {
        let number = tx.project().wire(wire).and_then(|w| w.number.clone());
        let Some(number) = number else { continue };
        let keep = mode == WireNumbers::Keep && !used.contains(&number);
        if keep {
            report.kept_wire_numbers.push(number);
            continue;
        }
        if mode == WireNumbers::Keep {
            report.clashes.push(number);
        }
        report.cleared_wires += 1;
        tx.update::<Wire>(wire, |w| w.number = None)?;
    }
    report.kept_wire_numbers.sort();
    report.clashes.sort();
    Ok(())
}

/// The temporary name a capture is made under, so it can never clash with a
/// definition already in the project.
const CAPTURE_NAME: &str = "\u{1}capture";

/// Captures `items` (symbols, junctions and free texts; anything else in
/// the selection is skipped, wires between captured objects come with them)
/// as a block definition named `name`, relative to their top-left corner.
/// The project is not changed: the capture runs on a copy.
pub fn capture(
    project: &Project,
    name: &str,
    description: &str,
    items: &[AnyId],
) -> Result<BlockDefinition> {
    let items: Vec<AnyId> = items
        .iter()
        .copied()
        .filter(|i| matches!(i, AnyId::Symbol(_) | AnyId::Junction(_) | AnyId::Text(_)))
        .collect();
    let origin = selection_origin(project, &items);
    let mut copy = project.clone();
    let (mut definition, _) = copy.transact("Capture macro", |tx| {
        let id = tx.define_block(CAPTURE_NAME, &items, origin, Vec::new())?;
        tx.project()
            .block_definition(id)
            .cloned()
            .ok_or(ModelError::NotFound(id.into()))
    })?;
    definition.name = name.to_owned();
    definition.description = description.to_owned();
    Ok(definition)
}

/// The top-left corner of the drawn objects among `items`.
pub fn selection_origin(project: &Project, items: &[AnyId]) -> Point {
    let points = items.iter().filter_map(|&item| match item {
        AnyId::Symbol(s) => project.symbol(s).map(|s| s.placement.origin),
        AnyId::Junction(j) => project.junction(j).map(|j| j.position),
        _ => None,
    });
    corner(points)
}

/// Where a macro goes by default: right of what is on `page` (including the
/// reach of each symbol's pins), top aligned, one `gap` away. An empty page gets the origin.
pub fn free_origin(project: &Project, page: PageId, gap: f64) -> Point {
    let symbols = project.symbols().filter(|s| s.page == page);
    let mut right = f64::NEG_INFINITY;
    let mut points = Vec::new();
    for s in symbols {
        // Pins reach past the origin of a wide symbol.
        for pin in &s.pins {
            right = right.max(s.placement.apply(pin.offset).x);
        }
        right = right.max(s.placement.origin.x);
        points.push(s.placement.origin);
    }
    for j in project.junctions().filter(|j| j.page == page) {
        right = right.max(j.position.x);
        points.push(j.position);
    }
    if points.is_empty() {
        return Point::new(0.0, 0.0);
    }
    Point::new(right + gap, corner(points.into_iter()).y)
}

fn corner(points: impl Iterator<Item = Point>) -> Point {
    let (mut x, mut y) = (f64::INFINITY, f64::INFINITY);
    for p in points {
        x = x.min(p.x);
        y = y.min(p.y);
    }
    if x.is_finite() && y.is_finite() {
        Point::new(x, y)
    } else {
        Point::new(0.0, 0.0)
    }
}
