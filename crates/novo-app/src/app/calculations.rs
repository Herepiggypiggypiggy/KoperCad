//! The Calculations window (Check menu, `check.calculations`): per
//! protected circuit the cable rating, voltage drop, breaking capacity,
//! short circuit, disconnection and protective-conductor results with the
//! numbers and the standard clause, the smallest passing section when a
//! circuit fails, the motor starters sized from their current and the
//! panel's short-circuit rating. The numbers come from
//! [`novo_checks::calc_view`]; this file only draws them.
//!
//! The ampacity tables are planning values until the engineer loads their
//! licensed copy, and the window says so at the top.

use std::fmt::Write as _;

use eframe::egui;
use novo_checks::calc::Rule;
use novo_checks::calc::circuit::{Row, Status};
use novo_checks::calc::starter::{Coordination, Scheme};
use novo_checks::calc::tables::TableSet;
use novo_checks::calc_view::{self, CalcSheet, CircuitEntry, StarterChoice, StarterEntry};
use novo_core::AnyId;
use novo_xref::Place;

use super::model_labels::{Model, badge};
use super::{Level, NovoApp};

/// What the window remembers between frames.
#[derive(Debug, Default)]
pub(super) struct CalcWindow {
    tab: usize,
    choice: StarterChoice,
    only_issues: bool,
    /// The sheet for a project revision and starter choice.
    cache: Option<(u64, StarterChoice, CalcSheet)>,
}

impl CalcWindow {
    /// The sheet last built, for tests.
    #[cfg(test)]
    pub(super) fn sheet(&self) -> Option<&CalcSheet> {
        self.cache.as_ref().map(|(_, _, s)| s)
    }
}

const TABS: [&str; 3] = ["Circuits", "Motor starters", "Panel SCCR"];

/// The warning every calculation view carries.
pub(super) const PLANNING_NOTE: &str = "Ampacity tables and defaults are planning values, not checked against the standard. Read the clauses in your licensed copy before relying on a number.";

fn status_label(status: Status) -> (&'static str, egui::Color32) {
    match status {
        Status::Pass => ("Pass", crate::theme::ui_palette().success),
        Status::Fail => ("Fail", crate::theme::ui_palette().error),
        Status::Warning => ("Warning", crate::theme::ui_palette().warning),
        Status::Unknown => ("Unknown", crate::theme::ui_palette().warning),
    }
}

fn amount(value: f64) -> String {
    if (value - value.round()).abs() < 0.005 {
        format!("{}", value.round())
    } else {
        format!("{value:.2}")
    }
}

/// The kind of model number a row rests on: the cable rating and the voltage
/// drop both read the ampacity and resistance tables, which are planning values.
fn row_model(row: &Row) -> Option<Model> {
    matches!(row.rule, Rule::CableRating | Rule::VoltageDrop).then_some(Model::Ampacity)
}

fn row_numbers(row: &Row) -> String {
    match (row.value, row.limit) {
        (Some(v), Some(l)) => format!("{} / {} {}", amount(v), amount(l), row.unit),
        (Some(v), None) => format!("{} {}", amount(v), row.unit),
        _ => String::new(),
    }
}

fn circuit_summary(entry: &CircuitEntry) -> String {
    let wire = if entry.wire_label.is_empty() {
        String::new()
    } else {
        format!(" · {}", entry.wire_label)
    };
    match &entry.report {
        None => format!("{}{wire} · needs inputs", entry.tag),
        Some(r) => {
            let fails = r.failures().count();
            let unknown = r
                .rows
                .iter()
                .filter(|x| x.status == Status::Unknown)
                .count();
            let state = if fails > 0 {
                format!("{fails} failing")
            } else if unknown > 0 {
                format!("{unknown} unknown")
            } else {
                "all pass".to_owned()
            };
            format!("{}{wire} · {state}", entry.tag)
        }
    }
}

