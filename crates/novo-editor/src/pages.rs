//! Multiple pages: new pages named in sequence and Page Up / Page Down
//! (WORKFLOW.md shortcut map and the Gate 1 benchmark's "new page 2").

use std::cmp::Ordering;

use novo_core::{AnyId, PageId, PageKind, Project, Result, Tx, Value, natural_cmp};

use crate::naming::PAGE_STRUCTURE;

/// The name a page inserted after `after` gets: the next number (`3` after
/// `2`) when that is free and still sorts before the following page,
/// otherwise a sub-page (`2.1`, `2.2`, ...). Without `after` the page goes
/// at the end.
pub fn next_page_name(project: &Project, after: Option<PageId>) -> String {
    let pages = project.pages_in_order();
    let after = after
        .and_then(|a| pages.iter().position(|p| p.id == a))
        .or_else(|| pages.len().checked_sub(1));
    let Some(index) = after else {
        return "1".into();
    };
    let name = pages[index].name.as_str();
    let next = pages.get(index + 1).map(|p| p.name.as_str());
    let free = |n: &str| !pages.iter().any(|p| p.name == n);
    let fits = |n: &str| {
        natural_cmp(n, name) == Ordering::Greater
            && next.is_none_or(|next| natural_cmp(n, next) == Ordering::Less)
    };
    if let Some(n) = increment(name)
        && free(&n)
        && fits(&n)
    {
        return n;
    }
    (1..)
        .map(|i| format!("{name}.{i}"))
        .find(|n| free(n) && fits(n))
        .expect("some sub-page number is free")
}

/// `2` gives `3`, `3.1` gives `3.2`, `A` gives `None`.
fn increment(name: &str) -> Option<String> {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (head, tail) = name.split_at(name.len() - digits);
    let n: u64 = tail.parse().ok()?;
    Some(format!("{head}{}", n + 1))
}

/// Adds a schematic page after `after` (or at the end), with the same paper
/// format and structure (`=A1+CAB1`) as that page.
pub fn add_page(tx: &mut Tx<'_>, after: Option<PageId>) -> Result<PageId> {
    let name = next_page_name(tx.project(), after);
    let template = after
        .or_else(|| tx.project().pages_in_order().last().map(|p| p.id))
        .and_then(|a| tx.project().page(a))
        .map(|p| (p.format, p.properties.get(PAGE_STRUCTURE).cloned()));
    let page = tx.add_page(name)?;
    if let Some((format, structure)) = template {
        tx.update::<novo_core::Page>(page, |p| {
            p.format = format;
            p.kind = PageKind::Schematic;
        })?;
        if let Some(structure @ Value::Text(_)) = structure {
            tx.set_property(AnyId::from(page), PAGE_STRUCTURE, Some(structure))?;
        }
    }
    Ok(page)
}

/// The page `step` places before (negative) or after (positive) `page` in
/// page order, stopping at the first and last page.
pub fn neighbour(project: &Project, page: PageId, step: i32) -> Option<PageId> {
    let pages = project.pages_in_order();
    let i = pages.iter().position(|p| p.id == page)? as i64;
    let j = (i + i64::from(step)).clamp(0, pages.len() as i64 - 1);
    (j != i).then(|| pages[j as usize].id)
}

#[cfg(test)]
mod tests {
    use novo_core::Document;

    use super::*;

    fn doc(names: &[&str]) -> (Document, Vec<PageId>) {
        let mut doc = Document::default();
        let (ids, _) = doc
            .transact("pages", |tx| {
                names
                    .iter()
                    .map(|n| tx.add_page(*n))
                    .collect::<Result<Vec<_>>>()
            })
            .unwrap();
        (doc, ids)
    }

    #[test]
    fn names_follow_on() {
        let (d, ids) = doc(&[]);
        assert!(ids.is_empty());
        assert_eq!(next_page_name(d.project(), None), "1");
        let (d, ids) = doc(&["1", "2"]);
        assert_eq!(next_page_name(d.project(), None), "3");
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "3");
        // Between 1 and 2 there is no room for a number: a sub-page.
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "1.1");
        let (d, ids) = doc(&["1", "1.1", "2"]);
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "1.2");
        let (d, ids) = doc(&["Cover"]);
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "Cover.1");
    }

    #[test]
    fn new_page_copies_format_and_structure() {
        let (mut d, ids) = doc(&["1"]);
        d.transact("setup", |tx| {
            tx.update::<novo_core::Page>(ids[0], |p| p.format.size = novo_core::PaperSize::A4)?;
            tx.set_property(
                AnyId::from(ids[0]),
                PAGE_STRUCTURE,
                Some(Value::Text("=A1+CAB1".into())),
            )
        })
        .unwrap();
        let (page, _) = d
            .transact("New page", |tx| add_page(tx, Some(ids[0])))
            .unwrap();
        let p = d.project().page(page).unwrap();
        assert_eq!(p.name, "2");
        assert_eq!(p.format.size, novo_core::PaperSize::A4);
        assert_eq!(
            crate::naming::next_device_tag(d.project(), page, "K").to_string(),
            "=A1+CAB1-K1"
        );
        assert_eq!(neighbour(d.project(), page, -1), Some(ids[0]));
        assert_eq!(neighbour(d.project(), page, 1), None);
    }
}
