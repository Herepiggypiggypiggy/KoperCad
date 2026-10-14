//! Renumber devices (J4): a preview of old and new tags, applied as one
//! undo step.
//!
//! Devices are numbered per letter code within their structure (function
//! and location), in reading order of their main symbol: page, then
//! column, then top to bottom. Numbers used by devices outside the scope
//! are skipped, so renumbering a page never collides with the rest of the
//! project. Renaming a device renames all its symbols with it, since
//! contacts and cross references point at the device, not at its tag.
//! Terminal strips keep their tags. Sub-devices with a suffix keep sharing
//! their number: `-K7.1` and `-K7.2` become `-K1.1` and `-K1.2`.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, PageId, Project, Result, Tx, natural_cmp, split_code,
};

/// Which devices to renumber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Project,
    /// Devices with any symbol on this page.
    Page(PageId),
    Devices(Vec<DeviceId>),
}

/// One row of the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renaming {
    pub device: DeviceId,
    pub old: DeviceTag,
    pub new: DeviceTag,
}

/// Where a device first appears, for reading order.
#[derive(Debug, Clone, PartialEq)]
struct Position {
    page_name: String,
    page: PageId,
    column: u16,
    y: f64,
    x: f64,
}

fn position(project: &Project, device: DeviceId) -> Option<Position> {
    let refs = project.cross_references(device);
    let first = refs.iter().find(|r| r.main).or_else(|| refs.first())?;
    let symbol = project.symbol(first.symbol)?;
    Some(Position {
        page_name: first.location.page_name.clone(),
        page: first.location.page,
        column: first.location.column,
        y: symbol.placement.origin.y,
        x: symbol.placement.origin.x,
    })
}

