//! A hook on an elastic rope above a floor (lane L2c-open): slack rope when the hook is lowered
//! onto the floor, the take-up of the slack and the pick-up of the load from the floor with the
//! tension peak ("snatch") that the rope stiffness and the lifting speed cause.
//!
//! Coordinates are hook heights in metres, positive up. `drum_m` is the height the hook would
//! have if the rope were taut and unstretched (the drum position, from the shaft angle); the
//! hook itself is a separate mass on a spring-damper between the drum and the hook, which rests
//! on the floor when the rope is slack or does not yet carry the whole weight.
//!
//! Exact by definition: the spring law (zero tension while the rope is slack, `k` times the
//! stretch while it is taut), the weight, the floor as a one-sided constraint with an inelastic
//! landing. Model: one stiffness and one damping ratio for the whole rope (no rope mass, no
//! hook swing, no sheave friction), the hook mass lumped, and an integration in sub-steps of a
//! fraction of the rope's natural period.

use crate::GRAVITY;
use crate::error::{self, InputError};

/// Hook speed on landing below which no landing is reported, m/s.
const LANDING_REPORT_M_S: f64 = 0.05;
/// Slack below this is not reported, m.
const SLACK_REPORT_M: f64 = 0.005;
/// Time after the load leaves the floor over which the tension peak is looked for, s.
const PEAK_WINDOW_S: f64 = 0.5;

/// The rope and the floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RopeConfig {
    /// Hook height of the floor, relative to the start height of the drum (0). At most 0: the
    /// hook starts at or above the floor.
    pub floor_m: f64,
    /// Stiffness of the rope as seen at the hook (all falls together), N/m.
    pub stiffness_n_per_m: f64,
    /// Damping as a fraction of critical damping (0 = none).
    pub damping_ratio: f64,
}

impl RopeConfig {
    pub fn validate(&self) -> Result<(), InputError> {
        error::finite("floor_m", self.floor_m)?;
        if self.floor_m > 0.0 {
            return Err(InputError::OutOfRange {
                field: "floor_m (the hook starts at or above the floor)",
                value: self.floor_m,
                min: f64::NEG_INFINITY,
                max: 0.0,
            });
        }
        error::positive("rope_stiffness_n_per_m", self.stiffness_n_per_m)?;
        error::non_negative("rope_damping_ratio", self.damping_ratio)?;
        Ok(())
    }
}

/// Something the rope did in a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RopeEvent {
    /// The hook came down on the floor at this speed (positive, m/s).
    Landed { speed_m_s: f64 },
    /// The rope became taut again after being slack by up to `slack_m`.
    SlackTaken { slack_m: f64 },
    /// The load left the floor at this drum speed (m/s, positive up).
    LiftOff { drum_speed_m_s: f64 },
    /// Highest rope tension within the window after the load left the floor.
    TensionPeak { newtons: f64, factor: f64 },
}

/// The hook and the rope.
#[derive(Debug, Clone, PartialEq)]
pub struct RopeLoad {
    cfg: RopeConfig,
    mass_kg: f64,
    hook_m: f64,
    hook_speed: f64,
    tension_n: f64,
    on_floor: bool,
    max_slack_m: f64,
    peak_window_s: Option<f64>,
    peak_n: f64,
    drum_prev_m: f64,
}

impl RopeLoad {
    /// The hook of `mass_kg` hanging taut at drum height 0, or resting on the floor with the rope
    /// just unstretched when the floor is at 0.
    pub fn new(cfg: RopeConfig, mass_kg: f64) -> Result<Self, InputError> {
        cfg.validate()?;
        error::positive("mass_kg", mass_kg)?;
        let static_stretch = mass_kg * GRAVITY / cfg.stiffness_n_per_m;
        let hanging = -static_stretch;
        let (hook_m, tension_n, on_floor) = if cfg.floor_m >= hanging {
            (
                cfg.floor_m,
                0.0_f64.max(-cfg.floor_m * cfg.stiffness_n_per_m),
                true,
            )
        } else {
            (hanging, mass_kg * GRAVITY, false)
        };
        Ok(Self {
            cfg,
            mass_kg,
            hook_m,
            hook_speed: 0.0,
            tension_n,
            on_floor,
            max_slack_m: 0.0,
            peak_window_s: None,
            peak_n: 0.0,
            drum_prev_m: 0.0,
        })
    }

