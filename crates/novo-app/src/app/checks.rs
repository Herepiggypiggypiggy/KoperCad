//! Design checks in the app (novo-checks, WS-14): the issue pill, the
//! outline's Issues mode and F8 / Shift+F8.

use eframe::egui;
use novo_canvas::model::item_id;
use novo_canvas::{Mark, MarkKind};
use novo_checks::{Problem, Severity, Summary};
use novo_core::{AnyId, PageId, Project};
use novo_xref::Place;

use super::{Level, NovoApp};

/// The open problems: the design checks plus the house-rule wire breaks.
pub(super) fn open_problems(project: &Project) -> Vec<Problem> {
    novo_reports::issues::open(project)
}

/// Symbols plus wires above which the design check runs on a worker thread.
const BACKGROUND_FROM: usize = 1500;

/// A design check running for one project revision.
pub(super) struct CheckJob {
    pub(super) revision: u64,
    pub(super) rx: std::sync::mpsc::Receiver<Vec<Problem>>,
}

impl NovoApp {
    /// Problems marked where they are (DESIGN.md §5, principle 7): a
    /// warning or error glyph on each drawn object of each problem on
    /// `page`, its message on hover. Infos are listed, not marked.
    pub(super) fn problem_marks(&mut self, page: PageId) -> Vec<Mark> {
        if self.learn.quiet_checks() {
            return Vec::new();
        }
        self.refresh_problems(true);
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let on_page = |id: &AnyId| match id {
            AnyId::Symbol(s) => project.symbol(*s).is_some_and(|s| s.page == page),
            AnyId::Wire(w) => project.wire(*w).is_some_and(|w| w.page == page),
            _ => false,
        };
        // While a background run is catching up, the marks of the last
        // finished one stay (they may be a revision old).
        let problems = self.wb.problems.as_ref().map_or(&[][..], |(_, p)| p);
        let mut marks = Vec::new();
        for p in problems {
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

    /// The problems the person is shown: none while a lesson about something
    /// else runs, because its sample project's own warnings are not theirs
    /// to fix. Exports and reports read [`Self::problems`] instead.
    pub(super) fn shown_problems(&mut self) -> &[Problem] {
        if self.learn.quiet_checks() {
            return &[];
        }
        self.refresh_problems(true);
        self.wb.problems.as_ref().map_or(&[], |(_, p)| p.as_slice())
    }

    /// True while the shown problems belong to an older revision than the
    /// drawing, because a background run has not finished yet.
    pub(super) fn checks_pending(&self) -> bool {
        let Some(doc) = self.backend.document() else {
            return false;
        };
        self.wb
            .problems
            .as_ref()
            .is_none_or(|(r, _)| *r != doc.project().revision())
    }

    /// Brings `wb.problems` up to the current revision. Small projects check
    /// right here (a few milliseconds). On a big one the check (about a
    /// second at 600 pages) runs on a worker thread when `background` is set,
    /// and the last finished result stays in place until the new one lands,
    /// so an edit never freezes the window. Exports and reports pass
    /// `false`: they need the current answer and wait for it.
    fn refresh_problems(&mut self, background: bool) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let revision = project.revision();
        if let Some(job) = &self.wb.check_job {
            match job.rx.try_recv() {
                Ok(problems) => {
                    let done = job.revision;
                    self.wb.check_job = None;
                    if self.wb.problems.as_ref().is_none_or(|(r, _)| *r < done) {
                        self.wb.problems = Some((done, problems));
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // The worker died (a panic in a check): do not start it
                    // again for this revision every frame.
                    self.wb.check_failed = Some(job.revision);
                    self.wb.check_job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if self
            .wb
            .problems
            .as_ref()
            .is_some_and(|(r, _)| *r == revision)
        {
            return;
        }
        let big = project.symbols().count() + project.wires().count() > BACKGROUND_FROM;
        // A revision whose worker died is checked right here instead: the pill
        // must not pulse forever over old counts.
        let failed = self.wb.check_failed == Some(revision);
        if !(background && big) || failed {
            // A worker still running for an older revision is left to finish;
            // its answer is not used once a newer one is stored.
            self.wb.problems = Some((revision, open_problems(project)));
            return;
        }
        // One worker at a time. While it runs for an older revision nothing
        // else starts; its answer is stored when it lands, and the next frame
        // starts one run for the current revision. A burst of edits therefore
        // costs one run in flight, not one per edit.
        if self.wb.check_job.is_none() {
            let snapshot = project.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(open_problems(&snapshot));
            });
            self.wb.check_started += 1;
            self.wb.check_job = Some(CheckJob { revision, rx });
        }
    }

    /// The design-check problems of the current revision.
    pub(super) fn problems(&mut self) -> &[Problem] {
        self.refresh_problems(false);
        self.wb.problems.as_ref().map_or(&[], |(_, p)| p.as_slice())
    }

    /// The issue pill: the counts; a click opens the outline's Issues mode.
    pub(super) fn issue_pill(&mut self, ui: &mut egui::Ui) {
        if !self.is_open() {
            return;
        }
        let summary = Summary::of(self.shown_problems());
        let pending = self.checks_pending();
        if pending {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        let response = if pending && self.wb.problems.is_none() {
            // Nothing finished yet: "No issues" would be a claim not yet checked.
            ui.weak("Checking…")
        } else {
            let response = novo_theme::widgets::issue_pill(ui, summary.errors, summary.warnings);
            if pending {
                // The counts are from before the last edit: a pulsing dot in
                // the gap before the pill says so without moving the breadcrumb.
                let t = ui.input(|i| i.time);
                let alpha = 0.55 + 0.45 * (t * 6.0).sin() as f32;
                let at = egui::pos2(response.rect.left() - 5.0, response.rect.center().y);
                ui.painter().circle_filled(
                    at,
                    2.5,
                    crate::theme::ui_palette()
                        .text_secondary
                        .gamma_multiply(alpha),
                );
                response.on_hover_text("Updating after your last edit; the counts may change")
            } else {
                response
            }
        };
        self.wb.pill_rect = response.rect;
        self.anchor_response("issue.pill", &response, "Issues");
        if response.clicked() {
            self.show_issues(true);
        }
    }

    /// The outline's Issues mode (DESIGN.md §3): filter chips with each
    /// severity's count, then the problems, errors first. A click goes to
    /// the problem; returns whether one was clicked.
    pub(super) fn issues_list(&mut self, ui: &mut egui::Ui) -> bool {
        let top = ui.cursor().min;
        let clicked = self.issues_list_rows(ui);
        let bottom = ui.min_rect().max;
        let rect = egui::Rect::from_min_max(
            top,
            egui::pos2(bottom.x.max(top.x + 1.0), bottom.y.max(top.y + 1.0)),
        );
        self.anchor("issues.list", rect, "Issues list");
        clicked
    }

    fn issues_list_rows(&mut self, ui: &mut egui::Ui) -> bool {
        let summary = Summary::of(self.shown_problems());
        let counts = [summary.errors, summary.warnings, summary.infos];
        ui.horizontal_wrapped(|ui| {
            for (i, name) in ["errors", "warnings", "infos"].into_iter().enumerate() {
                let chip = format!("{} {name}", counts[i]);
                let response = novo_theme::widgets::chip(ui, &chip, self.wb.issue_filter[i]);
                self.learn_row("issues.list", &chip, response.rect);
                if response.clicked() {
                    self.wb.issue_filter[i] ^= true;
                }
            }
        });
        let filter = self.wb.issue_filter;
        // Errors first, then warnings, then infos, each in check order.
        let mut problems: Vec<(usize, Problem)> = self
            .shown_problems()
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, p)| filter[severity_rank(p.severity)])
            .collect();
        problems.sort_by_key(|(_, p)| severity_rank(p.severity));
        let mut go = None;
        self.waive_form(ui);
        let mut toggle = None;
        if problems.is_empty() {
            ui.weak(if summary.errors + summary.warnings + summary.infos == 0 {
                "No issues"
            } else {
                "None shown. Turn a filter on."
            });
        }
        // Leave a line under the list for the key hint.
        let list_height = (ui.available_height() - 24.0).max(60.0);
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, p) in &problems {
                    let (mark, color) = match p.severity {
                        Severity::Error => (
                            novo_theme::icons::status::ERROR,
                            crate::theme::ui_palette().error,
                        ),
                        Severity::Warning => (
                            novo_theme::icons::status::WARNING,
                            crate::theme::ui_palette().warning,
                        ),
                        Severity::Info => (
                            novo_theme::icons::status::INFO,
                            crate::theme::ui_palette().info,
                        ),
                    };
                    let mut text = egui::text::LayoutJob::default();
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    text.append(
                        &format!("{mark}  "),
                        0.0,
                        egui::TextFormat::simple(font.clone(), color),
                    );
                    text.append(
                        &p.message,
                        0.0,
                        egui::TextFormat::simple(font, ui.visuals().text_color()),
                    );
                    let row = ui.selectable_label(false, text);
                    self.learn_row("issues.list", &p.message, row.rect);
                    if row.clicked() {
                        go = Some(*i);
                    }
                    row.context_menu(|ui| {
                        let go_to = ui.button("Go to");
                        self.learn_row("issues.list", "Go to", go_to.rect);
                        if go_to.clicked() {
                            go = Some(*i);
                            ui.close();
                        }
                        if p.kind == novo_checks::ProblemKind::MissingPart
                            && ui.button("Assign parts…").clicked()
                        {
                            self.wb.queued.push("parts.auto_assign");
                            ui.close();
                        }
                        let waive = ui.button("Waive with a reason…");
                        self.learn_row("issues.list", "Waive with a reason…", waive.rect);
                        if waive.clicked() {
                            self.wb.waiving = Some((*i, String::new()));
                            ui.close();
                        }
                        if self.settings.ai.enabled {
                            let explain = ui.button("Explain with Claude").on_hover_text(format!(
                                "Sends this problem's text (tags and names as shown) to \
                                 Claude, with your own API key, whatever data scope you chose \
                                 (now: {}). Its reading appears in Ask Claude.",
                                self.settings.ai.consent.effective().label()
                            ));
                            if explain.clicked() {
                                self.wb.assistant.ask_later(explain_question(p));
                                ui.close();
                            }
                        }
                        let copy = ui.button("Copy message");
                        self.learn_row("issues.list", "Copy message", copy.rect);
                        if copy.clicked() {
                            ui.ctx().copy_text(p.message.clone());
                            ui.close();
                        }
                    });
                }
                // The empty space under the list has its own menu: the
                // filters and all messages at once.
                let rest = ui.available_height().max(32.0);
                let (_, space) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), rest),
                    egui::Sense::click(),
                );
                space.context_menu(|ui| {
                    for (i, name) in ["errors", "warnings", "infos"].into_iter().enumerate() {
                        let shown = filter[i];
                        if ui.selectable_label(shown, format!("Show {name}")).clicked() {
                            toggle = Some(i);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(!problems.is_empty(), egui::Button::new("Copy all shown"))
                        .clicked()
                    {
                        let all: Vec<&str> =
                            problems.iter().map(|(_, p)| p.message.as_str()).collect();
                        ui.ctx().copy_text(all.join("\n"));
                        ui.close();
                    }
                });
            });
        if let Some(i) = toggle {
            self.wb.issue_filter[i] ^= true;
        }
        self.waived_list(ui);
        ui.weak("F8 next · Shift+F8 previous");
        if let Some(i) = go {
            self.go_problem(i);
        }
        go.is_some()
    }

    /// The reason field of a problem being waived.
    fn waive_form(&mut self, ui: &mut egui::Ui) {
        let Some((index, mut reason)) = self.wb.waiving.take() else {
            return;
        };
        let Some(problem) = self.shown_problems().get(index).cloned() else {
            return;
        };
        ui.group(|ui| {
            ui.label(format!("Waive: {}", problem.message));
            let field = ui.add(
                egui::TextEdit::singleline(&mut reason)
                    .hint_text("Why this is acceptable")
                    .desired_width(f32::INFINITY),
            );
            self.anchor("issues.waive_reason", field.rect, "Reason");
            let mut keep = true;
            ui.horizontal(|ui| {
                let ready = !reason.trim().is_empty();
                let waive = ui.add_enabled(ready, egui::Button::new("Waive"));
                self.anchor("issues.waive", waive.rect, "Waive");
                if waive.clicked() {
                    keep = false;
                    let Some(doc) = self.backend.document_mut() else {
                        return;
                    };
                    match doc.transact("Waive problem", |tx| {
                        novo_checks::waive(tx, &problem, &reason).map(|_| ())
                    }) {
                        Ok(((), changes)) => {
                            self.changed_outside_canvas(&changes);
                            self.learn_signal("problem_waived");
                        }
                        Err(e) => {
                            self.message(Level::Error, format!("Could not waive the problem: {e}"))
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
            if keep {
                self.wb.waiving = Some((index, std::mem::take(&mut reason)));
            }
        });
    }

    /// The waived problems with their reasons; Restore brings one back.
    fn waived_list(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let waived = novo_reports::issues::waived(doc.project());
        if waived.is_empty() {
            return;
        }
        let mut restore = None;
        let header = egui::CollapsingHeader::new(format!("Waived ({})", waived.len()));
        let shown = header.show(ui, |ui| {
            for w in &waived {
                ui.label(&w.problem.message);
                ui.horizontal(|ui| {
                    ui.weak(format!("Reason: {}", w.reason));
                    let button = ui.small_button("Restore");
                    self.anchor("issues.restore", button.rect, "Restore");
                    if button.clicked() {
                        restore = Some(w.waiver);
                    }
                });
            }
        });
        self.learn_row(
            "issues.list",
            &format!("Waived ({})", waived.len()),
            shown.header_response.rect,
        );
        if let Some(id) = restore
            && let Some(doc) = self.backend.document_mut()
        {
            match doc.transact("Restore problem", |tx| tx.remove(id)) {
                Ok(((), changes)) => {
                    self.changed_outside_canvas(&changes);
                    self.learn_signal("waiver_restored");
                }
                Err(e) => self.message(Level::Error, format!("Could not restore: {e}")),
            }
        }
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
        let n = self
            .shown_problems()
            .iter()
            .filter(|p| p.page.is_some())
            .count();
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
            .shown_problems()
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
        let Some(problem) = self.shown_problems().get(index).cloned() else {
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

/// The question "Explain with Claude" sends for one design-check problem.
fn explain_question(p: &Problem) -> String {
    let rule = p
        .rule
        .map(|r| format!(" (standards rule {r})"))
        .unwrap_or_default();
    format!(
        "KoperCAD's design check reports this {:?}{rule}: \"{}\"\n\nExplain in plain words why \
         it matters, how serious it really is, and what I would do about it. Look at the devices \
         it names if you are allowed to. This is your reading, not the program's verdict.",
        p.severity, p.message
    )
}
