//! The bridge between the project model and the layout engine: read a
//! panel's plate, rails and placements into a [`CabinetLayout`], list the
//! devices that belong on it, and apply a `place all` plan as commands.
//!
//! Engine ids are the raw model ids, which are unique across all kinds.

use std::collections::HashMap;

use novo_core::{
    CabinetPlacement, Command, Device, DeviceId, MountingPlate, Project, Rail as ModelRail, RailId,
    RailKind, Result, Structure, StructureId, Tx, Value,
};

use crate::plan::{Candidate, Plan};
use crate::state::CabinetLayout;
use crate::{Margins, Placement, Plate, Rail, Size};

/// Part property keys, the same as `novo_parts::keys`.
const WIDTH_KEY: &str = "width_mm";
const HEIGHT_KEY: &str = "height_mm";
const ABOVE_KEY: &str = "clearance_above_mm";
const BELOW_KEY: &str = "clearance_below_mm";
const LEFT_KEY: &str = "clearance_left_mm";
const RIGHT_KEY: &str = "clearance_right_mm";

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

/// The footprint of a device from the first of its parts that has a width
/// and height; guessed when none has.
pub fn device_size(project: &Project, device: &Device) -> Size {
    for part in &device.parts {
        if let Some(part) = project.get::<novo_core::Part>(*part) {
            let w = part.properties.get(WIDTH_KEY).and_then(number);
            let h = part.properties.get(HEIGHT_KEY).and_then(number);
            if w.is_some() && h.is_some() {
                return Size::from_part(w, h);
            }
        }
    }
    Size::from_part(None, None)
}

/// The free space a device asks for on each side, from its own properties
/// first, then from its first part that gives one (so a device can override
/// its part). Negative or non-numeric values count as missing.
pub fn device_margins(project: &Project, device: &Device) -> Margins {
    let read = |key: &str| {
        let usable = |v: &Value| number(v).filter(|v| v.is_finite() && *v >= 0.0);
        let own = device.properties.get(key).and_then(usable);
        let from_part = || {
            device.parts.iter().find_map(|part| {
                project
                    .get::<novo_core::Part>(*part)?
                    .properties
                    .get(key)
                    .and_then(usable)
            })
        };
        own.or_else(from_part)
    };
    Margins {
        above: read(ABOVE_KEY),
        below: read(BELOW_KEY),
        left: read(LEFT_KEY),
        right: read(RIGHT_KEY),
    }
}

fn structure(project: &Project, id: StructureId) -> Option<&Structure> {
    project.get::<Structure>(id)
}

/// The layout stored for an enclosure; `None` when it has no plate yet.
pub fn load(project: &Project, id: StructureId) -> Option<CabinetLayout> {
    let s = structure(project, id)?;
    let plate = s.plate?;
    let mut layout = CabinetLayout::new(Plate {
        width: plate.width,
        height: plate.height,
        margin: plate.margin,
    });
    layout.rails = project
        .all::<ModelRail>()
        .filter(|r| r.structure == id && r.kind == RailKind::DinRail)
        .map(|r| Rail {
            id: r.id.raw(),
            x: r.x,
            y: r.y,
            length: r.length,
        })
        .collect();
    layout.placements = project
        .all::<CabinetPlacement>()
        .filter(|p| p.structure == id)
        .filter_map(|p| {
            let device = project.get::<Device>(p.device)?;
            Some(Placement {
                id: p.device.raw(),
                x: p.x,
                y: p.y,
                rotated: p.rotated,
                size: device_size(project, device),
                clearance: None,
                margins: device_margins(project, device),
                rail: p.rail.map(RailId::raw),
            })
        })
        .collect();
    Some(layout)
}

/// The devices that belong in an enclosure: those whose tag has the same
/// location as the enclosure, whatever their function (page group). Sorted
/// by letter code, then device number.
pub fn candidates(project: &Project, id: StructureId) -> Vec<Candidate> {
    let Some(s) = structure(project, id) else {
        return Vec::new();
    };
    project
        .all::<Device>()
        .filter(|d| !s.tag.location.is_empty() && d.tag.location == s.tag.location)
        .map(|d| Candidate {
            id: d.id.raw(),
            size: device_size(project, d),
            sort_key: format!("{}{:06}", d.tag.letter_code(), d.tag.number().unwrap_or(0)),
        })
        .collect()
}

/// Sets the plate of an enclosure.
pub fn set_plate(tx: &mut Tx<'_>, id: StructureId, plate: MountingPlate) -> Result<()> {
    tx.execute(Command::SetPlateSize {
        structure: id,
        plate: Some(plate),
    })
    .map(|_| ())
}

/// Applies a plan inside one edit, so `place all` is one undo step: adds the
/// plan's rails, then places each device on its rail. Devices that are
/// already placed are left where they are.
pub fn apply_plan(tx: &mut Tx<'_>, id: StructureId, plan: &Plan) -> Result<()> {
    let mut rails: HashMap<u64, (RailId, f64)> = HashMap::new();
    for rail in &plan.rails {
        let created = tx.execute(Command::AddRail {
            structure: id,
            x: rail.x,
            y: rail.y,
            length: rail.length,
            kind: RailKind::DinRail,
            vertical: false,
            width: 0.0,
            height: 0.0,
        })?;
        if let Some(novo_core::AnyId::Rail(model_id)) = created.first() {
            rails.insert(rail.id, (*model_id, rail.x));
        }
    }
    for p in &plan.placements {
        let device = DeviceId::from(p.id);
        if tx
            .project()
            .all::<CabinetPlacement>()
            .any(|q| q.device == device)
        {
            continue;
        }
        let rail = rails.get(&p.rail).map(|(rail, x)| (*rail, p.x - x));
        tx.execute(Command::PlaceDevice {
            device,
            structure: id,
            x: p.x,
            y: p.y,
            rotated: false,
            rail,
        })?;
    }
    Ok(())
}

/// Places a device on the plate at `(x, y)`, or moves it there; it leaves
/// any rail it was on.
pub fn place_device(
    tx: &mut Tx<'_>,
    id: StructureId,
    device: u64,
    x: f64,
    y: f64,
    rotated: bool,
) -> Result<()> {
    tx.execute(Command::PlaceDevice {
        device: DeviceId::from(device),
        structure: id,
        x,
        y,
        rotated,
        rail: None,
    })
    .map(|_| ())
}

fn placement_of(tx: &Tx<'_>, device: u64) -> Option<(novo_core::CabinetPlacementId, bool)> {
    let device = DeviceId::from(device);
    tx.project()
        .all::<CabinetPlacement>()
        .find(|p| p.device == device)
        .map(|p| (p.id, p.rotated))
}

/// Turns a placed device by 90 degrees. A device that is not placed is left alone.
pub fn rotate_device(tx: &mut Tx<'_>, device: u64) -> Result<()> {
    if let Some((placement, rotated)) = placement_of(tx, device) {
        tx.execute(Command::RotatePlacement {
            placement,
            rotated: !rotated,
        })?;
    }
    Ok(())
}

/// Takes a device off the plate. A device that is not placed is left alone.
pub fn unplace_device(tx: &mut Tx<'_>, device: u64) -> Result<()> {
    if let Some((placement, _)) = placement_of(tx, device) {
        tx.execute(Command::RemovePlacement { placement })?;
    }
    Ok(())
}