fn reading_order(a: &Option<Position>, b: &Option<Position>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => natural_cmp(&a.page_name, &b.page_name)
            .then(a.page.cmp(&b.page))
            .then(a.column.cmp(&b.column))
            .then(a.y.total_cmp(&b.y))
            .then(a.x.total_cmp(&b.x)),
        // Devices that are not drawn go last.
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The renamings a renumber would make, in the order it numbers them. Only
/// devices whose tag changes are listed.
pub fn plan(project: &Project, scope: &Scope) -> Vec<Renaming> {
    let in_scope: BTreeSet<DeviceId> = match scope {
        Scope::Project => project.devices().map(|d| d.id).collect(),
        Scope::Page(page) => project
            .devices()
            .filter(|d| {
                project
                    .symbols_of_device(d.id)
                    .iter()
                    .any(|s| s.page == *page)
            })
            .map(|d| d.id)
            .collect(),
        Scope::Devices(ids) => ids.iter().copied().collect(),
    };

    // Group key: structure plus letter code.
    type Group = (Vec<String>, Vec<String>, Vec<String>, String);
    let group_of = |tag: &DeviceTag| -> Group {
        let parent = tag.parent();
        (
            parent.function,
            parent.location,
            parent.product,
            tag.letter_code().to_owned(),
        )
    };

    let mut taken: BTreeMap<Group, BTreeSet<u32>> = BTreeMap::new();
    let mut members: BTreeMap<Group, Vec<(Option<Position>, DeviceId)>> = BTreeMap::new();
    for device in project.devices() {
        if device.kind == DeviceKind::TerminalStrip || device.tag.letter_code().is_empty() {
            continue;
        }
        let group = group_of(&device.tag);
        if in_scope.contains(&device.id) {
            members
                .entry(group)
                .or_default()
                .push((position(project, device.id), device.id));
        } else if let Some(n) = device.tag.number() {
            taken.entry(group).or_default().insert(n);
        }
    }
    for cable in project.cables() {
        if let Some(n) = cable.tag.number() {
            taken.entry(group_of(&cable.tag)).or_default().insert(n);
        }
    }

    let mut out = Vec::new();
    for (group, mut devices) in members {
        devices.sort_by(|a, b| {
            reading_order(&a.0, &b.0).then_with(|| {
                let number = |id| project.device(id).and_then(|d| d.tag.number());
                number(a.1).cmp(&number(b.1)).then(a.1.cmp(&b.1))
            })
        });
        let taken = taken.remove(&group).unwrap_or_default();
        let mut next = 1;
        // Old number of a suffixed sub-device to its new number.
        let mut shared: BTreeMap<u32, u32> = BTreeMap::new();
        for (_, id) in devices {
            let Some(device) = project.device(id) else {
                continue;
            };
            let suffixed = device
                .tag
                .device_code()
                .map(split_code)
                .and_then(|(_, n, suffix)| n.filter(|_| !suffix.is_empty()));
            let number = match suffixed.and_then(|old| shared.get(&old)) {
                Some(&n) => n,
                None => {
                    while taken.contains(&next) {
                        next += 1;
                    }
                    next += 1;
                    if let Some(old) = suffixed {
                        shared.insert(old, next - 1);
                    }
                    next - 1
                }
            };
            let new = device.tag.with_number(number);
            if new != device.tag {
                out.push(Renaming {
                    device: id,
                    old: device.tag.clone(),
                    new,
                });
            }
        }
    }
    out
}

/// Applies a plan inside the caller's transaction (one undo step).
pub fn apply(tx: &mut Tx<'_>, plan: &[Renaming]) -> Result<()> {
    for r in plan {
        tx.set_tag(r.device, r.new.clone())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Placement, Point, SymbolRole};

    fn place(tx: &mut Tx<'_>, page: PageId, tag: &str, x: f64, y: f64) -> Result<DeviceId> {
        let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
        tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(x, y)),
            vec![],
            SymbolRole::Device { device, main: true },
        )?;
        Ok(device)
    }

    #[test]
    fn renumbers_in_reading_order_as_one_step() {
        let mut doc = Document::default();
        let (ids, _) = doc
            .transact("Draw", |tx| {
                let p1 = tx.add_page("1")?;
                let p2 = tx.add_page("2")?;
                Ok([
                    place(tx, p2, "-K7", 10.0, 10.0)?,
                    place(tx, p1, "-K3", 200.0, 10.0)?,
                    place(tx, p1, "-K9", 10.0, 50.0)?,
                    place(tx, p1, "-S4", 10.0, 10.0)?,
                ])
            })
            .unwrap();
        let rows = plan(doc.project(), &Scope::Project);
        let preview: Vec<String> = rows
            .iter()
            .map(|r| format!("{} {}", r.old, r.new))
            .collect();
        assert_eq!(preview, ["-K9 -K1", "-K3 -K2", "-K7 -K3", "-S4 -S1"]);
        doc.transact("Renumber devices", |tx| apply(tx, &rows))
            .unwrap();
        let tag = |id| doc.project().device(id).unwrap().tag.to_string();
        assert_eq!(tag(ids[0]), "-K3");
        assert_eq!(doc.undo_depth(), 2);
        doc.undo().unwrap();
        assert_eq!(doc.project().device(ids[0]).unwrap().tag.to_string(), "-K7");
    }

    #[test]
    fn a_page_skips_numbers_used_elsewhere() {
        let mut doc = Document::default();
        let (p2, _) = doc
            .transact("Draw", |tx| {
                let p1 = tx.add_page("1")?;
                let p2 = tx.add_page("2")?;
                place(tx, p1, "-K1", 10.0, 10.0)?;
                place(tx, p1, "-K2", 50.0, 10.0)?;
                place(tx, p2, "-K8", 10.0, 10.0)?;
                place(tx, p2, "-K5", 50.0, 10.0)?;
                Ok(p2)
            })
            .unwrap();
        let preview: Vec<String> = plan(doc.project(), &Scope::Page(p2))
            .iter()
            .map(|r| format!("{} {}", r.old, r.new))
            .collect();
        assert_eq!(preview, ["-K8 -K3", "-K5 -K4"]);
    }

    #[test]
    fn sub_devices_keep_sharing_their_number() {
        let mut doc = Document::default();
        doc.transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            place(tx, p1, "-K7.1", 10.0, 10.0)?;
            place(tx, p1, "-K7.2", 50.0, 10.0)?;
            place(tx, p1, "-K3", 90.0, 10.0)?;
            Ok(())
        })
        .unwrap();
        let preview: Vec<String> = plan(doc.project(), &Scope::Project)
            .iter()
            .map(|r| format!("{} {}", r.old, r.new))
            .collect();
        assert_eq!(preview, ["-K7.1 -K1.1", "-K7.2 -K1.2", "-K3 -K2"]);
    }

    #[test]
    fn structures_number_separately() {
        let mut doc = Document::default();
        doc.transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            place(tx, p1, "=A1-K4", 10.0, 10.0)?;
            place(tx, p1, "=A2-K6", 50.0, 10.0)?;
            Ok(())
        })
        .unwrap();
        let preview: Vec<String> = plan(doc.project(), &Scope::Project)
            .iter()
            .map(|r| r.new.to_string())
            .collect();
        assert_eq!(preview, ["=A1-K1", "=A2-K1"]);
    }
}
