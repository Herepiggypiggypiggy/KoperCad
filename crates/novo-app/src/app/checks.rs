//! Design checks in the app (novo-checks, WS-14): the issue pill, the
//! outline's Issues mode and F8 / Shift+F8.

use eframe::egui;
use novo_canvas::model::item_id;
use novo_canvas::{Mark, MarkKind};
use novo_checks::{Problem, Severity, Summary};
use novo_core::{AnyId, PageId};
use novo_xref::Place;

use super::{Level, NovoApp};

impl NovoApp {
    /// Problems marked where they are (DESIGN.md §5, principle 7): a
    /// warning or error glyph on each drawn object of each problem on
    /// `page`, its message on hover. Infos are listed, not marked.
    pub(super) fn problem_marks(&mut self, page: PageId) -> Vec<Mark> {
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let on_page = |id: &AnyId| match id {
            AnyId::Symbol(s) => project.symbol(*s).is_some_and(|s| s.page == page),
            AnyId::Wire(w) => project.wire(*w).is_some_and(|w| w.page == page),
            _ => false,
        };
        let problems = match &self.wb.problems {
            Some((revision, problems)) if *revision == project.revision() => problems.clone(),
            _ => novo_checks::check(project),
        };
        let mut marks = Vec::new();
        for p in &problems {
            let kind = match p.severity {
                Severity::Error => MarkKind::Error,
                Severity::Warning => MarkKind::Warning,
                Severity::Info => continue,
            };
            for id in p.at.iter().filter(|id| on_page(id)) {
                marks.push(Mark::on(kind, item_id(*id)).with_text(p.message.clone()));
            }
        }
        marks
    }

    /// The design-check problems of the current revision.
    pub(super) fn problems(&mut self) -> &[Problem] {
        let Some(doc) = self.backend.document() else {
            return &[];
        };
        let project = doc.project();
        let revision = project.revision();
        if self
            .wb
            .problems
            .as_ref()
            .is_none_or(|(r, _)| *r != revision)
        {
            self.wb.problems = Some((revision, novo_checks::check(project)));
        }
        self.wb.problems.as_ref().map_or(&[], |(_, p)| p.as_slice())
    }

    /// The issue pill: the counts; a click opens the outline's Issues mode.
    pub(super) fn issue_pill(&mut self, ui: &mut egui::Ui) {
        if !self.is_open() {
            return;
        }
        let summary = Summary::of(self.problems());
        let response = novo_theme::widgets::issue_pill(ui, summary.errors, summary.warnings);
        self.wb.pill_rect = response.rect;
        if response.clicked() {
            self.show_issues(true);
        }
    }

    /// The outline's Issues mode (DESIGN.md §3): filter chips with each
    /// severity's count, then the problems, errors first. A click goes to
    /// the problem; returns whether one was clicked.
    pub(super) fn issues_list(&mut self, ui: &mut egui::Ui) -> bool {
        let summary = Summary::of(self.problems());
        let counts = [summary.errors, summary.warnings, summary.infos];
        ui.horizontal_wrapped(|ui| {
            for (i, name) in ["errors", "warnings", "infos"].into_iter().enumerate() {
                let chip = format!("{} {name}", counts[i]);
                if ui.selectable_label(self.wb.issue_filter[i], chip).clicked() {
                    self.wb.issue_filter[i] ^= true;
                }
            }
        });
        let filter = self.wb.issue_filter;
        // Errors first, then warnings, then infos, each in check order.
        let mut problems: Vec<(usize, Problem)> = self
            .problems()
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, p)| filter[severity_rank(p.severity)])
            .collect();
        problems.sort_by_key(|(_, p)| severity_rank(p.severity));
        let mut go = None;
        if problems.is_empty() {
            ui.weak(if summary.errors + summary.warnings + summary.infos == 0 {
                "No issues"
            } else {
                "None shown. Turn a filter on."
            });
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, p) in &problems {
                    let mark = match p.severity {
                        Severity::Error => "⛔",
                        Severity::Warning => "⚠",
                        Severity::Info => "ℹ",
                    };
                    let text = format!("{mark} {}", p.message);
                    if ui.selectable_label(false, text).clicked() {
                        go = Some(*i);
                    }
                }
            });
        ui.weak("F8 next · Shift+F8 previous");
        if let Some(i) = go {
            self.go_problem(i);
        }
        go.is_some()
    }

    /// Shows the outline in Issues mode. With `toggle`, a second request
    /// while the unpinned flyout already shows the issues closes it.
    pub(super) fn show_issues(&mut self, toggle: bool) {
        let showing = self.outline_mode == super::OutlineMode::Issues;
        self.outline_mode = super::OutlineMode::Issues;
        if !self.outline_pinned() {
            self.outline_open = !(toggle && showing && self.outline_open);
        }
    }

    pub(super) fn step_problem(&mut self, forward: bool) {
        let n = self.problems().iter().filter(|p| p.page.is_some()).count();
        if n == 0 {
            self.message(Level::Info, "No problems on the drawing");
            return;
        }
        let at = match (self.wb.problem_at, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        self.wb.problem_at = Some(at);
        let index = self
            .problems()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.page.is_some())
            .nth(at)
            .map(|(i, _)| i);
        if let Some(i) = index {
            self.go_problem(i);
        }
    }

    pub(super) fn go_problem(&mut self, index: usize) {
        let Some(problem) = self.problems().get(index).cloned() else {
            return;
        };
        self.message(Level::Info, problem.message.clone());
        let Some(page) = problem.page else {
            return;
        };
        let symbol = problem.at.iter().find_map(|id| match id {
            AnyId::Symbol(s) => Some(*s),
            _ => None,
        });
        self.jump(Place { page, symbol }, true);
    }
}

/// The filter slot and sort order of a severity.
fn severity_rank(severity: Severity) -> usize {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}
