//! Cross-probing: the same device in the schematic, on the Cabinet plate and
//! in the 3D view. "Show in schematic" opens the drawing on the device's main
//! symbol; "Show in Cabinet" and "Show in 3D" select it there (the Cabinet
//! and 3D selections are one) and frame it.
//!
//! Where the user finds it: right-click a device in the schematic, on the
//! Cabinet plate or in the 3D view; View > Show in schematic / Cabinet / 3D;
//! Ctrl+K "show in".

use novo_core::{Device, DeviceId};

use super::view3d::Pending;
use super::workspace::Workspace;
use super::{Level, NovoApp};

impl NovoApp {
    /// The devices the user has selected in whichever view is showing.
    pub(super) fn probe_devices(&self) -> Vec<DeviceId> {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        match self.workspace() {
            Workspace::Cabinet | Workspace::View3d => {
                let mut devices = Vec::new();
                for item in self.cabinet.view.selection.ids() {
                    if let Some(raw) = novo_canvas::cabinet::box_of(*item) {
                        let id = DeviceId::from(raw);
                        if project.get::<Device>(id).is_some() && !devices.contains(&id) {
                            devices.push(id);
                        }
                    }
                }
                devices
            }
            _ => super::part_picker::selected_devices(project, &self.selection()),
        }
    }

    /// `nav.show_schematic`: the main symbol of the (first) selected device.
    pub(super) fn show_in_schematic(&mut self) {
        let devices = self.probe_devices();
        let Some(&first) = devices.first() else {
            self.message(Level::Info, "Select a device first");
            return;
        };
        let Some(place) = self.device_main(first) else {
            self.message(Level::Info, "That device is not drawn on any page yet");
            return;
        };
        self.switch_workspace(Workspace::Schematic);
        self.jump(place, true);
        if devices.len() > 1 {
            self.message(
                Level::Info,
                format!("Showing the first of {} selected devices", devices.len()),
            );
        }
    }

    /// The tags of the devices that sit in no cabinet layout yet: selected
    /// on the plate or in 3D they would show nothing.
    fn unplaced_tags(&self, devices: &[DeviceId]) -> Vec<String> {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        let layouts: Vec<_> = project
            .structures()
            .filter(|s| s.enclosure)
            .filter_map(|s| novo_layout::model::load(project, s.id))
            .collect();
        devices
            .iter()
            .filter(|d| {
                !layouts
                    .iter()
                    .any(|l| l.placements.iter().any(|p| p.id == d.raw()))
            })
            .filter_map(|d| project.get::<Device>(*d))
            .map(|d| d.tag.to_string())
            .collect()
    }

    /// `nav.show_cabinet` and `nav.show_3d`: selects the devices on the plate
    /// and switches there; the 3D view also zooms to them.
    pub(super) fn show_on_plate(&mut self, to: Workspace) {
        let devices = self.probe_devices();
        if devices.is_empty() {
            self.message(Level::Info, "Select a device first");
            return;
        }
        let items: Vec<_> = devices
            .iter()
            .map(|d| novo_canvas::cabinet::box_item(d.raw()))
            .collect();
        let unplaced = self.unplaced_tags(&devices);
        self.cabinet.table = false;
        self.cabinet.view.selection.apply(
            items.iter().copied(),
            novo_canvas::selection::SelectOp::Replace,
        );
        self.switch_workspace(to);
        if self.workspace() != to {
            return;
        }
        if to == Workspace::View3d {
            self.view3d.pending = Some(Pending::Fit);
        } else {
            self.cabinet.view.reveal(items);
        }
        if !unplaced.is_empty() {
            self.message(
                Level::Info,
                format!(
                    "Not placed in a cabinet yet, so nothing to show: {}",
                    unplaced.join(", ")
                ),
            );
        }
    }
}
