//! The fixed-step loop over open-loop drive hoists: a frequency drive with brake logic, an
//! induction motor, a rectifier-fed holding brake and a hanging load (lane L2c-open).
//!
//! One tick, in this order, always the same, so a run is deterministic:
//! 1. limit switches and the drive's brake relay contact are written from the previous tick,
//! 2. the circuit settles by one step,
//! 3. each module reads the operator inputs (up, down, speed steps), its mains, the brake feed
//!    and the brake feedback from the circuit,
//! 4. the brake logic (`novo_drive::hoist_logic`) turns the command and the state of the previous
//!    tick into a run command, a reference frequency and the brake relay,
//! 5. `Drive::step` ramps the frequency under that reference,
//! 6. the brake follows its coil (fed by the circuit through the relay, the brake contactor and
//!    the rectifier), and the motor, the brake and the hanging load advance the shaft.
//!
//! The brake relay of the drive is a contact in the drawing: the module writes its state to the
//! bound device like a limit switch, so the brake contactor, the rectifier and the brake coil
//! are real parts of the circuit (a contactor that does not pull in, or a feed that is not
//! wired, keeps the brake closed and the run shows what the motor does against it).
//!
//! Exact by definition: the order of the tick, the sign convention (positive shaft speed raises
//! the hook), the arithmetic of the hanging load. Model: everything inside `novo_drive` (drive,
//! induction motor, brake, logic), the build-up of the motor flux (one first-order lag with the
//! rotor time constant of the fitted circuit) and the stiction of a brake (the shaft is held
//! while the net torque is inside the friction plus the brake torque).

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::brake::{BrakeState, SpringBrake};
use novo_drive::brake_heat::{BrakeHeat, BrakeHeatParams};
use novo_drive::drive::{Drive, DriveInputs, DriveOutputs, Fault};
use novo_drive::hoist_logic::{
    HoistLogic, LogicEvent, LogicFault, LogicInput, LogicOutput, LogicPhase,
};
use novo_drive::hoist_thermal::{MotorThermal, ThermalParams};
use novo_drive::induction::{InductionMotor, OperatingPoint, Supply};
use novo_drive::mechanics::Hoist;
use novo_drive::motor::Direction;
use novo_drive::rope::{RopeConfig, RopeEvent, RopeLoad};

use crate::cosim::StepConfig;
use crate::hoist_loop::{BindingFinding, LimitSwitch};
use crate::port::CircuitPort;

/// Hook movement below this (m) is not reported as a rollback or a slip.
const REPORT_BELOW_M: f64 = 0.005;

/// A start rollback beyond this (m) is reported at once: the load is running away.
const RUNAWAY_M: f64 = 0.05;

/// Where a drive hoist is attached to the schematic.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DriveHoistBinding {
    /// Device tag whose energised state is the "up" command.
    pub up: Option<String>,
    /// Device tag whose energised state is the "down" command.
    pub down: Option<String>,
    /// Device tags of the speed inputs: entry `k` selects speed step `k + 1` when energised.
    pub speed_inputs: Vec<Option<String>>,
    /// Device tag of the main contactor coil that feeds the drive. `None` means mains is always on.
    pub mains: Option<String>,
    /// Device tag of the drive's brake relay contact. The module writes its state; the drawing
    /// connects it to the brake contactor.
    pub brake_relay: Option<String>,
    /// Device tag whose energised state feeds the brake coil (brake contactor, rectifier).
    /// `None`: the brake follows the relay directly (an ideal wiring).
    pub brake_feed: Option<String>,
    /// Device tag whose energised state confirms an open brake to the drive. `None`: no check.
    pub brake_feedback: Option<String>,
    /// Device tag whose energised state resets a tripped drive (rising edge).
    pub reset: Option<String>,
    /// Upper limit protects the top, lower limit the bottom, by hook height.
    pub limits: Vec<LimitSwitch>,
}

