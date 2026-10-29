//! Winch scenarios (`sim.winches`, Simulate > Winch scenarios): closed-loop (encoder) hoist
//! drives and two synchronous winches under a script (lane L2c, engines
//! `novo_drive::closed_loop` and `novo_machine::sync_winch`).
//!
//! Pick a scenario, change what the scenario lets you change, press Run: the window draws the
//! hook, speed, torque and rope-force traces, lists what happened when, and shows the design
//! findings (a brake that closes after the torque is gone, a drum that is a little too large,
//! a drive that trips without telling its partner). The hardware is a generic one; the numbers
//! are a model and the window says which are exact and which are not.

use eframe::egui;
use novo_drive::Fidelity;
use novo_drive::closed_loop::{FIDELITY, LEAVES_OUT, PreTorque, Severity};
use novo_machine::winch_scenarios::{Params, ScenarioId, SyncChoice, Trace, run};
use novo_theme::tokens;

use super::panel;

/// Which pre-torque the window offers (the fixed value follows the load).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreChoice {
    None,
    Fixed,
    LoadCell,
    Memory,
}

impl PreChoice {
    const ALL: [PreChoice; 4] = [Self::None, Self::Fixed, Self::LoadCell, Self::Memory];

    fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Fixed => "Fixed value (the weight torque)",
            Self::LoadCell => "Load cell on the hook",
            Self::Memory => "Memory (torque held at the last stop)",
        }
    }
}

/// What the Simulate > Winch scenarios window remembers.
pub(crate) struct WinchScenarios {
    pub(super) open: bool,
    id: ScenarioId,
    params: Params,
    pre: PreChoice,
    result: Option<Result<Trace, String>>,
}

impl Default for WinchScenarios {
    fn default() -> Self {
        let mut s = Self {
            open: false,
            id: ScenarioId::Cycle,
            params: Params::default(),
            pre: PreChoice::LoadCell,
            result: None,
        };
        s.run();
        s
    }
}

impl WinchScenarios {
    fn pre_torque(&self) -> PreTorque {
        match self.pre {
            PreChoice::None => PreTorque::None,
            PreChoice::Fixed => {
                PreTorque::Fixed(self.params.load_kg * novo_drive::GRAVITY * 0.2 / 31.5)
            }
            PreChoice::LoadCell => PreTorque::LoadCell,
            PreChoice::Memory => PreTorque::Memory,
        }
    }

    /// Runs the chosen scenario with the current settings (a few milliseconds).
    fn run(&mut self) {
        self.params.pre_torque = self.pre_torque();
        self.result = Some(run(self.id, &self.params).map_err(|e| e.to_string()));
    }

    fn trace(&self) -> Option<&Trace> {
        match &self.result {
            Some(Ok(t)) => Some(t),
            _ => None,
        }
    }
}

fn colour(i: usize) -> egui::Color32 {
    let p = crate::theme::ui_palette();
    [p.info, p.accent, p.success, p.warning, p.error][i % 5]
}

