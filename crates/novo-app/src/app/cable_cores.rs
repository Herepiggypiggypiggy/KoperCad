//! Editing an existing cable from the Cables drawer (Ctrl+4): its name, its
//! type, its cores (which wire is on which core, swapping two cores, spare
//! cores), deleting it, and the cable proposals (accept one with a type of
//! your choice, decline it, restore the declined ones).
//!
//! Every change is one undo step, and the commands `cable.rename`,
//! `cable.delete`, `cable.retype` and `cable.restore_proposals` run the same
//! code from the Draw menu and the command field.

use std::collections::BTreeMap;

use eframe::egui;
use novo_cables::{CableProposal, CableType};
use novo_core::{AnyId, Cable, CableId, DeviceTag, PageId, Project, WireId};

use super::cable_controls::{CableAction, CableEdit};
use super::drawers::DrawerAction;
use super::workbench::Drawer;
use super::{Level, NovoApp};

/// What the drawer remembers between frames about cable editing.
#[derive(Debug, Default)]
pub(super) struct CableUi {
    /// The name being typed.
    pub(super) name: String,
    /// The cable the name field belongs to.
    name_for: Option<CableId>,
    /// Put the cursor in the name field on the next frame (`cable.rename`).
    pub(super) focus_name: bool,
    /// The type picked for a proposal, by its first wire, as an index into
    /// [`novo_cables::default_types`].
    proposal_types: BTreeMap<u64, usize>,
}

/// The wires that can go on a core of `cable`: those on this cable and the
/// free ones. When the cable already has wires, the free ones are narrowed
/// to those that touch a device the cable's wires touch, which is what a
/// replacement for a core looks like; the whole free list when none does.
pub(super) fn candidate_wires(project: &Project, cable: CableId) -> Vec<(WireId, String)> {
    let Some(record) = project.cable(cable) else {
        return Vec::new();
    };
    let devices_of = |wire: WireId| -> Vec<novo_core::DeviceId> {
        project
            .wire(wire)
            .map(|w| {
                w.ends()
                    .into_iter()
                    .filter_map(|e| novo_cables::ends::owner(project, e))
                    .collect()
            })
            .unwrap_or_default()
    };
    let on_cable: Vec<WireId> = record.cores.iter().filter_map(|c| c.wire).collect();
    let touched: Vec<novo_core::DeviceId> = on_cable.iter().flat_map(|&w| devices_of(w)).collect();
    let free: Vec<WireId> = project
        .wires()
        .map(|w| w.id)
        .filter(|&w| project.core_of_wire(w).is_none())
        .collect();
    let near: Vec<WireId> = free
        .iter()
        .copied()
        .filter(|&w| devices_of(w).iter().any(|d| touched.contains(d)))
        .collect();
    let chosen = if touched.is_empty() || near.is_empty() {
        free
    } else {
        near
    };
    let mut out: Vec<(WireId, String)> = on_cable
        .into_iter()
        .chain(chosen)
        .map(|w| (w, wire_label(project, w)))
        .collect();
    out.sort_by(|a, b| novo_core::natural_cmp(&a.1, &b.1));
    out.dedup_by_key(|(w, _)| *w);
    out
}

/// A wire as the pickers name it: its number (or `#id`) and both ends.
pub(super) fn wire_label(project: &Project, wire: WireId) -> String {
    let Some(w) = project.wire(wire) else {
        return format!("#{}", wire.raw());
    };
    let end = |e| novo_cables::ends::designation(project, e).unwrap_or_else(|| "open".into());
    let name = w
        .number
        .clone()
        .unwrap_or_else(|| format!("#{}", wire.raw()));
    format!("{name}  {} to {}", end(w.from), end(w.to))
}

