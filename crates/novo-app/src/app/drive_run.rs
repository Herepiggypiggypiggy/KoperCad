//! Drive run (`sim.drive_run`, Simulate > Drive run): the live circuit drives the frequency
//! drive and the induction motor of a travel axis through the fixed-step loop of `novo-machine`
//! (lane L5a part 3, design `docs/design/electromechanical-loop.md`).
//!
//! A motor whose property sheet has a moving mass ("Travel drive", `trav_*` properties) is a
//! travel axis. With Live switched on, press the radio buttons of the drawing: the relays pull
//! in, the drive reads its inputs (forward, reverse, step 2), ramps the frequency, the motor
//! follows with its current and power fed back, and a limit switch bound in the property sheet
//! opens its drawn contact at its position. Badges on the motor show frequency, speed, current
//! and position.
//!
//! A motor with a hook load under "Drive hoist" (`dh_*` properties) is an open-loop drive hoist:
//! the drive's brake relay is a contact of the drawing that switches the brake contactor, the
//! drive opens the brake only when the motor current says there is torque, and the run lists what
//! it found (a hook that sank at the start, a brake that does not hold, a trip on over-voltage).
//!
//! A motor with a hook load under "Closed-loop winch" (`cw_*` properties) is a hoist behind an
//! encoder drive: torque is held at zero speed, the brake opens only after the drive proved its
//! torque, and the two STO channels, the encoder signal and the brake contactor are devices of the
//! drawing, so a defect injected there (Simulate > Defects and walk-throughs) reaches the drive.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use eframe::egui;
use novo_core::{PageId, Project, Value};
use novo_machine::cl_winch_loop::{ClWinchLoop, ClWinchModule};
use novo_machine::cl_winch_spec;
use novo_machine::cosim::StepConfig;
use novo_machine::drive_hoist_loop::{DriveHoistLoop, DriveHoistModule, FindingKind};
use novo_machine::drive_hoist_spec;
use novo_machine::duty_cycle::{self, DutyCycle, DutyEnd, DutyReport};
use novo_machine::travel_loop::{TravelLoop, TravelModule};
use novo_machine::travel_spec;
use novo_sim::is_motor;

use super::hoist_run::LiveCircuit;
use super::live::LiveView;
use super::{Level, NovoApp};

/// Most ticks run in one frame, so a slow frame cannot snowball.
const MAX_TICKS: usize = 40;

/// What the Simulate > Drive run window remembers.
#[derive(Default)]
pub(crate) struct DriveRun {
    pub(super) open: bool,
    active: Option<Active>,
    duty: DutyUi,
    /// Why the last start did not run, or what it found.
    notice: Vec<(Level, String)>,
}

/// The duty cycle form, the job running in the background and the last result.
struct DutyUi {
    travel_m: f64,
    step: usize,
    pause_top_s: f64,
    pause_bottom_s: f64,
    cycles: u32,
    stop_when_hot: bool,
    choice: usize,
    job: Option<DutyJob>,
    result: Option<(String, Result<DutyReport, String>)>,
}

impl Default for DutyUi {
    fn default() -> Self {
        Self {
            travel_m: 3.0,
            step: 3,
            pause_top_s: 10.0,
            pause_bottom_s: 10.0,
            cycles: 30,
            stop_when_hot: true,
            choice: 0,
            job: None,
            result: None,
        }
    }
}

struct DutyJob {
    tag: String,
    total: u32,
    progress: Arc<AtomicU32>,
    rx: mpsc::Receiver<Result<DutyReport, String>>,
}

/// Every drive hoist of a project with its property text, for a duty cycle run in the
/// background (the module is built again inside the job).
fn hoist_properties(project: &Project) -> Vec<(String, BTreeMap<String, String>)> {
    project
        .devices()
        .filter(|d| d.properties.contains_key(drive_hoist_spec::DRIVE_HOIST_KEY))
        .map(|d| {
            let props = d
                .properties
                .iter()
                .map(|(k, v): (&String, &Value)| (k.clone(), v.to_string()))
                .collect();
            (d.tag.to_string(), props)
        })
        .collect()
}

fn spawn_duty(tag: String, props: BTreeMap<String, String>, cycle: DutyCycle) -> DutyJob {
    let (tx, rx) = mpsc::channel();
    let progress = Arc::new(AtomicU32::new(0));
    let seen = Arc::clone(&progress);
    let total = cycle.cycles;
    let id = tag.clone();
    std::thread::spawn(move || {
        let result = (|| {
            let get = |k: &str| props.get(k).cloned();
            let (module, _) = drive_hoist_spec::drive_hoist_from_properties(&id, &get)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "this motor has no hook load".to_owned())?;
            duty_cycle::run(module, &cycle, &mut |n| seen.store(n, Ordering::Relaxed))
                .map_err(|e| e.to_string())
        })();
        let _ = tx.send(result);
    });
    DutyJob {
        tag,
        total,
        progress,
        rx,
    }
}

fn duration_text(s: f64) -> String {
    if s >= 3600.0 {
        format!(
            "{:.0} h {:.0} min",
            (s / 3600.0).floor(),
            (s % 3600.0) / 60.0
        )
    } else if s >= 60.0 {
        format!("{:.0} min {:.0} s", (s / 60.0).floor(), s % 60.0)
    } else {
        format!("{s:.0} s")
    }
}

