//! The wire panel: what the inspector shows when wires are selected. The
//! colour and cross-section are the facts the standards checks read; the
//! installation method, insulation and grouping are what a conductor's
//! current-carrying capacity depends on (IEC 60364-5-52). Each edit applies
//! to every selected wire and is one undo step.

use eframe::egui;
use novo_checks::standards::keys;
use novo_core::{Project, Tx, Value, Wire, WireId};
use novo_theme::widgets;

use super::{Level, NovoApp};
use crate::app::inspector::INSPECTOR_WIDTH;

/// What the panel shows for one wire.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct WireFacts {
    pub number: String,
    pub colour: String,
    pub section: String,
    /// `B1`, `B2`, `C`, or empty for none stated.
    pub method: String,
    /// `PVC`, `XLPE`, or empty for the default.
    pub insulation: String,
    pub grouped: String,
}

fn text_of(value: Option<&Value>) -> String {
    match value {
        Some(Value::Text(t)) => t.clone(),
        Some(Value::Int(i)) => i.to_string(),
        Some(Value::Float(f)) => f.to_string(),
        _ => String::new(),
    }
}

/// The facts of a wire as the model holds them (its own properties; a cable
/// core's values apply to the checks when the wire states none).
pub(super) fn facts(project: &Project, wire: WireId) -> Option<WireFacts> {
    let w = project.wire(wire)?;
    Some(WireFacts {
        number: w.number.clone().unwrap_or_default(),
        colour: w.color.clone().unwrap_or_default(),
        section: w
            .cross_section_mm2
            .map(|s| s.to_string())
            .unwrap_or_default(),
        method: text_of(w.properties.get(keys::INSTALLATION_METHOD)),
        insulation: text_of(w.properties.get(keys::INSULATION)),
        grouped: text_of(w.properties.get(keys::GROUPED_CIRCUITS)),
    })
}

/// One edit of the panel.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum WireEdit {
    Colour(String),
    /// Cross-section in mm²; empty clears it.
    Section(String),
    Method(String),
    Insulation(String),
    Grouped(String),
}

