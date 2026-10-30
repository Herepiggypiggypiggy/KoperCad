//! The calibration wizard's engine: leave the sticks alone to find the centre, then move them
//! to all corners to find the throw. The host feeds one [`InputState`] per frame; nothing here
//! reads a device or a clock.

use std::collections::BTreeMap;

use super::input::InputState;

/// Samples averaged for the centre.
pub const CENTRE_SAMPLES: u32 = 30;
/// A side counts as reached when the stick has moved this far from the centre.
pub const MIN_THROW: f64 = 0.5;
/// A pair of axes has visited a corner when both are this far from the centre.
pub const CORNER_THROW: f64 = 0.4;
/// The calibrated end is this share of the largest value seen, so full deflection stays
/// reachable on a stick that never quite repeats its maximum.
pub const END_SHARE: f64 = 0.97;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalPhase {
    /// Hands off: averaging the rest values.
    Centre,
    /// Move every stick to all corners.
    Range,
}

/// The result for one axis.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisCal {
    pub input: String,
    pub centre: f64,
    pub low: f64,
    pub high: f64,
}

/// Why the calibration cannot be finished yet.
#[derive(Debug, Clone, PartialEq)]
pub enum CalError {
    /// Still averaging the centre.
    CentreNotReady,
    /// These axes (and which end) were not moved far enough: `(input, "low" or "high")`.
    Incomplete(Vec<(String, &'static str)>),
}

/// How far one axis is.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisProgress {
    pub input: String,
    pub centre: f64,
    pub min: f64,
    pub max: f64,
    pub low_reached: bool,
    pub high_reached: bool,
    /// Triggers rest at one end and have no low side.
    pub one_sided: bool,
}

impl AxisProgress {
    pub fn done(&self) -> bool {
        (self.low_reached || self.one_sided) && self.high_reached
    }
}

#[derive(Debug, Clone)]
struct Track {
    sum: f64,
    n: u32,
    centre: f64,
    min: f64,
    max: f64,
}

/// Collects centre and range for a set of axes.
#[derive(Debug, Clone)]
pub struct Calibrator {
    order: Vec<String>,
    tracks: BTreeMap<String, Track>,
    pairs: Vec<(String, String)>,
    corners: Vec<[bool; 4]>,
    phase: CalPhase,
    samples: u32,
}

fn one_sided(input: &str) -> bool {
    input.ends_with("_trigger")
}

impl Calibrator {
    /// Calibrate these axes (duplicates are ignored).
    pub fn new(axes: &[String]) -> Calibrator {
        let mut order: Vec<String> = Vec::new();
        for a in axes {
            if !order.contains(a) {
                order.push(a.clone());
            }
        }
        let tracks = order
            .iter()
            .map(|a| {
                (
                    a.clone(),
                    Track {
                        sum: 0.0,
                        n: 0,
                        centre: 0.0,
                        min: 0.0,
                        max: 0.0,
                    },
                )
            })
            .collect();
        Calibrator {
            order,
            tracks,
            pairs: Vec::new(),
            corners: Vec::new(),
            phase: CalPhase::Centre,
            samples: 0,
        }
    }

    /// Also track the four corners of a stick (its left-right and up-down axes).
    pub fn with_pair(mut self, x: &str, y: &str) -> Calibrator {
        if self.tracks.contains_key(x) && self.tracks.contains_key(y) {
            self.pairs.push((x.to_owned(), y.to_owned()));
            self.corners.push([false; 4]);
        }
        self
    }

    pub fn phase(&self) -> CalPhase {
        self.phase
    }

    /// The centre has enough samples; [`Self::start_range`] may be called.
    pub fn centre_ready(&self) -> bool {
        self.samples >= CENTRE_SAMPLES
    }

    /// Fraction of the centre samples taken, 0 to 1.
    pub fn centre_fraction(&self) -> f64 {
        (self.samples as f64 / CENTRE_SAMPLES as f64).min(1.0)
    }

