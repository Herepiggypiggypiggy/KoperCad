//! The Cabinet canvas (Ctrl+2): a panel's mounting plate, rails and placed
//! devices, drawn by novo-canvas from the layout novo-layout reads out of the
//! model. `Place all` previews a plan on the canvas; Enter accepts it as one
//! undo step, Escape drops it (docs/design/cabinet-layout.md).

use eframe::egui;
use novo_canvas::cabinet::{
    BoxState, BoxView, CabinetScene, CabinetView, PlateView, RailView, view_bounds,
};
use novo_canvas::widget::CanvasView;
use novo_canvas::{Vec2, vec2};
use novo_core::{Command, Device, DeviceId, DeviceTag, MountingPlate, StructureId};
use novo_layout::model::{
    apply_plan, candidates, load, place_device, rotate_device, set_plate, unplace_device,
};
use novo_layout::plan::Plan;
use novo_layout::state::CabinetLayout;

use super::{Level, NovoApp};
use crate::theme;

/// The plate a new cabinet gets, in millimetres: a common 600 x 800 panel.
const DEFAULT_PLATE: MountingPlate = MountingPlate {
    width: 600.0,
    height: 800.0,
    margin: 10.0,
};

pub(super) struct CabinetState {
    view: CanvasView,
    scene: CabinetScene,
    /// The panel shown, by its location (`+K1B1G1`).
    pub(super) location: Option<DeviceTag>,
    /// A `Place all` plan waiting for Enter.
    pub(super) preview: Option<Plan>,
    /// The panel the drawing was last fitted to.
    fitted: Option<DeviceTag>,
    /// A placed device being dragged, with where it would land.
    drag: Option<Drag>,
    /// An unplaced device being dragged over the plate, with the snapped
    /// top left it would land on: drawn as the landing box.
    pub(super) ghost: Option<(u64, f64, f64)>,
    /// Keys the shell left for the Cabinet this frame (`edit.rotate`,
    /// `edit.delete`).
    pub(super) keys: Vec<String>,
}

/// A placed device being dragged across the plate.
#[derive(Clone, Copy)]
struct Drag {
    id: u64,
    /// The cursor's offset from the footprint's top left, in mm.
    grab: Vec2,
    /// The snapped top left it would land on.
    at: (f64, f64),
}

impl CabinetState {
    pub(super) fn new() -> Self {
        let mut view = CanvasView::new();
        view.style = theme::canvas_style();
        view.grid.spacing = novo_layout::state::DEFAULT_GRID_MM as f32;
        view.grid.visible = false;
        Self {
            view,
            scene: CabinetScene::new(theme::cabinet_style()),
            location: None,
            preview: None,
            fitted: None,
            drag: None,
            ghost: None,
            keys: Vec::new(),
        }
    }
}

/// What the header strip asked for this frame.
pub(super) enum Ask {
    Panel(DeviceTag),
    CreateCabinet(DeviceTag),
    SetPlate(StructureId),
    PlaceAll,
    Accept,
    Cancel,
    /// Place or move a device with its top left at `(x, y)` mm.
    Drop(u64, f64, f64),
    Rotate(Vec<u64>),
    Remove(Vec<u64>),
}

/// The Cabinet drawing of a layout, with the layout's problems as box states.
pub(super) fn cabinet_view(layout: &CabinetLayout, tag_of: &impl Fn(u64) -> String) -> CabinetView {
    let report = layout.report();
    let state = |id: u64| {
        if report.collisions.iter().any(|&(a, b)| a == id || b == id)
            || report.outside.contains(&id)
        {
            if report.outside.contains(&id) {
                BoxState::Outside
            } else {
                BoxState::Collision
            }
        } else if report.too_close.iter().any(|&(a, b, _)| a == id || b == id) {
            BoxState::TooClose
        } else {
            BoxState::Ok
        }
    };
    CabinetView {
        plate: Some(PlateView {
            width: layout.plate.width as f32,
            height: layout.plate.height as f32,
            margin: layout.plate.margin as f32,
        }),
        rails: layout
            .rails
            .iter()
            .map(|r| RailView {
                id: r.id,
                x: r.x as f32,
                y: r.y as f32,
                length: r.length as f32,
            })
            .collect(),
        boxes: layout
            .placements
            .iter()
            .map(|p| {
                let f = p.footprint();
                BoxView {
                    id: p.id,
                    x: f.x as f32,
                    y: f.y as f32,
                    width: f.w as f32,
                    height: f.h as f32,
                    tag: tag_of(p.id),
                    guessed: p.size.guessed,
                    clearance: p.clearance.unwrap_or(novo_layout::DEFAULT_CLEARANCE_MM) as f32,
                    state: state(p.id),
                }
            })
            .collect(),
        heat: None,
    }
}

