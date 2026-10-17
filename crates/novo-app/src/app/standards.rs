//! "Standards check…" (Ctrl+K): choose the standards the project is built to
//! and read the compliance report from novo-checks. The report can be saved
//! as a PDF or an HTML page to hand to a customer. The rules are in our own
//! words with clause pointers; the engineer reads the clauses in their own
//! licensed copy.

use eframe::egui;
use novo_checks::standards::{self, ComplianceReport, StandardSet, keys};
use novo_core::Value;

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
        let text_of = |key: &str| match project.info().properties.get(key) {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let method = text_of(keys::DEFAULT_INSTALLATION_METHOD);
        let mut new_method = method.clone();
        let ambient = match project.info().properties.get(keys::AMBIENT_C) {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(i)) => *i as f64,
            _ => 40.0,
        };
        // A drag is kept here until it ends: one drag, one undo step.
        let ambient_id = egui::Id::new("standards_ambient_pending");
        let mut new_ambient = ctx
            .data(|d| d.get_temp::<f64>(ambient_id))
            .unwrap_or(ambient);
        let mut ambient_done = false;
        let mut save: Option<StandardsFormat> = None;
        let (mut to_inputs, mut to_calc) = (false, false);
        let mut open = true;
        super::panel::dialog(ctx, "Standards check", [640.0, 520.0])
            .open(&mut open)
            .show(ctx, |ui| {
                novo_theme::widgets::section_label(ui, "Check against");
                ui.horizontal(|ui| {
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
                ui.add_space(6.0);
                novo_theme::widgets::section_label(ui, "Assumptions");
                ui.horizontal(|ui| {
                    ui.label("Wires with no installation method:");
                    egui::ComboBox::from_id_salt("standards_default_method")
                        .selected_text(if method.is_empty() { "not stated" } else { method.as_str() })
                        .show_ui(ui, |ui| {
                            for (key, label) in [
                                ("", "not stated"),
                                ("B1", "B1: single cores in conduit"),
                                ("B2", "B2: multicore in conduit"),
                                ("C", "C: clipped or on a tray"),
                            ] {
                                if ui.selectable_label(method == key, label).clicked() {
                                    new_method = key.to_owned();
                                }
                            }
                        });
                    ui.label("Ambient °C:");
                    let r = ui.add(
                        egui::DragValue::new(&mut new_ambient)
                            .range(10.0..=60.0)
                            .speed(0.5),
                    );
                    ambient_done = r.drag_stopped() || r.lost_focus();
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Electrical inputs...")
                        .on_hover_text("Supply short-circuit current, panel SCCR, machine data and e-stop: the numbers the rules below are unknown without")
                        .clicked()
                    {
                        to_inputs = true;
                    }
                    if ui
                        .button("Calculations...")
                        .on_hover_text("Cable rating, voltage drop, short circuit and starter sizing with the numbers")
                        .clicked()
                    {
                        to_calc = true;
                    }
                });
                if report.sets.is_empty() {
                    ui.label("Choose at least one standard to see the report.");
                    return;
                }
                ui.horizontal(|ui| {
                    if novo_theme::widgets::primary_button(ui, "Save report as PDF").clicked() {
                        save = Some(StandardsFormat::Pdf);
                    }
                    if ui.button("Save as web page").clicked() {
                        save = Some(StandardsFormat::Html);
                    }
                    if ui.button("Copy as Markdown").clicked() {
                        ctx.copy_text(markdown.clone());
                    }
                });
ui.weak(
                    "The report assists the engineer: the rules are a subset of the standard, not a certification. Verdicts marked planning value rest on numbers not yet checked against your licensed standard.",
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (bold, text) in &lines {
                        if *bold {
                            ui.add_space(4.0);
                            novo_theme::widgets::section_label(ui, text);
                        } else {
                            ui.label(text);
                        }
                    }
                });
            });
        if chosen != selected {
            self.edit_project("Select standards", |tx| standards::select(tx, &chosen));
        }
        if new_method != method {
            self.edit_project("Default installation method", |tx| {
                standards::set_info_property(
                    tx,
                    keys::DEFAULT_INSTALLATION_METHOD,
                    (!new_method.is_empty()).then(|| Value::Text(new_method.clone())),
                )
            });
        }
        if ambient_done {
            ctx.data_mut(|d| d.remove::<f64>(ambient_id));
        } else if (new_ambient - ambient).abs() > 1e-9 {
            ctx.data_mut(|d| d.insert_temp(ambient_id, new_ambient));
        }
        if ambient_done && (new_ambient - ambient).abs() > 1e-9 {
            self.edit_project("Ambient temperature", |tx| {
                standards::set_info_property(tx, keys::AMBIENT_C, Some(Value::Float(new_ambient)))
            });
        }
        if to_inputs {
            self.toggle_electrical_open();
        }
        if to_calc && self.wb.calculations.is_none() {
            self.toggle_calculations();
        }
        if let Some(format) = save {
            self.export_standards_report(format);
        }
        self.standards_open = open;
    }
}