/// Something the run found, with the time it happened.
#[derive(Debug, Clone, PartialEq)]
pub struct HoistFinding {
    pub time_s: f64,
    pub kind: FindingKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FindingKind {
    /// The hook went against the command (`metres`, positive) between the brake opening and the
    /// motor taking the load: the classic drop at start.
    StartRollback { metres: f64 },
    /// Net hook movement from the brake command to standstill (positive = up).
    StopTravel { metres: f64 },
    /// The hook moved while the brake was commanded closed and the drive off: the brake does not
    /// hold the load.
    BrakeSlip { metres: f64 },
    /// The hook sank below where it was when the direction changed, while the drive passed
    /// through zero frequency with the brake open (reversal with the brake kept open).
    ReversalSag { metres: f64 },
    /// The rope was slack by up to `metres` before it took the load again: the hook was resting
    /// on the floor while the drum paid out rope.
    SlackRope { metres: f64 },
    /// The hook came down on the floor at this speed (m/s).
    FloorLanding { speed_m_s: f64 },
    /// The load left the floor: the highest rope tension in the half second after was `newtons`,
    /// `factor` times the weight.
    LoadSnatch { newtons: f64, factor: f64 },
    /// The motor winding reached its limit rise; `cool_down_s` is the time at standstill until it
    /// is back to 80 % of the limit.
    MotorTooHot {
        rise_k: f64,
        limit_k: f64,
        cool_down_s: f64,
    },
    /// The brake reached its limit rise from slipping (late engagement, closing before the hook
    /// has stopped).
    BrakeHot { rise_k: f64, limit_k: f64 },
    /// The brake lining used this share of its life (reported at 50 % and 100 %).
    BrakeWear { share: f64 },
    /// The drive tripped.
    DriveTrip(Fault),
    /// The brake logic found something.
    Logic(LogicEvent),
}

impl FindingKind {
    /// One line for the window.
    pub fn text(&self) -> String {
        match self {
            Self::StartRollback { metres } => format!(
                "The hook went {:.0} mm against the command after the brake opened: \
                 the motor had not taken the load yet.",
                metres * 1000.0
            ),
            Self::StopTravel { metres } => format!(
                "The hook moved {:.0} mm {} between the brake command and standstill.",
                metres.abs() * 1000.0,
                if *metres >= 0.0 { "up" } else { "down" }
            ),
            Self::ReversalSag { metres } => format!(
                "The hook sank {:.0} mm while the drive passed through zero frequency with the \
                 brake open.",
                metres * 1000.0
            ),
            Self::SlackRope { metres } => format!(
                "The rope went slack by up to {:.0} mm: the hook rested on the floor while the \
                 drum paid out rope.",
                metres * 1000.0
            ),
            Self::FloorLanding { speed_m_s } => {
                format!("The hook came down on the floor at {:.2} m/s.", speed_m_s)
            }
            Self::LoadSnatch { newtons, factor } => format!(
                "The load left the floor: the rope tension peaked at {:.0} N, {:.2} times the weight.",
                newtons, factor
            ),
            Self::MotorTooHot {
                rise_k,
                limit_k,
                cool_down_s,
            } => format!(
                "The motor winding is {rise_k:.0} K over ambient (limit {limit_k:.0} K). \
                 At standstill it needs about {:.0} min to cool to 80 % of the limit.",
                cool_down_s / 60.0
            ),
            Self::BrakeHot { rise_k, limit_k } => format!(
                "The brake is {rise_k:.0} K over ambient (limit {limit_k:.0} K) from slipping."
            ),
            Self::BrakeWear { share } => format!(
                "The brake lining has used {:.0} % of its life by slipping.",
                share * 100.0
            ),
            Self::BrakeSlip { metres } => format!(
                "The brake does not hold the load: the hook moved {:.0} mm with the drive off.",
                metres * 1000.0
            ),
            Self::DriveTrip(f) => format!("The drive tripped: {f:?}."),
            Self::Logic(LogicEvent::LoadSensed {
                fraction,
                applied_hz,
                limited_by_motor,
            }) => format!(
                "Load sensed at {:.0} % of rated motor torque: top step {:.1} Hz{}.",
                fraction * 100.0,
                applied_hz,
                if *limited_by_motor {
                    " (limited by what the motor can deliver)"
                } else {
                    ""
                }
            ),
            Self::Logic(LogicEvent::ReversalStarted | LogicEvent::ReversalDone) => {
                "Reversal with the brake open.".to_owned()
            }
            Self::Logic(LogicEvent::Fault(LogicFault::NoTorqueAtRelease)) => {
                "The release current was not reached: the brake stays closed.".to_owned()
            }
            Self::Logic(LogicEvent::Fault(LogicFault::BrakeFeedback)) => {
                "The brake feedback did not confirm an open brake in time.".to_owned()
            }
            Self::Logic(LogicEvent::Fault(LogicFault::DriveOrMains)) => {
                "The drive tripped or the mains went away: the brake was closed at once.".to_owned()
            }
        }
    }
}

/// What a module did in the last tick.
#[derive(Debug, Clone, PartialEq)]
pub struct DriveHoistSnapshot {
    pub drive: DriveOutputs,
    pub motor: OperatingPoint,
    pub logic: LogicOutput,
    pub brake_state: BrakeState,
    pub brake_torque_nm: f64,
    /// Hook height in metres, 0 at the start (positive is up).
    pub height_m: f64,
    /// Hook speed in metres per second (positive is up).
    pub speed_m_s: f64,
    /// Motor flux per unit (the build-up after the output goes on).
    pub flux: f64,
    /// Top speed step in use, Hz (nominal or sensed).
    pub top_step_hz: f64,
    /// The hook and the rope, when the module has a floor.
    pub rope: Option<RopeSnapshot>,
    /// Winding and brake heat, when the module has a heat model.
    pub heat: Option<HeatSnapshot>,
}

/// The hook on its rope, in the last tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RopeSnapshot {
    /// Height of the hook itself (the snapshot's `height_m` is the drum position).
    pub hook_m: f64,
    pub tension_n: f64,
    /// Rope tension as a share of the weight.
    pub load_fraction: f64,
    pub slack_m: f64,
    pub on_floor: bool,
}

