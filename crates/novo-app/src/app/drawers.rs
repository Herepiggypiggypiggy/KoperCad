//! The drawer under the canvas (docs/DESIGN.md §3): Terminals (Ctrl+3),
//! Cables (Ctrl+4) and Devices (Ctrl+5), tables of the same model with the
//! drawing still in view. Selecting on the drawing selects rows, and a
//! click on a row selects and centres its drawing.

use eframe::egui;
use novo_core::{AnyId, CableId, Project, natural_cmp};
use novo_xref::Place;

use super::cable_controls::{CableAction, CableEdit, cable_controls};
use super::cable_cores::{candidate_wires, core_cells, header_controls, proposals_section};
use super::learn_ui::chip_button;
use super::part_picker::{Candidate, cable_candidates};
use super::terminal_edit::{ToolbarAction, strip_toolbar};
use super::terminals::strip_table;
use super::workbench::{DRAWER_SHARE, Drawer};
use super::{Level, NovoApp};

/// What a click in the drawer asks for.
pub(super) enum DrawerAction {
    Go(Place),
    ShowCable(CableId),
    /// A change from the cable controls, for this cable.
    Cable(CableId, CableAction),
    AcceptCables,
    AcceptTerminals,
    Terminal(ToolbarAction),
    /// Accept one proposed cable (its first wire's raw id), maybe on
    /// another type.
    AcceptOne(u64, Option<novo_cables::CableType>),
    DeclineOne(u64),
    RestoreDeclined,
    Tab(Drawer),
    Lesson(String),
    Close,
}

/// One row of the Devices tab.
struct DeviceRow {
    tag: String,
    description: String,
    part: String,
    location: String,
    /// Its first drawing, in page order; `None` when it is not drawn.
    place: Option<Place>,
    selected: bool,
}

