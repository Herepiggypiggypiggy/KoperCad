//! Multiple pages: new pages named in sequence and Page Up / Page Down
//! (DESIGN.md §4 keys and the §6 Gate 1 benchmark).

use std::cmp::Ordering;

use novo_core::{Page, PageId, PageKind, Project, Result, Tx, natural_cmp};

/// Letters for page names that fall between two numbers: no i, l or o,
/// which read as digits (the same 23 letters as generated sheets, DESIGN.md
/// §3 and §8).
pub const PAGE_LETTERS: &str = "abcdefghjkmnpqrstuvwxyz";

/// Why a page name is refused, if it is. A dot is reserved for
/// `page.column` references, so page names never contain one.
pub fn page_name_problem(name: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        Some("a page needs a name")
    } else if name.contains('.') {
        Some("a dot is reserved for page.column references")
    } else {
        None
    }
}

/// How a new page goes in after another one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInsert {
    /// The new page's name.
    pub name: String,
    /// Pages of a lettered run that move up one letter to make room
    /// (`3a` becomes `3b`), last page first. References follow, since they
    /// point at pages, not names.
    pub renames: Vec<(PageId, String)>,
    /// What to tell the user when the page could not go where asked
    /// ("inserted as 3d: 3a–3c are frozen").
    pub note: Option<String>,
}

/// The name a page inserted after `after` gets: the next number (`3` after
/// `2`) when that is free and still sorts before the following page,
/// otherwise the next free letter after the anchor's number (`3a`, `3b`,
/// ...; `Cover a` after a name that ends in a letter). Page names never
/// contain a dot. Without `after` the page goes at the end. The new page
/// has the structure of the page it follows (see [`add_page`]), and page
/// names only need to be unique and ordered within one structure
/// (`=A1/3` and `=A2/3` are different pages), so only pages of that
/// structure are compared.
pub fn next_page_name(project: &Project, after: Option<PageId>) -> String {
    plan_insert(project, after, |p| p.frozen.is_some()).name
}

