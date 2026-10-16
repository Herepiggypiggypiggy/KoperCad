//! The "Project safety functions" part of the Safety window (`check.safety`,
//! Check > Safety functions): sensor, logic and actuator blocks whose parts
//! are bound to devices of the project by tag. B10d or MTTFd is typed in or
//! taken from the bound device (properties `b10d_cycles`, `mttfd_years`).
//! The achieved PL against the required one is shown here and raised as
//! Problems in the Issues list when it falls short.
//!
//! Calculation aid, not a certification: see the disclaimer in the window.

use eframe::egui;
use novo_checks::safety::{
    B10D_CYCLES, Basis, Block, FUNCTIONS_KEY, MTTFD_YEARS, PartRef, Rating, Role, SafetyFunction,
    encode, functions, rate,
};
use novo_core::Project;
use novo_theme::{tokens, widgets};

use super::NovoApp;

/// The working copy of the stored functions.
#[derive(Debug, Default)]
pub(super) struct BoundFunctions {
    pub list: Vec<SafetyFunction>,
    pub selected: usize,
    /// Edited and not yet written to the project.
    pub dirty: bool,
}

impl BoundFunctions {
    /// Takes the stored functions again when nothing is being edited, so
    /// undo and redo show up in the window.
    pub(super) fn sync(&mut self, project: &Project) {
        if !self.dirty {
            let stored = functions(project);
            if stored != self.list {
                self.list = stored;
            }
        }
        if self.selected >= self.list.len() {
            self.selected = self.list.len().saturating_sub(1);
        }
    }
}

/// Text of one rating for "Copy as text".
pub(super) fn rating_text(rating: &Rating) -> String {
    use std::fmt::Write as _;
    let mut out = format!(
        "Safety function \"{}\": required {}\n",
        rating.name, rating.required
    );
    for b in &rating.blocks {
        match &b.result {
            Some(r) => {
                let _ = writeln!(
                    out,
                    "  {} {}: PFHd {:.2e} /h, {}",
                    b.role.label(),
                    b.name,
                    r.pfhd,
                    r.pl
                );
            }
            None => {
                let _ = writeln!(out, "  {} {}: not rated", b.role.label(), b.name);
            }
        }
        for text in &b.binding {
            let _ = writeln!(out, "    {text}");
        }
    }
    match &rating.total {
        Some(t) => {
            let _ = writeln!(
                out,
                "  Total PFHd {:.2e} /h, achieved {}: {}",
                t.pfhd,
                t.pl,
                if rating.met() == Some(true) {
                    "met"
                } else {
                    "NOT met"
                }
            );
        }
        None => {
            let _ = writeln!(out, "  Not rated until every part has data and a device");
        }
    }
    out
}

/// Draws the section. `copy` receives the text for the clipboard.
pub(super) fn section(
    ui: &mut egui::Ui,
    project: &Project,
    state: &mut BoundFunctions,
    copy: &mut Option<String>,
) {
    widgets::section_label(ui, "Project safety functions");
    ui.weak(format!(
        "Bound to devices by tag; stored in the project ({FUNCTIONS_KEY}) and listed in Issues when the level is not met. Set {B10D_CYCLES} or {MTTFD_YEARS} on a device or its part to take the number from there."
    ));
    ui.horizontal(|ui| {
        for (i, f) in state.list.iter().enumerate() {
            if ui.selectable_label(state.selected == i, &f.name).clicked() {
                state.selected = i;
            }
        }
        if ui.button("New safety function").clicked() {
            let n = state.list.len() + 1;
            state
                .list
                .push(SafetyFunction::new(format!("Safety function {n}")));
            state.selected = state.list.len() - 1;
            state.dirty = true;
        }
    });
    let devices: Vec<String> = {
        let mut v: Vec<String> = project.devices().map(|d| d.tag.to_string()).collect();
        v.sort();
        v.dedup();
        v
    };
    let Some(f) = state.list.get_mut(state.selected) else {
        ui.weak("No safety function yet.");
        return;
    };
    let rating = rate(project, f);
    let mut changed = false;
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.label("Name");
        changed |= ui
            .add(egui::TextEdit::singleline(&mut f.name).desired_width(200.0))
            .changed();
        if ui.small_button("Delete function").clicked() {
            remove = true;
        }
    });
    ui.horizontal(|ui| {
        changed |= choice(ui, "Severity", &mut f.severity, &["S1", "S2"]);
        changed |= choice(ui, "Exposure", &mut f.frequency, &["F1", "F2"]);
        changed |= choice(ui, "Avoidance", &mut f.avoidance, &["P1", "P2"]);
    });
    ui.horizontal(|ui| {
        ui.label("Days per year");
        changed |= ui
            .add(egui::DragValue::new(&mut f.days).range(1.0..=366.0))
            .changed();
        ui.label("Hours per day");
        changed |= ui
            .add(egui::DragValue::new(&mut f.hours).range(0.1..=24.0))
            .changed();
        ui.label("Cycle s");
        changed |= ui
            .add(egui::DragValue::new(&mut f.cycle_s).range(0.1..=86_400.0))
            .changed();
    });
    for (i, block) in f.blocks.iter_mut().enumerate() {
        let shown = rating.blocks.get(i);
        let title = format!(
            "{} · {} · {}",
            block.role.label(),
            block.name,
            shown
                .and_then(|b| b.result.as_ref())
                .map_or("not rated".to_owned(), |r| r.pl.to_string())
        );
        egui::CollapsingHeader::new(title)
            .id_salt(("safety_block", i))
            .default_open(true)
            .show(ui, |ui| {
                changed |= block_ui(ui, i, block, &devices);
                if let Some(b) = shown {
                    for text in &b.binding {
                        ui.colored_label(tokens::WARNING, text);
                    }
                }
            });
    }
    ui.horizontal(|ui| {
        for role in [Role::Sensor, Role::Logic, Role::Actuator] {
            if ui
                .small_button(format!("Add {}", role.label().to_lowercase()))
                .clicked()
            {
                f.blocks.push(Block::new(role, role.label()));
                changed = true;
            }
        }
    });
    match &rating.total {
        Some(t) => {
            let ok = rating.met() == Some(true);
            ui.colored_label(
                if ok { tokens::SUCCESS } else { tokens::ERROR },
                format!(
                    "Achieved {} (PFHd {:.2e} /h) against required {}: {}",
                    t.pl,
                    t.pfhd,
                    rating.required,
                    if ok { "met" } else { "NOT met" }
                ),
            );
        }
        None => {
            ui.colored_label(
                tokens::WARNING,
                format!(
                    "Required {}. Not rated until every part has a device and data.",
                    rating.required
                ),
            );
        }
    }
    if ui.button("Copy function as text").clicked() {
        *copy = Some(rating_text(&rating));
    }
    if remove {
        let at = state.selected;
        state.list.remove(at);
        changed = true;
    }
    state.dirty |= changed;
}

