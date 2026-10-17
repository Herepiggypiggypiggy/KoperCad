//! "Electrical inputs" (Check menu, `check.electrical_inputs`): the project
//! numbers the standards rules and the calculations read. Without a supply
//! short-circuit current (or a transformer), a voltage-drop limit, the
//! panel's SCCR and the machine data, those rules stay "unknown" and the
//! calculations stay silent; this window is where they are entered. Every
//! entry is one undo step and is written with
//! [`novo_checks::standards::set_info_property`] under the key the checks
//! read.

use std::collections::HashMap;

use eframe::egui;
use novo_checks::calc_view::inputs as calc;
use novo_checks::standards::{self, keys};
use novo_core::{Project, Value};
use novo_theme::widgets;

use super::{Level, NovoApp};

/// One number the project states.
#[derive(Debug, Clone, Copy)]
pub(super) struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub hint: &'static str,
    /// Whole numbers only (phases).
    pub integer: bool,
}

const fn field(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    hint: &'static str,
) -> Field {
    Field {
        key,
        label,
        unit,
        hint,
        integer: false,
    }
}

/// The supply: either a stated fault current or a transformer.
pub(super) const SUPPLY: [Field; 8] = [
    field(calc::SUPPLY_VOLTAGE_V, "System voltage", "V", "400"),
    field(
        calc::SUPPLY_SHORT_CIRCUIT_KA,
        "Short-circuit current Ik (max)",
        "kA",
        "10",
    ),
    field(
        calc::SUPPLY_MIN_SHORT_CIRCUIT_KA,
        "Short-circuit current Ik (min)",
        "kA",
        "8",
    ),
    field(calc::SUPPLY_TRANSFORMER_KVA, "Transformer", "kVA", "630"),
    field(calc::SUPPLY_UK_PERCENT, "Transformer uk", "%", "4"),
    field(
        calc::SUPPLY_TRANSFORMER_PK_W,
        "Transformer load losses",
        "W",
        "6500",
    ),
    field(
        calc::SUPPLY_NETWORK_SK_MVA_MAX,
        "Network behind it, Sk (max)",
        "MVA",
        "500",
    ),
    field(
        calc::SUPPLY_NETWORK_SK_MVA_MIN,
        "Network behind it, Sk (min)",
        "MVA",
        "250",
    ),
];

pub(super) const LIMITS: [Field; 2] = [
    field(calc::VOLTAGE_DROP_PCT, "Voltage-drop limit", "%", "5"),
    field(keys::PANEL_SCCR_KA, "Panel SCCR (marked)", "kA", "10"),
];

pub(super) const MACHINE: [Field; 4] = [
    field(keys::RATED_VOLTAGE_V, "Rated supply voltage", "V", "400"),
    Field {
        key: keys::PHASES,
        label: "Phases",
        unit: "",
        hint: "3",
        integer: true,
    },
    field(keys::FREQUENCY_HZ, "Frequency", "Hz", "50"),
    field(
        keys::FULL_LOAD_CURRENT_A,
        "Full-load current of the machine",
        "A",
        "63",
    ),
];

/// A number as text for an entry box.
fn shown_text(project: &Project, key: &str) -> String {
    match project.info().properties.get(key) {
        Some(Value::Float(f)) => format!("{f}"),
        Some(Value::Int(i)) => i.to_string(),
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// Reads an entry: empty clears the property, a positive number sets it.
pub(super) fn parse_input(field: &Field, text: &str) -> Result<Option<Value>, String> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(None);
    }
    let v: f64 = text
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite() && *v > 0.0)
        .ok_or_else(|| format!("\"{text}\" is not a positive number for {}", field.label))?;
    if field.integer {
        if v.fract() != 0.0 {
            return Err(format!("{} is a whole number", field.label));
        }
        return Ok(Some(Value::Int(v as i64)));
    }
    Ok(Some(Value::Float(v)))
}

/// Three-way answer for a yes/no that may be unstated.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stated {
    Unknown,
    Yes,
    No,
}

fn estop_of(project: &Project) -> Stated {
    match project.info().properties.get(keys::ESTOP_REQUIRED) {
        Some(Value::Bool(true)) => Stated::Yes,
        Some(Value::Bool(false)) => Stated::No,
        _ => Stated::Unknown,
    }
}

/// The window's entry boxes while they are edited.
#[derive(Debug, Default)]
pub(super) struct ElectricalWindow {
    buffers: HashMap<&'static str, String>,
}

impl NovoApp {
    pub(super) fn toggle_electrical(&mut self) {
        if self.wb.electrical.take().is_none() && self.is_open() {
            self.wb.electrical = Some(ElectricalWindow::default());
        }
    }

