//! Automatic names for interruption points.
//!
//! A new interruption point takes the lowest whole number no other
//! interruption point uses, so the first pair on a project is `1`, the next
//! `2`. The partner on another page takes the name when it is linked
//! ([`crate::link_command`]), so numbering never has to be typed.

use std::collections::HashSet;

use novo_core::{Project, Result, SymbolId, SymbolRole, Tx};

/// The lowest whole number no interruption point is named with yet.
pub fn next_interruption_name(project: &Project) -> String {
    let used: HashSet<u64> = project
        .symbols()
        .filter_map(|s| match &s.role {
            SymbolRole::Interruption { name } => name.parse().ok(),
            _ => None,
        })
        .collect();
    (1..).find(|n| !used.contains(n)).unwrap().to_string()
}

/// Names `symbol` with the next free number when it is an interruption point
/// without a name. Returns the new name, or `None` when nothing changed.
pub fn auto_name_interruption(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<Option<String>> {
    let unnamed = matches!(
        tx.project().symbol(symbol).map(|s| &s.role),
        Some(SymbolRole::Interruption { name }) if name.is_empty()
    );
    if !unnamed {
        return Ok(None);
    }
    let name = next_interruption_name(tx.project());
    tx.set_role(symbol, SymbolRole::Interruption { name: name.clone() })?;
    Ok(Some(name))
}

/// Numbers interruption points again from 1 in reading order (page order,
/// then top to bottom, then left to right). `symbols` are the points to
/// renumber, or every point in the project when it is empty. A pair keeps
/// one name: a point sharing its name with one of `symbols` comes along.
/// Numbers used by points left alone are skipped. Returns how many points
/// got a different name.
pub fn renumber_interruptions(tx: &mut Tx<'_>, symbols: &[SymbolId]) -> Result<usize> {
    let project = tx.project();
    let named = |id: SymbolId| match project.symbol(id).map(|s| &s.role) {
        Some(SymbolRole::Interruption { name }) => Some(name.clone()),
        _ => None,
    };
    let all: Vec<SymbolId> = project
        .symbols()
        .filter(|s| matches!(s.role, SymbolRole::Interruption { .. }))
        .map(|s| s.id)
        .collect();
    let picked: HashSet<SymbolId> = symbols
        .iter()
        .copied()
        .filter(|&s| named(s).is_some())
        .collect();
    if picked.is_empty() && !symbols.is_empty() {
        return Ok(0);
    }
    let picked_names: HashSet<String> = picked
        .iter()
        .filter_map(|&s| named(s))
        .filter(|n| !n.is_empty())
        .collect();
    let mut targets: Vec<SymbolId> = all
        .iter()
        .copied()
        .filter(|s| {
            symbols.is_empty()
                || picked.contains(s)
                || named(*s).is_some_and(|n| picked_names.contains(&n))
        })
        .collect();
    let page_index: std::collections::HashMap<_, _> = project
        .pages()
        .enumerate()
        .map(|(i, p)| (p.id, i))
        .collect();
    let key = |id: SymbolId| {
        let s = project.symbol(id).expect("listed above");
        (
            page_index.get(&s.page).copied().unwrap_or(usize::MAX),
            s.placement.origin.y,
            s.placement.origin.x,
            id,
        )
    };
    targets.sort_by(|&a, &b| {
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0)
            .then(ka.1.total_cmp(&kb.1))
            .then(ka.2.total_cmp(&kb.2))
            .then(ka.3.cmp(&kb.3))
    });
    let target_set: HashSet<SymbolId> = targets.iter().copied().collect();
    let kept: HashSet<u64> = all
        .iter()
        .filter(|s| !target_set.contains(s))
        .filter_map(|&s| named(s).and_then(|n| n.parse().ok()))
        .collect();
    let mut free = (1u64..).filter(|n| !kept.contains(n));
    let mut by_name: std::collections::HashMap<String, String> = Default::default();
    let mut renames = Vec::new();
    for &id in &targets {
        let old = named(id).unwrap_or_default();
        let new = if old.is_empty() {
            free.next().unwrap().to_string()
        } else {
            by_name
                .entry(old.clone())
                .or_insert_with(|| free.next().unwrap().to_string())
                .clone()
        };
        if new != old {
            renames.push((id, new));
        }
    }
    for (id, name) in &renames {
        tx.set_role(*id, SymbolRole::Interruption { name: name.clone() })?;
    }
    Ok(renames.len())
}

/// Names every unnamed interruption point, in symbol order. Returns the
/// symbols named.
pub fn auto_name_interruptions(tx: &mut Tx<'_>) -> Result<Vec<SymbolId>> {
    let mut unnamed: Vec<SymbolId> = tx
        .project()
        .symbols()
        .filter(|s| matches!(&s.role, SymbolRole::Interruption { name } if name.is_empty()))
        .map(|s| s.id)
        .collect();
    unnamed.sort();
    for &id in &unnamed {
        auto_name_interruption(tx, id)?;
    }
    Ok(unnamed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Placement, Point, Project, SymbolInstance, SymbolKey};

    fn point(tx: &mut Tx<'_>, page: novo_core::PageId, x: f64, y: f64, name: &str) -> SymbolId {
        tx.insert(|id| SymbolInstance {
            id,
            page,
            symbol: SymbolKey("iec/connectivity/interruption".into()),
            placement: Placement {
                origin: Point::new(x, y),
                ..Default::default()
            },
            role: SymbolRole::Interruption { name: name.into() },
            pins: Vec::new(),
            bridges: Vec::new(),
            pass_through: Vec::new(),
            feeds: Vec::new(),
            properties: Default::default(),
        })
        .unwrap()
    }

    fn names(doc: &Document, ids: &[SymbolId]) -> Vec<String> {
        ids.iter()
            .map(|&i| match &doc.project().symbol(i).unwrap().role {
                SymbolRole::Interruption { name } => name.clone(),
                _ => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn renumbering_goes_in_reading_order_and_keeps_pairs_together() {
        let mut doc = Document::new(Project::new("t"));
        let ids = doc
            .transact("setup", |tx| {
                let page = tx.add_page("A")?;
                Ok(vec![
                    point(tx, page, 50.0, 10.0, "7"),
                    point(tx, page, 10.0, 10.0, "3"),
                    point(tx, page, 10.0, 90.0, "7"),
                    point(tx, page, 10.0, 50.0, ""),
                ])
            })
            .unwrap()
            .0;
        let n = doc
            .transact("r", |tx| renumber_interruptions(tx, &[]))
            .unwrap()
            .0;
        assert_eq!(n, 4);
        // (10,10)=3 -> 1, (50,10)=7 -> 2, (10,50)="" -> 3, (10,90)=7 -> 2.
        assert_eq!(names(&doc, &ids), ["2", "1", "2", "3"]);
        let n = doc
            .transact("r", |tx| renumber_interruptions(tx, &[]))
            .unwrap()
            .0;
        assert_eq!(n, 0, "already in order");
    }

    #[test]
    fn a_selection_leaves_other_numbers_alone() {
        let mut doc = Document::new(Project::new("t"));
        let ids = doc
            .transact("setup", |tx| {
                let page = tx.add_page("A")?;
                Ok(vec![
                    point(tx, page, 10.0, 10.0, "1"),
                    point(tx, page, 10.0, 50.0, "9"),
                    point(tx, page, 10.0, 90.0, "9"),
                ])
            })
            .unwrap()
            .0;
        doc.transact("r", |tx| renumber_interruptions(tx, &[ids[1]]))
            .unwrap();
        assert_eq!(names(&doc, &ids), ["1", "2", "2"]);
    }
}
