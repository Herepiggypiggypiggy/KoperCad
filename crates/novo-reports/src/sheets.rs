//! Generated pages as printed sheets.
//!
//! A generated page is a [`PageKind::Generated`] page record: it has a
//! name, a place in the page order, a format and a structure like any
//! page, and its report decides the content. A report longer than one
//! sheet prints as several sheets under that one record:
//!
//! - one sheet: the page's own name (`20`);
//! - several: `20a`, `20b`, ... `20z`, then `20aa`, `20ab`, ... in order,
//!   like spreadsheet columns (DESIGN.md §8). Never a dot, so a sheet name
//!   cannot read as a `page.column` cross reference (`3.4`).
//!
//! A sheet name of a multi-sheet page never repeats another page's name
//! (a single sheet is the page's own name, so it is unique when page names
//! are). When a real page is already called `20a` (or `20b`, ...), the
//! generated page's sheets take the next form that is free for all of
//! them: `20-a`, `20-b`, ..., then `20/a`, then `20_a`. The form is chosen
//! per generated page, so its sheets always read as one family; if every
//! form is taken, each sheet still gets a free name by adding `'` marks
//! (`20a'`). The real page keeps its name: the user named it, the report
//! did not. Because the choice is made for the family as a whole, it can
//! change as the report grows: a 2-sheet `20a, 20b` that grows to 3 next
//! to a real `20c` becomes `20-a .. 20-c`. That is fine under DESIGN.md's
//! rule for generated pages (they reflow, and nothing references a sheet's
//! name).
//!
//! Each sheet is laid out with [`PageSetup::for_format`] of the page's
//! format, so it fits the same frame and title block as the schematic
//! pages. The canvas (WS-03) and the PDF export (WS-09) draw a sheet's
//! [`PageLayout`](crate::PageLayout) inside their frame and print the
//! sheet name where they print the page name.

use std::collections::BTreeSet;

use novo_core::{Page, PageId, PageKind, Project};

use crate::page::{PageLayout, PageSetup, ReportPage, layout_page, paginate};
use crate::report::LiveReports;

/// The letters of sheet `k` (1-based), like a spreadsheet column: `a` for
/// 1, `z` for 26, `aa` for 27, `ab` for 28, ...
fn letters(mut k: usize) -> String {
    let mut out = Vec::new();
    while k > 0 {
        k -= 1;
        out.push(b'a' + (k % 26) as u8);
        k /= 26;
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII letters")
}

/// The plain name of sheet `index` (0-based) of `count` sheets of a
/// generated page named `page_name` (`20`, or `20a`, `20b`, ...), ignoring
/// other pages. Printed sheets use [`sheet_names`], which steps around
/// names other pages already have.
pub fn sheet_name(page_name: &str, index: usize, count: usize) -> String {
    if count <= 1 {
        page_name.to_owned()
    } else {
        format!("{page_name}{}", letters(index + 1))
    }
}

/// Separators between the page name and the sheet letters, tried in order.
const SEPARATORS: [&str; 4] = ["", "-", "/", "_"];

/// The names of the `count` sheets of a generated page named `page_name`,
/// avoiding every name `taken` reports as used by another page (see the
/// module docs for the rule). Always `count` distinct names, none taken.
pub fn sheet_names(page_name: &str, count: usize, taken: impl Fn(&str) -> bool) -> Vec<String> {
    if count <= 1 {
        // The page's own name: it is already the page's, not another's.
        return (0..count).map(|_| page_name.to_owned()).collect();
    }
    let with = |sep: &str| -> Vec<String> {
        (1..=count)
            .map(|k| format!("{page_name}{sep}{}", letters(k)))
            .collect()
    };
    if let Some(names) = SEPARATORS
        .iter()
        .map(|sep| with(sep))
        .find(|names| names.iter().all(|n| !taken(n)))
    {
        return names;
    }
    with(SEPARATORS[0])
        .into_iter()
        .map(|mut name| {
            while taken(&name) {
                name.push('\'');
            }
            name
        })
        .collect()
}

/// One printed sheet of a generated page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSheet {
    /// The generated page record.
    pub page: PageId,
    /// The report that fills it.
    pub report: String,
    /// `20`, `20c`, or when a real page holds such a name, `20-c`, `20/c`,
    /// `20_c` or, last resort, `20c'` (see [`sheet_names`]).
    pub name: String,
    /// Which rows of which section the sheet holds.
    pub sheet: ReportPage,
}

impl LiveReports {
    /// The sheets of one generated page, in order. Empty for a page that is
    /// not generated or whose report is not registered (it prints as an
    /// empty frame, never as an error).
    pub fn sheets_of_page(&mut self, project: &Project, page: PageId) -> Vec<GeneratedSheet> {
        let Some(page) = project.page(page) else {
            return Vec::new();
        };
        let PageKind::Generated { generator } = &page.kind else {
            return Vec::new();
        };
        let setup = PageSetup::for_format(page.format);
        let Some(output) = self.get(project, generator) else {
            return Vec::new();
        };
        let sheets = paginate(output, &setup);
        // Every other page's name is taken, generated pages' own names too.
        let others: BTreeSet<&str> = project
            .pages()
            .filter(|p| p.id != page.id)
            .map(|p| p.name.as_str())
            .collect();
        let names = sheet_names(&page.name, sheets.len(), |n| others.contains(n));
        sheets
            .into_iter()
            .zip(names)
            .map(|(sheet, name)| GeneratedSheet {
                page: page.id,
                report: generator.clone(),
                name,
                sheet,
            })
            .collect()
    }

    /// Every sheet of every generated page, in page order.
    pub fn generated_sheets(&mut self, project: &Project) -> Vec<GeneratedSheet> {
        let pages: Vec<PageId> = project
            .pages_in_order()
            .into_iter()
            .filter(|p| matches!(p.kind, PageKind::Generated { .. }))
            .map(|p: &Page| p.id)
            .collect();
        pages
            .into_iter()
            .flat_map(|page| self.sheets_of_page(project, page))
            .collect()
    }

    /// The drawing of one sheet, ready for the canvas or PDF.
    pub fn layout_sheet(
        &mut self,
        project: &Project,
        sheet: &GeneratedSheet,
    ) -> Option<PageLayout> {
        let format = project.page(sheet.page)?.format;
        let output = self.get(project, &sheet.report)?;
        Some(layout_page(
            output,
            &sheet.sheet,
            &PageSetup::for_format(format),
        ))
    }
}
