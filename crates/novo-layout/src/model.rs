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

use crate::drill::{
    CUTOUT_KEY, DeviceFeatures, DrillTable, HOLE_PATTERN_KEY, MOUNTING_KEY, features_from_text,
};
use crate::enclosure::{DEFAULT_DOOR_ANGLE_DEG, Door, Enclosure, Hinge};
use crate::plan::{Candidate, Plan, PlannedDuct};
use crate::plate_drawing::{PlateDocument, documents};
use crate::rules::Class;
use crate::state::CabinetLayout;
use crate::{Margins, Placement, Plate, Rail, Size};

/// Property of a duct: what its wiring carries (`power`, `signal`, `mixed`).
pub const DUCT_CLASS_KEY: &str = "duct_class";

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
            vertical: r.vertical,
        })
        .collect();
    layout.enclosure = enclosure_of(project, id);
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
    layout.depths = layout
        .placements
        .iter()
        .filter_map(|p| {
            let device = project.get::<Device>(DeviceId::from(p.id))?;
            Some((p.id, device_depth(project, device)?))
        })
        .collect();
    Some(layout)
}

/// The wiring ducts of an enclosure's plate with the wiring they serve (the
/// `duct_class` property; `mixed` when it is not set).
pub fn ducts(project: &Project, id: StructureId) -> Vec<PlannedDuct> {
    project
        .all::<ModelRail>()
        .filter(|r| r.structure == id && r.kind == RailKind::Duct)
        .map(|r| PlannedDuct {
            x: r.x,
            y: r.y,
            length: r.length,
            vertical: r.vertical,
            width: r.width,
            height: r.height,
            class: match r.properties.get(DUCT_CLASS_KEY) {
                Some(Value::Text(t)) => Class::parse(t).unwrap_or_default(),
                _ => Class::Mixed,
            },
        })
        .collect()
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
    let mut rails: HashMap<u64, (RailId, bool, f64, f64)> = HashMap::new();
    for rail in &plan.rails {
        let created = tx.execute(Command::AddRail {
            structure: id,
            x: rail.x,
            y: rail.y,
            length: rail.length,
            kind: RailKind::DinRail,
            vertical: rail.vertical,
            width: 0.0,
            height: 0.0,
        })?;
        if let Some(novo_core::AnyId::Rail(model_id)) = created.first() {
            rails.insert(rail.id, (*model_id, rail.vertical, rail.x, rail.y));
        }
    }
    for duct in &plan.ducts {
        let created = tx.execute(Command::AddRail {
            structure: id,
            x: duct.x,
            y: duct.y,
            length: duct.length,
            kind: RailKind::Duct,
            vertical: duct.vertical,
            width: duct.width,
            height: duct.height,
        })?;
        if let Some(novo_core::AnyId::Rail(rail)) = created.first() {
            tx.set_property(
                *rail,
                DUCT_CLASS_KEY,
                Some(Value::Text(duct.class.as_str().to_owned())),
            )?;
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
        let rail = rails
            .get(&p.rail)
            .map(|(rail, vertical, x, y)| (*rail, if *vertical { p.y - y } else { p.x - x }));
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

fn text_property(project: &Project, device: &Device, key: &str) -> Option<String> {
    device.parts.iter().find_map(|part| {
        match project.get::<novo_core::Part>(*part)?.properties.get(key)? {
            Value::Text(t) if !t.trim().is_empty() => Some(t.clone()),
            _ => None,
        }
    })
}

/// What a placed device needs machined, read from the first of its parts
/// that has each of the mounting, hole pattern and cut-out keys.
pub fn features(project: &Project, device: &Device) -> DeviceFeatures {
    features_from_text(
        device.id.raw(),
        device.tag.to_string(),
        text_property(project, device, MOUNTING_KEY).as_deref(),
        text_property(project, device, HOLE_PATTERN_KEY).as_deref(),
        text_property(project, device, CUTOUT_KEY).as_deref(),
    )
}

/// The plate drawing documents and drill table of an enclosure; `None`
/// when it has no plate.
pub fn plate_documents(
    project: &Project,
    id: StructureId,
) -> Option<(Vec<PlateDocument>, DrillTable)> {
    let layout = load(project, id)?;
    let features: Vec<DeviceFeatures> = layout
        .placements
        .iter()
        .filter_map(|p| project.get::<Device>(DeviceId::from(p.id)))
        .map(|d| features(project, d))
        .collect();
    let title = structure(project, id)?.tag.to_string();
    Some(documents(&title, &layout, &features))
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

/// Structure property keys of the enclosure interior and door. They are
/// free-form properties, so no model change is needed.
pub mod keys {
    pub const INTERIOR_WIDTH: &str = "interior_width_mm";
    pub const INTERIOR_HEIGHT: &str = "interior_height_mm";
    pub const INTERIOR_DEPTH: &str = "interior_depth_mm";
    pub const PLATE_SETBACK: &str = "plate_setback_mm";
    pub const DOOR_HINGE: &str = "door_hinge";
    pub const DOOR_ANGLE: &str = "door_angle_deg";
    pub const DOOR_INNER: &str = "door_inner_mm";
    pub const FRONT_CLEARANCE: &str = "front_clearance_mm";
    pub const HINGE_SIDE_CLEARANCE: &str = "hinge_side_clearance_mm";
}

/// Part property key of a device's depth, the same as `novo_parts::keys`.
const DEPTH_KEY: &str = "depth_mm";

/// The depth of a device from the first of its parts that has one.
pub fn device_depth(project: &Project, device: &Device) -> Option<f64> {
    device
        .parts
        .iter()
        .filter_map(|part| project.get::<novo_core::Part>(*part))
        .filter_map(|part| part.properties.get(DEPTH_KEY).and_then(number))
        .find(|d| *d > 0.0)
}

/// The interior and door stored on an enclosure. `None` until width, height
/// and depth of the interior are all set. A door exists once a hinge side is
/// set.
pub fn enclosure_of(project: &Project, id: StructureId) -> Option<Enclosure> {
    let s = structure(project, id)?;
    let get = |key: &str| s.properties.get(key).and_then(number);
    let positive = |key: &str| get(key).filter(|v| *v > 0.0);
    let door = match s.properties.get(keys::DOOR_HINGE) {
        Some(Value::Text(text)) => Hinge::parse(text).map(|hinge| Door {
            hinge,
            max_angle: get(keys::DOOR_ANGLE).unwrap_or(DEFAULT_DOOR_ANGLE_DEG),
            inner_depth: get(keys::DOOR_INNER).unwrap_or(0.0).max(0.0),
        }),
        _ => None,
    };
    Some(Enclosure {
        width: positive(keys::INTERIOR_WIDTH)?,
        height: positive(keys::INTERIOR_HEIGHT)?,
        depth: positive(keys::INTERIOR_DEPTH)?,
        plate_setback: get(keys::PLATE_SETBACK).unwrap_or(0.0).max(0.0),
        door,
        front_clearance: get(keys::FRONT_CLEARANCE).filter(|v| *v >= 0.0),
        hinge_side_clearance: get(keys::HINGE_SIDE_CLEARANCE).filter(|v| *v >= 0.0),
    })
}

/// Stores the interior and door on an enclosure; a value that is `None`
/// removes its property.
pub fn set_enclosure(tx: &mut Tx<'_>, id: StructureId, e: &Enclosure) -> Result<()> {
    let float = |v: f64| Some(Value::Float(v));
    let door = e.door;
    let entries: [(&str, Option<Value>); 9] = [
        (keys::INTERIOR_WIDTH, float(e.width)),
        (keys::INTERIOR_HEIGHT, float(e.height)),
        (keys::INTERIOR_DEPTH, float(e.depth)),
        (keys::PLATE_SETBACK, float(e.plate_setback)),
        (
            keys::DOOR_HINGE,
            door.map(|d| Value::Text(d.hinge.as_str().to_owned())),
        ),
        (keys::DOOR_ANGLE, door.and_then(|d| float(d.max_angle))),
        (keys::DOOR_INNER, door.and_then(|d| float(d.inner_depth))),
        (keys::FRONT_CLEARANCE, e.front_clearance.and_then(float)),
        (
            keys::HINGE_SIDE_CLEARANCE,
            e.hinge_side_clearance.and_then(float),
        ),
    ];
    for (key, value) in entries {
        tx.set_property(id, key, value)?;
    }
    Ok(())
}
