//! The generated-page framework: the [`Report`] trait, change tracking and
//! the live cache that keeps every report current.

use std::collections::BTreeSet;

use novo_core::{ChangeSet, EntityKind, Page, PageKind, Project};

use crate::table::ReportOutput;

/// What a report reads from the model, so an edit that touches none of it
/// skips the report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dependencies {
    /// Kinds of records the report reads.
    pub kinds: BTreeSet<EntityKind>,
    /// Whether it reads the project info (name, rules).
    pub info: bool,
}

impl Dependencies {
    pub fn on(kinds: &[EntityKind]) -> Self {
        Dependencies {
            kinds: kinds.iter().copied().collect(),
            info: false,
        }
    }

    /// The same dependencies plus the project info.
    pub fn with_info(mut self) -> Self {
        self.info = true;
        self
    }

    /// Everything: for reports that cannot say what they read.
    pub fn everything() -> Self {
        use EntityKind::*;
        Dependencies::on(&[
            Page, Device, Symbol, Junction, Wire, Potential, Terminal, Cable, Part, Text,
        ])
        .with_info()
    }

    /// Whether an edit may change a report with these dependencies. One pass
    /// over the change set: cost is the size of the edit, not the project.
    pub fn affected_by(&self, changes: &ChangeSet) -> bool {
        (self.info && changes.info)
            || changes
                .added
                .iter()
                .chain(&changes.removed)
                .chain(&changes.modified)
                .any(|id| self.kinds.contains(&id.kind()))
    }
}

/// A report: a pure function from the project to tables.
///
/// This is the seam other workstreams plug their generated pages into
/// (terminal diagrams from WS-11, cable diagrams from WS-12): implement it,
/// register the report with [`LiveReports::register`], and the report gets
/// paging, page layout, change tracking, Excel and CSV export for free.
///
/// Rules for an implementation:
/// - `generate` reads the project only and returns the same output for the
///   same project: no clocks, no hash-map iteration order, no randomness.
///   Sort rows explicitly (natural order: [`novo_core::natural_cmp`],
///   [`crate::tag_cmp`]).
/// - `dependencies` names every record kind `generate` reads, directly or
///   through queries (the netlist reads symbols, wires, junctions and
///   potentials, plus the project rules). Too many only costs time; too few
///   leaves a stale page, so when unsure use [`Dependencies::everything`].
/// - One [`Section`](crate::Section) per independent table (one per terminal
///   strip); each starts on a new page.
pub trait Report: Send + Sync {
    /// Stable id: the `generator` of a [`PageKind::Generated`] page, the
    /// spreadsheet's sheet key and the command-field noun (`bom`, `wires`).
    fn id(&self) -> &str;
    /// Human title (`Bill of materials`).
    fn title(&self) -> String;
    /// What the report reads; see the trait docs.
    fn dependencies(&self) -> Dependencies;
    /// Builds the report's tables from the project.
    fn generate(&self, project: &Project) -> ReportOutput;

    /// Whether an edit may change this report. The default reads
    /// [`Report::dependencies`]; override for a finer test.
    fn affected_by(&self, changes: &ChangeSet) -> bool {
        self.dependencies().affected_by(changes)
    }
}

struct Entry {
    report: Box<dyn Report>,
    /// The output and the project revision it is known to match.
    output: Option<(u64, ReportOutput)>,
}

/// Every report of one document, kept current: the live views behind
/// generated pages. There is no "update reports" step: [`LiveReports::get`]
/// always returns output matching the project it is given.
///
/// Feed it each edit's [`ChangeSet`] with [`LiveReports::notify`] (the app
/// does this where it hands change sets to storage and the canvas; undo and
/// redo too). An edit then costs one pass over the change set per report,
/// and only the reports it affects are regenerated, lazily, on the next
/// `get`. A missed notification is safe: an untouched report rolls forward
/// only from the revision just before the edit, so after a gap `get` sees
/// the revision mismatch and regenerates.
///
/// One `LiveReports` belongs to one [`Document`](novo_core::Document):
/// output is matched by revision number, so call [`LiveReports::clear`]
/// (or make a new one) when another project is opened.
pub struct LiveReports {
    entries: Vec<Entry>,
}