impl NovoApp {
    /// The drawer, docked under the canvas while a tab is open.
    pub(super) fn drawer(&mut self, ui: &mut egui::Ui) {
        let Some(tab) = self.wb.drawer else {
            return;
        };
        if !self.is_open() {
            return;
        }
        let height = (ui.ctx().content_rect().height() * DRAWER_SHARE).max(120.0);
        let selected = self.selection();
        self.sync_strip_follow();
        let strip = self.drawer_strip();
        let declined_terminals = self.declined_wires();
        if tab == Drawer::Cables
            && self.wb.cable_edit.picker.open
            && let Some(problem) = self.wb.part_catalog.ensure()
        {
            self.message(Level::Error, problem);
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let cable_proposals = if tab == Drawer::Cables {
            novo_cables::propose_cables(project)
        } else {
            Vec::new()
        };
        let declined = if tab == Drawer::Cables {
            novo_cables::declined_wires(project).len()
        } else {
            0
        };
        let proposed = match tab {
            Drawer::Terminals => novo_terminals::suggest_terminals(project).len(),
            Drawer::Cables => cable_proposals.len(),
            Drawer::Devices => 0,
        };
        let chip = match tab {
            Drawer::Terminals => self.learn.chip("terminals"),
            Drawer::Cables => self.learn.chip("cables"),
            Drawer::Devices => None,
        };
        let mut actions = Vec::new();
        // Controls for lessons to point at, registered after the panel.
        let mut spots: Vec<(String, egui::Rect, bool, String)> = Vec::new();
        let panel = egui::Panel::bottom("drawer")
            .exact_size(height)
            .resizable(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (t, label) in [
                        (Drawer::Terminals, "Terminals"),
                        (Drawer::Cables, "Cables"),
                        (Drawer::Devices, "Devices"),
                    ] {
                        let response = ui.selectable_label(tab == t, label);
                        let name = label.to_lowercase();
                        spots.push((
                            format!("drawer.{name}.tab"),
                            response.rect,
                            response.clicked(),
                            label.to_owned(),
                        ));
                        if response.clicked() && tab != t {
                            actions.push(DrawerAction::Tab(t));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Close").clicked() {
                            actions.push(DrawerAction::Close);
                        }
                        if let Some(id) = chip_button(ui, chip.as_ref()) {
                            actions.push(DrawerAction::Lesson(id));
                        }
                        if proposed > 0 {
                            if tab == Drawer::Cables {
                                let response =
                                    ui.small_button(format!("Accept {proposed} proposed"));
                                spots.push((
                                    "drawer.cables.accept".into(),
                                    response.rect,
                                    response.clicked(),
                                    "Accept proposed".into(),
                                ));
                                if response.clicked() {
                                    actions.push(DrawerAction::AcceptCables);
                                }
                            }
                            if tab == Drawer::Terminals {
                                let response =
                                    ui.small_button(format!("Accept {proposed} proposed"));
                                spots.push((
                                    "drawer.terminals.accept".into(),
                                    response.rect,
                                    response.clicked(),
                                    "Accept proposed".into(),
                                ));
                                if response.clicked() {
                                    actions.push(DrawerAction::AcceptTerminals);
                                }
                            }
                        }
                    });
                });
                ui.separator();
                match tab {
                    Drawer::Terminals => {
                        let mut toolbar = Vec::new();
                        strip_toolbar(
                            ui,
                            project,
                            strip,
                            &mut self.wb.terminal_edit,
                            &declined_terminals,
                            &mut toolbar,
                        );
                        match strip {
                            Some(strip) => {
                                let view = novo_terminals::strip_view(project, strip);
                                let screen = novo_terminals::screen(project, strip);
                                let mut ticked = std::mem::take(&mut self.wb.terminal_edit.rows);
                                if let Some(o) = strip_table(
                                    ui,
                                    &view,
                                    &screen,
                                    &selected,
                                    &mut ticked,
                                    &mut toolbar,
                                ) {
                                    actions.push(DrawerAction::Go(Place {
                                        page: o.page,
                                        symbol: Some(o.symbol),
                                    }));
                                }
                                self.wb.terminal_edit.rows = ticked;
                            }
                            None => {
                                ui.weak(
                                    "No terminal strip yet. Place a terminal (I, terminal) or accept a proposal.",
                                );
                            }
                        }
                        actions.extend(toolbar.into_iter().map(DrawerAction::Terminal));
                    }
                    Drawer::Cables => {
                        let candidates = match self.wb.part_catalog.get() {
                            Some(catalog) if self.wb.cable_edit.picker.open => self
                                .wb
                                .cable_edit
                                .picker
                                .rows("cables", |query| cable_candidates(catalog, query)),
                            _ => Vec::new(),
                        };
                        cables_tab(
                            ui,
                            project,
                            self.wb.drawer_cable,
                            (&cable_proposals, declined),
                            (&mut self.wb.cable_edit, &candidates),
                            &mut actions,
                        );
                    }
                    Drawer::Devices => {
                        let rows = device_rows(project, &selected);
                        devices_tab(ui, &rows, &mut actions);
                    }
                }
            });
        spots.push((
            format!("drawer.{}", drawer_name(tab)),
            panel.response.rect,
            false,
            "drawer".into(),
        ));
        for (id, rect, clicked, label) in spots {
            self.anchor(&id, rect, &label);
            if clicked {
                self.learn_event(novo_learn::Event::Clicked(id));
            }
        }
        for action in actions {
            match action {
                DrawerAction::Go(place) => self.jump(place, true),
                DrawerAction::ShowCable(cable) => self.wb.drawer_cable = Some(cable),
                DrawerAction::Cable(cable, action) => self.cable_action(cable, action),
                DrawerAction::AcceptCables => self.accept_cable_proposals(),
                DrawerAction::AcceptTerminals => self.accept_all_proposals(),
                DrawerAction::Terminal(ToolbarAction::Command(id)) => self.wb.queued.push(id),
                DrawerAction::Terminal(ToolbarAction::Pick(strip)) => {
                    self.wb.terminal_edit.picked = Some(strip);
                    self.wb.terminal_edit.rows.clear();
                }
                DrawerAction::Terminal(ToolbarAction::Restore(wires)) => {
                    self.restore_declined(&wires);
                }
                DrawerAction::AcceptOne(key, cable_type) => self.accept_one_cable(key, cable_type),
                DrawerAction::DeclineOne(key) => self.decline_one_cable(key),
                DrawerAction::RestoreDeclined => self.restore_declined_cables(),
                DrawerAction::Tab(t) => self.wb.drawer = Some(t),
                DrawerAction::Lesson(id) => self.learn_run_command_lesson(&id),
                DrawerAction::Close => self.wb.drawer = None,
            }
        }
    }

    /// A cable drawn with C: the wires the line touched go on one cable, in
    /// one undo step, on the smallest usual type they fit.
    pub(super) fn make_cable(&mut self, wires: &[novo_core::WireId]) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(proposal) = novo_cables::proposal_for_wires(doc.project(), wires) else {
            self.message(Level::Warning, "None of those wires lands on a device");
            return;
        };
        let (tag, name) = (proposal.tag.clone(), proposal.cable_type.name());
        match doc.transact("Draw cable", |tx| {
            novo_cables::accept_proposals(tx, &[proposal], None)
        }) {
            Ok((created, changes)) => {
                self.wb.drawer_cable = created.first().copied();
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    format!("Cable {tag}: {name} on {} wires", wires.len()),
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not add the cable: {e}")),
        }
    }

    /// Accepts every proposed cable in one undo step.
    fn accept_cable_proposals(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let proposals = novo_cables::propose_cables(doc.project());
        if proposals.is_empty() {
            return;
        }
        let summary = novo_cables::summarize_proposals(&proposals);
        match doc.transact("Accept cables", |tx| {
            novo_cables::accept_proposals(tx, &proposals, None)
        }) {
            Ok((created, changes)) => {
                self.wb.drawer_cable = created.first().copied();
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Accepted {summary}"));
            }
            Err(e) => self.message(Level::Error, format!("Could not add the cables: {e}")),
        }
    }
}