/// The result of a duty cycle run: figures, the curve of the winding and the brake, and what the
/// run found.
fn show_duty_result(ui: &mut egui::Ui, tag: &str, report: &DutyReport) {
    let end = match report.end {
        DutyEnd::Completed => "completed",
        DutyEnd::MotorTooHot => "stopped: the winding reached its limit",
        DutyEnd::BrakeTooHot => "stopped: the brake reached its limit",
        DutyEnd::Stuck => "stopped: the hoist did not finish a stroke (fault or stuck brake)",
        DutyEnd::TimeCap => "stopped at the 24 h cap",
    };
    ui.strong(format!("{tag}: {end}"));
    ui.label(format!(
        "{} cycles in {} \u{b7} {} starts ({:.0} per hour)",
        report.cycles_done,
        duration_text(report.time_s),
        report.starts,
        report.starts_per_hour()
    ));
    ui.label(format!(
        "Winding peak {:.0} K of {:.0} K \u{b7} brake peak {:.0} K of {:.0} K \u{b7} brake lining \
         used {:.1} % ({:.0} kJ slipped)",
        report.peak_winding_rise_k,
        report.winding_limit_k,
        report.peak_brake_rise_k,
        report.brake_limit_k,
        report.brake_wear * 100.0,
        report.brake_energy_j / 1000.0
    ));
    if let Some(t) = report.first_hot_s {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "The winding first reached its limit after {}.",
                duration_text(t)
            ),
        );
    }
    if report.curve.len() > 1 {
        let width = ui.available_width().clamp(200.0, 520.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 84.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, crate::theme::ui_palette().raised);
        let t_max = report.curve.last().map_or(1.0, |c| c.0).max(1.0);
        let y_max = (report.winding_limit_k * 1.1)
            .max(report.peak_winding_rise_k * 1.05)
            .max(report.peak_brake_rise_k * 1.05)
            .max(1.0);
        let at = |t: f64, v: f64| {
            egui::pos2(
                rect.min.x + (t / t_max) as f32 * rect.width(),
                rect.max.y - (v / y_max) as f32 * rect.height(),
            )
        };
        let limit_y = at(0.0, report.winding_limit_k).y;
        painter.line_segment(
            [
                egui::pos2(rect.min.x, limit_y),
                egui::pos2(rect.max.x, limit_y),
            ],
            egui::Stroke::new(1.0, ui.visuals().warn_fg_color),
        );
        let line = |pick: &dyn Fn(&(f64, f64, f64)) -> f64, color: egui::Color32| {
            let pts: Vec<egui::Pos2> = report.curve.iter().map(|c| at(c.0, pick(c))).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, color)));
        };
        line(&|c| c.1, ui.visuals().hyperlink_color);
        line(&|c| c.2, ui.visuals().text_color());
        ui.weak(
            "Blue: winding rise, grey: brake rise (K over ambient), line: winding limit. \
             Time runs left to right.",
        );
    }
    for f in report
        .findings
        .iter()
        .filter(|f| {
            matches!(
                f.kind,
                FindingKind::MotorTooHot { .. }
                    | FindingKind::BrakeHot { .. }
                    | FindingKind::BrakeWear { .. }
                    | FindingKind::DriveTrip(_)
            )
        })
        .take(6)
    {
        ui.horizontal_wrapped(|ui| {
            ui.weak(duration_text(f.time_s));
            ui.label(f.kind.text());
        });
    }
}

struct Active {
    lp: TravelLoop,
    hl: DriveHoistLoop,
    cw: ClWinchLoop,
    acc: f64,
}

