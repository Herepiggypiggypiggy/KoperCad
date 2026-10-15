//! The revision fields of the title block (docs/DESIGN.md §7): each page
//! shows the latest revision in which it changed, and the set's revision.

use std::collections::{BTreeMap, BTreeSet};

use novo_canvas::model::{RevisionRow, TitleRevisions};
use novo_core::PageId;

use super::NovoApp;

impl NovoApp {
    /// Brings the per-page revisions up to date. The walk reads every kept
    /// snapshot, so it runs again only when the list of revisions changes.
    fn ensure_page_revisions(&mut self) {
        let Some(doc) = self.backend.document() else {
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
    }

    /// The title block fields of `page`: its own revision and the set's;
    /// `None` before the first revision.
    fn title_revisions_of(&self, page: PageId) -> Option<TitleRevisions> {
        let project = self.backend.document()?.project();
        let latest = project.latest_revision()?;
        let revisions = self.wb.reports.page_revisions.as_ref().map(|(_, r)| r);
        Some(TitleRevisions {
            page: revisions
                .and_then(|r| r.of(project, page))
                .map(|r| (r.index.clone(), r.date.clone())),
            project: Some(latest.index.clone()),
            table: project
                .revisions()
                .map(|r| RevisionRow {
                    index: r.index.clone(),
                    date: r.date.clone(),
                    author: r.author.clone(),
                    description: r.description.clone(),
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        })
    }

    /// Hands the shown page's title block its revision fields before the
    /// canvas draws.
    pub(super) fn update_title_revisions(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        self.ensure_page_revisions();
        let fields = self.title_revisions_of(page);
        if let Some(view) = self.canvas.schematic() {
            view.set_revisions(fields);
        }
    }

    /// The title block fields of every page, for the PDF.
    pub(super) fn all_title_revisions(&mut self) -> BTreeMap<PageId, TitleRevisions> {
        self.ensure_page_revisions();
        let Some(doc) = self.backend.document() else {
            return BTreeMap::new();
        };
        let pages: Vec<PageId> = doc.project().pages().map(|p| p.id).collect();
        pages
            .into_iter()
            .filter_map(|page| Some((page, self.title_revisions_of(page)?)))
            .collect()
    }

    /// The pages with changes since the latest revision, for the PDF
    /// bookmarks. Empty before the first revision or without its kept copy.
    pub(super) fn pages_changed_since_latest(&self) -> BTreeSet<PageId> {
        let Some(doc) = self.backend.document() else {
            return BTreeSet::new();
        };
        let project = doc.project();
        let Some(latest) = project.latest_revision() else {
            return BTreeSet::new();
        };
        let Some(snapshot) = self.backend.revision_snapshot(latest.id) else {
            return BTreeSet::new();
        };
        let set = novo_revisions::diff(&snapshot, project);
        set.pages
            .keys()
            .copied()
            .filter(|page| project.page(*page).is_some())
            .collect()
    }
}
