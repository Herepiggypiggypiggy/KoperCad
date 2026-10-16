//! The 3D workspace (docs/design/3d-view.md): the cabinet drawn from the same
//! plate, rails, ducts and placements the Cabinet canvas edits. Read only:
//! the scene is built again from the project whenever it changes.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use novo_core::{Device, DeviceId, Project, StructureId};
use novo_layout::model::{candidates, load};
use novo_view3d::build::{DOOR_HINGE, Options, WireInput, model};
use novo_view3d::camera::ViewPreset;
use novo_view3d::door::open_angle;
use novo_view3d::palette::View3dTheme;
use novo_view3d::scene::{ItemId, ItemKind};
use novo_view3d::widget::{View3d, ViewOptions};
use novo_view3d::{Layers, Look, MeshLibrary, Scene};

use super::cabinet_inspector;

use super::NovoApp;
use super::heat::{self, HeatState};
use super::heat_3d::{self, Heat3d, Place};
use super::heat_legend;
use super::heat_probe::{self, ProbeState};
use super::heat_spots;
use super::view3d_controls::{self, Controls};
use super::view3d_heat;
use super::view3d_move::{self, MoveState};
use super::view3d_nudge::{self, Fields, Step};
use super::view3d_place::{self, PlaceList};
use super::view3d_rail;
use super::view3d_stats::{Build, Stats};

/// The one view of the app.
const VIEW_ID: u64 = 1;

/// The 3D workspace's state: the camera and the scene it shows.
pub(crate) struct View3dMode {
    pub(super) view: View3d,
    /// Dragging, turning and deleting devices, rails and ducts in the view.
    pub(super) mv: MoveState,
    /// The Position box.
    pub(super) position: Fields,
    /// The list of unplaced devices and the one picked from it.
    pub(super) place: PlaceList,
    /// The landing box drawn last frame, to redraw the overlays when it changes.
    ghost_shown: Vec<([i64; 4], bool)>,
    library: Arc<MeshLibrary>,
    /// The enclosure and project revision the scene was built for.
    pub(super) built: Option<(StructureId, u64, Arc<Scene>)>,
    /// A standard view or a fit asked for by a command, run on the next frame
    /// (when the scene is at hand).
    pub(super) pending: Option<Pending>,
    /// A select-by-type asked for before the scene was built, applied right
    /// after it is.
    pub(super) pending_select: Option<ItemKind>,
    /// The Cabinet selection revision and the 3D selection as last synced, to
    /// tell which side changed.
    synced: (u64, BTreeSet<ItemId>),
    /// The right-click menu, while open.
    pub(super) menu: Option<super::cabinet_menu::PlateMenu>,
    /// What of the heat result the view shows, and the result and settings
    /// the overlays were built for.
    heat: Heat3d,
    /// The probe tool and the point it reads.
    probe: ProbeState,
    heat_shown: Option<(u64, Heat3d)>,
    /// Overlays without the moving tracers, and the lines the tracers follow.
    heat_base: Vec<novo_view3d::Overlay>,
    heat_flow: heat_3d::Flow,
    /// Scene with devices coloured by temperature, for the key (scene
    /// revision, result id).
    heat_scene: Option<((u64, u64), Arc<novo_view3d::Scene>)>,
    /// The layers the scene was built with.
    built_layers: Layers,
    /// The hidden items the scene was built with.
    built_hidden: BTreeSet<ItemId>,
    /// The routed wires of an enclosure at a project revision.
    wires: Option<(StructureId, u64, Vec<WireInput>)>,
    /// The toolbar's layers and door.
    pub(super) controls: Controls,
    /// The frame statistics panel and the cost of the last scene rebuild.
    pub(super) stats: Stats,
}

/// What a `view.*` command asks of the 3D view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Pending {
    Fit,
    Preset(ViewPreset),
}

