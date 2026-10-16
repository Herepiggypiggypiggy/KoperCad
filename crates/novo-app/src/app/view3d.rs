//! The 3D workspace (docs/design/3d-view.md): the cabinet drawn from the same
//! plate, rails, ducts and placements the Cabinet canvas edits. Read only:
//! the scene is built again from the project whenever it changes.

use std::sync::Arc;

use eframe::egui;
use novo_core::{Project, StructureId};
use novo_layout::model::load;
use novo_view3d::build::{Options, model::scene};
use novo_view3d::palette::View3dTheme;
use novo_view3d::widget::{View3d, ViewOptions};
use novo_view3d::{Look, MeshLibrary, Scene};

use super::NovoApp;
use super::heat_3d::{self, Heat3d, Place};

/// The one view of the app.
const VIEW_ID: u64 = 1;

/// The 3D workspace's state: the camera and the scene it shows.
pub(crate) struct View3dMode {
    view: View3d,
    library: Arc<MeshLibrary>,
    /// The enclosure and project revision the scene was built for.
    pub(super) built: Option<(StructureId, u64, Arc<Scene>)>,
    /// What of the heat result the view shows, and the result and settings
    /// the overlays were built for.
    heat: Heat3d,
    heat_shown: Option<(u64, Heat3d)>,
}

impl Default for View3dMode {
    fn default() -> Self {
        View3dMode {
            view: View3d::new(VIEW_ID),
            library: Arc::new(MeshLibrary::new()),
            built: None,
            heat: Heat3d::default(),
            heat_shown: None,
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
        let solution = self.cabinet.heat.solution();
        if let Some(sol) = &solution {
            heat_controls(ui, &mut mode.heat, sol);
        }
        let wanted = solution.as_ref().map(|s| (s.id, mode.heat));
        if wanted != mode.heat_shown {
            let overlays = solution.as_ref().map_or_else(Vec::new, |sol| {
                let place = Place {
                    plate_x: sol.plate[0],
                    plate_y: sol.plate[1],
                    frame: scene.frame,
                };
                heat_3d::overlays(&sol.snapshot, sol.ambient, place, &mode.heat)
            });
            mode.view.set_overlays(overlays);
            mode.heat_shown = wanted;
        }
        mode.view
            .show(ui, scene, &mode.library, &Look::for_theme(theme), options);
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
        ui.label(
            egui::RichText::new(format!("{:.0} to {:.0} °C", range.lo, range.hi))
                .small()
                .color(novo_theme::tokens::TEXT_MUTED),
        );
    });
}
