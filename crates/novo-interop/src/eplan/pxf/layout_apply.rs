//! Puts the mounting plate sheets of an import into the cabinet model: the
//! location becomes an enclosure with a plate, the rails and ducts are added
//! and every device the sheet shows is placed (see [`super::layout`]).

use std::collections::BTreeMap;

use novo_core::{
    Device, DeviceId, DeviceTag, MountingPlate, Part, Point, RailKind, Structure, StructureId, Tx,
    Value,
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
            let size = [
                (WIDTH_KEY, (d.w * 10.0).round() / 10.0),
                (HEIGHT_KEY, (d.h * 10.0).round() / 10.0),
            ];
            tx.update::<Device>(device, |dev| {
                for (key, v) in size {
                    dev.properties
                        .entry(key.to_string())
                        .or_insert(Value::Float(v));
                }
            })?;
            // The outline is that part's footprint: every other device with
            // the same part (not on this sheet) is sized by it too. A part
            // that already has a size keeps it.
            let parts = tx
                .project()
                .device(device)
                .map(|dev| dev.parts.clone())
                .unwrap_or_default();
            for part in parts {
                tx.update::<Part>(part, |p| {
                    if p.properties.contains_key(WIDTH_KEY) || p.properties.contains_key(HEIGHT_KEY)
                    {
                        return;
                    }
                    for (key, v) in size {
                        p.properties.insert(key.to_string(), Value::Float(v));
                    }
                })?;
            }
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
    fn the_outline_on_the_sheet_sizes_the_part_for_every_device_that_uses_it() {
        let mut doc = novo_core::Document::default();
        let (q1, q2, part) = doc
            .transact("setup", |tx| {
                tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
                let q1 = tx.add_device(DeviceTag::parse("=A1+CAB1-Q1")?, "")?;
                let q2 = tx.add_device(DeviceTag::parse("=A1+CAB1-Q2")?, "")?;
                let part = tx.add_part("Maker", "X-1")?;
                tx.assign_part(q1, part)?;
                tx.assign_part(q2, part)?;
                Ok((q1, q2, part))
            })
            .unwrap()
            .0;
        let sheet = LayoutSheet {
            page: 0,
            location: "CAB1".into(),
            cabinet: String::new(),
            width: 600.0,
            height: 760.0,
            margin: 15.0,
            rails: Vec::new(),
            ducts: Vec::new(),
            // Only -Q1 is drawn on the sheet.
            devices: vec![LayoutDevice {
                w: 45.0,
                h: 88.9,
                ..device("-Q1")
            }],
            scale: 1.0,
            notes: Vec::new(),
        };
        let ids: BTreeMap<String, DeviceId> = [
            ("=A1+CAB1-Q1".to_string(), q1),
            ("=A1+CAB1-Q2".to_string(), q2),
        ]
        .into();
        doc.transact("place", |tx| place_sheets(tx, &[sheet], &ids))
            .unwrap();
        let p = doc.project();
        let part = p.get::<Part>(part).unwrap();
        assert_eq!(part.properties.get(WIDTH_KEY), Some(&Value::Float(45.0)));
        assert_eq!(part.properties.get(HEIGHT_KEY), Some(&Value::Float(88.9)));
        // -Q2 is not on the sheet, but its part now has a footprint.
        assert!(!p.device(q2).unwrap().properties.contains_key(WIDTH_KEY));
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