/// Heat and wear in the last tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatSnapshot {
    pub winding_rise_k: f64,
    pub frame_rise_k: f64,
    pub brake_rise_k: f64,
    pub brake_energy_j: f64,
    /// Share of the brake lining life used.
    pub brake_wear: f64,
}

/// The heat model of a drive hoist: motor winding and frame, brake disc and lining.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatConfig {
    pub motor: ThermalParams,
    /// Motor losses at the rated point, W (sets the thermal conductances).
    pub rated_losses_w: f64,
    /// Rated shaft speed, rad/s (the fan cools in proportion to the speed over it).
    pub rated_omega_rad_s: f64,
    pub brake: BrakeHeatParams,
}

#[derive(Debug)]
struct HeatState {
    motor: MotorThermal,
    brake: BrakeHeat,
    rated_omega: f64,
    hot: bool,
    brake_hot: bool,
    wear_marks: u8,
}

/// An open-loop drive hoist: drive, brake logic, motor, brake, load and bindings.
#[derive(Debug)]
pub struct DriveHoistModule {
    pub id: String,
    pub drive: Drive,
    pub logic: HoistLogic,
    pub motor: InductionMotor,
    pub hoist: Hoist,
    pub brake: SpringBrake,
    pub binding: DriveHoistBinding,
    inertia_kg_m2: f64,
    rotor_inertia_kg_m2: f64,
    /// The hook on an elastic rope above a floor; `None`: a rigid rope that is always taut.
    rope: Option<RopeLoad>,
    heat: Option<HeatState>,
    /// Rotor time constant in s: how fast the flux builds.
    tau_flux_s: f64,
    flux: f64,
    omega: f64,
    height_m: f64,
    feedback: (f64, f64),
    relay: bool,
    last_reset: bool,
    last_fault: Option<Fault>,
    snapshot: Option<DriveHoistSnapshot>,
    findings: Vec<HoistFinding>,
    fault: Option<String>,
    // Trackers for the findings.
    start_mark: Option<(f64, bool)>,
    start_worst_m: f64,
    start_reported: bool,
    stop_mark: Option<f64>,
    relay_was_on: bool,
    idle_baseline: Option<f64>,
    slip_reported: bool,
    /// Hook height at the start of a reversal and the lowest height since.
    reversal_mark: Option<(f64, f64)>,
    base_ramp_s: (f64, f64),
}

/// The parts of a drive hoist, before they are bound to a schematic.
#[derive(Debug)]
pub struct DriveHoistParts {
    pub drive: Drive,
    pub logic: HoistLogic,
    pub motor: InductionMotor,
    pub hoist: Hoist,
    pub brake: SpringBrake,
    /// The motor's own inertia; the load is reflected from the hoist.
    pub rotor_inertia_kg_m2: f64,
}