impl Default for View3dMode {
    fn default() -> Self {
        View3dMode {
            view: View3d::new(VIEW_ID),
            mv: MoveState::default(),
            position: Fields::default(),
            place: PlaceList::default(),
            ghost_shown: Vec::new(),
            library: Arc::new(MeshLibrary::new()),
            built: None,
            pending: None,
            pending_select: None,
            synced: (0, BTreeSet::new()),
            menu: None,
            heat: Heat3d::default(),
            probe: ProbeState::default(),
            heat_shown: None,
            heat_base: Vec::new(),
            heat_flow: heat_3d::Flow::default(),
            heat_scene: None,
            built_layers: Layers::default(),
            built_hidden: BTreeSet::new(),
            wires: None,
            controls: Controls::default(),
            stats: Stats::default(),
        }
    }
}

/// The enclosure the 3D view shows: the one the Cabinet shows when it has a
/// plate, else the first with a plate.
fn enclosure(project: &Project, preferred: Option<&novo_core::DeviceTag>) -> Option<StructureId> {
    let has_plate = |s: &&novo_core::Structure| s.enclosure && load(project, s.id).is_some();
    project
        .structures()
        .filter(has_plate)
        .find(|s| Some(&s.tag) == preferred)
        .or_else(|| project.structures().find(has_plate))
        .map(|s| s.id)
}

impl NovoApp {
    /// How far the shown enclosure's door opens, in degrees; `None` without
    /// a door or a cabinet.
    pub(super) fn view3d_door_limit(&self) -> Option<f64> {
        let project = self.backend.document()?.project();
        let id = enclosure(project, self.cabinet.location.as_ref())?;
        model::door_limit_deg(project, id)
    }

    /// `view.3d_select_*`: selects every device, rail or duct in the view.
    pub(super) fn view3d_select_kind(&mut self, kind: ItemKind) {
        let mode = &mut self.view3d;
        let Some((_, _, scene)) = &mode.built else {
            // The scene is built on the next frame; select then.
            mode.pending_select = Some(kind);
            return;
        };
        mode.view.state.selection = novo_view3d::hide::of_kind(scene, kind);
        let n = mode.view.state.selection.len();
        let noun = match kind {
            ItemKind::Device => "device",
            ItemKind::Rail => "rail",
            _ => "duct",
        };
        let text = if n == 0 {
            format!("No {noun}s on the plate")
        } else {
            format!("Selected {n} {noun}{}", if n == 1 { "" } else { "s" })
        };
        self.message(super::Level::Info, text);
    }

    /// `view.3d_hide`, `view.3d_isolate` and `view.3d_show_all`: the
    /// selection goes out of the picture, or alone stays in it, or everything
    /// comes back. The project is untouched.
    pub(super) fn view3d_visibility(&mut self, id: &str) {
        let mode = &mut self.view3d;
        match id {
            "view.3d_show_all" => mode.controls.hidden.clear(),
            "view.3d_hide" => {
                let parts: Vec<_> = mode
                    .view
                    .state
                    .selection
                    .iter()
                    .copied()
                    .filter(|i| novo_view3d::hide::is_part(i.kind))
                    .collect();
                mode.controls.hidden.extend(parts);
                mode.view.state.selection.clear();
            }
            _ => {
                // Isolate from what is in the picture now, so a second
                // isolate narrows the first.
                let Some((_, _, scene)) = &mode.built else {
                    return;
                };
                let keep = mode.view.state.selection.clone();
                if keep.iter().any(|i| novo_view3d::hide::is_part(i.kind)) {
                    let out = novo_view3d::hide::all_but(scene, &keep);
                    mode.controls.hidden.extend(out);
                    mode.pending = Some(Pending::Fit);
                }
            }
        }
    }

    /// Whether some enclosure has a plate to draw.
    pub(super) fn view3d_has_cabinet(&self) -> bool {
        self.backend
            .document()
            .is_some_and(|d| enclosure(d.project(), self.cabinet.location.as_ref()).is_some())
    }

