//! Table of contents: every page in reading order, each sheet of a
//! generated page on its own row.

use novo_core::{
    DeviceTag, EntityKind, Page, PageKind, Project, contents_scope, panel_contents_generator,
};

use crate::report::{Dependencies, Report, Reports};
use crate::sheets::sheets_for;
use crate::table::{Cell, Column, ReportOutput, Section};

/// The table of contents: pages in [`Project::pages_in_order`] order with
/// their titles, structure and kind. A generated page lists each of its
/// sheets (`20a`, `20b`, ...) with what the sheet shows (`Terminals ·
/// =A1+CAB-X1`), as the PDF bookmarks do (DESIGN.md §8). A generated page
/// of an unknown report, and the contents page itself, are listed as one
/// page. "Known" differs by path: inside [`LiveReports`](crate::LiveReports)
/// it is every registered report, while a standalone
/// [`Report::generate`] only knows [`crate::standard_reports`], so a
/// report registered on top of those is listed per sheet live and as one
/// page standalone.
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
        let outputs = standalone_outputs(project);
        let lookup = |id: &str| outputs.iter().find(|o| o.report == id);
        self.generate_with(project, &lookup)
    }

    fn reads_reports(&self) -> bool {
        true
    }

    fn generate_with(&self, project: &Project, reports: Reports<'_>) -> ReportOutput {
        contents(project, reports, Self::ID, self.title(), None)
    }
}

/// The output of each built-in report a generated page shows, built once:
/// what a contents list reads when it is generated on its own.
fn standalone_outputs(project: &Project) -> Vec<ReportOutput> {
    let reports = crate::standard_reports();
    let mut outputs: Vec<ReportOutput> = Vec::new();
    for page in project.pages_in_order() {
        if let PageKind::Generated { generator } = &page.kind
            && generator != TableOfContents::ID
            && !outputs.iter().any(|o| o.report == *generator)
            && let Some(report) = reports.iter().find(|r| r.id() == generator)
        {
            outputs.push(report.generate(project));
        }
    }
    outputs
}

/// The contents list of one panel (`toc@+BAY1G1`): the pages of that panel
/// only, in the same order and with the same columns as the project's
/// [`TableOfContents`]. Pages of the panel under several page groups are
/// all listed. It is created for a generated page whose generator id names
/// the panel (see [`panel_contents_generator`]), which
/// [`LiveReports`](crate::LiveReports) registers when the page is first
/// read.
#[derive(Debug, Clone)]
pub struct PanelContents {
    scope: DeviceTag,
    id: String,
}

impl PanelContents {
    /// The list for `panel`, a tag holding location levels only. `None`
    /// when it holds none.
    pub fn new(panel: &DeviceTag) -> Option<Self> {
        if panel.location.is_empty() {
            return None;
        }
        let scope = DeviceTag {
            location: panel.location.clone(),
            ..Default::default()
        };
        Some(PanelContents {
            id: panel_contents_generator(&scope.location),
            scope,
        })
    }

    /// The list a generator id names, if it names a panel.
    pub fn of_generator(generator: &str) -> Option<Self> {
        Self::new(&contents_scope(generator)?)
    }

    /// The panel it lists.
    pub fn panel(&self) -> &DeviceTag {
        &self.scope
    }
}

impl Report for PanelContents {
    fn id(&self) -> &str {
        &self.id
    }

    fn title(&self) -> String {
        format!("Table of contents {}", self.scope)
    }

    fn dependencies(&self) -> Dependencies {
        TableOfContents.dependencies()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let outputs = standalone_outputs(project);
        let lookup = |id: &str| outputs.iter().find(|o| o.report == id);
        self.generate_with(project, &lookup)
    }

    fn reads_reports(&self) -> bool {
        true
    }

    fn generate_with(&self, project: &Project, reports: Reports<'_>) -> ReportOutput {
        contents(project, reports, &self.id, self.title(), Some(&self.scope))
    }
}

/// The rows of a contents list: every page, or with a `scope` only the
/// pages of that panel, each sheet of a generated page on its own row. The
/// project's list also gets the panels table.
fn contents(
    project: &Project,
    reports: Reports<'_>,
    id: &str,
    title: String,
    scope: Option<&DeviceTag>,
) -> ReportOutput {
    let mut section = Section::new(
        "pages",
        "",
        vec![
            Column::new("page", "Page", 15.0),
            Column::new("title", "Title", 60.0),
            Column::new("group", "Page group", 20.0),
            Column::new("panel", "Panel", 20.0),
            Column::new("kind", "Kind", 25.0),
        ],
    );
    for page in project.pages_in_order() {
        if scope.is_some_and(|s| page.structure.location != s.location) {
            continue;
        }
        if let PageKind::Generated { generator } = &page.kind
            && generator != TableOfContents::ID
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
                            Cell::text(group_of(page)),
                            Cell::text(panel_of(page)),
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
                Cell::text(if page.title.is_empty() {
                    "(untitled)".to_owned()
                } else {
                    page.title.clone()
                }),
                Cell::text(group_of(page)),
                Cell::text(panel_of(page)),
                Cell::text(kind),
            ],
            vec![page.id.into()],
        );
    }
    let mut sections = vec![section];
    let panels = if scope.is_none() {
        project.panels()
    } else {
        Vec::new()
    };
    if !panels.is_empty() {
        // Pages of one panel under several page groups are one layout.
        let mut by_panel = Section::new(
            "panels",
            "Panels",
            vec![
                Column::new("panel", "Panel", 25.0),
                Column::new("groups", "Page groups", 50.0),
                Column::new("pages", "Pages", 15.0),
            ],
        );
        for panel in panels {
            let groups: Vec<String> = panel
                .groups
                .iter()
                .filter(|g| !g.is_empty())
                .map(|g| g.to_string())
                .collect();
            by_panel.push(
                vec![
                    Cell::text(panel.location.to_string()),
                    Cell::text(groups.join(", ")),
                    Cell::text(panel.pages.len().to_string()),
                ],
                panel.pages.iter().map(|&p| p.into()).collect(),
            );
        }
        sections.push(by_panel);
    }
    ReportOutput {
        report: id.into(),
        title,
        revision: project.revision(),
        sections,
    }
}

/// The page group of a page (`=20D41`), empty for a page in none.
fn group_of(page: &Page) -> String {
    DeviceTag {
        function: page.structure.function.clone(),
        ..Default::default()
    }
    .to_string()
}

/// The panel of a page (`+BAY1G1`), empty for a page in none.
fn panel_of(page: &Page) -> String {
    DeviceTag {
        location: page.structure.location.clone(),
        ..Default::default()
    }
    .to_string()
}