/// Applies an edit to wires. Errors name what could not be read.
pub(super) fn apply(tx: &mut Tx<'_>, wires: &[WireId], edit: &WireEdit) -> Result<(), String> {
    for &wire in wires {
        match edit {
            WireEdit::Colour(text) => {
                let text = text.trim().to_owned();
                tx.update::<Wire>(wire, |w| w.color = (!text.is_empty()).then(|| text.clone()))
                    .map_err(|e| e.to_string())?;
            }
            WireEdit::Section(text) => {
                let text = text.trim().replace(',', ".");
                let value = if text.is_empty() {
                    None
                } else {
                    match text.parse::<f64>() {
                        Ok(v) if v > 0.0 && v.is_finite() => Some(v),
                        _ => return Err(format!("\"{text}\" is not a cross-section in mm²")),
                    }
                };
                tx.update::<Wire>(wire, |w| w.cross_section_mm2 = value)
                    .map_err(|e| e.to_string())?;
            }
            WireEdit::Method(text) => set_text(tx, wire, keys::INSTALLATION_METHOD, text)?,
            WireEdit::Insulation(text) => set_text(tx, wire, keys::INSULATION, text)?,
            WireEdit::Grouped(text) => {
                let text = text.trim();
                let value = if text.is_empty() {
                    None
                } else {
                    match text.parse::<i64>() {
                        Ok(n) if n >= 1 => Some(Value::Int(n)),
                        _ => return Err(format!("\"{text}\" is not a number of circuits")),
                    }
                };
                tx.set_property(wire, keys::GROUPED_CIRCUITS, value)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn set_text(tx: &mut Tx<'_>, wire: WireId, key: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    tx.set_property(
        wire,
        key,
        (!text.is_empty()).then(|| Value::Text(text.to_owned())),
    )
    .map_err(|e| e.to_string())
}

/// The panel's text fields while they are edited.
#[derive(Debug, Default)]
pub(super) struct WirePanel {
    pub(super) wire: Option<WireId>,
    pub(super) number: String,
    colour: String,
    section: String,
    grouped: String,
}

const METHODS: [(&str, &str); 4] = [
    ("", "Not stated"),
    ("B1", "B1: single cores in conduit"),
    ("B2", "B2: multicore in conduit"),
    ("C", "C: clipped or on a tray"),
];

/// Whether leaving a field commits: only when its text really changed, so
/// tabbing through never overwrites other selected wires or makes an empty
/// undo step.
pub(super) fn should_commit(lost_focus: bool, value: &str, shown: &str) -> bool {
    lost_focus && value.trim() != shown.trim()
}

/// A labelled text field whose edit is committed when it loses focus.
#[allow(clippy::too_many_arguments)]
fn text_row(
    ui: &mut egui::Ui,
    edits: &mut Vec<WireEdit>,
    label: &str,
    id: &str,
    value: &mut String,
    shown: &str,
    hint: &str,
    edit: fn(String) -> WireEdit,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let r = ui.add(
            egui::TextEdit::singleline(value)
                .id(egui::Id::new(id))
                .hint_text(hint)
                .desired_width(130.0),
        );
        if should_commit(r.lost_focus(), value, shown) {
            edits.push(edit(value.clone()));
        }
    });
}

impl NovoApp {
    /// Draws the wire panel for the selected wires; false when no wire is
    /// selected.
    pub(super) fn wire_panel(&mut self, ctx: &egui::Context, canvas: egui::Rect) -> bool {
        let wires = self.selected_wires();
        let Some(&first) = wires.first() else {
            return false;
        };
        let Some(shown) = self
            .backend
            .document()
            .and_then(|d| facts(d.project(), first))
        else {
            return false;
        };
        let panel = &mut self.wb.wire_panel;
        if panel.wire != Some(first) {
            *panel = WirePanel {
                wire: Some(first),
                ..WirePanel::default()
            };
        }
        let locked = self
            .backend
            .document()
            .is_some_and(|d| novo_numbering::is_locked(d.project(), first));
        let panel = &mut self.wb.wire_panel;
        let focused = ctx.memory(|m| m.focused());
        for (id, field, value) in [
            ("wire_number", &mut panel.number, &shown.number),
            ("wire_colour", &mut panel.colour, &shown.colour),
            ("wire_section", &mut panel.section, &shown.section),
            ("wire_grouped", &mut panel.grouped, &shown.grouped),
        ] {
            if focused != Some(egui::Id::new(id)) {
                field.clone_from(value);
            }
        }
        let mut edits: Vec<WireEdit> = Vec::new();
        let mut number = panel.number.clone();
        let mut number_edit: Option<Option<String>> = None;
        let (mut colour, mut section, mut grouped) = (
            panel.colour.clone(),
            panel.section.clone(),
            panel.grouped.clone(),
        );
        let mut method = shown.method.clone();
        let mut insulation = shown.insulation.clone();
        super::panel::Panel::new(
            "inspector",
            "Inspector",
            canvas,
            super::panel::Side::Right,
            INSPECTOR_WIDTH,
        )
        .fixed_open()
        .show(ctx, |ui| {
                    ui.heading(if wires.len() > 1 {
                        format!("{} wires", wires.len())
                    } else if shown.number.is_empty() {
                        "Wire".to_owned()
                    } else {
                        format!("Wire {}", shown.number)
                    });
                    if wires.len() == 1 {
                        widgets::section_label(ui, "Number");
                        ui.horizontal(|ui| {
                            ui.label("Wire number");
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut number)
                                    .id(egui::Id::new("wire_number"))
                                    .hint_text("105")
                                    .desired_width(90.0),
                            );
                            if should_commit(r.lost_focus(), &number, &shown.number) {
                                number_edit = Some(Some(number.clone()));
                            }
                            if locked
                                && ui
                                    .small_button("Unlock")
                                    .on_hover_text("Numbering runs may change this number again")
                                    .clicked()
                            {
                                number_edit = Some(None);
                            }
                        });
                        ui.weak(if locked {
                            "Fixed by hand: numbering runs leave it alone."
                        } else {
                            "Type a number to fix it; numbering runs then leave it alone."
                        });
                    }
                    widgets::section_label(ui, "Conductor");
                    text_row(ui, &mut edits, "Colour", "wire_colour", &mut colour, &shown.colour, "BK, BU, GNYE", WireEdit::Colour);
                    text_row(ui, &mut edits, "Section mm²", "wire_section", &mut section, &shown.section, "2.5", WireEdit::Section);
                    widgets::section_label(ui, "Current-carrying capacity");
                    ui.horizontal(|ui| {
                        ui.label("Installation");
                        egui::ComboBox::from_id_salt("wire_method")
                            .selected_text(
                                METHODS
                                    .iter()
                                    .find(|(k, _)| *k == method)
                                    .map_or(method.as_str(), |(_, l)| *l),
                            )
                            .show_ui(ui, |ui| {
                                for (key, label) in METHODS {
                                    if ui.selectable_label(method == key, label).clicked() {
                                        method = key.to_owned();
                                        edits.push(WireEdit::Method(method.clone()));
                                    }
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Insulation");
                        egui::ComboBox::from_id_salt("wire_insulation")
                            .selected_text(if insulation.is_empty() { "PVC (default)" } else { insulation.as_str() })
                            .show_ui(ui, |ui| {
                                for (key, label) in [("", "PVC (default)"), ("XLPE", "XLPE")] {
                                    if ui.selectable_label(insulation == key, label).clicked() {
                                        insulation = key.to_owned();
                                        edits.push(WireEdit::Insulation(insulation.clone()));
                                    }
                                }
                            });
                    });
                    text_row(ui, &mut edits, "Grouped circuits", "wire_grouped", &mut grouped, &shown.grouped, "1", WireEdit::Grouped);
                    ui.weak("The standards check reads these. A wire with no installation method is not rated, never guessed; set a default for the project in Standards check.");


            });
        let panel = &mut self.wb.wire_panel;
        (panel.number, panel.colour, panel.section, panel.grouped) =
            (number, colour, section, grouped);
        match number_edit {
            Some(Some(text)) => self.set_wire_number(Some(&text)),
            Some(None) => self.unlock_wire_number(),
            None => {}
        }
        for edit in edits {
            let Some(doc) = self.backend.document_mut() else {
                break;
            };
            match doc.transact("Edit wires", |tx| {
                apply(tx, &wires, &edit).map_err(novo_core::ModelError::Format)
            }) {
                Ok(((), changes)) => self.changed_outside_canvas(&changes),
                Err(e) => self.message(Level::Warning, format!("Wire: {e}")),
            }
        }
        true
    }
}