/// The whole sheet as plain text, for pasting into a report.
pub(super) fn sheet_text(sheet: &CalcSheet) -> String {
    let mut out = String::from("Calculations\n");
    let _ = writeln!(out, "{PLANNING_NOTE}");
    let _ = writeln!(out, "{}", sheet.tables);
    match &sheet.supply {
        Some(s) => {
            let _ = writeln!(out, "Supply: {s}");
        }
        None => out.push_str("Supply: not stated\n"),
    }
    for m in &sheet.missing {
        let _ = writeln!(out, "Missing: {m}");
    }
    for c in &sheet.circuits {
        out.push('\n');
        match &c.report {
            Some(r) => {
                let _ = write!(out, "{}", r.to_text());
                for row in &r.rows {
                    let clause = calc_view::clause(row.title);
                    let model =
                        row_model(row).map_or(String::new(), |m| format!(" ({})", m.label()));
                    if !clause.is_empty() {
                        let _ = writeln!(out, "    {}: {clause}{model}", row.title);
                    }
                }
                match c.smallest_section_mm2 {
                    Some(Some(s)) => {
                        let _ = writeln!(out, "  Smallest passing section: {} mm²", amount(s));
                    }
                    Some(None) => out.push_str(
                        "  No listed section passes: change the device, the length or the method\n",
                    ),
                    None => {}
                }
            }
            None => {
                let _ = writeln!(out, "Circuit {} (not judged)", c.tag);
            }
        }
        for m in &c.missing {
            let _ = writeln!(out, "  Missing: {m}");
        }
    }
    for s in &sheet.starters {
        let _ = writeln!(out, "\nMotor starter {}", s.tag);
        match &s.result {
            Some(Ok(p)) => {
                let _ = writeln!(
                    out,
                    "  Ie {} A; short-circuit device {}; contactor {}; overload {}; cable {} mm² (PE {} mm²)",
                    amount(p.motor_current_a),
                    p.short_circuit.id,
                    p.contactor.id,
                    p.overload.id,
                    amount(p.cable_mm2),
                    amount(p.pe_mm2)
                );
            }
            Some(Err(e)) => {
                for r in &e.reasons {
                    let _ = writeln!(out, "  No proposal: {r}");
                }
            }
            None => {}
        }
        for m in &s.missing {
            let _ = writeln!(out, "  Missing: {m}");
        }
    }
    let p = &sheet.panel;
    let _ = writeln!(
        out,
        "\nPanel SCCR: {}",
        p.result
            .panel_ka
            .map_or("unknown".to_owned(), |k| format!("{} kA", amount(k)))
    );
    if let Some(a) = p.available_ka {
        let _ = writeln!(out, "  Available at the supply: {} kA", amount(a));
    }
    for n in &p.result.notes {
        let _ = writeln!(out, "  {n}");
    }
    out
}

impl NovoApp {
    pub(super) fn toggle_calculations(&mut self) {
        if self.wb.calculations.take().is_none() && self.is_open() {
            self.wb.calculations = Some(CalcWindow::default());
        }
    }

    pub(super) fn calculations_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.calculations.take() else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let revision = project.revision();
        if win
            .cache
            .as_ref()
            .is_none_or(|(r, c, _)| *r != revision || *c != win.choice)
        {
            let sheet = calc_view::calc_sheet(
                project,
                &project.netlist(),
                win.choice,
                &TableSet::builtin(),
            );
            win.cache = Some((revision, win.choice, sheet));
        }
        let mut open = true;
        let mut go: Option<Vec<AnyId>> = None;
        let mut copy: Option<String> = None;
        let mut to_inputs = false;
        if let Some((_, _, sheet)) = win.cache.clone() {
            super::panel::dialog(ctx, "Calculations", [760.0, 620.0])
                .open(super::panel::esc(ctx, &mut open))
                .show(ctx, |ui| {
                    header(ui, &sheet, &mut to_inputs, &mut copy);
                    ui.separator();
                    novo_theme::widgets::tabs(ui, &mut win.tab, &TABS);
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().show(ui, |ui| match win.tab {
                        0 => circuits(ui, &sheet, &mut win.only_issues, &mut go),
                        1 => starters(ui, &sheet, &mut win.choice),
                        _ => panel(ui, &sheet),
                    });
                });
        }
        if let Some(text) = copy {
            ctx.copy_text(text);
            self.message(Level::Info, "Calculations copied as text");
        }
        if let Some(marks) = go {
            self.go_to_marks(&marks);
        }
        if to_inputs {
            self.toggle_electrical_open();
        }
        if open {
            self.wb.calculations = Some(win);
        }
    }

    /// Opens the Electrical inputs window if it is not already open.
    pub(super) fn toggle_electrical_open(&mut self) {
        if self.wb.electrical.is_none() {
            self.toggle_electrical();
        }
    }

    /// Shows the first of `marks` that is drawn on a page.
    pub(super) fn go_to_marks(&mut self, marks: &[AnyId]) {
        for id in marks {
            let place = match id {
                AnyId::Device(device) => self.device_main(*device),
                AnyId::Wire(wire) => self
                    .backend
                    .document()
                    .and_then(|d| d.project().wire(*wire))
                    .map(|w| Place {
                        page: w.page,
                        symbol: None,
                    }),
                _ => None,
            };
            if let Some(place) = place {
                self.jump(place, true);
                return;
            }
        }
        self.message(Level::Info, "That circuit is not drawn on a page");
    }
}

