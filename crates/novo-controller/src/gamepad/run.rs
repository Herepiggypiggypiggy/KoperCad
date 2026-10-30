//! The mapper: one device state in, holds and presses for one controller out.
//!
//! Call [`PadMapper::update`] once per frame with the current [`InputState`]. The result says,
//! per control, which position the pad holds (or `None` for let go), which one-shot actions to
//! apply (E-stop press, toggles, selector steps), and whether the radio link should drop or come
//! back. The host applies holds like a held mouse button or key, so the controller keeps all its
//! own rules: interlocks, E-stop latch, `requires` conditions, link effects.

use std::collections::{BTreeMap, BTreeSet};

use crate::engine::Controller;
use crate::panel::stick_axis;
use crate::profile::{ControlDef, ControlKind, Profile};
use crate::sequence::Action;

use super::input::InputState;
use super::map::{AxisMap, ButtonAction, GamepadMap, LinkLoss};

/// What the pad asks of the controller this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PadOutput {
    pub connected: bool,
    /// Every control the map holds with a spring: the position held, or `None` to let go.
    pub holds: BTreeMap<String, Option<String>>,
    /// One-shot actions, in a fixed order.
    pub actions: Vec<Action>,
    /// `Some(false)`: drop the radio link now; `Some(true)`: it comes back.
    pub link: Option<bool>,
    /// The dead-man input is held (always true when the map has none).
    pub gate_open: bool,
    /// Per axis map, for the monitor.
    pub axes: Vec<AxisReading>,
}

/// One axis as the monitor shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisReading {
    pub input: String,
    pub control: String,
    pub raw: f64,
    /// Calibrated and inverted, -1 to +1.
    pub value: f64,
    /// Step beyond the deadzone (negative on the negative side); 0 is rest.
    pub step: i8,
    /// The position the pad is asking for, if any.
    pub position: Option<String>,
    /// Held back until the stick has been at rest once.
    pub waiting_for_neutral: bool,
    /// Held back by the dead-man gate.
    pub gated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Presence {
    #[default]
    Unknown,
    Up,
    Down,
}

#[derive(Debug, Clone, Default)]
struct AxisRun {
    step: i8,
    wait: bool,
    /// Selector band, or the last position set on a control without spring.
    band: Option<usize>,
    last_set: Option<String>,
}

/// The state the mapper keeps between frames.
#[derive(Debug, Clone, Default)]
pub struct PadMapper {
    presence: Presence,
    link_dropped: bool,
    prev: BTreeMap<String, bool>,
    blocked: BTreeSet<String>,
    axes: BTreeMap<(String, String), AxisRun>,
    /// `hands_off` was called since the last update, and whether the last update was such a frame.
    away: bool,
    was_away: bool,
}

/// Rounding slack when a value sits exactly on a step edge (`0.2 + 0.4` is not `0.6` in floats).
const EDGE_SLACK: f64 = 1e-9;

/// The step (0 = rest, negative on the negative side) for a calibrated value `v` in -1..1.
///
/// The deadzone is rest. Beyond it the remaining throw is cut into equal bands, one per step of
/// that side, so a two-step rocker has step 1 up to the middle of the throw and step 2 from there
/// on. A step is left again only when the value has fallen `hysteresis` below the edge that
/// entered it. Pure, for tests and for the monitor.
pub fn step_of(prev: i8, v: f64, deadzone: f64, hysteresis: f64, n_neg: u8, n_pos: u8) -> i8 {
    let v = if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    // At exactly zero the side stays the one the stick was on.
    let side = if v < 0.0 || (v == 0.0 && prev < 0) {
        -1
    } else {
        1
    };
    let (n, prev_side) = if side > 0 {
        (n_pos, prev.max(0) as u8)
    } else {
        (n_neg, (-(prev.min(0) as i16)) as u8)
    };
    if n == 0 {
        return 0;
    }
    let m = v.abs();
    let dz = deadzone.clamp(0.0, 0.95);
    let h = hysteresis.clamp(0.0, 0.5);
    // Lower edge of step k (k >= 1) in value units; step 1 starts at the deadzone.
    let edge = |k: u8| dz + (k as f64 - 1.0) / n as f64 * (1.0 - dz);
    let mut step = prev_side.min(n);
    // Upwards: step 0 to 1 needs to be beyond the deadzone, the others reach their edge.
    while step < n {
        let next = step + 1;
        let enter = if next == 1 {
            m > dz
        } else {
            m + EDGE_SLACK >= edge(next)
        };
        if enter {
            step = next;
        } else {
            break;
        }
    }
    // Downwards: fall back below the edge by the hysteresis (step 1: below the deadzone).
    while step >= 1 {
        let leave = if step == 1 {
            m <= (dz - h).max(0.0)
        } else {
            m + EDGE_SLACK < edge(step) - h
        };
        if leave {
            step -= 1;
        } else {
            break;
        }
    }
    (step as i8) * side as i8
}