impl DriveHoistModule {
    pub fn new(
        id: impl Into<String>,
        parts: DriveHoistParts,
        binding: DriveHoistBinding,
    ) -> Result<Self, InputError> {
        let DriveHoistParts {
            drive,
            logic,
            motor,
            hoist,
            brake,
            rotor_inertia_kg_m2,
        } = parts;
        if !(rotor_inertia_kg_m2.is_finite() && rotor_inertia_kg_m2 >= 0.0) {
            return Err(InputError::NotFinite {
                field: "rotor_inertia_kg_m2",
            });
        }
        let inertia = rotor_inertia_kg_m2 + hoist.reflected_inertia_kg_m2();
        if !(inertia.is_finite() && inertia > 0.0) {
            return Err(InputError::NotPositive {
                field: "inertia_kg_m2",
                value: inertia,
            });
        }
        let base_ramp_s = (drive.cfg.accel_time_s, drive.cfg.decel_time_s);
        let c = motor.circuit();
        let tau = ((c.xm + c.xr) / (TAU * motor.rated_hz() * c.rr)).clamp(0.02, 5.0);
        Ok(Self {
            id: id.into(),
            drive,
            logic,
            motor,
            hoist,
            brake,
            binding,
            inertia_kg_m2: inertia,
            rotor_inertia_kg_m2,
            rope: None,
            heat: None,
            tau_flux_s: tau,
            flux: 0.0,
            omega: 0.0,
            height_m: 0.0,
            feedback: (0.0, 0.0),
            relay: false,
            last_reset: false,
            last_fault: None,
            snapshot: None,
            findings: Vec::new(),
            fault: None,
            start_mark: None,
            start_worst_m: 0.0,
            start_reported: false,
            stop_mark: None,
            relay_was_on: false,
            idle_baseline: Some(0.0),
            slip_reported: false,
            reversal_mark: None,
            base_ramp_s,
        })
    }

    /// Puts the hook on an elastic rope above a floor. The shaft then carries only the rotor and
    /// the gear (the hook is a separate mass on the rope), so the rotor inertia must be set.
    pub fn with_rope(mut self, cfg: RopeConfig) -> Result<Self, InputError> {
        if self.rotor_inertia_kg_m2 <= 0.0 {
            return Err(InputError::NotPositive {
                field: "rotor_inertia_kg_m2 (needed with a floor)",
                value: self.rotor_inertia_kg_m2,
            });
        }
        self.rope = Some(RopeLoad::new(cfg, self.hoist.mass_kg())?);
        self.inertia_kg_m2 = self.rotor_inertia_kg_m2;
        Ok(self)
    }

    /// The hook and the rope, when the module has a floor.
    pub fn rope(&self) -> Option<&RopeLoad> {
        self.rope.as_ref()
    }

    /// Limit rises `(winding K, brake K)` of the heat model, when the module has one.
    pub fn heat_limits(&self) -> Option<(f64, f64)> {
        self.heat
            .as_ref()
            .map(|h| (h.motor.params().limit_rise_k, h.brake.params().limit_rise_k))
    }

    /// Adds the heat model of the motor winding and the brake.
    pub fn with_heat(mut self, cfg: HeatConfig) -> Result<Self, InputError> {
        if cfg.rated_omega_rad_s <= 0.0 {
            return Err(InputError::NotPositive {
                field: "rated_omega_rad_s",
                value: cfg.rated_omega_rad_s,
            });
        }
        self.heat = Some(HeatState {
            motor: MotorThermal::new(cfg.motor, cfg.rated_losses_w)?,
            brake: BrakeHeat::new(cfg.brake)?,
            rated_omega: cfg.rated_omega_rad_s,
            hot: false,
            brake_hot: false,
            wear_marks: 0,
        });
        Ok(self)
    }

    /// Hook height in metres, 0 at the start (positive is up).
    pub fn height_m(&self) -> f64 {
        self.height_m
    }

    /// Motor speed in rpm (signed, positive raises).
    pub fn rpm(&self) -> f64 {
        self.omega * 60.0 / TAU
    }

    /// Rotor time constant used for the flux build-up, s.
    pub fn flux_time_constant_s(&self) -> f64 {
        self.tau_flux_s
    }

    pub fn snapshot(&self) -> Option<&DriveHoistSnapshot> {
        self.snapshot.as_ref()
    }

    /// What the run found so far, oldest first.
    pub fn findings(&self) -> &[HoistFinding] {
        &self.findings
    }

    /// Why this module stopped, if it did: the engine refused a number in the run.
    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    fn note(&mut self, time_s: f64, kind: FindingKind) {
        self.findings.push(HoistFinding { time_s, kind });
    }
}

