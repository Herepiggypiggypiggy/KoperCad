//! The 3D workspace (docs/design/3d-view.md): the cabinet drawn from the same
//! plate, rails, ducts and placements the Cabinet canvas edits. Read only:
//! the scene is built again from the project whenever it changes.

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;
use novo_core::{Device, DeviceId, Project, StructureId};
use novo_layout::model::{candidates, load};
use novo_view3d::build::{Options, model::scene};
use novo_view3d::camera::ViewPreset;
use novo_view3d::palette::View3dTheme;
use novo_view3d::scene::{ItemId, ItemKind};
use novo_view3d::widget::{View3d, ViewOptions};
use novo_view3d::{Look, MeshLibrary, Scene};

use super::cabinet_inspector;

use super::NovoApp;
use super::heat_3d::{self, Heat3d, Place};

/// The one view of the app.
const VIEW_ID: u64 = 1;

/// The 3D workspace's state: the camera and the scene it shows.
pub(crate) struct View3dMode {
    pub(super) view: View3d,
    library: Arc<MeshLibrary>,
    /// The enclosure and project revision the scene was built for.
    pub(super) built: Option<(StructureId, u64, Arc<Scene>)>,
    /// A standard view or a fit asked for by a command, run on the next frame
    /// (when the scene is at hand).
    pub(super) pending: Option<Pending>,
    /// The Cabinet selection revision and the 3D selection as last synced, to
    /// tell which side changed.
    synced: (u64, BTreeSet<ItemId>),
    /// What of the heat result the view shows, and the result and settings
    /// the overlays were built for.
    heat: Heat3d,
    heat_shown: Option<(u64, Heat3d)>,
    /// Overlays without the moving tracers, and the lines the tracers follow.
    heat_base: Vec<novo_view3d::Overlay>,
    heat_flow: heat_3d::Flow,
    /// Scene with devices coloured by temperature, for the key (scene
    /// revision, result id).
    heat_scene: Option<((u64, u64), Arc<novo_view3d::Scene>)>,
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
            library: Arc::new(MeshLibrary::new()),
            built: None,
            pending: None,
            synced: (0, BTreeSet::new()),
            heat: Heat3d::default(),
            heat_shown: None,
            heat_base: Vec::new(),
            heat_flow: heat_3d::Flow::default(),
            heat_scene: None,
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
        let mode = &mut self.view3d;
        let stale = !matches!(&mode.built, Some((s, r, _)) if *s == id && *r == revision);
        if stale && let Some(built) = scene(project, id, &Options::default()) {
            mode.built = Some((id, revision, Arc::new(built)));
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

        let solution = self.cabinet.heat.solution();
        if let Some(sol) = &solution {
            heat_controls(ui, &mut mode.heat, sol);
        }
        let wanted = solution.as_ref().map(|s| (s.id, mode.heat));
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
            if !tracing {
                mode.view.set_overlays(mode.heat_base.clone());
            }
        }
        if tracing {
            let mut all = mode.heat_base.clone();
            let t = heat_3d::tracers(&mode.heat_flow, ui.input(|i| i.time));
            if !t.positions.is_empty() {
                all.push(novo_view3d::Overlay::Lines(t));
            }
            mode.view.set_overlays(all);
            ui.ctx().request_repaint();
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
        mode.view.state.hover = out.hover;
        let state = &mut mode.view.state;
        if let Some(click) = &out.clicked {
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
        if let Some(items) = &out.marquee {
            if !ui.input(|i| i.modifiers.shift) {
                state.selection.clear();
            }
            state.selection.extend(items.iter().copied());
        }

        // The devices selected in 3D reach the Cabinet, and show their data.
        let devices: Vec<u64> = state
            .selection
            .iter()
            .filter(|i| i.kind == ItemKind::Device)
            .map(|i| i.raw)
            .collect();
        let mut ask = None;
        if let Some(stored) = load(project, id) {
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
                    out.rect,
                    false,
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
        if let Some(ask) = ask {
            let layout = load(project, id);
            let cands = candidates(project, id);
            self.cabinet_ask(ask, Some(id), layout, &cands);
        }
    }
}

/// The Heat strip above the 3D view: slice, shell and airflow of the last
/// simulation result (Cabinet > Heat > Simulate makes one).
fn heat_controls(ui: &mut egui::Ui, h: &mut Heat3d, sol: &super::heat_run::Solution) {
    use novo_heatview::cfd::Axis;
    let range = heat_3d::range(&sol.snapshot, sol.ambient);
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
        ui.label(
            egui::RichText::new(format!("{:.0} to {:.0} °C", range.lo, range.hi))
                .small()
                .color(novo_theme::tokens::TEXT_MUTED),
        );
    });
}
