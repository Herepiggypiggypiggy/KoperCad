//! "Standards check…" (Ctrl+K): the compliance report of the standards the
//! project selected (`standards.sets` in the project properties), from
//! novo-checks. The rules are in our own words with clause pointers; the
//! engineer reads the clauses in their own licensed copy.

use eframe::egui;
use novo_checks::standards::{self, ComplianceReport};

use super::NovoApp;

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
        let report: ComplianceReport = standards::compliance(project);
        let markdown = standards::to_markdown(project, &report);
        let lines = standards::lines(project, &report);
        let mut open = true;
        egui::Window::new("Standards check")
            .open(&mut open)
            .default_size([620.0, 480.0])
            .show(ctx, |ui| {
                if report.sets.is_empty() {
                    ui.label(
                        "No standard is selected. Set the project property `standards.sets` to a comma-separated list of: iec60204, nfpa79, ul508a.",
                    );
                    return;
                }
                if ui.button("Copy report (Markdown)").clicked() {
                    ctx.copy_text(markdown.clone());
                }
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
        self.standards_open = open;
    }
}
