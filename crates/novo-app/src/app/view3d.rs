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

        let out = mode
            .view
            .show(ui, scene, &mode.library, &Look::for_theme(theme), options);
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