/// Every cable with its type, ends and cores in use, then the cores of the
/// one picked (the first when none is).
fn cables_tab(
    ui: &mut egui::Ui,
    project: &Project,
    picked: Option<CableId>,
    (proposals, declined): (&[novo_cables::CableProposal], usize),
    (edit, candidates): (&mut CableEdit, &[Candidate]),
    actions: &mut Vec<DrawerAction>,
) {
    let overview = novo_cables::overview(project);
    let proposed = proposals.len();
    if overview.is_empty() {
        proposals_section(ui, project, proposals, declined, &mut edit.ui, actions);
        ui.weak(if proposed > 0 {
            "No cables yet. Accept the proposed ones, or draw wires between locations."
        } else {
            "No cables yet. Wires between two locations (+CAB to +FIELD) are proposed as cables."
        });
        return;
    }
    let shown = picked
        .filter(|c| overview.iter().any(|r| r.cable == *c))
        .unwrap_or(overview[0].cable);
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.columns(2, |cols| {
            proposals_section(
                &mut cols[0],
                project,
                proposals,
                declined,
                &mut edit.ui,
                actions,
            );
            egui::Grid::new("cables")
                .striped(true)
                .num_columns(5)
                .spacing([20.0, 4.0])
                .show(&mut cols[0], |ui| {
                    for head in ["Cable", "Type", "From", "To", "Cores"] {
                        ui.small(head);
                    }
                    ui.end_row();
                    for row in &overview {
                        if ui.selectable_label(row.cable == shown, &row.tag).clicked() {
                            actions.push(DrawerAction::ShowCable(row.cable));
                        }
                        ui.label(&row.type_name);
                        ui.label(or_unnamed(&row.from));
                        ui.label(or_unnamed(&row.to));
                        ui.label(format!("{} of {}", row.used, row.cores));
                        ui.end_row();
                    }
                });
            if let Some(screen) = novo_cables::cable_screen(project, shown) {
                let ui = &mut cols[1];
                ui.horizontal(|ui| {
                    ui.strong(&screen.tag);
                    ui.weak(&screen.type_name);
                });
                let mut changes = Vec::new();
                header_controls(ui, project, shown, edit, &mut changes);
                cable_controls(ui, project, shown, edit, candidates, &mut changes);
                let wires = candidate_wires(project, shown);
                let record = project.cable(shown);
                egui::Grid::new("cable_cores")
                    .striped(true)
                    .num_columns(7)
                    .spacing([14.0, 4.0])
                    .show(ui, |ui| {
                        let from = side_head(&screen.from, "From");
                        let to = side_head(&screen.to, "To");
                        for head in ["Core", from, to, "Potential", "Wire", "Swap", "Spare"] {
                            ui.small(head);
                        }
                        ui.end_row();
                        for core in &screen.rows {
                            let end = |i: usize| {
                                core.ends[i]
                                    .as_ref()
                                    .map(|e| e.target.clone())
                                    .unwrap_or_default()
                            };
                            ui.label(&core.core);
                            if core.spare {
                                ui.weak("spare");
                                ui.weak("");
                            } else {
                                ui.label(end(0));
                                ui.label(end(1));
                            }
                            ui.weak(core.potential.clone().unwrap_or_default());
                            if let Some(record) = record {
                                let current = core.wire_number.clone().unwrap_or_else(|| {
                                    core.wire
                                        .map(|w| format!("#{}", w.raw()))
                                        .unwrap_or_default()
                                });
                                core_cells(ui, record, core.index, &wires, &current, &mut changes);
                            }
                            ui.end_row();
                        }
                    });
                actions.extend(changes.into_iter().map(|a| DrawerAction::Cable(shown, a)));
            }
        });
    });
}