fn chart(ui: &mut egui::Ui, trace: &Trace, channels: &[usize]) {
    let pal = crate::theme::ui_palette();
    ui.horizontal_wrapped(|ui| {
        for (n, &c) in channels.iter().enumerate() {
            let ch = &trace.channels[c];
            ui.colored_label(colour(n), format!("\u{25A0} {} ({})", ch.name, ch.unit));
        }
    });
    let size = egui::vec2(ui.available_width().max(320.0), 120.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, pal.panel);
    let plot = rect.shrink2(egui::vec2(52.0, 10.0));
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &c in channels {
        for v in &trace.channels[c].values {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    if !lo.is_finite() || !hi.is_finite() || hi - lo < 1e-9 {
        lo -= 0.5;
        hi += 0.5;
    }
    let pad = 0.06 * (hi - lo);
    let (lo, hi) = (lo - pad, hi + pad);
    let t_end = trace.duration_s().max(1e-9);
    let at = |t: f64, v: f64| {
        egui::pos2(
            plot.left() + (t / t_end) as f32 * plot.width(),
            plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height(),
        )
    };
    let font = egui::FontId::proportional(tokens::FONT_SMALL);
    for k in 0..=3 {
        let v = lo + (hi - lo) * f64::from(k) / 3.0;
        let y = at(0.0, v).y;
        p.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(1.0, pal.border_subtle),
        );
        p.text(
            egui::pos2(plot.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{v:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned(),
            font.clone(),
            pal.text_secondary,
        );
    }
    p.text(
        egui::pos2(plot.right(), plot.bottom() + 1.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{t_end:.0} s"),
        font.clone(),
        pal.text_tertiary,
    );
    for (n, &c) in channels.iter().enumerate() {
        let ch = &trace.channels[c];
        let points: Vec<_> = ch
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| at(i as f64 * trace.dt_s, *v))
            .collect();
        p.add(egui::Shape::line(points, egui::Stroke::new(1.5, colour(n))));
    }
    if let Some(pos) = response.hover_pos()
        && plot.contains(pos)
    {
        let t = f64::from((pos.x - plot.left()) / plot.width()) * t_end;
        let i = ((t / trace.dt_s) as usize).min(trace.channels[channels[0]].values.len() - 1);
        p.line_segment(
            [
                egui::pos2(pos.x, plot.top()),
                egui::pos2(pos.x, plot.bottom()),
            ],
            egui::Stroke::new(1.0, pal.text_tertiary),
        );
        let mut text = format!("{t:.2} s");
        for &c in channels {
            let ch = &trace.channels[c];
            text.push_str(&format!("   {} {:.3} {}", ch.name, ch.values[i], ch.unit));
        }
        p.text(
            egui::pos2(plot.left() + 4.0, plot.top() + 2.0),
            egui::Align2::LEFT_TOP,
            text,
            font,
            pal.text_strong,
        );
    }
}

fn level(s: Severity) -> novo_theme::widgets::Level {
    match s {
        Severity::Problem => novo_theme::widgets::Level::Error,
        Severity::Warning => novo_theme::widgets::Level::Warning,
        Severity::Info => novo_theme::widgets::Level::Info,
    }
}

fn controls(ui: &mut egui::Ui, w: &mut WinchScenarios) -> bool {
    let mut run_now = false;
    ui.horizontal(|ui| {
        ui.label("Scenario");
        egui::ComboBox::from_id_salt("winch_scenario")
            .selected_text(w.id.title())
            .width(360.0)
            .show_ui(ui, |ui| {
                for id in ScenarioId::ALL {
                    if ui.selectable_value(&mut w.id, id, id.title()).clicked() {
                        run_now = true;
                    }
                }
            });
    });
    ui.weak(w.id.summary());
    ui.add_space(4.0);
    let p = &mut w.params;
    let single = !w.id.is_pair();
    egui::Grid::new("winch_params")
        .num_columns(2)
        .show(ui, |ui| {
            if single {
                ui.label("Hook load");
                run_now |= ui
                    .add(
                        egui::DragValue::new(&mut p.load_kg)
                            .range(200.0..=3000.0)
                            .suffix(" kg"),
                    )
                    .changed();
                ui.end_row();
                ui.label("Torque at brake release");
                egui::ComboBox::from_id_salt("winch_pre")
                    .selected_text(w.pre.label())
                    .show_ui(ui, |ui| {
                        for c in PreChoice::ALL {
                            run_now |= ui.selectable_value(&mut w.pre, c, c.label()).changed();
                        }
                    });
                ui.end_row();
                ui.label("Torque held after the brake relay opens");
                run_now |= ui
                    .add(
                        egui::DragValue::new(&mut p.brake_hold_s)
                            .range(0.0..=2.0)
                            .speed(0.02)
                            .suffix(" s"),
                    )
                    .changed();
                ui.end_row();
                ui.label("Braking resistor");
                run_now |= ui.checkbox(&mut p.braking_resistor, "fitted").changed();
                ui.end_row();
                if w.id == ScenarioId::EncoderLoss {
                    ui.label("Encoder monitoring");
                    run_now |= ui
                        .checkbox(&mut p.encoder_monitor, "compare with the motor model")
                        .changed();
                    ui.end_row();
                    ui.label("What fails");
                    run_now |= ui
                        .checkbox(
                            &mut p.direction_channel_only,
                            "only the direction channel (counts keep coming)",
                        )
                        .changed();
                    ui.end_row();
                }
            } else {
                ui.label("Load on the beam");
                run_now |= ui
                    .add(
                        egui::DragValue::new(&mut p.beam_kg)
                            .range(1000.0..=6000.0)
                            .suffix(" kg"),
                    )
                    .changed();
                ui.end_row();
                ui.label("Beam");
                run_now |= ui
                    .checkbox(
                        &mut p.guided,
                        "guided (tilt locked, as between guide rails)",
                    )
                    .changed();
                ui.end_row();
                ui.label("Synchronisation");
                egui::ComboBox::from_id_salt("winch_sync")
                    .selected_text(p.sync.label())
                    .show_ui(ui, |ui| {
                        for c in SyncChoice::ALL {
                            run_now |= ui.selectable_value(&mut p.sync, c, c.label()).changed();
                        }
                    });
                ui.end_row();
                if w.id == ScenarioId::PairDrumError {
                    ui.label("Drum B larger than entered");
                    run_now |= ui
                        .add(
                            egui::DragValue::new(&mut p.drum_error_pct)
                                .range(0.0..=5.0)
                                .speed(0.05)
                                .suffix(" %"),
                        )
                        .changed();
                    ui.end_row();
                }
                if w.id == ScenarioId::PairDriveTrip {
                    ui.label("Link between the drives");
                    run_now |= ui
                        .checkbox(&mut p.partner_link, "a fault stops the partner")
                        .changed();
                    ui.end_row();
                }
            }
        });
    run_now
}

fn result(ui: &mut egui::Ui, trace: &Trace) {
    for f in &trace.findings {
        novo_theme::widgets::banner(ui, level(f.severity), &f.text, |_| {});
    }
    ui.add_space(4.0);
    for chart_channels in &trace.charts {
        chart(ui, trace, chart_channels);
        ui.add_space(4.0);
    }
    egui::Grid::new("winch_summary")
        .striped(true)
        .show(ui, |ui| {
            for (k, v) in &trace.summary {
                ui.label(k);
                ui.strong(v);
                ui.end_row();
            }
        });
    ui.collapsing(
        format!("What happened when ({})", trace.events.len()),
        |ui| {
            egui::Grid::new("winch_events")
                .striped(true)
                .show(ui, |ui| {
                    for e in &trace.events {
                        ui.weak(format!("{:.2} s", e.t_s));
                        ui.label(&e.text);
                        ui.end_row();
                    }
                });
        },
    );
}

fn honesty(ui: &mut egui::Ui) {
    ui.separator();
    ui.label(egui::RichText::new("Model, not measurement").strong());
    ui.weak(
        "The hardware is generic (30 kW motor, 31.5:1 gear, 0.2 m drum, 400 N m brake, 1024 \
         pulse encoder), not a product. Parameters are meant to be edited, and a drive's real \
         behaviour needs its own parameter data.",
    );
    ui.collapsing("Which numbers are exact and which are a model", |ui| {
        for (what, fidelity) in FIDELITY {
            let tag = if *fidelity == Fidelity::Exact {
                "exact"
            } else {
                "model"
            };
            ui.weak(format!("\u{2022} {what}: {tag}"));
        }
    });
    ui.collapsing("What this model leaves out", |ui| {
        for line in LEAVES_OUT {
            ui.weak(format!("\u{2022} {line}"));
        }
    });
}

impl super::NovoApp {
    /// `sim.winches`: opens or closes the window.
    pub(super) fn toggle_winch_scenarios(&mut self) {
        self.wb.winches.open ^= true;
    }

    pub(super) fn winch_scenarios_window(&mut self, ctx: &egui::Context) {
        if !self.wb.winches.open {
            return;
        }
        let mut open = true;
        let state = &mut self.wb.winches;
        panel::dialog(ctx, "Winch scenarios", [680.0, 720.0])
            .open(panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(
                        "Closed-loop hoists with an encoder, and two winches carrying one load. \
                         Choose what goes wrong and read what the drives, the brakes and the ropes do.",
                    );
                    ui.weak(
                        "A model, not a measurement: generic hardware, averaged drive. See the \
                         notes at the end for what is exact and what is left out.",
                    );
                    ui.add_space(4.0);
                    if controls(ui, state) {
                        state.run();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Run again").clicked() {
                            state.run();
                        }
                    });
                    ui.separator();
                    match &state.result {
                        Some(Ok(_)) => {
                            if let Some(t) = state.trace() {
                                result(ui, t);
                            }
                        }
                        Some(Err(why)) => {
                            novo_theme::widgets::banner(
                                ui,
                                novo_theme::widgets::Level::Error,
                                why,
                                |_| {},
                            );
                        }
                        None => {}
                    }
                    honesty(ui);
                });
            });
        if !open {
            self.wb.winches.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_state_runs_every_scenario_with_its_settings() {
        let mut w = WinchScenarios::default();
        assert!(w.trace().is_some(), "the first run happens on creation");
        for id in ScenarioId::ALL {
            w.id = id;
            for pre in PreChoice::ALL {
                w.pre = pre;
                w.run();
                assert!(w.trace().is_some(), "{id:?} {pre:?}: {:?}", w.result);
            }
        }
    }

    #[test]
    fn charts_name_real_channels() {
        let w = WinchScenarios::default();
        let t = w.trace().unwrap();
        for c in &t.charts {
            assert!(c.iter().all(|i| *i < t.channels.len()));
        }
    }
}
