//! Generated pages, live (DESIGN.md §2, §8): one `LiveReports` per open
//! project hears every change set the canvas and storage hear, so a
//! report regenerates only when something it reads changed. A generated
//! page shows its report's sheet inside the page frame, drawn the way the
//! PDF prints it (`novo_pdf::sheet_content`).

use novo_canvas::model::SheetContent;
use novo_core::{ChangeSet, PageId, Project};
use novo_reports::{GeneratedSheet, LiveReports};

use super::NovoApp;

/// The open project's live reports and the sheet on the canvas.
pub(crate) struct Reports {
    live: LiveReports,
    /// The project revision the reports last caught up with.
    since: Option<u64>,
    /// Change sets heard since then.
    pending: ChangeSet,
    /// The sheet last handed to the canvas, by page, sheet and revision.
    shown: Option<((PageId, usize, u64), Option<SheetContent>)>,
    /// Which revision each page shows, kept until the revisions change.
    pub(super) page_revisions: Option<(Vec<novo_core::RevisionId>, novo_revisions::PageRevisions)>,
}

impl Default for Reports {
    fn default() -> Self {
        Reports {
            live: LiveReports::standard(),
            since: None,
            pending: ChangeSet::default(),
            shown: None,
            page_revisions: None,
        }
    }
}

impl Reports {
    /// A change set the document went through (an edit, undo or redo).
    pub(crate) fn heard(&mut self, changes: &ChangeSet) {
        self.pending.merge(changes);
    }

    /// The live reports, caught up with `project`.
    pub(crate) fn live(&mut self, project: &Project) -> &mut LiveReports {
        if let Some(since) = self.since
            && !self.pending.is_empty()
        {
            self.live.notify_since(project, since, &self.pending);
        }
        self.pending = ChangeSet::default();
        self.since = Some(project.revision());
        &mut self.live
    }

    /// The sheets of a generated page, in order; empty for any other page.
    pub(crate) fn sheets(&mut self, project: &Project, page: PageId) -> Vec<GeneratedSheet> {
        self.live(project).sheets_of_page(project, page)
    }

    /// What the canvas draws inside the frame of `page`: sheet `index` of
    /// its report, or `None` for a page that is not generated.
    pub(crate) fn content(
        &mut self,
        project: &Project,
        page: PageId,
        index: usize,
    ) -> Option<SheetContent> {
        let key = (page, index, project.revision());
        if let Some((at, content)) = &self.shown
            && *at == key
        {
            return content.clone();
        }
        let content = self.render(project, page, index);
        self.shown = Some((key, content.clone()));
        content
    }

    fn render(&mut self, project: &Project, page: PageId, index: usize) -> Option<SheetContent> {
        let generated = matches!(
            project.page(page)?.kind,
            novo_core::PageKind::Generated { .. }
        );
        if !generated {
            return None;
        }
        let sheets = self.sheets(project, page);
        // A report with nothing to list yet, or not registered, leaves the
        // frame empty, as it prints.
        let last = sheets.len().checked_sub(1)?;
        let index = index.min(last);
        let sheet = &sheets[index];
        let layout = self.live.layout_sheet(project, sheet).unwrap_or_default();
        Some(novo_pdf::sheet_content(
            &sheet.name,
            (index + 1, sheets.len()),
            &layout,
        ))
    }
}

impl NovoApp {
    /// Adds the generated pages the project should have and lacks (DESIGN.md
    /// §2: automatic): the contents first, the reports after the drawing
    /// pages, each once it has something to list. Runs on new and open, as
    /// one undo step (the project's first, so one undo takes them all back). A kind the user turned off (deleting a generated page
    /// does, in `ProjectRules::generated_pages_off`) is not added again.
    pub(super) fn add_generated_pages(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let project = doc.project();
        // An issued project is not changed by opening it: its frozen pages
        // are a record, and the file is not rewritten behind the user.
        if project.pages().any(|p| p.frozen.is_some()) {
            if !novo_reports::default_generated_pages(project).is_empty() {
                self.message(
                    super::Level::Info,
                    "Generated pages not added: the project has frozen pages",
                );
            }
            return;
        }
        let rules = &project.info().rules;
        let specs =
            novo_reports::default_generated_pages_with(project, &|id| rules.wants_generated(id));
        // The contents comes with the first report that has something to
        // list, so a new, empty project stays a single drawing page.
        if specs
            .iter()
            .all(|s| s.generator == novo_reports::TableOfContents::ID)
        {
            return;
        }
        match doc.transact("Generated pages", |tx| {
            novo_reports::add_generated_pages(tx, &specs)
        }) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(
                super::Level::Error,
                format!("Could not add the generated pages: {e}"),
            ),
        }
    }

    /// Tells the live reports about a change set the document went through.
    pub(super) fn reports_heard(&mut self, changes: &ChangeSet) {
        self.wb.reports.heard(changes);
    }

    /// Hands the shown page's sheet to the canvas before it draws: a
    /// generated page's report, or nothing for a schematic page.
    pub(super) fn update_sheet(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            return;
        };
        let content = self.wb.reports.content(doc.project(), page, self.wb.sheet);
        if let Some(view) = self.canvas.schematic() {
            view.set_sheet(content);
        }
        self.update_title_revisions();
    }
}
