//! Cable controls: give a cable its part, its length and its shield. They
//! sit above the core table in the Cables drawer (Ctrl+4): pick a cable in
//! the list and its Part, Length and Shield rows appear. Every change is one
//! undo step, and the cable checks that concern the cable are listed under
//! the controls.
//!
//! The shield is connected at the cabinet end (to PE or to a terminal),
//! which is what the model records (`novo_cables::connect_shield`); the cable
//! screen shows it in the cabinet column.

use eframe::egui;
use novo_cables::ShieldConnection;
use novo_core::{Cable, CableId, Project, TerminalId};
use novo_parts::PartKey;

use super::part_picker::{Candidate, PartPicker, picker_ui};
use super::{Level, NovoApp};

/// The cable controls' fields while they are edited.
#[derive(Debug, Default)]
pub(super) struct CableEdit {
    cable: Option<CableId>,
    length: String,
    pub(super) picker: PartPicker,
    /// Name, type and proposal choices (cable_cores.rs).
    pub(super) ui: super::cable_cores::CableUi,
}

/// What a click or edit in the cable controls asks for.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum CableAction {
    AssignPart(PartKey),
    SetLength(Option<f64>),
    /// Take the length from the longest routed wire on the cable.
    EstimateLength,
    AddShield,
    ConnectShield {
        core: usize,
        to: Option<ShieldConnection>,
    },
    /// Put a wire on a core, or free the core with `None`.
    SetCoreWire {
        core: usize,
        wire: Option<novo_core::WireId>,
    },
    SwapCores(usize, usize),
    SetSpare {
        core: usize,
        spare: bool,
    },
    Rename(String),
    Delete,
    Retype(novo_cables::CableType),
}

/// A length typed as `12`, `12.5` or `12,5` metres. Empty clears it.
pub(super) fn parse_length(text: &str) -> Result<Option<f64>, &'static str> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() && v > 0.0 => Ok(Some(v)),
        _ => Err("Type the length in metres, for example 12.5"),
    }
}

/// The longest routed wire among the cable's cores, in metres rounded up to
/// 0.1: the part of the cable that runs inside the cabinet. `None` when no
/// core wire is routed yet.
pub(super) fn routed_length_m(project: &Project, cable: CableId) -> Option<f64> {
    let wires: Vec<u64> = project
        .cable(cable)?
        .cores
        .iter()
        .filter_map(|c| c.wire)
        .map(|w| w.raw())
        .collect();
    if wires.is_empty() {
        return None;
    }
    let routed = novo_routing::model::route_project(project);
    let mm = routed
        .plans
        .iter()
        .flat_map(|(_, plan)| plan.routes.iter())
        .filter(|r| wires.contains(&r.job))
        .map(|r| r.length_mm)
        .fold(None, |m: Option<f64>, l| Some(m.map_or(l, |m| m.max(l))))?;
    Some((mm / 100.0).ceil() / 10.0)
}

fn connection_text(project: &Project, connection: Option<ShieldConnection>) -> String {
    match connection {
        None => "not connected".into(),
        Some(ShieldConnection::Pe) => "PE".into(),
        Some(ShieldConnection::Terminal(t)) => project
            .terminal_designation(t)
            .unwrap_or_else(|| "a terminal".into()),
        Some(ShieldConnection::Wire(_)) => "a drawn wire".into(),
    }
}

