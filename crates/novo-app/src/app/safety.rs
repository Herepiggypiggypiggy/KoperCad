//! "Safety functions" (Check menu, `check.safety`): the ISO 13849-1
//! performance level of a machine safety function. Pick the risk graph
//! (S, F, P) for the required PL, describe each subsystem (category, the
//! parts of each channel with MTTFd or B10d, DCavg, CCF) and read PFHd and
//! the achieved PL, with the series total against the requirement.
//!
//! This is a calculation aid, not a certification: the engine's own
//! disclaimer is shown in the window. The functions are kept for the
//! session; use "Copy as text" to put the result in the documentation.

use std::fmt::Write as _;

use eframe::egui;
use novo_safety::{
    Category, Combined, Component, DISCLAIMER, Frequency, PerformanceLevel, Probability, Severity,
    Subsystem, SubsystemResult, combine_series, mttfd_from_b10d, operations_per_year, required_pl,
};
use novo_theme::{tokens, widgets};

use super::{Level, NovoApp};

/// How a part's reliability is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PartBasis {
    /// MTTFd in years, from the maker.
    Mttfd,
    /// B10d in cycles; MTTFd follows from the duty.
    B10d,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PartInput {
    pub name: String,
    pub basis: PartBasis,
    pub value: f64,
}

impl PartInput {
    fn new(name: &str) -> Self {
        PartInput {
            name: name.to_owned(),
            basis: PartBasis::Mttfd,
            value: 30.0,
        }
    }

