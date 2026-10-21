//! Generated pages as printed sheets.
//!
//! A generated page is a [`PageKind::Generated`] page record: it has a
//! name, a place in the page order, a format and a structure like any
//! page, and its report decides the content. A report longer than one
//! sheet prints as several sheets under that one record:
//!
//! - one sheet: the page's own name (`20`);
//! - several: `20a`, `20b`, ... `20z`, then `20aa`, `20ab`, ... in order,
//!   like spreadsheet columns but without `i`, `l` and `o`, which misread
//!   as `1` and `0` in small type (`20h`, `20j`, `20k`, `20m`, `20n`, `20p`).
//!   Never a dot, so a sheet name cannot read as a `page.column` cross
//!   reference (`3.4`) (DESIGN.md §8).
//!
//! A page whose name a sheet would take (`20b`, inserted or named so by
//! the user) keeps its name, and the clashing sheet gets `'` marks
//! (`20b'`), so printed names stay distinct.
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
use crate::table::ReportOutput;

/// The 23 letters sheet names use: no `i`, `l` or `o`.
const ALPHABET: &[u8; 23] = b"abcdefghjkmnpqrstuvwxyz";

/// The letters of sheet `k` (1-based), like a spreadsheet column over
/// [`ALPHABET`]: `a` for 1, `z` for 23, `aa` for 24, `ab` for 25, ...
fn letters(mut k: usize) -> String {
    let mut out = Vec::new();
    while k > 0 {
        k -= 1;
        out.push(ALPHABET[k % ALPHABET.len()]);
        k /= ALPHABET.len();
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII letters")
}

/// The plain name of sheet `index` (0-based) of `count` sheets of a
/// generated page named `page_name` (`20`, or `20a`, `20b`, ...), ignoring
/// other pages. Printed sheets use [`sheet_names`], which also keeps them
/// apart from a page the user named like a sheet.
pub fn sheet_name(page_name: &str, index: usize, count: usize) -> String {
    if count <= 1 {
        page_name.to_owned()
    } else {
        format!("{page_name}{}", letters(index + 1))
    }
}

/// The names of the `count` sheets of a generated page named `page_name`
/// (see the module docs): `20a`, `20b`, ..., with `'` marks added to any
/// name `taken` reports as another page's. Always `count` distinct names,
/// none taken.
pub fn sheet_names(page_name: &str, count: usize, taken: impl Fn(&str) -> bool) -> Vec<String> {
    (0..count)
        .map(|i| {
            let mut name = sheet_name(page_name, i, count);
            // A single sheet is the page's own name, never another's.
            while count > 1 && taken(&name) {
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
    /// `20`, `20c`, or `20c'` when a page is already named `20c` (see
    /// [`sheet_names`]).
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
        let Some(output) = self.get(project, generator) else {
            return Vec::new();
        };
        sheets_for(project, page, generator, output)
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
        let setup = PageSetup::for_format(format);
        let output = self.get(project, &sheet.report)?;
        // The single-line diagram prints as the drawing; a schedule longer
        // than the sheet continues as the plain table.
        if sheet.report == crate::SingleLineDiagram::ID && sheet.sheet.rows.start == 0 {
            return crate::single_line::sheet_layout(project, sheet.sheet.section, &setup);
        }
        // Terminal diagrams print as drawn strips, not as tables.
        if sheet.report == crate::TerminalDiagram::ID
            && let Some(section) = output.sections.get(sheet.sheet.section)
            && let Some(strip) = project.devices().find(|d| {
                d.kind == novo_core::DeviceKind::TerminalStrip && d.tag.to_string() == section.key
            })
        {
            let count = output
                .sections
                .get(sheet.sheet.section)
                .map_or(1, |s| s.rows.len().div_ceil(setup.rows_per_page()).max(1));
            let index = sheet.sheet.rows.start / setup.rows_per_page().max(1);
            return Some(crate::strip_diagram::strip_sheet(
                project,
                strip.id,
                sheet.sheet.rows.clone(),
                (index, count),
                &setup,
            ));
        }
        Some(layout_page(output, &sheet.sheet, &setup))
    }
}

/// What a sheet shows, boiled down for revisions: which sheet of which
/// page, and a hash of its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetFingerprint {
    pub page: PageId,
    /// The sheet's printed name (`20b`).
    pub name: String,
    /// The report that fills it.
    pub report: String,
    /// A hash of the report title, the section title, the column headers
    /// and the sheet's rows (every cell as printed). It leaves out the
    /// sheet name, "page n / m" and "continued", so renaming or
    /// renumbering a sheet does not change it, while any change to what it
    /// lists does. Stable across runs and platforms (FNV-1a over UTF-8),
    /// so it can be stored in a revision and compared later.
    pub hash: u64,
}

impl LiveReports {
    /// A fingerprint for every sheet of every generated page, in page
    /// order. Compare two states of a project sheet by sheet (by page and
    /// position, or by `name`) to find the generated sheets whose content
    /// changed (DESIGN.md §8).
    pub fn sheet_fingerprints(&mut self, project: &Project) -> Vec<SheetFingerprint> {
        let sheets = self.generated_sheets(project);
        sheets
            .into_iter()
            .filter_map(|sheet| {
                let output = self.get(project, &sheet.report)?;
                Some(SheetFingerprint {
                    page: sheet.page,
                    name: sheet.name,
                    report: sheet.report,
                    hash: fingerprint(output, &sheet.sheet),
                })
            })
            .collect()
    }
}

/// The hash of what `page` of `output` shows; see [`SheetFingerprint::hash`].
pub fn fingerprint(output: &ReportOutput, page: &ReportPage) -> u64 {
    // FNV-1a, with a 0xFF byte between fields so "ab","c" differs from
    // "a","bc" (0xFF never occurs in UTF-8).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |text: &str| {
        for byte in text.bytes().chain(std::iter::once(0xFF)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    put(&output.title);
    if let Some(section) = output.sections.get(page.section) {
        put(&section.title);
        for column in &section.columns {
            put(&column.header);
        }
        for row in section.rows.get(page.rows.clone()).unwrap_or_default() {
            for cell in &row.cells {
                put(&cell.to_string());
            }
            put("");
        }
    }
    hash
}

/// The sheets `output` fills on generated `page`, named by [`sheet_names`]
/// with every other page's name taken.
pub(crate) fn sheets_for(
    project: &Project,
    page: &Page,
    generator: &str,
    output: &ReportOutput,
) -> Vec<GeneratedSheet> {
    let sheets = paginate(output, &PageSetup::for_format(page.format));
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
            report: generator.to_owned(),
            name,
            sheet,
        })
        .collect()
}
