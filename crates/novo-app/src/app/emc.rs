//! "EMC check…" (Check menu, Ctrl+K "EMC"): the EMC rules of novo-emc on the
//! open project. Turn the check on, read each finding with its reason, its
//! level (rule, index or estimate) and its suggestion, apply the suggestions
//! the program can do as one undo step, and see what the numbers assume and
//! what is not checked. A design aid, never a test result.
//!
//! The findings also arrive in the Issues list once the check is on; this
//! window adds the risk by wire, the reasons behind each score, the
//! assumptions and the Apply buttons.

use eframe::egui;
use novo_emc::project as emc;
use novo_emc::{Band, Fix, Level, Report, Severity, Status, Subject};

use super::NovoApp;

/// The report for one revision of the project, so the window does not route
/// every wire on every frame.
#[derive(Default)]
pub(super) struct EmcState {
    pub(super) open: bool,
    cache: Option<(u64, Cached)>,
    /// What the last Apply did, in a sentence.
    done: Option<String>,
}

impl EmcState {
    #[cfg(test)]
    pub(super) fn done_text(&self) -> Option<&str> {
        self.done.as_deref()
    }
}

struct Cached {
    report: Report,
    classes: Vec<(novo_emc::EmcClass, usize)>,
    names: std::collections::HashMap<u64, String>,
}

fn level_chip(level: Level) -> &'static str {
    match level {
        Level::Fact => "Rule",
        Level::Index => "Index",
        Level::Estimate => "Estimate",
        Level::Field => "Field",
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::NoFinding => "No finding",
        Status::Finding => "Finding",
        Status::NotApplicable => "Not applicable",
        Status::NotEvaluated => "Not evaluated",
    }
}

