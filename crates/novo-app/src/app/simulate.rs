//! Electrical simulation in the app: the values the live view (Shift+L)
//! shows (volts on a wire, amps and kW per load) and the "Simulate motor
//! start" window (Ctrl+K, "Simulate"): speed and line current against time
//! for direct, star-delta, auto-transformer and soft starting.
//!
//! The numbers come from novo-sim's analog solver and motor model. They are
//! a model: loads are linear, conductors ideal, a motor's equivalent circuit
//! is fitted to its nameplate, and values the drawing lacks are typical ones
//! that the window lists.

use eframe::egui;
use novo_core::{Netlist, Project, SymbolId};
use novo_sim::PoleState;
use novo_sim::analog::Complex;
use novo_sim::analog::motor::{
    Changeover, Load, Motor, Stage, StartResult, Starter, Supply, start,
};
use novo_sim::network::{Analysis, Note, NoteKind, Reading, analyse, motor_of};
use novo_theme::{tokens, widgets};

/// The analysis for one project revision and pole state, solved again only
/// when either changed.
#[derive(Default)]
pub(super) struct SimState {
    cache: Option<(u64, PoleState, Analysis)>,
    pub(super) motor_open: bool,
    motor: Option<SymbolId>,
    starter: StarterChoice,
    load: LoadChoice,
    /// Percent of rated torque the load takes.
    load_percent: f64,
    /// Load inertia as a multiple of the motor's own.
    load_inertia: f64,
    /// Supply short-circuit current in kA; 0 is a stiff supply.
    ik_ka: f64,
    run: Option<Run>,
    run_key: Option<RunKey>,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum StarterChoice {
    #[default]
    Direct,
    StarDelta,
    AutoTransformer,
    SoftStart,
}

impl StarterChoice {
    const ALL: [StarterChoice; 4] = [
        StarterChoice::Direct,
        StarterChoice::StarDelta,
        StarterChoice::AutoTransformer,
        StarterChoice::SoftStart,
    ];

    fn label(self) -> &'static str {
        match self {
            StarterChoice::Direct => "Direct on line",
            StarterChoice::StarDelta => "Star-delta",
            StarterChoice::AutoTransformer => "Auto-transformer (65 %)",
            StarterChoice::SoftStart => "Soft starter (40 % to 100 % in 8 s)",
        }
    }