/// The band of a selector for `v` in -1..1 among `n` positions, with hysteresis `h`.
pub fn band_of(prev: Option<usize>, v: f64, n: usize, h: f64) -> usize {
    if n == 0 {
        return 0;
    }
    let t = ((if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }) + 1.0)
        / 2.0;
    let raw = ((t * n as f64).floor() as usize).min(n - 1);
    match prev {
        Some(p) if p < n => {
            let margin = h.clamp(0.0, 0.5) / 2.0;
            let (lo, hi) = (
                p as f64 / n as f64 - margin,
                (p + 1) as f64 / n as f64 + margin,
            );
            if t >= lo && t <= hi { p } else { raw }
        }
        _ => raw,
    }
}

fn offer(holds: &mut BTreeMap<String, Option<String>>, control: &str, position: Option<String>) {
    match holds.get_mut(control) {
        None => {
            holds.insert(control.to_owned(), position);
        }
        Some(slot) if slot.is_none() && position.is_some() => *slot = position,
        Some(_) => {}
    }
}

/// The E-stop the pad should press: the first one a button of the map presses, else the first
/// of the profile.
fn estop_control(map: &GamepadMap, profile: &Profile) -> Option<String> {
    map.buttons
        .iter()
        .find(|b| b.action == ButtonAction::Estop && profile.control(&b.control).is_some())
        .map(|b| b.control.clone())
        .or_else(|| {
            profile
                .controls
                .iter()
                .find(|c| c.kind == ControlKind::EStop)
                .map(|c| c.id.clone())
        })
}

/// The values at which steps 1 to `n` begin on one side (the deadzone, then the equal bands
/// beyond it); for drawing ticks next to a live stick value.
pub fn step_edges(deadzone: f64, n: u8) -> Vec<f64> {
    let dz = deadzone.clamp(0.0, 0.95);
    (1..=n)
        .map(|k| dz + (k as f64 - 1.0) / n as f64 * (1.0 - dz))
        .collect()
}

/// Detent counts `(negative, positive)` of a rocker or axis control.
pub fn detent_counts(def: &ControlDef) -> (u8, u8) {
    let s = stick_axis(def);
    let neg = s.rest;
    let pos = s.detents.len().saturating_sub(s.rest + 1);
    (neg.min(250) as u8, pos.min(250) as u8)
}

impl PadMapper {
    pub fn new() -> PadMapper {
        PadMapper::default()
    }

    /// Forget everything: the next connected frame is a fresh connect.
    pub fn reset(&mut self) {
        *self = PadMapper::default();
    }

    /// The host takes the pad away from the controller for a while (window in the background,
    /// calibration) and feeds a pad at rest. Call it for each such frame: when real input comes
    /// back, a deflected stick or held button must first return to rest, like after a connect.
    pub fn hands_off(&mut self) {
        self.away = true;
    }

    /// Whether the last frame saw a connected device.
    pub fn is_connected(&self) -> bool {
        self.presence == Presence::Up
    }

