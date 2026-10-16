//! Putting a placed device on another surface of the cabinet (the door, a
//! wall, an extra plate) from the Cabinet inspector's Placement section. The
//! choice is the `surface` property of the device's placement; the plate is
//! the default and clears it. The position stays as it is, read on the new
//! surface from its own corner. Reach it: Cabinet, select a device, Inspector,
//! Placement, "On surface".

use eframe::egui;
use novo_core::{AnyId, DeviceId, StructureId, Value};
use novo_layout::surfaces::{MAIN_PLATE, SURFACE_PROPERTY};

use super::cabinet_inspector::{DeviceFacts, Facts};
use super::cabinet_surfaces::SurfaceSummary;

/// The surfaces a device can be put on, and the one it is on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SurfaceChoice {
    pub current: String,
    pub names: Vec<String>,
}

/// Adds the choice to the devices in `facts`; a cabinet with only its plate
/// has nothing to choose.
pub(super) fn annotate(facts: &mut Facts, summary: Option<&SurfaceSummary>) {
    let (Facts::Devices(devices), Some(summary)) = (facts, summary) else {
        return;
    };
    if summary.rows.len() < 2 {
        return;
    }
    let names: Vec<String> = summary.rows.iter().map(|r| r.name.clone()).collect();
    for d in devices.iter_mut() {
        let current = summary
            .assigned
            .get(&d.id)
            .cloned()
            .unwrap_or_else(|| MAIN_PLATE.to_owned());
        d.surface = Some(SurfaceChoice {
            current,
            names: names.clone(),
        });
    }
}

/// Drops the candidates already mounted on another surface: they are placed,
/// so the plate view must not list them as unplaced or plan them a spot.
pub(super) fn keep_plate_candidates(
    cands: &mut Vec<novo_layout::plan::Candidate>,
    summary: &SurfaceSummary,
) {
    cands.retain(|c| {
        summary
            .assigned
            .get(&c.id)
            .is_none_or(|name| name.as_str() == MAIN_PLATE)
    });
}

/// The "On surface" row; returns the surface picked.
pub(super) fn show(ui: &mut egui::Ui, d: &DeviceFacts) -> Option<String> {
    let choice = d.surface.as_ref()?;
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("On surface");
        egui::ComboBox::from_id_salt(("cabinet_device_surface", d.id))
            .selected_text(&choice.current)
            .show_ui(ui, |ui| {
                for name in &choice.names {
                    if ui.selectable_label(*name == choice.current, name).clicked()
                        && *name != choice.current
                    {
                        picked = Some(name.clone());
                    }
                }
            })
            .response
            .on_hover_text(
                "Mount this device on the plate, the door, a wall or an extra plate. \
                 The position is kept and read from the corner of the new surface.",
            );
    });
    picked
}

/// Writes the surface of `device`'s placement in `structure`; the plate clears
/// the property. Errors name what is missing.
pub(super) fn set(
    tx: &mut novo_core::Tx<'_>,
    structure: StructureId,
    device: u64,
    surface: &str,
) -> novo_core::Result<()> {
    let id = tx
        .project()
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.structure == structure && p.device == DeviceId::from(device))
        .map(|p| p.id)
        .ok_or_else(|| novo_core::ModelError::Format("that device is not placed".into()))?;
    let value = (surface != MAIN_PLATE).then(|| Value::Text(surface.to_owned()));
    tx.set_property(AnyId::CabinetPlacement(id), SURFACE_PROPERTY, value)
}

/// The surface `device` is on, for tests and messages.
#[cfg(test)]
pub(super) fn surface_of(
    project: &novo_core::Project,
    structure: StructureId,
    device: u64,
) -> String {
    project
        .all::<novo_core::CabinetPlacement>()
        .find(|p| p.structure == structure && p.device == DeviceId::from(device))
        .and_then(|p| match p.properties.get(SURFACE_PROPERTY) {
            Some(Value::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .unwrap_or_else(|| MAIN_PLATE.to_owned())
}