/// The loop over any number of drive hoists.
pub struct DriveHoistLoop {
    pub config: StepConfig,
    pub modules: Vec<DriveHoistModule>,
    time_s: f64,
}

impl DriveHoistLoop {
    pub fn new(config: StepConfig) -> Result<Self, InputError> {
        config.validate()?;
        Ok(Self {
            config,
            modules: Vec::new(),
            time_s: 0.0,
        })
    }

    pub fn time_s(&self) -> f64 {
        self.time_s
    }

    pub fn module(&self, id: &str) -> Option<&DriveHoistModule> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Binding problems to show before the run.
    pub fn check_bindings(&self, circuit: &impl CircuitPort) -> Vec<BindingFinding> {
        let mut out = Vec::new();
        let finding = |module: &str, message: String| BindingFinding {
            module: module.to_owned(),
            message,
        };
        for m in &self.modules {
            let b = &m.binding;
            for (what, tag) in [
                ("up input", &b.up),
                ("down input", &b.down),
                ("main contactor", &b.mains),
                ("brake relay contact", &b.brake_relay),
                ("brake feed", &b.brake_feed),
                ("brake feedback", &b.brake_feedback),
                ("reset input", &b.reset),
            ] {
                if let Some(tag) = tag
                    && !circuit.knows_device(tag)
                {
                    out.push(finding(
                        &m.id,
                        format!("{what} {tag} is not in the circuit"),
                    ));
                }
            }
            for tag in b.speed_inputs.iter().flatten() {
                if !circuit.knows_device(tag) {
                    out.push(finding(
                        &m.id,
                        format!("speed input {tag} is not in the circuit"),
                    ));
                }
            }
            for limit in &b.limits {
                if !circuit.knows_device(&limit.binding.device_tag) {
                    out.push(finding(
                        &m.id,
                        format!(
                            "limit switch {} is not in the circuit",
                            limit.binding.device_tag
                        ),
                    ));
                }
            }
            if b.up.is_none() && b.down.is_none() {
                out.push(finding(
                    &m.id,
                    "no up or down input is bound: the hoist cannot be commanded".into(),
                ));
            }
            if b.brake_relay.is_none() && b.brake_feed.is_some() {
                out.push(finding(
                    &m.id,
                    "the brake is fed from the circuit but the drive's brake relay contact is \
                     not bound, so the brake is never commanded open"
                        .into(),
                ));
            }
            if b.brake_feed.is_none() {
                out.push(finding(
                    &m.id,
                    "no brake feed device is bound: the brake follows the drive's relay \
                     directly (ideal wiring, the brake contactor and rectifier are not checked)"
                        .into(),
                ));
            }
            for problem in m.drive.config_problems() {
                out.push(finding(&m.id, problem.to_owned()));
            }
        }
        out
    }

    /// One tick.
    pub fn step(&mut self, circuit: &mut impl CircuitPort) {
        let dt = self.config.dt_s;
        for m in &self.modules {
            for limit in &m.binding.limits {
                circuit.set_contact(&limit.binding.device_tag, limit.closed_at(m.height_m));
            }
            if let Some(tag) = &m.binding.brake_relay {
                circuit.set_contact(tag, m.relay);
            }
        }
        circuit.settle(dt);
        let time_s = self.time_s;
        for m in &mut self.modules {
            if m.fault.is_some() {
                continue;
            }
            if let Err(why) = step_module(m, circuit, dt, time_s) {
                m.fault = Some(format!("the run stopped: {why}"));
            }
        }
        self.time_s += dt;
    }

    /// Runs `ticks` ticks.
    pub fn run(&mut self, circuit: &mut impl CircuitPort, ticks: usize) {
        for _ in 0..ticks {
            self.step(circuit);
        }
    }
}

fn energised(circuit: &impl CircuitPort, tag: &Option<String>) -> bool {
    tag.as_deref().is_some_and(|t| circuit.is_energised(t))
}

