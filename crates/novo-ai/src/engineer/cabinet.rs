//! The cabinet and the part picker, for Claude.
//!
//! Reads: the `cabinet` section of `read_project` (every enclosure with its plate, rails, ducts,
//! placed devices, devices not placed yet and the layout problems in words) and the
//! `search_catalog` tool (the part picker's search). Edits: set the plate, add a rail, place,
//! move, turn or take off a device, place all that are left, and assign a catalog part. All of
//! them go through the same model functions the Cabinet and Parts windows use, in the draft, so
//! the person sees them in the change set and applies them as one undo step.

use super::resolve::{self, text_of};
use novo_core::{DeviceId, DeviceTag, MountingPlate, Project, RailKind, StructureId, Tx};
use novo_layout::model;
use novo_parts::{AssignMode, Catalog};
use serde_json::{Value, json};

fn tag_of(p: &Project, raw: u64) -> String {
    p.device(DeviceId::from(raw))
        .map_or_else(|| format!("#{raw}"), |d| d.tag.to_string())
}

fn mm(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// The enclosures that have a mounting plate or could get one.
fn enclosures(p: &Project) -> Vec<&novo_core::Structure> {
    let mut list: Vec<_> = p.structures().filter(|s| s.enclosure).collect();
    list.sort_by_key(|s| s.tag.to_string());
    list
}

/// The `cabinet` section: one entry per enclosure.
pub fn section(p: &Project) -> Value {
    Value::Array(enclosures(p).into_iter().map(|s| one(p, s)).collect())
}

fn one(p: &Project, s: &novo_core::Structure) -> Value {
    let name = s.tag.to_string();
    let Some(layout) = model::load(p, s.id) else {
        return json!({
            "enclosure": name, "description": s.description, "plate": null,
            "note": "no mounting plate yet: set_cabinet_plate gives it one",
            "devices_to_place": model::belonging(p, s.id).iter().map(|d| d.tag.to_string()).collect::<Vec<_>>(),
        });
    };
    let on_plate = layout.on_plate();
    let placed: Vec<Value> = layout
        .placements
        .iter()
        .map(|pl| {
            let f = pl.footprint();
            json!({
                "device": tag_of(p, pl.id),
                "x_mm": mm(pl.x), "y_mm": mm(pl.y),
                "width_mm": mm(f.w), "height_mm": mm(f.h),
                "rotated": pl.rotated,
                "rail_id": pl.rail,
                "size_is_a_guess": pl.size.guessed,
                "depth_mm": layout.depths.get(&pl.id),
                "surface": layout.off_plate.get(&pl.id).cloned().unwrap_or_else(|| "plate".into()),
            })
        })
        .collect();
    let off = model::off_plate_devices(p, s.id);
    let unplaced: Vec<String> = model::belonging(p, s.id)
        .into_iter()
        .filter(|d| {
            !layout.placements.iter().any(|x| x.id == d.id.raw()) && !off.contains(&d.id.raw())
        })
        .map(|d| d.tag.to_string())
        .collect();
    let report = layout.report();
    let t = |id: &u64| tag_of(p, *id);
    let mut problems: Vec<String> = Vec::new();
    for (a, b) in &report.collisions {
        problems.push(format!("{} overlaps {}", t(a), t(b)));
    }
    for id in &report.outside {
        problems.push(format!("{} is outside the usable plate", t(id)));
    }
    for (a, b, gap) in &report.too_close {
        problems.push(format!(
            "{} and {} are {:.1} mm apart, less than they need",
            t(a),
            t(b),
            gap
        ));
    }
    for (rail, over) in &report.rail_overflows {
        problems.push(format!(
            "rail {rail} is {over:.0} mm too short for its devices"
        ));
    }
    for c in &report.opening_conflicts {
        problems.push(format!("a device covers a wiring opening ({c:?})"));
    }
    for d in &report.duct_overlaps {
        problems.push(format!("a duct lies over a rail or device ({d:?})"));
    }
    for (dev, rail, off) in &report.off_centre {
        problems.push(format!(
            "{} is {:.1} mm off the line of rail {rail}",
            t(dev),
            off
        ));
    }
    if !report.guessed.is_empty() {
        problems.push(format!(
            "size is a default guess (no part dimensions) for {}",
            report.guessed.iter().map(t).collect::<Vec<_>>().join(", ")
        ));
    }
    let door = layout.door_report().map(|d| d.problems());
    let _ = on_plate;
    let interior = layout.enclosure.map(|e| {
        json!({
            "width_mm": e.width, "height_mm": e.height, "depth_mm": e.depth,
            "plate_setback_mm": e.plate_setback,
            "usable_depth_mm": mm(e.usable_depth()),
            "door": e.door.map(|d| json!({
                "hinge": d.hinge.as_str(), "max_angle_deg": d.max_angle,
                "inner_depth_mm": d.inner_depth})),
            "front_clearance_mm": e.front_clearance,
            "hinge_side_clearance_mm": e.hinge_side_clearance,
        })
    });
    let door_check = layout.door_report().map(|d| {
        json!({
            "too_deep_so_the_door_cannot_close": d.too_deep.iter().map(|(id, by)| format!("{} by {:.0} mm", tag_of(p, *id), by)).collect::<Vec<_>>(),
            "tight": d.tight.iter().map(|(id, gap)| format!("{} leaves {:.0} mm", tag_of(p, *id), gap)).collect::<Vec<_>>(),
            "depth_unknown": d.unknown_depth.iter().map(|id| tag_of(p, *id)).collect::<Vec<_>>(),
            "plate_bigger_than_interior": d.plate_too_big,
            "front_room_short_mm": d.front_short, "hinge_side_room_short_mm": d.hinge_side_short,
        })
    });
    let swing = layout.swing_report().map(|s| {
        json!({
            "door_devices_swept": s.swept,
            "hits": s.hits.len(),
            "not_checked_no_depth": s.unchecked_plate.iter().chain(s.unknown_depth.iter()).map(|id| tag_of(p, *id)).collect::<Vec<_>>(),
        })
    });
    json!({
        "enclosure": name,
        "description": s.description,
        "plate": {"width_mm": layout.plate.width, "height_mm": layout.plate.height,
                  "margin_mm": layout.plate.margin},
        "rails": layout.rails.iter().map(|r| json!({
            "id": r.id, "x_mm": mm(r.x), "y_mm": mm(r.y), "length_mm": mm(r.length),
            "vertical": r.vertical})).collect::<Vec<_>>(),
        "ducts": layout.ducts.iter().map(|d| json!({
            "id": d.id, "x_mm": mm(d.x), "y_mm": mm(d.y), "length_mm": mm(d.length),
            "width_mm": mm(d.width), "vertical": d.vertical})).collect::<Vec<_>>(),
        "placed": placed,
        "not_placed_yet": unplaced,
        "layout_problems": problems,
        "door_problems": door,
        "interior": interior,
        "door_check": door_check,
        "door_swing": swing,
        "what_3d_shows": "the 3D view draws exactly this data (plate, rails, ducts, devices with depth, door); change it with the cabinet actions, camera and display settings are not mine",
        "axes": "millimetres from the plate's top left, x right, y down; x/y are a device's top left",
    })
}

/// A catalog part as the picker shows it, for the model.
fn part_json(c: &novo_parts::CatalogPart) -> Value {
    json!({
        "manufacturer": c.manufacturer,
        "order_number": c.order_number,
        "type_number": c.type_number,
        "description": c.description,
        "fits_letter_codes": c.letter_codes,
        "size_mm": c.dimensions.map(|d| json!([d.width_mm, d.height_mm, d.depth_mm])),
    })
}

/// The part picker's search: free text, optionally narrowed to what suits a device.
pub fn search_catalog(
    catalog: &Catalog,
    project: &Project,
    input: &Value,
) -> Result<Value, String> {
    let query = input["query"].as_str().unwrap_or_default();
    let limit = input["limit"]
        .as_u64()
        .map_or(15, |l| l as usize)
        .clamp(1, 40);
    let letter = match text_of(&input["device"]) {
        Some(t) => {
            let id = resolve::device(project, &t)?;
            project
                .device(id)
                .map(|d| d.tag.letter_code().to_string())
                .unwrap_or_default()
        }
        None => String::new(),
    };
    if query.trim().is_empty() && letter.is_empty() {
        return Err(
            "give a query (words, order number) and/or a device to find parts that suit".into(),
        );
    }
    let hits = catalog.suggest(&letter, query, limit);
    Ok(json!({
        "parts": hits.into_iter().map(part_json).collect::<Vec<_>>(),
        "catalog_size": catalog.len(),
        "tip": "assign one with propose_edit assign_catalog_part; never invent an order number",
    }))
}

// ----------------------------------------------------------------- edits

pub(super) fn enclosure_arg(p: &Project, args: &Value) -> Result<StructureId, String> {
    let list = enclosures(p);
    let names = || {
        list.iter()
            .map(|s| s.tag.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let Some(t) = text_of(&args["enclosure"]) {
        let want = DeviceTag::parse(&t).ok();
        return list
            .iter()
            .find(|s| Some(&s.tag) == want.as_ref() || s.tag.to_string().eq_ignore_ascii_case(&t))
            .map(|s| s.id)
            .ok_or_else(|| format!("there is no enclosure {t}. Enclosures: {}", names()));
    }
    match list.as_slice() {
        [one] => Ok(one.id),
        [] => Err(
            "the project has no enclosure (cabinet) yet; set one up in the Cabinet workspace"
                .into(),
        ),
        _ => Err(format!("give the enclosure; the project has {}", names())),
    }
}

fn plate_number(args: &Value, key: &str, min: f64) -> Result<f64, String> {
    args[key]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= min && *v <= 10_000.0)
        .ok_or_else(|| format!("{key} must be a number of at least {min} mm"))
}

pub fn add_cabinet(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let text = need_text(args, "tag")?;
    let tag = DeviceTag::parse(&text).map_err(|e| format!("tag {text:?}: {e}"))?;
    if tag.location.is_empty() || !tag.function.is_empty() || !tag.product.is_empty() {
        return Err("an enclosure tag is a location only, such as +CAB1".into());
    }
    if tx.project().structure_by_tag(&tag).is_some() {
        return Err(format!("{tag} already exists"));
    }
    let description = text_of(&args["description"]).unwrap_or_default();
    let id = tx
        .add_structure(tag.clone(), description, true)
        .map_err(|e| e.to_string())?;
    if args["width"].is_null() && args["height"].is_null() {
        return Ok(format!("Add enclosure {tag}"));
    }
    let width = plate_number(args, "width", 50.0)?;
    let height = plate_number(args, "height", 50.0)?;
    let margin = if args["margin"].is_null() {
        10.0
    } else {
        plate_number(args, "margin", 0.0)?
    };
    if margin * 2.0 >= width.min(height) {
        return Err("the margin leaves no usable plate".into());
    }
    model::set_plate(
        tx,
        id,
        MountingPlate {
            width,
            height,
            margin,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "Add enclosure {tag} with a {width} x {height} mm mounting plate"
    ))
}

pub fn set_cabinet_plate(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let id = enclosure_arg(tx.project(), args)?;
    let width = plate_number(args, "width", 50.0)?;
    let height = plate_number(args, "height", 50.0)?;
    let margin = if args["margin"].is_null() {
        10.0
    } else {
        plate_number(args, "margin", 0.0)?
    };
    if margin * 2.0 >= width.min(height) {
        return Err("the margin leaves no usable plate".into());
    }
    model::set_plate(
        tx,
        id,
        MountingPlate {
            width,
            height,
            margin,
        },
    )
    .map_err(|e| e.to_string())?;
    let tag = tx
        .project()
        .get::<novo_core::Structure>(id)
        .map(|s| s.tag.to_string())
        .unwrap_or_default();
    Ok(format!(
        "Mounting plate of {tag}: {width} x {height} mm, margin {margin} mm"
    ))
}

pub fn add_cabinet_rail(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let id = enclosure_arg(tx.project(), args)?;
    if tx
        .project()
        .get::<novo_core::Structure>(id)
        .and_then(|s| s.plate)
        .is_none()
    {
        return Err("the enclosure has no mounting plate yet; call set_cabinet_plate first".into());
    }
    let x = plate_number(args, "x", 0.0)?;
    let y = plate_number(args, "y", 0.0)?;
    let length = plate_number(args, "length", 20.0)?;
    let rail = tx
        .add_rail(id, x, y, length, RailKind::DinRail)
        .map_err(|e| e.to_string())?;
    if args["vertical"].as_bool() == Some(true) {
        tx.update::<novo_core::Rail>(rail, |r| r.vertical = true)
            .map_err(|e| e.to_string())?;
    }
    Ok(format!("Add a {length} mm DIN rail at ({x}, {y})"))
}

pub fn add_cabinet_duct(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let id = enclosure_arg(tx.project(), args)?;
    if tx
        .project()
        .get::<novo_core::Structure>(id)
        .and_then(|s| s.plate)
        .is_none()
    {
        return Err("the enclosure has no mounting plate yet; call set_cabinet_plate first".into());
    }
    let x = plate_number(args, "x", 0.0)?;
    let y = plate_number(args, "y", 0.0)?;
    let length = plate_number(args, "length", 20.0)?;
    let width = if args["width"].is_null() {
        40.0
    } else {
        plate_number(args, "width", 5.0)?
    };
    let height = if args["height"].is_null() {
        60.0
    } else {
        plate_number(args, "height", 5.0)?
    };
    let class = match args["class"].as_str().unwrap_or("mixed") {
        c @ ("power" | "signal" | "mixed") => c,
        other => {
            return Err(format!(
                "class must be power, signal or mixed, not {other:?}"
            ));
        }
    };
    let created = tx
        .execute(novo_core::Command::AddRail {
            structure: id,
            x,
            y,
            length,
            kind: RailKind::Duct,
            vertical: args["vertical"].as_bool() == Some(true),
            width,
            height,
        })
        .map_err(|e| e.to_string())?;
    if let Some(novo_core::AnyId::Rail(rail)) = created.first() {
        tx.set_property(
            *rail,
            novo_layout::model::DUCT_CLASS_KEY,
            Some(novo_core::Value::Text(class.to_owned())),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "Add a {length} mm {class} duct ({width} x {height} mm) at ({x}, {y})"
    ))
}

pub fn set_cabinet_interior(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    use novo_layout::enclosure::{Door, Enclosure, Hinge};
    let id = enclosure_arg(tx.project(), args)?;
    let old = model::enclosure_of(tx.project(), id);
    let pick = |key: &str, now: Option<f64>, min: f64| -> Result<f64, String> {
        if args[key].is_null() {
            now.ok_or_else(|| {
                format!("{key} is missing (width, height and depth are needed the first time)")
            })
        } else {
            plate_number(args, key, min)
        }
    };
    let width = pick("width", old.map(|e| e.width), 100.0)?;
    let height = pick("height", old.map(|e| e.height), 100.0)?;
    let depth = pick("depth", old.map(|e| e.depth), 50.0)?;
    let setback = if args["plate_setback"].is_null() {
        old.map_or(0.0, |e| e.plate_setback)
    } else {
        plate_number(args, "plate_setback", 0.0)?
    };
    let mut door = old.and_then(|e| e.door);
    match args["door_hinge"].as_str() {
        None => {}
        Some("none") => door = None,
        Some(h) => {
            let hinge = Hinge::parse(h).ok_or("door_hinge must be left, right or none")?;
            let mut d = door.unwrap_or_default();
            d.hinge = hinge;
            door = Some(d);
        }
    }
    if let Some(d) = door.as_mut() {
        if !args["door_angle"].is_null() {
            d.max_angle = plate_number(args, "door_angle", 0.0)?.min(180.0);
        }
        if !args["door_inner"].is_null() {
            d.inner_depth = plate_number(args, "door_inner", 0.0)?;
        }
    }
    let _: Option<Door> = door;
    let keep = |key: &str, now: Option<f64>| -> Result<Option<f64>, String> {
        if args[key].is_null() {
            Ok(now)
        } else {
            plate_number(args, key, 0.0).map(Some)
        }
    };
    let e = Enclosure {
        width,
        height,
        depth,
        plate_setback: setback,
        door,
        front_clearance: keep("front_clearance", old.and_then(|e| e.front_clearance))?,
        hinge_side_clearance: keep(
            "hinge_side_clearance",
            old.and_then(|e| e.hinge_side_clearance),
        )?,
    };
    model::set_enclosure(tx, id, &e).map_err(|e| e.to_string())?;
    let tag = tx
        .project()
        .get::<novo_core::Structure>(id)
        .map(|s| s.tag.to_string())
        .unwrap_or_default();
    Ok(format!(
        "Interior of {tag}: {width} x {height} x {depth} mm{}",
        match door {
            Some(d) => format!(", door hinged {}", d.hinge.as_str()),
            None => ", no door".into(),
        }
    ))
}

pub fn place_in_cabinet(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let id = enclosure_arg(tx.project(), args)?;
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    if tx
        .project()
        .get::<novo_core::Structure>(id)
        .and_then(|s| s.plate)
        .is_none()
    {
        return Err("the enclosure has no mounting plate yet; call set_cabinet_plate first".into());
    }
    if !model::belonging(tx.project(), id)
        .iter()
        .any(|d| d.id == device)
    {
        return Err(format!(
            "{tag} does not belong to this enclosure by its tag (location) or is a field device"
        ));
    }
    let x = plate_number(args, "x", 0.0)?;
    let y = plate_number(args, "y", 0.0)?;
    let rotated = args["rotated"].as_bool().unwrap_or(false);
    let surface = text_of(&args["surface"]).filter(|s| !s.eq_ignore_ascii_case("plate"));
    if let Some(name) = &surface {
        let kind = novo_layout::surfaces::SurfaceKind::from_name(name).ok_or_else(|| {
            format!("surface {name:?} is not one of plate, door, left, right, roof, floor")
        })?;
        if kind == novo_layout::surfaces::SurfaceKind::Door
            && !model::enclosure_of(tx.project(), id).is_some_and(|e| e.door.is_some())
        {
            return Err(
                "the enclosure has no door; call set_cabinet_interior with door_hinge first".into(),
            );
        }
    }
    model::place_device(tx, id, device.raw(), x, y, rotated).map_err(|e| e.to_string())?;
    let placement = tx
        .project()
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.device == device)
        .map(|p| p.id);
    if let Some(placement) = placement {
        let value = surface.clone().map(novo_core::Value::Text);
        tx.set_property(placement, novo_layout::surfaces::SURFACE_PROPERTY, value)
            .map_err(|e| e.to_string())?;
    }
    let on = surface.map_or("the plate".to_owned(), |s| format!("the {s}"));
    Ok(format!(
        "Place {tag} on {on} at ({x}, {y}) mm{}",
        if rotated { ", turned" } else { "" }
    ))
}

pub fn turn_in_cabinet(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    if !tx
        .project()
        .all::<novo_core::CabinetPlacement>()
        .any(|p| p.device == device)
    {
        return Err(format!("{tag} is not placed in a cabinet"));
    }
    model::rotate_device(tx, device.raw()).map_err(|e| e.to_string())?;
    Ok(format!("Turn {tag} by 90 degrees on the plate"))
}

pub fn unplace_from_cabinet(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    if !tx
        .project()
        .all::<novo_core::CabinetPlacement>()
        .any(|p| p.device == device)
    {
        return Err(format!("{tag} is not placed in a cabinet"));
    }
    model::unplace_device(tx, device.raw()).map_err(|e| e.to_string())?;
    Ok(format!("Take {tag} off the plate"))
}

pub fn place_all_in_cabinet(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let id = enclosure_arg(tx.project(), args)?;
    let layout = model::load(tx.project(), id)
        .ok_or("the enclosure has no mounting plate yet; call set_cabinet_plate first")?;
    let off = model::off_plate_devices(tx.project(), id);
    let candidates: Vec<_> = model::candidates(tx.project(), id)
        .into_iter()
        .filter(|c| !off.contains(&c.id))
        .collect();
    let plan = layout.preview(&candidates);
    if plan.placements.is_empty() && plan.unplaced.is_empty() {
        return Err("every device of this enclosure is already placed".into());
    }
    model::apply_plan(tx, id, &plan).map_err(|e| e.to_string())?;
    let left = if plan.unplaced.is_empty() {
        String::new()
    } else {
        format!(
            "; no room for {}",
            plan.unplaced
                .iter()
                .map(|d| tag_of(tx.project(), *d))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Ok(format!(
        "Place {} devices on {} new rails{left}",
        plan.placements.len(),
        plan.rails.len()
    ))
}

/// Finds the catalog part and returns it, or says what is there.
fn catalog_part<'a>(
    catalog: &'a Catalog,
    args: &Value,
) -> Result<&'a novo_parts::CatalogPart, String> {
    let order = need_text(args, "order_number")?;
    let maker = text_of(&args["manufacturer"]);
    let found = match &maker {
        Some(m) => catalog.find(m, &order),
        None => catalog.find_order_number(&order),
    };
    found.ok_or_else(|| {
        format!(
            "the catalog has no part {order}{}; use search_catalog and copy the manufacturer and order number exactly",
            maker.map(|m| format!(" from {m}")).unwrap_or_default()
        )
    })
}

fn need_text(args: &Value, key: &str) -> Result<String, String> {
    text_of(&args[key]).ok_or_else(|| format!("{key} is missing"))
}

pub fn assign_catalog_part(
    tx: &mut Tx<'_>,
    catalog: &Catalog,
    args: &Value,
) -> Result<String, String> {
    let device = resolve::device_arg(tx.project(), args)?;
    let tag = tx
        .project()
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let part = catalog_part(catalog, args)?;
    let mode = if args["add"].as_bool() == Some(true) {
        AssignMode::Add
    } else {
        AssignMode::Replace
    };
    let command =
        novo_parts::assign_part(tx.project(), &[device], part, mode).ok_or_else(|| {
            format!(
                "{tag} already has {} {}",
                part.manufacturer, part.order_number
            )
        })?;
    tx.execute(command).map_err(|e| e.to_string())?;
    Ok(format!(
        "Assign {} {} to {tag} from the part catalog",
        part.manufacturer, part.order_number
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineer::{Resources, actions};

    fn starter() -> (Project, Resources) {
        let res = Resources::new(Vec::new()).with_catalog(Catalog::starter());
        let mut p = Project::new("t");
        for (a, args) in [
            ("add_page", json!({"name": "1"})),
            (
                "place_macro",
                json!({"macro": "Motor starter DOL", "page": "1"}),
            ),
        ] {
            actions::run_on(&mut p, &res.env(), a, &args).unwrap();
        }
        (p, res)
    }

    fn run(p: &mut Project, res: &Resources, a: &str, args: Value) -> Result<String, String> {
        actions::run_on(p, &res.env(), a, &args)
    }

    #[test]
    fn claude_builds_and_reads_a_cabinet() {
        let (mut p, res) = starter();
        assert_eq!(section(&p), json!([]), "no enclosure yet");
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB1", "width": 600, "height": 800}),
        )
        .unwrap();
        let before = section(&p);
        assert_eq!(before[0]["plate"]["width_mm"], 600.0);
        assert!(
            before[0]["not_placed_yet"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t == "-K1")
        );
        let said = run(&mut p, &res, "place_all_in_cabinet", json!({})).unwrap();
        assert!(said.starts_with("Place "), "{said}");
        let after = section(&p);
        let placed = after[0]["placed"].as_array().unwrap();
        assert!(placed.iter().any(|d| d["device"] == "-K1"));
        assert!(
            after[0]["not_placed_yet"].as_array().unwrap().is_empty(),
            "{after}"
        );
        // Move -K1 on top of -F1: the layout problems say so, in words.
        let f1 = placed
            .iter()
            .find(|d| d["device"] == "-F1")
            .unwrap()
            .clone();
        run(
            &mut p,
            &res,
            "place_in_cabinet",
            json!({"device": "-K1", "x": f1["x_mm"], "y": f1["y_mm"]}),
        )
        .unwrap();
        let problems = section(&p)[0]["layout_problems"].to_string();
        assert!(problems.contains("overlaps"), "{problems}");
        run(&mut p, &res, "turn_in_cabinet", json!({"device": "-K1"})).unwrap();
        run(
            &mut p,
            &res,
            "unplace_from_cabinet",
            json!({"device": "-K1"}),
        )
        .unwrap();
        assert!(section(&p)[0]["not_placed_yet"].to_string().contains("-K1"));
    }

    #[test]
    fn rails_need_a_plate_and_bad_input_is_explained() {
        let (mut p, res) = starter();
        run(&mut p, &res, "add_cabinet", json!({"tag": "+CAB1"})).unwrap();
        let why = run(
            &mut p,
            &res,
            "add_cabinet_rail",
            json!({"x": 10, "y": 10, "length": 300}),
        )
        .unwrap_err();
        assert!(why.contains("set_cabinet_plate"), "{why}");
        let why = run(
            &mut p,
            &res,
            "set_cabinet_plate",
            json!({"width": 600, "height": 20}),
        )
        .unwrap_err();
        assert!(why.contains("height"), "{why}");
        run(
            &mut p,
            &res,
            "set_cabinet_plate",
            json!({"width": 600, "height": 800}),
        )
        .unwrap();
        run(
            &mut p,
            &res,
            "add_cabinet_rail",
            json!({"x": 20, "y": 40, "length": 400}),
        )
        .unwrap();
        assert_eq!(section(&p)[0]["rails"].as_array().unwrap().len(), 1);
        let why = run(
            &mut p,
            &res,
            "place_in_cabinet",
            json!({"device": "-Z9", "x": 1, "y": 1}),
        )
        .unwrap_err();
        assert!(why.contains("no device"), "{why}");
        let why = run(&mut p, &res, "add_cabinet", json!({"tag": "+CAB1"})).unwrap_err();
        assert!(why.contains("already"), "{why}");
    }

    #[test]
    fn the_part_picker_search_and_assign() {
        let (mut p, res) = starter();
        let hits = search_catalog(
            &res.catalog,
            &p,
            &json!({"query": "contactor", "device": "-K1"}),
        )
        .unwrap();
        let first = &hits["parts"][0];
        let (maker, order) = (
            first["manufacturer"].as_str().unwrap(),
            first["order_number"].as_str().unwrap(),
        );
        run(
            &mut p,
            &res,
            "assign_catalog_part",
            json!({"device": "-K1", "manufacturer": maker, "order_number": order}),
        )
        .unwrap();
        let k1 = p.devices().find(|d| d.tag.to_string() == "-K1").unwrap();
        let part = p.part(k1.parts[0]).unwrap();
        assert_eq!(part.order_number, order);
        let again = run(
            &mut p,
            &res,
            "assign_catalog_part",
            json!({"device": "-K1", "manufacturer": maker, "order_number": order}),
        );
        assert!(again.unwrap_err().contains("already"));
        let why = run(
            &mut p,
            &res,
            "assign_catalog_part",
            json!({"device": "-K1", "order_number": "NOPE-123"}),
        )
        .unwrap_err();
        assert!(why.contains("search_catalog"), "{why}");
        assert!(search_catalog(&res.catalog, &p, &json!({})).is_err());
    }

    #[test]
    fn the_cabinet_section_follows_the_data_scope() {
        use crate::consent::DataScope;
        let (mut p, res) = starter();
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB1", "width": 600, "height": 800}),
        )
        .unwrap();
        let whole = crate::engineer::snapshot::snapshot_of(&p, &[], DataScope::Project);
        assert!(whole.project["cabinet"][0]["plate"].is_object());
        let outline = crate::engineer::snapshot::snapshot_of(&p, &[], DataScope::Outline);
        assert!(
            outline.project.is_null(),
            "no cabinet data below the project scope"
        );
    }

    #[test]
    fn ducts_interior_door_and_surfaces() {
        let (mut p, res) = starter();
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB1", "width": 600, "height": 800}),
        )
        .unwrap();
        run(
            &mut p,
            &res,
            "add_cabinet_duct",
            json!({"x": 20, "y": 200, "length": 500, "class": "power"}),
        )
        .unwrap();
        let d = &section(&p)[0]["ducts"][0];
        assert_eq!(d["length_mm"], 500.0);
        assert!(
            run(
                &mut p,
                &res,
                "add_cabinet_duct",
                json!({"x": 1, "y": 1, "length": 50, "class": "loud"})
            )
            .is_err()
        );
        // The door must exist before a device can go on it.
        let why = run(
            &mut p,
            &res,
            "place_in_cabinet",
            json!({"device": "-S1", "x": 50, "y": 50, "surface": "door"}),
        )
        .unwrap_err();
        assert!(why.contains("set_cabinet_interior"), "{why}");
        let why = run(&mut p, &res, "set_cabinet_interior", json!({"width": 800})).unwrap_err();
        assert!(why.contains("height"), "{why}");
        run(&mut p, &res, "set_cabinet_interior", json!({"width": 800, "height": 1000, "depth": 300, "door_hinge": "right", "door_angle": 120})).unwrap();
        let i = &section(&p)[0]["interior"];
        assert_eq!(i["door"]["hinge"], "right");
        assert_eq!(i["door"]["max_angle_deg"], 120.0);
        run(
            &mut p,
            &res,
            "place_in_cabinet",
            json!({"device": "-S1", "x": 50, "y": 50, "surface": "door"}),
        )
        .unwrap();
        let placed = section(&p)[0]["placed"].clone();
        assert_eq!(placed[0]["surface"], "door", "{placed}");
        run(
            &mut p,
            &res,
            "place_in_cabinet",
            json!({"device": "-S1", "x": 50, "y": 50}),
        )
        .unwrap();
        assert_eq!(section(&p)[0]["placed"][0]["surface"], "plate");
        assert!(
            run(
                &mut p,
                &res,
                "place_in_cabinet",
                json!({"device": "-S1", "x": 5, "y": 5, "surface": "ceiling"})
            )
            .is_err()
        );
    }

    #[test]
    fn several_enclosures_must_be_named() {
        let (mut p, res) = starter();
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB1", "width": 600, "height": 800}),
        )
        .unwrap();
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB2", "width": 600, "height": 800}),
        )
        .unwrap();
        let why = run(&mut p, &res, "place_all_in_cabinet", json!({})).unwrap_err();
        assert!(why.contains("+CAB1") && why.contains("+CAB2"), "{why}");
    }
}