    fn component(&self, nop: f64) -> Component {
        match self.basis {
            PartBasis::Mttfd => Component::new(&self.name, self.value),
            PartBasis::B10d => Component::new(&self.name, mttfd_from_b10d(self.value, nop)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SubsystemInput {
    pub name: String,
    pub category: Category,
    pub channel1: Vec<PartInput>,
    pub channel2: Vec<PartInput>,
    pub dc_percent: f64,
    pub ccf: u32,
}

impl SubsystemInput {
    fn new(name: &str) -> Self {
        SubsystemInput {
            name: name.to_owned(),
            category: Category::C3,
            channel1: vec![PartInput::new("Part 1")],
            channel2: Vec::new(),
            dc_percent: 90.0,
            ccf: 65,
        }
    }

    pub(super) fn calculate(&self, nop: f64) -> SubsystemResult {
        Subsystem {
            name: self.name.clone(),
            category: self.category,
            channel1: self.channel1.iter().map(|p| p.component(nop)).collect(),
            channel2: self.channel2.iter().map(|p| p.component(nop)).collect(),
            dc_avg_percent: self.dc_percent,
            ccf_score: self.ccf,
        }
        .calculate()
    }
}

/// What the window holds.
#[derive(Debug)]
pub(super) struct SafetyWindow {
    pub severity: Severity,
    pub frequency: Frequency,
    pub avoidance: Probability,
    pub days: f64,
    pub hours: f64,
    pub cycle_s: f64,
    pub subsystems: Vec<SubsystemInput>,
    /// The functions bound to project devices, kept in the project.
    pub bound: super::safety_functions::BoundFunctions,
}

impl Default for SafetyWindow {
    fn default() -> Self {
        SafetyWindow {
            severity: Severity::S2,
            frequency: Frequency::F1,
            avoidance: Probability::P1,
            days: 220.0,
            hours: 16.0,
            cycle_s: 600.0,
            subsystems: vec![
                SubsystemInput::new("Input (E-stop)"),
                SubsystemInput::new("Logic (safety relay)"),
                SubsystemInput::new("Output (contactors)"),
            ],
            bound: Default::default(),
        }
    }
}

impl SafetyWindow {
    pub(super) fn required(&self) -> PerformanceLevel {
        required_pl(self.severity, self.frequency, self.avoidance)
    }

    pub(super) fn nop(&self) -> f64 {
        operations_per_year(self.days, self.hours, self.cycle_s)
    }

    /// Per-subsystem results and their series total.
    pub(super) fn results(&self) -> (Vec<SubsystemResult>, Option<Combined>) {
        let nop = self.nop();
        let each: Vec<SubsystemResult> = self.subsystems.iter().map(|s| s.calculate(nop)).collect();
        let total = (!each.is_empty()).then(|| combine_series(&each));
        (each, total)
    }

    /// Whether the series total reaches the required PL.
    pub(super) fn meets(&self) -> Option<bool> {
        let (_, total) = self.results();
        total.map(|t| t.pl >= self.required())
    }

    pub(super) fn text(&self) -> String {
        let (each, total) = self.results();
        let mut out = format!("Safety function\nRequired {}\n", self.required());
        for (s, r) in self.subsystems.iter().zip(&each) {
            let _ = writeln!(
                out,
                "  {} (category {}): MTTFd {:.1} years, PFHd {:.2e} /h, {}",
                s.name,
                s.category.label(),
                r.mttfd_years,
                r.pfhd,
                r.pl
            );
            for p in &r.problems {
                let _ = writeln!(out, "    Problem: {}", p.message);
            }
        }
        if let Some(t) = total {
            let _ = writeln!(out, "Total PFHd {:.2e} /h, achieved {}", t.pfhd, t.pl);
        }
        let _ = writeln!(out, "{DISCLAIMER}");
        out
    }
}

fn pl_text(pl: PerformanceLevel) -> String {
    format!("{pl}")
}

impl NovoApp {
    pub(super) fn toggle_safety(&mut self) {
        match self.wb.safety.take() {
            Some(win) if win.bound.dirty => {
                self.store_safety_functions(&win.bound.list);
            }
            Some(_) => {}
            None if self.is_open() => self.wb.safety = Some(SafetyWindow::default()),
            None => {}
        }
    }

    pub(super) fn safety_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.safety.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        win.bound.sync(&project);
        let mut open = true;
        let mut copy = None;
        super::panel::dialog(ctx, "Safety functions", [700.0, 640.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.colored_label(tokens::WARNING, DISCLAIMER);
                ui.weak(
                    "ISO 13849-1. Project safety functions are kept in the project; the free calculator below is kept for this session only.",
                );
                ui.separator();
                super::safety_functions::section(ui, &project, &mut win.bound, &mut copy);
                ui.separator();
                widgets::section_label(ui, "Calculator (not bound to devices)");
                body(ui, &mut win, &mut copy);
            });
        if let Some(text) = copy {
            ctx.copy_text(text);
            self.message(Level::Info, "Safety function copied as text");
        }
        if open {
            self.wb.safety = Some(win);
            self.commit_safety_functions(ctx);
        } else if win.bound.dirty {
            self.store_safety_functions(&win.bound.list);
        }
    }
}

fn body(ui: &mut egui::Ui, win: &mut SafetyWindow, copy: &mut Option<String>) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        widgets::section_label(ui, "Required performance level (risk graph, annex A)");
        ui.horizontal(|ui| {
            ui.label("Severity");
            ui.selectable_value(&mut win.severity, Severity::S1, "S1 slight");
            ui.selectable_value(&mut win.severity, Severity::S2, "S2 serious");
            ui.label("Exposure");
            ui.selectable_value(&mut win.frequency, Frequency::F1, "F1 seldom");
            ui.selectable_value(&mut win.frequency, Frequency::F2, "F2 frequent");
            ui.label("Avoidance");
            ui.selectable_value(&mut win.avoidance, Probability::P1, "P1 possible");
            ui.selectable_value(&mut win.avoidance, Probability::P2, "P2 scarcely");
        });
        ui.strong(format!("Required: {}", pl_text(win.required())));
        widgets::section_label(ui, "Duty (for parts given by B10d)");
        ui.horizontal(|ui| {
            ui.label("Days per year");
            ui.add(egui::DragValue::new(&mut win.days).range(1.0..=366.0));
            ui.label("Hours per day");
            ui.add(egui::DragValue::new(&mut win.hours).range(0.1..=24.0));
            ui.label("Cycle s");
            ui.add(egui::DragValue::new(&mut win.cycle_s).range(0.1..=86_400.0));
        });
        ui.weak(format!("{:.0} operations per year", win.nop()));
        let nop = win.nop();
        let mut remove = None;
        for (i, s) in win.subsystems.iter_mut().enumerate() {
            subsystem(ui, i, s, nop, &mut remove);
        }
        if let Some(i) = remove {
            win.subsystems.remove(i);
        }
        if ui.button("Add subsystem").clicked() {
            let n = win.subsystems.len() + 1;
            win.subsystems
                .push(SubsystemInput::new(&format!("Subsystem {n}")));
        }
        ui.separator();
        let (_, total) = win.results();
        if let Some(t) = total {
            let ok = win.meets().unwrap_or(false);
            let color = if ok { tokens::SUCCESS } else { tokens::ERROR };
            ui.colored_label(
                color,
                format!(
                    "Series total: PFHd {:.2e} /h, achieved {} against required {}: {}",
                    t.pfhd,
                    pl_text(t.pl),
                    pl_text(win.required()),
                    if ok { "met" } else { "NOT met" }
                ),
            );
        }
        if ui.button("Copy as text").clicked() {
            *copy = Some(win.text());
        }
    });
}