/// The name, type and delete rows above the cable's other controls.
pub(super) fn header_controls(
    ui: &mut egui::Ui,
    project: &Project,
    cable: CableId,
    edit: &mut CableEdit,
    actions: &mut Vec<CableAction>,
) {
    let Some(record) = project.cable(cable) else {
        return;
    };
    let shown = record.tag.to_string();
    let name_id = egui::Id::new(("cable_name", cable.raw()));
    let ui_state = &mut edit.ui;
    if ui_state.name_for != Some(cable) {
        ui_state.name_for = Some(cable);
        ui_state.name.clone_from(&shown);
    }
    if ui.ctx().memory(|m| m.focused()) != Some(name_id) {
        ui_state.name.clone_from(&shown);
    }
    ui.horizontal(|ui| {
        ui.label("Name");
        let field = ui.add(
            egui::TextEdit::singleline(&mut ui_state.name)
                .id(name_id)
                .desired_width(90.0)
                .hint_text("-W1"),
        );
        if std::mem::take(&mut ui_state.focus_name) {
            field.request_focus();
        }
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if field.lost_focus() && !escaped && ui_state.name.trim() != shown {
            actions.push(CableAction::Rename(ui_state.name.clone()));
        }
        if ui
            .small_button("Delete cable")
            .on_hover_text(format!(
                "Removes the cable; its wires stay and are proposed again. {} brings it back",
                super::tips::key_or(ui.ctx(), "edit.undo", "Undo")
            ))
            .clicked()
        {
            actions.push(CableAction::Delete);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Type").on_hover_text(
            "Changing the type rebuilds the cores like a proposal: hand-made core assignments are reset",
        );
        let current = novo_cables::type_label(project, record);
        egui::ComboBox::from_id_salt(("cable_type", cable.raw()))
            .selected_text(current)
            .height(260.0)
            .show_ui(ui, |ui| {
                let fits = fitting_types(project, record);
                for (cable_type, fit) in fits {
                    let label = cable_type.name();
                    let response = ui.add_enabled(
                        fit,
                        egui::Button::selectable(CableType::of_cable(record) == cable_type, label),
                    );
                    if response
                        .on_disabled_hover_text("The wires on this cable do not fit that type")
                        .clicked()
                    {
                        actions.push(CableAction::Retype(cable_type));
                        ui.close();
                    }
                }
            });
    });
}

/// Every usual cable type, and whether the wires on the cable fit it.
fn fitting_types(project: &Project, cable: &Cable) -> Vec<(CableType, bool)> {
    let netlist = project.netlist();
    let conductors: Vec<novo_cables::Conductor> = cable
        .cores
        .iter()
        .filter(|c| !c.shield)
        .filter_map(|c| project.wire(c.wire?))
        .map(|w| novo_cables::propose::conductor(project, &netlist, w))
        .collect();
    let shield_in_use = cable.cores.iter().enumerate().any(|(i, c)| {
        c.shield && (c.wire.is_some() || novo_cables::shield_connection(cable, i).is_some())
    });
    novo_cables::default_types()
        .into_iter()
        .map(|t| {
            let fit = novo_cables::propose::assign(&conductors, &t).is_some()
                && (t.shield || !shield_in_use);
            (t, fit)
        })
        .collect()
}

/// The three extra cells of a core row: the wire on it, swap with another
/// core, and the spare mark.
pub(super) fn core_cells(
    ui: &mut egui::Ui,
    record: &Cable,
    core: usize,
    wires: &[(WireId, String)],
    current: &str,
    actions: &mut Vec<CableAction>,
) {
    let current = if current.is_empty() { "free" } else { current };
    egui::ComboBox::from_id_salt(("core_wire", record.id.raw(), core))
        .selected_text(current)
        .width(150.0)
        .show_ui(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    if ui.selectable_label(current == "free", "free").clicked() {
                        actions.push(CableAction::SetCoreWire { core, wire: None });
                        ui.close();
                    }
                    for (wire, label) in wires {
                        let on = record.cores[core].wire == Some(*wire);
                        if ui.selectable_label(on, label).clicked() {
                            actions.push(CableAction::SetCoreWire {
                                core,
                                wire: Some(*wire),
                            });
                            ui.close();
                        }
                    }
                });
        });
    egui::ComboBox::from_id_salt(("core_swap", record.id.raw(), core))
        .selected_text("swap with")
        .width(70.0)
        .show_ui(ui, |ui| {
            for (other, c) in record.cores.iter().enumerate() {
                if other != core && ui.selectable_label(false, &c.name).clicked() {
                    actions.push(CableAction::SwapCores(core, other));
                    ui.close();
                }
            }
        });
    let mut spare = novo_cables::is_spare(record, core);
    if ui
        .checkbox(&mut spare, "")
        .on_hover_text("Spare: kept free on purpose")
        .changed()
    {
        actions.push(CableAction::SetSpare { core, spare });
    }
}

