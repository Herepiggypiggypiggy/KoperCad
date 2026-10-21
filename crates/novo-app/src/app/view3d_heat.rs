//! The Heat panel of the 3D view: Simulate, its progress and the choices
//! behind a run (quality, turbulence model, vents), with a plain statement of
//! what the result is and is not. The slice, shell and airflow controls of a
//! finished result stay in `view3d::heat_controls`.

use eframe::egui;
use novo_theme::tokens as t;

use super::heat::{self, HeatState};
use super::heat_settings::{LEVELS, MODELS, VentSpec, Wall};

/// Margin from the view's corner, in points.
#[cfg(test)]
const MARGIN: f32 = 12.0;
const WIDTH: f32 = 330.0;
/// The view cube's size and margin (`novo-view3d::widget`); the panel starts
/// below it so the cube stays usable.
pub(super) const CUBE_SIZE: f32 = 88.0;
const CUBE_MARGIN: f32 = 12.0;
const GAP: f32 = 8.0;

/// Top right corner of the panel: under the cube, inside the view.
#[cfg(test)]
fn anchor(view: egui::Rect) -> egui::Pos2 {
    view.right_top() + egui::vec2(-MARGIN, CUBE_MARGIN + CUBE_SIZE + GAP)
}

/// The cube's rectangle, as `novo-view3d` draws it.
#[cfg(test)]
fn cube_rect(view: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            view.max.x - CUBE_MARGIN - CUBE_SIZE,
            view.min.y + CUBE_MARGIN,
        ),
        egui::Vec2::splat(CUBE_SIZE),
    )
}

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
        (false, true, false) => (
            "Result for this layout. Quality, turbulence and vents apply to the next run.",
            false,
        ),
    }
}

/// The panel, top right of the 3D view. `stale` is true when the shown result
/// was made for another layout.
pub(super) fn show(
    ctx: &egui::Context,
    view: egui::Rect,
    state: &mut HeatState,
    stale: bool,
    tag_of: &dyn Fn(u64) -> String,
) -> Option<u64> {
    let mut jump = None;
    super::panel::Panel::new(
        "view3d_heat_panel",
        "Heat",
        view,
        super::panel::Side::Right,
        WIDTH,
    )
    // Starts under the view cube; the person can drag it anywhere in the view.
    .top(CUBE_MARGIN + CUBE_SIZE + GAP)
    .height(300.0)
    .show(ctx, |ui| {
        let (text, warn) = phase_line(state.running(), state.solution().is_some(), stale);
        ui.label(egui::RichText::new(text).small().color(if warn {
            t::WARNING
        } else {
            t::TEXT_MUTED
        }));
        if let Some(sol) = state.solution() {
            jump = verdict_line(ui, &sol.snapshot, tag_of, stale || state.running());
        }
        ui.horizontal_wrapped(|ui| heat::simulate_controls_with(ui, state, false));
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
    jump
}

/// The verdict sentence of the shown result, its device name a button that
/// selects the device in the view. Greyed while the result is out of date or
/// a new run is under way. Returns the device picked.
fn verdict_line(
    ui: &mut egui::Ui,
    snap: &novo_cfd::CfdSnapshot,
    tag_of: &dyn Fn(u64) -> String,
    dim: bool,
) -> Option<u64> {
    use super::heat_verdict::{Tone, verdict};
    let v = verdict(snap, tag_of)?;
    let pal = crate::theme::ui_palette();
    let colour = if dim {
        pal.text_secondary
    } else {
        match v.tone {
            Tone::Fine => pal.success,
            Tone::Tight => pal.warning,
            Tone::TooHot => pal.error,
            Tone::Unknown => pal.text_secondary,
        }
    };
    let mut jump = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(egui::RichText::new(&v.lead).strong().color(colour));
        if let Some((id, tag)) = &v.device {
            let resp = ui.add(
                egui::Button::new(egui::RichText::new(tag).strong().color(colour)).frame(false),
            );
            if resp.hovered() {
                // Underline on hover marks the name as a button.
                let y = resp.rect.bottom() - 2.0;
                ui.painter().hline(
                    resp.rect.x_range(),
                    y,
                    egui::Stroke::new(1.0, colour),
                );
            }
            if resp
                .on_hover_text("Select the device and bring it into view")
                .clicked()
            {
                jump = Some(*id);
            }
        }
        ui.label(egui::RichText::new(&v.tail).strong().color(colour));
    })
    .response
    .on_hover_text(
        "From the solver's per-device margins. A model estimate, not a measurement: limits come from the parts data.",
    );
    ui.label(
        egui::RichText::new(if dim {
            "Out of date or running: this is the previous result."
        } else {
            "Model estimate, not a measurement."
        })
        .small()
        .color(pal.text_secondary),
    );
    if !dim
        && super::assistant::claude_is_on(ui.ctx())
        && ui
            .small_button("Describe with Claude")
            .on_hover_text(
                "Sends the verdict and hot-spot lines shown here to Claude (your own API \
                 key, whatever data scope you chose) and shows a short paragraph for a \
                 report in Ask Claude. It is Claude's reading of a model estimate.",
            )
            .clicked()
    {
        super::assistant::ask_claude_later(ui.ctx(), describe_question(snap, tag_of, &v));
    }
    jump
}

/// The question sent for "Describe with Claude": only the text the panel shows.
fn describe_question(
    snap: &novo_cfd::CfdSnapshot,
    tag_of: &dyn Fn(u64) -> String,
    v: &super::heat_verdict::Verdict,
) -> String {
    let mut q = String::from(
        "Write a short paragraph (3 to 5 sentences, plain words) for the thermal section of a \
         design report, describing this enclosure heat result. Say that it is a model \
         estimate, not a measurement. Use only the numbers below.\n\nVerdict: ",
    );
    q.push_str(&v.lead);
    if let Some((_, tag)) = &v.device {
        q.push_str(tag);
    }
    q.push_str(&v.tail);
    q.push_str("\nHottest devices:\n");
    for r in super::heat_spots::rows(snap, tag_of, 8) {
        q.push_str(&format!("- {}\n", r.text()));
    }
    q
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
            // The run's own notes: heat balance, a fan case, a preview.
            for n in &sol.snapshot.notes {
                ui.label(egui::RichText::new(format!("• {n}")).small());
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
    fn the_panel_clears_the_view_cube_at_small_sizes() {
        for (w, h) in [(1366.0, 768.0), (900.0, 600.0)] {
            let view = egui::Rect::from_min_size(egui::pos2(200.0, 40.0), egui::vec2(w, h));
            let panel = egui::Rect::from_min_max(
                egui::pos2(anchor(view).x - WIDTH, anchor(view).y),
                anchor(view) + egui::vec2(0.0, 100.0),
            );
            assert!(!panel.intersects(cube_rect(view)), "{w} x {h}");
            assert!(panel.min.y > cube_rect(view).max.y);
        }
    }

    #[test]
    fn a_running_simulation_wins_over_an_old_result() {
        assert!(phase_line(true, true, true).0.starts_with("Simulating"));
    }
}