/// Draws the Part, Length and Shield rows for `cable`.
pub(super) fn cable_controls(
    ui: &mut egui::Ui,
    project: &Project,
    cable: CableId,
    edit: &mut CableEdit,
    candidates: &[Candidate],
    actions: &mut Vec<CableAction>,
) {
    let Some(record) = project.cable(cable) else {
        return;
    };
    if edit.cable != Some(cable) {
        edit.cable = Some(cable);
        edit.length.clear();
        edit.picker = PartPicker::default();
    }
    let length_id = egui::Id::new(("cable_length", cable.raw()));
    if ui.ctx().memory(|m| m.focused()) != Some(length_id) {
        edit.length = record.length_m.map(|l| l.to_string()).unwrap_or_default();
    }

    let part = record
        .part
        .and_then(|p| project.part(p))
        .map(|p| format!("{} {}", p.manufacturer, p.order_number));
    ui.horizontal(|ui| {
        ui.label("Part");
        match &part {
            Some(text) => ui.strong(text),
            None => ui.weak("none"),
        };
        let label = if edit.picker.open {
            "Close"
        } else if part.is_some() {
            "Change..."
        } else {
            "Choose..."
        };
        if ui
            .small_button(label)
            .on_hover_text("Pick the cable part: it sets the cores and the cross-section")
            .clicked()
        {
            edit.picker.toggle();
        }
    });
    if edit.picker.open
        && let Some(key) = picker_ui(
            ui,
            &mut edit.picker,
            candidates,
            "No cable part in the catalog matches. Cable parts list their cores.",
        )
    {
        actions.push(CableAction::AssignPart(key));
        edit.picker.close();
    }

    ui.horizontal(|ui| {
        ui.label("Length (m)").on_hover_text(
            "The whole cable as bought, between its two ends. Each core is cut at this length plus a service loop at each end, and the wire list counts it as exact. If a connection has wires on several cables, the longest wins.",
        );
        let field = ui.add(
            egui::TextEdit::singleline(&mut edit.length)
                .id(length_id)
                .desired_width(70.0)
                .hint_text("12.5"),
        );
        let committed = field.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape));
        if committed {
            match parse_length(&edit.length) {
                Ok(length) if length != record.length_m => {
                    actions.push(CableAction::SetLength(length));
                }
                Ok(_) => {}
                Err(_) => edit.length = record.length_m.map(|l| l.to_string()).unwrap_or_default(),
            }
        }
        if ui
            .small_button("From layout")
            .on_hover_text(
                "The longest routed core wire (the run inside the cabinet; add the run outside by hand)",
            )
            .clicked()
        {
            actions.push(CableAction::EstimateLength);
        }
    });

    let shields: Vec<usize> = record
        .cores
        .iter()
        .enumerate()
        .filter(|(_, c)| c.shield)
        .map(|(i, _)| i)
        .collect();
    ui.horizontal(|ui| {
        ui.label("Shield");
        if shields.is_empty() {
            ui.weak("none");
        }
        if ui
            .small_button("Add shield")
            .on_hover_text("Adds a shield core (SH); connect it to PE or a shield terminal")
            .clicked()
        {
            actions.push(CableAction::AddShield);
        }
    });
    for core in shields {
        shield_row(ui, project, record, core, actions);
    }

    let findings: Vec<String> = novo_cables::check(project)
        .into_iter()
        .filter(|f| f.cable() == cable)
        .map(|f| f.message(project))
        .collect();
    for text in findings {
        ui.colored_label(crate::theme::ui_palette().warning, text);
    }
    ui.add_space(4.0);
}

fn shield_row(
    ui: &mut egui::Ui,
    project: &Project,
    record: &Cable,
    core: usize,
    actions: &mut Vec<CableAction>,
) {
    let name = record.cores[core].name.clone();
    let current = novo_cables::shield_connection(record, core);
    ui.horizontal(|ui| {
        ui.label(format!("{name} at the cabinet end"));
        egui::ComboBox::from_id_salt(("shield_to", record.id.raw(), core))
            .selected_text(connection_text(project, current))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), "not connected")
                    .clicked()
                {
                    actions.push(CableAction::ConnectShield { core, to: None });
                }
                if ui
                    .selectable_label(current == Some(ShieldConnection::Pe), "PE")
                    .clicked()
                {
                    actions.push(CableAction::ConnectShield {
                        core,
                        to: Some(ShieldConnection::Pe),
                    });
                }
                let terminals: Vec<(TerminalId, String)> = project
                    .terminals()
                    .filter_map(|t| Some((t.id, project.terminal_designation(t.id)?)))
                    .collect();
                for (id, text) in terminals {
                    let on = current == Some(ShieldConnection::Terminal(id));
                    if ui.selectable_label(on, text).clicked() {
                        actions.push(CableAction::ConnectShield {
                            core,
                            to: Some(ShieldConnection::Terminal(id)),
                        });
                    }
                }
            });
    });
}

impl NovoApp {
    /// Runs one change to a cable as an undo step of its own.
    pub(super) fn change_cable<R>(
        &mut self,
        label: String,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<R>,
    ) -> Option<R> {
        let doc = self.backend.document_mut()?;
        match doc.transact(label, edit) {
            Ok((value, changes)) => {
                self.changed_outside_canvas(&changes);
                Some(value)
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not change the cable: {e}"));
                None
            }
        }
    }

