//! The Cabinet canvas (Ctrl+2): a panel's mounting plate, rails and placed
//! devices, drawn by novo-canvas from the layout novo-layout reads out of the
//! model. `Place all` previews a plan on the canvas; Enter accepts it as one
//! undo step, Escape drops it (docs/design/cabinet-layout.md).

use eframe::egui;
use novo_canvas::cabinet::{
    BoxState, BoxView, CabinetScene, CabinetView, DuctView, PlateView, RailView, view_bounds,
};
use novo_canvas::widget::CanvasView;
use novo_canvas::{Vec2, vec2};
use novo_core::{Command, Device, DeviceId, DeviceTag, MountingPlate, StructureId};
use novo_layout::model::{
    apply_plan, candidates, load, place_device, rotate_device, set_enclosure, set_plate,
    unplace_device,
};
use novo_layout::plan::Plan;
use novo_layout::state::CabinetLayout;

use super::cabinet_door::DoorState;
use super::cabinet_inspector;
use super::cabinet_routes;
use super::cabinet_rules;
use super::{Level, NovoApp};
use crate::theme;

/// The plate a new cabinet gets, in millimetres: a common 600 x 800 panel.
const DEFAULT_PLATE: MountingPlate = MountingPlate {
    width: 600.0,
    height: 800.0,
    margin: 10.0,
};

pub(super) struct CabinetState {
    pub(super) view: CanvasView,
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
    /// The door and depth panel: its form and the model value it follows.
    door: DoorState,
    /// Text buffers of the inspector's panel-data section.
    pub(super) panel_fields: super::cabinet_panel::PanelFields,
    /// The table face (Ctrl+T) is shown instead of the plate.
    pub(super) table: bool,
    /// The plate card of the inspector is shown with nothing selected.
    pub(super) plate_card: bool,
    /// Every wire's route is drawn over the plate, with the route table
    /// beside it (Route all).
    pub(super) routes: bool,
    routes_cache: cabinet_routes::OverlayCache,
    /// The place rules, why `Place all` chose what it did, and the cut list.
    pub(super) rules: cabinet_rules::RulesState,
    /// The Heat view over the plate.
    pub(super) heat: super::heat::HeatState,
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
            door: DoorState::default(),
            panel_fields: Default::default(),
            table: false,
            plate_card: false,
            routes: false,
            routes_cache: Default::default(),
            rules: cabinet_rules::RulesState::new(),
            heat: super::heat::HeatState::new(),
        }
    }
}