fn header(ui: &mut egui::Ui, sheet: &CalcSheet, to_inputs: &mut bool, copy: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        match &sheet.supply {
            Some(s) => ui.label(format!("Supply: {s}")),
            None => ui.colored_label(
                crate::theme::ui_palette().warning,
                "Supply not stated: nothing can be judged.",
            ),
        };
        if ui.button("Electrical inputs...").clicked() {
            *to_inputs = true;
        }
        if ui.button("Copy as text").clicked() {
            *copy = Some(sheet_text(sheet));
        }
    });
    if sheet.planning_values {
        ui.colored_label(crate::theme::ui_palette().warning, PLANNING_NOTE);
    }
    ui.weak(&sheet.tables);
    for m in &sheet.missing {
        ui.colored_label(crate::theme::ui_palette().warning, format!("Missing: {m}"));
    }
}

fn circuits(
    ui: &mut egui::Ui,
    sheet: &CalcSheet,
    only_issues: &mut bool,
    go: &mut Option<Vec<AnyId>>,
) {
    if sheet.circuits.is_empty() {
        ui.label("No fuse, breaker or motor protection device is drawn yet. A circuit is a protective device with the wires behind it.");
        return;
    }
    ui.horizontal(|ui| {
        ui.label(format!(
            "{} circuits, {} failing",
            sheet.circuits.len(),
            sheet.failing()
        ));
        ui.checkbox(only_issues, "Only circuits with a problem or missing input");
    });
    for entry in &sheet.circuits {
        let issue =
            entry.report.as_ref().is_none_or(|r| !r.all_pass()) || !entry.missing.is_empty();
        if *only_issues && !issue {
            continue;
        }
        let failing = entry.report.as_ref().is_some_and(|r| r.has_failure());
        let title = egui::RichText::new(circuit_summary(entry));
        let title = if failing {
            title.color(crate::theme::ui_palette().error)
        } else {
            title
        };
        egui::CollapsingHeader::new(title)
            .id_salt(("calc_circuit", entry.device))
            .default_open(failing)
            .show(ui, |ui| circuit_body(ui, entry, go));
    }
}

fn circuit_body(ui: &mut egui::Ui, entry: &CircuitEntry, go: &mut Option<Vec<AnyId>>) {
    if ui.small_button("Show on the drawing").clicked() {
        *go = Some(calc_view::marks(entry));
    }
    if let Some(report) = &entry.report {
        egui::Grid::new(("calc_rows", entry.device))
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                for row in &report.rows {
                    let (label, color) = status_label(row.status);
                    ui.colored_label(color, label);
                    ui.horizontal(|ui| {
                        ui.label(row.title);
                        if let Some(model) = row_model(row) {
                            badge(ui, model);
                        }
                    });
                    ui.label(row_numbers(row));
                    ui.weak(calc_view::clause(row.title));
                    ui.end_row();
                    ui.label("");
                    ui.add(egui::Label::new(&row.text).wrap());
                    ui.end_row();
                }
            });
        if let Some(iz) = report.iz_a {
            ui.horizontal(|ui| {
                ui.label(format!("Cable rating Iz: {} A", amount(iz)));
                badge(ui, Model::Ampacity);
            });
        }
        if let Some(f) = &report.at_device {
            ui.label(format!(
                "Short-circuit current at the device: {} kA (3-phase, max)",
                amount(f.ik3_max_a / 1000.0)
            ));
        }
        if let Some(f) = &report.at_load {
            let min = f
                .ik1_min_a
                .map_or("unknown".to_owned(), |a| format!("{} A", amount(a)));
            ui.label(format!(
                "At the cable end: {} kA max, earth fault min {min}",
                amount(f.ik3_max_a / 1000.0)
            ));
        }
        match (entry.smallest_section_mm2, entry.section_mm2) {
            (Some(Some(s)), Some(now)) => {
                ui.colored_label(
                    crate::theme::ui_palette().warning,
                    format!(
                        "Smallest passing section: {} mm² (now {} mm²)",
                        amount(s),
                        amount(now)
                    ),
                );
            }
            (Some(None), _) => {
                ui.colored_label(
                    crate::theme::ui_palette().error,
                    "No listed section passes: change the device, the length or the installation.",
                );
            }
            _ => {}
        }
        if entry.judged_wires > 1 {
            ui.weak(format!(
                "{} wires behind this device had the data to be judged; the worst is shown.",
                entry.judged_wires
            ));
        }
        if !report.assumptions.is_empty() {
            egui::CollapsingHeader::new("Assumed")
                .id_salt(("calc_assumed", entry.device))
                .show(ui, |ui| {
                    for a in &report.assumptions {
                        ui.weak(format!("- {a}"));
                    }
                });
        }
    }
    for m in &entry.missing {
        ui.colored_label(crate::theme::ui_palette().warning, format!("Missing: {m}"));
    }
}

