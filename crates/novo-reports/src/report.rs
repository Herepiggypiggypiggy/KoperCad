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

    /// The same dependencies plus more record kinds.
    pub fn with_kinds(mut self, kinds: &[EntityKind]) -> Self {
        self.kinds.extend(kinds.iter().copied());
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

    /// Whether [`crate::default_generated_pages`] proposes a page for this
    /// report. False for reports that are exports first (the cable schedule).
    fn proposed_by_default(&self) -> bool {
        true
    }

    /// Whether the report reads other reports' output (the table of
    /// contents counts their sheets). [`LiveReports`] then hands it its
    /// cached outputs through [`Report::generate_with`] instead of the
    /// report building them a second time.
    fn reads_reports(&self) -> bool {
        false
    }

    /// Like [`Report::generate`], given the current output of other
    /// reports by id (`None` for an id that is not registered). Only called
    /// when [`Report::reads_reports`] is true; must return what `generate`
    /// would.
    fn generate_with(&self, project: &Project, _reports: Reports<'_>) -> ReportOutput {
        self.generate(project)
    }

    /// Whether an edit may change this report. The default reads
    /// [`Report::dependencies`]; override for a finer test.
    fn affected_by(&self, changes: &ChangeSet) -> bool {
        self.dependencies().affected_by(changes)
    }
}

/// Other reports' current output by id, for [`Report::generate_with`].
pub type Reports<'a> = &'a dyn Fn(&str) -> Option<&'a ReportOutput>;

/// Which project state an output matches: the revision, plus two cheap
/// facts that tell projects apart when one replaces another in place (a
/// forgotten [`LiveReports::clear`]): the next free id and the name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    revision: u64,
    next_id: u64,
    name: String,
}

impl Stamp {
    fn of(project: &Project) -> Self {
        Stamp {
            revision: project.revision(),
            next_id: project.next_id(),
            name: project.info().name.clone(),
        }
    }
}

struct Entry {
    report: Box<dyn Report>,
    /// The output and the project state it is known to match.
    output: Option<(Stamp, ReportOutput)>,
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
/// One `LiveReports` belongs to one [`Document`](novo_core::Document);
/// call [`LiveReports::clear`] (or make a new one) when another project is
/// opened. Output is matched by revision, next free id and project name, so
/// a forgotten `clear` is caught unless the new project agrees on all
/// three.
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
        let now = Stamp::of(project);
        let mut stale = Vec::new();
        for entry in &mut self.entries {
            let Some((at, output)) = &mut entry.output else {
                continue;
            };
            if *at == now {
                continue;
            }
            if at.revision == since && at.name == now.name && !entry.report.affected_by(changes) {
                // Still correct for the new revision: say so, so readers of
                // `ReportOutput::revision` see it as current.
                output.revision = now.revision;
                *at = now.clone();
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
            e.report.id() == id && matches!(&e.output, Some((at, _)) if *at == Stamp::of(project))
        })
    }

    /// The current output of a report, regenerated if anything it reads
    /// changed. `None` for an unknown id.
    pub fn get(&mut self, project: &Project, id: &str) -> Option<&ReportOutput> {
        let i = self.entries.iter().position(|e| e.report.id() == id)?;
        self.refresh(i, project);
        Some(self.output_at(i))
    }

    /// Every report's current output, in registration order.
    pub fn all(&mut self, project: &Project) -> Vec<&ReportOutput> {
        for i in 0..self.entries.len() {
            self.refresh(i, project);
        }
        (0..self.entries.len()).map(|i| self.output_at(i)).collect()
    }

    fn output_at(&self, i: usize) -> &ReportOutput {
        &self.entries[i].output.as_ref().expect("refreshed").1
    }

    fn refresh(&mut self, i: usize, project: &Project) {
        let now = Stamp::of(project);
        if matches!(&self.entries[i].output, Some((at, _)) if *at == now) {
            return;
        }
        let output = if self.entries[i].report.reads_reports() {
            // Bring the reports it may read current first: those of the
            // project's generated pages (all a contents list reads).
            let generators: Vec<&str> = project
                .pages()
                .filter_map(|p| match &p.kind {
                    PageKind::Generated { generator } => Some(generator.as_str()),
                    _ => None,
                })
                .collect();
            for j in 0..self.entries.len() {
                let report = &self.entries[j].report;
                if j != i && !report.reads_reports() && generators.contains(&report.id()) {
                    self.refresh(j, project);
                }
            }
            let entries = &self.entries;
            let lookup = |id: &str| {
                entries
                    .iter()
                    .enumerate()
                    .find(|(j, e)| *j != i && e.report.id() == id)
                    .and_then(|(_, e)| e.output.as_ref())
                    .filter(|(at, _)| *at == now)
                    .map(|(_, output)| output)
            };
            entries[i].report.generate_with(project, &lookup)
        } else {
            self.entries[i].report.generate(project)
        };
        self.entries[i].output = Some((now, output));
    }
}
