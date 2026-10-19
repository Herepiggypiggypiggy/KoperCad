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
use novo_layout::surfaces::MAIN_PLATE;

use super::cabinet_bar::{self, Foldable};
use super::cabinet_door::DoorState;
use super::cabinet_inspector;
use super::cabinet_routes;
use super::cabinet_rules;
use super::{Level, NovoApp};
use crate::theme;
use novo_theme::icons;

/// The plate a new cabinet gets, in millimetres: a common 600 x 800 panel.
const DEFAULT_PLATE: MountingPlate = MountingPlate {
    width: 600.0,
    height: 800.0,
    margin: 10.0,
};

/// What the plate Place all would need was worked out for: the panel, its
/// plate, how many devices are placed and not, and the rules.
type FitKey = (
    StructureId,
    [u64; 3],
    usize,
    usize,
    novo_layout::rules::RuleSet,
);

pub(super) struct CabinetState {
    pub(super) view: CanvasView,
    pub(super) scene: CabinetScene,
    /// The panel shown, by its location (`+BAY1G1`).
    pub(super) location: Option<DeviceTag>,
    /// A `Place all` plan waiting for Enter.
    pub(super) preview: Option<Plan>,
    /// The part of that plan for other surfaces (the door, a side wall):
    /// devices the surface rules send there, stored together with it.
    pub(super) surface_preview: Vec<(String, Plan)>,
    /// The plate that would hold every device, kept until its key changes.
    fit: Option<(FitKey, Option<MountingPlate>)>,
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
    pub(super) door: DoorState,
    /// Text buffers of the inspector's panel-data section.
    pub(super) panel_fields: super::cabinet_panel::PanelFields,
    /// The table face (Ctrl+T) is shown instead of the plate.
    pub(super) table: bool,
    /// The plate card of the inspector is shown with nothing selected.
    pub(super) plate_card: bool,
    /// The surface picked in the header (`None` is the mounting plate) and
    /// the picker asked to open by `cabinet.surface`.
    pub(super) surface: Option<String>,
    pub(super) surface_open: bool,
    /// The id of the picker's popup, as egui's combo box keys it.
    pub(super) surface_popup: Option<egui::Id>,
    /// Every wire's route is drawn over the plate, with the route table
    /// beside it (Route all).
    pub(super) routes: bool,
    routes_cache: cabinet_routes::OverlayCache,
    /// The place rules, why `Place all` chose what it did, and the cut list.
    pub(super) rules: cabinet_rules::RulesState,
    /// The Heat view over the plate.
    pub(super) heat: super::heat::HeatState,
    /// The list for adding, moving, resizing and deleting rails and ducts.
    pub(super) rails: super::cabinet_rails::RailsState,
    /// The right-click menu of the plate, while open.
    pub(super) menu: Option<super::cabinet_menu::PlateMenu>,
    /// Where the plate canvas was drawn last frame (for tests and menus).
    pub(super) plate_rect: Option<egui::Rect>,
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
    /// The theme changed: the plate view and scene take the new colours
    /// (the scene is filled again every frame).
    pub(super) fn restyle(&mut self) {
        self.view.style = theme::canvas_style();
        self.scene = CabinetScene::new(theme::cabinet_style());
    }

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
            surface_preview: Vec::new(),
            fit: None,
            fitted: None,
            drag: None,
            ghost: None,
            keys: Vec::new(),
            door: DoorState::default(),
            panel_fields: Default::default(),
            table: false,
            plate_card: false,
            surface: None,
            surface_open: false,
            surface_popup: None,
            routes: false,
            routes_cache: Default::default(),
            rules: cabinet_rules::RulesState::new(),
            heat: super::heat::HeatState::new(),
            rails: Default::default(),
            menu: None,
            plate_rect: None,
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
    ExportPlateDxf,
    Accept,
    Cancel,
    ToggleTable,
    TogglePlateCard,
    ToggleRoutes,
    ExportWireCut,
    /// Set a wire production rule of the project (`None` back to its default).
    SetWireRule(&'static str, Option<f64>),
    ExportWireProduction,
    OpenBench,
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
    /// Put a placed device on another surface of the cabinet (plate, door, ...).
    SetSurface(u64, String),
    /// The size of a surface other than the plate (`None`: planning values).
    SetSurfaceSize(
        StructureId,
        String,
        Option<novo_layout::surfaces::SurfaceSize>,
    ),
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
    for c in &report.opening_conflicts {
        if c.device == id {
            lines.push(format!(
                "Covers the wiring opening '{}' [{}]",
                c.opening,
                novo_layout::openings::RULE_OPENING_KEEPOUT
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
    for o in &report.duct_overlaps {
        if let novo_layout::DuctHit::Device {
            id: device,
            across,
            along,
        } = o.hit
            && device == id
        {
            lines.push(format!(
                "Lies under a duct ({across:.0} x {along:.0} mm): move it clear"
            ));
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

/// One line for every rail that runs under a duct (a rail has no tag, so
/// it is named by where it lies).
pub(super) fn rail_duct_lines(layout: &CabinetLayout) -> Vec<String> {
    layout
        .report()
        .duct_overlaps
        .iter()
        .filter_map(|o| {
            let novo_layout::DuctHit::Rail { id, by } = o.hit else {
                return None;
            };
            let rail = layout.rails.iter().find(|r| r.id == id)?;
            let (axis, at) = if rail.vertical {
                ("x", rail.x)
            } else {
                ("y", rail.y)
            };
            Some(format!(
                "The rail at {axis} {at:.0} mm runs under a duct for {by:.0} mm: move the rail or the duct"
            ))
        })
        .collect()
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
    pub(super) fn cabinet_selected(&self) -> Vec<u64> {
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
        // The open redlines are outlined on the plate, so their store has to
        // be there even when Commissioning has not been visited.
        self.load_commissioning();
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
            self.cabinet.surface_preview.clear();
        }
        let location = self.cabinet.location.clone();
        let structure: Option<StructureId> = location.as_ref().and_then(|l| {
            project
                .structures()
                .find(|s| s.enclosure && &s.tag == l)
                .map(|s| s.id)
        });
        let layout = structure.and_then(|s| load(project, s));
        let surfaces = structure.and_then(|s| super::cabinet_surfaces::summarize(project, s));
        // Said once the frame is done with the project borrow.
        let mut notices: Vec<String> = Vec::new();
        if let Some(sum) = &surfaces
            && let Some(gone) = self
                .cabinet
                .surface
                .clone()
                .filter(|n| sum.row(n).is_none())
        {
            self.cabinet.surface = None;
            notices.push(format!(
                "Surface '{gone}' no longer exists; showing the mounting plate."
            ));
        }
        // A picked surface other than the plate is drawn from its own
        // layout; the stored plate layout is untouched.
        let on_surface: Option<String> = self
            .cabinet
            .surface
            .clone()
            .filter(|n| n.as_str() != MAIN_PLATE);
        let layout = match (&on_surface, &surfaces, &layout) {
            (Some(n), Some(sum), Some(l)) => {
                let found = super::cabinet_surfaces::surface_layout(l, sum, n);
                if found.is_none() {
                    // The surface is gone: say so, and go back to the plate
                    // instead of drawing the plate under another name.
                    self.cabinet.surface = None;
                    notices.push(format!(
                        "Surface '{n}' no longer exists; showing the mounting plate."
                    ));
                }
                found.or(layout)
            }
            // The plate view draws what is mounted on the plate; devices put
            // on the door or a wall show on that surface instead.
            (None, Some(sum), Some(_)) => layout.map(|mut l| {
                l.placements.retain(|p| {
                    sum.assigned
                        .get(&p.id)
                        .is_none_or(|name| name.as_str() == MAIN_PLATE)
                });
                l
            }),
            _ => layout,
        };
        self.cabinet.rules.ensure_loaded();
        let judged = match (structure, &layout) {
            (Some(s), Some(_)) if on_surface.is_none() => {
                Some(cabinet_rules::judge(project, s, &self.cabinet.rules.rules))
            }
            _ => None,
        };
        let mut cands = structure
            .map(|s| candidates(project, s))
            .unwrap_or_default();
        if let Some(l) = on_surface.as_ref().and(layout.as_ref()) {
            cands.retain(|c| l.placements.iter().any(|p| p.id == c.id));
        } else if let Some(sum) = &surfaces {
            // A device mounted on the door or a wall is placed, not waiting
            // for a spot on the plate.
            super::cabinet_surface_move::keep_plate_candidates(&mut cands, sum);
        }
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
        // On the door or a wall the preview is that surface's part of the
        // plan, not the plate plan.
        let all_cands = structure
            .map(|s| candidates(project, s))
            .unwrap_or_default();
        let shown = layout
            .as_ref()
            .map(|l| match (&self.cabinet.preview, &on_surface) {
                (Some(plan), None) => {
                    let mut preview = l.clone();
                    preview.accept(plan, &cands);
                    preview
                }
                (Some(_), Some(name)) => {
                    let mut preview = l.clone();
                    if let Some((_, plan)) = self
                        .cabinet
                        .surface_preview
                        .iter()
                        .find(|(surface, _)| surface == name)
                    {
                        preview.accept(plan, &all_cands);
                    }
                    preview
                }
                _ => l.clone(),
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
                    + r.opening_conflicts.len()
                    + s.opening_problems.len()
                    + r.duct_overlaps.len()
                    + s.door_report().map_or(0, |d| d.problems())
                    + s.swing_report().map_or(0, |d| d.problems());
                (l.placements.len(), l.unplaced(&cands).len(), problems)
            }
            _ => (0, cands.len(), 0),
        };
        let has_plan = self.cabinet.preview.is_some();
        let current_plate = layout.as_ref().map(|l| MountingPlate {
            width: l.plate.width,
            height: l.plate.height,
            margin: l.plate.margin,
        });
        // Some devices do not fit: the plate that would hold them, found by
        // planning on bigger plates (what Place all would really do), offered
        // as one button instead of making the person do sums. Kept until the
        // plate, the devices or the rules change.
        let plate_offer = match (&layout, structure) {
            (Some(l), Some(s)) if unplaced > 0 => {
                self.cabinet.rules.ensure_loaded();
                let key = (
                    s,
                    [
                        l.plate.width.to_bits(),
                        l.plate.height.to_bits(),
                        l.plate.margin.to_bits(),
                    ],
                    placed,
                    unplaced,
                    self.cabinet.rules.rules.clone(),
                );
                if self.cabinet.fit.as_ref().is_none_or(|(k, _)| *k != key) {
                    let found =
                        cabinet_rules::plate_that_holds(project, s, l, &self.cabinet.rules.rules);
                    self.cabinet.fit = Some((key, found));
                }
                self.cabinet.fit.as_ref().and_then(|(_, p)| *p)
            }
            _ => None,
        };
        let homeless = structure.map_or(0, |s| {
            novo_layout::model::without_enclosure(project, s).len()
        });

        let mut ask: Option<Ask> = None;
        let place_key = self.command_shortcut_text(ui.ctx(), "cabinet.place-all");
        let plate_face = !self.cabinet.table;
        let mut anchors: Vec<cabinet_bar::Anchor> = Vec::new();
        let fold = {
            use cabinet_bar::{Foldable, GROUP_GAP, button_width, text_width};
            let pad = 2.0 * ui.spacing().button_padding.x;
            let panel = text_width(
                ui,
                &location
                    .as_ref()
                    .map_or_else(|| "No panel".to_owned(), ToString::to_string),
            ) + pad
                + ui.spacing().icon_width
                + 8.0;
            let place = 1.2 * button_width(ui, icons::SQUARES_FOUR, "Place all")
                + place_key
                    .as_deref()
                    .map_or(0.0, |k| text_width(ui, k) + 12.0);
            let route = if plate_face {
                GROUP_GAP + button_width(ui, icons::PATH, "Route all")
            } else {
                0.0
            };
            let view = button_width(ui, "", "Plate") + button_width(ui, "", "Table") + pad;
            let fixed = 4.0 + GROUP_GAP * 3.0 + panel + place + route + view;
            let more = GROUP_GAP + button_width(ui, icons::DOTS_THREE, "More");
            let mut items = vec![
                (
                    Foldable::Rules,
                    button_width(ui, icons::SLIDERS_HORIZONTAL, "Rules"),
                ),
                (
                    Foldable::Rails,
                    button_width(ui, icons::ROWS, "Rails and ducts"),
                ),
                (
                    Foldable::CutList,
                    button_width(ui, icons::SCISSORS, "Cut list"),
                ),
                (
                    Foldable::Export,
                    GROUP_GAP + button_width(ui, icons::EXPORT, "Export") + 8.0,
                ),
            ];
            if plate_face {
                items.push((
                    Foldable::Arrange,
                    GROUP_GAP
                        + button_width(ui, icons::RULER, "Plate setup")
                        + button_width(ui, icons::ARROWS_OUT, "Size enclosure"),
                ));
                items.push((Foldable::Heat, button_width(ui, icons::FLAME, "Heat")));
            }
            if layout.is_none() || has_plan {
                items.clear();
            }
            cabinet_bar::Fold::plan(ui.available_width(), fixed, more, &items)
        };
        let captions = ui.available_width() >= cabinet_bar::WIDE;
        let caption = |c: &'static str| captions.then_some(c);
        let mut all_placed_note = false;
        let mut delete_rail: Option<u64> = None;
        let mut move_rail: Option<super::cabinet_rails::RailDrag> = None;
        // Narrower than the fixed part of the bar (a 683 point window), the bar
        // scrolls sideways instead of running off the pane and the window.
        egui::ScrollArea::horizontal()
            .id_salt("cabinet_bar_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    cabinet_bar::group(ui, caption("Panel"), |ui| {
                        let current = location
                            .as_ref()
                            .map_or_else(|| "No panel".to_owned(), ToString::to_string);
                        egui::ComboBox::from_id_salt("cabinet_panel")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for l in &locations {
                                    if ui
                                        .selectable_label(
                                            location.as_ref() == Some(l),
                                            l.to_string(),
                                        )
                                        .clicked()
                                    {
                                        ask = Some(Ask::Panel(l.clone()));
                                    }
                                }
                            });
                    });
                    if layout.is_none() {
                        return;
                    }
                    if let Some(sum) = surfaces.as_ref().filter(|s| s.rows.len() > 1) {
                        cabinet_bar::group(ui, caption("Surface"), |ui| {
                            let picked = self.cabinet.surface.clone();
                            let current = picked.clone().unwrap_or_else(|| MAIN_PLATE.to_owned());
                            let combo = egui::ComboBox::from_id_salt("cabinet_surface")
                                .selected_text(current.clone());
                            let popup = ui.make_persistent_id("cabinet_surface").with("popup");
                            self.cabinet.surface_popup = Some(popup);
                            if std::mem::take(&mut self.cabinet.surface_open) {
                                egui::Popup::open_id(ui.ctx(), popup);
                            }
                            let r = combo.show_ui(ui, |ui| {
                                for row in &sum.rows {
                                    let on = row.name == current;
                                    let line = ui.selectable_label(on, row.label());
                                    if line.clicked() {
                                        self.cabinet.pick_surface(
                                            (row.name != MAIN_PLATE).then(|| row.name.clone()),
                                        );
                                    }
                                }
                            });
                            let r = r.response.on_hover_text(
                                "Surface to work on (View > Cabinet > Cabinet surfaces)",
                            );
                            cabinet_bar::note(&mut anchors, "cabinet.surface", "Surface", &r);
                            if !sum.problems.is_empty() {
                                let text = format!("{} surface problems", sum.problems.len());
                                cabinet_bar::pill(
                                    ui,
                                    &text,
                                    Some(novo_theme::widgets::Level::Warning),
                                )
                                .on_hover_text(sum.problems.join("\n"));
                            }
                        });
                    }
                    if has_plan {
                        return;
                    }
                    cabinet_bar::group(ui, caption("Place"), |ui| {
                        let r = cabinet_bar::primary(
                            ui,
                            icons::SQUARES_FOUR,
                            "Place all",
                            place_key.as_deref(),
                            unplaced > 0 || homeless > 0,
                        );
                        let r = if unplaced > 0 {
                            r.on_hover_text(format!(
                        "Place the {unplaced} unplaced devices by the rules. You review the \
                         preview before anything is stored."
                    ))
                        } else if homeless > 0 {
                            r.on_hover_text(format!(
                        "{homeless} devices have no cabinet: Place all names them and says why."
                    ))
                        } else {
                            r
                        };
                        cabinet_bar::note(&mut anchors, "cabinet.place_all", "Place all", &r);
                        if r.clicked() {
                            ask = Some(Ask::PlaceAll);
                        }
                        if !fold.has(Foldable::Rules) {
                            let rules =
                                cabinet_bar::button(ui, icons::SLIDERS_HORIZONTAL, "Rules", false)
                                    .on_hover_text("Where Place all puts each kind of device");
                            if cabinet_bar::note(&mut anchors, "cabinet.rules", "Rules", &rules)
                                .clicked()
                            {
                                self.cabinet.rules.editor_open ^= true;
                            }
                        }
                    });
                    if plate_face && !fold.has(Foldable::Arrange) {
                        cabinet_bar::group(ui, caption("Arrange"), |ui| {
                            let plate = cabinet_bar::button(
                                ui,
                                icons::RULER,
                                "Plate setup",
                                self.cabinet.plate_card,
                            )
                            .on_hover_text("Size, fill and door of the mounting plate");
                            if cabinet_bar::note(
                                &mut anchors,
                                "cabinet.plate",
                                "Plate setup",
                                &plate,
                            )
                            .clicked()
                            {
                                ask = Some(Ask::TogglePlateCard);
                            }
                            let size = cabinet_bar::button(
                                ui,
                                icons::ARROWS_OUT,
                                "Size enclosure",
                                self.sizing.panel_open,
                            );
                            if cabinet_bar::note(
                                &mut anchors,
                                "cabinet.sizing",
                                "Size enclosure",
                                &size,
                            )
                            .clicked()
                            {
                                self.sizing.panel_open = !self.sizing.panel_open;
                            }
                        });
                    }
                    if plate_face || !(fold.has(Foldable::Rails) && fold.has(Foldable::CutList)) {
                        cabinet_bar::group(ui, caption("Route"), |ui| {
                            if plate_face {
                                let routes = cabinet_bar::button(
                            ui,
                            icons::PATH,
                            "Route all",
                            self.cabinet.routes,
                        )
                        .on_hover_text(
                            "Route every wire through the ducts: lengths, duct fill and problems",
                        );
                                if cabinet_bar::note(
                                    &mut anchors,
                                    "cabinet.routes",
                                    "Route all",
                                    &routes,
                                )
                                .clicked()
                                {
                                    ask = Some(Ask::ToggleRoutes);
                                }
                            }
                            if !fold.has(Foldable::Rails) {
                                let rails = cabinet_bar::button(
                                    ui,
                                    icons::ROWS,
                                    "Rails and ducts",
                                    self.cabinet.rails.open,
                                )
                                .on_hover_text(super::tips::hover(
                                    ui.ctx(),
                                    "Add, move, resize and delete rails and ducts",
                                    "cabinet.rails",
                                ));
                                if cabinet_bar::note(
                                    &mut anchors,
                                    "cabinet.rails",
                                    "Rails and ducts",
                                    &rails,
                                )
                                .clicked()
                                {
                                    self.cabinet.rails.open ^= true;
                                }
                            }
                            if !fold.has(Foldable::CutList) {
                                let cut = cabinet_bar::button(
                                    ui,
                                    icons::SCISSORS,
                                    "Cut list",
                                    self.cabinet.rules.cut_open,
                                )
                                .on_hover_text("Bars of rail and duct to buy and cut");
                                if cabinet_bar::note(
                                    &mut anchors,
                                    "cabinet.cutlist",
                                    "Cut list",
                                    &cut,
                                )
                                .clicked()
                                {
                                    self.cabinet.rules.cut_open ^= true;
                                }
                            }
                        });
                    }
                    cabinet_bar::group(ui, caption("View"), |ui| {
                        ui.scope(|ui| {
                            // One joined control: no gap between the two halves.
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if cabinet_bar::segment(ui, "Plate", plate_face).clicked()
                                && !plate_face
                            {
                                ask = Some(Ask::ToggleTable);
                            }
                            let table = cabinet_bar::segment(ui, "Table", !plate_face)
                                .on_hover_text(super::tips::hover(
                                    ui.ctx(),
                                    "Devices as a table",
                                    "view.cabinet_table",
                                ));
                            if cabinet_bar::note(&mut anchors, "cabinet.table", "Table", &table)
                                .clicked()
                                && plate_face
                            {
                                ask = Some(Ask::ToggleTable);
                            }
                        });
                        if plate_face && !fold.has(Foldable::Heat) {
                            let heat = super::heat::toggle(ui, &mut self.cabinet.heat);
                            cabinet_bar::note(&mut anchors, "cabinet.heat", "Heat", &heat);
                        }
                    });
                    if fold.any() {
                        cabinet_bar::group(ui, caption("More"), |ui| {
                            let more =
                                ui.menu_button(format!("{}  More", icons::DOTS_THREE), |ui| {
                                    if fold.has(Foldable::Rules) {
                                        let r = cabinet_bar::button(
                                            ui,
                                            icons::SLIDERS_HORIZONTAL,
                                            "Rules",
                                            self.cabinet.rules.editor_open,
                                        )
                                        .on_hover_text("Where Place all puts each kind of device");
                                        if cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.rules",
                                            "Rules",
                                            &r,
                                        )
                                        .clicked()
                                        {
                                            self.cabinet.rules.editor_open ^= true;
                                            ui.close();
                                        }
                                    }
                                    if fold.has(Foldable::Arrange) {
                                        let plate = cabinet_bar::button(
                                            ui,
                                            icons::RULER,
                                            "Plate setup",
                                            self.cabinet.plate_card,
                                        )
                                        .on_hover_text("Size, fill and door of the mounting plate");
                                        if cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.plate",
                                            "Plate setup",
                                            &plate,
                                        )
                                        .clicked()
                                        {
                                            ask = Some(Ask::TogglePlateCard);
                                            ui.close();
                                        }
                                        let size = cabinet_bar::button(
                                            ui,
                                            icons::ARROWS_OUT,
                                            "Size enclosure",
                                            self.sizing.panel_open,
                                        );
                                        if cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.sizing",
                                            "Size enclosure",
                                            &size,
                                        )
                                        .clicked()
                                        {
                                            self.sizing.panel_open = !self.sizing.panel_open;
                                            ui.close();
                                        }
                                    }
                                    if fold.has(Foldable::Rails) {
                                        let rails = cabinet_bar::button(
                                            ui,
                                            icons::ROWS,
                                            "Rails and ducts",
                                            self.cabinet.rails.open,
                                        )
                                        .on_hover_text(super::tips::hover(
                                            ui.ctx(),
                                            "Add, move, resize and delete rails and ducts",
                                            "cabinet.rails",
                                        ));
                                        if cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.rails",
                                            "Rails and ducts",
                                            &rails,
                                        )
                                        .clicked()
                                        {
                                            self.cabinet.rails.open ^= true;
                                            ui.close();
                                        }
                                    }
                                    if fold.has(Foldable::CutList) {
                                        let cut = cabinet_bar::button(
                                            ui,
                                            icons::SCISSORS,
                                            "Cut list",
                                            self.cabinet.rules.cut_open,
                                        )
                                        .on_hover_text("Bars of rail and duct to buy and cut");
                                        if cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.cutlist",
                                            "Cut list",
                                            &cut,
                                        )
                                        .clicked()
                                        {
                                            self.cabinet.rules.cut_open ^= true;
                                            ui.close();
                                        }
                                    }
                                    if fold.has(Foldable::Heat) {
                                        let heat = super::heat::toggle(ui, &mut self.cabinet.heat);
                                        cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.heat",
                                            "Heat",
                                            &heat,
                                        );
                                    }
                                    if fold.has(Foldable::Export) {
                                        let item = cabinet_bar::button(
                                            ui,
                                            icons::EXPORT,
                                            "Plate drawing (PDF)",
                                            false,
                                        )
                                        .on_hover_text("Plate drawing with drill table");
                                        cabinet_bar::note(
                                            &mut anchors,
                                            "cabinet.plate_drawing",
                                            "Plate drawing",
                                            &item,
                                        );
                                        if item.clicked() {
                                            ask = Some(Ask::ExportPlate);
                                            ui.close();
                                        }
                                    }
                                    if cabinet_bar::button(
                                        ui,
                                        icons::EXPORT,
                                        "Plate drawing (DXF)",
                                        false,
                                    )
                                    .on_hover_text(
                                        "Plate drawing for a CNC or CAD, with the drill table",
                                    )
                                    .clicked()
                                    {
                                        ask = Some(Ask::ExportPlateDxf);
                                        ui.close();
                                    }
                                });
                            cabinet_bar::note(&mut anchors, "cabinet.more", "More", &more.response);
                            // Lessons point at folded controls: until the menu is open
                            // the More button stands in for them.
                            for (f, id, label) in [
                                (Foldable::Rules, "cabinet.rules", "Rules"),
                                (Foldable::Arrange, "cabinet.plate", "Plate setup"),
                                (Foldable::Arrange, "cabinet.sizing", "Size enclosure"),
                                (Foldable::Rails, "cabinet.rails", "Rails and ducts"),
                                (Foldable::CutList, "cabinet.cutlist", "Cut list"),
                                (Foldable::Heat, "cabinet.heat", "Heat"),
                                (Foldable::Export, "cabinet.plate_drawing", "Plate drawing"),
                            ] {
                                if fold.has(f) && !anchors.iter().any(|a| a.id == id) {
                                    anchors.push(cabinet_bar::Anchor {
                                        id,
                                        label,
                                        rect: more.response.rect,
                                        clicked: false,
                                    });
                                }
                            }
                        });
                    } else {
                        cabinet_bar::group(ui, caption("Output"), |ui| {
                            let export =
                                ui.menu_button(format!("{}  Export", icons::EXPORT), |ui| {
                                    let item = ui
                                        .button("Plate drawing (PDF)")
                                        .on_hover_text("Plate drawing with drill table");
                                    cabinet_bar::note(
                                        &mut anchors,
                                        "cabinet.plate_drawing",
                                        "Plate drawing",
                                        &item,
                                    );
                                    if item.clicked() {
                                        ask = Some(Ask::ExportPlate);
                                        ui.close();
                                    }
                                    if ui
                                        .button("Plate drawing (DXF)")
                                        .on_hover_text(
                                            "Plate drawing for a CNC or CAD, with the drill table",
                                        )
                                        .clicked()
                                    {
                                        ask = Some(Ask::ExportPlateDxf);
                                        ui.close();
                                    }
                                });
                            // The menu button stands in until the menu is open.
                            let at = cabinet_bar::Anchor {
                                id: "cabinet.plate_drawing",
                                label: "Plate drawing",
                                rect: export.response.rect,
                                clicked: false,
                            };
                            if !anchors.iter().any(|a| a.id == at.id) {
                                anchors.push(at);
                            }
                        });
                    }
                });
            });
        if layout.is_some() && plate_face && self.cabinet.heat.on && !has_plan {
            // The Heat strip is a second row of its own while Heat is on: the
            // bar above and the counts below keep their place, and a long
            // strip wraps instead of running off the pane.
            ui.horizontal_wrapped(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Heat").strong());
                super::heat::strip(ui, &mut self.cabinet.heat);
            });
        }
        if has_plan {
            let (n, left) = self
                .cabinet
                .preview
                .as_ref()
                .map_or((0, 0), |p| (p.placements.len(), p.unplaced.len()));
            let mut text = format!(
                "Preview: {n} {} would be placed. Nothing is stored until you accept.",
                if n == 1 { "device" } else { "devices" }
            );
            if left > 0 {
                text.push_str(&format!(" {left} do not fit."));
            }
            let mut accept = None;
            novo_theme::widgets::banner(ui, novo_theme::widgets::Level::Info, &text, |ui| {
                let r = cabinet_bar::primary(ui, icons::CHECK, "Accept", Some("Enter"), true);
                if cabinet_bar::note(&mut anchors, "cabinet.accept", "Accept", &r).clicked() {
                    accept = Some(Ask::Accept);
                }
                if ui.button(format!("{}  Cancel   Esc", icons::X)).clicked() {
                    accept = Some(Ask::Cancel);
                }
                if let Some(plate) = plate_offer
                    && ui
                        .button(format!("Apply plate {} x {} mm", plate.width, plate.height))
                        .on_hover_text(
                            "Cancels this preview and sets a plate that holds every device",
                        )
                        .clicked()
                    && let Some(s) = structure
                {
                    accept = Some(Ask::SetPlateSize(s, plate));
                }
            });
            ask = accept.or(ask);
        } else if let Some(plate) = plate_offer {
            let text = format!(
                "These devices need a larger plate than {} x {} mm.",
                current_plate.map_or(0.0, |c| c.width),
                current_plate.map_or(0.0, |c| c.height)
            );
            novo_theme::widgets::banner(ui, novo_theme::widgets::Level::Warning, &text, |ui| {
                if ui
                    .button(format!("Apply plate {} x {} mm", plate.width, plate.height))
                    .on_hover_text("Sets the plate of this panel to that size (one undo step)")
                    .clicked()
                    && let Some(s) = structure
                {
                    ask = Some(Ask::SetPlateSize(s, plate));
                }
            });
        }
        if layout.is_some() {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(8.0);
                cabinet_bar::pill(ui, &format!("{placed} placed"), None);
                if unplaced > 0 {
                    cabinet_bar::pill(
                        ui,
                        &format!("{unplaced} unplaced"),
                        Some(novo_theme::widgets::Level::Warning),
                    );
                } else if homeless > 0 {
                    cabinet_bar::pill(
                        ui,
                        &format!("{homeless} without a cabinet"),
                        Some(novo_theme::widgets::Level::Warning),
                    );
                } else if cands.is_empty() {
                    ui.weak("No devices on this panel");
                } else {
                    ui.weak("Every device of this panel is placed");
                }
                let label = format!(
                    "{problems} {}",
                    if problems == 1 { "problem" } else { "problems" }
                );
                if problems == 0 {
                    let pill = cabinet_bar::pill(ui, &label, None);
                    cabinet_bar::note(&mut anchors, "cabinet.problems", "Problems", &pill);
                } else if let Some(shown) = &shown {
                    // The count opens the list; a row selects its device. It is
                    // a pill like the counts beside it, in the error colour.
                    let pill =
                        cabinet_bar::pill(ui, &label, Some(novo_theme::widgets::Level::Error))
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                    egui::Popup::menu(&pill).show(|ui| {
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
                        for line in rail_duct_lines(shown) {
                            ui.label(line);
                        }
                    });
                    cabinet_bar::note(&mut anchors, "cabinet.problems", "Problems", &pill);
                }
                if let Some(j) = judged.as_ref().filter(|j| !j.violations.is_empty()) {
                    let n = j.violations.len();
                    let pill = cabinet_bar::pill(
                        ui,
                        &format!("{n} rule {}", if n == 1 { "problem" } else { "problems" }),
                        Some(novo_theme::widgets::Level::Warning),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                    // A click opens the list, like the problems pill.
                    egui::Popup::menu(&pill).show(|ui| {
                        for v in j.violations.iter().take(12) {
                            ui.label(v.describe(&self.cabinet.rules.rules, &|id| tag_of(id)));
                        }
                    });
                }
            });
        }
        // The picked surface and what is wrong with surface names, in full
        // and wrapped to the width (not squeezed into the status strip).
        if let Some(sum) = &surfaces {
            let note = self.cabinet.surface.as_deref().and_then(|n| sum.note(n));
            if note.is_some() || !sum.problems.is_empty() {
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    if let Some(note) = note {
                        ui.add(egui::Label::new(note).wrap());
                        ui.add(
                            egui::Label::new(egui::RichText::new(
                                "Drag, turn or take off its devices here. Put a device on this surface with Inspector > On surface.",
                            ))
                            .wrap(),
                        );
                    }
                    for problem in &sum.problems {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(problem)
                                    .color(novo_theme::Palette::of(ui).warning),
                            )
                            .wrap(),
                        );
                    }
                });
            }
        }
        ui.separator();

        match shown {
            Some(shown) if self.cabinet.table => {
                // Keys, drags and the landing box belong to the plate: none
                // may wait there while the table is shown.
                self.cabinet.keys.clear();
                self.cabinet.drag = None;
                self.cabinet.ghost = None;
                // Escape leaves the table for the plate.
                let leave = !self.wb.field.is_open()
                    && !self.modal_open()
                    && !ui.ctx().egui_wants_keyboard_input()
                    && ui.input(|i| i.key_pressed(egui::Key::Escape));
                if leave {
                    ask = Some(Ask::ToggleTable);
                }
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
                                // One banner says what "~" means, not a word on every row.
                                let size = if c.size.guessed {
                                    format!("~{size}")
                                } else {
                                    size
                                };
                                (c.id, tag_of(c.id), size)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let unplaced_panel = egui::Panel::left("cabinet_unplaced")
                    .exact_size(200.0)
                    .resizable(false)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.strong("Unplaced");
                        ui.weak("Drag onto the plate");
                        if unplaced_list
                            .iter()
                            .any(|(_, _, size)| size.starts_with('~'))
                        {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(
                                    "~ Size guessed from the device type until a part is assigned.",
                                )
                                .size(12.0)
                                .color(novo_theme::Palette::of(ui).text_tertiary),
                            );
                        }
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
                cabinet_bar::note_rect(
                    ui.ctx(),
                    "cabinet.unplaced",
                    "Unplaced",
                    unplaced_panel.response.rect,
                    false,
                );

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
                let on_surface = self
                    .cabinet
                    .surface
                    .as_deref()
                    .is_some_and(|n| n != MAIN_PLATE);
                let inspector = match (&layout, structure) {
                    // A surface other than the plate with nothing selected:
                    // its size card.
                    (Some(_), Some(s)) if selected.is_empty() && on_surface => self
                        .cabinet
                        .surface
                        .as_deref()
                        .and_then(|n| {
                            let sum = surfaces.as_ref()?;
                            super::cabinet_surface_size::card(project, s, sum, n)
                        })
                        .map(|card| (cabinet_inspector::Facts::Surface(card), false)),
                    (Some(stored), Some(s)) if !selected.is_empty() || self.cabinet.plate_card => {
                        let mut facts = cabinet_inspector::gather(
                            project, s, stored, &shown, &selected, &tag_of,
                        );
                        if let Some(j) = &judged {
                            cabinet_rules::annotate(&mut facts, &self.cabinet.rules, j, &tag_of);
                        }
                        super::cabinet_surface_move::annotate(&mut facts, surfaces.as_ref());
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
                    let items = super::heat::items_of(project, &shown, &tag_of);
                    super::heat::Input {
                        plate: (shown.plate.width, shown.plate.height),
                        ambient: self.sizing.request().ambient_max,
                        items,
                        solution: self.cabinet.heat.solution(),
                        kept: self.cabinet.heat.kept(),
                    }
                });
                if let (Some(s), true) = (structure, heat_input.is_some()) {
                    let ambient = self.sizing.request().ambient_max;
                    let dragging = self.cabinet.drag.is_some();
                    self.cabinet.heat.sync_live(
                        project,
                        s,
                        ambient,
                        dragging,
                        ui.input(|i| i.time),
                        ui.ctx(),
                    );
                }
                if self.cabinet.heat.take_request() {
                    match (structure, &heat_input) {
                        (Some(s), Some(input)) => {
                            super::heat::start_run(&mut self.cabinet.heat, project, s, input)
                        }
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
                if self.cabinet.heat.take_fit()
                    && let Some(input) = &heat_input
                {
                    let extent = self.cabinet.heat.extent_mm(input.plate.0);
                    let bounds = novo_canvas::Aabb::new(
                        vec2(0.0, 0.0),
                        vec2(extent as f32, input.plate.1 as f32),
                    );
                    self.cabinet.view.viewport.fit(&bounds, 24.0);
                }
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
                cabinet_bar::note_rect(
                    ui.ctx(),
                    "cabinet.canvas",
                    "Plate",
                    out.response.rect,
                    false,
                );
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
                if let Some(mode) = self.wb.commissioning.as_ref() {
                    let marks = super::commissioning_cabinet::marks(&mode.store, project, &shown);
                    if !marks.is_empty() {
                        let rect = out.response.rect;
                        let hovered = super::commissioning_cabinet::draw(
                            &ui.painter_at(rect),
                            rect,
                            &self.cabinet.view.viewport,
                            &marks,
                            novo_theme::Palette::of(ui).warning,
                            ui.ctx()
                                .pointer_hover_pos()
                                .filter(|_| out.response.hovered()),
                        );
                        if let Some(text) = hovered {
                            out.response.clone().on_hover_text(text);
                        }
                    }
                }
                if let Some(s) = structure {
                    let rect = out.response.rect;
                    let zoom = f64::from(self.cabinet.view.viewport.zoom.max(0.01));
                    super::cabinet_rails::draw_grips(
                        &ui.painter_at(rect),
                        rect,
                        &self.cabinet.view.viewport,
                        project,
                        s,
                        (self.cabinet.rails.hover, self.cabinet.rails.selected),
                        ((14.0 / zoom).max(8.0), novo_theme::Palette::of(ui)),
                    );
                    if self.cabinet.rails.hover.is_some() {
                        out.response
                            .clone()
                            .on_hover_text("Drag to move. Click to select, Delete removes.");
                    }
                }
                if let Some(rail) = self.cabinet.rails.selected {
                    let rect = out.response.rect;
                    let moved = self
                        .cabinet
                        .rails
                        .drag
                        .filter(|d| d.id == rail)
                        .map(|d| d.at);
                    super::cabinet_rails::draw_selected(
                        &ui.painter_at(rect),
                        rect,
                        &self.cabinet.view.viewport,
                        project,
                        rail,
                        moved,
                        novo_theme::Palette::of(ui),
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
                        cabinet_inspector::Room {
                            left: *left,
                            ..Default::default()
                        },
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
                self.cabinet.plate_rect = Some(out.response.rect);
                // Right-click: select what is under the pointer and open the
                // plate menu for it.
                if let Some(click) = out.context_click.as_ref().filter(|_| layout.is_some()) {
                    use super::cabinet_menu::{Face, PlateMenu, Target};
                    // The raw pointer, not the snapped point, which may sit on a
                    // grid line outside the box it was meant for.
                    let raw = ui.ctx().pointer_latest_pos().unwrap_or(egui::Pos2::ZERO);
                    let rect = out.response.rect;
                    let w = self
                        .cabinet
                        .view
                        .viewport
                        .screen_to_world(vec2(raw.x - rect.min.x, raw.y - rect.min.y));
                    let (wx, wy) = (f64::from(w.x), f64::from(w.y));
                    let _ = click;
                    let zoom = f64::from(self.cabinet.view.viewport.zoom.max(0.01));
                    let device = shown.placements.iter().rev().find(|p| {
                        let f = p.footprint();
                        wx >= f.x && wx <= f.right() && wy >= f.y && wy <= f.bottom()
                    });
                    let target = if let Some(p) = device {
                        self.cabinet.view.selection.apply(
                            [novo_canvas::cabinet::box_item(p.id)],
                            novo_canvas::selection::SelectOp::Replace,
                        );
                        self.cabinet.rails.selected = None;
                        Target::Device(p.id)
                    } else if let Some(rail) = structure
                        .and_then(|s| super::cabinet_rails::pick(project, s, wx, wy, 3.0 / zoom))
                    {
                        self.cabinet.view.selection.clear();
                        self.cabinet.rails.selected = Some(rail);
                        Target::Rail(rail)
                    } else {
                        Target::Plate
                    };
                    let anchor = ui
                        .ctx()
                        .pointer_latest_pos()
                        .unwrap_or_else(|| out.response.rect.center());
                    self.cabinet.menu = Some(PlateMenu::new(Face::Plate, target, anchor));
                }
                if let Some(command) = super::cabinet_menu::show(ui.ctx(), &mut self.cabinet.menu) {
                    self.wb.queued.push(command);
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
                } else if let Some(layout) = &layout {
                    let on_plate = shown;
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
                        let zoom = f64::from(self.cabinet.view.viewport.zoom.max(0.01));
                        let on_grip = structure.is_some_and(|s| {
                            super::cabinet_rails::pick_grip(
                                project,
                                s,
                                wx,
                                wy,
                                3.0 / zoom,
                                (14.0 / zoom).max(8.0),
                            )
                            .is_some()
                        });
                        if !on_grip
                            && let Some(p) = on_plate.placements.iter().rev().find(|p| {
                                let f = p.footprint();
                                wx >= f.x && wx <= f.right() && wy >= f.y && wy <= f.bottom()
                            })
                        {
                            let f = p.footprint();
                            self.cabinet.drag = Some(Drag {
                                id: p.id,
                                grab: vec2(world.x - f.x as f32, world.y - f.y as f32),
                                at: (f.x, f.y),
                            });
                        }
                    }
                    // A rail or duct is picked by a click on it, away from the
                    // devices, and dragged to move it.
                    if let Some(s) = structure {
                        let rect = out.response.rect;
                        let to_world = |p: egui::Pos2| {
                            let w = self
                                .cabinet
                                .view
                                .viewport
                                .screen_to_world(vec2(p.x - rect.min.x, p.y - rect.min.y));
                            (f64::from(w.x), f64::from(w.y))
                        };
                        let on_device = |(wx, wy): (f64, f64)| {
                            layout.placements.iter().any(|p| {
                                let f = p.footprint();
                                wx >= f.x && wx <= f.right() && wy >= f.y && wy <= f.bottom()
                            })
                        };
                        let zoom = f64::from(self.cabinet.view.viewport.zoom.max(0.01));
                        let slack = 3.0 / zoom;
                        let grip = (14.0 / zoom).max(8.0);
                        // A grip wins over the devices on its rail; the rail
                        // itself only where no device is.
                        let pick_at = |w: (f64, f64)| {
                            super::cabinet_rails::pick_grip(project, s, w.0, w.1, slack, grip)
                                .or_else(|| {
                                    if on_device(w) {
                                        None
                                    } else {
                                        super::cabinet_rails::pick(project, s, w.0, w.1, slack)
                                    }
                                })
                        };
                        self.cabinet.rails.hover = if self.cabinet.drag.is_some() {
                            None
                        } else {
                            out.response.hover_pos().and_then(|p| pick_at(to_world(p)))
                        };
                        if self.cabinet.rails.drag.is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        } else if self.cabinet.rails.hover.is_some() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                        if out.response.clicked_by(egui::PointerButton::Primary)
                            && let Some(p) = out.response.interact_pointer_pos()
                        {
                            self.cabinet.rails.selected = pick_at(to_world(p));
                        }
                        if self.cabinet.drag.is_none()
                            && out.response.drag_started_by(egui::PointerButton::Primary)
                            && let Some(press) = press
                        {
                            let w = to_world(press);
                            if let Some(id) = pick_at(w)
                                && let Some(r) =
                                    project.get::<novo_core::Rail>(novo_core::RailId::from(id))
                            {
                                self.cabinet.rails.selected = Some(id);
                                self.cabinet.rails.drag = Some(super::cabinet_rails::RailDrag {
                                    id,
                                    grab: (w.0 - r.x, w.1 - r.y),
                                    at: (r.x, r.y),
                                });
                            }
                        }
                        if let (Some(d), Some(at)) =
                            (self.cabinet.rails.drag.as_mut(), out.cursor_raw)
                        {
                            d.at = layout
                                .snapped(f64::from(at.x) - d.grab.0, f64::from(at.y) - d.grab.1);
                        }
                        if out.response.drag_stopped()
                            && let Some(d) = self.cabinet.rails.drag.take()
                        {
                            move_rail = Some(d);
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
                    // Arrow keys nudge the selected rail or duct by a grid
                    // step (1 mm with Shift) when no device is selected and
                    // no text field has the keys.
                    if self.cabinet.rails.drag.is_none()
                        && self.cabinet_selected().is_empty()
                        && !ui.ctx().egui_wants_keyboard_input()
                        && let Some(id) = self.cabinet.rails.deletable(project)
                        && let Some(r) = project.get::<novo_core::Rail>(novo_core::RailId::from(id))
                    {
                        // Plain Up and Down are the schematic's strip keys, so
                        // the app hands them over as commands.
                        let from_keys = |name: &str| {
                            f64::from(self.cabinet.keys.iter().filter(|k| *k == name).count() as i32)
                        };
                        let (up, down) = (
                            from_keys("terminal.strip_up"),
                            from_keys("terminal.strip_down"),
                        );
                        let (dx, dy, fine) = ui.input(|i| {
                            let key = |k| i.key_pressed(k);
                            (
                                f64::from(
                                    i32::from(key(egui::Key::ArrowRight))
                                        - i32::from(key(egui::Key::ArrowLeft)),
                                ),
                                f64::from(
                                    i32::from(key(egui::Key::ArrowDown))
                                        - i32::from(key(egui::Key::ArrowUp)),
                                ) + down
                                    - up,
                                i.modifiers.shift
                                    || i.events.iter().any(|e| {
                                        matches!(e, egui::Event::Key { pressed: true, modifiers, .. } if modifiers.shift)
                                    }),
                            )
                        });
                        if dx != 0.0 || dy != 0.0 {
                            let step = if fine || layout.grid <= 0.0 {
                                1.0
                            } else {
                                layout.grid
                            };
                            move_rail = Some(super::cabinet_rails::RailDrag {
                                id,
                                grab: (0.0, 0.0),
                                at: (r.x + dx * step, r.y + dy * step),
                            });
                        }
                    }
                    // Rotate and delete act on the selected devices, and
                    // Delete on the selected rail when no device is.
                    let selected = self.cabinet_selected();
                    for key in std::mem::take(&mut self.cabinet.keys) {
                        match key.as_str() {
                            "edit.rotate" if !selected.is_empty() => {
                                ask = Some(Ask::Rotate(selected.clone()));
                            }
                            "edit.delete" if !selected.is_empty() => {
                                ask = Some(Ask::Remove(selected.clone()));
                            }
                            "cabinet.plate" => {
                                ask = Some(Ask::TogglePlateCard);
                            }
                            "cabinet.place-all" if (unplaced > 0 || homeless > 0) && !has_plan => {
                                ask = Some(Ask::PlaceAll);
                            }
                            "cabinet.place-all" if !has_plan => {
                                all_placed_note = true;
                            }
                            "edit.delete" => {
                                delete_rail = self.cabinet.rails.deletable(project);
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
                // The empty state names the next step and puts its button
                // right under the sentence, not at the far end of the bar.
                ui.add_space(48.0);
                ui.vertical_centered(|ui| {
                    ui.label(match (&location, structure) {
                        (None, _) => "No panel yet. Give a page a panel name (+K1) to lay it out.",
                        (Some(_), None) => {
                            "This panel has no cabinet yet. Create one to set its plate and place devices."
                        }
                        _ => "Set the size of the mounting plate to start placing devices.",
                    });
                    ui.add_space(8.0);
                    match (&location, structure) {
                        (Some(l), None) => {
                            if cabinet_bar::primary(ui, icons::CUBE, "Create cabinet", None, true)
                                .clicked()
                            {
                                ask = Some(Ask::CreateCabinet(l.clone()));
                            }
                        }
                        (Some(_), Some(s)) => {
                            if cabinet_bar::primary(
                                ui,
                                icons::RULER,
                                "Set plate 600 x 800 mm",
                                None,
                                true,
                            )
                            .clicked()
                            {
                                ask = Some(Ask::SetPlate(s));
                            }
                            ui.add_space(4.0);
                            ui.weak("You can change the size later under Plate setup.");
                        }
                        _ => {}
                    }
                });
            }
        }

        let sizing_chip = self.learn.chip("sizing");
        let rails_chip = self.learn.chip("rails");
        let sizing_lesson = super::sizing::show_window(
            ui.ctx(),
            &mut self.sizing,
            &sizing_devices,
            current_plate,
            plate_offer,
            sizing_chip.as_ref(),
        );
        if let (Some(plate), Some(s)) = (self.sizing.apply.take(), structure) {
            ask = Some(Ask::SetPlateSize(s, plate));
        }
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

        let mut rail_ask = super::cabinet_rails::show_window(
            ui.ctx(),
            &mut self.cabinet.rails,
            project,
            structure,
            rails_chip.as_ref(),
        );

        if let Some(id) = delete_rail {
            rail_ask = Some(super::cabinet_rails::RailAsk::Delete(id));
        }
        if let Some(d) = move_rail {
            rail_ask = Some(super::cabinet_rails::RailAsk::MoveTo(d.id, d.at.0, d.at.1));
        }
        if all_placed_note {
            self.message(Level::Info, "Every device of this panel is placed");
        }
        anchors.extend(cabinet_bar::take_noted(ui.ctx()));
        for a in anchors {
            self.anchor(a.id, a.rect, a.label);
            if a.clicked {
                self.learn_event(novo_learn::Event::Clicked(a.id.to_owned()));
            }
        }
        for text in notices {
            self.message(Level::Warning, text);
        }
        if let Some(ask) = ask {
            self.cabinet_ask(ask, structure, layout, &cands);
        }
        if let Some(rail_ask) = rail_ask {
            self.cabinet_rail_ask(structure, rail_ask);
        }
        if let Some(id) = sizing_lesson {
            self.learn_run_command_lesson(&id);
        }
    }

    /// Tells what the surface rules did to `Place all`: the devices that
    /// belong on another surface and are not on the plate plan, and rules
    /// that could not be used. Returns how many devices were sent to another
    /// surface.
    fn surface_rule_messages(
        &mut self,
        structure: Option<StructureId>,
        layout: &CabinetLayout,
    ) -> usize {
        self.cabinet.surface_preview.clear();
        let (Some(s), Some(project)) = (structure, self.backend.document().map(|d| d.project()))
        else {
            return 0;
        };
        let split = cabinet_rules::surface_split(project, s, layout);
        let plans = cabinet_rules::surface_plans(project, s, layout, &split);
        let tag = |id: u64| {
            project
                .get::<novo_core::Device>(novo_core::DeviceId::from(id))
                .map_or_else(|| format!("device {id}"), |d| d.tag.to_string())
        };
        let sent: usize = split.others.iter().map(|(_, specs)| specs.len()).sum();
        let mut notes: Vec<(Level, String)> = split
            .problems
            .iter()
            .map(|p| (Level::Warning, p.clone()))
            .collect();
        for (surface, specs) in &split.others {
            let tags: Vec<String> = specs.iter().map(|sp| tag(sp.id)).collect();
            let rule = novo_layout::surface_rules::RULE_SURFACE;
            let plan = plans
                .iter()
                .find(|(name, _)| name == surface)
                .map(|(_, p)| p);
            notes.push(match plan {
                Some(plan) => {
                    let left: Vec<String> = plan.unplaced.iter().map(|&id| tag(id)).collect();
                    let fits = format!(
                        "By the surface rules {} go on the {surface} [{rule}]; Enter places them there with the plate plan; pick the {surface} in the Surface picker to see them",
                        tags.join(", ")
                    );
                    if left.is_empty() {
                        (Level::Info, fits)
                    } else {
                        (
                            Level::Warning,
                            format!("{fits}. {} do not fit on the {surface}", left.join(", ")),
                        )
                    }
                }
                None => (
                    Level::Info,
                    format!(
                        "By the surface rules {} belong on the {surface} [{rule}] and are not placed on the plate",
                        tags.join(", ")
                    ),
                ),
            });
        }
        for (level, text) in notes {
            self.message(level, text);
        }
        self.cabinet.surface_preview = plans
            .into_iter()
            .filter(|(_, p)| !p.placements.is_empty())
            .collect();
        sent
    }

    /// What Place all leaves off the plate, by name with the reason: devices
    /// that do not fit, and devices no cabinet takes because their tag has
    /// no location while the project has several. `None` when all is placed.
    fn left_out_text(
        &self,
        structure: Option<StructureId>,
        plan: &novo_layout::plan::Plan,
    ) -> Option<String> {
        let project = self.backend.document()?.project();
        let tag = |id: u64| {
            project
                .get::<novo_core::Device>(novo_core::DeviceId::from(id))
                .map_or_else(|| format!("device {id}"), |d| d.tag.to_string())
        };
        // One entry per reason: a few devices are named, many are counted, so
        // a plate that is too small gives one line and not a page of them.
        let mut by_reason: Vec<(String, Vec<String>)> = Vec::new();
        for &id in &plan.unplaced {
            let why = self
                .cabinet
                .rules
                .explain
                .as_ref()
                .and_then(|e| e.unplaced.iter().find(|(u, _)| *u == id))
                .map_or("no room left on the plate", |(_, why)| why.text());
            match by_reason.iter_mut().find(|(w, _)| w == why) {
                Some((_, tags)) => tags.push(tag(id)),
                None => by_reason.push((why.to_owned(), vec![tag(id)])),
            }
        }
        const NAMED: usize = 5;
        let mut parts: Vec<String> = by_reason
            .into_iter()
            .flat_map(|(why, tags)| {
                if tags.len() <= NAMED {
                    tags.into_iter()
                        .map(|t| format!("{t} ({why})"))
                        .collect::<Vec<_>>()
                } else {
                    vec![format!(
                        "{} devices ({why}), for example {} and {} more",
                        tags.len(),
                        tags[..3].join(", "),
                        tags.len() - 3
                    )]
                }
            })
            .collect();
        if let Some(s) = structure {
            parts.extend(novo_layout::model::without_enclosure(project, s).into_iter().map(
                |id| {
                    format!(
                        "{} (its tag has no location, and the project has several cabinets: add the cabinet's, for example +CAB1)",
                        tag(id)
                    )
                },
            ));
        }
        (!parts.is_empty()).then(|| format!("Not placed: {}", parts.join("; ")))
    }

    /// The plate the Cabinet shows, with its stored layout and the devices
    /// that could go on it: what a canvas action works on. Used by lesson
    /// replays, which act without the canvas.
    #[cfg(test)]
    pub(super) fn cabinet_plate(
        &self,
    ) -> Option<(
        StructureId,
        Option<CabinetLayout>,
        Vec<novo_layout::plan::Candidate>,
    )> {
        let project = self.backend.document()?.project();
        let location = self.cabinet.location.as_ref()?;
        let structure = project
            .structures()
            .find(|s| s.enclosure && &s.tag == location)?
            .id;
        Some((
            structure,
            load(project, structure),
            candidates(project, structure),
        ))
    }

    pub(super) fn cabinet_ask(
        &mut self,
        ask: Ask,
        structure: Option<StructureId>,
        layout: Option<CabinetLayout>,
        cands: &[novo_layout::plan::Candidate],
    ) {
        // A surface other than the plate is drawn from its own layout. Moving,
        // turning and taking off its devices work as on the plate (they act
        // on the device's own placement); the plan, the rails and the
        // enclosure belong to the plate. Accepting a Place all preview stores
        // a plan made on the plate, so it goes through from the door view too.
        if self
            .cabinet
            .surface
            .as_deref()
            .is_some_and(|n| n != MAIN_PLATE)
            && (matches!(ask, Ask::Accept) && self.cabinet.preview.is_none()
                || matches!(
                    ask,
                    Ask::PlaceAll
                        | Ask::SetRailLengths(_)
                        | Ask::SetPlate(_)
                        | Ask::SetPlateSize(..)
                        | Ask::SetEnclosure(_)
                ))
        {
            self.message(
                Level::Info,
                "Place all, rails, plate size and the enclosure can only be changed on the plate. Pick the plate first.",
            );
            return;
        }
        // Editing the plate while a Place all preview is up takes the
        // preview first (one undo step), so a drag, turn or take-off never
        // acts on a layout that is not stored yet.
        if self.cabinet.preview.is_some()
            && matches!(ask, Ask::Drop(..) | Ask::Rotate(_) | Ask::Remove(_))
        {
            self.cabinet_ask(Ask::Accept, structure, None, cands);
        }
        match ask {
            Ask::Panel(l) => {
                self.cabinet.location = Some(l);
                self.cabinet.preview = None;
                self.cabinet.surface_preview.clear();
                self.cabinet.rules.explain = None;
            }
            Ask::Cancel => {
                self.cabinet.preview = None;
                self.cabinet.surface_preview.clear();
            }
            Ask::ToggleTable => self.cabinet.table ^= true,
            Ask::TogglePlateCard => self.cabinet.plate_card ^= true,
            Ask::ExportPlate => {
                self.learn_command("export.plate");
                self.export_plate_pdf();
            }
            Ask::ExportPlateDxf => {
                self.export_plate_dxf();
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
            Ask::OpenBench => self.open_bench(),
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
                    let sent_elsewhere = self.surface_rule_messages(structure, &layout);
                    let left_out = self.left_out_text(structure, &plan);
                    if plan.placements.is_empty() && !self.cabinet.surface_preview.is_empty() {
                        // Only the other surfaces have something to place.
                        if let Some(text) = left_out {
                            self.message(Level::Warning, text);
                        }
                        self.cabinet.preview = Some(plan);
                    } else if plan.placements.is_empty() {
                        match left_out {
                            Some(text) => self.message(Level::Warning, text),
                            None if !cands.is_empty()
                                && cands
                                    .iter()
                                    .all(|c| layout.placements.iter().any(|p| p.id == c.id)) =>
                            {
                                self.message(Level::Info, "Every device is already on the plate");
                            }
                            // Everything that was left went to another surface;
                            // the surface message above already said so.
                            None if sent_elsewhere > 0 => {}
                            None => self.message(Level::Info, "Nothing fits on the plate"),
                        }
                    } else {
                        if let Some(text) = left_out {
                            self.message(Level::Warning, text);
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
                let others = std::mem::take(&mut self.cabinet.surface_preview);
                if self.cabinet_edit("Place all", move |tx| {
                    apply_plan(tx, structure, &plan)?;
                    for (surface, plan) in &others {
                        apply_plan(tx, structure, plan)?;
                        for p in &plan.placements {
                            super::cabinet_surface_move::set(tx, structure, p.id, surface)?;
                        }
                    }
                    Ok(())
                }) {
                    self.learn_signal("cabinet_plan_accepted");
                }
            }
            Ask::Drop(device, x, y) => {
                let Some(structure) = structure else {
                    return;
                };
                let rotated = layout
                    .as_ref()
                    .and_then(|l| l.placements.iter().find(|p| p.id == device))
                    .is_some_and(|p| p.rotated);
                if self.cabinet_edit("Place device", move |tx| {
                    place_device(tx, structure, device, x, y, rotated)
                }) {
                    self.learn_signal("cabinet_dropped");
                }
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
                if self.cabinet_edit("Rotate device", move |tx| {
                    devices.iter().try_for_each(|d| rotate_device(tx, *d))
                }) {
                    self.learn_signal("cabinet_rotated");
                }
            }
            Ask::Remove(devices) => {
                if self.cabinet_edit("Remove from plate", move |tx| {
                    devices.iter().try_for_each(|d| unplace_device(tx, *d))
                }) {
                    self.learn_signal("cabinet_removed");
                }
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
            Ask::SetSurface(device, surface) => {
                let Some(structure) = structure else {
                    return;
                };
                self.cabinet_edit("Set surface", move |tx| {
                    super::cabinet_surface_move::set(tx, structure, device, &surface)
                });
            }
            Ask::SetSurfaceSize(s, name, size) => {
                self.cabinet_edit("Set surface size", move |tx| {
                    super::cabinet_surface_size::set(tx, s, &name, size)
                });
            }
            Ask::SetPlateSize(s, plate) => {
                // A preview was made for the old plate.
                self.cabinet.preview = None;
                self.cabinet.surface_preview.clear();
                self.cabinet_edit("Set plate size", move |tx| set_plate(tx, s, plate));
            }
            Ask::SetPlate(s) => {
                self.cabinet_edit("Set plate", move |tx| set_plate(tx, s, DEFAULT_PLATE));
            }
            Ask::CreateCabinet(tag) => {
                // An imported project already has the node, as a plain
                // location: it becomes the cabinet instead of a second node.
                let existing = self
                    .backend
                    .document()
                    .and_then(|d| d.project().structure_by_tag(&tag).map(|s| s.id));
                self.cabinet_edit("Create cabinet", move |tx| match existing {
                    Some(id) => tx.set_enclosure(id, true),
                    None => tx
                        .execute(Command::AddStructure {
                            tag,
                            description: String::new(),
                            enclosure: true,
                        })
                        .map(|_| ()),
                });
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
    /// True when the edit was made.
    pub(super) fn cabinet_edit(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<()>,
    ) -> bool {
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        match doc.transact(label, edit) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("{label} failed: {e}"));
                false
            }
        }
    }
}