/// The open-loop drive hoists of a project: every motor with a hook load under "Drive hoist". A
/// hoist whose numbers the engine refuses is reported and left out.
pub(super) fn drive_hoists_of(project: &Project) -> (Vec<DriveHoistModule>, Vec<(Level, String)>) {
    let mut modules = Vec::new();
    let mut notes = Vec::new();
    for device in project.devices() {
        if !device
            .properties
            .contains_key(drive_hoist_spec::DRIVE_HOIST_KEY)
        {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(|v: &Value| v.to_string());
        match drive_hoist_spec::drive_hoist_from_properties(&tag, &get) {
            Ok(Some((module, fit))) => {
                modules.push(module);
                notes.extend(
                    fit.into_iter()
                        .map(|n| (Level::Info, format!("{tag}: {n}"))),
                );
            }
            Ok(None) => {}
            Err(why) => notes.push((Level::Warning, format!("{tag} is left out: {why}"))),
        }
    }
    (modules, notes)
}

/// The closed-loop winches of a project: every motor with a hook load under "Closed-loop winch". A
/// winch whose numbers the engine refuses is reported and left out. The notes are the design
/// findings of its numbers.
pub(super) fn closed_winches_of(project: &Project) -> (Vec<ClWinchModule>, Vec<(Level, String)>) {
    let mut modules = Vec::new();
    let mut notes = Vec::new();
    for device in project.devices() {
        if !device
            .properties
            .contains_key(cl_winch_spec::CLOSED_WINCH_KEY)
        {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(|v: &Value| v.to_string());
        match cl_winch_spec::closed_winch_from_properties(&tag, &get) {
            Ok(Some((module, found))) => {
                modules.push(module);
                notes.extend(
                    found
                        .into_iter()
                        .map(|n| (Level::Info, format!("{tag}: {n}"))),
                );
            }
            Ok(None) => {}
            Err(why) => notes.push((Level::Warning, format!("{tag} is left out: {why}"))),
        }
    }
    (modules, notes)
}

/// A one-line reading of a closed-loop winch, for the badge and the window.
pub(super) fn winch_reading(module: &ClWinchModule) -> String {
    use novo_drive::closed_loop::ClState;
    let Some(s) = module.snapshot() else {
        return "waiting".to_owned();
    };
    let state = match s.drive.fault {
        Some(f) => format!("tripped {}", f.code()),
        None => match s.drive.state {
            ClState::Off => "brake closed".to_owned(),
            ClState::Magnetising => "magnetising".to_owned(),
            ClState::TorqueProof => "proving torque".to_owned(),
            ClState::Releasing => "opening brake".to_owned(),
            ClState::Running => "running".to_owned(),
            ClState::Stopping => "slowing".to_owned(),
            ClState::BrakeClosing => "holding, brake closing".to_owned(),
            ClState::StoActive => "STO".to_owned(),
            ClState::Faulted => "stopped by a fault".to_owned(),
        },
    };
    format!(
        "{state} \u{b7} {:.0} rpm \u{b7} {:.0} N m \u{b7} {:.1} A \u{b7} {:.1} m/min \u{b7} {:.2} m",
        s.shaft_rpm,
        s.drive.torque_nm,
        s.drive.current_a,
        s.hook_speed_m_s * 60.0,
        s.hook_height_m
    )
}

/// A one-line reading of a drive hoist, for the badge and the window.
pub(super) fn hoist_reading(module: &DriveHoistModule) -> String {
    if let Some(why) = module.fault() {
        return why.to_owned();
    }
    let Some(s) = module.snapshot() else {
        return "waiting".to_owned();
    };
    use novo_drive::hoist_logic::LogicPhase;
    let phase = match s.logic.phase {
        LogicPhase::Idle => "brake closed",
        LogicPhase::Magnetising => "magnetising",
        LogicPhase::Releasing => "opening brake",
        LogicPhase::Running => "running",
        LogicPhase::Decelerating => "slowing",
        LogicPhase::Engaging => "closing brake",
        LogicPhase::Fault(_) => "stopped by a fault",
    };
    let heat = s.heat.map_or_else(String::new, |h| {
        format!(
            " \u{b7} winding +{:.0} K \u{b7} brake +{:.0} K",
            h.winding_rise_k, h.brake_rise_k
        )
    });
    format!(
        "{phase} \u{b7} {:.1} Hz \u{b7} {:.1} A \u{b7} {:.1} m/min \u{b7} {:.2} m{heat}",
        s.drive.frequency_hz.abs(),
        s.motor.stator_current_a,
        s.speed_m_s * 60.0,
        s.height_m
    )
}

impl DriveRun {
    pub(super) fn is_running(&self) -> bool {
        self.active.is_some()
    }

    /// Position in metres from the start (forward positive) and speed in m/s of a travel axis of
    /// the run, `None` without a run, for an unknown axis or before its first tick.
    pub(super) fn travel_of(&self, id: &str) -> Option<(f64, f64)> {
        let module = self.active.as_ref()?.lp.module(id)?;
        let speed = module.snapshot()?.speed_m_s;
        Some((module.position_m(), speed))
    }
}

/// The tags of the travel axes of a project (motors with a moving mass), without fitting their
/// motors: cheap enough to ask every frame.
pub(super) fn travel_ids(project: &Project) -> Vec<String> {
    project
        .devices()
        .filter(|d| d.properties.contains_key(travel_spec::TRAVEL_KEY))
        .map(|d| d.tag.to_string())
        .collect()
}

/// The travel axes of a project: every motor with a moving mass set. An axis whose numbers the
/// engine refuses is reported and left out. The notes are what the motor fit assumed.
pub(super) fn axes_of(project: &Project) -> (Vec<TravelModule>, Vec<(Level, String)>) {
    let mut modules = Vec::new();
    let mut notes = Vec::new();
    for device in project.devices() {
        if !device.properties.contains_key(travel_spec::TRAVEL_KEY) {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(|v: &Value| v.to_string());
        match travel_spec::travel_from_properties(&tag, &get) {
            Ok(Some((module, fit))) => {
                modules.push(module);
                notes.extend(
                    fit.into_iter()
                        .map(|n| (Level::Info, format!("{tag}: {n}"))),
                );
            }
            Ok(None) => {}
            Err(why) => notes.push((Level::Warning, format!("{tag} is left out: {why}"))),
        }
    }
    (modules, notes)
}

/// Steps the loop for one frame. Returns whether a run is active (the frame then reads the
/// circuit without advancing its own clock again).
pub(super) fn advance(
    run: &mut DriveRun,
    live: &mut LiveView,
    project: &Project,
    frame_dt: f64,
) -> bool {
    let Some(active) = run.active.as_mut() else {
        return false;
    };
    if !live.on {
        return false;
    }
    let dt = active.lp.config.dt_s;
    active.acc += frame_dt.clamp(0.0, 0.1);
    let mut circuit = LiveCircuit::new(live, project);
    let mut ticks = 0;
    while active.acc >= dt && ticks < MAX_TICKS {
        active.lp.step(&mut circuit);
        active.hl.step(&mut circuit);
        active.cw.step(&mut circuit);
        active.acc -= dt;
        ticks += 1;
    }
    if ticks == MAX_TICKS {
        active.acc = 0.0;
    }
    true
}

/// A one-line reading of an axis, for the badge and the window.
pub(super) fn reading(module: &TravelModule) -> String {
    if let Some(why) = module.fault() {
        return why.to_owned();
    }
    let Some(s) = module.snapshot() else {
        return "waiting".to_owned();
    };
    format!(
        "{:.1} Hz \u{b7} {:.0} rpm \u{b7} {:.1} A \u{b7} {:.2} m/s \u{b7} {:.2} m",
        s.drive.frequency_hz.abs(),
        module.rpm().abs(),
        s.motor.stator_current_a,
        s.speed_m_s.abs(),
        s.position_m
    )
}

/// Badges on the motors of the axes, on the page shown.
pub(super) fn paint_badges(
    ctx: &egui::Context,
    canvas: egui::Rect,
    to_screen: &dyn Fn(egui::Vec2) -> egui::Pos2,
    project: &Project,
    library: Option<&novo_symbols::Library>,
    page: PageId,
    run: &DriveRun,
) {
    let Some(active) = run.active.as_ref() else {
        return;
    };
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("drive_badges"),
        ))
        .with_clip_rect(canvas);
    let palette = crate::theme::ui_palette();
    let readings: Vec<(&str, String)> = active
        .lp
        .modules
        .iter()
        .map(|m| (m.id.as_str(), reading(m)))
        .chain(
            active
                .hl
                .modules
                .iter()
                .map(|m| (m.id.as_str(), hoist_reading(m))),
        )
        .chain(
            active
                .cw
                .modules
                .iter()
                .map(|m| (m.id.as_str(), winch_reading(m))),
        )
        .collect();
    for (id, text) in readings {
        for symbol in project.symbols().filter(|s| s.page == page && is_motor(s)) {
            let tag = symbol
                .device()
                .and_then(|d| project.device(d))
                .map(|d| d.tag.to_string());
            if tag.as_deref() != Some(id) {
                continue;
            }
            let Some((at, r)) = super::live_anim::body(library, symbol) else {
                continue;
            };
            let centre = to_screen(at);
            let galley = painter.layout_no_wrap(
                text.clone(),
                egui::FontId::proportional(12.0),
                palette.text_strong,
            );
            let pos = centre + egui::vec2(-galley.size().x / 2.0, r * 3.0 + 6.0);
            let rect = egui::Rect::from_min_size(pos, galley.size()).expand2(egui::vec2(6.0, 3.0));
            painter.rect_filled(rect, 4.0, palette.raised);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, palette.border_control),
                egui::StrokeKind::Inside,
            );
            painter.galley(pos, galley, palette.text_strong);
        }
    }
    if active.lp.modules.iter().any(|m| m.snapshot().is_some())
        || active.hl.modules.iter().any(|m| m.snapshot().is_some())
        || active.cw.modules.iter().any(|m| m.snapshot().is_some())
    {
        ctx.request_repaint();
    }
}

