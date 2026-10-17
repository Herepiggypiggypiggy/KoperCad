//! Wires and cables as outline items: what the outline's Wires and Cables
//! sections list. Pure data (no egui), so the grouping, the filter and the
//! item text are tested on a plain [`Project`].
//!
//! Wires are grouped by page (in page order), cables are one flat list.
//! Each item names what it is and where it goes, so it can be found by
//! number, tag or device and jumped to.

use novo_core::{CableId, Endpoint, PageId, Project, Wire, WireId, natural_cmp};

/// One wire row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireItem {
    pub id: WireId,
    /// `105  -K1 to -X1`; an unnumbered wire shows `Wire`.
    pub text: String,
}

/// The wires of one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGroup {
    pub page: PageId,
    pub page_name: String,
    pub wires: Vec<WireItem>,
}

/// One cable row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CableItem {
    pub id: CableId,
    /// `-W1  4G1.5  3 of 4 cores wired`.
    pub text: String,
    /// The wire of its first assigned core: where a click goes.
    pub first_wire: Option<WireId>,
}

fn end_text(project: &Project, end: &Endpoint) -> String {
    match end {
        Endpoint::Pin { symbol, .. } => project
            .symbol(*symbol)
            .and_then(|s| project.device(s.device()?))
            .map_or_else(|| "?".to_owned(), |d| d.tag.to_string()),
        Endpoint::Junction { .. } => "junction".to_owned(),
        Endpoint::Free { .. } => "open end".to_owned(),
    }
}

fn wire_text(project: &Project, wire: &Wire) -> String {
    let name = wire.number.as_deref().unwrap_or("Wire");
    format!(
        "{name}  {} to {}",
        end_text(project, &wire.from),
        end_text(project, &wire.to)
    )
}

fn matches(text: &str, filter: &str) -> bool {
    filter.is_empty() || text.to_lowercase().contains(filter)
}

/// Total wires, for the section title.
pub fn wire_count(project: &Project) -> usize {
    project.wires().count()
}

/// Wires by page, numbered ones first in natural order, then the unnumbered.
/// `filter` (case-insensitive) keeps the wires whose text or page name
/// contains it; pages left empty are dropped.
pub fn wire_groups(project: &Project, filter: &str) -> Vec<WireGroup> {
    let filter = filter.trim().to_lowercase();
    let mut groups = Vec::new();
    for page in project.pages_in_order() {
        let mut wires: Vec<&Wire> = project.wires().filter(|w| w.page == page.id).collect();
        if wires.is_empty() {
            continue;
        }
        wires.sort_by(|a, b| match (&a.number, &b.number) {
            (Some(x), Some(y)) => natural_cmp(x, y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        let page_hit = matches(&page.name, &filter);
        let items: Vec<WireItem> = wires
            .into_iter()
            .map(|w| WireItem {
                id: w.id,
                text: wire_text(project, w),
            })
            .filter(|i| page_hit || matches(&i.text, &filter))
            .collect();
        if !items.is_empty() {
            groups.push(WireGroup {
                page: page.id,
                page_name: page.name.clone(),
                wires: items,
            });
        }
    }
    groups
}

/// Cables in tag order, filtered like the wires.
pub fn cable_items(project: &Project, filter: &str) -> Vec<CableItem> {
    let filter = filter.trim().to_lowercase();
    let mut cables: Vec<_> = project.cables().collect();
    cables.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    cables
        .into_iter()
        .map(|c| {
            let wired = c.cores.iter().filter(|core| core.wire.is_some()).count();
            let first_wire = c
                .cores
                .iter()
                .filter_map(|core| core.wire)
                .find(|&w| project.wire(w).is_some());
            let mut text = c.tag.to_string();
            if !c.description.is_empty() {
                text += &format!("  {}", c.description);
            }
            text += &format!("  {wired} of {} cores wired", c.cores.len());
            CableItem {
                id: c.id,
                text,
                first_wire,
            }
        })
        .filter(|i| matches(&i.text, &filter))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{CableCore, DeviceTag, Document, Point, Tx};

    fn project() -> (Document, WireId, WireId, CableId) {
        let mut doc = Document::new(Project::new("t"));
        let (w1, w2, cable) = doc
            .transact("setup", |tx: &mut Tx<'_>| {
                let page = tx.add_page("1")?;
                let a = tx.add_junction(page, Point::new(0.0, 0.0))?;
                let b = tx.add_junction(page, Point::new(10.0, 0.0))?;
                let add = |tx: &mut Tx<'_>, number: &str| {
                    tx.insert(|id| {
                        let mut w =
                            Wire::new(id, page, Endpoint::junction(a), Endpoint::junction(b));
                        w.number = Some(number.to_owned());
                        w
                    })
                };
                let w1 = add(tx, "12")?;
                let w2 = add(tx, "2")?;
                let cable = tx.add_cable(
                    DeviceTag::parse("-W1").unwrap(),
                    vec![CableCore::new("1"), CableCore::new("2")],
                )?;
                tx.assign_core(cable, 1, Some(w2))?;
                Ok((w1, w2, cable))
            })
            .unwrap()
            .0;
        (doc, w1, w2, cable)
    }

    #[test]
    fn wires_group_by_page_in_natural_order() {
        let (doc, w1, w2, _) = project();
        let groups = wire_groups(doc.project(), "");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].page_name, "1");
        let ids: Vec<_> = groups[0].wires.iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![w2, w1], "2 before 12");
        assert_eq!(wire_count(doc.project()), 2);
    }

    #[test]
    fn the_filter_matches_number_and_page() {
        let (doc, w1, _, _) = project();
        let hit = wire_groups(doc.project(), " 12 ");
        assert_eq!(hit[0].wires.len(), 1);
        assert_eq!(hit[0].wires[0].id, w1);
        assert!(wire_groups(doc.project(), "nothing").is_empty());
        assert_eq!(wire_groups(doc.project(), "1")[0].wires.len(), 2, "page 1");
    }

    #[test]
    fn cables_list_with_their_first_wire() {
        let (doc, _, w2, cable) = project();
        let items = cable_items(doc.project(), "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, cable);
        assert_eq!(items[0].first_wire, Some(w2));
        assert!(items[0].text.contains("-W1") && items[0].text.contains("1 of 2"));
        assert!(cable_items(doc.project(), "w9").is_empty());
    }
}