fn step_module(
    m: &mut DriveHoistModule,
    circuit: &impl CircuitPort,
    dt: f64,
    time_s: f64,
) -> Result<(), InputError> {
    let up = energised(circuit, &m.binding.up);
    let down = energised(circuit, &m.binding.down);
    let step = m
        .binding
        .speed_inputs
        .iter()
        .enumerate()
        .filter(|(_, t)| t.as_deref().is_some_and(|t| circuit.is_energised(t)))
        .map(|(k, _)| k + 1)
        .max()
        .unwrap_or(0);
    let mains_ok = m
        .binding
        .mains
        .as_deref()
        .is_none_or(|tag| circuit.is_energised(tag));
    let reset = energised(circuit, &m.binding.reset);
    let reset_edge = reset && !m.last_reset;
    m.last_reset = reset;
    let brake_feedback = m
        .binding
        .brake_feedback
        .as_deref()
        .map(|tag| circuit.is_energised(tag));

    // Brake logic, from the state of the previous tick.
    let prev = m.snapshot.as_ref();
    let (prev_freq, prev_ok) = prev.map_or((0.0, true), |s| {
        (
            s.drive.frequency_hz.abs(),
            s.drive.fault.is_none() && s.drive.state != novo_drive::drive::DriveState::NotReady,
        )
    });
    let logic_in = LogicInput {
        up,
        down,
        step,
        drive_ok: prev_ok && mains_ok,
        mains_ok,
        frequency_hz: prev_freq,
        motor_current_a: m.feedback.0,
        motor_torque_nm: prev.map_or(0.0, |s| s.motor.torque_nm),
        brake_feedback,
    };
    // Largest shaft torque the motor can deliver at a frequency on the drive's V/f law (model:
    // the breakdown torque of the fitted circuit); load sensing keeps a margin below it.
    let motor = m.motor;
    let vf = m.drive.cfg.vf;
    let cap = move |f: f64| {
        let volts = vf.volts_for_frequency(f);
        Supply::new(f, volts, Direction::Positive)
            .map(|s| motor.breakdown(&s).motoring_nm)
            .unwrap_or(0.0)
    };
    let logic = m.logic.step(&logic_in, &cap, dt);
    for e in &logic.events {
        match e {
            // A fault from a drive trip is already reported as the trip itself.
            LogicEvent::Fault(LogicFault::DriveOrMains) => {}
            // A reversal is reported by what it did to the hook, not by its start.
            LogicEvent::ReversalStarted => m.reversal_mark = Some((m.height_m, m.height_m)),
            LogicEvent::ReversalDone => {
                if let Some((h0, low)) = m.reversal_mark.take()
                    && h0 - low > REPORT_BELOW_M
                {
                    m.note(time_s, FindingKind::ReversalSag { metres: h0 - low });
                }
            }
            _ => m.note(time_s, FindingKind::Logic(*e)),
        }
    }
    if let Some((_, low)) = m.reversal_mark.as_mut() {
        *low = low.min(m.height_m);
    }
    // The fast band of a reversal uses its own ramp time.
    match logic.ramp_time_s {
        Some(t) => {
            m.drive.cfg.accel_time_s = t;
            m.drive.cfg.decel_time_s = t;
        }
        None => {
            m.drive.cfg.accel_time_s = m.base_ramp_s.0;
            m.drive.cfg.decel_time_s = m.base_ramp_s.1;
        }
    }

    // Drive: the logic gives the run command and the reference.
    let mut di = vec![logic.run_up, logic.run_down];
    di.push(reset_edge);
    let inputs = DriveInputs {
        di,
        word_hz: logic.reference_hz,
        mains_ok,
        motor_current_a: m.feedback.0,
        motor_power_w: m.feedback.1,
        ..DriveInputs::default()
    };
    let out = m.drive.step(&inputs, dt);
    if out.fault != m.last_fault {
        if let Some(f) = out.fault {
            m.note(time_s, FindingKind::DriveTrip(f));
        }
        m.last_fault = out.fault;
    }

    // Brake coil: fed through the circuit, or directly by the relay without a feed device.
    m.relay = logic.brake_relay;
    let coil_fed = match &m.binding.brake_feed {
        Some(tag) => circuit.is_energised(tag),
        None => logic.brake_relay,
    };
    let brake_t = m.brake.holding_torque_nm();
    m.brake.step(coil_fed, dt);

    // Flux builds with the rotor time constant while the output is on.
    let on = out.running && out.voltage_v > 0.0;
    let target = if on { 1.0 } else { 0.0 };
    m.flux += (target - m.flux) * (1.0 - (-dt / m.tau_flux_s).exp());

    let direction = if out.frequency_hz < 0.0 {
        Direction::Negative
    } else {
        Direction::Positive
    };
    let volts = if on { out.voltage_v * m.flux } else { 0.0 };
    let supply = Supply::new(out.frequency_hz.abs(), volts.max(0.0), direction)?;
    let load_scale = m
        .rope
        .as_ref()
        .map_or(1.0, |r| r.load_fraction().clamp(0.0, 4.0));
    let (point, omega_end) = advance_shaft(m, &supply, brake_t, dt, load_scale)?;
    let omega_start = m.omega;
    m.omega = omega_end;
    m.feedback = (point.stator_current_a, point.input_power_w);
    let metres_per_rad = m.hoist.metres_per_rad();
    m.height_m += 0.5 * (omega_start + omega_end) * dt * metres_per_rad;
    let speed = omega_end * metres_per_rad;
    if let Some(rope) = m.rope.as_mut() {
        for e in rope.step(m.height_m, dt) {
            let kind = match e {
                RopeEvent::Landed { speed_m_s } => FindingKind::FloorLanding { speed_m_s },
                RopeEvent::SlackTaken { slack_m } => FindingKind::SlackRope { metres: slack_m },
                RopeEvent::TensionPeak { newtons, factor } => {
                    FindingKind::LoadSnatch { newtons, factor }
                }
                RopeEvent::LiftOff { .. } => continue,
            };
            m.findings.push(HoistFinding { time_s, kind });
        }
    }

    track_findings(m, &logic, time_s, speed);
    track_heat(
        m,
        &point,
        brake_t,
        0.5 * (omega_start + omega_end),
        time_s,
        dt,
    );

    m.snapshot = Some(DriveHoistSnapshot {
        drive: out,
        motor: point,
        brake_state: m.brake.state(),
        brake_torque_nm: brake_t,
        height_m: m.height_m,
        speed_m_s: speed,
        flux: m.flux,
        top_step_hz: m.logic.top_step_hz(),
        rope: m.rope.as_ref().map(|r| RopeSnapshot {
            hook_m: r.hook_m(),
            tension_n: r.tension_n(),
            load_fraction: r.load_fraction(),
            slack_m: r.slack_m(),
            on_floor: r.on_floor(),
        }),
        heat: m.heat.as_ref().map(|h| HeatSnapshot {
            winding_rise_k: h.motor.winding_rise_k(),
            frame_rise_k: h.motor.frame_rise_k(),
            brake_rise_k: h.brake.rise_k(),
            brake_energy_j: h.brake.energy_j(),
            brake_wear: h.brake.wear_share(),
        }),
        logic,
    });
    Ok(())
}

