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

/// Device tags of the import, parsed once per sheet set.
struct Tags(Vec<(DeviceTag, DeviceId)>);

impl Tags {
    fn new(device_ids: &BTreeMap<String, DeviceId>) -> Self {
        Self(
            device_ids
                .iter()
                .filter_map(|(text, id)| DeviceTag::parse(text).ok().map(|t| (t, *id)))
                .collect(),
        )
    }

    fn find(&self, tx: &Tx<'_>, location: &str, d: &LayoutDevice) -> Option<DeviceId> {
        let want_loc = format!("+{location}-");
        let matches = |tag: &DeviceTag, loose: bool| {
            let s = tag.to_string();
            s.contains(&want_loc)
                && s.ends_with(&d.name)
                && (loose || d.function.is_empty() || s.starts_with(&format!("={}", d.function)))
        };
        // Devices of this import first, then any device of the project.
        for loose in [false, true] {
            if let Some((_, id)) = self.0.iter().find(|(t, _)| matches(t, loose)) {
                return Some(*id);
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
}

/// How many devices the sheet shows that have no device of this import to
/// stand for them (so they cannot be placed).
pub(super) fn unmatched(sheet: &LayoutSheet, tags: &[DeviceTag]) -> usize {
    let want_loc = format!("+{}-", sheet.location);
    let texts: Vec<String> = tags
        .iter()
        .map(|t| t.to_string())
        .filter(|s| s.contains(&want_loc))
        .collect();
    sheet
        .devices
        .iter()
        .filter(|d| !texts.iter().any(|s| s.ends_with(&d.name)))
        .count()
}

/// Returns how many records it created.
pub(super) fn place_sheets(
    tx: &mut Tx<'_>,
    sheets: &[LayoutSheet],
    device_ids: &BTreeMap<String, DeviceId>,
) -> Result<usize, novo_core::ModelError> {
    let tags = Tags::new(device_ids);
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
            let Some(device) = tags.find(tx, &sheet.location, d) else {
                continue;
            };
            // On a rail when the rail's centre line crosses the footprint
            // and the footprint lies within the rail's run.
            let rail = rail_ids.iter().find_map(|(id, r)| {
                (r.y >= d.y - 0.5
                    && r.y <= d.y + d.h + 0.5
                    && d.x >= r.x - 0.5
                    && d.x + d.w <= r.x + r.length + 0.5)
                    .then_some((*id, d.x - r.x))
            });
            // A device that cannot be placed is left out; the import goes on.
            if tx
                .place_device(device, sid, Point::new(d.x, d.y), false, rail)
                .is_err()
            {
                continue;
            }
            tx.update::<Device>(device, |dev| {
                for (key, v) in [(WIDTH_KEY, d.w), (HEIGHT_KEY, d.h)] {
                    dev.properties
                        .entry(key.to_string())
                        .or_insert(Value::Float((v * 10.0).round() / 10.0));
                }
            })?;
            created += 1;
        }
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str) -> LayoutDevice {
        LayoutDevice {
            name: name.to_string(),
            function: String::new(),
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            strip: None,
        }
    }

    #[test]
    fn sheet_devices_without_a_device_in_the_import_are_counted() {
        let sheet = LayoutSheet {
            page: 0,
            location: "CAB1".into(),
            cabinet: String::new(),
            width: 600.0,
            height: 760.0,
            margin: 15.0,
            rails: Vec::new(),
            ducts: Vec::new(),
            devices: vec![device("-Q1"), device("-Q2"), device("-Q3")],
            scale: 1.0,
            notes: Vec::new(),
        };
        let tags = [
            DeviceTag::parse("=A1+CAB1-Q1").unwrap(),
            // Same name, other location: does not stand for -Q2 here.
            DeviceTag::parse("=A1+CAB2-Q2").unwrap(),
        ];
        assert_eq!(unmatched(&sheet, &tags), 2);
        // A device the project already holds stands for its sheet entry too.
        let more = [tags[0].clone(), DeviceTag::parse("=A1+CAB1-Q3").unwrap()];
        assert_eq!(unmatched(&sheet, &more), 1);
    }
}