impl NovoApp {
    pub(super) fn emc_window(&mut self, ctx: &egui::Context) {
        if !self.emc.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            self.emc.open = false;
            return;
        };
        let project = doc.project();
        let revision = project.revision();
        if self.emc.cache.as_ref().is_none_or(|(r, _)| *r != revision) {
            let netlist = project.netlist();
            let input = emc::input_of(project, &netlist);
            let report = novo_emc::analyse(&input);
            let cached = Cached {
                classes: emc::class_counts(&input).into_iter().collect(),
                names: input.runs.iter().map(|r| (r.id, r.name.clone())).collect(),
                report,
            };
            self.emc.cache = Some((revision, cached));
        }
        let enabled = emc::is_enabled(project);
        let Some((_, cached)) = self.emc.cache.as_ref() else {
            return;
        };
        let report = &cached.report;
        let p = crate::theme::ui_palette();
        let mut turn: Option<bool> = None;
        let mut apply: Option<Fix> = None;
        let mut open = true;
        super::panel::dialog(ctx, "EMC check", [680.0, 540.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                let mut on = enabled;
                if ui
                    .checkbox(&mut on, "Run the EMC rules with the design checks")
                    .on_hover_text("Findings join the Issues list and the count in the top bar, and the standards report gets an EMC section")
                    .changed()
                {
                    turn = Some(on);
                }
                ui.weak(format!(
                    "{} Findings are rules on what is drawn, geometric indices and closed-form estimates with planning distances from common practice; clause numbers are unverified.",
                    novo_emc::DESIGN_AID
                ));
                if let Some(done) = &self.emc.done {
                    ui.colored_label(p.success, done);
                }
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    novo_theme::widgets::section_label(ui, "Wire classes");
                    if cached.classes.is_empty() {
                        ui.label("No wires drawn yet.");
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            for (class, n) in &cached.classes {
                                ui.label(format!("{} {n}", class.label()));
                            }
                        });
                        ui.weak("Each class is read from the drawing (drive outputs, safety and analog devices, potentials); set `emc.class` on a wire or cable to override it.");
                    }
                    ui.add_space(6.0);
                    novo_theme::widgets::section_label(ui, &format!("Findings ({})", report.findings.len()));
                    if report.findings.is_empty() {
                        ui.label("No finding. That is not a statement that the design is fine: see Not checked below.");
                    }
                    for f in &report.findings {
                        let colour = match f.severity {
                            Severity::Error => p.error,
                            Severity::Warning => p.warning,
                            Severity::Info => p.text_secondary,
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(colour, egui::RichText::new(f.rule).strong());
                            ui.weak(level_chip(f.level));
                            ui.label(&f.message);
                        });
                        for m in &f.metrics {
                            ui.weak(format!("    {} ({}): {}", m.label, level_chip(m.level).to_lowercase(), m.value));
                        }
                        ui.horizontal_wrapped(|ui| {
                            for fix in &f.fixes {
                                if fix.applicable() {
                                    if ui.button(fix.hint()).on_hover_text("One undo step").clicked() {
                                        apply = Some(fix.clone());
                                    }
                                } else {
                                    ui.weak(format!("By hand: {}", fix.hint()));
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                    ui.add_space(6.0);
                    novo_theme::widgets::section_label(ui, "Risk by wire");
                    let mut runs: Vec<_> = report.runs.iter().filter(|s| s.points > 0).collect();
                    runs.sort_by_key(|r| std::cmp::Reverse(r.points));
                    if runs.is_empty() {
                        ui.label("No wire has a finding against it.");
                    }
                    for s in runs.iter().take(10) {
                        let name = match s.subject {
                            Subject::Run(id) => cached.names.get(&id).cloned().unwrap_or_default(),
                            _ => String::new(),
                        };
                        let band = match s.band {
                            Band::High => p.error,
                            Band::Medium => p.warning,
                            _ => p.text_secondary,
                        };
                        egui::CollapsingHeader::new(egui::RichText::new(format!("{name}: {} of 100, {}", s.points, s.band.label().to_lowercase())).color(band))
                            .id_salt(("emc_run", s.points, &name))
                            .show(ui, |ui| {
                                ui.weak("The score is the sum of these reasons, capped at 100. It orders wires for attention; it is not a measurement.");
                                for r in &s.reasons {
                                    ui.label(format!("+{}  {}  {}", r.points, r.rule, r.text));
                                }
                            });
                    }
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new("Rules that ran").show(ui, |ui| {
                        for r in &report.rules {
                            let rule = novo_emc::rules::rule(r.rule);
                            ui.label(format!("{}  {}: {} ({}) {}", r.rule, rule.title, status_text(r.status), level_chip(rule.level).to_lowercase(), r.note));
                            ui.weak(format!("    {} Source: {}", rule.requirement, rule.source));
                        }
                    });
                    egui::CollapsingHeader::new("Assumptions").show(ui, |ui| {
                        for a in &report.assumptions {
                            ui.label(format!(
                                "{}: {}  ({})",
                                a.name,
                                a.value,
                                if a.user_set { "set in this project" } else { "program default, edit with the property" }
                            ));
                        }
                    });
                    egui::CollapsingHeader::new("Not checked").default_open(true).show(ui, |ui| {
                        for t in novo_emc::NOT_CHECKED {
                            ui.label(*t);
                        }
                    });
                });
            });
        self.emc.open = open;
        if let Some(on) = turn {
            self.emc_turn(on);
        }
        if let Some(fix) = apply {
            self.emc_apply(&fix);
        }
    }

    /// Turns the EMC rules on or off for the project (one undo step).
    pub(super) fn emc_turn(&mut self, on: bool) {
        self.edit_project("EMC check", |tx| emc::set_enabled(tx, on));
    }

    /// Applies one suggestion as one undo step and says what it did.
    pub(super) fn emc_apply(&mut self, fix: &Fix) {
        let mut said = String::new();
        let label = format!("EMC: {}", fix.hint());
        let ok = self.edit_project(&label, |tx| {
            said = emc::apply(tx, fix)?;
            Ok(())
        });
        self.emc.done = ok.then_some(said);
    }
}
