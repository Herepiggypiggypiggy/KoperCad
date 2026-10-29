//! A duty cycle for a drive hoist (lane L2c-open, "heat over a duty cycle"): lift a given travel,
//! pause, lower it, pause, and repeat, faster than real time, to see how the motor winding and
//! the brake heat up and how many starts per hour the hoist stands.
//!
//! The run uses an ideal wiring of its own: an operator who holds up, down and a speed step, a
//! main contactor that stays in, and a brake contactor that follows the drive's brake relay. It
//! does not read the drawing, so wiring faults do not show here (Simulate > Drive run shows
//! those); what it shows is what the numbers of the hoist do over time. The heat numbers are the
//! model of `dh_heat_*` and `dh_brake_*`, generic until replaced by the maker's data.

use crate::cosim::StepConfig;
use crate::drive_hoist_loop::{
    DriveHoistBinding, DriveHoistLoop, DriveHoistModule, FindingKind, HoistFinding,
};
use crate::port::CircuitPort;
use novo_drive::InputError;
use novo_drive::hoist_logic::LogicPhase;

const UP: &str = "=DUTY-UP";
const DOWN: &str = "=DUTY-DOWN";
const MAINS: &str = "=DUTY-MAINS";
const RELAY: &str = "=DUTY-R";
const BRAKE_FEED: &str = "=DUTY-KB";
const STEP_INPUTS: [&str; 3] = ["=DUTY-S2", "=DUTY-S3", "=DUTY-S4"];

/// The longest one stroke may take before the run calls the hoist stuck, s.
const STROKE_TIMEOUT_S: f64 = 900.0;
/// The longest simulated time of a run, s.
const MAX_TIME_S: f64 = 24.0 * 3600.0;
/// Largest time step of a run, s: the hoist model is validated at the loop's default step, and a
/// larger one let a load hang on a drive that was just started to lower it.
const MAX_DT_S: f64 = 0.005;
/// Seconds between the samples of the curve.
const SAMPLE_S: f64 = 10.0;

/// The cycle to run.
#[derive(Debug, Clone, PartialEq)]
pub struct DutyCycle {
    /// Hook travel per stroke, m.
    pub travel_m: f64,
    /// Speed step used for both strokes, 1 (slowest) to the number of steps.
    pub step: usize,
    /// Pause at the top and at the bottom after the hook has stopped, s.
    pub pause_top_s: f64,
    pub pause_bottom_s: f64,
    /// Number of up and down pairs.
    pub cycles: u32,
    /// Time step of the run, s (at most 5 ms).
    pub dt_s: f64,
    /// Stop at the first time the winding or the brake reaches its limit.
    pub stop_when_hot: bool,
}

impl DutyCycle {
    pub fn validate(&self, steps: usize) -> Result<(), InputError> {
        let bad = |field: &'static str, value: f64| InputError::NotPositive { field, value };
        if !(self.travel_m > 0.0 && self.travel_m.is_finite()) {
            return Err(bad("travel_m", self.travel_m));
        }
        if self.pause_top_s < 0.0 || self.pause_bottom_s < 0.0 {
            return Err(bad("pause_s", self.pause_top_s.min(self.pause_bottom_s)));
        }
        if self.cycles == 0 {
            return Err(bad("cycles", 0.0));
        }
        if !(self.dt_s > 0.0 && self.dt_s <= MAX_DT_S) {
            return Err(bad("dt_s", self.dt_s));
        }
        if self.step == 0 || self.step > steps {
            return Err(InputError::OutOfRange {
                field: "step",
                value: self.step as f64,
                min: 1.0,
                max: steps as f64,
            });
        }
        Ok(())
    }
}

/// Why the run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DutyEnd {
    Completed,
    /// The winding reached its limit rise (only when `stop_when_hot`).
    MotorTooHot,
    BrakeTooHot,
    /// The hoist did not finish a stroke in time (a fault, a stuck brake, a trip).
    Stuck,
    /// The simulated time reached the cap.
    TimeCap,
}

/// What a duty cycle run found.
#[derive(Debug, Clone, PartialEq)]
pub struct DutyReport {
    pub end: DutyEnd,
    /// Complete cycles.
    pub cycles_done: u32,
    pub time_s: f64,
    pub starts: u32,
    pub peak_winding_rise_k: f64,
    pub peak_brake_rise_k: f64,
    pub winding_limit_k: f64,
    pub brake_limit_k: f64,
    /// Share of the brake lining used.
    pub brake_wear: f64,
    pub brake_energy_j: f64,
    /// Time of the first limit crossing of the winding, s.
    pub first_hot_s: Option<f64>,
    /// `(time s, winding rise K, brake rise K)`, one point every 10 s.
    pub curve: Vec<(f64, f64, f64)>,
    pub findings: Vec<HoistFinding>,
}

impl DutyReport {
    pub fn starts_per_hour(&self) -> f64 {
        if self.time_s > 0.0 {
            f64::from(self.starts) / (self.time_s / 3600.0)
        } else {
            0.0
        }
    }
}

