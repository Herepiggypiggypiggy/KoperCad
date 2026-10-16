//! The Heat panel of the 3D view: Simulate, its progress and the choices
//! behind a run (quality, turbulence model, vents), with a plain statement of
//! what the result is and is not. The slice, shell and airflow controls of a
//! finished result stay in `view3d::heat_controls`.

use eframe::egui;
use novo_theme::tokens as t;

use super::heat::{self, HeatState};
use super::heat_settings::{LEVELS, MODELS, VentSpec, Wall};

/// Margin from the view's corner, in points.
const MARGIN: f32 = 12.0;
const WIDTH: f32 = 330.0;

/// What the panel says above its button, in words a first-time user reads.
pub(super) fn phase_line(running: bool, has_result: bool, stale: bool) -> (&'static str, bool) {
    match (running, has_result, stale) {
        (true, _, _) => (
            "Simulating: the picture improves as each level finishes.",
            false,
        ),
        (false, false, _) => (
            "No simulation yet. Simulate shows how heat and air move inside this cabinet.",
            false,
        ),
        (false, true, true) => (
            "Out of date: the layout changed after this run. Simulate again.",
            true,
        ),
        (false, true, false) => ("Result for the cabinet as it is now.", false),
    }
}

/// The panel, top right of the 3D view. `stale` is true when the shown result
/// was made for another layout.
pub(super) fn show(ctx: &egui::Context, view: egui::Rect, state: &mut HeatState, stale: bool) {
    egui::Area::new(egui::Id::new("view3d_heat_panel"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(view.right_top() + egui::vec2(-MARGIN, MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(WIDTH);
                ui.label(egui::RichText::new("Heat").strong());
                let (text, warn) = phase_line(state.running(), state.solution().is_some(), stale);
                ui.label(egui::RichText::new(text).small().color(if warn {
                    t::WARNING
                } else {
                    t::TEXT_MUTED
                }));
                ui.horizontal_wrapped(|ui| heat::simulate_controls(ui, state));
                let running = state.running();
                egui::CollapsingHeader::new("Settings for the next run")
                    .id_salt("view3d_heat_settings")
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!running, |ui| settings(ui, state));
                    });
                egui::CollapsingHeader::new("What is this?")
                    .id_salt("view3d_heat_about")
                    .show(ui, |ui| about(ui, state));
            });
        });
}

fn settings(ui: &mut egui::Ui, state: &mut HeatState) {
    let s = &mut state.settings;
    ui.label("Quality");
    for (level, name, hint) in LEVELS {
        ui.radio_value(&mut s.top, level, name).on_hover_text(hint);
    }
    ui.add_space(4.0);
    ui.label("Turbulence");
    for (model, name, hint) in MODELS {
        ui.radio_value(&mut s.turbulence, model, name)
            .on_hover_text(hint);
    }
    ui.add_space(4.0);
    ui.label("Vents");
    if s.vents.is_empty() {
        ui.label(
            egui::RichText::new("None: the enclosure is sealed.")
                .small()
                .color(t::TEXT_MUTED),
        );
    }
    let mut remove = None;
    for (i, v) in s.vents.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("vent_wall", i))
                .width(70.0)
                .selected_text(v.wall.label())
                .show_ui(ui, |ui| {
                    for w in Wall::ALL {
                        ui.selectable_value(&mut v.wall, w, w.label());
                    }
                });
            ui.add(egui::Slider::new(&mut v.at, 0.0..=1.0).show_value(false))
                .on_hover_text("Where along the wall the opening is centred");
            ui.add(
                egui::DragValue::new(&mut v.width_mm)
                    .range(10.0..=2000.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Opening size along the wall");
            ui.add(
                egui::DragValue::new(&mut v.height_mm)
                    .range(10.0..=2000.0)
                    .suffix(" mm"),
            )
            .on_hover_text("Opening size the other way (the depth for a top or bottom vent)");
            ui.add(
                egui::DragValue::new(&mut v.free_fraction)
                    .range(0.05..=1.0)
                    .speed(0.01)
                    .fixed_decimals(2),
            )
            .on_hover_text("Free share of the opening: a louvred grille is 0.5 to 0.7");
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        s.vents.remove(i);
    }
    if ui
        .button("Add vent")
        .on_hover_text("A passive grille or louvre; air enters low and leaves high")
        .clicked()
    {
        s.vents.push(VentSpec::new(Wall::Bottom));
    }
    ui.label(
        egui::RichText::new(
            "Vents are kept for this session; they are not saved in the project yet.",
        )
        .small()
        .color(t::TEXT_MUTED),
    );
}

fn about(ui: &mut egui::Ui, state: &HeatState) {
    ui.label(
        "Simulate solves the air flow and temperatures inside the cabinet from the \
         power loss of the placed devices. The quality levels run one after the other, so \
         a rough picture appears first and sharpens; Cancel keeps the latest one.",
    );
    ui.add_space(4.0);
    ui.label("Before any run, the Cabinet's Heat view shows a quick estimate, not a simulation.");
    ui.add_space(4.0);
    match state.solution() {
        Some(sol) => {
            let acc = &sol.snapshot.accuracy;
            let tag = if acc.validated {
                "Checked against benchmarks."
            } else {
                "Not validated: treat it as a model, not a measurement."
            };
            ui.label(egui::RichText::new(tag).strong());
            ui.label(&acc.headline);
            for l in &acc.limits {
                ui.label(egui::RichText::new(format!("• {l}")).small());
            }
            if !sol.messages.is_empty() {
                ui.add_space(4.0);
                ui.label("Missing or assumed:");
                for m in &sol.messages {
                    ui.label(egui::RichText::new(format!("• {m}")).small());
                }
            }
        }
        None => {
            ui.label(
                egui::RichText::new(
                    "Not validated: results are a model, not a measurement. The accuracy \
                     statement and its limits appear here after a run.",
                )
                .strong(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_state_has_its_own_plain_line() {
        let a = phase_line(false, false, false).0;
        let b = phase_line(true, false, false).0;
        let c = phase_line(false, true, false).0;
        let d = phase_line(false, true, true);
        assert!(a.starts_with("No simulation"));
        assert!(b.starts_with("Simulating"));
        assert!(c.starts_with("Result"));
        assert!(d.0.starts_with("Out of date") && d.1);
    }

    #[test]
    fn a_running_simulation_wins_over_an_old_result() {
        assert!(phase_line(true, true, true).0.starts_with("Simulating"));
    }
}
