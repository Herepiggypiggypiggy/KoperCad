//! Brake logic and load sensing of an open-loop frequency-drive hoist (lane L2c-open).
//!
//! The drive runs the motor without an encoder. Between the operator's command and the drive
//! sits a sequence that protects the load: magnetise the motor at a low frequency, open the
//! brake only when the motor current says there is torque, wait for the brake to be open,
//! run, and on stop come down to a low frequency, close the brake and keep the output on
//! until the brake holds. The sequence is generic (public knowledge of how open-loop hoist
//! drives behave); every number is an entered parameter, never a vendor's value.
//!
//! The optional load sensing measures the motor torque at a probe frequency and allows the
//! top speed step to go above its nominal frequency when the load is light, within what the
//! motor can still deliver (a margin over the sensed torque). The name and the exact rule of
//! a manufacturer's function are not reproduced.
//!
//! Exact by definition: the order of the states. Model: all timing, thresholds and the sensed
//! torque (one averaged value, no dynamics).

use crate::error::{self, InputError};

/// Fidelity of the whole sequence: a generic model, not any maker's firmware.
pub const LOGIC_FIDELITY: crate::Fidelity = crate::Fidelity::Model;

/// Brake sequence parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct BrakeLogic {
    /// Frequency the motor is magnetised at before the brake opens, Hz.
    pub release_hz: f64,
    /// Motor current that must be reached before the brake opens, going up, A (0 = no check).
    pub release_current_up_a: f64,
    /// The same going down, A (0 = no check).
    pub release_current_down_a: f64,
    /// Wait after the brake command before the run goes on, s.
    pub release_time_s: f64,
    /// Frequency the drive comes down to before the brake closes, Hz.
    pub engage_hz: f64,
    /// Time the output stays on after the brake command to close it, s.
    pub engage_time_s: f64,
    /// Shortest time between a stop and the next start, s.
    pub restart_time_s: f64,
    /// Longest wait for the release current before the drive gives up, s.
    pub release_timeout_s: f64,
    /// Longest wait for the brake feedback after the release time, s.
    pub feedback_timeout_s: f64,
}

impl BrakeLogic {
    pub fn validate(&self) -> Result<(), InputError> {
        error::non_negative("release_hz", self.release_hz)?;
        error::non_negative("release_current_up_a", self.release_current_up_a)?;
        error::non_negative("release_current_down_a", self.release_current_down_a)?;
        error::non_negative("release_time_s", self.release_time_s)?;
        error::non_negative("engage_hz", self.engage_hz)?;
        error::non_negative("engage_time_s", self.engage_time_s)?;
        error::non_negative("restart_time_s", self.restart_time_s)?;
        error::positive("release_timeout_s", self.release_timeout_s)?;
        error::positive("feedback_timeout_s", self.feedback_timeout_s)?;
        Ok(())
    }

    /// A conservative starting point: 3 Hz magnetising, brake opens at 40 % of the entered
    /// rated current, 0.3 s release and 0.4 s engage time. Meant to be edited.
    pub fn generic(motor_rated_a: f64) -> Self {
        Self {
            release_hz: 3.0,
            release_current_up_a: 0.4 * motor_rated_a,
            release_current_down_a: 0.4 * motor_rated_a,
            release_time_s: 0.3,
            engage_hz: 2.0,
            engage_time_s: 0.4,
            restart_time_s: 0.5,
            release_timeout_s: 2.0,
            feedback_timeout_s: 1.0,
        }
    }
}

/// Load sensing: the top step may run faster than nominal when the load is light.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadSense {
    /// Frequency at which the torque is measured, Hz.
    pub probe_hz: f64,
    /// Averaging time at the probe frequency, s.
    pub probe_time_s: f64,
    /// Load, as a fraction of the rated motor torque, at or below which the full boost is allowed.
    pub light_fraction: f64,
    /// Highest frequency the top step may reach, Hz.
    pub boost_hz: f64,
    /// The motor must be able to deliver this many times the sensed torque at the boosted frequency.
    pub margin: f64,
}

