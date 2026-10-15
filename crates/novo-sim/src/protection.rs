//! Protective devices in time.
//!
//! The time-current *curves* (IEC 60898-1 B/C/D bands, IEC 60947-2, gG/aM
//! fuses, overload relay classes) belong to the protection model of
//! novo-checks; this module consumes them through [`TripModel`] and adds what
//! a static curve cannot say: what happens when the current *changes*.
//!
//! A real thermal element does not reset when the current dips. The standard
//! way to follow a varying current with a curve is cumulative damage
//! (Miner's rule): every interval at constant current `I` uses up
//! `dt / t_trip(I)` of the device's life, and the device trips when the sum
//! reaches one. Below the pickup current the sum decays with the cooling time
//! constant. That reproduces the behaviour you see on a bench: a motor start
//! that rides through at cold, the same start tripping a warm relay, and a
//! fault that arrives while an overload is already heating the breaker.
//!
//! A curve is a *band*: the same device trips anywhere between `t_min` and
//! `t_max` depending on tolerance and temperature. [`Band`] picks the point in
//! the band, so a scenario can be run at the fast edge (does this trip when it
//! must not?), the slow edge (does this still protect the cable?) and the
//! middle.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A time-current characteristic: how long the device takes to open at a
/// constant rms current. Implemented by the protection model of novo-checks;
/// [`TableCurve`] covers a manufacturer's printed curve.
pub trait TripModel: fmt::Debug {
    /// `(t_min, t_max)` seconds for a constant current of `i` A: the device
    /// does not trip before `t_min` and has tripped by `t_max`. Both are
    /// infinite below the pickup current; `t_max` alone is infinite in the
    /// gap where the device may or may not trip.
    fn trip_time_range(&self, i: f64) -> (f64, f64);

    /// Longest time to clear a prospective current (the latest the device
    /// opens), as novo-checks names it.
    fn clears(&self, ik: f64) -> f64 {
        self.trip_time_range(ik).1
    }

    /// Let-through energy `∫i²dt` in A²s at a prospective current, for
    /// current-limiting devices; `None` when not known.
    fn let_through_i2t(&self, _ik: f64) -> Option<f64> {
        None
    }
}

/// Where in the tolerance band a run places every trip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// The earliest the device may trip (tests nuisance tripping).
    Fastest,
    /// The geometric middle of the band.
    Typical,
    /// The latest it may take (tests whether the cable is still protected).
    Slowest,
    /// A fraction 0..=1 from fastest to slowest, linear in log time.
    At(f64),
}

impl Band {
    pub fn fraction(self) -> f64 {
        match self {
            Band::Fastest => 0.0,
            Band::Typical => 0.5,
            Band::Slowest => 1.0,
            Band::At(f) => f.clamp(0.0, 1.0),
        }
    }

    /// The trip time of `model` at `i` for this band; infinite when the
    /// device does not trip there.
    pub fn time(self, model: &dyn TripModel, i: f64) -> f64 {
        let (lo, hi) = model.trip_time_range(i);
        let p = self.fraction();
        if !lo.is_finite() {
            return f64::INFINITY;
        }
        if p == 0.0 || lo == hi {
            return lo;
        }
        if !hi.is_finite() {
            // In the gap the device may hold forever: only the fast edge
            // trips.
            return f64::INFINITY;
        }
        (lo.ln() + p * (hi.ln() - lo.ln())).exp()
    }
}

/// A printed curve: `(multiple of In, t_min s, t_max s)` points, joined
/// straight in log-log space. Below the first point the device holds, above
/// the last it keeps the last time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCurve {
    pub rated_a: f64,
    pub points: Vec<(f64, f64, f64)>,
}

impl TableCurve {
    pub fn new(rated_a: f64, points: Vec<(f64, f64, f64)>) -> Self {
        let mut points = points;
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { rated_a, points }
    }
}

impl TripModel for TableCurve {
    fn trip_time_range(&self, i: f64) -> (f64, f64) {
        let m = i / self.rated_a;
        let (Some(first), Some(last)) = (self.points.first(), self.points.last()) else {
            return (f64::INFINITY, f64::INFINITY);
        };
        // `!(m >= ..)` also catches NaN (a device carrying 0 A with a bad
        // rating) and negative currents: the device holds.
        if m.is_nan() || m < first.0 || !self_ok(first) {
            return (f64::INFINITY, f64::INFINITY);
        }
        if m >= last.0 {
            return (last.1, last.2);
        }
        let hi = self.points.partition_point(|p| p.0 <= m);
        let (a, b) = (self.points[hi - 1], self.points[hi]);
        let f = (m.ln() - a.0.ln()) / (b.0.ln() - a.0.ln());
        let lerp = |x: f64, y: f64| (x.ln() + f * (y.ln() - x.ln())).exp();
        (lerp(a.1, b.1), lerp(a.2, b.2))
    }
}

/// A point the log-log interpolation can use.
fn self_ok(p: &(f64, f64, f64)) -> bool {
    p.0 > 0.0 && p.1 > 0.0 && p.2 > 0.0 && p.0.is_finite()
}

/// One device in a run: a curve plus how it behaves in time.
#[derive(Debug)]
pub struct ProtectiveDevice {
    /// Key the circuit model and scenario files refer to it by.
    pub key: String,
    /// What the drawing calls it (`-F1`, `-Q3`).
    pub label: String,
    pub model: Box<dyn TripModel>,
    pub band: Band,
    /// Seconds from the trip decision until the contacts have parted (a
    /// miniature breaker 5 to 10 ms; zero for a fuse, whose melting time
    /// already includes the arc).
    pub opening_time: f64,
    /// Time constant of the cooling when the current is below pickup.
    pub cool_tau: f64,
    /// Fraction of thermal life already used when the run starts: 0 cold, a
    /// relay that has been carrying its rated current for hours is around
    /// 0.5 to 1.
    pub initial_heat: f64,
}

impl ProtectiveDevice {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        model: impl TripModel + 'static,
    ) -> Self {
        let key = key.into();
        Self {
            label: label.into(),
            key,
            model: Box::new(model),
            band: Band::Typical,
            opening_time: 0.010,
            cool_tau: 600.0,
            initial_heat: 0.0,
        }
    }

    pub fn with_band(mut self, band: Band) -> Self {
        self.band = band;
        self
    }

    pub fn with_opening_time(mut self, seconds: f64) -> Self {
        self.opening_time = seconds;
        self
    }

    pub fn with_initial_heat(mut self, fraction: f64) -> Self {
        self.initial_heat = fraction.max(0.0);
        self
    }
}

/// The accumulated heat of one device (fraction of its life used).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ThermalMemory {
    pub heat: f64,
}

impl ThermalMemory {
    /// Seconds until the device trips if the current stays at `i`; infinite
    /// when it never does.
    pub fn time_to_trip(&self, device: &ProtectiveDevice, i: f64) -> f64 {
        let t = device.band.time(device.model.as_ref(), i);
        if t.is_finite() {
            ((1.0 - self.heat) * t).max(0.0)
        } else {
            f64::INFINITY
        }
    }

    /// Lets `dt` seconds pass at a constant current. The caller never steps
    /// beyond [`Self::time_to_trip`], so the heat stays at or below one.
    pub fn advance(&mut self, device: &ProtectiveDevice, i: f64, dt: f64) {
        let t = device.band.time(device.model.as_ref(), i);
        if t.is_finite() && t > 0.0 {
            self.heat = (self.heat + dt / t).min(1.0);
        } else {
            self.heat *= (-dt / device.cool_tau).exp();
        }
    }
}