    /// Leave the centre phase. Returns false while the centre has too few samples.
    pub fn start_range(&mut self) -> bool {
        if !self.centre_ready() {
            return false;
        }
        for t in self.tracks.values_mut() {
            t.centre = t.sum / t.n.max(1) as f64;
            t.min = t.centre;
            t.max = t.centre;
        }
        self.phase = CalPhase::Range;
        true
    }

    /// Take one frame. A disconnected device adds nothing.
    pub fn feed(&mut self, input: &InputState) {
        if !input.connected {
            return;
        }
        match self.phase {
            CalPhase::Centre => {
                if self.samples < CENTRE_SAMPLES {
                    self.samples += 1;
                    for (name, t) in &mut self.tracks {
                        t.sum += input.axis(name);
                        t.n += 1;
                    }
                }
            }
            CalPhase::Range => {
                for (name, t) in &mut self.tracks {
                    let v = input.axis(name);
                    t.min = t.min.min(v);
                    t.max = t.max.max(v);
                }
                for (i, (x, y)) in self.pairs.iter().enumerate() {
                    let (cx, cy) = (self.tracks[x].centre, self.tracks[y].centre);
                    let (dx, dy) = (input.axis(x) - cx, input.axis(y) - cy);
                    if dx.abs() >= CORNER_THROW && dy.abs() >= CORNER_THROW {
                        let q = match (dx > 0.0, dy > 0.0) {
                            (true, true) => 0,
                            (false, true) => 1,
                            (false, false) => 2,
                            (true, false) => 3,
                        };
                        self.corners[i][q] = true;
                    }
                }
            }
        }
    }

    /// Per axis, in the order given to [`Self::new`].
    pub fn progress(&self) -> Vec<AxisProgress> {
        self.order
            .iter()
            .map(|a| {
                let t = &self.tracks[a];
                let centre = if self.phase == CalPhase::Centre {
                    t.sum / t.n.max(1) as f64
                } else {
                    t.centre
                };
                AxisProgress {
                    input: a.clone(),
                    centre,
                    min: t.min,
                    max: t.max,
                    low_reached: self.phase == CalPhase::Range && centre - t.min >= MIN_THROW,
                    high_reached: self.phase == CalPhase::Range && t.max - centre >= MIN_THROW,
                    one_sided: one_sided(a),
                }
            })
            .collect()
    }

    /// The corners visited so far, per stick pair: counter-clockwise from upper right.
    pub fn corners(&self) -> Vec<(&str, &str, [bool; 4])> {
        self.pairs
            .iter()
            .zip(&self.corners)
            .map(|((x, y), c)| (x.as_str(), y.as_str(), *c))
            .collect()
    }

    /// Whether every axis has been moved far enough on both sides, and every pair has visited
    /// all four corners.
    pub fn complete(&self) -> bool {
        self.phase == CalPhase::Range
            && self.progress().iter().all(AxisProgress::done)
            && self.corners.iter().all(|c| c.iter().all(|&b| b))
    }

    /// The calibration. Needs the range phase and every axis moved at least [`MIN_THROW`] to
    /// each of its sides (a trigger only to its pressed side). The corners are guidance only.
    pub fn finish(&self) -> Result<Vec<AxisCal>, CalError> {
        if self.phase != CalPhase::Range {
            return Err(CalError::CentreNotReady);
        }
        let mut missing = Vec::new();
        let mut out = Vec::new();
        for p in self.progress() {
            if !p.low_reached && !p.one_sided {
                missing.push((p.input.clone(), "low"));
            }
            if !p.high_reached {
                missing.push((p.input.clone(), "high"));
            }
            let low = if p.one_sided {
                p.centre - 1.0
            } else {
                p.centre - (p.centre - p.min) * END_SHARE
            };
            out.push(AxisCal {
                input: p.input.clone(),
                centre: p.centre,
                low,
                high: p.centre + (p.max - p.centre) * END_SHARE,
            });
        }
        if missing.is_empty() {
            Ok(out)
        } else {
            Err(CalError::Incomplete(missing))
        }
    }
}