fn parts(ui: &mut egui::Ui, id: (&str, usize), list: &mut Vec<PartInput>) {
    let mut remove = None;
    for (i, p) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(140.0));
            ui.selectable_value(&mut p.basis, PartBasis::Mttfd, "MTTFd (years)");
            ui.selectable_value(&mut p.basis, PartBasis::B10d, "B10d (cycles)");
            ui.add(
                egui::DragValue::new(&mut p.value)
                    .range(0.001..=1.0e9)
                    .speed(1.0),
            );
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
        let _ = id;
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui.small_button("Add part").clicked() {
        let n = list.len() + 1;
        list.push(PartInput::new(&format!("Part {n}")));
    }
}

fn subsystem(
    ui: &mut egui::Ui,
    index: usize,
    s: &mut SubsystemInput,
    nop: f64,
    remove: &mut Option<usize>,
) {
    let result = s.calculate(nop);
    let title = format!(
        "{} · category {} · {}",
        s.name,
        s.category.label(),
        pl_text(result.pl)
    );
    egui::CollapsingHeader::new(title)
        .id_salt(("safety_subsystem", index))
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(200.0));
                ui.label("Category");
                for c in [
                    Category::B,
                    Category::C1,
                    Category::C2,
                    Category::C3,
                    Category::C4,
                ] {
                    ui.selectable_value(&mut s.category, c, c.label());
                }
                if ui.small_button("Remove subsystem").clicked() {
                    *remove = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("DCavg %");
                ui.add(egui::DragValue::new(&mut s.dc_percent).range(0.0..=99.9));
                ui.label("CCF score");
                ui.add(egui::DragValue::new(&mut s.ccf).range(0..=100));
            });
            ui.label("Channel 1");
            parts(ui, ("c1", index), &mut s.channel1);
            ui.label("Channel 2 (blank: same as channel 1)");
            parts(ui, ("c2", index), &mut s.channel2);
            ui.label(format!(
                "MTTFd {:.1} years, DCavg {:?}, PFHd {:.2e} /h, {}",
                result.mttfd_years,
                result.dc,
                result.pfhd,
                pl_text(result.pl)
            ));
            for p in &result.problems {
                ui.colored_label(tokens::ERROR, &p.message);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_risk_graph_gives_the_required_level() {
        let mut w = SafetyWindow {
            severity: Severity::S2,
            frequency: Frequency::F2,
            avoidance: Probability::P2,
            ..SafetyWindow::default()
        };
        assert_eq!(w.required(), PerformanceLevel::E);
        w.severity = Severity::S1;
        w.frequency = Frequency::F1;
        w.avoidance = Probability::P1;
        assert_eq!(w.required(), PerformanceLevel::A);
    }

    #[test]
    fn b10d_parts_use_the_duty_to_get_their_mttfd() {
        let nop = SafetyWindow::default().nop();
        let w = SafetyWindow {
            subsystems: vec![SubsystemInput {
                name: "Contactor".into(),
                category: Category::C3,
                channel1: vec![PartInput {
                    name: "K1".into(),
                    basis: PartBasis::B10d,
                    value: 2_000_000.0,
                }],
                channel2: Vec::new(),
                dc_percent: 99.0,
                ccf: 70,
            }],
            ..SafetyWindow::default()
        };
        let r = w.subsystems[0].calculate(nop);
        let expected = mttfd_from_b10d(2_000_000.0, nop).min(100.0);
        assert!((r.mttfd_years - expected).abs() < 1e-6, "{r:?}");
        assert!(w.text().contains("Required PL"));
        assert!(w.text().contains("Calculation aid only"));
    }

    #[test]
    fn a_weak_subsystem_fails_the_requirement_and_says_why() {
        let w = SafetyWindow {
            severity: Severity::S2,
            frequency: Frequency::F2,
            avoidance: Probability::P2,
            subsystems: vec![SubsystemInput {
                name: "Weak".into(),
                category: Category::B,
                channel1: vec![PartInput {
                    name: "Switch".into(),
                    basis: PartBasis::Mttfd,
                    value: 4.0,
                }],
                channel2: Vec::new(),
                dc_percent: 0.0,
                ccf: 0,
            }],
            ..SafetyWindow::default()
        };
        assert_eq!(w.meets(), Some(false));
    }
}
