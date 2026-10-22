//! The Vents and fans panel of the 3D view: add a grille, fan or filter fan
//! from a list of sensible places, then move and size it with millimetre
//! values while it is drawn in the cabinet. Everything is kept inside its
//! wall, so the opening drawn is the opening solved. Edits undo here.

use eframe::egui;
use novo_thermal::model::Enclosure;

use super::heat_openings::{self, Role};
use super::heat_settings::{HeatSettings, Kind, MIN_SIZE_MM, VentSpec, Wall};

const WIDTH: f32 = 316.0;

/// The panel, left side of the 3D view under the toolbar. `flows` is the air
/// measured through each opening of the shown result (empty without one).
pub(super) fn show(
    ctx: &egui::Context,
    view: egui::Rect,
    settings: &mut HeatSettings,
    enclosure: Option<&Enclosure>,
    flows: &[Option<f64>],
    running: bool,
) {
    super::panel::Panel::new(
        "view3d_vents",
        "Vents and fans",
        view,
        super::panel::Side::Left,
        WIDTH,
    )
    .top(super::view3d_controls::BELOW)
    // As tall as its content: the list, and the form of the one open opening.
    .fit()
    .show(ctx, |ui| body(ui, settings, enclosure, flows, running));
}

fn body(
    ui: &mut egui::Ui,
    settings: &mut HeatSettings,
    enclosure: Option<&Enclosure>,
    flows: &[Option<f64>],
    running: bool,
) {
    // Long notes wrap inside the panel instead of stretching it.
    ui.set_max_width(WIDTH - 24.0);
    let pal = crate::theme::ui_palette();
    let muted = |t: &str| {
        egui::RichText::new(t.to_owned())
            .small()
            .color(pal.text_secondary)
    };
    let Some(e) = enclosure else {
        ui.label(muted(
            "Set the enclosure size first (Cabinet, Size enclosure): the openings sit in its walls.",
        ));
        return;
    };
    ui.label(muted(
        "Where air gets in (blue) and out (orange). Grey: the direction is known after a run.",
    ));
    if running {
        ui.label(
            egui::RichText::new("Locked while the run goes: Cancel it to edit.")
                .small()
                .color(pal.warning),
        );
    }
    ui.add_enabled_ui(!running, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("Add opening", |ui| {
                ui.set_min_width(250.0);
                for p in VentSpec::presets(e) {
                    if ui.button(p.name).on_hover_text(p.hint).clicked() {
                        settings.add(p.spec);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("Grilles and fans in places that work: air in low, out high");
            if ui
                .add_enabled(settings.can_undo(), egui::Button::new("Undo"))
                .on_hover_text("Undo the last change to the openings")
                .clicked()
            {
                settings.undo();
            }
            if ui
                .add_enabled(settings.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                settings.redo();
            }
        });
    });
    if settings.vents.is_empty() {
        ui.add_space(4.0);
        ui.label(muted(
            "None: the enclosure is sealed. Add a grille low and another high, or a filter fan and an exhaust grille.",
        ));
    }
    let mut remove = None;
    for i in 0..settings.vents.len() {
        let spec = settings.vents[i];
        let flow = flows.get(i).copied().flatten();
        let role = heat_openings::role_of(&spec, flow);
        let open = settings.selected == Some(i);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let c = role.rgb();
            ui.label(egui::RichText::new("●").color(egui::Color32::from_rgb(c[0], c[1], c[2])));
            let title = format!(
                "{}  {}, {}",
                i + 1,
                spec.kind.label(),
                heat_openings::wall_phrase(spec.wall)
            );
            if ui
                .selectable_label(open, title)
                .on_hover_text("Open it here and mark it in the cabinet")
                .clicked()
            {
                settings.selected = if open { None } else { Some(i) };
            }
            let tag = match (role, flow) {
                (Role::In, Some(q)) => format!("air in {:.0} m³/h (model)", q.abs()),
                (Role::Out, Some(q)) => format!("air out {:.0} m³/h (model)", q.abs()),
                (Role::In, None) => "air in".into(),
                (Role::Out, None) => "air out".into(),
                (Role::Open, _) => "direction after a run".into(),
            };
            ui.label(muted(&tag));
        });
        if open {
            let mut edited = spec;
            ui.add_enabled_ui(!running, |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    editor(ui, &mut edited, i, e);
                    if ui.button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            });
            let edited = edited.fitted(e);
            if edited != spec && remove.is_none() {
                settings.checkpoint_edit();
                settings.vents[i] = edited;
            }
        }
    }
    if let Some(i) = remove {
        settings.remove(i);
    }
    if let Some(line) = balance_line(flows, settings) {
        ui.add_space(4.0);
        ui.label(muted(&line));
    }
    egui::CollapsingHeader::new("What is this?")
        .id_salt("view3d_vents_about")
        .show(ui, |ui| {
            ui.label(muted(
                "Walls are named as seen from the front of the cabinet; the one you pick is tinted. \
                 Openings are kept inside their wall and saved with the project. They apply to the \
                 next run. A grille's flow is read from the solved air speed next to it: a model \
                 value, not a measurement. Drag an opening's label in the cabinet to slide it.",
            ));
        });
}