/// A cable end's location, or a dash when its devices name none.
fn or_unnamed(location: &str) -> &str {
    if location.is_empty() { "–" } else { location }
}

/// A core column's heading: the end's location, else `fallback`.
fn side_head<'a>(location: &'a str, fallback: &'a str) -> &'a str {
    if location.is_empty() {
        fallback
    } else {
        location
    }
}

/// Every device in natural tag order, with what the Devices tab shows.
fn device_rows(project: &Project, selected: &[AnyId]) -> Vec<DeviceRow> {
    let page_order: Vec<_> = project.pages_in_order().iter().map(|p| p.id).collect();
    let mut rows: Vec<DeviceRow> = project
        .devices()
        .map(|d| {
            let mut symbols = project.symbols_of_device(d.id);
            symbols.sort_by_key(|s| page_order.iter().position(|&p| p == s.page));
            let part = d
                .parts
                .first()
                .and_then(|&p| project.part(p))
                .map(|p| format!("{} {}", p.manufacturer, p.order_number))
                .unwrap_or_default();
            let location = d
                .tag
                .location
                .iter()
                .map(|l| format!("+{l}"))
                .collect::<String>();
            DeviceRow {
                tag: d.tag.to_string(),
                description: d.description.clone(),
                part,
                location,
                place: symbols.first().map(|s| Place {
                    page: s.page,
                    symbol: Some(s.id),
                }),
                selected: selected.contains(&AnyId::Device(d.id))
                    || symbols
                        .iter()
                        .any(|s| selected.contains(&AnyId::Symbol(s.id))),
            }
        })
        .collect();
    rows.sort_by(|a, b| natural_cmp(&a.tag, &b.tag));
    rows
}

fn devices_tab(ui: &mut egui::Ui, rows: &[DeviceRow], actions: &mut Vec<DrawerAction>) {
    if rows.is_empty() {
        ui.weak("No devices yet. Place a symbol with I.");
        return;
    }
    ui.weak(format!("{} devices", rows.len()));
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("devices")
            .striped(true)
            .num_columns(4)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for head in ["Device", "Description", "Part", "Location"] {
                    ui.small(head);
                }
                ui.end_row();
                for row in rows {
                    match row.place {
                        Some(place) => {
                            if ui
                                .selectable_label(row.selected, &row.tag)
                                .on_hover_text("Show on the drawing")
                                .clicked()
                            {
                                actions.push(DrawerAction::Go(place));
                            }
                        }
                        None => {
                            ui.label(&row.tag).on_hover_text("Not drawn yet");
                        }
                    }
                    ui.label(&row.description);
                    ui.label(&row.part);
                    ui.label(&row.location);
                    ui.end_row();
                }
            });
    });
}

/// The name a drawer goes by in lesson anchors (`drawer.terminals`).
fn drawer_name(tab: Drawer) -> &'static str {
    match tab {
        Drawer::Terminals => "terminals",
        Drawer::Cables => "cables",
        Drawer::Devices => "devices",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::DeviceTag;

    #[test]
    fn devices_are_listed_in_natural_tag_order() {
        let mut project = Project::new("Plant");
        project
            .transact("Add", |tx| {
                for tag in ["-K10", "-K2", "+CAB-K1"] {
                    tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                }
                Ok(())
            })
            .unwrap();
        let rows = device_rows(&project, &[]);
        let tags: Vec<_> = rows.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags.len(), 3);
        let k2 = tags.iter().position(|t| *t == "-K2").unwrap();
        let k10 = tags.iter().position(|t| *t == "-K10").unwrap();
        assert!(k2 < k10, "{tags:?}");
        let cab = rows.iter().find(|r| r.tag == "+CAB-K1").unwrap();
        assert_eq!(cab.location, "+CAB");
        assert!(rows.iter().all(|r| r.place.is_none()), "nothing drawn");
    }
}