    /// The 3D view in the central area.
    pub(super) fn view3d_show(&mut self, ui: &mut egui::Ui) {
        let Some(document) = self.backend.document() else {
            return;
        };
        let project = document.project();
        let Some(id) = enclosure(project, self.cabinet.location.as_ref()) else {
            ui.centered_and_justified(|ui| {
                ui.weak("Set a mounting plate in the Cabinet to see it in 3D");
            });
            return;
        };
        let revision = project.revision();
        let ambient = self.sizing.request().ambient_max;
        let empty_hint = super::tips::empty_3d_hint(&self.settings, &self.wb.keymap, project, id);
        let mode = &mut self.view3d;
        let layers = mode.controls.layers;
        let stale = !matches!(&mode.built, Some((s, r, _)) if *s == id && *r == revision)
            || mode.built_layers != layers
            || mode.built_hidden != mode.controls.hidden;
        if stale {
            let started = Instant::now();
            let mut routing_ms = None;
            // Routing is the slow part: it is redone only for a new revision.
            let wires = if layers.wires {
                if !matches!(&mode.wires, Some((s, r, _)) if *s == id && *r == revision) {
                    mode.wires = Some((id, revision, model::wires(project, id)));
                    routing_ms = Some(started.elapsed().as_secs_f32() * 1000.0);
                }
                mode.wires.as_ref().map(|w| w.2.clone()).unwrap_or_default()
            } else {
                Vec::new()
            };
            let options = Options {
                shell: model::shell(project, id),
                wires,
            };
            if let Some(built) = model::scene(project, id, &options) {
                // Items deleted from the project are no longer hidden.
                mode.controls.hidden.retain(|i| built.get(*i).is_some());
                // The renderer redraws on a new scene revision only: `refresh`
                // bumps it when the items differ (an edit that changes no
                // selection or camera would otherwise stay unseen).
                let mut shown =
                    novo_view3d::hide::without(layers.apply(built), &mode.controls.hidden);
                if let Some((_, _, old)) = &mode.built {
                    let mut current = Scene::clone(old);
                    novo_view3d::build::refresh(&mut current, shown);
                    shown = current;
                }
                mode.built = Some((id, revision, Arc::new(shown)));
                mode.built_layers = layers;
                mode.built_hidden = mode.controls.hidden.clone();
                let total = started.elapsed().as_secs_f32() * 1000.0;
                mode.stats.build = Build {
                    scene_ms: total - routing_ms.unwrap_or(0.0),
                    routing_ms,
                };
            }
        }
        let Some((_, _, scene)) = &mode.built else {
            return;
        };
        let theme = if ui.visuals().dark_mode {
            View3dTheme::Dark
        } else {
            View3dTheme::Light
        };
        let options = ViewOptions {
            reduced_motion: self.settings.reduce_motion,
        };
        let now = ui.input(|i| i.time);
        if let Some(kind) = mode.pending_select.take() {
            mode.view.state.selection = novo_view3d::hide::of_kind(scene, kind);
        }
        match mode.pending.take() {
            Some(Pending::Fit) => mode.view.fit(scene, now, options.reduced_motion),
            Some(Pending::Preset(p)) => mode.view.go_to(p, scene, now, options.reduced_motion),
            None => {}
        }

        // The Cabinet's selection reaches the 3D view when it changed there.
        let cabinet_rev = self.cabinet.view.selection.revision();
        if cabinet_rev != mode.synced.0 {
            mode.view.state.selection = self
                .cabinet
                .view
                .selection
                .ids()
                .iter()
                .filter_map(|i| novo_canvas::cabinet::box_of(*i))
                .map(|d| ItemId::new(ItemKind::Device, d))
                .collect();
            mode.synced = (cabinet_rev, mode.view.state.selection.clone());
        }

        // A run started here advances here, not only in the Cabinet; the
        // result follows the cabinet (out of date marking, auto re-solve).
        self.cabinet.heat.poll(ui.ctx());
        {
            let heat = &mut self.cabinet.heat;
            if heat.solution().is_some() || heat.running() || heat.on {
                let moving = !mode.mv.landings().is_empty();
                let now = ui.input(|i| i.time);
                heat.sync_live(project, id, ambient, moving, now, ui.ctx());
            }
        }
        let solution = self.cabinet.heat.solution();
        if let Some(sol) = &solution {
            let tag_of = |raw: u64| {
                project
                    .get::<Device>(DeviceId::from(raw))
                    .and_then(|d| d.tag.device_code().map(str::to_owned))
                    .unwrap_or_else(|| format!("device {raw}"))
            };
            if let Some(raw) = heat_controls(
                ui,
                &mut mode.heat,
                &mut mode.probe,
                sol,
                mode.heat_flow.top,
                &tag_of,
            ) {
                // Jump to the device: select it and bring it into view.
                let item = ItemId::new(ItemKind::Device, raw);
                if scene.get(item).is_some() {
                    mode.view.state.selection = BTreeSet::from([item]);
                    mode.view
                        .fit(scene, ui.input(|i| i.time), options.reduced_motion);
                }
            }
        }
        let wanted = solution.as_ref().map(|s| (s.id, mode.heat));
        // The landing box of a dragged device, rail or duct.
        let mut landings = mode.mv.landings();
        let stored_layout = load(project, id);
        let cands = candidates(project, id);
        if let Some(lay) = &stored_layout {
            landings.extend(mode.place.landing(lay, &cands));
        }
        let ghost_key: Vec<([i64; 4], bool)> = landings
            .iter()
            .map(|l| {
                let r = l.rect;
                (
                    [r.x, r.y, r.w, r.h].map(|v| (v * 10.0).round() as i64),
                    l.clash,
                )
            })
            .collect();
        let ghost_changed = mode.ghost_shown != ghost_key;
        mode.ghost_shown = ghost_key;
        let stored = load(project, id);
        if let Some(lay) = &stored {
            let tag_of = |d: u64| {
                project
                    .get::<Device>(DeviceId::from(d))
                    .and_then(|d| d.tag.device_code().map(str::to_owned))
                    .unwrap_or_default()
            };
            if let Some(text) = view3d_move::warning(lay, &landings, &tag_of) {
                view3d_move::show_warning(ui.ctx(), &text);
            }
        }
        let ghost: Vec<_> = landings
            .iter()
            .flat_map(|l| {
                let top = stored
                    .as_ref()
                    .filter(|_| l.item.kind == ItemKind::Device)
                    .and_then(|lay| lay.depths.get(&l.item.raw).copied())
                    .unwrap_or(10.0);
                view3d_move::ghost(l, &scene.frame, top)
            })
            .collect();
        let mut heat_changed = false;
        if !mode.probe.on {
            mode.probe.at = None;
        }
        let probe_key = solution.as_ref().map(|s| (s.id, mode.probe.at));
        let probe_changed = probe_key != mode.probe.shown;
        mode.probe.shown = probe_key;
        let probe_marker = |sol: &super::heat_run::Solution, at: [f64; 3]| {
            heat_probe::marker(
                Place {
                    plate_x: sol.plate[0],
                    plate_y: sol.plate[1],
                    frame: scene.frame,
                },
                at,
            )
        };
        let tracing = solution.is_some() && mode.heat.lines && !self.settings.reduce_motion;
        if wanted != mode.heat_shown {
            let place = |sol: &super::heat_run::Solution| Place {
                plate_x: sol.plate[0],
                plate_y: sol.plate[1],
                frame: scene.frame,
            };
            mode.heat_flow = match &solution {
                Some(sol) if mode.heat.lines => heat_3d::flow(&sol.snapshot, place(sol)),
                _ => heat_3d::Flow::default(),
            };
            mode.heat_base = solution.as_ref().map_or_else(Vec::new, |sol| {
                heat_3d::overlays(
                    &sol.snapshot,
                    sol.ambient,
                    place(sol),
                    &mode.heat,
                    &mode.heat_flow,
                )
            });
            mode.heat_shown = wanted;
            heat_changed = true;
        }
        let marker = solution
            .as_ref()
            .zip(mode.probe.at)
            .map(|(sol, at)| probe_marker(sol, at));
        if !tracing && (heat_changed || ghost_changed || probe_changed) {
            let mut all = mode.heat_base.clone();
            all.extend(ghost.iter().cloned());
            all.extend(marker.clone());
            mode.view.set_overlays(all);
        }
        if tracing {
            let mut all = mode.heat_base.clone();
            all.extend(ghost.iter().cloned());
            all.extend(marker);
            let t = heat_3d::tracers(&mode.heat_flow, ui.input(|i| i.time));
            if !t.positions.is_empty() {
                all.push(novo_view3d::Overlay::Lines(t));
            }
            mode.view.set_overlays(all);
            ui.ctx().request_repaint();
        }
        mode.view.drag_items = mode.controls.move_items && mode.place.armed.is_none();
        mode.view.state.display.shadows = mode.controls.shadows;
        mode.view.state.display.dimensions = mode.controls.dimensions;
        mode.view.state.display.missing_marks = mode.controls.missing_data;
        // The toolbar sits in the view's corner; the door goes where it says.
        let limit = model::door_limit_deg(project, id);
        let view_rect = ui.available_rect_before_wrap();
        mode.controls.reduced = mode.view.stepped_down().is_some();
        let (_, asked) = view3d_controls::toolbar(
            ui.ctx(),
            view_rect,
            &mut mode.controls,
            limit,
            !mode.view.state.selection.is_empty(),
        );
        if let Some(command) = asked {
            self.wb.queued.push(command);
        }
        if std::mem::take(&mut mode.controls.restore) {
            mode.view.reset_quality();
        }
        let stale = run_heat(
            &mut self.cabinet.heat,
            project,
            id,
            stored.as_ref(),
            ambient,
        );
        if mode.controls.heat_panel {
            view3d_heat::show(ui.ctx(), view_rect, &mut self.cabinet.heat, stale);
        }
        if let Some(text) = empty_hint {
            ui.painter().text(
                view_rect.center_bottom() - egui::vec2(0.0, 40.0),
                egui::Align2::CENTER_BOTTOM,
                text,
                egui::FontId::proportional(14.0),
                ui.visuals().weak_text_color(),
            );
        }
        mode.stats.frame(ui.ctx(), view_rect, &mut mode.view);
        mode.controls.clamp_door(limit);
        let target = mode.controls.door_target(limit);
        if let Some(angle) = open_angle(scene, DOOR_HINGE, target) {
            mode.view
                .set_door(DOOR_HINGE, angle, now, options.reduced_motion);
        }
        let shown = match &solution {
            Some(sol) if mode.heat.devices => {
                let key = (scene.revision, sol.id);
                if !matches!(&mode.heat_scene, Some((k, _)) if *k == key) {
                    let place = Place {
                        plate_x: sol.plate[0],
                        plate_y: sol.plate[1],
                        frame: scene.frame,
                    };
                    let tinted =
                        heat_3d::tint_devices(scene, &sol.snapshot, sol.ambient, place, sol.id);
                    mode.heat_scene = Some((key, Arc::new(tinted)));
                }
                mode.heat_scene.as_ref().map(|(_, s)| s)
            }
            _ => None,
        };
        let out = mode.view.show(
            ui,
            shown.unwrap_or(scene),
            &mode.library,
            &Look::for_theme(theme),
            options,
        );
        super::cabinet_bar::note_rect(ui.ctx(), "workspace.view3d", "3D canvas", out.rect, false);
        mode.view.state.hover = out.hover;
        // Placing: the pointer moves the landing box, a click drops the
        // device, Escape lets go of it.
        let placing = mode.place.armed.is_some();
        let mut place_drop = None;
        if mode.place.open {
            let tag_of = |d: u64| {
                project
                    .get::<Device>(DeviceId::from(d))
                    .and_then(|d| d.tag.device_code().map(str::to_owned))
                    .unwrap_or_default()
            };
            let items: Vec<(u64, String, String)> = stored_layout
                .as_ref()
                .map(|l| {
                    l.unplaced(&cands)
                        .into_iter()
                        .map(|c| {
                            let size = format!("{} x {} mm", c.size.width, c.size.height);
                            (c.id, tag_of(c.id), size)
                        })
                        .collect()
                })
                .unwrap_or_default();
            if view3d_place::panel(ui.ctx(), out.rect, &mut mode.place, &items).close {
                mode.place.toggle();
            }
        }
        if mode.place.armed.is_some() {
            mode.place.pointer = out.plate_point;
            ui.ctx().request_repaint();
            if out.clicked.is_some()
                && let Some(l) = stored_layout
                    .as_ref()
                    .and_then(|lay| mode.place.landing(lay, &cands))
            {
                place_drop = Some((l.item.raw, l.at.0, l.at.1));
                mode.place.armed = None;
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                mode.place.armed = None;
            }
        }
        let state = &mut mode.view.state;
        if mode.probe.on
            && let Some(sol) = &solution
        {
            if let Some(click) = &out.clicked {
                let place = Place {
                    plate_x: sol.plate[0],
                    plate_y: sol.plate[1],
                    frame: scene.frame,
                };
                let plane = mode.heat.slice.then(|| {
                    let a = mode.heat.axis;
                    (
                        a,
                        heat_3d::slice_offset(&sol.snapshot, a, mode.heat.slice_at),
                    )
                });
                mode.probe.at = heat_probe::locate(&sol.snapshot, place, &click.ray, plane);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                mode.probe.at = None;
            }
            if let Some(at) = mode.probe.at {
                let reading = heat_probe::read(&sol.snapshot, sol.ambient, at);
                egui::Area::new(egui::Id::new("heat_probe_readout"))
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::LEFT_BOTTOM)
                    .fixed_pos(out.rect.left_bottom() + egui::vec2(12.0, -12.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(reading.text());
                        });
                    });
            } else {
                egui::Area::new(egui::Id::new("heat_probe_readout"))
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::LEFT_BOTTOM)
                    .fixed_pos(out.rect.left_bottom() + egui::vec2(12.0, -12.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label("Probe: click the air to read it");
                        });
                    });
            }
            ui.ctx().request_repaint();
        }
        let probing = mode.probe.on && solution.is_some();
        if let Some(click) = out.clicked.as_ref().filter(|_| !placing && !probing) {
            let extend = click.modifiers.shift || click.modifiers.command;
            match click.id {
                Some(id) if extend => {
                    if !state.selection.remove(&id) {
                        state.selection.insert(id);
                    }
                }
                Some(id) => state.selection = BTreeSet::from([id]),
                None if !extend => state.selection.clear(),
                None => {}
            }
        }
        // Right-click: select what is under the pointer and offer the same
        // actions as the Cabinet's plate menu, plus the view's own.
        if let Some((pos, item)) = out.context {
            use super::cabinet_menu::{Face, PlateMenu, Target};
            let target = match item {
                Some(i) if i.kind == ItemKind::Device => {
                    state.selection = BTreeSet::from([i]);
                    Target::Device(i.raw)
                }
                Some(i) if matches!(i.kind, ItemKind::Rail | ItemKind::Duct) => {
                    state.selection = BTreeSet::from([i]);
                    self.cabinet.rails.selected = Some(i.raw);
                    Target::Rail(i.raw)
                }
                _ => Target::Plate,
            };
            mode.menu = Some(PlateMenu::new(Face::View3d, target, pos));
        }
        if let Some(command) = super::cabinet_menu::show(ui.ctx(), &mut mode.menu) {
            self.wb.queued.push(command);
        }
        if let Some(items) = &out.marquee {
            if !ui.input(|i| i.modifiers.shift) {
                state.selection.clear();
            }
            state.selection.extend(items.iter().copied());
        }

        // Dragging a device, rail or duct moves it by the Cabinet's rules.
        let mut edits = Vec::new();
        edits
            .extend(place_drop.map(|(d, x, y)| {
                view3d_move::Edit::Device(view3d_move::DeviceEdit::Drop(d, x, y))
            }));
        if let Some(stored) = load(project, id) {
            if let Some(d) = &out.item_drag
                && !mode.mv.dragging()
            {
                let extend = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                if !state.selection.contains(&d.id) && !extend {
                    state.selection = BTreeSet::from([d.id]);
                } else if !state.selection.contains(&d.id) {
                    state.selection.insert(d.id);
                }
            }
            let (_, edit) = mode.mv.step(&stored, &state.selection, out.item_drag);
            edits.extend(edit);
            if mode.mv.dragging() {
                ui.ctx().request_repaint();
            }
            let keys = std::mem::take(&mut self.cabinet.keys);
            // Arrow keys nudge the selection while the pointer is over the
            // view and no text field has the keys. Plain Up and Down reach
            // the app as the schematic's strip commands, so those count too.
            if !ui.ctx().egui_wants_keyboard_input()
                && ui
                    .input(|i| i.pointer.hover_pos())
                    .is_some_and(|p| out.rect.contains(p))
            {
                let strip = |name: &str| keys.iter().filter(|k| *k == name).count() as i32;
                let (dir, step) = ui.input(|i| {
                    let k = |key| i32::from(i.key_pressed(key));
                    let dx = k(egui::Key::ArrowRight) - k(egui::Key::ArrowLeft);
                    let raw = k(egui::Key::ArrowDown) - k(egui::Key::ArrowUp);
                    let cmd = strip("terminal.strip_down") - strip("terminal.strip_up");
                    let dy = if cmd != 0 { cmd } else { raw };
                    (
                        (dx.signum(), dy.signum()),
                        Step::from_modifiers(i.modifiers),
                    )
                });
                edits.extend(view3d_nudge::nudge(&stored, &state.selection, dir, step));
            }
            edits.extend(view3d_move::key_edits(&keys, &state.selection, &stored));
        } else {
            self.cabinet.keys.clear();
        }

        // The devices selected in 3D reach the Cabinet, and show their data.
        let devices: Vec<u64> = state
            .selection
            .iter()
            .filter(|i| i.kind == ItemKind::Device)
            .map(|i| i.raw)
            .collect();
        let selected_rails: Vec<u64> = state
            .selection
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Rail | ItemKind::Duct))
            .map(|i| i.raw)
            .collect();
        let mut ask = None;
        if let Some(stored) = load(project, id) {
            if let [one] = devices.as_slice()
                && !mode.position.hidden
            {
                edits.extend(mode.position.show(ui.ctx(), out.rect, &stored, *one));
            }
            if state.selection != mode.synced.1 {
                self.cabinet.view.selection.apply(
                    devices.iter().map(|d| novo_canvas::cabinet::box_item(*d)),
                    novo_canvas::selection::SelectOp::Replace,
                );
                mode.synced = (
                    self.cabinet.view.selection.revision(),
                    state.selection.clone(),
                );
            }
            if !devices.is_empty() {
                let tag_of = |d: u64| {
                    project
                        .get::<Device>(DeviceId::from(d))
                        .and_then(|d| d.tag.device_code().map(str::to_owned))
                        .unwrap_or_default()
                };
                let facts =
                    cabinet_inspector::gather(project, id, &stored, &stored, &devices, &tag_of);
                ask = cabinet_inspector::overlay(
                    ui.ctx(),
                    view_rect,
                    // Left, under the toolbar: the right edge is the view cube
                    // and the Heat panel, the bottom left the statistics.
                    cabinet_inspector::Room {
                        left: true,
                        top: Some(view3d_controls::BELOW),
                        bottom: if self.view3d.stats.shown {
                            super::view3d_stats::ROOM
                        } else {
                            0.0
                        },
                    },
                    &facts,
                    project,
                    &mut self.cabinet.panel_fields,
                    cabinet_inspector::DoorView {
                        state: &mut self.cabinet.door,
                        layout: &stored,
                        tag_of: &tag_of,
                    },
                );
            }
        }
        // One rail or duct selected: its values to type, in millimetres.
        let mut rail_ask = None;
        if devices.is_empty()
            && let [rail] = selected_rails.as_slice()
            && let Some(stored) = load(project, id)
        {
            let tag_of = |d: u64| {
                project
                    .get::<Device>(DeviceId::from(d))
                    .and_then(|d| d.tag.device_code().map(str::to_owned))
                    .unwrap_or_default()
            };
            rail_ask = view3d_rail::card(
                ui.ctx(),
                (
                    view_rect,
                    if self.view3d.stats.shown {
                        super::view3d_stats::ROOM
                    } else {
                        0.0
                    },
                ),
                &mut self.cabinet.rails,
                project,
                &stored,
                *rail,
                &tag_of,
            );
        }
        if let Some(ask) = ask {
            let layout = load(project, id);
            let cands = candidates(project, id);
            self.cabinet_ask(ask, Some(id), layout, &cands);
        }

        if let Some(rail_ask) = rail_ask {
            self.cabinet_rail_ask(Some(id), rail_ask);
            // The scene is rebuilt on the next frame; nothing else asks for it.
            ui.ctx().request_repaint();
        }
        for edit in edits {
            if let view3d_move::Edit::Group { devices, rails } = edit {
                self.view3d_move_group(id, devices, rails);
                continue;
            }
            let (device, rail) = edit.ask();
            if let Some(device) = device {
                let doc = self.backend.document().map(|d| d.project());
                let layout = doc.and_then(|p| load(p, id));
                let cands = doc.map_or_else(Vec::new, |p| candidates(p, id));
                self.cabinet_ask(device, Some(id), layout, &cands);
            }
            if let Some(rail) = rail {
                self.cabinet_rail_ask(Some(id), rail);
            }
        }
    }
}