/// The duty cycle form: what to lift, how often, and the result of the last run.
fn duty_cycle_form(
    ui: &mut egui::Ui,
    duty: &mut DutyUi,
    hoists: &[(String, BTreeMap<String, String>)],
) {
    ui.weak(
        "Lifts and lowers the chosen hoist again and again, faster than real time, with an \
         ideal operator and wiring (not the drawing), and shows how the motor winding and the \
         brake heat up. The heat numbers are generic (Drive hoist section, heat and brake \
         rows) until you enter the maker's.",
    );
    duty.choice = duty.choice.min(hoists.len() - 1);
    let running = duty.job.is_some();
    ui.add_enabled_ui(!running, |ui| {
        if hoists.len() > 1 {
            egui::ComboBox::from_label("Hoist")
                .selected_text(&hoists[duty.choice].0)
                .show_ui(ui, |ui| {
                    for (i, (tag, _)) in hoists.iter().enumerate() {
                        ui.selectable_value(&mut duty.choice, i, tag);
                    }
                });
        }
        egui::Grid::new("duty_form").num_columns(2).show(ui, |ui| {
            ui.label("Travel per stroke (m)");
            ui.add(
                egui::DragValue::new(&mut duty.travel_m)
                    .range(0.1..=100.0)
                    .speed(0.1),
            );
            ui.end_row();
            ui.label("Speed step (1 = slowest)");
            ui.add(egui::DragValue::new(&mut duty.step).range(1..=4));
            ui.end_row();
            ui.label("Pause at the top (s)");
            ui.add(egui::DragValue::new(&mut duty.pause_top_s).range(0.0..=3600.0));
            ui.end_row();
            ui.label("Pause at the bottom (s)");
            ui.add(egui::DragValue::new(&mut duty.pause_bottom_s).range(0.0..=3600.0));
            ui.end_row();
            ui.label("Cycles (up and down)");
            ui.add(egui::DragValue::new(&mut duty.cycles).range(1..=5000));
            ui.end_row();
        });
        ui.checkbox(
            &mut duty.stop_when_hot,
            "Stop when the winding or the brake reaches its limit",
        );
    });
    if let Some(job) = duty.job.as_ref() {
        let done = job.progress.load(Ordering::Relaxed);
        ui.add(
            egui::ProgressBar::new(done as f32 / job.total.max(1) as f32)
                .text(format!("{} {done} of {} cycles", job.tag, job.total)),
        );
    } else if ui.button("Run duty cycle").clicked() {
        let (tag, props) = hoists[duty.choice].clone();
        let cycle = DutyCycle {
            travel_m: duty.travel_m,
            step: duty.step,
            pause_top_s: duty.pause_top_s,
            pause_bottom_s: duty.pause_bottom_s,
            cycles: duty.cycles,
            dt_s: 0.005,
            stop_when_hot: duty.stop_when_hot,
        };
        duty.result = None;
        duty.job = Some(spawn_duty(tag, props, cycle));
    }
    match duty.result.as_ref() {
        Some((tag, Ok(report))) => show_duty_result(ui, tag, report),
        Some((tag, Err(why))) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("{tag}: {why}"));
        }
        None => {}
    }
}

impl NovoApp {
    /// `sim.drive_run`: opens or closes the window.
    pub(super) fn toggle_drive_run(&mut self) {
        self.wb.drive_run.open ^= true;
    }