impl LoadSense {
    pub fn validate(&self) -> Result<(), InputError> {
        error::positive("probe_hz", self.probe_hz)?;
        error::positive("probe_time_s", self.probe_time_s)?;
        error::positive("light_fraction", self.light_fraction)?;
        error::positive("boost_hz", self.boost_hz)?;
        error::positive("margin", self.margin)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoistLogicConfig {
    pub brake: BrakeLogic,
    /// Frequency of each speed step, slowest first (one to four steps). Step 0 runs when only
    /// a direction is given, step `k` when the k-th speed input is also on.
    pub steps_hz: Vec<f64>,
    pub sense: Option<LoadSense>,
    /// Rated shaft torque of the motor, N m (reference for the load fraction).
    pub rated_torque_nm: f64,
}

impl HoistLogicConfig {
    pub fn validate(&self) -> Result<(), InputError> {
        self.brake.validate()?;
        if self.steps_hz.is_empty() || self.steps_hz.len() > 4 {
            return Err(InputError::OutOfRange {
                field: "steps",
                value: self.steps_hz.len() as f64,
                min: 1.0,
                max: 4.0,
            });
        }
        for f in &self.steps_hz {
            error::positive("step_hz", *f)?;
        }
        error::positive("rated_torque_nm", self.rated_torque_nm)?;
        if let Some(s) = &self.sense {
            s.validate()?;
        }
        Ok(())
    }
}

/// Why the sequence is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicFault {
    /// The release current was not reached in time: the brake stays closed.
    NoTorqueAtRelease,
    /// The brake feedback did not confirm the opening in time.
    BrakeFeedback,
    /// The drive tripped or the mains went away: the brake closes at once.
    DriveOrMains,
}

/// Where the sequence is, for the reading in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicPhase {
    Idle,
    Magnetising,
    Releasing,
    Running,
    Decelerating,
    Engaging,
    Fault(LogicFault),
}

/// Something the sequence found, to show next to the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicEvent {
    /// The load was sensed and the top step set to `applied_hz`.
    LoadSensed {
        fraction: f64,
        applied_hz: f64,
        limited_by_motor: bool,
    },
    Fault(LogicFault),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicInput {
    pub up: bool,
    pub down: bool,
    /// Highest speed step asked for (0 = slowest).
    pub step: usize,
    /// The drive is healthy (not tripped).
    pub drive_ok: bool,
    pub mains_ok: bool,
    /// Actual output frequency of the drive, Hz (magnitude).
    pub frequency_hz: f64,
    pub motor_current_a: f64,
    /// Shaft torque of the motor, N m (any sign).
    pub motor_torque_nm: f64,
    /// Brake feedback contact, if one is wired.
    pub brake_feedback: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogicOutput {
    pub run_up: bool,
    pub run_down: bool,
    pub reference_hz: f64,
    /// The drive's brake relay: on means "open the brake".
    pub brake_relay: bool,
    pub phase: LogicPhase,
    pub events: Vec<LogicEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle { restart_left_s: f64 },
    Magnetising { t: f64 },
    Releasing { t: f64 },
    Running,
    Decelerating,
    Engaging { t: f64 },
    Fault(LogicFault),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Sense {
    /// Not measured in this run.
    Unknown,
    Measuring {
        t: f64,
        sum: f64,
    },
    /// Top step frequency allowed for this run.
    Done {
        applied_hz: f64,
    },
}

/// Slack for "the frequency is at the reference".
const HZ_TOL: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq)]
pub struct HoistLogic {
    cfg: HoistLogicConfig,
    phase: Phase,
    /// Direction of the cycle that is running: `true` = up.
    up: bool,
    sense: Sense,
}