    fn starter(self) -> Starter {
        match self {
            StarterChoice::Direct => Starter::Direct,
            StarterChoice::StarDelta => Starter::StarDelta {
                changeover: Changeover::AtSpeed(0.85),
                open_s: 0.08,
            },
            StarterChoice::AutoTransformer => Starter::AutoTransformer {
                tap: 0.65,
                changeover: Changeover::AtSpeed(0.85),
            },
            StarterChoice::SoftStart => Starter::SoftStart {
                initial: 0.4,
                ramp_s: 8.0,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum LoadChoice {
    /// Conveyor, hoist, compressor.
    #[default]
    Constant,
    /// Fan or centrifugal pump.
    Fan,
}

impl LoadChoice {
    fn label(self) -> &'static str {
        match self {
            LoadChoice::Constant => "Constant torque (conveyor, compressor)",
            LoadChoice::Fan => "Fan or pump (torque grows with speed squared)",
        }
    }
}

struct Run {
    motor: Motor,
    notes: Vec<Note>,
    result: StartResult,
}

#[derive(PartialEq, Clone)]
struct RunKey {
    revision: u64,
    motor: SymbolId,
    starter: StarterChoice,
    load: LoadChoice,
    load_percent: u32,
    load_inertia: u32,
    ik: u32,
}

impl SimState {
    pub(super) fn new() -> Self {
        SimState {
            load_percent: 100.0,
            load_inertia: 1.0,
            ..Default::default()
        }
    }

    /// The analysis under `poles`, solved only when something changed.
    pub(super) fn analysis(
        &mut self,
        project: &Project,
        netlist: &Netlist,
        poles: &PoleState,
    ) -> &Analysis {
        let stale = self
            .cache
            .as_ref()
            .is_none_or(|(rev, p, _)| *rev != project.revision() || p != poles);
        if stale {
            let a = analyse(project, netlist, poles);
            self.cache = Some((project.revision(), poles.clone(), a));
        }
        &self.cache.as_ref().expect("solved above").2
    }
}

/// "230 V" or "24.0 V DC" or "0 V": what a net carries, for a tooltip.
pub(super) fn reading_text(r: Reading) -> String {
    if r.ac.abs() > 0.05 {
        format!("{:.0} V AC, {:.0}°", r.ac.abs(), r.ac.arg().to_degrees())
    } else if r.dc.abs() > 0.05 {
        format!("{:.1} V DC", r.dc)
    } else {
        "0 V".to_string()
    }
}

pub(super) fn power_text(watts: f64) -> String {
    if watts.abs() >= 1000.0 {
        format!("{:.2} kW", watts / 1000.0)
    } else {
        format!("{watts:.1} W")
    }
}

/// The "Electrical" block of the live card: each load's current and power,
/// what the supplies deliver, and how many numbers are assumptions.
/// Returns the motor whose "Start..." button was clicked.
pub(super) fn values_block(
    ui: &mut egui::Ui,
    project: &Project,
    analysis: &Analysis,
    name: &dyn Fn(SymbolId) -> String,
) -> Option<SymbolId> {
    let mut start = None;
    let loads: Vec<_> = analysis.loads.iter().filter(|l| l.current > 1e-6).collect();
    let motors: Vec<SymbolId> = analysis
        .loads
        .iter()
        .map(|l| l.symbol)
        .filter(|s| project.symbol(*s).is_some_and(novo_sim::is_motor))
        .collect();
    if loads.is_empty() && motors.is_empty() {
        return None;
    }
    ui.add_space(4.0);
    widgets::section_label(ui, "Electrical");
    for l in &loads {
        ui.horizontal(|ui| {
            ui.label(name(l.symbol));
            ui.weak(format!("{:.2} A · {}", l.current, power_text(l.power_w)));
        });
    }
    if loads.is_empty() {
        ui.weak("Close a contactor to see currents");
    }
    let total: f64 = analysis.supplies.iter().map(|s| s.current).sum();
    if total > 1e-6 {
        ui.weak(format!("Supply delivers {total:.1} A over all conductors"));
    }
    for m in motors {
        if ui
            .small_button(format!("Start {}…", name(m)))
            .on_hover_text("Simulate this motor starting: speed and current")
            .clicked()
        {
            start = Some(m);
        }
    }
    let assumed = analysis
        .notes
        .iter()
        .filter(|n| n.kind == NoteKind::Assumed)
        .count();
    let missing: Vec<_> = analysis
        .notes
        .iter()
        .filter(|n| n.kind == NoteKind::Missing)
        .collect();
    if assumed > 0 {
        ui.weak(format!("{assumed} value(s) assumed from typical data"))
            .on_hover_text(note_list(&analysis.notes, NoteKind::Assumed, name));
    }
    for n in missing {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            format!("{}: {}", n.symbol.map(name).unwrap_or_default(), n.text),
        );
    }
    start
}

fn note_list(notes: &[Note], kind: NoteKind, name: &dyn Fn(SymbolId) -> String) -> String {
    notes
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| match n.symbol {
            Some(s) => format!("{}: {}", name(s), n.text),
            None => n.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rated torque basis for a load choice, N·m at synchronous speed.
fn load_for(choice: LoadChoice, motor: &Motor, percent: f64) -> Load {
    let tn = motor.rated_torque() * percent / 100.0;
    match choice {
        LoadChoice::Constant => Load::Constant(tn),
        LoadChoice::Fan => {
            let ratio = motor.rated_rpm / motor.sync_rpm();
            Load::Quadratic(tn / (ratio * ratio))
        }
    }
}

/// Typical rotor inertia of a four-pole motor, kg·m² (planning value).
fn motor_inertia(motor: &Motor) -> f64 {
    0.04 * (motor.rated_power_w / 7500.0).max(0.05).powf(1.46)
}

impl super::NovoApp {
    /// Opens the motor start window (Ctrl+K, "Simulate motor start").
    pub(super) fn open_motor_start(&mut self) {
        self.wb.sim.motor_open = true;
    }

    /// Opens it on `motor`.
    pub(super) fn open_motor_start_for(&mut self, motor: SymbolId) {
        self.wb.sim.motor_open = true;
        self.wb.sim.motor = Some(motor);
    }

    pub(super) fn motor_start_window(&mut self, ctx: &egui::Context) {
        if !self.wb.sim.motor_open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let motors: Vec<SymbolId> = project
            .symbols()
            .filter(|s| novo_sim::is_motor(s))
            .map(|s| s.id)
            .collect();
        let name = |id: SymbolId| symbol_tag(project, id);
        let state = &mut self.wb.sim;
        if state.motor.is_none_or(|m| !motors.contains(&m)) {
            state.motor = motors.first().copied();
        }
        let mut open = true;
        super::panel::dialog(ctx, "Simulate motor start", [560.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(motor_symbol) = state.motor else {
                    ui.weak("The project has no motor. Place one and give it a power (kW).");
                    return;
                };
                ui.horizontal(|ui| {
                    ui.label("Motor");
                    egui::ComboBox::from_id_salt("sim_motor")
                        .selected_text(name(motor_symbol))
                        .show_ui(ui, |ui| {
                            for &m in &motors {
                                ui.selectable_value(&mut state.motor, Some(m), name(m));
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Starter");
                    egui::ComboBox::from_id_salt("sim_starter")
                        .selected_text(state.starter.label())
                        .show_ui(ui, |ui| {
                            for s in StarterChoice::ALL {
                                ui.selectable_value(&mut state.starter, s, s.label());
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Load");
                    egui::ComboBox::from_id_salt("sim_load")
                        .selected_text(state.load.label())
                        .show_ui(ui, |ui| {
                            for l in [LoadChoice::Constant, LoadChoice::Fan] {
                                ui.selectable_value(&mut state.load, l, l.label());
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut state.load_percent)
                            .range(0.0..=200.0)
                            .suffix(" % of rated torque"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut state.load_inertia)
                            .range(0.0..=50.0)
                            .speed(0.05)
                            .suffix(" × motor inertia (load)"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut state.ik_ka)
                            .range(0.0..=100.0)
                            .speed(0.1)
                            .suffix(" kA supply, 0 = stiff"),
                    );
                });
                let key = RunKey {
                    revision: project.revision(),
                    motor: motor_symbol,
                    starter: state.starter,
                    load: state.load,
                    load_percent: (state.load_percent * 10.0) as u32,
                    load_inertia: (state.load_inertia * 100.0) as u32,
                    ik: (state.ik_ka * 100.0) as u32,
                };
                if state.run_key.as_ref() != Some(&key) {
                    let netlist = project.netlist();
                    state.run = match motor_of(project, &netlist, motor_symbol) {
                        Ok((motor, notes)) => {
                            let v_ll = motor.rated_ll;
                            let supply = if state.ik_ka > 0.0 {
                                let z = v_ll / 3f64.sqrt() / (state.ik_ka * 1000.0);
                                let r = z / 101f64.sqrt();
                                Supply {
                                    v_ll,
                                    source: Complex::new(r, 10.0 * r),
                                }
                            } else {
                                Supply::stiff(v_ll)
                            };
                            let j = motor_inertia(&motor) * (1.0 + state.load_inertia);
                            let result = start(
                                &motor,
                                supply,
                                state.starter.starter(),
                                load_for(state.load, &motor, state.load_percent),
                                j,
                                40.0,
                                0.002,
                            );
                            Some(Run {
                                motor,
                                notes,
                                result,
                            })
                        }
                        Err(why) => {
                            ui.colored_label(crate::theme::ui_palette().warning, why);
                            None
                        }
                    };
                    state.run_key = Some(key);
                }
                match &state.run {
                    Some(run) => show_run(ui, run),
                    None => {
                        ui.add_space(6.0);
                        ui.colored_label(
                            crate::theme::ui_palette().warning,
                            "This motor cannot be modelled yet. Give it a power in kW on its device.",
                        );
                    }
                }
            });
        if !open {
            self.wb.sim.motor_open = false;
        }
    }
}

fn symbol_tag(project: &Project, id: SymbolId) -> String {
    project
        .symbol(id)
        .and_then(|s| s.device())
        .and_then(|d| project.device(d))
        .map_or_else(|| format!("Motor {}", id.0), |d| d.tag.to_string())
}

fn show_run(ui: &mut egui::Ui, run: &Run) {
    let r = &run.result;
    let m = &run.motor;
    ui.add_space(6.0);
    let rated = m.rated_current();
    if r.stalled {
        ui.colored_label(
            crate::theme::ui_palette().error,
            "The motor does not start: the load needs more torque than it makes at standstill",
        );
    } else if let Some(t) = r.start_time {
        ui.label(format!(
            "Up to speed in {t:.1} s · final speed {:.0} rpm",
            r.final_rpm
        ));
    } else {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            "Not up to speed within 40 s",
        );
    }
    ui.weak(format!(
        "Peak line current {:.0} A ({:.1} × rated {:.1} A) · I²t {:.0} kA²s",
        r.peak_line_current,
        r.peak_line_current / rated,
        rated,
        r.i2t / 1e6
    ));
    let (bd, _) = m.breakdown();
    ui.weak(format!(
        "Estimated from the nameplate: locked-rotor torque {:.1} × rated, breakdown {:.1} × rated",
        m.locked_rotor_torque() / m.rated_torque(),
        bd / m.rated_torque()
    ));
    ui.add_space(4.0);
    chart(ui, r, m);
    for n in &run.notes {
        ui.weak(&n.text);
    }
    super::model_labels::badge(ui, super::model_labels::Model::MotorStart);
    super::model_labels::note(ui, super::model_labels::Model::MotorStart);
}

/// Speed (green) and line current (accent) against time.
fn chart(ui: &mut egui::Ui, r: &StartResult, m: &Motor) {
    // As tall as the dialog allows (less the notes under it), so the time
    // axis is not cut off on a small screen.
    let height = (ui.available_height() - 70.0).clamp(130.0, 220.0);
    let size = egui::vec2(ui.available_width().max(300.0), height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, crate::theme::ui_palette().panel);
    let plot = rect.shrink2(egui::vec2(46.0, 18.0));
    let t_end = r
        .start_time
        .map_or(r.samples.last().map_or(1.0, |s| s.t), |t| {
            (t * 1.4).max(1.0)
        })
        .min(r.samples.last().map_or(1.0, |s| s.t));
    let i_max = r.peak_line_current.max(1.0) * 1.1;
    let n_max = m.sync_rpm();
    let at = |t: f64, frac: f64| {
        egui::pos2(
            plot.left() + (t / t_end) as f32 * plot.width(),
            plot.bottom() - frac as f32 * plot.height(),
        )
    };
    for k in 0..=4 {
        let f = f64::from(k) / 4.0;
        let y = at(0.0, f).y;
        p.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(1.0, crate::theme::ui_palette().border_subtle),
        );
        p.text(
            egui::pos2(plot.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}", f * n_max),
            egui::FontId::proportional(tokens::FONT_SMALL),
            crate::theme::ui_palette().success,
        );
        p.text(
            egui::pos2(plot.right() + 4.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{:.0}", f * i_max),
            egui::FontId::proportional(tokens::FONT_SMALL),
            crate::theme::ui_palette().accent,
        );
    }
    p.text(
        egui::pos2(plot.left(), rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        "rpm",
        egui::FontId::proportional(tokens::FONT_SMALL),
        crate::theme::ui_palette().success,
    );
    p.text(
        egui::pos2(plot.right(), rect.top() + 2.0),
        egui::Align2::RIGHT_TOP,
        "A",
        egui::FontId::proportional(tokens::FONT_SMALL),
        crate::theme::ui_palette().accent,
    );
    p.text(
        egui::pos2(plot.right(), plot.bottom() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{t_end:.1} s"),
        egui::FontId::proportional(tokens::FONT_SMALL),
        crate::theme::ui_palette().text_secondary,
    );
    let stride = (r.samples.len() / 600).max(1);
    let shown = r
        .samples
        .iter()
        .step_by(stride)
        .take_while(|s| s.t <= t_end);
    let speed: Vec<_> = shown.clone().map(|s| at(s.t, s.rpm / n_max)).collect();
    let current: Vec<_> = shown
        .clone()
        .map(|s| at(s.t, s.line_current / i_max))
        .collect();
    p.add(egui::Shape::line(
        speed,
        egui::Stroke::new(2.0, crate::theme::ui_palette().success),
    ));
    p.add(egui::Shape::line(
        current,
        egui::Stroke::new(2.0, crate::theme::ui_palette().accent),
    ));
    // Where a two-stage starter changes over.
    if let Some(s) = r.samples.iter().find(|s| s.stage != Stage::First) {
        let x = at(s.t, 0.0).x;
        p.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            egui::Stroke::new(1.0, crate::theme::ui_palette().text_secondary),
        );
        p.text(
            egui::pos2(x + 3.0, plot.top()),
            egui::Align2::LEFT_TOP,
            "changeover",
            egui::FontId::proportional(tokens::FONT_SMALL),
            crate::theme::ui_palette().text_secondary,
        );
    }
    if let Some(pos) = response.hover_pos()
        && plot.contains(pos)
    {
        let t = f64::from((pos.x - plot.left()) / plot.width()) * t_end;
        if let Some(s) = r.samples.iter().find(|s| s.t >= t) {
            response.on_hover_text(format!(
                "{:.2} s · {:.0} rpm · {:.0} A · motor {:.0} N·m, load {:.0} N·m",
                s.t, s.rpm, s.line_current, s.motor_torque, s.load_torque
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_sim::analog::motor::{Connection, Nameplate};

    fn motor() -> Motor {
        Motor::from_nameplate(Nameplate {
            power_w: 7_500.0,
            v_ll: 400.0,
            hz: 50.0,
            rpm: 1_450.0,
            current: 14.6,
            power_factor: 0.83,
            efficiency: 0.893,
            lr_current_ratio: 7.0,
            connection: Connection::Delta,
        })
        .unwrap()
    }

    #[test]
    fn readings_and_power_read_in_plain_units() {
        let ac = Reading {
            dc: 0.0,
            ac: Complex::polar(230.9, -2.0943951),
        };
        assert_eq!(reading_text(ac), "231 V AC, -120°");
        let dc = Reading {
            dc: 24.0,
            ac: Complex::ZERO,
        };
        assert_eq!(reading_text(dc), "24.0 V DC");
        assert_eq!(reading_text(Reading::default()), "0 V");
        assert_eq!(power_text(8_395.0), "8.39 kW");
        assert_eq!(power_text(2.4), "2.4 W");
    }

    #[test]
    fn a_fan_load_takes_rated_torque_at_rated_speed() {
        let m = motor();
        let ws = m.omega_sync();
        let wr = m.rated_rpm * 2.0 * std::f64::consts::PI / 60.0;
        let t = load_for(LoadChoice::Fan, &m, 100.0).torque(wr, ws);
        assert!((t - m.rated_torque()).abs() < 1e-6);
        let t = load_for(LoadChoice::Constant, &m, 50.0).torque(0.0, ws);
        assert!((t - 0.5 * m.rated_torque()).abs() < 1e-9);
    }

    #[test]
    fn typical_inertia_grows_with_power() {
        let m = motor();
        assert!((motor_inertia(&m) - 0.04).abs() < 1e-9);
        let mut big = m;
        big.rated_power_w = 90_000.0;
        assert!(motor_inertia(&big) > 1.0);
    }
}