    pub(super) fn cable_action(&mut self, cable: CableId, action: CableAction) {
        let Some(tag) = self
            .backend
            .document()
            .and_then(|d| d.project().cable(cable))
            .map(|c| c.tag.to_string())
        else {
            return;
        };
        match action {
            CableAction::AssignPart(key) => self.assign_cable_part(cable, &key),
            CableAction::SetLength(length) => {
                self.change_cable(format!("Set the length of {tag}"), |tx| {
                    tx.update::<Cable>(cable, |c| c.length_m = length)
                });
            }
            CableAction::EstimateLength => {
                let routed = self
                    .backend
                    .document()
                    .and_then(|d| routed_length_m(d.project(), cable));
                match routed {
                    Some(length) => self.cable_action(cable, CableAction::SetLength(Some(length))),
                    None => self.message(
                        Level::Info,
                        format!(
                            "No wire of {tag} is routed yet. Place its devices in the Cabinet and use Route all."
                        ),
                    ),
                }
            }
            CableAction::AddShield => {
                self.change_cable(format!("Add a shield to {tag}"), |tx| {
                    novo_cables::add_shield(tx, cable)
                });
            }
            CableAction::ConnectShield { core, to } => {
                self.change_cable(format!("Connect the shield of {tag}"), |tx| {
                    novo_cables::connect_shield(tx, cable, core, to)
                });
            }
            other => self.cable_edit_action(cable, &tag, other),
        }
    }

    fn assign_cable_part(&mut self, cable: CableId, key: &PartKey) {
        let Some(command) = self.backend.document().and_then(|d| {
            let part = self.wb.part_catalog.get()?.get(key)?;
            novo_parts::assign_cable_part(d.project(), cable, part)
        }) else {
            return;
        };
        if let Some(Err(e)) = self.execute(command) {
            self.message(Level::Error, format!("Could not set the cable part: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceTag, Document};

    fn cable_project() -> (Project, CableId) {
        let mut project = Project::new("Plant");
        let (cable, _) = project
            .transact("Add", |tx| {
                tx.add_cable(DeviceTag::parse("-W1").unwrap(), Vec::new())
            })
            .unwrap();
        (project, cable)
    }

    #[test]
    fn lengths_accept_a_comma_and_refuse_nonsense() {
        assert_eq!(parse_length("12,5"), Ok(Some(12.5)));
        assert_eq!(parse_length(" 3 "), Ok(Some(3.0)));
        assert_eq!(parse_length(""), Ok(None));
        assert!(parse_length("0").is_err());
        assert!(parse_length("-2").is_err());
        assert!(parse_length("abc").is_err());
    }

    #[test]
    fn a_shield_is_added_and_connected_with_one_undo_step_each() {
        let (project, cable) = cable_project();
        let mut doc = Document::new(project);
        let (core, _) = doc
            .transact("Add a shield", |tx| novo_cables::add_shield(tx, cable))
            .unwrap();
        let depth = doc.undo_depth();
        doc.transact("Connect", |tx| {
            novo_cables::connect_shield(tx, cable, core, Some(ShieldConnection::Pe))
        })
        .unwrap();
        assert_eq!(doc.undo_depth(), depth + 1);
        let record = doc.project().cable(cable).unwrap();
        assert_eq!(
            novo_cables::shield_connection(record, core),
            Some(ShieldConnection::Pe)
        );
        doc.undo().unwrap();
        let record = doc.project().cable(cable).unwrap();
        assert_eq!(novo_cables::shield_connection(record, core), None);
        assert!(
            novo_cables::check(doc.project())
                .iter()
                .any(|f| matches!(f, novo_cables::Finding::ShieldUnconnected { .. })),
            "an unconnected shield is reported"
        );
    }

    #[test]
    fn a_cable_part_sets_the_cores_in_one_step_and_a_length_is_kept() {
        let (project, cable) = cable_project();
        let mut part = novo_parts::CatalogPart::new("Lapp", "1119304");
        part.data.insert(
            novo_parts::keys::CORES.into(),
            novo_core::Value::Text("BN,BK,GY,GNYE".into()),
        );
        let part = &part;
        let mut doc = Document::new(project);
        let depth = doc.undo_depth();
        let command = novo_parts::assign_cable_part(doc.project(), cable, part).unwrap();
        doc.execute(command).unwrap();
        assert_eq!(doc.undo_depth(), depth + 1);
        let record = doc.project().cable(cable).unwrap();
        assert!(record.part.is_some());
        assert_eq!(record.cores.len(), part.cores().len());
        doc.transact("Length", |tx| {
            tx.update::<Cable>(cable, |c| c.length_m = Some(12.0))
        })
        .unwrap();
        assert_eq!(doc.project().cable(cable).unwrap().length_m, Some(12.0));
        assert_eq!(
            routed_length_m(doc.project(), cable),
            None,
            "no wires, no estimate"
        );
    }
}