/// What the header strip asked for this frame.
pub(super) enum Ask {
    Panel(DeviceTag),
    CreateCabinet(DeviceTag),
    SetPlate(StructureId),
    /// Set the plate to new dimensions from the inspector.
    SetPlateSize(StructureId, MountingPlate),
    PlaceAll,
    /// Export the plate drawing PDF of the shown enclosure.
    ExportPlate,
    Accept,
    Cancel,
    ToggleTable,
    TogglePlateCard,
    ToggleRoutes,
    ExportWireCut,
    /// Set a wire production rule of the project (`None` back to its default).
    SetWireRule(&'static str, Option<f64>),
    ExportWireProduction,
    ExportWiringDxf(StructureId),
    /// Select a placed device (from the problems list), on the plate.
    Select(u64),
    /// Place or move a device with its top left at `(x, y)` mm.
    Drop(u64, f64, f64),
    Rotate(Vec<u64>),
    Remove(Vec<u64>),
    SetEnclosure(novo_layout::enclosure::Enclosure),
    /// Set a panel-building value (power loss, mounting, clearance) of a device.
    SetPanelValue(super::cabinet_panel::PanelEdit),
    /// Cut rails to new lengths (rail id, length in mm), from the cut list.
    SetRailLengths(Vec<(u64, f64)>),
}

/// The Cabinet drawing of a layout, with the layout's problems as box states.
pub(super) fn cabinet_view(layout: &CabinetLayout, tag_of: &impl Fn(u64) -> String) -> CabinetView {
    let report = layout.report();
    let deep: Vec<u64> = layout
        .door_report()
        .map(|d| d.too_deep.iter().map(|&(id, _)| id).collect())
        .unwrap_or_default();
    let state = |id: u64| {
        if report.collisions.iter().any(|&(a, b)| a == id || b == id)
            || report.outside.contains(&id)
        {
            if report.outside.contains(&id) {
                BoxState::Outside
            } else {
                BoxState::Collision
            }
        } else if deep.contains(&id) {
            BoxState::TooDeep
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
        ducts: layout
            .ducts
            .iter()
            .map(|d| DuctView {
                id: d.id,
                x: d.x as f32,
                y: d.y as f32,
                length: d.length as f32,
                width: d.width as f32,
                vertical: d.vertical,
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
                    clearance: p.max_need() as f32,
                    state: state(p.id),
                }
            })
            .collect(),
    }
}

/// What is wrong with a placed device, in words: the hover text of its mark
/// and the table's Problems column.
pub(super) fn problem_lines(
    layout: &CabinetLayout,
    id: u64,
    tag_of: &impl Fn(u64) -> String,
) -> Vec<String> {
    let report = layout.report();
    let mut lines = Vec::new();
    for &(a, b) in &report.collisions {
        if a == id || b == id {
            lines.push(format!("Overlaps {}", tag_of(if a == id { b } else { a })));
        }
    }
    if report.outside.contains(&id) {
        lines.push("Reaches past the usable plate".to_owned());
    }
    for &(a, b, gap) in &report.too_close {
        if a == id || b == id {
            lines.push(format!(
                "Only {gap:.0} mm from {}",
                tag_of(if a == id { b } else { a })
            ));
        }
    }
    if let Some(door) = layout.door_report() {
        for &(d, over) in &door.too_deep {
            if d == id {
                lines.push(format!("{over:.0} mm too deep: the door cannot close"));
            }
        }
    }
    if let Some(p) = layout.placements.iter().find(|p| p.id == id) {
        for &(rail, over) in &report.rail_overflows {
            if p.rail == Some(rail) {
                lines.push(format!("Its rail is {over:.0} mm too short"));
            }
        }
        if p.size.guessed {
            lines.push("Size is a guess: the part has none".to_owned());
        }
    }
    lines
}

/// The table face (Ctrl+T): every device of the panel with where it is and
/// what is wrong.
/// A sort key in which numbers count by value: -K2 comes before -K10.
pub(super) fn natural_key(tag: &str) -> String {
    let mut key = String::new();
    let mut digits = String::new();
    for ch in tag.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if !digits.is_empty() {
            key.push_str(&format!("{digits:0>12}"));
            digits.clear();
        }
        key.push(ch);
    }
    key.pop();
    key
}

pub(super) fn cabinet_table(
    ui: &mut egui::Ui,
    layout: &CabinetLayout,
    cands: &[novo_layout::plan::Candidate],
    tag_of: &impl Fn(u64) -> String,
    selected: &[u64],
) -> Option<u64> {
    let mut clicked = None;
    let mut rows: Vec<_> = cands.iter().collect();
    rows.sort_by_key(|c| natural_key(&tag_of(c.id)));
    // Numbers read best flush right.
    let num = |ui: &mut egui::Ui, text: String| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(text);
        });
    };
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("cabinet_table")
            .striped(true)
            .num_columns(7)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                for head in [
                    "Device",
                    "Size (mm)",
                    "X",
                    "Y",
                    "Turned",
                    "Rail",
                    "Problems",
                ] {
                    ui.strong(head);
                }
                ui.end_row();
                for c in rows {
                    if ui
                        .selectable_label(selected.contains(&c.id), tag_of(c.id))
                        .clicked()
                    {
                        clicked = Some(c.id);
                    }
                    let size = format!("{} x {}", c.size.width, c.size.height);
                    match layout.placements.iter().find(|p| p.id == c.id) {
                        Some(p) => {
                            num(ui, size);
                            num(ui, format!("{}", p.x));
                            num(ui, format!("{}", p.y));
                            ui.label(if p.rotated { "yes" } else { "" });
                            ui.label(p.rail.map(|r| format!("#{r}")).unwrap_or_default());
                            ui.label(problem_lines(layout, c.id, tag_of).join("; "));
                        }
                        None => {
                            num(ui, size);
                            ui.weak("");
                            ui.weak("");
                            ui.weak("");
                            ui.weak("");
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                "Unplaced (use Place all, or drag it in)",
                            );
                        }
                    }
                    ui.end_row();
                }
            });
    });
    clicked
}

