//! "Standards check…" (Ctrl+K): choose the standards the project is built to
//! and read the compliance report from novo-checks. The report can be saved
//! as a PDF or an HTML page to hand to a customer. The rules are in our own
//! words with clause pointers; the engineer reads the clauses in their own
//! licensed copy.

use eframe::egui;
use novo_checks::standards::{self, ComplianceReport, StandardSet};

use super::NovoApp;
use super::standards_report::StandardsFormat;

impl NovoApp {
    pub(super) fn standards_window(&mut self, ctx: &egui::Context) {
        if !self.standards_open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            self.standards_open = false;
            return;
        };
        let project = doc.project();
        let selected = standards::selected(project);
        let report: ComplianceReport = standards::compliance(project);
        let markdown = standards::to_markdown(project, &report);
        let lines = standards::lines(project, &report);
        let mut chosen = selected.clone();
        let mut save: Option<StandardsFormat> = None;
        let mut open = true;
        egui::Window::new("Standards check")
            .open(&mut open)
            .default_size([640.0, 520.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Check against:");
                    for set in StandardSet::ALL {
                        let mut on = chosen.contains(&set);
                        if ui.checkbox(&mut on, set.short()).changed() {
                            if on {
                                chosen.push(set);
                            } else {
                                chosen.retain(|s| *s != set);
                            }
                        }
                    }
                });
                if report.sets.is_empty() {
                    ui.label("Choose at least one standard to see the report.");
                    return;
                }
                ui.horizontal(|ui| {
                    if ui.button("Save report as PDF").clicked() {
                        save = Some(StandardsFormat::Pdf);
                    }
                    if ui.button("Save as web page").clicked() {
                        save = Some(StandardsFormat::Html);
                    }
                    if ui.button("Copy as Markdown").clicked() {
                        ctx.copy_text(markdown.clone());
                    }
                });
                ui.label("The report assists the engineer and is not a certificate. Verdicts marked planning value rest on numbers not yet checked against your licensed standard.");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (bold, text) in &lines {
                        if *bold {
                            ui.strong(text);
                        } else {
                            ui.label(text);
                        }
                    }
                });
            });
        if chosen != selected {
            self.edit_project("Select standards", |tx| standards::select(tx, &chosen));
        }
        if let Some(format) = save {
            self.export_standards_report(format);
        }
        self.standards_open = open;
    }
}