/// One line on how much air comes in and goes out of the shown result, so a
/// gap between the two is explained instead of left to the reader.
fn balance_line(flows: &[Option<f64>], settings: &HeatSettings) -> Option<String> {
    let (mut inflow, mut outflow) = (0.0_f64, 0.0_f64);
    let mut any = false;
    for (v, q) in settings.vents.iter().zip(flows) {
        let Some(q) = q else { continue };
        any = true;
        match heat_openings::role_of(v, Some(*q)) {
            Role::In => inflow += q.abs(),
            Role::Out => outflow += q.abs(),
            Role::Open => {}
        }
    }
    if !any {
        return None;
    }
    let gap = (inflow - outflow).abs();
    let larger = inflow.max(outflow).max(1e-9);
    Some(format!(
        "Air in {inflow:.0}, out {outflow:.0} m³/h (model). The {gap:.0} m³/h ({:.0} %) gap is \
         how closely the solved speeds next to the openings add up; the flow itself balances.",
        100.0 * gap / larger
    ))
}

/// Centre of `wall` along its first axis, mm.
fn middle(wall: Wall, e: &Enclosure) -> f64 {
    match wall {
        Wall::Left | Wall::Right => 0.5 * e.interior_height_mm(),
        _ => 0.5 * e.interior_width_mm(),
    }
}

