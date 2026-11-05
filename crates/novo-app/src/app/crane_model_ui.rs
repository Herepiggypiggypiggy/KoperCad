//! The crane model controls of the Hoist in 3D window (Simulate > Hoist in 3D): the button that
//! picks a STEP file of the person's own crane, the name of the model in use, and the "Crane model
//! parts" window where the file's axes and the role of each solid are set.

use super::NovoApp;
use super::crane_model_state::{ModelSlot, Status};
use eframe::egui;
use novo_view3d::crane_model::{Orientation, PartRole, UpAxis};

/// The model buttons of the window's top row.
pub(super) fn bar(ui: &mut egui::Ui, slot: &mut ModelSlot) {
    match slot.status() {
        Status::None => {}
        Status::Reading(name) => {
            ui.add(egui::Spinner::new());
            ui.weak(format!("Reading {name}"));
        }
        Status::Failed(name, why) => {
            ui.weak(format!("{name} not read"));
            super::window_help::add(ui.ctx(), format!("{name} could not be used: {why}"));
        }
        Status::Ready { name, .. } => {
            ui.weak(name);
        }
    }
    let has = slot.want().is_some();
    let label = if has {
        "Change model\u{2026}"
    } else {
        "Crane model\u{2026}"
    };
    if ui
        .button(label)
        .on_hover_text("Show your own crane from a STEP file")
        .clicked()
        && let Some(path) = crate::dialogs::FileDialog::new()
            .set_title("Choose your crane model (STEP)")
            .add_filter("STEP model", &["stp", "step", "p21"])
            .pick_file()
    {
        slot.pick(&path);
    }
    if has {
        if slot.shown() && ui.button("Parts").clicked() {
            slot.parts_open ^= true;
        }
        if ui
            .button("Generic crane")
            .on_hover_text("Go back to the generic crane")
            .clicked()
        {
            slot.clear();
            slot.parts_open = false;
        }
    }
}

const TURNS: [&str; 4] = ["0\u{b0}", "90\u{b0}", "180\u{b0}", "270\u{b0}"];

impl NovoApp {
    /// The "Crane model parts" window: which axis of the file is up, the turn about it, and a role
    /// for every solid of the model.
    pub(super) fn crane_model_parts_window(&mut self, ctx: &egui::Context) {
        let slot = &mut self.wb.hoist.three_d.model;
        if !slot.parts_open {
            return;
        }
        let (
            Some(want),
            Status::Ready {
                parts, triangles, ..
            },
        ) = (slot.want().cloned(), slot.status())
        else {
            slot.parts_open = false;
            return;
        };
        let rows = slot.rows();
        let mut orientation = want.orientation;
        let mut set_role: Option<(usize, PartRole)> = None;
        let mut reset = false;
        let mut open = true;
        // Floats beside the picture: a pane docked in the right strip would fold the list away.
        super::panel::dialog(ctx, "Crane model parts", [470.0, 440.0])
            .floating()
            .default_pos([ctx.content_rect().min.x + 70.0, 110.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Up");
                    egui::ComboBox::from_id_salt("crane_model_up")
                        .selected_text(match orientation.up {
                            UpAxis::Y => "Y",
                            UpAxis::Z => "Z",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut orientation.up, UpAxis::Z, "Z");
                            ui.selectable_value(&mut orientation.up, UpAxis::Y, "Y");
                        });
                    ui.label("Turn");
                    egui::ComboBox::from_id_salt("crane_model_turn")
                        .selected_text(TURNS[usize::from(orientation.turns % 4)])
                        .show_ui(ui, |ui| {
                            for (t, label) in TURNS.iter().enumerate() {
                                ui.selectable_value(&mut orientation.turns, t as u8, *label);
                            }
                        });
                    let changed = rows.iter().any(|r| r.role != r.guessed);
                    if ui
                        .add_enabled(changed, egui::Button::new("Guess from names"))
                        .clicked()
                    {
                        reset = true;
                    }
                });
                ui.weak(format!("{parts} parts \u{b7} {triangles} triangles"));
                let height = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(
                    ui,
                    height,
                    rows.len(),
                    |ui, range| {
                        egui::Grid::new("crane_model_rows")
                            .striped(true)
                            .num_columns(3)
                            .show(ui, |ui| {
                                for n in range {
                                    let r = &rows[n];
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(210.0, height - 4.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.add(egui::Label::new(&r.name).truncate());
                                        },
                                    );
                                    let mut role = r.role;
                                    egui::ComboBox::from_id_salt(("crane_model_role", n))
                                        .width(96.0)
                                        .selected_text(role.label())
                                        .show_ui(ui, |ui| {
                                            for choice in PartRole::ALL {
                                                ui.selectable_value(
                                                    &mut role,
                                                    choice,
                                                    choice.label(),
                                                );
                                            }
                                        })
                                        .response
                                        .on_hover_text(format!(
                                            "Guessed from the name: {}",
                                            r.guessed.label()
                                        ));
                                    if role != r.role {
                                        set_role = Some((n, role));
                                    }
                                    let [x, y, z] = r.size_mm;
                                    ui.weak(format!("{x:.0} \u{d7} {y:.0} \u{d7} {z:.0}"));
                                    ui.end_row();
                                }
                            });
                    },
                );
                super::window_help::add(
                    ui.ctx(),
                    "A solid's role is a guess from its name. Change it here: the colour follows the role, and in the moving picture so does the motion (wheels turn, the drum turns, the hook block and hook follow the rope). \
                     Up is the axis of your file that points up; Turn lays the runway along the picture's long side. Sizes are in millimetres along the runway, up and across the span.",
                );
            });
        let slot = &mut self.wb.hoist.three_d.model;
        if !open {
            slot.parts_open = false;
        }
        if orientation != want.orientation {
            slot.set_orientation(Orientation {
                up: orientation.up,
                turns: orientation.turns % 4,
            });
        }
        if let Some((n, role)) = set_role {
            slot.set_role(n, role);
        }
        if reset {
            slot.reset_roles();
        }
    }
}
