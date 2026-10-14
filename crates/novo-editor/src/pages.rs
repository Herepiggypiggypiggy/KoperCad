//! Multiple pages: new pages named in sequence and Page Up / Page Down
//! (DESIGN.md §4 keys and the §6 Gate 1 benchmark).

use std::cmp::Ordering;

use novo_core::{PageId, PageKind, Project, Result, Tx, natural_cmp};

/// The name a page inserted after `after` gets: the next number (`3` after
/// `2`) when that is free and still sorts before the following page,
/// otherwise a sub-page (`2.1`, `2.2`, ...). Without `after` the page goes
/// at the end. The new page has the structure of the page it follows (see
/// [`add_page`]), and page names only need to be unique and ordered within
/// one structure (`=A1/3` and `=A2/3` are different pages), so only pages
/// of that structure are compared.
pub fn next_page_name(project: &Project, after: Option<PageId>) -> String {
    let all = project.pages_in_order();
    let Some(anchor) = after
        .and_then(|a| all.iter().find(|p| p.id == a))
        .or_else(|| all.last())
    else {
        return "1".into();
    };
    let pages: Vec<_> = all
        .iter()
        .filter(|p| p.structure.same_structure(&anchor.structure))
        .collect();
    let index = pages
        .iter()
        .position(|p| p.id == anchor.id)
        .expect("the anchor page has its own structure");
    let name = anchor.name.as_str();
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
        .map(|p| (p.format, p.structure.clone()));
    let page = tx.add_page(name)?;
    if let Some((format, structure)) = template {
        tx.update::<novo_core::Page>(page, |p| {
            p.format = format;
            p.kind = PageKind::Schematic;
            p.structure = structure;
        })?;
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

    fn structure(text: &str) -> novo_core::DeviceTag {
        novo_core::DeviceTag::parse(text).unwrap()
    }

    #[test]
    fn names_follow_on_within_the_structure() {
        // =A1 has pages 1 and 2, =A2 has pages 1, 2 and 5. In page order
        // =A1/2 is followed by =A2/1, which must not stop =A1 from going on
        // to 3, and =A2's names do not count as taken for =A1.
        let (mut d, ids) = doc(&["1", "2", "1", "2", "5"]);
        d.transact("structures", |tx| {
            for (i, id) in ids.iter().enumerate() {
                let s = if i < 2 { "=A1" } else { "=A2" };
                tx.update::<novo_core::Page>(*id, |p| p.structure = structure(s))?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "3");
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "1.1");
        assert_eq!(next_page_name(d.project(), Some(ids[3])), "3");
        assert_eq!(next_page_name(d.project(), Some(ids[4])), "6");
        // Without a page: after the last page in order, =A2/5.
        assert_eq!(next_page_name(d.project(), None), "6");
        let (page, _) = d
            .transact("New page", |tx| add_page(tx, Some(ids[1])))
            .unwrap();
        let p = d.project().page(page).unwrap();
        assert_eq!(
            (p.name.as_str(), p.structure.to_string()),
            ("3", "=A1".into())
        );
        assert_eq!(neighbour(d.project(), ids[1], 1), Some(page));
        assert_eq!(neighbour(d.project(), page, 1), Some(ids[2]));
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
            tx.update::<novo_core::Page>(ids[0], |p| {
                p.format.size = novo_core::PaperSize::A4;
                p.structure = structure("=A1+CAB1");
            })
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