fn choice(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) -> bool {
    let mut changed = false;
    ui.label(label);
    for o in options {
        if ui.selectable_label(value == o, *o).clicked() && value != o {
            *value = (*o).to_owned();
            changed = true;
        }
    }
    changed
}

fn block_ui(ui: &mut egui::Ui, index: usize, block: &mut Block, devices: &[String]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Name");
        changed |= ui
            .add(egui::TextEdit::singleline(&mut block.name).desired_width(160.0))
            .changed();
        ui.label("Category");
        changed |= choice(ui, "", &mut block.category, &["B", "1", "2", "3", "4"]);
    });
    ui.horizontal(|ui| {
        ui.label("DCavg %");
        changed |= ui
            .add(egui::DragValue::new(&mut block.dc_percent).range(0.0..=99.9))
            .changed();
        ui.label("CCF score");
        changed |= ui
            .add(egui::DragValue::new(&mut block.ccf).range(0..=100))
            .changed();
    });
    ui.label("Channel 1");
    changed |= parts_ui(ui, ("c1", index), &mut block.channel1, devices);
    ui.label("Channel 2 (blank: same as channel 1)");
    changed |= parts_ui(ui, ("c2", index), &mut block.channel2, devices);
    changed
}

fn parts_ui(
    ui: &mut egui::Ui,
    id: (&str, usize),
    list: &mut Vec<PartRef>,
    devices: &[String],
) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (i, p) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::TextEdit::singleline(&mut p.name).desired_width(110.0))
                .changed();
            let bound = p.device.clone().unwrap_or_default();
            egui::ComboBox::from_id_salt((id.0, id.1, i, "device"))
                .selected_text(if bound.is_empty() {
                    "no device".to_owned()
                } else {
                    bound.clone()
                })
                .width(110.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(bound.is_empty(), "no device").clicked() {
                        p.device = None;
                        changed = true;
                    }
                    for d in devices {
                        if ui.selectable_label(&bound == d, d).clicked() {
                            p.device = Some(d.clone());
                            changed = true;
                        }
                    }
                });
            for (b, label) in [(Basis::B10d, "B10d"), (Basis::Mttfd, "MTTFd")] {
                if ui.selectable_label(p.basis == b, label).clicked() && p.basis != b {
                    p.basis = b;
                    changed = true;
                }
            }
            let mut own = p.value.is_some();
            if ui.checkbox(&mut own, "own value").changed() {
                p.value = own.then_some(0.0);
                changed = true;
            }
            if let Some(v) = p.value.as_mut() {
                changed |= ui
                    .add(
                        egui::DragValue::new(v)
                            .range(0.0..=1.0e9)
                            .speed(1.0)
                            .custom_formatter(|n, _| {
                                if n > 0.0 {
                                    format!("{n}")
                                } else {
                                    "enter a value".to_owned()
                                }
                            })
                            .custom_parser(|t| t.trim().replace(',', ".").parse().ok()),
                    )
                    .changed();
            } else {
                ui.weak("from device");
            }
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
        changed = true;
    }
    if ui.small_button("Add part").clicked() {
        let n = list.len() + 1;
        list.push(PartRef::new(format!("Part {n}")));
        changed = true;
    }
    changed
}

impl NovoApp {
    /// Writes the edited functions into the project as one undo step, once
    /// nothing is being typed or dragged.
    pub(super) fn commit_safety_functions(&mut self, ctx: &egui::Context) {
        let Some(win) = self.wb.safety.as_ref() else {
            return;
        };
        if !win.bound.dirty
            || ctx.input(|i| i.pointer.any_down())
            || ctx.memory(|m| m.focused()).is_some()
        {
            return;
        }
        let list = win.bound.list.clone();
        let ok = self.store_safety_functions(&list);
        if let Some(win) = self.wb.safety.as_mut() {
            win.bound.dirty = !ok;
        }
    }

    /// Writes `list` into the project now. Used when the window closes with
    /// an edit not yet written, so a close never loses it.
    pub(super) fn store_safety_functions(&mut self, list: &[SafetyFunction]) -> bool {
        let value = encode(list);
        self.edit_project("Safety functions", |tx| {
            novo_checks::standards::set_info_property(tx, FUNCTIONS_KEY, value)
        })
    }
}