    pub fn update(&mut self, map: &GamepadMap, ctl: &Controller, input: &InputState) -> PadOutput {
        let profile = ctl.profile();
        let mut out = PadOutput {
            connected: input.connected,
            ..PadOutput::default()
        };
        if !input.connected {
            self.lose(map, profile, &mut out);
            return out;
        }
        let away = std::mem::take(&mut self.away);
        let fresh = self.presence != Presence::Up || (self.was_away && !away);
        self.was_away = away;
        if fresh {
            if self.link_dropped {
                out.link = Some(true);
                self.link_dropped = false;
            }
            self.presence = Presence::Up;
            self.axes.clear();
            self.blocked.clear();
            self.prev.clear();
        }

        // The dead-man gate.
        let gate_open = match &map.deadman {
            Some(d) => input.is_down(&d.input),
            None => true,
        };
        out.gate_open = gate_open;
        if let Some(d) = &map.deadman
            && let Some(c) = &d.control
            && let Some(def) = profile.control(c)
        {
            let pos = if gate_open {
                def.first_active_position()
            } else {
                None
            };
            offer(&mut out.holds, c, pos);
        }

        // Buttons seen before this frame, to find press edges.
        let was = |this: &Self, name: &str, now: bool| {
            if fresh {
                now
            } else {
                this.prev.get(name).copied().unwrap_or(now)
            }
        };

        for a in &map.axes {
            self.axis(a, ctl, input, gate_open, fresh, map, &mut out);
        }

        let mut now_down: BTreeMap<String, bool> = BTreeMap::new();
        for b in &map.buttons {
            let Some(def) = profile.control(&b.control) else {
                continue;
            };
            let down = input.is_down(&b.input);
            now_down.insert(b.input.clone(), down);
            let edge = down && !was(self, &b.input, down);
            match b.action {
                ButtonAction::Hold => {
                    let mut effective = down;
                    if map.neutral_required && b.gated {
                        if !down {
                            self.blocked.remove(&b.input);
                        } else if fresh || !gate_open {
                            self.blocked.insert(b.input.clone());
                        }
                        effective = down && !self.blocked.contains(&b.input);
                    }
                    if b.gated && !gate_open {
                        effective = false;
                    }
                    let pos = if effective {
                        b.position.clone().or_else(|| def.first_active_position())
                    } else {
                        None
                    };
                    offer(&mut out.holds, &b.control, pos);
                }
                ButtonAction::Toggle if edge => out.actions.push(Action::Press {
                    control: b.control.clone(),
                }),
                ButtonAction::Estop if edge => out.actions.push(Action::Press {
                    control: b.control.clone(),
                }),
                ButtonAction::Reset if edge => out.actions.push(Action::Reset {
                    control: b.control.clone(),
                }),
                ButtonAction::Set if edge => {
                    if let Some(p) = &b.position {
                        out.actions.push(Action::SetPosition {
                            control: b.control.clone(),
                            position: p.clone(),
                        });
                    }
                }
                ButtonAction::Next | ButtonAction::Prev if edge => {
                    let list = def.position_list();
                    if let Some(at) = ctl
                        .position(&b.control)
                        .and_then(|p| list.iter().position(|q| q == p))
                    {
                        let to = if b.action == ButtonAction::Next {
                            (at + 1).min(list.len() - 1)
                        } else {
                            at.saturating_sub(1)
                        };
                        if to != at {
                            out.actions.push(Action::SetPosition {
                                control: b.control.clone(),
                                position: list[to].clone(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        // Remember every mapped input, including the dead-man.
        if let Some(d) = &map.deadman {
            now_down.insert(d.input.clone(), input.is_down(&d.input));
        }
        self.prev = now_down;
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn axis(
        &mut self,
        a: &AxisMap,
        ctl: &Controller,
        input: &InputState,
        gate_open: bool,
        fresh: bool,
        map: &GamepadMap,
        out: &mut PadOutput,
    ) {
        let profile = ctl.profile();
        let Some(def) = profile.control(&a.control) else {
            return;
        };
        let raw = input.axis(&a.input);
        let v = a.value(raw);
        let key = (a.input.clone(), a.control.clone());
        let run = self.axes.entry(key).or_insert_with(|| AxisRun {
            wait: map.neutral_required,
            ..AxisRun::default()
        });
        match def.kind {
            ControlKind::Selector => {
                let list = def.position_list();
                let band = band_of(run.band, v, list.len(), a.hysteresis);
                let changed = run.band.is_some_and(|b| b != band);
                if run.band.is_none() || changed {
                    run.band = Some(band);
                }
                if changed && !fresh {
                    out.actions.push(Action::SetPosition {
                        control: a.control.clone(),
                        position: list[band].clone(),
                    });
                }
                out.axes.push(AxisReading {
                    input: a.input.clone(),
                    control: a.control.clone(),
                    raw,
                    value: v,
                    step: 0,
                    position: list.get(band).cloned(),
                    waiting_for_neutral: false,
                    gated: false,
                });
            }
            ControlKind::Rocker2 | ControlKind::Rocker3 | ControlKind::Axis => {
                let (n_neg, n_pos) = detent_counts(def);
                let step = step_of(run.step, v, a.deadzone, a.hysteresis, n_neg, n_pos);
                run.step = step;
                let gated = a.gated && !gate_open;
                if map.neutral_required && gated {
                    run.wait = true;
                }
                if step == 0 && !gated {
                    run.wait = false;
                }
                let held_back = gated || run.wait;
                let s = stick_axis(def);
                let wanted = if step == 0 || held_back {
                    None
                } else if step > 0 {
                    s.detents.get(s.rest + step as usize).cloned()
                } else {
                    s.rest
                        .checked_sub((-step) as usize)
                        .and_then(|i| s.detents.get(i).cloned())
                };
                if def.is_spring() {
                    offer(&mut out.holds, &a.control, wanted.clone());
                } else {
                    // Without a spring, letting go leaves the control where it is: set the
                    // rest position explicitly, and only when what is wanted changes.
                    let target = wanted.clone().unwrap_or_else(|| def.rest_position());
                    if run.last_set.as_deref() != Some(target.as_str()) {
                        if run.last_set.is_some() || wanted.is_some() {
                            out.actions.push(Action::SetPosition {
                                control: a.control.clone(),
                                position: target.clone(),
                            });
                        }
                        run.last_set = Some(target);
                    }
                }
                out.axes.push(AxisReading {
                    input: a.input.clone(),
                    control: a.control.clone(),
                    raw,
                    value: v,
                    step,
                    position: wanted,
                    waiting_for_neutral: run.wait && !gated,
                    gated,
                });
            }
            _ => {}
        }
    }

    /// The device is gone (or was never there): let go of everything, once apply the policy.
    fn lose(&mut self, map: &GamepadMap, profile: &Profile, out: &mut PadOutput) {
        let transition = self.presence == Presence::Up;
        if let Some(d) = &map.deadman
            && let Some(c) = &d.control
        {
            offer(&mut out.holds, c, None);
        }
        for a in &map.axes {
            if let Some(def) = profile.control(&a.control)
                && matches!(
                    def.kind,
                    ControlKind::Rocker2 | ControlKind::Rocker3 | ControlKind::Axis
                )
            {
                if def.is_spring() {
                    offer(&mut out.holds, &a.control, None);
                } else if transition {
                    out.actions.push(Action::SetPosition {
                        control: a.control.clone(),
                        position: def.rest_position(),
                    });
                }
            }
        }
        for b in &map.buttons {
            if b.action == ButtonAction::Hold && profile.control(&b.control).is_some() {
                offer(&mut out.holds, &b.control, None);
            }
        }
        if transition {
            match map.on_disconnect {
                LinkLoss::Release => {}
                LinkLoss::DropLink => {
                    out.link = Some(false);
                    self.link_dropped = true;
                }
                LinkLoss::Estop => {
                    if let Some(c) = estop_control(map, profile) {
                        out.actions.push(Action::Press { control: c });
                    }
                }
            }
        }
        self.presence = Presence::Down;
        self.axes.clear();
        self.blocked.clear();
        self.prev.clear();
    }
}