fn editor(ui: &mut egui::Ui, v: &mut VentSpec, i: usize, e: &Enclosure) {
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    egui::Grid::new(("vent_editor", i))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Type");
            egui::ComboBox::from_id_salt(("vent_kind", i))
                .selected_text(v.kind.label())
                .show_ui(ui, |ui| {
                    for k in Kind::ALL {
                        ui.selectable_value(&mut v.kind, k, k.label());
                    }
                });
            ui.end_row();
            ui.label("Where");
            let before = v.wall;
            egui::ComboBox::from_id_salt(("vent_wall", i))
                .selected_text(v.wall.label())
                .show_ui(ui, |ui| {
                    for wall in Wall::ALL {
                        ui.selectable_value(&mut v.wall, wall, wall.label());
                    }
                });
            ui.end_row();
            if v.wall != before {
                // A new wall starts centred, low for the door and side walls.
                v.u_mm = middle(v.wall, e);
                v.v_mm = 0.5 * h;
            }
            if v.kind.is_fan() {
                ui.label("Blows");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut v.exhaust, false, "in");
                    ui.radio_value(&mut v.exhaust, true, "out");
                });
                ui.end_row();
            }
            let across_label = match v.wall {
                Wall::Left | Wall::Right => "Height above floor",
                _ => "From the left",
            };
            ui.label(across_label);
            ui.horizontal(|ui| {
                match v.wall {
                    Wall::Left | Wall::Right => {
                        let mut up = v.height_above_floor_mm(e);
                        let r = ui
                            .add(
                                egui::DragValue::new(&mut up)
                                    .range(0.0..=h)
                                    .speed(2.0)
                                    .max_decimals(0)
                                    .suffix(" mm"),
                            )
                            .on_hover_text("Centre of the opening above the cabinet floor");
                        if r.changed() {
                            v.set_height_above_floor(up, e);
                        }
                    }
                    _ => {
                        ui.add(
                            egui::DragValue::new(&mut v.u_mm)
                                .range(0.0..=w)
                                .speed(2.0)
                                .max_decimals(0)
                                .suffix(" mm"),
                        )
                        .on_hover_text("Centre of the opening from the left edge of the inside");
                    }
                }
                if ui
                    .small_button("Centre")
                    .on_hover_text("Put it in the middle of the wall")
                    .clicked()
                {
                    v.u_mm = middle(v.wall, e);
                }
            });
            ui.end_row();
            if v.wall == Wall::Door {
                ui.label("Height above floor");
                let mut up = v.height_above_floor_mm(e);
                let r = ui
                    .add(
                        egui::DragValue::new(&mut up)
                            .range(0.0..=h)
                            .speed(2.0)
                            .max_decimals(0)
                            .suffix(" mm"),
                    )
                    .on_hover_text("Centre of the opening above the cabinet floor");
                if r.changed() {
                    v.set_height_above_floor(up, e);
                }
                ui.end_row();
            }
            let (along_max, across_max, across_label) = match v.wall {
                Wall::Top | Wall::Bottom => (w, d, "Width × front to back"),
                Wall::Left | Wall::Right => (h, d, "Height × front to back"),
                Wall::Door => (w, h, "Width × height"),
            };
            ui.label(across_label);
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut v.width_mm)
                        .range(MIN_SIZE_MM..=along_max)
                        .speed(2.0)
                        .max_decimals(0),
                )
                .on_hover_text("Size along the wall, mm");
                ui.label("×");
                ui.add(
                    egui::DragValue::new(&mut v.height_mm)
                        .range(MIN_SIZE_MM..=across_max)
                        .speed(2.0)
                        .max_decimals(0)
                        .suffix(" mm"),
                )
                .on_hover_text("Size the other way, mm");
            });
            ui.end_row();
            if v.kind.is_fan() {
                ui.label("Flow, pressure");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut v.flow_m3h)
                            .range(1.0..=5000.0)
                            .speed(1.0)
                            .max_decimals(0)
                            .suffix(" m³/h"),
                    )
                    .on_hover_text(
                        "Flow at free air (no back pressure), from the maker's datasheet",
                    );
                    ui.add(
                        egui::DragValue::new(&mut v.pressure_pa)
                            .range(1.0..=2000.0)
                            .speed(1.0)
                            .max_decimals(0)
                            .suffix(" Pa"),
                    )
                    .on_hover_text("Maximum pressure: where the fan delivers nothing (shut-off)");
                });
                ui.end_row();
            } else {
                ui.label("Free area");
                let mut pct = (v.free_fraction * 100.0).round();
                let r = ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(5.0..=100.0)
                            .speed(1.0)
                            .max_decimals(0)
                            .suffix(" %"),
                    )
                    .on_hover_text("Free share of the opening: a louvred grille is 50 to 70 %");
                if r.changed() {
                    v.free_fraction = pct / 100.0;
                }
                ui.end_row();
            }
        });
    // Notes go under the grid: a grid cell does not wrap and would stretch
    // the panel.
    let note = match v.kind {
        Kind::FilterFan => Some(
            "Enter the datasheet values with the filter fitted: the filter mat is not modelled \
             apart from them.",
        ),
        Kind::Fan => Some("Starts at a planning size: enter the datasheet values of the fan."),
        Kind::Grille => None,
    };
    if let Some(note) = note {
        ui.label(
            egui::RichText::new(note)
                .small()
                .color(crate::theme::ui_palette().text_secondary),
        );
    }
}