fn starters(ui: &mut egui::Ui, sheet: &CalcSheet, choice: &mut StarterChoice) {
    ui.horizontal(|ui| {
        ui.label("Protection");
        egui::ComboBox::from_id_salt("calc_scheme")
            .selected_text(match choice.scheme {
                Scheme::FuseContactorRelay => "gG fuse, contactor, overload relay",
                Scheme::MotorBreakerContactor => "motor protection breaker, contactor",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut choice.scheme,
                    Scheme::FuseContactorRelay,
                    "gG fuse, contactor, overload relay",
                );
                ui.selectable_value(
                    &mut choice.scheme,
                    Scheme::MotorBreakerContactor,
                    "motor protection breaker, contactor",
                );
            });
        ui.label("Coordination");
        ui.selectable_value(&mut choice.coordination, Coordination::Type1, "Type 1");
        ui.selectable_value(&mut choice.coordination, Coordination::Type2, "Type 2");
    });
    ui.weak("Sized from the motor's nameplate current with generic ratings (IEC 60947-4-1). Maker coordination tables are not loaded, so those points stay open.");
    if sheet.starters.is_empty() {
        ui.label("No motor is drawn yet.");
    }
    for s in &sheet.starters {
        starter(ui, s);
    }
}

fn starter(ui: &mut egui::Ui, s: &StarterEntry) {
    egui::CollapsingHeader::new(s.tag.to_string())
        .id_salt(("calc_starter", s.motor))
        .default_open(true)
        .show(ui, |ui| {
            match &s.result {
                Some(Ok(p)) => {
                    ui.label(format!("Motor current Ie: {} A", amount(p.motor_current_a)));
                    for (what, c) in [
                        ("Short-circuit device", &p.short_circuit),
                        ("Contactor", &p.contactor),
                        ("Overload", &p.overload),
                    ] {
                        ui.label(format!("{what}: {} ({})", c.id, c.why));
                    }
                    ui.label(format!(
                        "Cable: {} mm², protective conductor {} mm²",
                        amount(p.cable_mm2),
                        amount(p.pe_mm2)
                    ));
                    for u in &p.unresolved {
                        ui.colored_label(crate::theme::ui_palette().warning, u);
                    }
                    for a in &p.assumptions {
                        ui.weak(format!("- {a}"));
                    }
                }
                Some(Err(e)) => {
                    for r in &e.reasons {
                        ui.colored_label(
                            crate::theme::ui_palette().error,
                            format!("No proposal: {r}"),
                        );
                    }
                }
                None => {}
            }
            for n in &s.notes {
                ui.weak(format!("- {n}"));
            }
            for m in &s.missing {
                ui.colored_label(crate::theme::ui_palette().warning, format!("Missing: {m}"));
            }
        });
}

fn panel(ui: &mut egui::Ui, sheet: &CalcSheet) {
    let p = &sheet.panel;
    ui.label(format!(
        "{} power components counted in one series chain (branch limiting is not credited); the default table applies to components without a stated rating.",
        p.components
    ));
    match p.result.panel_ka {
        Some(k) => {
            ui.heading(format!("Panel SCCR {} kA", amount(k)));
        }
        None => {
            ui.heading("Panel SCCR unknown");
        }
    }
    if let Some(l) = &p.result.limiting {
        ui.label(format!(
            "Limited by {} at {} kA ({})",
            l.component,
            amount(l.ka),
            l.basis
        ));
    }
    if let Some(a) = p.available_ka {
        let (text, color) = match p.fit {
            Some(true) => ("covers", crate::theme::ui_palette().success),
            Some(false) => ("is BELOW", crate::theme::ui_palette().error),
            None => (
                "cannot be compared with",
                crate::theme::ui_palette().warning,
            ),
        };
        ui.colored_label(
            color,
            format!(
                "The panel {text} the {} kA available at the supply.",
                amount(a)
            ),
        );
    } else {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            "Available short-circuit current not stated: enter it in Electrical inputs.",
        );
    }
    if let Some(m) = p.marked_ka {
        ui.label(format!("Rating marked on the panel: {} kA", amount(m)));
    }
    if !p.result.defaulted.is_empty() {
        ui.weak(format!(
            "Rated by default table (replace with marked sccr_ka): {}",
            p.result.defaulted.join(", ")
        ));
    }
    if !p.result.unknown.is_empty() {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            format!("No rating known: {}", p.result.unknown.join(", ")),
        );
    }
    for n in &p.result.notes {
        ui.weak(n);
    }
    ui.weak(&p.result.table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_drop_empty_decimals() {
        assert_eq!(amount(16.0), "16");
        assert_eq!(amount(2.5), "2.50");
        assert_eq!(amount(9.0001), "9");
    }

    #[test]
    fn the_planning_note_is_always_available_to_the_views() {
        assert!(PLANNING_NOTE.contains("planning values"));
    }
}