impl NovoApp {
    /// The devices selected on the plate.
    fn cabinet_selected(&self) -> Vec<u64> {
        self.cabinet
            .view
            .selection
            .ids()
            .iter()
            .filter_map(|i| novo_canvas::cabinet::box_of(*i))
            .collect()
    }

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
        self.cabinet.rules.ensure_loaded();
        let judged = match (structure, &layout) {
            (Some(s), Some(_)) => Some(cabinet_rules::judge(project, s, &self.cabinet.rules.rules)),
            _ => None,
        };
        let cands = structure
            .map(|s| candidates(project, s))
            .unwrap_or_default();
        let sizing_devices = structure
            .map(|s| novo_layout::sizing_model::sizing_devices(project, s))
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
                    + r.rail_overflows.len()
                    + s.door_report().map_or(0, |d| d.problems());
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
                if !self.cabinet.table
                    && ui
                        .selectable_label(self.cabinet.plate_card, "Plate")
                        .on_hover_text("Size and fill of the mounting plate")
                        .clicked()
                {
                    ask = Some(Ask::TogglePlateCard);
                }
                if !self.cabinet.table
                    && ui
                        .selectable_label(self.cabinet.routes, "Route all")
                        .on_hover_text(
                            "Route every wire through the ducts: lengths, duct fill and problems",
                        )
                        .clicked()
                {
                    ask = Some(Ask::ToggleRoutes);
                }
                if !self.cabinet.table {
                    super::heat::chips(ui, &mut self.cabinet.heat);
                }
                if ui
                    .selectable_label(self.cabinet.table, "Table")
                    .on_hover_text("Devices as a table (Ctrl+T)")
                    .clicked()
                {
                    ask = Some(Ask::ToggleTable);
                }
                if ui
                    .button("Plate drawing")
                    .on_hover_text("Plate drawing with drill table (PDF)")
                    .clicked()
                {
                    ask = Some(Ask::ExportPlate);
                }
                if ui.button("Size enclosure").clicked() {
                    self.sizing.panel_open = !self.sizing.panel_open;
                }
                if ui
                    .button("Rules")
                    .on_hover_text("Where Place all puts each kind of device")
                    .clicked()
                {
                    self.cabinet.rules.editor_open ^= true;
                }
                if ui
                    .button("Cut list")
                    .on_hover_text("Bars of rail and duct to buy and cut")
                    .clicked()
                {
                    self.cabinet.rules.cut_open ^= true;
                }
                ui.weak(format!("{placed} placed, {unplaced} unplaced"));
                let label = format!(
                    "{problems} {}",
                    if problems == 1 { "problem" } else { "problems" }
                );
                if problems == 0 {
                    ui.weak(label);
                } else if let Some(shown) = &shown {
                    // The count opens the list; a row selects its device.
                    ui.menu_button(label, |ui| {
                        for p in &shown.placements {
                            let lines = problem_lines(shown, p.id, &tag_of);
                            if lines.is_empty() {
                                continue;
                            }
                            if ui
                                .button(format!("{}: {}", tag_of(p.id), lines.join("; ")))
                                .clicked()
                            {
                                ask = Some(Ask::Select(p.id));
                                ui.close();
                            }
                        }
                    });
                }
                if let Some(j) = judged.as_ref().filter(|j| !j.violations.is_empty()) {
                    let n = j.violations.len();
                    ui.weak(format!(
                        "{n} rule {}",
                        if n == 1 { "problem" } else { "problems" }
                    ))
                    .on_hover_ui(|ui| {
                        for v in j.violations.iter().take(12) {
                            ui.label(v.describe(&self.cabinet.rules.rules, &|id| tag_of(id)));
                        }
                    });
                }
            }
        });
        ui.separator();

        match shown {
            Some(shown) if self.cabinet.table => {
                // Keys, drags and the landing box belong to the plate: none
                // may wait there while the table is shown.
                self.cabinet.keys.clear();
                self.cabinet.drag = None;
                self.cabinet.ghost = None;
                let selected = self.cabinet_selected();
                if let Some(id) = cabinet_table(ui, &shown, &cands, &tag_of, &selected) {
                    // A row is its device: select it and show it on the plate.
                    ask = Some(Ask::Select(id));
                }
            }
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

                if self.cabinet.routes
                    && let Some(s) = structure
                {
                    let overlay = self.cabinet.routes_cache.get(project, s);
                    if let Some(a) = cabinet_routes::panel(ui, overlay, s) {
                        ask = Some(a);
                    }
                }

                let selected: Vec<u64> = self
                    .cabinet
                    .view
                    .selection
                    .ids()
                    .iter()
                    .filter_map(|i| novo_canvas::cabinet::box_of(*i))
                    .collect();
                // The inspector: with a selection, or the plate card when it
                // is switched on; otherwise the plate gets the whole window.
                let inspector = match (&layout, structure) {
                    (Some(stored), Some(s)) if !selected.is_empty() || self.cabinet.plate_card => {
                        let mut facts = cabinet_inspector::gather(
                            project, s, stored, &shown, &selected, &tag_of,
                        );
                        if let Some(j) = &judged {
                            cabinet_rules::annotate(&mut facts, &self.cabinet.rules, j, &tag_of);
                        }
                        // On the side away from the selection.
                        let centre = shown
                            .placements
                            .iter()
                            .filter(|p| selected.contains(&p.id))
                            .map(|p| p.footprint().x + p.footprint().w / 2.0)
                            .sum::<f64>()
                            / selected.len().max(1) as f64;
                        Some((facts, centre > shown.plate.width / 2.0))
                    }
                    _ => None,
                };

                let view = cabinet_view(&shown, &tag_of);
                self.cabinet.scene.set(&view);
                if self.cabinet.fitted != location {
                    self.cabinet.view.viewport.fit(&view_bounds(&view), 24.0);
                    self.cabinet.fitted = location.clone();
                }
                let heat_input = self.cabinet.heat.on.then(|| {
                    let items = shown
                        .placements
                        .iter()
                        .map(|p| {
                            let f = p.footprint();
                            let loss =
                                project
                                    .get::<Device>(DeviceId::from(p.id))
                                    .map_or(0.0, |d| {
                                        let s =
                                            novo_layout::sizing_model::sizing_device(project, d);
                                        s.power_loss * s.load_factor
                                    });
                            super::heat::Item {
                                id: p.id,
                                tag: tag_of(p.id),
                                x: f.x,
                                y: f.y,
                                w: f.w,
                                h: f.h,
                                loss_w: loss,
                            }
                        })
                        .collect();
                    super::heat::Input {
                        plate: (shown.plate.width, shown.plate.height),
                        ambient: self.sizing.request().ambient_max,
                        items,
                        solution: self.cabinet.heat.solution(),
                    }
                });
                if self.cabinet.heat.take_request() {
                    match (structure, &heat_input) {
                        (Some(s), Some(input)) => super::heat::start_run(
                            &mut self.cabinet.heat,
                            project,
                            s,
                            input,
                            ui.ctx(),
                        ),
                        _ => self
                            .cabinet
                            .heat
                            .fail("Open an enclosure with a mounting plate to simulate".into()),
                    }
                }
                let dragging = self.cabinet.drag.is_some();
                // The map goes under the canvas: it paints no background of
                // its own while the map is on.
                let backdrop = theme::canvas_style().background;
                self.cabinet.view.style.background = if self.cabinet.heat.under_canvas() {
                    novo_canvas::Color([0, 0, 0, 0])
                } else {
                    backdrop
                };
                let canvas_rect = ui.available_rect_before_wrap();
                if let Some(input) = &heat_input {
                    let [r, g, b, a] = backdrop.0;
                    super::heat::paint_under(
                        ui,
                        canvas_rect,
                        &self.cabinet.view.viewport,
                        &mut self.cabinet.heat,
                        input,
                        dragging,
                        egui::Color32::from_rgba_unmultiplied(r, g, b, a),
                    );
                }
                let out = self.cabinet.view.show(ui, self.cabinet.scene.scene());
                if self.cabinet.routes
                    && let Some(s) = structure
                {
                    let overlay = self.cabinet.routes_cache.get(project, s);
                    let rect = out.response.rect;
                    cabinet_routes::draw(
                        &ui.painter_at(rect),
                        rect,
                        &self.cabinet.view.viewport,
                        overlay,
                    );
                }
                if let Some(input) = &heat_input {
                    super::heat::paint_over(
                        ui,
                        out.response.rect,
                        &self.cabinet.view.viewport,
                        &mut self.cabinet.heat,
                        input,
                        dragging,
                    );
                }
                if let Some((facts, left)) = &inspector
                    && let Some(a) = cabinet_inspector::overlay(
                        ui.ctx(),
                        out.response.rect,
                        *left,
                        facts,
                        project,
                        &mut self.cabinet.panel_fields,
                        cabinet_inspector::DoorView {
                            state: &mut self.cabinet.door,
                            layout: &shown,
                            tag_of: &tag_of,
                        },
                    )
                {
                    ask = Some(a);
                }
                if let Some(id) = out.hovered.and_then(novo_canvas::cabinet::box_of) {
                    let lines = problem_lines(&shown, id, &tag_of);
                    out.response.clone().on_hover_ui_at_pointer(|ui| {
                        ui.strong(tag_of(id));
                        if lines.is_empty() {
                            ui.weak("No problems");
                        }
                        for line in &lines {
                            ui.label(line);
                        }
                    });
                }
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
                    let selected = self.cabinet_selected();
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

        super::sizing::show_window(ui.ctx(), &mut self.sizing, &sizing_devices);
        super::sizing::show_editor(ui.ctx(), &mut self.sizing);
        cabinet_rules::show_rules_window(ui.ctx(), &mut self.cabinet.rules);
        let lists = if self.cabinet.rules.cut_open {
            structure.and_then(|s| cabinet_rules::cut_lists(project, s))
        } else {
            None
        };
        if let Some(a) =
            cabinet_rules::show_cut_window(ui.ctx(), &mut self.cabinet.rules, lists.as_ref())
        {
            ask = Some(a);
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
                self.cabinet.rules.explain = None;
            }
            Ask::Cancel => self.cabinet.preview = None,
            Ask::ToggleTable => self.cabinet.table ^= true,
            Ask::TogglePlateCard => self.cabinet.plate_card ^= true,
            Ask::ExportPlate => {
                self.export_plate_pdf();
            }
            Ask::ToggleRoutes => self.cabinet.routes ^= true,
            Ask::SetWireRule(key, value) => {
                self.cabinet_edit("Wire rule", move |tx| {
                    novo_routing::project_rules::set_rule(tx, key, value)
                });
            }
            Ask::ExportWireCut => {
                self.export_wire_cut();
            }
            Ask::ExportWireProduction => {
                self.export_wire_production();
            }
            Ask::ExportWiringDxf(s) => {
                self.export_wiring_dxf(s);
            }
            Ask::Select(device) => {
                self.cabinet.table = false;
                self.cabinet.view.selection.apply(
                    [novo_canvas::cabinet::box_item(device)],
                    novo_canvas::selection::SelectOp::Replace,
                );
            }
            Ask::PlaceAll => {
                if let Some(layout) = layout {
                    self.cabinet.rules.ensure_loaded();
                    let project = self.backend.document().map(|d| d.project());
                    let plan = match (structure, project) {
                        (Some(s), Some(project)) => {
                            let (plan, why) =
                                cabinet_rules::plan(project, s, &layout, &self.cabinet.rules.rules);
                            self.cabinet.rules.explain = Some(why);
                            plan
                        }
                        _ => layout.preview(cands),
                    };
                    if plan.placements.is_empty() {
                        self.message(Level::Info, "Nothing fits on the plate");
                    } else {
                        if !plan.unplaced.is_empty() {
                            self.message(
                                Level::Warning,
                                format!("{} devices do not fit on the plate", plan.unplaced.len()),
                            );
                        }
                        if self
                            .cabinet
                            .rules
                            .explain
                            .as_ref()
                            .is_some_and(|e| e.separation_relaxed)
                        {
                            self.message(
                                Level::Warning,
                                "The plate is too small to keep power and signal ducts apart; \
                                 the separation was given up to fit every device",
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
            Ask::SetRailLengths(lengths) => {
                self.cabinet_edit("Cut rails to length", move |tx| {
                    for (id, length) in lengths {
                        let rail = novo_core::RailId::from(id);
                        let Some(r) = tx.project().get::<novo_core::Rail>(rail) else {
                            continue;
                        };
                        let (width, height) = (r.width, r.height);
                        tx.execute(Command::SetRailSize {
                            rail,
                            length,
                            width,
                            height,
                        })?;
                    }
                    Ok(())
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
            Ask::SetEnclosure(enclosure) => {
                let Some(structure) = structure else {
                    return;
                };
                self.cabinet_edit("Set inside and door", move |tx| {
                    set_enclosure(tx, structure, &enclosure)
                });
            }
            Ask::SetPanelValue(edit) => {
                let Some(doc) = self.backend.document() else {
                    return;
                };
                match super::cabinet_panel::command(doc.project(), &edit) {
                    Ok(Some(command)) => {
                        self.cabinet_edit("Set panel data", move |tx| {
                            tx.execute(command).map(|_| ())
                        });
                    }
                    Ok(None) => {}
                    Err(e) => self.message(Level::Warning, e.to_string()),
                }
            }
            Ask::SetPlateSize(s, plate) => {
                self.cabinet_edit("Set plate size", move |tx| set_plate(tx, s, plate));
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

    /// Route all (Ctrl+K): shows the Cabinet with every wire's route.
    pub(super) fn toggle_cabinet_routes(&mut self) {
        if self.backend.document().is_none() {
            return;
        }
        if self.workspace() == super::workspace::Workspace::Cabinet && self.cabinet.routes {
            self.cabinet.routes = false;
            return;
        }
        self.switch_workspace(super::workspace::Workspace::Cabinet);
        self.cabinet.table = false;
        self.cabinet.routes = true;
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