/// Starts the run Simulate asked for (here or by command) and says whether
/// the shown result belongs to another layout than the plate now has.
fn run_heat(
    heat: &mut HeatState,
    project: &Project,
    id: StructureId,
    layout: Option<&novo_layout::state::CabinetLayout>,
    ambient: f64,
) -> bool {
    let wants_run = heat.take_request();
    if !wants_run && heat.solution().is_none() {
        return false;
    }
    let Some(layout) = layout else {
        if wants_run {
            heat.fail("Open an enclosure with a mounting plate to simulate".into());
        }
        return false;
    };
    let tag_of = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .and_then(|d| d.tag.device_code().map(str::to_owned))
            .unwrap_or_default()
    };
    let input = heat::Input {
        plate: (layout.plate.width, layout.plate.height),
        ambient,
        items: heat::items_of(project, layout, &tag_of),
        solution: None,
        kept: None,
    };
    if wants_run {
        heat::start_run(heat, project, id, &input);
    }
    // The scenario decides, so a vent added in the settings counts too.
    heat.live.is_stale()
}

/// The Heat strip above the 3D view: slice, shell and airflow of the last
/// simulation result (Cabinet > Heat > Simulate makes one).
fn heat_controls(
    ui: &mut egui::Ui,
    h: &mut Heat3d,
    probe: &mut ProbeState,
    sol: &super::heat_run::Solution,
    top_speed: f32,
    tag_of: &dyn Fn(u64) -> String,
) -> Option<u64> {
    use novo_heatview::cfd::Axis;
    let range = heat_3d::range(&sol.snapshot, sol.ambient);
    let mut jump = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Heat");
        ui.checkbox(&mut h.slice, "Slice").on_hover_text(
            "A plane through the cabinet coloured by temperature (X across, Y up, Z out of the plate)",
        );
        if h.slice {
            for (axis, name) in [(Axis::X, "X"), (Axis::Y, "Y"), (Axis::Z, "Z")] {
                ui.selectable_value(&mut h.axis, axis, name);
            }
            ui.add(egui::Slider::new(&mut h.slice_at, 0.0..=1.0).show_value(false));
        }
        ui.checkbox(&mut h.iso, "Shell")
            .on_hover_text("The surface where the air reaches one temperature");
        if h.iso {
            ui.add(egui::Slider::new(&mut h.iso_at, 0.05..=0.95).show_value(false));
            let level = range.lo + f64::from(h.iso_at) * (range.hi - range.lo);
            ui.label(egui::RichText::new(format!("{level:.0} °C")).small());
        }
        ui.checkbox(&mut h.lines, "Airflow")
            .on_hover_text("Streamlines from in front of the devices; faster air is brighter");
        ui.checkbox(&mut h.devices, "Devices").on_hover_text(
            "Colour each device by the air temperature in front of it, on the same scale as the slice",
        );
        ui.checkbox(&mut probe.on, "Probe").on_hover_text(
            "Click the air to read its temperature and speed there (Esc clears the point)",
        );
        heat_legend::strip(ui, h, range, top_speed);
        jump = heat_spots::menu(ui, &sol.snapshot, tag_of);
    });
    jump
}
