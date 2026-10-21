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
use super::view3d_align;
use super::view3d_controls::{self, Controls};
use super::view3d_heat;
use super::view3d_hints::Hints;
use super::view3d_measure::{self, Measure};
use super::view3d_move::{self, MoveState};
use super::view3d_nudge::{self, Fields, Step};
use super::view3d_place::{self, PlaceList};
use super::view3d_rail;
use super::view3d_stats::{Build, Stats};
use super::view3d_views::{self, ViewsAction};
use super::view3d_wires::{self, WireFacts};
use super::{heat_carry, heat_cloud};

/// The one view of the app.
const VIEW_ID: u64 = 1;

/// The 3D workspace's state: the camera and the scene it shows.
pub(crate) struct View3dMode {
    pub(super) view: View3d,
    /// Dragging, turning and deleting devices, rails and ducts in the view.
    pub(super) mv: MoveState,
    /// The Measure tool and the points it holds.
    pub(super) measure: Measure,
    measure_shown: Vec<[i64; 3]>,
    /// The Position box.
    pub(super) position: Fields,
    /// The list of unplaced devices and the one picked from it.
    pub(super) place: PlaceList,
    /// The landing box drawn last frame, to redraw the overlays when it changes.
    ghost_shown: Vec<([i64; 4], bool)>,
    /// The surface panes drawn, and what they were built for (enclosure,
    /// project revision, picked surface, shown).
    surfaces_shown: Option<(StructureId, u64, Option<String>, bool)>,
    pub(super) surface_panes: Vec<novo_view3d::Overlay>,
    /// The terminal markers drawn, and what they were built for (enclosure,
    /// project revision, pose revision, shown).
    pins_shown: Option<(StructureId, u64, u64, bool)>,
    pin_marks: Vec<novo_view3d::Overlay>,
    /// A device on a wall, roof, floor or extra plate being dragged.
    pub(super) surface_drag: Option<super::view3d_surface_devices::SurfaceDrag>,
    library: Arc<MeshLibrary>,
    /// Maker models of the devices, and the arrival count the scene was built with.
    pub(super) models: super::view3d_models::Models,
    built_models: u64,
    /// The enclosure and project revision the scene was built for.
    pub(super) built: Option<(StructureId, u64, Arc<Scene>)>,
    /// A standard view or a fit asked for by a command, run on the next frame
    /// (when the scene is at hand).
    pub(super) pending: Option<Pending>,
    /// `view.3d_save_view` asked for: saved after the view has been shown, so
    /// the camera is the fitted one a person saw.
    pub(super) save_view: bool,
    /// A message for the next frame (the save's result).
    pub(super) notice: Option<&'static str>,
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
    /// The heat-flow layer: lines the heat follows and conduction arrows.
    heat_carry: super::heat_carry::Carry,
    /// Scene with devices coloured by temperature, for the key (scene
    /// revision, result id).
    heat_scene: Option<((u64, u64), Arc<novo_view3d::Scene>)>,
    /// The layers the scene was built with.
    built_layers: Layers,
    /// The hidden items the scene was built with.
    built_hidden: BTreeSet<ItemId>,
    /// The routed wires of an enclosure at a project revision.
    wires: Option<(StructureId, u64, Vec<WireInput>)>,
    /// What each routed wire comes to, for the hover card and the list.
    pub(super) wire_facts: Option<(StructureId, u64, std::collections::BTreeMap<u64, WireFacts>)>,
    /// The Wire runs list is open.
    pub(super) wire_runs: bool,
    /// The toolbar's layers and door.
    pub(super) controls: Controls,
    /// The legend of mouse and keys.
    pub(super) hints: Hints,
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
            measure: Measure::default(),
            measure_shown: Vec::new(),
            position: Fields::default(),
            place: PlaceList::default(),
            ghost_shown: Vec::new(),
            surfaces_shown: None,
            surface_panes: Vec::new(),
            pins_shown: None,
            pin_marks: Vec::new(),
            surface_drag: None,
            library: Arc::new(MeshLibrary::new()),
            models: Default::default(),
            built_models: 0,
            built: None,
            pending: None,
            save_view: false,
            notice: None,
            pending_select: None,
            synced: (0, BTreeSet::new()),
            menu: None,
            heat: Heat3d::default(),
            probe: ProbeState::default(),
            heat_shown: None,
            heat_base: Vec::new(),
            heat_flow: heat_3d::Flow::default(),
            heat_carry: super::heat_carry::Carry::default(),
            heat_scene: None,
            built_layers: Layers::default(),
            built_hidden: BTreeSet::new(),
            wires: None,
            wire_facts: None,
            wire_runs: false,
            controls: Controls::default(),
            hints: Hints::default(),
            stats: Stats::default(),
        }
    }
}

