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
/// Roomier than the schematic inspector: it holds the plate and door forms.
const WIDTH: f32 = 320.0;
/// Tall enough for the plate form and the door form without scrolling much.
const HEIGHT: f32 = 620.0;

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
    /// The place rule that applies to the device, as written in the rule file.
    pub rule: Option<String>,
    /// Why the last `Place all` put the device where it is.
    pub why: Option<String>,
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
                rule: None,
                why: None,
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

/// What the inspector's "Inside and door" section needs: its form state, the
/// layout as drawn and the tag names.
pub(super) struct DoorView<'a, F> {
    pub state: &'a mut DoorState,
    pub layout: &'a CabinetLayout,
    pub tag_of: &'a F,
}

/// Where the inspector starts when other panels share the canvas edge: how far
/// below the top (None: under the top bar) and how much of the bottom stays
/// free.
#[derive(Clone, Copy, Default)]
pub(super) struct Room {
    /// The side away from the selection: the left edge.
    pub left: bool,
    pub top: Option<f32>,
    pub bottom: f32,
}

/// Draws the inspector; returns what the user asked for.
/// The inspector as an overlay on the canvas (DESIGN.md section 3), on the
/// side away from the selection so it never hides it.
pub(super) fn overlay(
    ctx: &egui::Context,
    canvas: egui::Rect,
    room: Room,
    facts: &Facts,
    project: &Project,
    fields: &mut super::cabinet_panel::PanelFields,
    door: DoorView<'_, impl Fn(u64) -> String>,
) -> Option<Ask> {
    let side = if room.left {
        super::panel::Side::Left
    } else {
        super::panel::Side::Right
    };
    let mut panel = super::panel::Panel::new("cabinet_inspector", "Inspector", canvas, side, WIDTH)
        .fixed_open()
        .scroll_bar_always()
        .height(HEIGHT)
        .bottom_room(room.bottom);
    if let Some(top) = room.top {
        panel = panel.top(top);
    }
    let shown = panel.show(ctx, |ui| show(ui, facts, project, fields, door));
    super::cabinet_bar::note_rect(
        ctx,
        "cabinet.inspector",
        "Inspector",
        shown
            .as_ref()
            .map_or(egui::Rect::NOTHING, |r| r.response.rect),
        false,
    );
    shown.and_then(|r| r.inner).flatten()
}

fn show(
    ui: &mut egui::Ui,
    facts: &Facts,
    project: &Project,
    fields: &mut super::cabinet_panel::PanelFields,
    door: DoorView<'_, impl Fn(u64) -> String>,
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
                if let Some(rule) = &d.rule {
                    widgets::field(ui, "Place rule", rule);
                }
                if let Some(why) = &d.why {
                    ui.add_space(2.0);
                    ui.weak(why);
                }
                // One device only: the text buffers are shared, and bulk edits
                // belong to the property sheet.
                if devices.len() == 1
                    && let Some(edit) = super::cabinet_panel::show(ui, fields, project, d.id)
                {
                    ask = Some(Ask::SetPanelValue(edit));
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
                    if let Some(a) = door.state.show(ui, door.layout, door.tag_of) {
                        ask = Some(a);
                    }
                });
        }
    }
    ask
}