impl Default for LiveReports {
    fn default() -> Self {
        LiveReports::standard()
    }
}

impl LiveReports {
    /// No reports.
    pub fn empty() -> Self {
        LiveReports {
            entries: Vec::new(),
        }
    }

    /// The built-in reports: table of contents, bill of materials, device
    /// list and wire list.
    pub fn standard() -> Self {
        let mut reports = LiveReports::empty();
        for report in crate::standard_reports() {
            reports.register(report);
        }
        reports
    }

    /// Adds a report, replacing one with the same id.
    pub fn register(&mut self, report: Box<dyn Report>) {
        let entry = Entry {
            report,
            output: None,
        };
        match self
            .entries
            .iter()
            .position(|e| e.report.id() == entry.report.id())
        {
            Some(i) => self.entries[i] = entry,
            None => self.entries.push(entry),
        }
    }

    /// Drops every cached output (another project was opened).
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.output = None;
        }
    }

    /// Report ids in registration order.
    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.report.id()).collect()
    }

    /// The report with this id.
    pub fn report(&self, id: &str) -> Option<&dyn Report> {
        self.entries
            .iter()
            .find(|e| e.report.id() == id)
            .map(|e| e.report.as_ref())
    }

    /// The report a generated page shows, if it is registered.
    pub fn report_of_page(&self, page: &Page) -> Option<&dyn Report> {
        match &page.kind {
            PageKind::Generated { generator } => self.report(generator),
            _ => None,
        }
    }

    /// Tells the cache about one transaction (an edit, undo or redo) after
    /// it is applied to `project`. Returns the ids of the reports it made
    /// stale. For a change set merged over several transactions use
    /// [`LiveReports::notify_since`].
    pub fn notify(&mut self, project: &Project, changes: &ChangeSet) -> Vec<&str> {
        if changes.is_empty() {
            // An empty transaction does not move the revision.
            return Vec::new();
        }
        self.notify_since(project, project.revision().saturating_sub(1), changes)
    }

    /// Like [`LiveReports::notify`], for `changes` covering every edit
    /// since the project was at revision `since`.
    pub fn notify_since(
        &mut self,
        project: &Project,
        since: u64,
        changes: &ChangeSet,
    ) -> Vec<&str> {
        let revision = project.revision();
        let mut stale = Vec::new();
        for entry in &mut self.entries {
            let Some((at, output)) = &mut entry.output else {
                continue;
            };
            if *at == revision {
                continue;
            }
            if *at == since && !entry.report.affected_by(changes) {
                // Still correct for the new revision: say so, so readers of
                // `ReportOutput::revision` see it as current.
                *at = revision;
                output.revision = revision;
            } else {
                entry.output = None;
                stale.push(entry.report.id());
            }
        }
        stale
    }

    /// Whether a report's cached output matches the project (no work needed).
    pub fn is_current(&self, project: &Project, id: &str) -> bool {
        self.entries.iter().any(|e| {
            e.report.id() == id && matches!(&e.output, Some((at, _)) if *at == project.revision())
        })
    }

    /// The current output of a report, regenerated if anything it reads
    /// changed. `None` for an unknown id.
    pub fn get(&mut self, project: &Project, id: &str) -> Option<&ReportOutput> {
        let entry = self.entries.iter_mut().find(|e| e.report.id() == id)?;
        Some(Self::refresh(entry, project))
    }

    /// Every report's current output, in registration order.
    pub fn all(&mut self, project: &Project) -> Vec<&ReportOutput> {
        self.entries
            .iter_mut()
            .map(|entry| Self::refresh(entry, project))
            .collect()
    }

    fn refresh<'a>(entry: &'a mut Entry, project: &Project) -> &'a ReportOutput {
        let revision = project.revision();
        if !matches!(&entry.output, Some((at, _)) if *at == revision) {
            entry.output = Some((revision, entry.report.generate(project)));
        }
        &entry.output.as_ref().expect("just generated").1
    }
}
