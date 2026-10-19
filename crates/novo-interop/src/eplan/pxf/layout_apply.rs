//! Puts the mounting plate sheets of an import into the cabinet model: the
//! location becomes an enclosure with a plate, the rails and ducts are added
//! and every device the sheet shows is placed (see [`super::layout`]).

use std::collections::BTreeMap;

use novo_core::{
    Device, DeviceId, DeviceTag, MountingPlate, Point, RailKind, Structure, StructureId, Tx, Value,
};

use super::layout::{LayoutDevice, LayoutSheet};

/// Property keys the cabinet view reads the footprint from.
const WIDTH_KEY: &str = "width_mm";
const HEIGHT_KEY: &str = "height_mm";
/// Cross-section depth of a duct: the sheet only shows the width.
const DUCT_DEPTH_MM: f64 = 75.0;

fn find_device(
    tx: &Tx<'_>,
    device_ids: &BTreeMap<String, DeviceId>,
    location: &str,
    d: &LayoutDevice,
) -> Option<DeviceId> {
    let want_loc = format!("+{location}-");
    let matches = |tag: &DeviceTag, loose: bool| {
        let s = tag.to_string();
        s.contains(&want_loc)
            && s.ends_with(&d.name)
            && (loose || d.function.is_empty() || s.starts_with(&format!("={}", d.function)))
    };
    // Devices of this import first, then any device of the project.
    for loose in [false, true] {
        for (text, id) in device_ids {
            if let Ok(tag) = DeviceTag::parse(text)
                && matches(&tag, loose)
            {
                return Some(*id);
            }
        }
        let found = tx
            .project()
            .devices()
            .find(|dev| matches(&dev.tag, loose))
            .map(|dev| dev.id);
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Returns how many records it created.
pub(super) fn place_sheets(
    tx: &mut Tx<'_>,
    sheets: &[LayoutSheet],
    device_ids: &BTreeMap<String, DeviceId>,
) -> Result<usize, novo_core::ModelError> {
    let mut created = 0usize;
    for sheet in sheets {
        let Ok(tag) = DeviceTag::parse(&format!("+{}", sheet.location)) else {
            continue;
        };
        let Some(structure) = tx.project().structure_by_tag(&tag).map(|s| (s.id, s.plate)) else {
            continue;
        };
        let (sid, plate): (StructureId, _) = structure;
        // An import only adds: a location that already has a plate keeps it.
        if plate.is_some() {
            continue;
        }
        tx.set_enclosure(sid, true)?;
        tx.update::<Structure>(sid, |s| {
            if s.description.is_empty() {
                s.description = sheet.cabinet.clone();
            }
        })?;
        tx.set_plate(
            sid,
            Some(MountingPlate {
                width: sheet.width,
                height: sheet.height,
                margin: sheet.margin,
            }),
        )?;
        created += 1;
        let mut rail_ids = Vec::new();
        for r in &sheet.rails {
            let id = tx.add_rail(sid, r.x, r.y, r.length, RailKind::DinRail)?;
            rail_ids.push((id, r.clone()));
            created += 1;
        }
        for d in &sheet.ducts {
            let id = tx.add_rail(sid, d.x, d.y, d.length, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(id, |r| r.vertical = d.vertical)?;
            tx.set_rail_size(id, d.length, d.width, DUCT_DEPTH_MM)?;
            created += 1;
        }
        for d in &sheet.devices {
            let Some(device) = find_device(tx, device_ids, &sheet.location, d) else {
                continue;
            };
            tx.update::<Device>(device, |dev| {
                for (key, v) in [(WIDTH_KEY, d.w), (HEIGHT_KEY, d.h)] {
                    dev.properties
                        .entry(key.to_string())
                        .or_insert(Value::Float((v * 10.0).round() / 10.0));
                }
            })?;
            // On a rail when the rail's centre line crosses the footprint
            // and the footprint lies within the rail's run.
            let rail = rail_ids.iter().find_map(|(id, r)| {
                (r.y >= d.y - 0.5
                    && r.y <= d.y + d.h + 0.5
                    && d.x >= r.x - 0.5
                    && d.x + d.w <= r.x + r.length + 0.5)
                    .then_some((*id, d.x - r.x))
            });
            tx.place_device(device, sid, Point::new(d.x, d.y), false, rail)?;
            created += 1;
        }
    }
    Ok(created)
}