/// The proposed cables, each with the type to use, Accept and Decline, and
/// the button that brings the declined ones back.
pub(super) fn proposals_section(
    ui: &mut egui::Ui,
    project: &Project,
    proposals: &[CableProposal],
    declined: usize,
    ui_state: &mut CableUi,
    actions: &mut Vec<DrawerAction>,
) {
    if proposals.is_empty() && declined == 0 {
        return;
    }
    ui.small("Proposed cables");
    let types = novo_cables::default_types();
    for proposal in proposals {
        let key = proposal.wires.first().map_or(0, |w| w.raw());
        let chosen = ui_state
            .proposal_types
            .get(&key)
            .and_then(|&i| types.get(i))
            .cloned();
        let shown = chosen
            .clone()
            .unwrap_or_else(|| proposal.cable_type.clone());
        ui.horizontal(|ui| {
            ui.label(proposal.tag.to_string());
            egui::ComboBox::from_id_salt(("proposal_type", key))
                .selected_text(shown.name())
                .show_ui(ui, |ui| {
                    for (i, t) in types.iter().enumerate() {
                        let mut probe = proposal.clone();
                        let fits = probe.retype(t.clone());
                        let response =
                            ui.add_enabled(fits, egui::Button::selectable(shown == *t, t.name()));
                        if response.clicked() {
                            ui_state.proposal_types.insert(key, i);
                            ui.close();
                        }
                    }
                });
            ui.weak(format!("{} wires", proposal.wires.len()));
            if ui.small_button("Accept").clicked() {
                actions.push(DrawerAction::AcceptOne(key, chosen));
            }
            if ui
                .small_button("Decline")
                .on_hover_text("Do not propose these wires as a cable again")
                .clicked()
            {
                actions.push(DrawerAction::DeclineOne(key));
            }
        });
    }
    if declined > 0
        && ui
            .small_button(format!("Restore {declined} declined"))
            .on_hover_text("Propose the declined wires as cables again")
            .clicked()
    {
        actions.push(DrawerAction::RestoreDeclined);
    }
    let _ = project;
    ui.add_space(6.0);
}

impl NovoApp {
    /// The cable the Cables drawer shows: the one picked, else the first.
    pub(super) fn shown_cable(&self) -> Option<CableId> {
        let project = self.backend.document()?.project();
        self.wb
            .drawer_cable
            .filter(|&c| project.cable(c).is_some())
            .or_else(|| novo_cables::overview(project).first().map(|r| r.cable))
    }

    /// Opens the Cables drawer on a cable.
    pub(super) fn show_cable(&mut self, cable: CableId) {
        self.wb.drawer = Some(Drawer::Cables);
        self.wb.drawer_cable = Some(cable);
    }

    /// Runs one cable edit as an undo step of its own.
    pub(super) fn cable_edit_action(&mut self, cable: CableId, tag: &str, action: CableAction) {
        match action {
            CableAction::SetCoreWire {
                core,
                wire: Some(wire),
            } => {
                self.change_cable(format!("Put a wire on a core of {tag}"), |tx| {
                    novo_cables::assign_core(tx, cable, core, wire)
                });
            }
            CableAction::SetCoreWire { core, wire: None } => {
                self.change_cable(format!("Free a core of {tag}"), |tx| {
                    novo_cables::unassign_core(tx, cable, core)
                });
            }
            CableAction::SwapCores(a, b) => {
                self.change_cable(format!("Swap two cores of {tag}"), |tx| {
                    novo_cables::swap_cores(tx, cable, a, b)
                });
            }
            CableAction::SetSpare { core, spare } => {
                self.change_cable(format!("Mark a spare core of {tag}"), |tx| {
                    novo_cables::set_spare(tx, cable, core, spare)
                });
            }
            CableAction::Rename(text) => self.rename_cable(cable, &text),
            CableAction::Delete => self.delete_cable(cable),
            CableAction::Retype(cable_type) => self.retype_cable(cable, &cable_type),
            _ => {}
        }
    }

