//! Design checks in the app (novo-checks, WS-14): the issue pill, its
//! problem list and F8 / Shift+F8.

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

    /// The issue pill: the count; a click lists the problems.
    pub(super) fn issue_pill(&mut self, ui: &mut egui::Ui) {
        if !self.is_open() {
            return;
        }
        let summary = Summary::of(self.problems());
        let color = if summary.errors > 0 {
            ui.visuals().error_fg_color
        } else if summary.warnings > 0 {
            ui.visuals().warn_fg_color
        } else {
            ui.visuals().weak_text_color()
        };
        let pill = egui::Button::new(egui::RichText::new(summary.pill_text()).color(color))
            .corner_radius(10.0);
        let response = ui.add(pill);
        self.wb.pill_rect = response.rect;
        if response.clicked() {
            self.wb.issues_open ^= true;
        }
    }

    /// The problem list under the pill. A click goes to the problem.
    pub(super) fn issues_popup(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.issues_open || !self.is_open() {
            return;
        }
        // Errors first, then warnings, then infos, each in check order.
        let mut problems: Vec<(usize, Problem)> =
            self.problems().iter().cloned().enumerate().collect();
        problems.sort_by_key(|(_, p)| match p.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        });
        let mut go = None;
        let area = egui::Area::new(egui::Id::new("issues"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-8.0, 8.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(380.0);
                    ui.small("ISSUES · F8 next");
                    if problems.is_empty() {
                        ui.weak("No issues");
                    }
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
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
                });
            });
        if let Some(i) = go {
            self.go_problem(i);
        }
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        // The click on the pill that opened the list is not "elsewhere".
        let on_pill = ctx
            .pointer_interact_pos()
            .is_some_and(|p| self.wb.pill_rect.contains(p));
        if escape || (area.response.clicked_elsewhere() && go.is_none() && !on_pill) {
            self.wb.issues_open = false;
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