impl NovoApp {
    /// The Cabinet canvas, in place of the schematic canvas.
    pub(super) fn cabinet_show(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();

        // The panels: every location with pages, plus every enclosure.
        let mut locations: Vec<DeviceTag> =
            project.panels().into_iter().map(|p| p.location).collect();
        for s in project.structures().filter(|s| s.enclosure) {
            if !locations.contains(&s.tag) {
                locations.push(s.tag.clone());
            }
        }
        locations.retain(|l| !l.is_empty());
        if self
            .cabinet
            .location
            .as_ref()
            .is_none_or(|l| !locations.contains(l))
        {
            self.cabinet.location = locations.first().cloned();
            self.cabinet.preview = None;
        }
        let location = self.cabinet.location.clone();
        let structure: Option<StructureId> = location.as_ref().and_then(|l| {
            project
                .structures()
                .find(|s| s.enclosure && &s.tag == l)
                .map(|s| s.id)
        });
        let layout = structure.and_then(|s| load(project, s));
        let cands = structure
            .map(|s| candidates(project, s))
            .unwrap_or_default();
        let tag_of = |id: u64| {
            project
                .get::<Device>(DeviceId::from(id))
                .and_then(|d| d.tag.device_code().map(str::to_owned))
                .unwrap_or_default()
        };

        // What is drawn: the stored layout, or the layout with the plan applied.
        let shown = layout.as_ref().map(|l| match &self.cabinet.preview {
            Some(plan) => {
                let mut preview = l.clone();
                preview.accept(plan, &cands);
                preview
            }
            None => l.clone(),
        });
        let shown = shown.map(|mut s| {
            if let (Some(d), None) = (self.cabinet.drag, &self.cabinet.preview) {
                s.move_to(d.id, d.at.0, d.at.1);
            }
            if let (Some((id, x, y)), None) = (self.cabinet.ghost, &self.cabinet.preview)
                && let Some(c) = cands.iter().find(|c| c.id == id)
                && !s.placements.iter().any(|p| p.id == id)
            {
                s.drop_device(id, c.size, x, y);
            }
            s
        });
        let (placed, unplaced, problems) = match (&layout, &shown) {
            (Some(l), Some(s)) => {
                let r = s.report();
                let problems = r.collisions.len()
                    + r.outside.len()
                    + r.too_close.len()
                    + r.rail_overflows.len();
                (l.placements.len(), l.unplaced(&cands).len(), problems)
            }
            _ => (0, cands.len(), 0),
        };
        let has_plan = self.cabinet.preview.is_some();

        let mut ask: Option<Ask> = None;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let current = location
                .as_ref()
                .map_or_else(|| "No panel".to_owned(), ToString::to_string);
            egui::ComboBox::from_id_salt("cabinet_panel")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for l in &locations {
                        if ui
                            .selectable_label(location.as_ref() == Some(l), l.to_string())
                            .clicked()
                        {
                            ask = Some(Ask::Panel(l.clone()));
                        }
                    }
                });
            match (structure, &location) {
                (None, Some(l)) => {
                    if ui.button("Create cabinet").clicked() {
                        ask = Some(Ask::CreateCabinet(l.clone()));
                    }
                }
                (Some(s), _)
                    if layout.is_none() && ui.button("Set plate 600 x 800 mm").clicked() =>
                {
                    ask = Some(Ask::SetPlate(s));
                }
                _ => {}
            }
            if layout.is_some() {
                if has_plan {
                    if ui.button("Accept  (Enter)").clicked() {
                        ask = Some(Ask::Accept);
                    }
                    if ui.button("Cancel  (Esc)").clicked() {
                        ask = Some(Ask::Cancel);
                    }
                } else {
                    let enabled = unplaced > 0;
                    if ui
                        .add_enabled(enabled, egui::Button::new("Place all"))
                        .on_disabled_hover_text("Every device of this panel is placed")
                        .clicked()
                    {
                        ask = Some(Ask::PlaceAll);
                    }
                }
                ui.weak(format!(
                    "{placed} placed, {unplaced} unplaced, {problems} {}",
                    if problems == 1 { "problem" } else { "problems" }
                ));
            }
        });
        ui.separator();

        match shown {
            Some(shown) => {
                let unplaced_list: Vec<(u64, String, String)> = layout
                    .as_ref()
                    .map(|l| {
                        l.unplaced(&cands)
                            .into_iter()
                            .map(|c| {
                                let size = format!("{} x {} mm", c.size.width, c.size.height);
                                let size = if c.size.guessed {
                                    format!("{size} (guessed)")
                                } else {
                                    size
                                };
                                (c.id, tag_of(c.id), size)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                egui::Panel::left("cabinet_unplaced")
                    .exact_size(200.0)
                    .resizable(false)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.strong("Unplaced");
                        ui.weak("Drag onto the plate");
                        ui.add_space(4.0);
                        for (id, tag, size) in &unplaced_list {
                            ui.dnd_drag_source(egui::Id::new(("cabinet_device", *id)), *id, |ui| {
                                ui.label(tag.as_str());
                                ui.weak(size.as_str());
                            });
                            ui.add_space(2.0);
                        }
                        if unplaced_list.is_empty() {
                            ui.weak("Every device is placed");
                        }
                    });

                let view = cabinet_view(&shown, &tag_of);
                self.cabinet.scene.set(&view);
                if self.cabinet.fitted != location {
                    self.cabinet.view.viewport.fit(&view_bounds(&view), 24.0);
                    self.cabinet.fitted = location.clone();
                }
                let out = self.cabinet.view.show(ui, self.cabinet.scene.scene());
                // Enter and Escape belong to the palette or a text field when
                // one is open.
                let typing = self.wb.field.is_open()
                    || self.wb.occurrences.is_some()
                    || self.modal_open()
                    || ui.ctx().egui_wants_keyboard_input();
                let (enter, escape) = ui.input(|i| {
                    (
                        !typing && i.key_pressed(egui::Key::Enter),
                        !typing && i.key_pressed(egui::Key::Escape),
                    )
                });
                if has_plan && enter {
                    ask = Some(Ask::Accept);
                } else if has_plan && escape {
                    ask = Some(Ask::Cancel);
                } else if let (Some(layout), false) = (&layout, has_plan) {
                    // Dropping a device from the list.
                    if let (Some(id), Some(at)) =
                        (out.response.dnd_release_payload::<u64>(), out.cursor_raw)
                        && let Some(c) = cands.iter().find(|c| c.id == *id)
                    {
                        let (x, y) = layout.snapped(
                            f64::from(at.x) - c.size.width / 2.0,
                            f64::from(at.y) - c.size.height / 2.0,
                        );
                        ask = Some(Ask::Drop(*id, x, y));
                    }
                    // The landing box of a device dragged in from the list.
                    self.cabinet.ghost = None;
                    if let (Some(id), Some(at)) =
                        (out.response.dnd_hover_payload::<u64>(), out.cursor_raw)
                        && let Some(c) = cands.iter().find(|c| c.id == *id)
                    {
                        let (x, y) = layout.snapped(
                            f64::from(at.x) - c.size.width / 2.0,
                            f64::from(at.y) - c.size.height / 2.0,
                        );
                        self.cabinet.ghost = Some((*id, x, y));
                    }
                    // Dragging a placed device: egui starts a drag a few
                    // points after the press, so the box is the one under
                    // the press, not under the pointer now.
                    let press = ui.input(|i| i.pointer.press_origin());
                    if out.response.drag_started_by(egui::PointerButton::Primary)
                        && let Some(press) = press
                    {
                        let rect = out.response.rect;
                        let world = self
                            .cabinet
                            .view
                            .viewport
                            .screen_to_world(vec2(press.x - rect.min.x, press.y - rect.min.y));
                        let (wx, wy) = (f64::from(world.x), f64::from(world.y));
                        if let Some(p) = layout.placements.iter().rev().find(|p| {
                            let f = p.footprint();
                            wx >= f.x && wx <= f.right() && wy >= f.y && wy <= f.bottom()
                        }) {
                            let f = p.footprint();
                            self.cabinet.drag = Some(Drag {
                                id: p.id,
                                grab: vec2(world.x - f.x as f32, world.y - f.y as f32),
                                at: (f.x, f.y),
                            });
                        }
                    }
                    if let (Some(d), Some(at)) = (self.cabinet.drag.as_mut(), out.cursor_raw) {
                        let (x, y) =
                            layout.snapped(f64::from(at.x - d.grab.x), f64::from(at.y - d.grab.y));
                        d.at = (x, y);
                    }
                    if out.response.drag_stopped()
                        && let Some(d) = self.cabinet.drag.take()
                    {
                        ask = Some(Ask::Drop(d.id, d.at.0, d.at.1));
                    }
                    // Rotate and delete act on the selected devices.
                    let selected: Vec<u64> = self
                        .cabinet
                        .view
                        .selection
                        .ids()
                        .iter()
                        .filter_map(|i| novo_canvas::cabinet::box_of(*i))
                        .collect();
                    for key in std::mem::take(&mut self.cabinet.keys) {
                        match key.as_str() {
                            "edit.rotate" if !selected.is_empty() => {
                                ask = Some(Ask::Rotate(selected.clone()));
                            }
                            "edit.delete" if !selected.is_empty() => {
                                ask = Some(Ask::Remove(selected.clone()));
                            }
                            _ => {}
                        }
                    }
                } else {
                    self.cabinet.drag = None;
                    self.cabinet.ghost = None;
                    self.cabinet.keys.clear();
                }
            }
            None => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.weak(match (&location, structure) {
                        (None, _) => "No panel yet. Give a page a panel name (+K1) to lay it out.",
                        (Some(_), None) => "This panel has no cabinet yet.",
                        _ => "Set the size of the mounting plate to start placing devices.",
                    });
                });
            }
        }

        if let Some(ask) = ask {
            self.cabinet_ask(ask, structure, layout, &cands);
        }
    }

    pub(super) fn cabinet_ask(
        &mut self,
        ask: Ask,
        structure: Option<StructureId>,
        layout: Option<CabinetLayout>,
        cands: &[novo_layout::plan::Candidate],
    ) {
        match ask {
            Ask::Panel(l) => {
                self.cabinet.location = Some(l);
                self.cabinet.preview = None;
            }
            Ask::Cancel => self.cabinet.preview = None,
            Ask::PlaceAll => {
                if let Some(layout) = layout {
                    let plan = layout.preview(cands);
                    if plan.placements.is_empty() {
                        self.message(Level::Info, "Nothing fits on the plate");
                    } else {
                        if !plan.unplaced.is_empty() {
                            self.message(
                                Level::Warning,
                                format!("{} devices do not fit on the plate", plan.unplaced.len()),
                            );
                        }
                        self.cabinet.preview = Some(plan);
                    }
                }
            }
            Ask::Accept => {
                let (Some(plan), Some(structure)) = (self.cabinet.preview.take(), structure) else {
                    return;
                };
                self.cabinet_edit("Place all", move |tx| apply_plan(tx, structure, &plan));
            }
            Ask::Drop(device, x, y) => {
                let Some(structure) = structure else {
                    return;
                };
                let rotated = layout
                    .as_ref()
                    .and_then(|l| l.placements.iter().find(|p| p.id == device))
                    .is_some_and(|p| p.rotated);
                self.cabinet_edit("Place device", move |tx| {
                    place_device(tx, structure, device, x, y, rotated)
                });
            }
            Ask::Rotate(devices) => {
                self.cabinet_edit("Rotate device", move |tx| {
                    devices.iter().try_for_each(|d| rotate_device(tx, *d))
                });
            }
            Ask::Remove(devices) => {
                self.cabinet_edit("Remove from plate", move |tx| {
                    devices.iter().try_for_each(|d| unplace_device(tx, *d))
                });
            }
            Ask::SetPlate(s) => {
                self.cabinet_edit("Set plate", move |tx| set_plate(tx, s, DEFAULT_PLATE));
            }
            Ask::CreateCabinet(tag) => {
                let command = Command::AddStructure {
                    tag,
                    description: String::new(),
                    enclosure: true,
                };
                self.cabinet_edit("Create cabinet", move |tx| tx.execute(command).map(|_| ()));
            }
        }
    }

    /// Runs an edit of the model from the Cabinet canvas as one undo step.
    fn cabinet_edit(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<()>,
    ) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact(label, edit) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("{label} failed: {e}")),
        }
    }
}