    fn rename_cable(&mut self, cable: CableId, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let text = if text.starts_with(['=', '+', '-']) {
            text.to_owned()
        } else {
            format!("-{text}")
        };
        let Ok(tag) = DeviceTag::parse(&text) else {
            self.message(
                Level::Warning,
                format!("\"{text}\" is not a cable tag, for example -W3"),
            );
            return;
        };
        let taken = self
            .backend
            .document()
            .is_some_and(|d| d.project().cables().any(|c| c.id != cable && c.tag == tag));
        if taken {
            self.message(
                Level::Warning,
                format!("Another cable is already called {tag}"),
            );
            return;
        }
        let label = format!("Rename cable to {tag}");
        self.change_cable(label, |tx| novo_cables::rename_cable(tx, cable, tag));
    }

    fn delete_cable(&mut self, cable: CableId) {
        let Some(tag) = self
            .backend
            .document()
            .and_then(|d| d.project().cable(cable))
            .map(|c| c.tag.to_string())
        else {
            return;
        };
        if self
            .change_cable(format!("Delete cable {tag}"), |tx| {
                novo_cables::delete_cable(tx, cable)
            })
            .is_some()
        {
            if self.wb.drawer_cable == Some(cable) {
                self.wb.drawer_cable = None;
            }
            self.message(
                Level::Info,
                format!("Deleted cable {tag}. Ctrl+Z brings it back"),
            );
        }
    }

