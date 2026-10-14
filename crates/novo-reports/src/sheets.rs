//! Generated pages as printed sheets.
//!
//! A generated page is a [`PageKind::Generated`] page record: it has a
//! name, a place in the page order, a format and a structure like any
//! page, and its report decides the content. A report longer than one
//! sheet prints as several sheets under that one record:
//!
//! - one sheet: the page's own name (`20`);
//! - several: `20.1`, `20.2`, ... in order. Page names with a dot (`3.1`)
//!   are already the model's convention for sub-pages, and they sort
//!   naturally between `20` and `21`.
//!
//! Each sheet is laid out with [`PageSetup::for_format`] of the page's
//! format, so it fits the same frame and title block as the schematic
//! pages. The canvas (WS-03) and the PDF export (WS-09) draw a sheet's
//! [`PageLayout`](crate::PageLayout) inside their frame and print the
//! sheet name where they print the page name.

use novo_core::{Page, PageId, PageKind, Project};

use crate::page::{PageLayout, PageSetup, ReportPage, layout_page, paginate};
use crate::report::LiveReports;

/// The name of sheet `index` (0-based) of `count` sheets of a generated
/// page named `page_name`.
pub fn sheet_name(page_name: &str, index: usize, count: usize) -> String {
    if count <= 1 {
        page_name.to_owned()
    } else {
        format!("{page_name}.{}", index + 1)
    }
}

/// One printed sheet of a generated page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSheet {
    /// The generated page record.
    pub page: PageId,
    /// The report that fills it.
    pub report: String,
    /// `20` or `20.3`.
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
        let count = sheets.len();
        sheets
            .into_iter()
            .enumerate()
            .map(|(i, sheet)| GeneratedSheet {
                page: page.id,
                report: generator.clone(),
                name: sheet_name(&page.name, i, count),
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
