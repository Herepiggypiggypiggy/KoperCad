//! The revision fields of the title block (docs/DESIGN.md §7): each page
//! shows the latest revision in which it changed, and the set's revision.

use super::NovoApp;

impl NovoApp {
    /// Hands the shown page's title block its revision fields before the
    /// canvas draws. The per-page walk reads every kept snapshot, so it
    /// runs again only when the list of revisions changes.
    pub(super) fn update_title_revisions(&mut self) {
        let (Some(page), Some(doc)) = (self.current_page, self.backend.document()) else {
            return;
        };
        let project = doc.project();
        let ids: Vec<_> = project.revisions().map(|r| r.id).collect();
        let cache = &mut self.wb.reports.page_revisions;
        if cache.as_ref().map(|(known, _)| known) != Some(&ids) {
            let backend = &self.backend;
            let computed =
                novo_revisions::PageRevisions::compute(project, |id| backend.revision_snapshot(id));
            *cache = Some((ids, computed));
        }
        let revisions = cache.as_ref().map(|(_, r)| r);
        let fields = project
            .latest_revision()
            .map(|latest| novo_canvas::model::TitleRevisions {
                page: revisions
                    .and_then(|r| r.of(project, page))
                    .map(|r| (r.index.clone(), r.date.clone())),
                project: Some(latest.index.clone()),
            });
        if let Some(view) = self.canvas.schematic() {
            view.set_revisions(fields);
        }
    }
}