/// Advances the shaft one step under the motor, the weight, the losses and the brake. Returns
/// the operating point and the shaft speed at the end of the step.
fn advance_shaft(
    m: &mut DriveHoistModule,
    supply: &Supply,
    brake_t: f64,
    dt: f64,
    load_scale: f64,
) -> Result<(OperatingPoint, f64), InputError> {
    // `load_scale` is the share of the weight the drum carries (1 with a rigid rope).
    let weight = -m.hoist.static_torque_nm() * load_scale;
    let fric = |s: f64| {
        load_scale
            * if s > 0.0 {
                m.hoist.friction_up_nm()
            } else {
                m.hoist.friction_down_nm()
            }
    };
    let mut omega = m.omega;
    let sign = if omega != 0.0 {
        omega.signum()
    } else {
        // At rest the shaft stays put while the net torque is inside friction plus brake.
        let p0 = m.motor.evaluate(supply, 0.0)?;
        let net = p0.torque_nm + weight;
        if net > fric(1.0) + brake_t {
            1.0
        } else if net < -(fric(-1.0) + brake_t) {
            -1.0
        } else {
            return Ok((p0, 0.0));
        }
    };
    let resist = fric(sign) + brake_t;
    let step = m
        .motor
        .step_shaft(&mut omega, supply, m.inertia_kg_m2, dt, |_| {
            weight - sign * resist
        })?;
    if omega * sign <= 0.0 {
        // The resisting torque stopped the shaft inside this step.
        omega = 0.0;
    }
    Ok((step.point, omega))
}

/// Time at standstill until the winding is back to 80 % of its limit, s (capped at 6 h).
fn cool_down_time_s(motor: &MotorThermal) -> f64 {
    let mut cold = *motor;
    let target = 0.8 * cold.params().limit_rise_k;
    let mut t = 0.0;
    while cold.winding_rise_k() > target && t < 6.0 * 3600.0 {
        cold.step(0.0, 0.0, 10.0);
        t += 10.0;
    }
    t
}