/// The ideal circuit of the run.
#[derive(Default)]
struct DutyCircuit {
    up: bool,
    down: bool,
    step_input: Option<usize>,
    relay: bool,
}

impl CircuitPort for DutyCircuit {
    fn settle(&mut self, _dt_s: f64) {}

    fn motor_feed(&self, _motor: &str) -> crate::port::MotorFeed {
        crate::port::MotorFeed::Off
    }

    fn is_energised(&self, device: &str) -> bool {
        match device {
            UP => self.up,
            DOWN => self.down,
            MAINS => true,
            BRAKE_FEED => self.relay,
            d => STEP_INPUTS
                .iter()
                .position(|t| *t == d)
                .is_some_and(|k| self.step_input == Some(k)),
        }
    }

    fn set_contact(&mut self, device: &str, closed: bool) {
        if device == RELAY {
            self.relay = closed;
        }
    }

    fn knows_device(&self, _device: &str) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Up,
    SettleTop(f64),
    Down,
    SettleBottom(f64),
}

/// Runs the duty cycle on a drive hoist. `progress` is called with the number of complete cycles
/// whenever one more is done. The module is consumed; its own wiring is replaced by the ideal
/// one described in the module docs.
pub fn run(
    mut module: DriveHoistModule,
    cycle: &DutyCycle,
    progress: &mut dyn FnMut(u32),
) -> Result<DutyReport, InputError> {
    let steps = module.logic.config().steps_hz.len();
    cycle.validate(steps)?;
    module.binding = DriveHoistBinding {
        up: Some(UP.into()),
        down: Some(DOWN.into()),
        speed_inputs: (0..steps - 1)
            .map(|k| Some(STEP_INPUTS[k].into()))
            .collect(),
        mains: Some(MAINS.into()),
        brake_relay: Some(RELAY.into()),
        brake_feed: Some(BRAKE_FEED.into()),
        brake_feedback: None,
        reset: None,
        limits: Vec::new(),
    };
    let mut lp = DriveHoistLoop::new(StepConfig { dt_s: cycle.dt_s })?;
    lp.modules.push(module);
    let mut circuit = DutyCircuit {
        step_input: cycle.step.checked_sub(2),
        ..DutyCircuit::default()
    };
    let (mut stage, mut stage_since) = (Stage::Up, 0.0);
    circuit.up = true;
    let mut cycles_done = 0;
    let (mut starts, mut was_idle) = (0u32, true);
    let (mut peak_w, mut peak_b) = (0.0_f64, 0.0_f64);
    let mut curve = Vec::new();
    let mut next_sample = 0.0;
    let mut first_hot = None;
    let mut limits = (f64::INFINITY, f64::INFINITY);
    let mut end = DutyEnd::Completed;
    loop {
        lp.step(&mut circuit);
        let t = lp.time_s();
        let m = &lp.modules[0];
        let Some(s) = m.snapshot() else {
            continue;
        };
        let (h, idle, still) = (
            s.height_m,
            matches!(s.logic.phase, LogicPhase::Idle),
            s.speed_m_s == 0.0,
        );
        if was_idle && !idle {
            starts += 1;
        }
        was_idle = idle;
        if let Some(heat) = s.heat {
            peak_w = peak_w.max(heat.winding_rise_k);
            peak_b = peak_b.max(heat.brake_rise_k);
            if t >= next_sample {
                curve.push((t, heat.winding_rise_k, heat.brake_rise_k));
                next_sample = t + SAMPLE_S;
            }
        }
        if limits.0.is_infinite() {
            limits = (
                m.heat_limits().map_or(f64::INFINITY, |l| l.0),
                m.heat_limits().map_or(f64::INFINITY, |l| l.1),
            );
        }
        if first_hot.is_none()
            && m.findings()
                .iter()
                .any(|f| matches!(f.kind, FindingKind::MotorTooHot { .. }))
        {
            first_hot = Some(t);
            if cycle.stop_when_hot {
                end = DutyEnd::MotorTooHot;
                break;
            }
        }
        if cycle.stop_when_hot
            && m.findings()
                .iter()
                .any(|f| matches!(f.kind, FindingKind::BrakeHot { .. }))
        {
            end = DutyEnd::BrakeTooHot;
            break;
        }
        if t > MAX_TIME_S {
            end = DutyEnd::TimeCap;
            break;
        }
        if t - stage_since > STROKE_TIMEOUT_S + cycle.pause_top_s.max(cycle.pause_bottom_s)
            || m.fault().is_some()
        {
            end = DutyEnd::Stuck;
            break;
        }
        match stage {
            Stage::Up if h >= cycle.travel_m => {
                circuit.up = false;
                stage = Stage::SettleTop(f64::NAN);
            }
            Stage::Down if h <= 0.0 => {
                circuit.down = false;
                stage = Stage::SettleBottom(f64::NAN);
            }
            Stage::SettleTop(since) | Stage::SettleBottom(since) => {
                let top = matches!(stage, Stage::SettleTop(_));
                let since = if since.is_nan() && idle && still {
                    t
                } else {
                    since
                };
                stage = if top {
                    Stage::SettleTop(since)
                } else {
                    Stage::SettleBottom(since)
                };
                let pause = if top {
                    cycle.pause_top_s
                } else {
                    cycle.pause_bottom_s
                };
                if !since.is_nan() && t - since >= pause {
                    stage_since = t;
                    if top {
                        circuit.down = true;
                        stage = Stage::Down;
                    } else {
                        cycles_done += 1;
                        progress(cycles_done);
                        if cycles_done >= cycle.cycles {
                            break;
                        }
                        circuit.up = true;
                        stage = Stage::Up;
                    }
                }
            }
            _ => {}
        }
    }
    let m = &lp.modules[0];
    let heat = m.snapshot().and_then(|s| s.heat);
    Ok(DutyReport {
        end,
        cycles_done,
        time_s: lp.time_s(),
        starts,
        peak_winding_rise_k: peak_w,
        peak_brake_rise_k: peak_b,
        winding_limit_k: limits.0,
        brake_limit_k: limits.1,
        brake_wear: heat.map_or(0.0, |h| h.brake_wear),
        brake_energy_j: heat.map_or(0.0, |h| h.brake_energy_j),
        first_hot_s: first_hot,
        curve,
        findings: m.findings().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive_hoist_spec::{DRIVE_HOIST_KEY, drive_hoist_from_properties};
    use std::collections::BTreeMap;

    fn module(extra: &[(&str, &str)]) -> DriveHoistModule {
        let mut p: BTreeMap<String, String> = BTreeMap::new();
        p.insert(DRIVE_HOIST_KEY.into(), "2000".into());
        for (k, v) in extra {
            p.insert((*k).into(), (*v).into());
        }
        drive_hoist_from_properties("=M1", &|k| p.get(k).cloned())
            .unwrap()
            .unwrap()
            .0
    }

    fn cycle(cycles: u32, pause: f64) -> DutyCycle {
        DutyCycle {
            travel_m: 0.8,
            step: 3,
            pause_top_s: pause,
            pause_bottom_s: pause,
            cycles,
            dt_s: 0.005,
            stop_when_hot: false,
        }
    }

    fn go(extra: &[(&str, &str)], c: &DutyCycle) -> DutyReport {
        run(module(extra), c, &mut |_| {}).unwrap()
    }

    #[test]
    fn a_short_duty_cycle_completes_with_four_starts_per_two_cycles() {
        let mut seen = Vec::new();
        let r = run(module(&[]), &cycle(2, 2.0), &mut |n| seen.push(n)).unwrap();
        assert_eq!(r.end, DutyEnd::Completed);
        assert_eq!((r.cycles_done, r.starts), (2, 4));
        assert_eq!(seen, vec![1, 2]);
        assert!(r.peak_winding_rise_k > 0.5, "{}", r.peak_winding_rise_k);
        assert!(!r.curve.is_empty());
        assert!(r.starts_per_hour() > 100.0, "{}", r.starts_per_hour());
    }

    #[test]
    fn more_cycles_heat_more_and_a_long_pause_cools() {
        let few = go(&[], &cycle(2, 1.0));
        let many = go(&[], &cycle(6, 1.0));
        assert!(many.peak_winding_rise_k > few.peak_winding_rise_k + 0.3);
        // Same strokes with a long wait between them: more time to shed heat.
        let rested = go(&[], &cycle(6, 120.0));
        assert!(
            rested.peak_winding_rise_k < many.peak_winding_rise_k,
            "{} vs {}",
            rested.peak_winding_rise_k,
            many.peak_winding_rise_k
        );
        assert!(rested.starts_per_hour() < many.starts_per_hour());
    }

    #[test]
    fn a_low_limit_ends_the_run_with_a_cool_down_time() {
        let mut c = cycle(50, 0.5);
        c.stop_when_hot = true;
        let r = go(&[("dh_heat_limit_k", "3")], &c);
        assert_eq!(r.end, DutyEnd::MotorTooHot);
        assert!(r.first_hot_s.is_some());
        let cool = r.findings.iter().find_map(|f| match f.kind {
            FindingKind::MotorTooHot { cool_down_s, .. } => Some(cool_down_s),
            _ => None,
        });
        assert!(cool.unwrap() >= 10.0, "{cool:?}");
        assert!(r.cycles_done < 50);
    }

    #[test]
    fn a_normal_duty_cycle_hardly_wears_the_brake() {
        let r = go(&[], &cycle(3, 1.0));
        assert!(r.brake_wear < 0.001, "{}", r.brake_wear);
        assert!(r.peak_brake_rise_k < 5.0, "{}", r.peak_brake_rise_k);
    }

    #[test]
    fn wrong_cycles_are_refused() {
        let m = module(&[]);
        let mut c = cycle(1, 1.0);
        c.step = 9;
        assert!(run(m, &c, &mut |_| {}).is_err());
        let mut c = cycle(0, 1.0);
        c.cycles = 0;
        assert!(run(module(&[]), &c, &mut |_| {}).is_err());
        let mut c = cycle(1, 1.0);
        c.travel_m = 0.0;
        assert!(run(module(&[]), &c, &mut |_| {}).is_err());
    }
}