/// Where and how a page inserted after `after` goes in (see
/// [`next_page_name`]). Between `3` and an existing `3a` no letter fits, so
/// the run `3a`, `3b`, ... moves up one letter and the new page becomes
/// `3a`, unless a page of the run is `frozen` (printed or issued; frozen
/// names never move): then the new page goes at the end of the run.
pub fn plan_insert(
    project: &Project,
    after: Option<PageId>,
    frozen: impl Fn(&Page) -> bool,
) -> PageInsert {
    let plain = |name: String| PageInsert {
        name,
        renames: Vec::new(),
        note: None,
    };
    let all = project.pages_in_order();
    let Some(anchor) = after
        .and_then(|a| all.iter().find(|p| p.id == a))
        .or_else(|| all.last())
    else {
        return plain("1".into());
    };
    let pages: Vec<&Page> = all
        .iter()
        .copied()
        .filter(|p| p.structure.same_structure(&anchor.structure))
        .collect();
    let index = pages
        .iter()
        .position(|p| p.id == anchor.id)
        .expect("the anchor page has its own structure");
    let name = anchor.name.as_str();
    let free = |n: &str| !pages.iter().any(|p| p.name == n);
    let fits_before = |n: &str, next: Option<&Page>| {
        natural_cmp(n, name) == Ordering::Greater
            && next.is_none_or(|next| natural_cmp(n, &next.name) == Ordering::Less)
    };
    let next = pages.get(index + 1).copied();
    let fits = |n: &str| free(n) && fits_before(n, next);
    if let Some(n) = increment(name)
        && fits(&n)
    {
        return plain(n);
    }
    // `3a` takes the letters after its own (`3b`); `3` and `Cover` start
    // a run of their own.
    let (stem, from) = letter_suffix(name).unwrap_or_else(|| {
        let sep = if name.ends_with(|c: char| c.is_ascii_digit()) {
            ""
        } else {
            " "
        };
        (format!("{name}{sep}"), 0)
    });
    let letters: Vec<char> = PAGE_LETTERS.chars().collect();
    // All single letters taken up to the next page: two letters after the
    // anchor itself (`3ba` between `3b` and `3c`).
    if let Some(n) = letters[from..]
        .iter()
        .map(|l| format!("{stem}{l}"))
        .chain(letters.iter().map(|l| format!("{name}{l}")))
        .find(|n| fits(n))
    {
        return plain(n);
    }
    // Between `3` and `3a` no letter sorts before `a`. The run is every
    // page after the anchor whose name is the anchor's prefix plus a page
    // letter and anything after it (`3a`, `3b`, `3ba`, `3c`).
    let prefix = if name.ends_with(|c: char| c.is_ascii_digit()) || letter_suffix(name).is_some() {
        name.to_string()
    } else {
        format!("{name} ")
    };
    let first_letter = |n: &str| {
        n.strip_prefix(prefix.as_str())
            .and_then(|rest| rest.chars().next())
            .and_then(|c| PAGE_LETTERS.find(c))
    };
    let run: Vec<&Page> = pages[index + 1..]
        .iter()
        .copied()
        .take_while(|p| first_letter(&p.name).is_some())
        .collect();
    let after_run = pages.get(index + 1 + run.len()).copied();
    let in_run = |id: PageId| run.iter().any(|r| r.id == id);
    // Each run page moves up by its first letter only, so `3ba` becomes
    // `3ca` and the order stays the same.
    let shift = |n: &str| {
        let i = first_letter(n)?;
        let next = letters.get(i + 1)?;
        let rest = &n[prefix.len() + letters[i].len_utf8()..];
        Some(format!("{prefix}{next}{rest}"))
    };
    let frozen_pages: Vec<&Page> = run.iter().copied().filter(|p| frozen(p)).collect();
    if !run.is_empty() && frozen_pages.is_empty() {
        let moved: Option<Vec<String>> = run.iter().map(|p| shift(&p.name)).collect();
        if let Some(moved) = moved
            && moved.iter().all(|n| {
                !pages.iter().any(|p| p.name == *n && !in_run(p.id)) && fits_before(n, after_run)
            })
        {
            return PageInsert {
                name: format!("{prefix}{}", letters[0]),
                renames: run
                    .iter()
                    .zip(moved)
                    .rev()
                    .map(|(p, n)| (p.id, n))
                    .collect(),
                note: None,
            };
        }
    }
    // Frozen names never move (and a run that cannot shift stays put): the
    // page goes at the end of the run.
    let last = run.last().map_or(name, |p| p.name.as_str());
    let end = |n: &str| free(n) && fits_before(n, after_run) && natural_cmp(n, last).is_gt();
    let n = letters
        .iter()
        .map(|l| format!("{prefix}{l}"))
        .chain(letters.iter().map(|l| format!("{last}{l}")))
        .chain((1..=letters.len()).map(|i| match i {
            1 => format!("{last} new"),
            i => format!("{last} new {i}"),
        }))
        .find(|n| end(n))
        // Nothing fits in order: any free name, never a crash.
        .unwrap_or_else(|| {
            (1..)
                .map(|i| format!("{name} new {i}"))
                .find(|n| free(n))
                .expect("some name is free")
        });
    let note = match frozen_pages.as_slice() {
        [] => format!("inserted as {n}"),
        [only] => format!("inserted as {n}: {} is frozen", only.name),
        [first, .., last] if frozen_pages.len() == run.len() => {
            format!("inserted as {n}: {}–{} are frozen", first.name, last.name)
        }
        some => format!(
            "inserted as {n}: {} are frozen",
            some.iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    PageInsert {
        name: n,
        renames: Vec::new(),
        note: Some(note),
    }
}

/// `3a` gives (`3`, index after `a`) and `Cover a` gives (`Cover `, index
/// after `a`): one page letter after a number, or after a space that
/// follows a name ending in a letter.
fn letter_suffix(name: &str) -> Option<(String, usize)> {
    let last = name.chars().last()?;
    let head = &name[..name.len() - last.len_utf8()];
    let i = PAGE_LETTERS.find(last)?;
    let after_word = head
        .strip_suffix(' ')
        .is_some_and(|h| h.ends_with(|c: char| c.is_alphabetic()));
    (head.ends_with(|c: char| c.is_ascii_digit()) || after_word).then(|| (head.to_string(), i + 1))
}

/// `2` gives `3`, `A` gives `None`.
fn increment(name: &str) -> Option<String> {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (head, tail) = name.split_at(name.len() - digits);
    let n: u64 = tail.parse().ok()?;
    Some(format!("{head}{}", n + 1))
}

/// Adds a schematic page after `after` (or at the end), with the same paper
/// format and structure (`=A1+CAB1`) as that page, re-lettering a run of
/// lettered pages to make room when needed ([`plan_insert`]). Returns the
/// page and the note for the status line, if any.
pub fn add_page(tx: &mut Tx<'_>, after: Option<PageId>) -> Result<(PageId, Option<String>)> {
    let plan = plan_insert(tx.project(), after, |p| p.frozen.is_some());
    for (page, name) in plan.renames {
        tx.update::<Page>(page, |p| p.name = name)?;
    }
    let template = after
        .or_else(|| tx.project().pages_in_order().last().map(|p| p.id))
        .and_then(|a| tx.project().page(a))
        .map(|p| (p.format, p.structure.clone()));
    let page = tx.add_page(plan.name)?;
    if let Some((format, structure)) = template {
        tx.update::<Page>(page, |p| {
            p.format = format;
            p.kind = PageKind::Schematic;
            p.structure = structure;
        })?;
    }
    Ok((page, plan.note))
}

/// Why `page` cannot be renamed to `name`, if it cannot: a dot, an empty
/// name, a name another page of its structure has, or a frozen page
/// (frozen names never move).
pub fn rename_problem(project: &Project, page: PageId, name: &str) -> Option<String> {
    if let Some(problem) = page_name_problem(name) {
        return Some(problem.into());
    }
    let this = project.page(page)?;
    if let Some(freeze) = &this.frozen
        && this.name != name
    {
        return Some(format!(
            "page {} is frozen (issued in revision {})",
            this.name, freeze.revision
        ));
    }
    project
        .pages()
        .any(|p| p.id != page && p.name == name && p.structure.same_structure(&this.structure))
        .then(|| format!("page {name} already exists"))
}

/// Renames a page, refusing the names [`rename_problem`] names.
pub fn rename_page(tx: &mut Tx<'_>, page: PageId, name: &str) -> std::result::Result<(), String> {
    if let Some(problem) = rename_problem(tx.project(), page, name) {
        return Err(problem);
    }
    tx.update::<Page>(page, |p| p.name = name.to_string())
        .map_err(|e| e.to_string())
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
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "1a");
        assert_eq!(next_page_name(d.project(), Some(ids[3])), "3");
        assert_eq!(next_page_name(d.project(), Some(ids[4])), "6");
        // Without a page: after the last page in order, =A2/5.
        assert_eq!(next_page_name(d.project(), None), "6");
        let (page, _) = d
            .transact("New page", |tx| add_page(tx, Some(ids[1])).map(|(p, _)| p))
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
        // Between 1 and 2 there is no room for a number: a letter, never a
        // dot (`1.1` would read as page 1, column 1).
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "1a");
        let (d, ids) = doc(&["1", "1a", "2"]);
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "1b");
        let (d, ids) = doc(&["3", "3a", "3b", "4"]);
        assert_eq!(next_page_name(d.project(), Some(ids[2])), "3c");
        // Between 3b and 3c: two letters.
        let (d, ids) = doc(&["3", "3a", "3b", "3c"]);
        assert_eq!(next_page_name(d.project(), Some(ids[2])), "3ba");
        let (d, ids) = doc(&["Cover", "1"]);
        assert_eq!(next_page_name(d.project(), Some(ids[0])), "Cover a");
        let (d, ids) = doc(&["Cover", "Cover a", "1"]);
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "Cover b");
        // i, l and o are skipped.
        let (d, ids) = doc(&["5", "5h", "6"]);
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "5j");
        let (d, ids) = doc(&["5", "5k", "6"]);
        assert_eq!(next_page_name(d.project(), Some(ids[1])), "5m");

        // Between 3 and 3a no letter fits: the run moves up one letter and
        // the new page becomes 3a.
        let (d, ids) = doc(&["3", "3a", "3b", "4"]);
        let plan = plan_insert(d.project(), Some(ids[0]), |_| false);
        assert_eq!(plan.name, "3a");
        assert_eq!(
            plan.renames,
            vec![(ids[2], "3c".to_string()), (ids[1], "3b".to_string())]
        );
        assert_eq!(plan.note, None);
        // A frozen page in the run: nothing moves, the page goes at the
        // end of the run, and the status line says why.
        let frozen = |p: &Page| p.id != ids[0];
        let plan = plan_insert(d.project(), Some(ids[0]), frozen);
        assert_eq!(plan.name, "3c");
        assert!(plan.renames.is_empty());
        assert_eq!(
            plan.note.as_deref(),
            Some("inserted as 3c: 3a–3b are frozen")
        );
        let (d, ids) = doc(&["3", "3a", "4"]);
        let plan = plan_insert(d.project(), Some(ids[0]), |p| p.id == ids[1]);
        assert_eq!(plan.name, "3b");
        assert_eq!(plan.note.as_deref(), Some("inserted as 3b: 3a is frozen"));
    }

    #[test]
    fn two_letter_pages_in_a_run_move_with_it() {
        // 3ba comes from inserting after 3b with 3c taken; inserting after
        // 3 then shifts every first letter: 3ba becomes 3ca.
        let (d, ids) = doc(&["3", "3a", "3b", "3ba", "3c"]);
        let plan = plan_insert(d.project(), Some(ids[0]), |_| false);
        assert_eq!(plan.name, "3a");
        assert_eq!(
            plan.renames,
            vec![
                (ids[4], "3d".to_string()),
                (ids[3], "3ca".into()),
                (ids[2], "3c".into()),
                (ids[1], "3b".into()),
            ]
        );
        let (d, ids) = doc(&["3", "3a", "3b", "3ba", "4"]);
        let plan = plan_insert(d.project(), Some(ids[0]), |_| false);
        assert_eq!(plan.name, "3a");
        assert_eq!(plan.renames.len(), 3);
        assert_eq!(plan.note, None);
        // Frozen: nothing moves and nothing panics; only the frozen page
        // is named.
        let plan = plan_insert(d.project(), Some(ids[0]), |p| p.id == ids[3]);
        assert_eq!(plan.name, "3c");
        assert_eq!(plan.note.as_deref(), Some("inserted as 3c: 3ba is frozen"));
        // A run that cannot shift (z is the last letter) is not frozen and
        // says nothing about freezing.
        let (d, ids) = doc(&["3", "3a", "3z"]);
        let plan = plan_insert(d.project(), Some(ids[0]), |_| false);
        assert_eq!(plan.name, "3za");
        assert_eq!(plan.note.as_deref(), Some("inserted as 3za"));
        // Inserting after 3b, just before 3ba: a letter after the anchor.
        let (d, ids) = doc(&["3", "3a", "3b", "3ba", "3c"]);
        let plan = plan_insert(d.project(), Some(ids[2]), |_| false);
        assert_eq!(plan.name, "3ba");
        assert_eq!(plan.renames, vec![(ids[3], "3bb".to_string())]);
    }

    #[test]
    fn every_insert_gets_a_new_dotless_name() {
        let sets: [&[&str]; 6] = [
            &["3", "3a", "3b", "3ba", "3c"],
            &["3", "3a", "3b", "3ba", "4"],
            &["3", "3a", "3z"],
            &["3", "3a", "3aa", "3ab", "3b"],
            &["Cover", "Cover a", "Cover b", "1"],
            &["1", "1a", "1b", "1c", "2", "2a"],
        ];
        for names in sets {
            let (d, ids) = doc(names);
            for &at in &ids {
                for all_frozen in [false, true] {
                    let plan = plan_insert(d.project(), Some(at), |_| all_frozen);
                    // The names after the insert: all distinct, none dotted.
                    let mut after: Vec<String> = ids
                        .iter()
                        .zip(names)
                        .map(|(id, n)| {
                            plan.renames
                                .iter()
                                .find(|(r, _)| r == id)
                                .map_or(n.to_string(), |(_, m)| m.clone())
                        })
                        .collect();
                    after.push(plan.name.clone());
                    assert!(
                        after.iter().all(|n| !n.contains('.')),
                        "{names:?} after {at}"
                    );
                    let count = after.len();
                    after.sort();
                    after.dedup();
                    assert_eq!(after.len(), count, "{names:?} after {at}: {plan:?}");
                }
            }
        }
    }

    #[test]
    fn inserting_before_a_lettered_run_re_letters_it() {
        let (mut d, ids) = doc(&["3", "3a", "3b", "4"]);
        let ((page, note), _) = d
            .transact("New page", |tx| add_page(tx, Some(ids[0])))
            .unwrap();
        assert_eq!(note, None);
        let names: Vec<_> = d
            .project()
            .pages_in_order()
            .iter()
            .map(|p| (p.id, p.name.clone()))
            .collect();
        assert_eq!(
            names,
            vec![
                (ids[0], "3".to_string()),
                (page, "3a".into()),
                (ids[1], "3b".into()),
                (ids[2], "3c".into()),
                (ids[3], "4".into()),
            ]
        );
    }

    #[test]
    fn frozen_pages_keep_their_names() {
        let (mut d, ids) = doc(&["3", "3a", "3b", "4"]);
        d.transact("Issue", |tx| tx.freeze_page(ids[1], "B"))
            .unwrap();
        let ((page, note), _) = d
            .transact("New page", |tx| add_page(tx, Some(ids[0])))
            .unwrap();
        assert_eq!(d.project().page(page).unwrap().name, "3c");
        assert_eq!(d.project().page(ids[1]).unwrap().name, "3a");
        // Only the frozen page is named.
        assert_eq!(note.as_deref(), Some("inserted as 3c: 3a is frozen"));
        assert_eq!(
            rename_problem(d.project(), ids[1], "3x"),
            Some("page 3a is frozen (issued in revision B)".into())
        );
        assert_eq!(rename_problem(d.project(), ids[2], "3x"), None);
    }

    #[test]
    fn a_dot_is_refused_in_page_names() {
        assert_eq!(
            page_name_problem("1.1"),
            Some("a dot is reserved for page.column references")
        );
        assert_eq!(page_name_problem(" "), Some("a page needs a name"));
        assert_eq!(page_name_problem("3a"), None);
        assert_eq!(page_name_problem("Cover a"), None);
        let (mut d, ids) = doc(&["1", "2"]);
        let err = d
            .transact("Rename page", |tx| Ok(rename_page(tx, ids[1], "1.1")))
            .unwrap()
            .0;
        assert_eq!(
            err,
            Err("a dot is reserved for page.column references".to_string())
        );
        assert_eq!(
            rename_problem(d.project(), ids[1], "1"),
            Some("page 1 already exists".into())
        );
        d.transact("Rename page", |tx| Ok(rename_page(tx, ids[1], "1a")))
            .unwrap()
            .0
            .unwrap();
        assert_eq!(d.project().page(ids[1]).unwrap().name, "1a");
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
        let ((page, _), _) = d
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