fn track_heat(
    m: &mut DriveHoistModule,
    point: &OperatingPoint,
    brake_t: f64,
    omega_mean: f64,
    time_s: f64,
    dt: f64,
) {
    let Some(h) = m.heat.as_mut() else {
        return;
    };
    h.motor
        .step(point.losses_w(), omega_mean.abs() / h.rated_omega, dt);
    h.brake.step(brake_t, omega_mean, dt);
    let mut found = Vec::new();
    let limit = h.motor.params().limit_rise_k;
    if h.motor.too_hot() && !h.hot {
        h.hot = true;
        found.push(FindingKind::MotorTooHot {
            rise_k: h.motor.winding_rise_k(),
            limit_k: limit,
            cool_down_s: cool_down_time_s(&h.motor),
        });
    } else if h.hot && h.motor.winding_rise_k() < 0.9 * limit {
        h.hot = false;
    }
    let brake_limit = h.brake.params().limit_rise_k;
    if h.brake.too_hot() && !h.brake_hot {
        h.brake_hot = true;
        found.push(FindingKind::BrakeHot {
            rise_k: h.brake.rise_k(),
            limit_k: brake_limit,
        });
    } else if h.brake_hot && h.brake.rise_k() < 0.9 * brake_limit {
        h.brake_hot = false;
    }
    let share = h.brake.wear_share();
    if (share >= 0.5 && h.wear_marks == 0) || (share >= 1.0 && h.wear_marks == 1) {
        h.wear_marks += 1;
        found.push(FindingKind::BrakeWear { share });
    }
    for kind in found {
        m.note(time_s, kind);
    }
}

fn track_findings(m: &mut DriveHoistModule, logic: &LogicOutput, time_s: f64, speed: f64) {
    let relay_rose = logic.brake_relay && m.start_mark.is_none() && m.stop_mark.is_none();
    let phase = logic.phase;
    // Start: from the moment the relay closes until the hook moves with the command.
    if relay_rose && matches!(phase, LogicPhase::Releasing) {
        m.start_mark = Some((m.height_m, logic.run_up));
        m.start_worst_m = 0.0;
        m.start_reported = false;
        m.idle_baseline = None;
        m.slip_reported = false;
    }
    if let Some((h0, up)) = m.start_mark {
        let against = if up { h0 - m.height_m } else { m.height_m - h0 };
        m.start_worst_m = m.start_worst_m.max(against);
        // A load that keeps sinking never "settles": it is reported once it is clearly running
        // away, and the first number shown is the distance at that moment.
        if m.start_worst_m > RUNAWAY_M && !m.start_reported {
            let metres = m.start_worst_m;
            m.start_reported = true;
            m.note(time_s, FindingKind::StartRollback { metres });
        }
        let settled = -against > 0.02 || !logic.brake_relay;
        if settled {
            if m.start_worst_m > REPORT_BELOW_M && !m.start_reported {
                let metres = m.start_worst_m;
                m.note(time_s, FindingKind::StartRollback { metres });
            }
            m.start_mark = None;
        }
    }
    // Stop: from the relay opening (or any loss of the relay) until the shaft is at rest and
    // the drive is off.
    if !logic.brake_relay && m.relay_was_on {
        m.stop_mark = Some(m.height_m);
    }
    m.relay_was_on = logic.brake_relay;
    if let Some(h1) = m.stop_mark
        && m.omega == 0.0
        && speed == 0.0
        && !logic.run_up
        && !logic.run_down
    {
        let metres = m.height_m - h1;
        if metres.abs() > REPORT_BELOW_M {
            m.note(time_s, FindingKind::StopTravel { metres });
        }
        m.stop_mark = None;
        m.idle_baseline = Some(m.height_m);
    }
    // Slip with the drive off and the brake commanded closed.
    if matches!(phase, LogicPhase::Idle)
        && let Some(base) = m.idle_baseline
        && !m.slip_reported
        && (m.height_m - base).abs() > 0.02
        && m.stop_mark.is_none()
    {
        let metres = (m.height_m - base).abs();
        m.slip_reported = true;
        m.note(time_s, FindingKind::BrakeSlip { metres });
    }
    if !matches!(phase, LogicPhase::Idle) {
        m.slip_reported = false;
    }
}
