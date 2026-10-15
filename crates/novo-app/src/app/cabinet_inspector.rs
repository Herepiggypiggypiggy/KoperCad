//! The Cabinet inspector: a side panel with the facts of what is selected on
//! the plate (the device, its part, its place and what is wrong with it), or
//! the panel's plate when nothing is. Edits go back to the Cabinet as asks,
//! so they stay one undo step each.

use eframe::egui;
use novo_core::{Device, DeviceId, MountingPlate, Part, Project, StructureId};
use novo_layout::state::CabinetLayout;

use novo_theme::widgets;

use super::cabinet::{Ask, problem_lines};
use super::cabinet_door::DoorState;
/// The inspector's width, the same as the schematic's.
const WIDTH: f32 = super::inspector::INSPECTOR_WIDTH;
const MARGIN: f32 = 12.0;
const TOP: f32 = 40.0;

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
    tag_of: &impl Fn(u64) -> String,
) -> Facts {
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
            let problems = problem_lines(shown, p.id, tag_of);
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

/// Draws the inspector; returns what the user asked for.
/// The inspector as an overlay on the canvas (DESIGN.md section 3), on the
/// side away from the selection so it never hides it.
pub(super) fn overlay(
    ctx: &egui::Context,
    canvas: egui::Rect,
    left: bool,
    facts: &Facts,
    door: &mut DoorState,
    shown: &CabinetLayout,
    tag_of: &impl Fn(u64) -> String,
) -> Option<Ask> {
    let (pivot, at) = if left {
        (
            egui::Align2::LEFT_TOP,
            canvas.left_top() + egui::vec2(MARGIN, TOP),
        )
    } else {
        (
            egui::Align2::RIGHT_TOP,
            canvas.right_top() + egui::vec2(-MARGIN, TOP),
        )
    };
    let max_height = (canvas.height() - TOP - 72.0).max(120.0);
    egui::Area::new(egui::Id::new("cabinet_inspector"))
        .order(egui::Order::Middle)
        .pivot(pivot)
        .fixed_pos(at)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    egui::ScrollArea::vertical()
                        .max_height(max_height)
                        .show(ui, |ui| show(ui, facts, door, shown, tag_of))
                        .inner
                })
                .inner
        })
        .inner
}

fn show(
    ui: &mut egui::Ui,
    facts: &Facts,
    door: &mut DoorState,
    shown: &CabinetLayout,
    tag_of: &impl Fn(u64) -> String,
) -> Option<Ask> {
    let mut ask = None;
    match facts {
        Facts::Devices(devices) => {
            for d in devices {
                ui.strong(&d.tag);
                if !d.description.is_empty() {
                    ui.weak(&d.description);
                }
                ui.add_space(4.0);
                widgets::field(ui, "Part", d.part.as_deref().unwrap_or("none assigned"));
                widgets::field(
                    ui,
                    "Size",
                    &if d.guessed {
                        format!("{} (guessed)", d.size)
                    } else {
                        d.size.clone()
                    },
                );
                widgets::field(ui, "Position", &d.position);
                if d.rotated {
                    widgets::field(ui, "Rotated", "90 degrees");
                }
                if let Some(rail) = &d.rail {
                    widgets::field(ui, "Mounted on", rail);
                }
                if !d.problems.is_empty() {
                    ui.add_space(4.0);
                    widgets::section_label(ui, "Problems");
                    for p in &d.problems {
                        ui.label(format!("\u{2022} {p}"));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Rotate").clicked() {
                        ask = Some(Ask::Rotate(vec![d.id]));
                    }
                    if ui.button("Remove from plate").clicked() {
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
            widgets::section_label(ui, "Mounting plate (mm)");
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
                                .range(if label == "Margin" { 0.0 } else { 100.0 }..=max),
                        )
                        .changed();
                });
            }
            if changed {
                ask = Some(Ask::SetPlateSize(*structure, p));
            }
            ui.add_space(6.0);
            widgets::field(ui, "Placed", &placed.to_string());
            widgets::field(ui, "Plate used", &format!("{used_percent:.0} %"));
            ui.add_space(6.0);
            // Inside size, door and depth checks (open by default: this is
            // where they are edited).
            egui::CollapsingHeader::new("Inside and door")
                .default_open(true)
                .show(ui, |ui| {
                    if let Some(a) = door.show(ui, shown, tag_of) {
                        ask = Some(a);
                    }
                });
        }
    }
    ask
}
