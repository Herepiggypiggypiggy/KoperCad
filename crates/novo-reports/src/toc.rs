//! Table of contents: every page in reading order, each sheet of a
//! generated page on its own row.

use novo_core::{EntityKind, PageKind, Project};

use crate::report::{Dependencies, Report, Reports};
use crate::sheets::sheets_for;
use crate::table::{Cell, Column, ReportOutput, Section};

/// The table of contents: pages in [`Project::pages_in_order`] order with
/// their titles, structure and kind. A generated page lists each of its
/// sheets (`20a`, `20b`, ...) with what the sheet shows (`Terminals ·
/// =A1+CAB-X1`), as the PDF bookmarks do (DESIGN.md §8). A generated page
/// of a report this crate does not know, and the contents page itself, are
/// listed as one page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TableOfContents;

impl TableOfContents {
    pub const ID: &'static str = "toc";
}

impl Report for TableOfContents {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Table of contents".into()
    }

    fn dependencies(&self) -> Dependencies {
        // Its pages, and the sheets that follow every other report's length.
        let mut deps = Dependencies::on(&[EntityKind::Page]);
        for report in crate::standard_reports() {
            if report.id() != Self::ID {
                let other = report.dependencies();
                deps.kinds.extend(other.kinds);
                deps.info |= other.info;
            }
        }
        deps
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        // Standalone: build each report a generated page shows, once.
        let reports = crate::standard_reports();
        let mut outputs: Vec<ReportOutput> = Vec::new();
        for page in project.pages_in_order() {
            if let PageKind::Generated { generator } = &page.kind
                && generator != Self::ID
                && !outputs.iter().any(|o| o.report == *generator)
                && let Some(report) = reports.iter().find(|r| r.id() == generator)
            {
                outputs.push(report.generate(project));
            }
        }
        let lookup = |id: &str| outputs.iter().find(|o| o.report == id);
        self.generate_with(project, &lookup)
    }

    fn reads_reports(&self) -> bool {
        true
    }

    fn generate_with(&self, project: &Project, reports: Reports<'_>) -> ReportOutput {
        let mut section = Section::new(
            "pages",
            "",
            vec![
                Column::new("page", "Page", 15.0),
                Column::new("title", "Title", 60.0),
                Column::new("structure", "Structure", 25.0),
                Column::new("kind", "Kind", 25.0),
            ],
        );
        for page in project.pages_in_order() {
            if let PageKind::Generated { generator } = &page.kind
                && generator != Self::ID
            {
                let output = reports(generator);
                if let Some(output) = output {
                    let sheets = sheets_for(project, page, generator, output);
                    let kind = format!("Generated: {generator}");
                    for sheet in &sheets {
                        let shows = output
                            .sections
                            .get(sheet.sheet.section)
                            .map_or("", |s| s.title.as_str());
                        let title: Vec<&str> = [page.title.as_str(), shows]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect();
                        section.push(
                            vec![
                                Cell::text(sheet.name.clone()),
                                Cell::text(title.join(" · ")),
                                Cell::text(page.structure.to_string()),
                                Cell::text(kind.clone()),
                            ],
                            vec![page.id.into()],
                        );
                    }
                    if !sheets.is_empty() {
                        continue;
                    }
                }
            }
            let kind = match &page.kind {
                PageKind::Schematic => "Schematic".to_owned(),
                PageKind::Overview => "Overview".to_owned(),
                PageKind::Generated { generator } => format!("Generated: {generator}"),
            };
            section.push(
                vec![
                    Cell::text(page.name.clone()),
                    Cell::text(page.title.clone()),
                    Cell::text(page.structure.to_string()),
                    Cell::text(kind),
                ],
                vec![page.id.into()],
            );
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![section],
        }
    }
}