impl HoistLogic {
    pub fn new(cfg: HoistLogicConfig) -> Result<Self, InputError> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            phase: Phase::Idle {
                restart_left_s: 0.0,
            },
            up: true,
            sense: Sense::Unknown,
        })
    }

    pub fn config(&self) -> &HoistLogicConfig {
        &self.cfg
    }

    pub fn phase(&self) -> LogicPhase {
        match self.phase {
            Phase::Idle { .. } => LogicPhase::Idle,
            Phase::Magnetising { .. } => LogicPhase::Magnetising,
            Phase::Releasing { .. } => LogicPhase::Releasing,
            Phase::Running => LogicPhase::Running,
            Phase::Decelerating => LogicPhase::Decelerating,
            Phase::Engaging { .. } => LogicPhase::Engaging,
            Phase::Fault(f) => LogicPhase::Fault(f),
        }
    }

    /// Top step frequency in use now (nominal or the sensed one).
    pub fn top_step_hz(&self) -> f64 {
        let nominal = self.cfg.steps_hz[self.cfg.steps_hz.len() - 1];
        match self.sense {
            Sense::Done { applied_hz } => applied_hz,
            _ => nominal,
        }
    }

    /// Advance by `dt_s`. `capability_nm(f)` is the largest shaft torque the motor can deliver
    /// at frequency `f` (the caller's motor model); only load sensing uses it.
    pub fn step(
        &mut self,
        inp: &LogicInput,
        capability_nm: &dyn Fn(f64) -> f64,
        dt_s: f64,
    ) -> LogicOutput {
        let mut events = Vec::new();
        let want_dir = match (inp.up, inp.down) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        };
        let healthy = inp.drive_ok && inp.mains_ok;
        let b = self.cfg.brake.clone();

        // A trip or lost mains closes the brake at once, whatever the sequence was doing.
        if !healthy && !matches!(self.phase, Phase::Idle { .. } | Phase::Fault(_)) {
            self.enter_fault(LogicFault::DriveOrMains, &mut events);
        }

        self.phase = match self.phase {
            Phase::Idle { restart_left_s } => {
                let left = (restart_left_s - dt_s).max(0.0);
                match want_dir {
                    Some(up) if left == 0.0 && healthy => {
                        self.up = up;
                        self.sense = Sense::Unknown;
                        Phase::Magnetising { t: 0.0 }
                    }
                    _ => Phase::Idle {
                        restart_left_s: left,
                    },
                }
            }
            Phase::Magnetising { t } => {
                let t = t + dt_s;
                let needed = if self.up {
                    b.release_current_up_a
                } else {
                    b.release_current_down_a
                };
                if want_dir != Some(self.up) {
                    // Stopped before the brake opened: nothing to hold, drop the output.
                    Phase::Idle {
                        restart_left_s: b.restart_time_s,
                    }
                } else if inp.frequency_hz >= b.release_hz - HZ_TOL && inp.motor_current_a >= needed
                {
                    Phase::Releasing { t: 0.0 }
                } else if t > b.release_timeout_s {
                    events.push(LogicEvent::Fault(LogicFault::NoTorqueAtRelease));
                    Phase::Fault(LogicFault::NoTorqueAtRelease)
                } else {
                    Phase::Magnetising { t }
                }
            }
            Phase::Releasing { t } => {
                let t = t + dt_s;
                if want_dir != Some(self.up) {
                    Phase::Decelerating
                } else {
                    match inp.brake_feedback {
                        Some(open) => {
                            if t >= b.release_time_s && open {
                                Phase::Running
                            } else if t >= b.release_time_s + b.feedback_timeout_s {
                                events.push(LogicEvent::Fault(LogicFault::BrakeFeedback));
                                Phase::Fault(LogicFault::BrakeFeedback)
                            } else {
                                Phase::Releasing { t }
                            }
                        }
                        None if t >= b.release_time_s => Phase::Running,
                        None => Phase::Releasing { t },
                    }
                }
            }
            Phase::Running => {
                if want_dir != Some(self.up) {
                    Phase::Decelerating
                } else {
                    Phase::Running
                }
            }
            Phase::Decelerating => {
                if inp.frequency_hz <= b.engage_hz + HZ_TOL {
                    Phase::Engaging { t: 0.0 }
                } else {
                    Phase::Decelerating
                }
            }
            Phase::Engaging { t } => {
                let t = t + dt_s;
                if t >= b.engage_time_s {
                    Phase::Idle {
                        restart_left_s: b.restart_time_s,
                    }
                } else {
                    Phase::Engaging { t }
                }
            }
            Phase::Fault(f) => {
                // Leaves when the operator lets go and the drive is healthy again.
                if want_dir.is_none() && !inp.up && !inp.down && healthy {
                    Phase::Idle {
                        restart_left_s: b.restart_time_s,
                    }
                } else {
                    Phase::Fault(f)
                }
            }
        };

        let (run, reference_hz) = match self.phase {
            Phase::Idle { .. } | Phase::Fault(_) => (false, 0.0),
            Phase::Magnetising { .. } | Phase::Releasing { .. } => (true, b.release_hz),
            Phase::Decelerating | Phase::Engaging { .. } => (true, b.engage_hz),
            Phase::Running => (
                true,
                self.running_reference(inp, capability_nm, dt_s, &mut events),
            ),
        };
        let brake_relay = matches!(
            self.phase,
            Phase::Releasing { .. } | Phase::Running | Phase::Decelerating
        );
        LogicOutput {
            run_up: run && self.up,
            run_down: run && !self.up,
            reference_hz,
            brake_relay,
            phase: self.phase(),
            events,
        }
    }

    fn enter_fault(&mut self, why: LogicFault, events: &mut Vec<LogicEvent>) {
        events.push(LogicEvent::Fault(why));
        self.phase = Phase::Fault(why);
    }

    /// Reference in the running phase: the chosen step, held at the probe frequency while the
    /// load is being sensed.
    fn running_reference(
        &mut self,
        inp: &LogicInput,
        capability_nm: &dyn Fn(f64) -> f64,
        dt_s: f64,
        events: &mut Vec<LogicEvent>,
    ) -> f64 {
        let n = self.cfg.steps_hz.len();
        let step = inp.step.min(n - 1);
        let nominal = self.cfg.steps_hz[step];
        let Some(sense) = self.cfg.sense.clone() else {
            return nominal;
        };
        // Only the top step is sensed, and only when it lies above the probe frequency.
        if step != n - 1 || nominal <= sense.probe_hz {
            return nominal;
        }
        match self.sense {
            Sense::Done { applied_hz } => applied_hz,
            Sense::Unknown | Sense::Measuring { .. } => {
                let (mut t, mut sum) = match self.sense {
                    Sense::Measuring { t, sum } => (t, sum),
                    _ => (0.0, 0.0),
                };
                // Averaged over the probe time, once the output is at the probe frequency.
                if (inp.frequency_hz - sense.probe_hz).abs() <= 1e-3 {
                    t += dt_s;
                    sum += inp.motor_torque_nm.abs() * dt_s;
                }
                if t >= sense.probe_time_s {
                    let torque = sum / t;
                    let (applied_hz, limited_by_motor) =
                        self.allowed_top(nominal, torque, &sense, capability_nm);
                    self.sense = Sense::Done { applied_hz };
                    events.push(LogicEvent::LoadSensed {
                        fraction: torque / self.cfg.rated_torque_nm,
                        applied_hz,
                        limited_by_motor,
                    });
                    applied_hz
                } else {
                    self.sense = Sense::Measuring { t, sum };
                    sense.probe_hz
                }
            }
        }
    }

    /// Top step frequency for a sensed torque, and whether the motor limited it.
    fn allowed_top(
        &self,
        nominal: f64,
        torque_nm: f64,
        sense: &LoadSense,
        capability_nm: &dyn Fn(f64) -> f64,
    ) -> (f64, bool) {
        let fraction = torque_nm / self.cfg.rated_torque_nm;
        let boost = sense.boost_hz.max(nominal);
        let want = if fraction >= 1.0 {
            nominal
        } else if fraction <= sense.light_fraction {
            boost
        } else {
            let k = (1.0 - fraction) / (1.0 - sense.light_fraction);
            nominal + (boost - nominal) * k
        };
        let need = sense.margin * torque_nm;
        if capability_nm(want) >= need {
            return (want, false);
        }
        if capability_nm(nominal) < need {
            // Not even the nominal step has the margin: no boost, and the caller's checks say so.
            return (nominal, true);
        }
        // Capability falls with frequency: bisect between the nominal and the wanted frequency.
        let (mut lo, mut hi) = (nominal, want);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if capability_nm(mid) >= need {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.01;

    fn cfg() -> HoistLogicConfig {
        HoistLogicConfig {
            brake: BrakeLogic {
                release_hz: 3.0,
                release_current_up_a: 5.0,
                release_current_down_a: 3.0,
                release_time_s: 0.3,
                engage_hz: 2.0,
                engage_time_s: 0.4,
                restart_time_s: 0.5,
                release_timeout_s: 1.0,
                feedback_timeout_s: 0.5,
            },
            steps_hz: vec![10.0, 25.0, 50.0],
            sense: None,
            rated_torque_nm: 36.0,
        }
    }

    fn input(up: bool) -> LogicInput {
        LogicInput {
            up,
            down: false,
            step: 0,
            drive_ok: true,
            mains_ok: true,
            frequency_hz: 0.0,
            motor_current_a: 0.0,
            motor_torque_nm: 0.0,
            brake_feedback: None,
        }
    }

    fn flat(_f: f64) -> f64 {
        1e6
    }

    #[test]
    fn the_brake_opens_only_after_the_current_is_there() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Magnetising);
        assert!(o.run_up && !o.run_down && !o.brake_relay);
        assert_eq!(o.reference_hz, 3.0);
        // At frequency but not enough current: still magnetising.
        i.frequency_hz = 3.0;
        i.motor_current_a = 4.9;
        assert!(!l.step(&i, &flat, DT).brake_relay);
        i.motor_current_a = 5.0;
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Releasing);
        assert!(o.brake_relay);
    }

    #[test]
    fn the_run_goes_on_after_the_release_time() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        l.step(&i, &flat, DT);
        l.step(&i, &flat, DT);
        let mut t = 0.0;
        let mut o = l.step(&i, &flat, DT);
        while o.phase == LogicPhase::Releasing {
            t += DT;
            o = l.step(&i, &flat, DT);
            assert!(t < 1.0);
        }
        assert!((t - 0.3).abs() < 0.03, "{t}");
        assert_eq!(o.phase, LogicPhase::Running);
        assert_eq!(o.reference_hz, 10.0);
        i.step = 2;
        assert_eq!(l.step(&i, &flat, DT).reference_hz, 50.0);
        i.step = 9;
        assert_eq!(
            l.step(&i, &flat, DT).reference_hz,
            50.0,
            "clamped to the top step"
        );
    }

    #[test]
    fn no_current_in_time_is_a_fault_and_the_brake_stays_closed() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        let mut last = None;
        for _ in 0..200 {
            let o = l.step(&i, &flat, DT);
            assert!(!o.brake_relay);
            last = Some(o);
        }
        assert_eq!(
            last.unwrap().phase,
            LogicPhase::Fault(LogicFault::NoTorqueAtRelease)
        );
    }

    #[test]
    fn stop_comes_down_to_the_engage_frequency_then_holds_the_output() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        for _ in 0..60 {
            l.step(&i, &flat, DT);
        }
        assert_eq!(l.phase(), LogicPhase::Running);
        i.up = false;
        i.frequency_hz = 10.0;
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Decelerating);
        assert!(o.brake_relay && o.run_up);
        assert_eq!(o.reference_hz, 2.0);
        i.frequency_hz = 2.0;
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Engaging);
        assert!(!o.brake_relay, "the relay opens first");
        assert!(o.run_up, "the output stays on while the brake closes");
        for _ in 0..39 {
            assert!(l.step(&i, &flat, DT).run_up);
        }
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Idle);
        assert!(!o.run_up);
    }

    #[test]
    fn restart_waits_for_the_restart_time() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        // After a stop, a new command is held back for 0.5 s.
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        for _ in 0..60 {
            l.step(&i, &flat, DT);
        }
        i.up = false;
        i.frequency_hz = 2.0;
        for _ in 0..60 {
            l.step(&i, &flat, DT);
        }
        assert!(matches!(l.phase(), LogicPhase::Idle));
        i.up = true;
        let o = l.step(&i, &flat, DT);
        assert_eq!(o.phase, LogicPhase::Idle, "restart time not over");
        assert!(!o.run_up);
    }

    #[test]
    fn a_trip_closes_the_brake_at_once() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        for _ in 0..60 {
            l.step(&i, &flat, DT);
        }
        i.drive_ok = false;
        let o = l.step(&i, &flat, DT);
        assert!(!o.brake_relay && !o.run_up);
        assert_eq!(o.phase, LogicPhase::Fault(LogicFault::DriveOrMains));
        // Stays in the fault until the operator lets go and the drive is healthy.
        i.drive_ok = true;
        assert!(matches!(l.step(&i, &flat, DT).phase, LogicPhase::Fault(_)));
        i.up = false;
        assert_eq!(l.step(&i, &flat, DT).phase, LogicPhase::Idle);
    }

    #[test]
    fn the_brake_feedback_must_confirm() {
        let mut l = HoistLogic::new(cfg()).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        i.brake_feedback = Some(false);
        let mut phase = LogicPhase::Idle;
        for _ in 0..300 {
            phase = l.step(&i, &flat, DT).phase;
        }
        assert_eq!(phase, LogicPhase::Fault(LogicFault::BrakeFeedback));
        // And it passes when the contact follows.
        let mut l = HoistLogic::new(cfg()).unwrap();
        i.brake_feedback = Some(true);
        for _ in 0..100 {
            phase = l.step(&i, &flat, DT).phase;
        }
        assert_eq!(phase, LogicPhase::Running);
    }

    #[test]
    fn load_sense_boosts_a_light_load_within_the_motor_capability() {
        let mut c = cfg();
        c.sense = Some(LoadSense {
            probe_hz: 20.0,
            probe_time_s: 0.2,
            light_fraction: 0.3,
            boost_hz: 87.0,
            margin: 1.5,
        });
        // Capability 100 N m at 50 Hz falling with 1/f^2 above it.
        let cap = |f: f64| {
            if f <= 50.0 {
                100.0
            } else {
                100.0 * (50.0 / f).powi(2)
            }
        };
        let run = |torque: f64| {
            let mut l = HoistLogic::new(c.clone()).unwrap();
            let mut i = input(true);
            i.frequency_hz = 3.0;
            i.motor_current_a = 6.0;
            for _ in 0..60 {
                l.step(&i, &cap, DT);
            }
            i.step = 2;
            i.frequency_hz = 20.0;
            i.motor_torque_nm = torque;
            let mut out = l.step(&i, &cap, DT);
            // Held at the probe frequency until the torque is averaged.
            assert_eq!(out.reference_hz, 20.0);
            let mut events = Vec::new();
            for _ in 0..40 {
                out = l.step(&i, &cap, DT);
                events.extend(out.events.clone());
            }
            (out.reference_hz, events)
        };
        // 5 N m is 14 % of 36: light, full boost wanted; the motor delivers 1.5 x 5 = 7.5 N m
        // up to f with 100 (50/f)^2 = 7.5, f = 182 Hz, so 87 Hz is allowed.
        let (hz, events) = run(5.0);
        assert!((hz - 87.0).abs() < 1e-9, "{hz}");
        assert!(matches!(
            events[0],
            LogicEvent::LoadSensed {
                limited_by_motor: false,
                ..
            }
        ));
        // 20 N m is 56 %: part way. Wanted: 50 + 37 * (1 - 0.556) / 0.7 = 73.5 Hz; capability
        // at 73.5 Hz is 46 N m which is above 1.5 x 20 = 30.
        let (hz, _) = run(20.0);
        assert!((hz - 73.47).abs() < 0.1, "{hz}");
        // Full rated load: nominal only.
        let (hz, _) = run(36.0);
        assert_eq!(hz, 50.0);
        // A load that the motor can only just carry at 60 Hz: limited by capability.
        let mut c2 = c.clone();
        c2.sense.as_mut().unwrap().light_fraction = 0.9;
        let cap2 = |f: f64| {
            if f <= 50.0 {
                60.0
            } else {
                60.0 * (50.0 / f).powi(2)
            }
        };
        let mut l = HoistLogic::new(c2).unwrap();
        let mut i = input(true);
        i.frequency_hz = 3.0;
        i.motor_current_a = 6.0;
        for _ in 0..60 {
            l.step(&i, &cap2, DT);
        }
        i.step = 2;
        i.frequency_hz = 20.0;
        i.motor_torque_nm = 20.0;
        let mut hz = 0.0;
        for _ in 0..40 {
            hz = l.step(&i, &cap2, DT).reference_hz;
        }
        // 1.5 x 20 = 30 N m: 60 (50/f)^2 = 30 gives f = 70.7 Hz.
        assert!((hz - 70.71).abs() < 0.01, "{hz}");
    }

    #[test]
    fn invalid_settings_are_refused() {
        let mut c = cfg();
        c.steps_hz.clear();
        assert!(HoistLogic::new(c).is_err());
        let mut c = cfg();
        c.steps_hz = vec![1.0; 5];
        assert!(HoistLogic::new(c).is_err());
        let mut c = cfg();
        c.brake.release_timeout_s = 0.0;
        assert!(HoistLogic::new(c).is_err());
        let mut c = cfg();
        c.brake.release_hz = f64::NAN;
        assert!(HoistLogic::new(c).is_err());
    }
}