/// The enclosure the 3D view shows: the one the Cabinet shows when it has a
/// plate, else the first with a plate.
pub(super) fn enclosure(
    project: &Project,
    preferred: Option<&novo_core::DeviceTag>,
) -> Option<StructureId> {
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
        if let Some(text) = self.view3d.notice.take() {
            self.message(super::Level::Info, text);
        }
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
        mode.models.collect(&mut mode.library);
        let stale = mode.built_models != mode.models.revision
            || !matches!(&mode.built, Some((s, r, _)) if *s == id && *r == revision)
            || mode.built_layers != layers
            || mode.built_hidden != mode.controls.hidden;
        if stale {
            let started = Instant::now();
            let mut routing_ms = None;
            // Routing is the slow part: it is redone only for a new revision.
            let wires = if layers.wires {
                if !matches!(&mode.wires, Some((s, r, _)) if *s == id && *r == revision) {
                    let routed = novo_routing::model::route_project(project);
                    let inputs = model::wires_with(project, id, &routed);
                    let on_plate = inputs.iter().map(|w| w.id).collect();
                    mode.wire_facts = Some((
                        id,
                        revision,
                        view3d_wires::facts(project, &routed, &on_plate),
                    ));
                    mode.wires = Some((id, revision, inputs));
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
            if let Some(mut built) = model::scene(project, id, &options) {
                mode.models.plan(ui.ctx(), project, id, &built);
                mode.models.apply(&mut built);
                mode.built_models = mode.models.revision;
                // Devices on walls, roof, floor and extra plates.
                if let (Some(sum), Some(lay)) = (
                    super::cabinet_surfaces::summarize(project, id),
                    load(project, id),
                ) {
                    super::view3d_surface_devices::push(
                        &mut built,
                        &sum,
                        &lay,
                        &model::inputs(project, id),
                    );
                }
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
                novo_view3d::device_shape::ensure(Arc::make_mut(&mut mode.library), &shown);
                mode.models.prune(&mut mode.library, &shown);
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
        if let Some(kind) = mode.pending_select.take() {
            mode.view.state.selection = novo_view3d::hide::of_kind(scene, kind);
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

        let now = ui.input(|i| i.time);
        match mode.pending.take() {
            Some(Pending::Fit) => mode.view.fit(scene, now, options.reduced_motion),
            Some(Pending::Preset(p)) => mode.view.go_to(p, scene, now, options.reduced_motion),
            None => {}
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
                mode.heat_carry.top_air,
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
        // The cloud is cut across the axis the camera looks along most.
        mode.heat.cloud_view = if mode.heat.cloud {
            heat_cloud::CloudView::from_eye(mode.view.camera.dir(), mode.heat.cloud_view)
        } else {
            heat_cloud::CloudView::default()
        };
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
            .chain(mode.surface_drag.iter().map(|d| {
                let r = d.rect;
                (
                    [r.x, r.y, r.w, r.h].map(|v| (v * 10.0).round() as i64),
                    d.clash,
                )
            }))
            .collect();
        let measure_key = mode.measure.key();
        let mut ghost_changed = mode.ghost_shown != ghost_key || mode.measure_shown != measure_key;
        mode.measure_shown = measure_key;
        mode.ghost_shown = ghost_key;
        // The surfaces as faint panes: rebuilt when the model, the picked
        // surface or the toggle changed.
        let picked = self.cabinet.surface.clone();
        let panes_on = super::view3d_surfaces::wanted(mode.controls.surfaces, picked.as_deref());
        let panes_key = (id, revision, picked.clone(), panes_on);
        if mode.surfaces_shown.as_ref() != Some(&panes_key) {
            mode.surface_panes = match super::cabinet_surfaces::summarize(project, id) {
                Some(sum) if panes_on => {
                    super::view3d_surfaces::overlays(&sum, picked.as_deref(), scene.frame)
                }
                _ => Vec::new(),
            };
            mode.surfaces_shown = Some(panes_key);
            ghost_changed = true;
        }
        let pins_key = (id, revision, scene.pose_revision, mode.controls.pins);
        if mode.pins_shown != Some(pins_key) {
            mode.pin_marks = if mode.controls.pins {
                super::view3d_pins::overlays(project, scene)
            } else {
                Vec::new()
            };
            mode.pins_shown = Some(pins_key);
            ghost_changed = true;
        }
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
        let mut ghost: Vec<_> = landings
            .iter()
            .flat_map(|l| {
                let top = stored
                    .as_ref()
                    .filter(|_| l.item.kind == ItemKind::Device)
                    .and_then(|lay| lay.depths.get(&l.item.raw).copied())
                    .unwrap_or(10.0);
                view3d_move::ghost(l, &scene.frame, top)
            })
            .chain(mode.surface_drag.iter().flat_map(|d| {
                // Only while a surface device is dragged.
                super::cabinet_surfaces::summarize(project, id)
                    .map(|sum| super::view3d_surface_devices::ghost(&sum, d, scene.frame))
                    .unwrap_or_default()
            }))
            .collect();
        ghost.extend(mode.measure.overlays());
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
        let tracing = solution.is_some()
            && (mode.heat.lines || mode.heat.carry)
            && !self.settings.reduce_motion;
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
            mode.heat_carry = match &solution {
                Some(sol) if mode.heat.carry => {
                    heat_carry::carry(&sol.snapshot, sol.ambient, place(sol))
                }
                _ => heat_carry::Carry::default(),
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
            if mode.heat.carry {
                let lines = heat_3d::stream_lines_in(&mode.heat_carry.flow, heat_carry::RGB, 5);
                if !lines.positions.is_empty() {
                    mode.heat_base.push(novo_view3d::Overlay::Lines(lines));
                }
                if !mode.heat_carry.conduction.positions.is_empty() {
                    mode.heat_base.push(novo_view3d::Overlay::Lines(
                        mode.heat_carry.conduction.clone(),
                    ));
                }
            }
            mode.heat_shown = wanted;
            heat_changed = true;
        }
        let marker = solution
            .as_ref()
            .zip(mode.probe.at)
            .map(|(sol, at)| probe_marker(sol, at));
        if !tracing && (heat_changed || ghost_changed || probe_changed) {
            let mut all = mode.heat_base.clone();
            all.extend(mode.surface_panes.iter().cloned());
            all.extend(mode.pin_marks.iter().cloned());
            all.extend(ghost.iter().cloned());
            all.extend(marker.clone());
            mode.view.set_overlays(all);
        }
        if tracing {
            let mut all = mode.heat_base.clone();
            all.extend(mode.surface_panes.iter().cloned());
            all.extend(mode.pin_marks.iter().cloned());
            all.extend(ghost.iter().cloned());
            all.extend(marker);
            let time = ui.input(|i| i.time);
            let t = heat_3d::tracers(&mode.heat_flow, time);
            if !t.positions.is_empty() {
                all.push(novo_view3d::Overlay::Lines(t));
            }
            if mode.heat.carry {
                let t = heat_3d::tracers_in(&mode.heat_carry.flow, time, heat_carry::TRACER_RGB, 6);
                if !t.positions.is_empty() {
                    all.push(novo_view3d::Overlay::Lines(t));
                }
            }
            mode.view.set_overlays(all);
            ui.ctx().request_repaint();
        }
        mode.view.drag_items =
            mode.controls.move_items && !mode.measure.on && mode.place.armed.is_none();
        mode.view.state.display.shadows = mode.controls.shadows;
        // Switching occlusion off and on asks for full quality back, the same
        // as "Restore quality" (one governor, one restore path).
        if mode.controls.ssao && !mode.view.state.display.ssao {
            mode.view.reset_quality();
        }
        mode.view.state.display.ssao = mode.controls.ssao;
        mode.view.state.display.dimensions = mode.controls.dimensions;
        // The choice lives in the settings, so it survives a restart.
        mode.controls.trackpad_pans = self.settings.trackpad_pans;
        mode.view.state.display.trackpad_pans = mode.controls.trackpad_pans;
        mode.view.state.display.missing_marks = mode.controls.missing_data;
        // The toolbar sits in the view's corner; the door goes where it says.
        let limit = model::door_limit_deg(project, id);
        let view_rect = ui.available_rect_before_wrap();
        mode.controls.ssao_available = mode.view.occlusion_available();
        mode.controls.models_text = mode.models.counts().text();
        mode.controls.reduced = mode.view.stepped_down().is_some() || mode.view.occlusion_paused();
        let was_measuring = mode.measure.on;
        let (changed, asked) = view3d_controls::toolbar(
            ui.ctx(),
            view_rect,
            &mut mode.controls,
            &mut mode.measure.on,
            limit,
            &self.settings.camera_views,
            !mode.view.state.selection.is_empty(),
            &mut mode.view.state.section,
            &|| mode.built.as_ref().map(|(_, _, scene)| scene.bounds()),
        );
        self.settings.trackpad_pans = mode.controls.trackpad_pans;
        if let Some(command) = asked {
            self.wb.queued.push(command);
        }
        match changed.views {
            Some(ViewsAction::Save) => {
                // The menu disables Save when it is full.
                view3d_views::save(&mut self.settings.camera_views, &mode.view.camera);
            }
            Some(ViewsAction::Go(i)) => {
                if let Some(b) = self.settings.camera_views.get(i) {
                    mode.view.go_to_camera(
                        view3d_views::camera_of(b),
                        now,
                        self.settings.reduce_motion,
                    );
                }
            }
            Some(ViewsAction::Delete(i)) if i < self.settings.camera_views.len() => {
                self.settings.camera_views.remove(i);
            }
            _ => {}
        }
        if was_measuring && !mode.measure.on {
            mode.measure.clear();
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
        mode.hints.show(ui.ctx(), view_rect);
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
        // A save asked for from another workspace waits for the first shown
        // (and fitted) camera.
        if mode.save_view && mode.view.is_fitted() {
            mode.save_view = false;
            mode.notice = Some(
                if view3d_views::save(&mut self.settings.camera_views, &mode.view.camera) {
                    "View saved: Views menu in the 3D toolbar"
                } else {
                    "The Views menu holds twelve views: forget one first"
                },
            );
        }
        mode.view.state.hover = out.hover;
        // Measure: a click picks a point; selection stays as it is.
        if mode.measure.on {
            if let Some(click) = &out.clicked {
                let point = click
                    .point
                    .or_else(|| out.plate_point.map(|(x, y)| scene.frame.world(x, y, 0.0)));
                if let Some(point) = point {
                    mode.measure.click(point, click.id);
                }
            }
            let reading = mode
                .measure
                .reading(&scene.frame, load(project, id).as_ref());
            if let (Some(r), Some(mid)) = (reading, mode.measure.midpoint())
                && let Some(at) = mode.view.project(out.rect, scene, mid)
            {
                let pal = crate::theme::ui_palette();
                let font = egui::FontId::proportional(13.0);
                let text = view3d_measure::mm(r.distance);
                let galley = ui.painter().layout_no_wrap(text, font, pal.sunken);
                let chip = egui::Rect::from_center_size(at, galley.size() + egui::vec2(12.0, 6.0));
                ui.painter().rect_filled(chip, 4.0, pal.warning);
                ui.painter()
                    .galley(chip.min + egui::vec2(6.0, 3.0), galley, pal.sunken);
            }
            if view3d_measure::readout(ui.ctx(), out.rect, &mode.measure, reading) {
                mode.measure.clear();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if mode.measure.has_pins() {
                    mode.measure.clear();
                } else {
                    mode.measure.on = false;
                }
            }
        }
        if let Some(hover) = out.hover.filter(|h| h.kind == ItemKind::Device)
            && !mode.mv.dragging()
        {
            let solution = self.cabinet.heat.solution();
            let row = solution.as_ref().and_then(|sol| {
                let row = sol.snapshot.devices.iter().find(|d| d.id == hover.raw)?;
                Some((row, sol.ambient))
            });
            if let Some(tip) = super::view3d_tip::tip(
                project,
                DeviceId::from(hover.raw),
                row,
                self.cabinet.heat.live.is_stale(),
            ) {
                super::view3d_tip::show(ui.ctx(), &tip);
            }
        }
        // Wire runs: a card for the wire under the pointer, and the list.
        let wire_facts = mode
            .wire_facts
            .as_ref()
            .filter(|(s, r, _)| *s == id && *r == revision)
            .map(|(_, _, f)| f);
        if let (Some(facts), Some(h), Some(at)) = (
            wire_facts,
            out.hover.filter(|h| h.kind == ItemKind::Wire),
            ui.input(|i| i.pointer.hover_pos()),
        ) && let Some(f) = facts.get(&h.raw)
            && !mode.mv.dragging()
        {
            view3d_wires::hover_card(ui.ctx(), at, f);
        }
        if mode.wire_runs
            && let Some(facts) = wire_facts
        {
            let asked = view3d_wires::list(ui.ctx(), out.rect, facts, &mode.view.state.selection);
            if let Some(w) = asked.select {
                mode.view.state.selection = BTreeSet::from([ItemId::new(ItemKind::Wire, w)]);
            }
            if asked.close {
                mode.wire_runs = false;
            }
            if asked.cut_list {
                self.wb.queued.push("report.cutlist");
            }
        }
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
            // A point belongs to the result it was read from.
            mode.probe.bind(sol.id);
            if let Some(click) = &out.clicked {
                mode.probe.at = heat_probe::locate(&sol.snapshot, place, &click.ray, plane);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                mode.probe.at = None;
            }
            // Without a clicked point the strip shows what is under the pointer.
            let (at, pointed) = match mode.probe.at {
                Some(at) => (Some(at), false),
                None => (
                    out.hover_ray
                        .and_then(|r| heat_probe::locate(&sol.snapshot, place, &r, plane)),
                    true,
                ),
            };
            let text = heat_probe::strip_text(
                at.map(|at| heat_probe::read(&sol.snapshot, sol.ambient, at)),
                pointed,
            );
            if mode.probe.set_text(text) {
                ui.ctx().request_repaint();
            }
            if out.hover_ray.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        } else if !mode.probe.text.is_empty() {
            mode.probe.text.clear();
        }
        let probing = mode.probe.on && solution.is_some();
        if let Some(click) = out
            .clicked
            .as_ref()
            .filter(|_| !placing && !probing && !mode.measure.on)
        {
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
            if !ui.input(|i| i.modifiers.shift || i.modifiers.command) {
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
            // A device on a wall, roof, floor or extra plate moves on its own
            // surface; the plate's rules do not apply to it.
            let surface_drag = out.item_drag.as_ref().and_then(|d| {
                let sum = super::cabinet_surfaces::summarize(project, id)?;
                super::view3d_surface_devices::landing(
                    &sum,
                    &stored,
                    &model::inputs(project, id),
                    d,
                )
            });
            match &surface_drag {
                Some(land) if land.done => edits.push(view3d_move::Edit::Device(
                    view3d_move::DeviceEdit::Drop(land.device, land.at.0, land.at.1),
                )),
                Some(_) => ui.ctx().request_repaint(),
                None => {}
            }
            let plate_drag = out.item_drag.filter(|_| surface_drag.is_none());
            mode.surface_drag = surface_drag.filter(|l| !l.done);
            let (_, edit) = mode.mv.step(&stored, &state.selection, plate_drag);
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
                let plate = view3d_nudge::nudge(&stored, &state.selection, dir, step);
                let surfaces = super::cabinet_surfaces::summarize(project, id).and_then(|sum| {
                    super::view3d_surface_devices::nudge(&sum, &stored, &state.selection, dir, step)
                });
                edits.extend(super::view3d_surface_devices::merge(plate, surfaces));
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
        if devices.len() >= 2
            && let Some(op) = view3d_align::strip(ui.ctx(), out.rect, devices.len())
        {
            self.wb.queued.push(
                view3d_align::OPS
                    .iter()
                    .find(|o| o.3 == op)
                    .map_or("view.3d_align_left", |o| o.0),
            );
        }
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
    top_flux: f32,
    tag_of: &dyn Fn(u64) -> String,
) -> Option<u64> {
    use novo_heatview::cfd::Axis;
    let range = heat_3d::range(&sol.snapshot, sol.ambient);
    let mut jump = None;
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
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
            slice_position(ui, h, &sol.snapshot);
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
        let top = if h.arrows {
            super::heat_arrows::top_speed(&sol.snapshot, h.axis, h.slice_at)
        } else {
            0.0
        };
        ui.checkbox(&mut h.cloud, "Cloud").on_hover_text(
            "The whole cabinet volume as see-through layers coloured by air temperature; air near ambient is invisible, hotter air is more opaque. Layers are cut across the direction you look along, so turning the view across the cabinet re-cuts them",
        );
        if h.cloud {
            ui.add(
                egui::Slider::new(&mut h.cloud_opacity, 0.1..=1.0)
                    .show_value(false),
            )
            .on_hover_text("How solid the cloud looks");
            ui.add(
                egui::DragValue::new(&mut h.cloud_layers)
                    .range(heat_cloud::LAYERS_RANGE)
                    .suffix(" layers"),
            )
            .on_hover_text(
                "Slices through the cabinet: fewer is lighter on a slow graphics card, more is smoother",
            );
        }
        ui.checkbox(&mut h.carry, "Heat flow").on_hover_text(
            "Where the heat goes: orange lines and moving dashes follow the heat the air carries away from hot parts (brighter is more heat per area); arrows inside plate, rails and devices show heat conducted through them. Derived from the solved temperature and air speed; surface radiation is not included. Air colder than ambient takes heat in, so its lines point against the air",
        );
        ui.checkbox(&mut h.arrows, "Arrows").on_hover_text(format!(
            "Arrows on the slice plane point where the air goes. Length is relative to the fastest air on this plane (now {top:.2} m/s), so two planes cannot be compared by length; air under 2 % of that is not drawn"
        ));
        if h.arrows {
            ui.label(egui::RichText::new(format!("longest {top:.2} m/s")).small());
        }
        ui.checkbox(&mut h.devices, "Devices").on_hover_text(
            "Colour each device by the air temperature in front of it, on the same scale as the slice",
        );
        ui.checkbox(&mut probe.on, "Probe").on_hover_text(
            "Click the air to read its temperature and speed there (Esc clears the point)",
        );
        jump = heat_spots::menu(ui, &sol.snapshot, tag_of);
        if probe.on {
            // Its own row, always there while the probe is on, so the strip
            // does not change height when a point is picked.
            ui.end_row();
            let text = if probe.text.is_empty() {
                "Probe: click the air to read its temperature and speed"
            } else {
                probe.text.as_str()
            };
            ui.label(egui::RichText::new(text).small());
        }
        heat_legend::strip(ui, h, range, top_speed, top_flux);
            });
        });
    jump
}

/// The slice plane's position in millimetres along its axis, with a step to
/// each side (one cell of the result grid).
fn slice_position(ui: &mut egui::Ui, h: &mut Heat3d, snap: &novo_cfd::CfdSnapshot) {
    let edges = heat_3d::slice_edges(snap, h.axis);
    let (lo, hi) = (edges[0], edges[edges.len() - 1]);
    let mut mm = lo + f64::from(h.slice_at) * (hi - lo);
    let back = ui
        .small_button("-")
        .on_hover_text("To the centre of the previous cell")
        .clicked();
    if back {
        mm = heat_3d::step_cell(edges, mm, -1);
    }
    let drag = ui
        .add(
            egui::DragValue::new(&mut mm)
                .range(lo..=hi)
                .speed(1.0)
                .suffix(" mm"),
        )
        .on_hover_text("Where the plane cuts the enclosure; drag or type");
    let forward = ui
        .small_button("+")
        .on_hover_text("To the centre of the next cell")
        .clicked();
    if forward {
        mm = heat_3d::step_cell(edges, mm, 1);
    }
    if back || forward || drag.changed() {
        h.slice_at = (((mm - lo) / (hi - lo).max(1e-9)) as f32).clamp(0.0, 1.0);
    }
}