    pub fn config(&self) -> &RopeConfig {
        &self.cfg
    }

    pub fn weight_n(&self) -> f64 {
        self.mass_kg * GRAVITY
    }

    /// Rope tension in newtons (zero while slack).
    pub fn tension_n(&self) -> f64 {
        self.tension_n
    }

    /// Tension as a fraction of the weight: the share of the load the drum carries.
    pub fn load_fraction(&self) -> f64 {
        self.tension_n / self.weight_n()
    }

    pub fn hook_m(&self) -> f64 {
        self.hook_m
    }

    pub fn hook_speed_m_s(&self) -> f64 {
        self.hook_speed
    }

    pub fn on_floor(&self) -> bool {
        self.on_floor
    }

    /// Slack rope in metres: how much more rope is paid out than the hook needs.
    pub fn slack_m(&self) -> f64 {
        (self.hook_m - self.drum_prev_m).max(0.0)
    }

    /// Advances the hook by `dt_s` while the drum moves linearly from its previous height to
    /// `drum_m`.
    pub fn step(&mut self, drum_m: f64, dt_s: f64) -> Vec<RopeEvent> {
        let mut events = Vec::new();
        let k = self.cfg.stiffness_n_per_m;
        let omega_n = (k / self.mass_kg).sqrt();
        let n = ((dt_s * omega_n / 0.05).ceil() as usize).clamp(1, 400);
        let ds = dt_s / n as f64;
        let drum0 = self.drum_prev_m;
        let drum_speed = (drum_m - drum0) / dt_s;
        let c = 2.0 * self.cfg.damping_ratio * (k * self.mass_kg).sqrt();
        let weight = self.weight_n();
        let was_on_floor = self.on_floor;
        for i in 1..=n {
            let drum = drum0 + (drum_m - drum0) * i as f64 / n as f64;
            let stretch = drum - self.hook_m;
            let tension = if stretch > 0.0 {
                (k * stretch + c * (drum_speed - self.hook_speed)).max(0.0)
            } else {
                0.0
            };
            let accel = (tension - weight) / self.mass_kg;
            let v_next = self.hook_speed + accel * ds;
            let h_next = self.hook_m + v_next * ds;
            if h_next <= self.cfg.floor_m && v_next <= 0.0 {
                if !self.on_floor && self.hook_speed < -LANDING_REPORT_M_S {
                    events.push(RopeEvent::Landed {
                        speed_m_s: -self.hook_speed,
                    });
                }
                self.hook_m = self.cfg.floor_m;
                self.hook_speed = 0.0;
                self.on_floor = true;
            } else {
                self.hook_m = h_next;
                self.hook_speed = v_next;
                self.on_floor = false;
            }
            self.tension_n = tension;
            if self.hook_m > drum {
                self.max_slack_m = self.max_slack_m.max(self.hook_m - drum);
            } else if self.max_slack_m > 0.0 && tension > 0.0 {
                if self.max_slack_m > SLACK_REPORT_M {
                    events.push(RopeEvent::SlackTaken {
                        slack_m: self.max_slack_m,
                    });
                }
                self.max_slack_m = 0.0;
            }
            if let Some(left) = self.peak_window_s.as_mut() {
                self.peak_n = self.peak_n.max(tension);
                *left -= ds;
            }
        }
        if was_on_floor && !self.on_floor {
            events.push(RopeEvent::LiftOff {
                drum_speed_m_s: drum_speed,
            });
            self.peak_window_s = Some(PEAK_WINDOW_S);
            self.peak_n = self.tension_n;
        }
        if let Some(left) = self.peak_window_s
            && left <= 0.0
        {
            events.push(RopeEvent::TensionPeak {
                newtons: self.peak_n,
                factor: self.peak_n / weight,
            });
            self.peak_window_s = None;
        }
        self.drum_prev_m = drum_m;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.005;

    fn cfg(floor_m: f64) -> RopeConfig {
        RopeConfig {
            floor_m,
            stiffness_n_per_m: 1.0e6,
            damping_ratio: 0.1,
        }
    }

    /// Moves the drum at `v` m/s for `t` seconds and collects the events.
    fn drive(rope: &mut RopeLoad, from: f64, v: f64, t: f64) -> (f64, Vec<RopeEvent>) {
        let mut drum = from;
        let mut ev = Vec::new();
        for _ in 0..(t / DT).round() as usize {
            drum += v * DT;
            ev.extend(rope.step(drum, DT));
        }
        (drum, ev)
    }

    #[test]
    fn a_hanging_load_starts_in_equilibrium_and_stays_there() {
        let mut r = RopeLoad::new(cfg(-5.0), 2000.0).unwrap();
        assert!((r.load_fraction() - 1.0).abs() < 1e-9);
        let (_, ev) = drive(&mut r, 0.0, 0.0, 2.0);
        assert!(ev.is_empty());
        assert!((r.load_fraction() - 1.0).abs() < 1e-3);
        assert!(!r.on_floor());
    }

    #[test]
    fn lowering_onto_the_floor_pays_out_slack_and_unloads_the_drum() {
        let mut r = RopeLoad::new(cfg(-1.0), 2000.0).unwrap();
        let (drum, ev) = drive(&mut r, 0.0, -0.5, 3.0);
        // The drum went 1.5 m down, the hook stopped at the floor 1.0 m down: 0.5 m of slack.
        assert!((drum + 1.5).abs() < 1e-9);
        assert!(r.on_floor());
        assert!(r.tension_n() < 1.0);
        assert!((r.slack_m() - 0.5).abs() < 0.01, "{}", r.slack_m());
        assert!(
            ev.iter().any(
                |e| matches!(e, RopeEvent::Landed { speed_m_s } if (*speed_m_s - 0.5).abs() < 0.05)
            ),
            "{ev:?}"
        );
    }

    #[test]
    fn raising_takes_the_slack_then_lifts_the_load_with_a_tension_peak() {
        let mut r = RopeLoad::new(cfg(-1.0), 2000.0).unwrap();
        let (drum, _) = drive(&mut r, 0.0, -0.5, 3.0);
        let (drum, ev) = drive(&mut r, drum, 0.5, 4.0);
        let slack = ev.iter().find_map(|e| match e {
            RopeEvent::SlackTaken { slack_m } => Some(*slack_m),
            _ => None,
        });
        assert!((slack.unwrap() - 0.5).abs() < 0.02, "{ev:?}");
        assert!(ev.iter().any(|e| matches!(e, RopeEvent::LiftOff { .. })));
        let factor = ev.iter().find_map(|e| match e {
            RopeEvent::TensionPeak { factor, .. } => Some(*factor),
            _ => None,
        });
        // Taking up the load at 0.5 m/s overshoots the weight; a rope is not rigid.
        assert!(factor.unwrap() > 1.1, "{ev:?}");
        // After the swing dies out the drum carries the weight again.
        drive(&mut r, drum, 0.0, 4.0);
        assert!(
            (r.load_fraction() - 1.0).abs() < 0.05,
            "{}",
            r.load_fraction()
        );
    }

    #[test]
    fn a_faster_pick_up_gives_a_larger_peak() {
        let peak = |v: f64| {
            let mut r = RopeLoad::new(cfg(0.0), 2000.0).unwrap();
            let (_, ev) = drive(&mut r, 0.0, v, 3.0);
            ev.iter()
                .find_map(|e| match e {
                    RopeEvent::TensionPeak { factor, .. } => Some(*factor),
                    _ => None,
                })
                .unwrap()
        };
        assert!(peak(0.6) > peak(0.1));
    }

    #[test]
    fn a_floor_above_the_start_and_a_zero_stiffness_are_refused() {
        assert!(RopeLoad::new(cfg(0.5), 100.0).is_err());
        assert!(
            RopeLoad::new(
                RopeConfig {
                    stiffness_n_per_m: 0.0,
                    ..cfg(-1.0)
                },
                100.0
            )
            .is_err()
        );
    }

    #[test]
    fn the_hook_that_starts_on_the_floor_carries_nothing_until_the_rope_is_taut() {
        let r = RopeLoad::new(cfg(0.0), 500.0).unwrap();
        assert!(r.on_floor());
        assert_eq!(r.tension_n(), 0.0);
        assert_eq!(r.slack_m(), 0.0);
    }
}