    fn retype_cable(&mut self, cable: CableId, cable_type: &CableType) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let tag = doc
            .project()
            .cable(cable)
            .map(|c| c.tag.to_string())
            .unwrap_or_default();
        let name = cable_type.name();
        match doc.transact(format!("Change {tag} to {name}"), |tx| {
            novo_cables::retype_cable(tx, cable, cable_type)
        }) {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("{tag} is now {name}"));
            }
            Err(_) => self.message(
                Level::Warning,
                format!("The wires on {tag} do not fit {name}"),
            ),
        }
    }

    /// Accepts one proposed cable, on the type picked for it when there is
    /// one. `key` is the raw id of its first wire.
    pub(super) fn accept_one_cable(&mut self, key: u64, cable_type: Option<CableType>) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let mut proposals = novo_cables::propose_cables(doc.project());
        let Some(at) = proposals
            .iter()
            .position(|p| p.wires.first().map(|w| w.raw()) == Some(key))
        else {
            return;
        };
        let mut proposal = proposals.swap_remove(at);
        if let Some(t) = cable_type
            && !proposal.retype(t.clone())
        {
            self.message(
                Level::Warning,
                format!("The wires of {} do not fit {}", proposal.tag, t.name()),
            );
            return;
        }
        let (tag, name) = (proposal.tag.clone(), proposal.cable_type.name());
        match doc.transact(format!("Accept cable {tag}"), |tx| {
            novo_cables::accept_proposals(tx, &[proposal], None)
        }) {
            Ok((created, changes)) => {
                self.wb.drawer_cable = created.first().copied();
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Cable {tag}: {name}"));
            }
            Err(e) => self.message(Level::Error, format!("Could not add the cable: {e}")),
        }
    }

    /// Declines one proposed cable for good; "Restore declined" or undo
    /// brings it back.
    pub(super) fn decline_one_cable(&mut self, key: u64) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let proposals = novo_cables::propose_cables(doc.project());
        let Some(proposal) = proposals
            .iter()
            .find(|p| p.wires.first().map(|w| w.raw()) == Some(key))
        else {
            return;
        };
        let tag = proposal.tag.clone();
        match doc.transact(format!("Decline cable {tag}"), |tx| {
            novo_cables::dismiss_proposal(tx, proposal)
        }) {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Declined {tag}"));
            }
            Err(e) => self.message(Level::Error, format!("Could not decline the cable: {e}")),
        }
    }

    /// `cable.restore_proposals`: every declined cable proposal comes back.
    pub(super) fn restore_declined_cables(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let declined = novo_cables::declined_wires(doc.project());
        if declined.is_empty() {
            self.message(Level::Info, "No cable proposal was declined");
            return;
        }
        match doc.transact("Restore declined cables", |tx| {
            novo_cables::restore_proposal(tx, &declined)
        }) {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                let n =
                    novo_cables::propose_cables(self.backend.document().unwrap().project()).len();
                self.message(Level::Info, format!("{n} cable proposals are back"));
            }
            Err(e) => self.message(Level::Error, format!("Could not restore: {e}")),
        }
    }

    /// `cable.rename [name]`: renames the cable the drawer shows; with no
    /// name it opens the drawer with the cursor in the name field.
    pub(super) fn rename_cable_command(&mut self, argument: Option<&str>) {
        let Some(cable) = self.shown_cable() else {
            self.message(Level::Info, "No cable yet");
            return;
        };
        match argument.map(str::trim).filter(|a| !a.is_empty()) {
            Some(name) => self.rename_cable(cable, name),
            None => {
                self.show_cable(cable);
                self.wb.cable_edit.ui.focus_name = true;
            }
        }
    }

    /// `cable.delete`: deletes the cable the drawer shows.
    pub(super) fn delete_cable_command(&mut self) {
        match self.shown_cable() {
            Some(cable) => self.delete_cable(cable),
            None => self.message(Level::Info, "No cable yet"),
        }
    }

    /// `cable.retype [4G1.5]`: gives the shown cable another type; with no
    /// type it opens the drawer, which lists the types that fit.
    pub(super) fn retype_cable_command(&mut self, argument: Option<&str>) {
        let Some(cable) = self.shown_cable() else {
            self.message(Level::Info, "No cable yet");
            return;
        };
        let Some(wanted) = argument.map(str::trim).filter(|a| !a.is_empty()) else {
            self.show_cable(cable);
            self.message(
                Level::Info,
                "Pick the type in the Type list of the Cables drawer",
            );
            return;
        };
        let found = novo_cables::default_types()
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(wanted));
        match found {
            Some(cable_type) => self.retype_cable(cable, &cable_type),
            None => self.message(
                Level::Warning,
                format!("No usual cable type is called {wanted}, for example 4G1.5 or 7x1"),
            ),
        }
    }

    /// Opens the page of a cable's first wired core and selects the cores'
    /// wires on it together. Cores on other pages are counted in a message:
    /// a selection belongs to one page.
    pub(super) fn go_to_cable_wires(&mut self, wires: &[WireId]) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let pages: Vec<(WireId, PageId)> = wires
            .iter()
            .filter_map(|&w| project.wire(w).map(|x| (w, x.page)))
            .collect();
        let Some(&(_, page)) = pages.first() else {
            self.message(Level::Info, "Those wires no longer exist");
            return;
        };
        let here: Vec<AnyId> = pages
            .iter()
            .filter(|(_, p)| *p == page)
            .map(|(w, _)| AnyId::Wire(*w))
            .collect();
        let elsewhere = pages.len() - here.len();
        self.show_page(page);
        self.wb.pending_items = Some((page, here));
        if elsewhere > 0 {
            self.message(
                Level::Info,
                format!("{elsewhere} more cores are wired on other pages"),
            );
        }
    }

    /// Selects a wire on its page (the command field's "wire 105").
    pub(super) fn go_to_wire(&mut self, wire: WireId) {
        let page = self
            .backend
            .document()
            .and_then(|d| d.project().wire(wire))
            .map(|w| w.page);
        match page {
            Some(page) => {
                self.show_page(page);
                self.wb.pending_item = Some((page, AnyId::Wire(wire)));
            }
            None => self.message(Level::Info, "That wire no longer exists"),
        }
    }
}
