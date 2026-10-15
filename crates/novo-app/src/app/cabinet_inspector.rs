//! The Cabinet inspector: a side panel with the facts of what is selected on
//! the plate (the device, its part, its place and what is wrong with it), or
//! the panel's plate when nothing is. Edits go back to the Cabinet as asks,
//! so they stay one undo step each.

use eframe::egui;
use novo_core::{Device, DeviceId, MountingPlate, Part, Project, StructureId};
use novo_layout::state::CabinetLayout;

use super::cabinet::Ask;

/// The widest a plate dimension can be set to, in mm.
const MAX_MM: f64 = 5000.0;

/// What the inspector shows for one placed device.
pub(super) struct DeviceFacts {
    pub id: u64,
    pub tag: String,
    pub description: String,
    pub part: Option<String>,
    pub size: String,
    pub guessed: bool,
    pub position: String,
    pub rotated: bool,
    pub rail: Option<String>,
    pub problems: Vec<String>,
}

/// What the inspector shows: the selected devices, or the plate.
pub(super) enum Facts {
    Devices(Vec<DeviceFacts>),
    Plate {
        structure: StructureId,
        plate: MountingPlate,
        used_percent: f64,
        placed: usize,
    },
}

/// Collects what the inspector shows for the layout as drawn (`shown`).
pub(super) fn gather(
    project: &Project,
    structure: StructureId,
    stored: &CabinetLayout,
    shown: &CabinetLayout,
    selected: &[u64],
) -> Facts {
    let report = shown.report();
    let name = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_default()
    };
    let devices: Vec<DeviceFacts> = shown
        .placements
        .iter()
        .filter(|p| selected.contains(&p.id))
        .map(|p| {
            let device = project.get::<Device>(DeviceId::from(p.id));
            let part = device.and_then(|d| d.parts.first()).and_then(|part| {
                project.get::<Part>(*part).map(|part| {
                    format!(
                        "{} {}",
                        part.manufacturer,
                        if part.type_number.is_empty() {
                            &part.order_number
                        } else {
                            &part.type_number
                        }
                    )
                })
            });
            let f = p.footprint();
            let mut problems = Vec::new();
            for &(a, b) in &report.collisions {
                if a == p.id || b == p.id {
                    problems.push(format!("Overlaps {}", name(if a == p.id { b } else { a })));
                }
            }
            if report.outside.contains(&p.id) {
                problems.push("Reaches past the usable plate area".to_owned());
            }
            for &(a, b, gap) in &report.too_close {
                if a == p.id || b == p.id {
                    problems.push(format!(
                        "{:.0} mm from {}, wants more room",
                        gap,
                        name(if a == p.id { b } else { a })
                    ));
                }
            }
            if p.size.guessed {
                problems.push("Size is a guess: the part has no dimensions".to_owned());
            }
            DeviceFacts {
                id: p.id,
                tag: name(p.id),
                description: device.map(|d| d.description.clone()).unwrap_or_default(),
                part,
                size: format!("{} x {} mm", f.w, f.h),
                guessed: p.size.guessed,
                position: format!("{:.0}, {:.0} mm", f.x, f.y),
                rotated: p.rotated,
                rail: p.rail.map(|r| format!("Rail {r}")),
                problems,
            }
        })
        .collect();
    if !devices.is_empty() {
        return Facts::Devices(devices);
    }
    let usable = stored.plate.usable().area();
    let used: f64 = stored.placements.iter().map(|p| p.footprint().area()).sum();
    Facts::Plate {
        structure,
        plate: MountingPlate {
            width: stored.plate.width,
            height: stored.plate.height,
            margin: stored.plate.margin,
        },
        used_percent: if usable > 0.0 {
            100.0 * used / usable
        } else {
            0.0
        },
        placed: stored.placements.len(),
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.weak(label);
        ui.label(value);
    });
}

/// Draws the inspector; returns what the user asked for.
pub(super) fn show(ui: &mut egui::Ui, facts: &Facts) -> Option<Ask> {
    let mut ask = None;
    ui.add_space(6.0);
    match facts {
        Facts::Devices(devices) => {
            for d in devices {
                ui.strong(&d.tag);
                if !d.description.is_empty() {
                    ui.weak(&d.description);
                }
                ui.add_space(4.0);
                row(ui, "Part", d.part.as_deref().unwrap_or("none assigned"));
                row(
                    ui,
                    "Size",
                    &if d.guessed {
                        format!("{} (guessed)", d.size)
                    } else {
                        d.size.clone()
                    },
                );
                row(ui, "Position", &d.position);
                row(ui, "Rotated", if d.rotated { "90 degrees" } else { "no" });
                if let Some(rail) = &d.rail {
                    row(ui, "Mounted on", rail);
                }
                if !d.problems.is_empty() {
                    ui.add_space(4.0);
                    ui.strong("Problems");
                    for p in &d.problems {
                        ui.label(format!("\u{2022} {p}"));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Rotate  (R)").clicked() {
                        ask = Some(Ask::Rotate(vec![d.id]));
                    }
                    if ui.button("Remove  (Del)").clicked() {
                        ask = Some(Ask::Remove(vec![d.id]));
                    }
                });
                ui.separator();
            }
        }
        Facts::Plate {
            structure,
            plate,
            used_percent,
            placed,
        } => {
            ui.strong("Mounting plate");
            ui.weak("Select a device to see its data");
            ui.add_space(6.0);
            let mut p = *plate;
            let mut changed = false;
            for (label, value, max) in [
                ("Width", &mut p.width, MAX_MM),
                ("Height", &mut p.height, MAX_MM),
                ("Margin", &mut p.margin, 200.0),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    changed |= ui
                        .add(
                            egui::DragValue::new(value)
                                .range(if label == "Margin" { 0.0 } else { 100.0 }..=max)
                                .suffix(" mm"),
                        )
                        .changed();
                });
            }
            if changed {
                ask = Some(Ask::SetPlateSize(*structure, p));
            }
            ui.add_space(6.0);
            row(ui, "Placed", &placed.to_string());
            row(ui, "Plate used", &format!("{used_percent:.0} %"));
        }
    }
    ask
}
