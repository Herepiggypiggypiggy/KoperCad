//! Table of contents: every page in reading order.

use novo_core::{EntityKind, PageKind, Project};

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The table of contents: pages in [`Project::pages_in_order`] order with
/// their titles, structure and kind. Generated pages are listed like any
/// other page, under the report that owns them.
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
        Dependencies::on(&[EntityKind::Page])
    }

    fn generate(&self, project: &Project) -> ReportOutput {
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