    pub(super) fn drive_run_window(&mut self, ctx: &egui::Context) {
        if !self.wb.drive_run.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let (axes, _) = axes_of(doc.project());
        let (drive_hoists, _) = drive_hoists_of(doc.project());
        let (winches, _) = closed_winches_of(doc.project());
        let live_on = self.wb.live.on;
        let hoist_props = hoist_properties(doc.project());
        let mut duty = std::mem::take(&mut self.wb.drive_run.duty);
        if let Some(job) = duty.job.as_ref() {
            match job.rx.try_recv() {
                Ok(result) => {
                    duty.result = Some((job.tag.clone(), result));
                    duty.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint(),
                Err(mpsc::TryRecvError::Disconnected) => {
                    duty.result = Some((job.tag.clone(), Err("the run stopped".to_owned())));
                    duty.job = None;
                }
            }
        }
        let mut open = true;
        let (mut start, mut stop, mut live_toggle) = (false, false, false);
        let run = &self.wb.drive_run;
        let rows: Vec<(String, String)> = run.active.as_ref().map_or_else(Vec::new, |a| {
            a.lp.modules
                .iter()
                .map(|m| (m.id.clone(), reading(m)))
                .chain(
                    a.hl.modules
                        .iter()
                        .map(|m| (m.id.clone(), hoist_reading(m))),
                )
                .chain(
                    a.cw.modules
                        .iter()
                        .map(|m| (m.id.clone(), winch_reading(m))),
                )
                .collect()
        });
        // What the drive hoists found, newest last (the run keeps the whole list).
        let findings: Vec<(String, String)> = run.active.as_ref().map_or_else(Vec::new, |a| {
            a.hl.modules
                .iter()
                .flat_map(|m| {
                    m.findings()
                        .iter()
                        .map(|f| (format!("{} \u{b7} {:.1} s", m.id, f.time_s), f.kind.text()))
                })
                .chain(a.cw.modules.iter().flat_map(|m| {
                    m.findings()
                        .iter()
                        .map(|f| (format!("{} \u{b7} {:.1} s", m.id, f.time_s), f.kind.text()))
                }))
                .collect()
        });
        let running = run.active.is_some();
        let notice = run.notice.clone();
        let names: Vec<String> = axes
            .iter()
            .map(|a| a.id.clone())
            .chain(drive_hoists.iter().map(|h| h.id.clone()))
            .chain(winches.iter().map(|w| w.id.clone()))
            .collect();
        // Foreground: the marks of the live view (contacts, coil rings) are painted on the Middle
        // layer and must not run through the notes.
        super::panel::dialog(ctx, "Drive run", [560.0, 560.0])
            .default_pos([ctx.content_rect().max.x, 60.0])
            .order(egui::Order::Foreground)
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.label(
                    "Press the radio buttons of the drawing in the live view: the relays pull in, \
                     the drive ramps its frequency, the motor follows with its current fed back, \
                     and the end limits you bound open their contacts at their position.",
                );
                ui.add_space(4.0);
                if names.is_empty() {
                    ui.weak(
                        "No travel axis, drive hoist or closed-loop winch yet. Select a motor and \
                         enter its moving mass under Travel drive, or its hook load under Drive \
                         hoist or Closed-loop winch, in the property sheet.",
                    );
                } else {
                    ui.label(format!(
                        "Travel axes, drive hoists and closed-loop winches in this project: {}",
                        names.join(", ")
                    ));
                }
                ui.horizontal(|ui| {
                    if !live_on && ui.button("Switch on Live").clicked() {
                        live_toggle = true;
                    }
                    let can = live_on && !names.is_empty();
                    if running {
                        if ui.button("Stop").clicked() {
                            stop = true;
                        }
                    } else if ui.add_enabled(can, egui::Button::new("Start")).clicked() {
                        start = true;
                    }
                    if !live_on {
                        ui.weak("The run reads the live view (Shift+L).");
                    }
                });
                // Solid behind the notes: marks of the drawing must not run through them.
                let palette = crate::theme::ui_palette();
                let fit: Vec<&str> = notice
                    .iter()
                    .filter(|(l, _)| matches!(l, Level::Info))
                    .map(|(_, t)| t.as_str())
                    .collect();
                egui::Frame::new().fill(palette.raised).show(ui, |ui| {
                    for (level, text) in notice.iter().filter(|(l, _)| !matches!(l, Level::Info)) {
                        let w = match level {
                            Level::Error => novo_theme::widgets::Level::Error,
                            _ => novo_theme::widgets::Level::Warning,
                        };
                        novo_theme::widgets::banner(ui, w, text, |_| {});
                    }
                    if !fit.is_empty() {
                        ui.collapsing(
                            format!("What the motor fit assumed ({})", fit.len()),
                            |ui| {
                                for line in &fit {
                                    ui.weak(format!("\u{2022} {line}"));
                                }
                            },
                        );
                    }
                });
                if !rows.is_empty() {
                    ui.separator();
                    egui::Grid::new("drive_run_rows")
                        .striped(true)
                        .show(ui, |ui| {
                            for (tag, text) in &rows {
                                ui.strong(tag);
                                ui.label(text);
                                ui.end_row();
                            }
                        });
                }
                if !findings.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("What the hoist run found").strong());
                    for (when, text) in findings.iter().rev().take(12) {
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(when);
                            ui.label(text);
                        });
                    }
                }
                if !hoist_props.is_empty() {
                    ui.separator();
                    egui::CollapsingHeader::new("Duty cycle: heat over time")
                        .id_salt("duty_cycle")
                        .show(ui, |ui| {
                            duty_cycle_form(ui, &mut duty, &hoist_props);
                        });
                }
                ui.separator();
                ui.label(egui::RichText::new("Model, not measurement").strong());
                ui.weak(
                    "\u{2022} The drive is a generic V/f drive (ramps, current limit, trip \
                     levels); its parameters are the ones you enter, not a vendor's.",
                );
                ui.weak(
                    "\u{2022} The motor is fitted from the plate (equivalent circuit, constant \
                     rotor resistance): it can underestimate the standstill torque of large \
                     deep-bar motors.",
                );
                ui.weak(
                    "\u{2022} Rolling resistance is one constant; no stiction, no travel brake, \
                     no skew or sway yet.",
                );
                ui.weak(
                    "\u{2022} The drive replaces the contactors: its output terminals are not \
                     traced through the drawing, its inputs and mains are.",
                );
                ui.weak(
                    "\u{2022} Drive hoist: the brake sequence (magnetise, open the brake on \
                     current, close it at a low frequency) is a generic model with your numbers, \
                     not a maker's firmware or parameter set. The motor flux builds with one \
                     time constant; a stuck or missing brake contactor shows as a brake that \
                     never opens.",
                );
                ui.weak(
                    "\u{2022} Closed-loop winch: an averaged encoder drive (speed loop around a \
                     first order torque loop, current limit, constant power above rated speed). \
                     Torque is proven before the brake opens and held at zero speed; the STO \
                     channels, the encoder signal and the brake contactor are read from the \
                     drawing. Brake timing is entered, not measured; the plausibility check uses \
                     the true shaft speed. Not a maker's firmware.",
                );
                ui.weak("\u{2022} Not a safety or compliance statement.");
            });
        self.wb.drive_run.duty = duty;
        if !open {
            self.wb.drive_run.open = false;
        }
        if live_toggle {
            self.toggle_live();
        }
        if stop {
            self.wb.drive_run.active = None;
        }
        if start {
            self.start_drive_run();
        }
    }

    /// Builds the loop from the travel axes of the project and checks their bindings.
    fn start_drive_run(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let (modules, mut notes) = axes_of(project);
        let (hoist_modules, hoist_notes) = drive_hoists_of(project);
        notes.extend(hoist_notes);
        let (winch_modules, winch_notes) = closed_winches_of(project);
        notes.extend(winch_notes);
        if modules.is_empty() && hoist_modules.is_empty() && winch_modules.is_empty() {
            notes.push((
                Level::Warning,
                "No axis can run: see the notes above.".into(),
            ));
            self.wb.drive_run.notice = notes;
            return;
        }
        let mut lp = match TravelLoop::new(StepConfig::default()) {
            Ok(lp) => lp,
            Err(why) => {
                self.wb.drive_run.notice = vec![(Level::Error, why.to_string())];
                return;
            }
        };
        lp.modules = modules;
        let mut hl = match DriveHoistLoop::new(StepConfig::default()) {
            Ok(hl) => hl,
            Err(why) => {
                self.wb.drive_run.notice = vec![(Level::Error, why.to_string())];
                return;
            }
        };
        hl.modules = hoist_modules;
        let mut cw = match ClWinchLoop::new(StepConfig::default()) {
            Ok(cw) => cw,
            Err(why) => {
                self.wb.drive_run.notice = vec![(Level::Error, why.to_string())];
                return;
            }
        };
        cw.modules = winch_modules;
        let findings = {
            let circuit = LiveCircuit::new(&mut self.wb.live, project);
            let mut f = lp.check_bindings(&circuit);
            f.extend(hl.check_bindings(&circuit));
            f.extend(cw.check_bindings(&circuit));
            f
        };
        notes.extend(
            findings
                .into_iter()
                .map(|f| (Level::Warning, format!("{}: {}", f.module, f.message))),
        );
        self.wb.drive_run.notice = notes;
        self.wb.drive_run.active = Some(Active {
            lp,
            hl,
            cw,
            acc: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{
        DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId,
        SymbolInstance, SymbolRole,
    };

    fn put(
        tx: &mut novo_core::Tx<'_>,
        page: PageId,
        key: &str,
        y: f64,
        role: SymbolRole,
    ) -> SymbolId {
        let id = tx
            .place_symbol(
                page,
                key,
                Placement::at(Point::new(0.0, y)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                role,
            )
            .unwrap();
        if key.contains("contact") {
            tx.update::<SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
                .unwrap();
        }
        id
    }

    /// L1 -> button S1 -> limit S9 (NC) -> coil KF -> N; the drive's forward input is KF.
    fn axis(limit_m: Option<f64>) -> (Document, SymbolId) {
        let mut doc = Document::default();
        let (button, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let dev = |d, main| SymbolRole::Device { device: d, main };
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let kf = tx.add_device(DeviceTag::product("KF"), "")?;
                let s1 = tx.add_device(DeviceTag::product("S1"), "")?;
                let s9 = tx.add_device(DeviceTag::product("S9"), "")?;
                let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
                let feed = put(
                    tx,
                    page,
                    "t/feed",
                    0.0,
                    SymbolRole::PotentialFeed { potential: l1 },
                );
                let feed_n = put(
                    tx,
                    page,
                    "t/feed",
                    30.0,
                    SymbolRole::PotentialFeed { potential: n },
                );
                let button = put(tx, page, "t/contact/no", 80.0, dev(s1, true));
                let limit = put(tx, page, "t/contact/nc", 90.0, dev(s9, true));
                let coil = put(tx, page, "t/coil", 100.0, dev(kf, true));
                tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(button, 0))?;
                tx.connect(Endpoint::pin(button, 1), Endpoint::pin(limit, 0))?;
                tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(coil, 0))?;
                tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 1))?;
                let m = tx.project().device(m1).map(|d| d.id).unwrap();
                tx.set_property(m, "trav_mass_kg", Some(5000.0.into()))?;
                tx.set_property(
                    m,
                    "trav_in_fwd",
                    Some(DeviceTag::product("KF").to_string().into()),
                )?;
                if let Some(at) = limit_m {
                    tx.set_property(
                        m,
                        "trav_limit_fwd",
                        Some(DeviceTag::product("S9").to_string().into()),
                    )?;
                    tx.set_property(m, "trav_limit_fwd_m", Some(at.into()))?;
                }
                Ok(button)
            })
            .unwrap();
        (doc, button)
    }

    fn run_held(doc: &Document, button: SymbolId, seconds: f64) -> DriveRun {
        let project = doc.project();
        let (modules, notes) = axes_of(project);
        assert!(
            notes.iter().all(|(l, _)| matches!(l, Level::Info)),
            "{notes:?}"
        );
        let mut lp = TravelLoop::new(StepConfig::default()).unwrap();
        lp.modules = modules;
        let mut run = DriveRun {
            active: Some(Active {
                lp,
                hl: DriveHoistLoop::new(StepConfig::default()).unwrap(),
                cw: ClWinchLoop::new(StepConfig::default()).unwrap(),
                acc: 0.0,
            }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        live.poles.set_all(project, button, true);
        for _ in 0..(seconds / 0.016) as usize {
            advance(&mut run, &mut live, project, 0.016);
        }
        run
    }

    fn position(run: &DriveRun) -> f64 {
        run.active.as_ref().unwrap().lp.modules[0].position_m()
    }

    #[test]
    fn the_button_in_the_drawing_drives_the_axis() {
        let (doc, button) = axis(None);
        let run = run_held(&doc, button, 20.0);
        // Step 1 is 10 Hz: about 0.15 m/s after the 4 s ramp (5 s at 50 Hz scale).
        assert!(position(&run) > 1.5, "{}", position(&run));
        assert!(reading(&run.active.as_ref().unwrap().lp.modules[0]).contains("Hz"));
    }

    #[test]
    fn the_3d_picture_follows_a_travel_axis_and_the_hook_leans_back_when_it_starts() {
        let (doc, button) = axis(None);
        let ids = travel_ids(doc.project());
        assert_eq!(ids.len(), 1, "{ids:?}");
        // Drive the axis by hand, frame by frame, and feed the picture the way the window does.
        let project = doc.project();
        let (modules, _) = axes_of(project);
        let mut lp = TravelLoop::new(StepConfig::default()).unwrap();
        lp.modules = modules;
        let mut run = DriveRun {
            active: Some(Active {
                lp,
                hl: DriveHoistLoop::new(StepConfig::default()).unwrap(),
                cw: ClWinchLoop::new(StepConfig::default()).unwrap(),
                acc: 0.0,
            }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        live.poles.set_all(project, button, true);
        let mut picture = super::super::hoist_3d::Hoist3d::default();
        let start = picture.bridge_start_m();
        let mut lean = 0.0_f32;
        let mut last = None;
        for _ in 0..1250 {
            advance(&mut run, &mut live, project, 0.016);
            let t = picture.travel(0.016, [run.travel_of(&ids[0]), None], 4.0);
            lean = lean.min(t.swing_rad[0]);
            // The trolley is not driven: it stays put and its hook does not swing.
            assert_eq!(t.swing_rad[1], 0.0);
            last = Some(t);
        }
        let t = last.unwrap();
        let moved = position(&run);
        assert!(moved > 1.5, "{moved}");
        assert!((f64::from(t.bridge_m) - (f64::from(start) + moved)).abs() < 1e-3);
        // Pulled forward, the hook trails behind: a negative lean, about atan(a/g) for the ramp.
        assert!(lean < -0.001, "{lean}");
        assert!(lean > -0.05, "{lean}");
        assert!(!t.bridge_at_end);
    }

    #[test]
    fn the_drawn_end_limit_stops_the_axis() {
        let (doc, button) = axis(Some(1.0));
        let run = run_held(&doc, button, 30.0);
        let p = position(&run);
        assert!((1.0..1.15).contains(&p), "{p}");
    }

    /// L1 -> S1 -> coil KU (up command); L1 -> KR2 (the drive's brake relay contact, written by
    /// the loop) -> coil KB (brake contactor). Motor M1 is an open-loop drive hoist on KU and KB.
    fn drive_hoist(brake_wired: bool) -> (Document, SymbolId) {
        let mut doc = Document::default();
        let (button, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let dev = |d, main| SymbolRole::Device { device: d, main };
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let ku = tx.add_device(DeviceTag::product("KU"), "")?;
                let kb = tx.add_device(DeviceTag::product("KB"), "")?;
                let kr2 = tx.add_device(DeviceTag::product("KR2"), "")?;
                let s1 = tx.add_device(DeviceTag::product("S1"), "")?;
                let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
                let feed = |tx: &mut novo_core::Tx<'_>, y, p| {
                    put(
                        tx,
                        page,
                        "t/feed",
                        y,
                        SymbolRole::PotentialFeed { potential: p },
                    )
                };
                let (f1, f2, f3, f4) = (
                    feed(tx, 0.0, l1),
                    feed(tx, 30.0, n),
                    feed(tx, 200.0, l1),
                    feed(tx, 230.0, n),
                );
                let button = put(tx, page, "t/contact/no", 80.0, dev(s1, true));
                let coil_u = put(tx, page, "t/coil", 100.0, dev(ku, true));
                let relay = put(tx, page, "t/contact/no", 280.0, dev(kr2, true));
                let coil_b = put(tx, page, "t/coil", 300.0, dev(kb, true));
                tx.connect(Endpoint::pin(f1, 1), Endpoint::pin(button, 0))?;
                tx.connect(Endpoint::pin(button, 1), Endpoint::pin(coil_u, 0))?;
                tx.connect(Endpoint::pin(coil_u, 1), Endpoint::pin(f2, 1))?;
                tx.connect(Endpoint::pin(f3, 1), Endpoint::pin(relay, 0))?;
                tx.connect(Endpoint::pin(relay, 1), Endpoint::pin(coil_b, 0))?;
                tx.connect(Endpoint::pin(coil_b, 1), Endpoint::pin(f4, 1))?;
                let m = tx.project().device(m1).map(|d| d.id).unwrap();
                tx.set_property(m, "dh_mass_kg", Some(2000.0.into()))?;
                let tag = |t: &str| Some(DeviceTag::product(t).to_string().into());
                tx.set_property(m, "dh_in_up", tag("KU"))?;
                tx.set_property(m, "dh_brake_relay", tag("KR2"))?;
                if brake_wired {
                    tx.set_property(m, "dh_brake_feed", tag("KB"))?;
                }
                Ok(button)
            })
            .unwrap();
        (doc, button)
    }

    fn run_hoist(doc: &Document, button: SymbolId, seconds: f64) -> DriveRun {
        let project = doc.project();
        let (hoists, notes) = drive_hoists_of(project);
        assert!(
            notes.iter().all(|(l, _)| matches!(l, Level::Info)),
            "{notes:?}"
        );
        assert_eq!(hoists.len(), 1);
        let mut hl = DriveHoistLoop::new(StepConfig::default()).unwrap();
        hl.modules = hoists;
        let mut run = DriveRun {
            active: Some(Active {
                lp: TravelLoop::new(StepConfig::default()).unwrap(),
                hl,
                cw: ClWinchLoop::new(StepConfig::default()).unwrap(),
                acc: 0.0,
            }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        live.poles.set_all(project, button, true);
        for _ in 0..(seconds / 0.016) as usize {
            advance(&mut run, &mut live, project, 0.016);
        }
        run
    }

    #[test]
    fn the_drives_brake_relay_in_the_drawing_opens_the_brake_and_the_hook_rises() {
        let (doc, button) = drive_hoist(true);
        let run = run_hoist(&doc, button, 12.0);
        let hl = &run.active.as_ref().unwrap().hl;
        let m = &hl.modules[0];
        assert!(m.height_m() > 0.1, "{}", m.height_m());
        assert!(hoist_reading(m).contains("running"), "{}", hoist_reading(m));
    }

    #[test]
    fn the_reading_shows_the_heat_and_a_duty_cycle_runs_in_the_background() {
        let (doc, button) = drive_hoist(true);
        let run = run_hoist(&doc, button, 6.0);
        let m = &run.active.as_ref().unwrap().hl.modules[0];
        assert!(
            hoist_reading(m).contains("winding +"),
            "{}",
            hoist_reading(m)
        );
        let hoists = hoist_properties(doc.project());
        assert_eq!(hoists.len(), 1);
        let (tag, props) = hoists[0].clone();
        let cycle = DutyCycle {
            travel_m: 0.5,
            step: 3,
            pause_top_s: 0.5,
            pause_bottom_s: 0.5,
            cycles: 1,
            dt_s: 0.005,
            stop_when_hot: false,
        };
        let job = spawn_duty(tag, props, cycle);
        let report = job
            .rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .expect("the job answers")
            .expect("the cycle runs");
        assert_eq!(report.cycles_done, 1);
        assert_eq!(job.progress.load(Ordering::Relaxed), 1);
        assert!(report.peak_winding_rise_k > 0.0);
    }

    #[test]
    fn a_missing_brake_contactor_wire_is_a_binding_finding_not_a_silent_run() {
        let (doc, _) = drive_hoist(false);
        let (hoists, _) = drive_hoists_of(doc.project());
        let mut hl = DriveHoistLoop::new(StepConfig::default()).unwrap();
        hl.modules = hoists;
        let mut live = LiveView::default();
        let circuit = LiveCircuit::new(&mut live, doc.project());
        let found = hl.check_bindings(&circuit);
        assert!(
            found.iter().any(|f| f.message.contains("ideal wiring")),
            "{found:?}"
        );
    }

    /// L1 -> S1 -> coil KU (up). STOA, STOB (the two STO channels) and ENC (the encoder signal
    /// monitor) are coils fed straight from L1; L1 -> KR (the drive's brake relay contact, written
    /// by the loop) -> coil KB (the brake contactor). M1 is a closed-loop winch on all of them.
    fn closed_winch() -> (Document, SymbolId) {
        let mut doc = Document::default();
        let (button, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let dev = |d, main| SymbolRole::Device { device: d, main };
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let mut devices = std::collections::BTreeMap::new();
                for t in ["KU", "KB", "KR", "S1", "STOA", "STOB", "ENC", "M1"] {
                    devices.insert(t, tx.add_device(DeviceTag::product(t), "")?);
                }
                let feed = |tx: &mut novo_core::Tx<'_>, y, p| {
                    put(
                        tx,
                        page,
                        "t/feed",
                        y,
                        SymbolRole::PotentialFeed { potential: p },
                    )
                };
                let mut y = 0.0;
                let mut rung =
                    |tx: &mut novo_core::Tx<'_>, contact: Option<(&str, &str)>, coil: &str| {
                        let top = feed(tx, y, l1);
                        let bottom = feed(tx, y + 30.0, n);
                        let c = put(tx, page, "t/coil", y + 100.0, dev(devices[coil], true));
                        let first = match contact {
                            Some((key, tag)) => {
                                let k = put(tx, page, key, y + 80.0, dev(devices[tag], true));
                                tx.connect(Endpoint::pin(top, 1), Endpoint::pin(k, 0))?;
                                tx.connect(Endpoint::pin(k, 1), Endpoint::pin(c, 0))?;
                                Some(k)
                            }
                            None => {
                                tx.connect(Endpoint::pin(top, 1), Endpoint::pin(c, 0))?;
                                None
                            }
                        };
                        tx.connect(Endpoint::pin(c, 1), Endpoint::pin(bottom, 1))?;
                        y += 200.0;
                        Ok::<_, novo_core::ModelError>(first)
                    };
                let button = rung(tx, Some(("t/contact/no", "S1")), "KU")?.unwrap();
                rung(tx, None, "STOA")?;
                rung(tx, None, "STOB")?;
                rung(tx, None, "ENC")?;
                rung(tx, Some(("t/contact/no", "KR")), "KB")?;
                let m = tx.project().device(devices["M1"]).map(|d| d.id).unwrap();
                tx.set_property(m, "cw_mass_kg", Some(2500.0.into()))?;
                for (key, t) in [
                    ("cw_in_up", "KU"),
                    ("cw_sto_a", "STOA"),
                    ("cw_sto_b", "STOB"),
                    ("cw_encoder", "ENC"),
                    ("cw_brake_relay", "KR"),
                    ("cw_brake_feed", "KB"),
                ] {
                    tx.set_property(m, key, Some(DeviceTag::product(t).to_string().into()))?;
                }
                Ok(button)
            })
            .unwrap();
        (doc, button)
    }

    fn winch_run(doc: &Document) -> (DriveRun, LiveView) {
        let project = doc.project();
        let (winches, notes) = closed_winches_of(project);
        assert!(
            notes.iter().all(|(l, _)| matches!(l, Level::Info)),
            "{notes:?}"
        );
        assert_eq!(winches.len(), 1);
        let mut cw = ClWinchLoop::new(StepConfig::default()).unwrap();
        cw.modules = winches;
        let run = DriveRun {
            active: Some(Active {
                lp: TravelLoop::new(StepConfig::default()).unwrap(),
                hl: DriveHoistLoop::new(StepConfig::default()).unwrap(),
                cw,
                acc: 0.0,
            }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        (run, live)
    }

    fn seconds(run: &mut DriveRun, live: &mut LiveView, project: &Project, s: f64) {
        for _ in 0..(s / 0.016) as usize {
            advance(run, live, project, 0.016);
        }
    }

    fn winch(run: &DriveRun) -> &ClWinchModule {
        &run.active.as_ref().unwrap().cw.modules[0]
    }

    fn tag_of(t: &str) -> String {
        DeviceTag::product(t).to_string()
    }

    #[test]
    fn the_button_in_the_drawing_lifts_the_closed_loop_winch_through_its_brake_contactor() {
        let (doc, button) = closed_winch();
        let project = doc.project();
        let (mut run, mut live) = winch_run(&doc);
        live.poles.set_all(project, button, true);
        seconds(&mut run, &mut live, project, 8.0);
        let m = winch(&run);
        assert!(m.height_m() > 0.3, "{}", m.height_m());
        assert!(winch_reading(m).contains("running"), "{}", winch_reading(m));
        assert!(m.findings().is_empty(), "{:?}", m.findings());
    }

    #[test]
    fn an_open_sto_channel_coil_trips_the_discrepancy_and_stops_the_winch() {
        let (doc, button) = closed_winch();
        let project = doc.project();
        let (mut run, mut live) = winch_run(&doc);
        live.poles.set_all(project, button, true);
        seconds(&mut run, &mut live, project, 4.0);
        live.defects.inject(novo_sim::inject::Defect::CoilOpen {
            device: tag_of("STOA"),
        });
        seconds(&mut run, &mut live, project, 3.0);
        let m = winch(&run);
        assert!(
            m.findings().iter().any(|f| matches!(
                f.kind,
                novo_machine::cl_winch_loop::ClFindingKind::Trip(
                    novo_drive::closed_loop::ClFault::StoDiscrepancy
                )
            )),
            "{:?}",
            m.findings()
        );
        assert!(m.snapshot().unwrap().shaft_rpm.abs() < 1.0);
    }

    #[test]
    fn a_lost_encoder_signal_device_is_found_and_trips_the_drive() {
        let (doc, button) = closed_winch();
        let project = doc.project();
        let (mut run, mut live) = winch_run(&doc);
        live.poles.set_all(project, button, true);
        seconds(&mut run, &mut live, project, 4.0);
        live.defects.inject(novo_sim::inject::Defect::CoilOpen {
            device: tag_of("ENC"),
        });
        seconds(&mut run, &mut live, project, 3.0);
        let f = winch(&run).findings();
        assert!(
            f.iter().any(|f| matches!(
                f.kind,
                novo_machine::cl_winch_loop::ClFindingKind::EncoderSignalLost
            )),
            "{f:?}"
        );
        assert!(
            f.iter()
                .any(|f| matches!(f.kind, novo_machine::cl_winch_loop::ClFindingKind::Trip(_))),
            "{f:?}"
        );
    }

    #[test]
    fn a_brake_contactor_with_an_open_coil_keeps_the_hook_down() {
        let (doc, button) = closed_winch();
        let project = doc.project();
        let (mut run, mut live) = winch_run(&doc);
        live.defects.inject(novo_sim::inject::Defect::CoilOpen {
            device: tag_of("KB"),
        });
        live.poles.set_all(project, button, true);
        seconds(&mut run, &mut live, project, 6.0);
        assert!(
            winch(&run).height_m().abs() < 0.01,
            "{}",
            winch(&run).height_m()
        );
    }

    #[test]
    fn a_released_button_leaves_the_axis_at_rest() {
        let (doc, _) = axis(None);
        let (modules, _) = axes_of(doc.project());
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].position_m(), 0.0);
    }
}