    pub(super) fn electrical_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.electrical.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        let mut commits: Vec<(Field, String)> = Vec::new();
        let mut estop = estop_of(&project);
        let estop_before = estop;
        let mut open = true;
        let mut to_calc = false;
        super::panel::dialog(ctx, "Electrical inputs", [560.0, 560.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("The numbers the standards rules and the calculations read. A blank box is unknown: the rule says so and the calculation stays silent, nothing is guessed.");
                ui.add_space(4.0);
                let supplied = project
                    .info()
                    .properties
                    .contains_key(calc::SUPPLY_SHORT_CIRCUIT_KA)
                    || (project.info().properties.contains_key(calc::SUPPLY_TRANSFORMER_KVA)
                        && project.info().properties.contains_key(calc::SUPPLY_UK_PERCENT));
                if !supplied {
                    ui.colored_label(
                        crate::theme::ui_palette().warning,
                        "No supply stated: enter Ik, or a transformer with its uk. Breaking-capacity, disconnection and short-circuit results need it.",
                    );
                }
                widgets::section_label(ui, "Supply");
                grid(ui, "electrical_supply", &SUPPLY, &project, &mut win, &mut commits);
                ui.weak("A transformer (kVA and uk) replaces the Ik value; the network's Sk is optional.");
                widgets::section_label(ui, "Limits");
                grid(ui, "electrical_limits", &LIMITS, &project, &mut win, &mut commits);
                widgets::section_label(ui, "Machine");
                grid(ui, "electrical_machine", &MACHINE, &project, &mut win, &mut commits);
                ui.horizontal(|ui| {
                    ui.label("Emergency stop required");
                    egui::ComboBox::from_id_salt("electrical_estop")
                        .selected_text(match estop {
                            Stated::Unknown => "not stated",
                            Stated::Yes => "yes",
                            Stated::No => "no (risk assessment says so)",
                        })
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                (Stated::Unknown, "not stated"),
                                (Stated::Yes, "yes"),
                                (Stated::No, "no (risk assessment says so)"),
                            ] {
                                if ui.selectable_label(estop == value, label).clicked() {
                                    estop = value;
                                }
                            }
                        });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Open the calculations").clicked() {
                        to_calc = true;
                    }
                    ui.weak("Installation method and ambient temperature are in Standards check.");
                });
            });
        for (field, text) in commits {
            self.apply_electrical_input(&field, &text);
        }
        if estop != estop_before {
            let value = match estop {
                Stated::Unknown => None,
                Stated::Yes => Some(Value::Bool(true)),
                Stated::No => Some(Value::Bool(false)),
            };
            self.edit_project("Emergency stop requirement", |tx| {
                standards::set_info_property(tx, keys::ESTOP_REQUIRED, value)
            });
        }
        if to_calc {
            self.toggle_calculations();
        }
        if open {
            self.wb.electrical = Some(win);
        }
    }

    /// Writes one entry as one undo step; a refused entry is shown.
    pub(super) fn apply_electrical_input(&mut self, field: &Field, text: &str) -> bool {
        match parse_input(field, text) {
            Ok(value) => self.edit_project(&format!("Set {}", field.label), |tx| {
                standards::set_info_property(tx, field.key, value)
            }),
            Err(why) => {
                self.message(Level::Warning, why);
                false
            }
        }
    }
}

/// A labelled grid of entry boxes. A box is committed when it loses focus
/// with changed text; while it has focus it keeps what is typed.
fn grid(
    ui: &mut egui::Ui,
    id: &str,
    fields: &[Field],
    project: &Project,
    win: &mut ElectricalWindow,
    commits: &mut Vec<(Field, String)>,
) {
    egui::Grid::new(id).num_columns(3).show(ui, |ui| {
        for f in fields {
            let shown = shown_text(project, f.key);
            let edit_id = egui::Id::new(("electrical_input", f.key));
            let focused = ui.memory(|m| m.has_focus(edit_id));
            let buffer = win.buffers.entry(f.key).or_default();
            if !focused {
                buffer.clone_from(&shown);
            }
            ui.label(f.label);
            let response = ui
                .add(
                    egui::TextEdit::singleline(buffer)
                        .id(edit_id)
                        .hint_text(f.hint)
                        .desired_width(90.0),
                )
                .on_hover_text(f.key);
            ui.weak(f.unit);
            if response.lost_focus() && buffer.trim() != shown.trim() {
                commits.push((*f, buffer.clone()));
            }
            ui.end_row();
        }
    });
}
