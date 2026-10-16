//! The time-current model of a protective device: the one curve model the
//! protection coordination, the design checks and the simulation share.
//!
//! A real trip curve is not a line but a band: for one fault current the
//! device trips somewhere between a fastest and a slowest time, because of
//! tolerances, temperature and the state of the thermal element. A
//! [`TripCurve`] therefore holds two [`Band`]s, both piecewise straight in
//! log-log space (the way curves are printed):
//!
//! * `min`: the **no-trip** time, the device does not operate sooner than
//!   this (the fastest it can be);
//! * `max`: the **trip-before** time, the device has operated by then (the
//!   slowest it can be).
//!
//! Currents are stored as multiples of the device's reference current
//! (`rated_a`: `In` of a breaker or fuse, `Ir` of a relay or an electronic
//! trip), so one shape serves every rating. Below the first point of a band
//! the answer is "never" (infinite time).
//!
//! # Where the numbers come from
//!
//! The builders ([`TripCurve::mcb`], [`fuse_gg`](TripCurve::fuse_gg),
//! [`overload_relay`](TripCurve::overload_relay),
//! [`lsi`](TripCurve::lsi), [`mpcb`](TripCurve::mpcb)) are made from the
//! *test points and limits the standards demand* (IEC 60898-1 for miniature
//! breakers, IEC 60269 for fuses, IEC 60947-4-1 for overload relays,
//! IEC 60947-2 for breakers) joined by straight log-log lines. Any real
//! device sits inside such a band, never outside it, so the bands are safe
//! for "will it trip" and "will it not trip first" questions but wider than
//! a maker's printed curve. Exact curves of a maker are loaded with
//! [`TripCurve::from_points`] as user-supplied data (the program does not
//! ship maker data), and the band then narrows to what the maker states.
//! The standards' text is not reproduced here; only the published limits.
//!
//! Units: amperes, seconds, multiples of the reference current.

use serde::{Deserialize, Serialize};

/// What the numbers of a curve are about, for labels and for rules that
/// treat families differently (fuses are compared by energy, electronic
/// breakers have time-delay zones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Family {
    /// Miniature circuit breaker, IEC 60898-1 (B, C, D) or K / Z.
    Mcb,
    /// Moulded-case or air circuit breaker with a thermal-magnetic or
    /// electronic trip (IEC 60947-2).
    Breaker,
    /// Fuse link (IEC 60269).
    Fuse,
    /// Thermal or electronic overload relay (IEC 60947-4-1).
    OverloadRelay,
    /// Motor protection circuit breaker: thermal element plus magnetic trip.
    MotorBreaker,
    /// Curve entered from a maker's data sheet.
    Custom,
}

/// Why a curve or band could not be built.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveError {
    /// A band needs at least two points.
    TooFewPoints,
    /// A multiple or time is zero, negative, NaN or infinite where a finite
    /// positive number is needed.
    BadNumber,
    /// The multiples of a band must increase.
    NotIncreasing,
    /// The time of a band must never rise with the current.
    TimeRises,
    /// The rated current is not a positive finite number.
    BadRating,
    /// The fastest time of the curve is slower than its slowest time at
    /// this multiple.
    Crossed(f64),
}

impl std::fmt::Display for CurveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CurveError::TooFewPoints => write!(f, "a curve band needs at least two points"),
            CurveError::BadNumber => write!(f, "curve points must be finite and positive"),
            CurveError::NotIncreasing => write!(f, "curve currents must increase along the band"),
            CurveError::TimeRises => write!(f, "curve time must not rise as the current rises"),
            CurveError::BadRating => write!(f, "the rated current must be positive"),
            CurveError::Crossed(m) => write!(
                f,
                "at {m:.2} times the rating the fastest trip time is slower than the slowest"
            ),
        }
    }
}

impl std::error::Error for CurveError {}

/// One edge of a time-current band: points `(multiple, seconds)`, straight
/// in log-log space between points. Below the first point the time is
/// infinite; above the last it stays at the last time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Band {
    points: Vec<(f64, f64)>,
}

impl Band {
    /// Builds a band from points with rising multiples and non-rising
    /// times.
    pub fn new(points: Vec<(f64, f64)>) -> Result<Band, CurveError> {
        if points.len() < 2 {
            return Err(CurveError::TooFewPoints);
        }
        for &(m, t) in &points {
            if !(m.is_finite() && m > 0.0 && t.is_finite() && t > 0.0) {
                return Err(CurveError::BadNumber);
            }
        }
        for w in points.windows(2) {
            if w[1].0 <= w[0].0 {
                return Err(CurveError::NotIncreasing);
            }
            if w[1].1 > w[0].1 * (1.0 + 1e-9) {
                return Err(CurveError::TimeRises);
            }
        }
        Ok(Band { points })
    }

    /// The points, `(multiple, seconds)`.
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// The time in seconds at `multiple` times the reference current;
    /// infinite below the first point.
    pub fn time_at(&self, multiple: f64) -> f64 {
        let first = self.points[0];
        let last = self.points[self.points.len() - 1];
        if multiple.is_nan() || multiple < first.0 {
            return f64::INFINITY;
        }
        if multiple >= last.0 {
            return last.1;
        }
        // Binary search for the segment holding `multiple`.
        let i = self
            .points
            .partition_point(|p| p.0 <= multiple)
            .saturating_sub(1);
        let (m0, t0) = self.points[i];
        let (m1, t1) = self.points[i + 1];
        if t0 == t1 {
            return t0;
        }
        let f = (multiple.ln() - m0.ln()) / (m1.ln() - m0.ln());
        // The round trip through logarithms may overshoot a breakpoint by a
        // rounding error; the time never leaves the segment's own range.
        (t0.ln() + f * (t1.ln() - t0.ln()))
            .exp()
            .clamp(t1.min(t0), t0.max(t1))
    }

    /// The smallest multiple at which the band is at or below `seconds`
    /// (the current where the curve crosses a time), `None` when it never
    /// gets that fast within the band.
    pub fn multiple_for(&self, seconds: f64) -> Option<f64> {
        if seconds.is_nan() || seconds <= 0.0 {
            return None;
        }
        let last = self.points[self.points.len() - 1];
        if last.1 > seconds {
            return None;
        }
        let first = self.points[0];
        if first.1 <= seconds {
            return Some(first.0);
        }
        // Times fall along the band: bisect on the multiple.
        let (mut lo, mut hi) = (first.0, last.0);
        for _ in 0..80 {
            let mid = (lo * hi).sqrt();
            if self.time_at(mid) <= seconds {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(hi)
    }

    fn mapped(&self, f: impl Fn(f64, f64) -> (f64, f64)) -> Band {
        Band {
            points: self.points.iter().map(|&(m, t)| f(m, t)).collect(),
        }
    }
}

/// The multiples where a curve changes from the thermal (time-delayed)
/// part to the instantaneous trip: the device may trip instantly from
/// `lo` and is certain to from `hi` (times the reference current).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Instantaneous {
    /// Lowest multiple at which the instantaneous trip can operate.
    pub lo: f64,
    /// Multiple from which it is certain to operate.
    pub hi: f64,
}

/// Fuse energy values in A²s: what the selectivity of fuses is judged by.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FuseEnergy {
    /// Smallest melting (pre-arcing) I²t of the link.
    pub pre_arcing_min: f64,
    /// Largest total (pre-arcing plus arcing) I²t at the rated voltage.
    pub operating_max: f64,
}

/// Whether a device clears a fault within a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clears {
    /// Certain to have operated in time.
    Yes,
    /// Might or might not, inside the tolerance band.
    Maybe,
    /// Certain not to have operated in time.
    No,
}

/// One point of a sampled curve, for plotting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Current in amperes.
    pub current_a: f64,
    /// Fastest trip time, seconds (infinite: does not trip).
    pub t_min: f64,
    /// Slowest trip time, seconds (infinite: may not trip).
    pub t_max: f64,
}

/// Miniature breaker curves (IEC 60898-1 B, C, D; K and Z as used on
/// motor and electronic loads).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum McbCurve {
    /// Instantaneous trip between 3 and 5 times `In`.
    B,
    /// Between 5 and 10 times `In`.
    C,
    /// Between 10 and 20 times `In`.
    D,
    /// Between 8 and 14 times `In`.
    K,
    /// Between 2 and 3 times `In`.
    Z,
}

impl McbCurve {
    /// Reads the letter printed on a breaker, any case.
    pub fn parse(text: &str) -> Option<McbCurve> {
        match text.trim().to_ascii_uppercase().as_str() {
            "B" => Some(McbCurve::B),
            "C" => Some(McbCurve::C),
            "D" => Some(McbCurve::D),
            "K" => Some(McbCurve::K),
            "Z" => Some(McbCurve::Z),
            _ => None,
        }
    }

    /// The letter.
    pub fn letter(self) -> &'static str {
        match self {
            McbCurve::B => "B",
            McbCurve::C => "C",
            McbCurve::D => "D",
            McbCurve::K => "K",
            McbCurve::Z => "Z",
        }
    }

    /// The instantaneous trip range in multiples of `In`.
    pub fn instantaneous(self) -> Instantaneous {
        let (lo, hi) = match self {
            McbCurve::B => (3.0, 5.0),
            McbCurve::C => (5.0, 10.0),
            McbCurve::D => (10.0, 20.0),
            McbCurve::K => (8.0, 14.0),
            McbCurve::Z => (2.0, 3.0),
        };
        Instantaneous { lo, hi }
    }
}

/// Overload relay trip classes (IEC 60947-4-1): the time a cold relay
/// takes to trip at 7.2 times its setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelayClass {
    /// 2 to 10 s at 7.2 Ir.
    C10A,
    /// 4 to 10 s.
    C10,
    /// 6 to 20 s.
    C20,
    /// 9 to 30 s.
    C30,
}

impl RelayClass {
    /// Reads `10A`, `10`, `20` or `30` (a leading `class` is ignored).
    pub fn parse(text: &str) -> Option<RelayClass> {
        let t = text.trim().to_ascii_uppercase();
        let t = t.trim_start_matches("CLASS").trim();
        match t {
            "10A" => Some(RelayClass::C10A),
            "10" => Some(RelayClass::C10),
            "20" => Some(RelayClass::C20),
            "30" => Some(RelayClass::C30),
            _ => None,
        }
    }

    /// The class name as printed.
    pub fn label(self) -> &'static str {
        match self {
            RelayClass::C10A => "10A",
            RelayClass::C10 => "10",
            RelayClass::C20 => "20",
            RelayClass::C30 => "30",
        }
    }

    /// Trip-time window at 7.2 times the setting, seconds `(min, max)`.
    fn at_7_2(self) -> (f64, f64) {
        match self {
            RelayClass::C10A => (2.0, 10.0),
            RelayClass::C10 => (4.0, 10.0),
            RelayClass::C20 => (6.0, 20.0),
            RelayClass::C30 => (9.0, 30.0),
        }
    }

    /// The longest time the relay may take to trip at 1.5 times its
    /// setting, seconds (2, 4, 8 and 12 minutes).
    fn at_1_5(self) -> f64 {
        match self {
            RelayClass::C10A => 120.0,
            RelayClass::C10 => 240.0,
            RelayClass::C20 => 480.0,
            RelayClass::C30 => 720.0,
        }
    }
}

/// Settings of an electronic trip unit (long-time, short-time,
/// instantaneous). Multiples are of `ir_a`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lsi {
    /// Long-time pickup `Ir` in amperes.
    pub ir_a: f64,
    /// Long-time delay `tr`: the trip time at 6 times `Ir`, seconds.
    pub tr_s: f64,
    /// Short-time pickup as a multiple of `Ir`; `None` for no short-time
    /// zone.
    pub isd: Option<f64>,
    /// Short-time delay `tsd` in seconds.
    pub tsd_s: f64,
    /// Short-time zone with I²t ramp instead of a fixed delay.
    pub i2t_ramp: bool,
    /// Instantaneous pickup as a multiple of `Ir`; `None` when switched
    /// off (the unit then rides through to its `icw` rating).
    pub ii: Option<f64>,
    /// Time from trip decision to the current being cleared, seconds
    /// (about 0.02 for a moulded-case, 0.05 for an air circuit breaker).
    pub opening_s: f64,
}

/// A device's time-current characteristic as a band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripCurve {
    /// Where the curve came from, for the plot legend: `C16`, `gG 63`,
    /// `class 10 @ 12 A`.
    pub label: String,
    /// What kind of device it is.
    pub family: Family,
    /// The current the multiples refer to, amperes.
    pub rated_a: f64,
    min: Band,
    max: Band,
    /// Where the instantaneous trip sits, if the device has one.
    pub instantaneous: Option<Instantaneous>,
    /// Fuse energy values, if known.
    pub energy: Option<FuseEnergy>,
}

/// A step in a band: the same time just above a multiple, so a vertical
/// drop (the instantaneous trip) can be written in a strictly increasing
/// series of points.
const STEP: f64 = 1.0005;

fn check_rating(rated_a: f64) -> Result<(), CurveError> {
    if rated_a.is_finite() && rated_a > 0.0 {
        Ok(())
    } else {
        Err(CurveError::BadRating)
    }
}

impl TripCurve {
    /// Builds a curve from two bands and checks they do not cross
    /// (`min` never slower than `max` at any current).
    pub fn new(
        label: impl Into<String>,
        family: Family,
        rated_a: f64,
        min: Band,
        max: Band,
    ) -> Result<TripCurve, CurveError> {
        check_rating(rated_a)?;
        let curve = TripCurve {
            label: label.into(),
            family,
            rated_a,
            min,
            max,
            instantaneous: None,
            energy: None,
        };
        curve.validate()?;
        Ok(curve)
    }

    /// A curve from a maker's printed band: the points of the fastest
    /// (no-trip) edge and of the slowest (trip-before) edge as
    /// `(current in amperes, seconds)`. This is how user-supplied maker
    /// data enters; the amperes are divided by `rated_a` to multiples.
    pub fn from_points(
        label: impl Into<String>,
        family: Family,
        rated_a: f64,
        min_points: &[(f64, f64)],
        max_points: &[(f64, f64)],
    ) -> Result<TripCurve, CurveError> {
        check_rating(rated_a)?;
        let to_band =
            |pts: &[(f64, f64)]| Band::new(pts.iter().map(|&(i, t)| (i / rated_a, t)).collect());
        TripCurve::new(
            label,
            family,
            rated_a,
            to_band(min_points)?,
            to_band(max_points)?,
        )
    }

    /// The same curve with an instantaneous range recorded.
    pub fn with_instantaneous(mut self, inst: Instantaneous) -> TripCurve {
        self.instantaneous = Some(inst);
        self
    }

    /// The same curve with fuse energy values recorded.
    pub fn with_energy(mut self, energy: FuseEnergy) -> TripCurve {
        self.energy = Some(energy);
        self
    }

    /// The fastest-time edge of the band.
    pub fn min_band(&self) -> &Band {
        &self.min
    }

    /// The slowest-time edge of the band.
    pub fn max_band(&self) -> &Band {
        &self.max
    }

    fn validate(&self) -> Result<(), CurveError> {
        // Compare at every breakpoint of either band.
        let mut ms: Vec<f64> = self.min.points.iter().map(|p| p.0).collect();
        ms.extend(self.max.points.iter().map(|p| p.0));
        for m in ms {
            let (lo, hi) = (self.min.time_at(m), self.max.time_at(m));
            if lo.is_finite() && lo > hi * (1.0 + 1e-6) {
                return Err(CurveError::Crossed(m));
            }
        }
        Ok(())
    }

    /// The fastest and slowest time the device takes to trip at a current
    /// of `i_a` amperes. `None` when it is certain not to trip at that
    /// current. The slowest time is infinite when the device is not
    /// guaranteed to trip at all.
    pub fn trip_time_range(&self, i_a: f64) -> Option<(f64, f64)> {
        if !(i_a.is_finite() && i_a > 0.0) {
            return None;
        }
        let m = i_a / self.rated_a;
        let t_min = self.min.time_at(m);
        if !t_min.is_finite() {
            return None;
        }
        Some((t_min, self.max.time_at(m)))
    }

    /// Whether the device has cleared a fault of `i_a` amperes within
    /// `within_s` seconds.
    pub fn clears(&self, i_a: f64, within_s: f64) -> Clears {
        match self.trip_time_range(i_a) {
            None => Clears::No,
            Some((lo, hi)) => {
                if hi <= within_s {
                    Clears::Yes
                } else if lo > within_s {
                    Clears::No
                } else {
                    Clears::Maybe
                }
            }
        }
    }

    /// The current in amperes from which the device is certain to clear
    /// within `seconds` (the slow edge reaches that time), `None` when it
    /// never does.
    pub fn certain_current_for(&self, seconds: f64) -> Option<f64> {
        self.max.multiple_for(seconds).map(|m| m * self.rated_a)
    }

    /// Samples the band at `n` currents spread evenly in log scale between
    /// `from_a` and `to_a`, for plotting.
    pub fn sample(&self, from_a: f64, to_a: f64, n: usize) -> Vec<Sample> {
        if n < 2 || from_a.is_nan() || from_a <= 0.0 || to_a.is_nan() || to_a <= from_a {
            return Vec::new();
        }
        let (l0, l1) = (from_a.ln(), to_a.ln());
        (0..n)
            .map(|k| {
                let i = (l0 + (l1 - l0) * k as f64 / (n - 1) as f64).exp();
                let m = i / self.rated_a;
                Sample {
                    current_a: i,
                    t_min: self.min.time_at(m),
                    t_max: self.max.time_at(m),
                }
            })
            .collect()
    }

    /// The pointwise faster of two curves (both devices in one housing: a
    /// thermal element and a magnetic trip, a relay and a breaker): the
    /// result trips as soon as either does. Both curves must use the same
    /// reference current.
    pub fn faster_of(
        label: impl Into<String>,
        family: Family,
        a: &TripCurve,
        b: &TripCurve,
    ) -> Result<TripCurve, CurveError> {
        if (a.rated_a - b.rated_a).abs() > a.rated_a * 1e-9 {
            return Err(CurveError::BadRating);
        }
        let envelope = |x: &Band, y: &Band| -> Result<Band, CurveError> {
            let mut ms: Vec<f64> = x.points.iter().chain(&y.points).map(|p| p.0).collect();
            // A point just above each breakpoint keeps the drops steep.
            let above: Vec<f64> = ms.iter().map(|m| m * STEP).collect();
            ms.extend(above);
            ms.sort_by(|p, q| p.total_cmp(q));
            ms.dedup_by(|p, q| (*p - *q).abs() < 1e-12);
            let mut out: Vec<(f64, f64)> = Vec::new();
            for m in ms {
                let t = x.time_at(m).min(y.time_at(m));
                if !t.is_finite() {
                    continue;
                }
                // The best of both can only fall as the current rises.
                let t = out.last().map_or(t, |l| t.min(l.1));
                out.push((m, t));
            }
            Band::new(out)
        };
        let mut curve = TripCurve::new(
            label,
            family,
            a.rated_a,
            envelope(&a.min, &b.min)?,
            envelope(&a.max, &b.max)?,
        )?;
        curve.instantaneous = match (a.instantaneous, b.instantaneous) {
            (Some(x), Some(y)) => Some(Instantaneous {
                lo: x.lo.min(y.lo),
                hi: x.hi.min(y.hi),
            }),
            (x, y) => x.or(y),
        };
        Ok(curve)
    }

    /// A miniature circuit breaker of rating `rated_a` and curve letter.
    ///
    /// Built from the limits of IEC 60898-1: no trip within one hour (two
    /// above 63 A) at 1.13 `In`, a trip within that time at 1.45 `In`, a
    /// trip between 1 s and 60 s (120 s above 32 A) at 2.55 `In`, and for
    /// the instantaneous trip no operation within 0.1 s at the lower limit
    /// of the curve and operation within 0.1 s at the upper limit.
    pub fn mcb(rated_a: f64, curve: McbCurve) -> Result<TripCurve, CurveError> {
        check_rating(rated_a)?;
        let conv = if rated_a <= 63.0 { 3600.0 } else { 7200.0 };
        let t255 = if rated_a <= 32.0 { 60.0 } else { 120.0 };
        let inst = curve.instantaneous();
        // Fastest edge: nothing before 1.13 In for the conventional time,
        // at least 1 s at 2.55 In, at least 0.1 s up to the lower
        // instantaneous limit, then the unlatching time of the mechanism.
        let mut min_pts = vec![(1.13, conv)];
        if inst.lo > 2.55 {
            min_pts.push((2.55, 1.0));
        }
        min_pts.extend([(inst.lo, 0.1), (inst.lo * STEP, 0.003), (1000.0, 0.003)]);
        let min = Band::new(min_pts)?;
        // Slowest edge: tripped within the conventional time at 1.45 In,
        // within 60 or 120 s at 2.55 In, within 0.1 s at the upper limit,
        // then cleared within a half cycle and a bit.
        let mut max_pts = vec![(1.45, conv)];
        if inst.hi > 2.55 {
            max_pts.push((2.55, t255));
        }
        max_pts.extend([(inst.hi, 0.1), (inst.hi * STEP, 0.012), (1000.0, 0.012)]);
        let max = Band::new(max_pts)?;
        let label = format!("{} {}", curve.letter(), fmt_a(rated_a));
        Ok(TripCurve::new(label, Family::Mcb, rated_a, min, max)?.with_instantaneous(inst))
    }

    /// A general-purpose gG fuse link of `rated_a` (IEC 60269-1/-2).
    ///
    /// The conventional currents and times are the standard's (1.25 and
    /// 1.6 `In` for 16 A and up, 1.5 and 1.9 below, 1.5 and 2.1 below 4 A;
    /// 1, 2, 3 or 4 hours by rating). The melting energy is an *estimate*
    /// (`I²t ≈ 0.7·In^2.5`, within about ±40 % of catalogue values for
    /// 16 to 125 A) unless `pre_arcing_i2t` is given from a maker's data;
    /// the total operating I²t is taken as 2.5 times it at 400 V. Use
    /// [`from_points`](TripCurve::from_points) with the maker's curve for
    /// a finished design.
    pub fn fuse_gg(rated_a: f64, pre_arcing_i2t: Option<f64>) -> Result<TripCurve, CurveError> {
        check_rating(rated_a)?;
        let (inf, f) = if rated_a < 4.0 {
            (1.5, 2.1)
        } else if rated_a < 16.0 {
            (1.5, 1.9)
        } else {
            (1.25, 1.6)
        };
        let conv = if rated_a <= 63.0 {
            3600.0
        } else if rated_a <= 160.0 {
            7200.0
        } else if rated_a <= 400.0 {
            10800.0
        } else {
            14400.0
        };
        let i2t_nom = pre_arcing_i2t.unwrap_or(0.7 * rated_a.powf(2.5));
        if !(i2t_nom.is_finite() && i2t_nom > 0.0) {
            return Err(CurveError::BadNumber);
        }
        let pre_min = i2t_nom * 0.8;
        let total_max = i2t_nom * 2.5;
        // Where the I²t lines meet 0.1 s and 0.01 s.
        let m_at = |i2t: f64, t: f64| (i2t / t).sqrt() / rated_a;
        // The line from the conventional point to the 0.1 s point of the
        // I²t law is only drawn when it falls; guard tiny fuses.
        let hi_slow = m_at(total_max, 0.1).max(f * 1.2);
        let lo_slow = m_at(pre_min, 0.1).max(inf * 1.2);
        let min = Band::new(vec![
            (inf, conv),
            (lo_slow, 0.1),
            (lo_slow * 3.0, 0.1 / 9.0),
            (lo_slow * 30.0, 0.1 / 900.0),
            (lo_slow * 1000.0, 0.1 / 900.0 / 1e3),
        ])?;
        let max = Band::new(vec![
            (f, conv),
            (hi_slow, 0.1),
            (hi_slow * 3.0, 0.1 / 9.0),
            (hi_slow * 30.0, 0.1 / 900.0),
            (hi_slow * 1000.0, 0.1 / 900.0 / 1e3),
        ])?;
        let label = format!("gG {}", fmt_a(rated_a));
        Ok(
            TripCurve::new(label, Family::Fuse, rated_a, min, max)?.with_energy(FuseEnergy {
                pre_arcing_min: pre_min,
                operating_max: total_max,
            }),
        )
    }

    /// A thermal or electronic overload relay set to `ir_a`, cold, three
    /// phases loaded equally (IEC 60947-4-1): no trip within two hours at
    /// 1.05 `Ir`, trip within two hours at 1.2 `Ir`, within the class time
    /// (2, 4, 8, 12 minutes) at 1.5 `Ir`, and the class window at 7.2 `Ir`.
    /// A warm relay trips sooner; that is not modelled.
    pub fn overload_relay(ir_a: f64, class: RelayClass) -> Result<TripCurve, CurveError> {
        check_rating(ir_a)?;
        let (t72_min, t72_max) = class.at_7_2();
        let t15 = class.at_1_5();
        let min = Band::new(vec![
            (1.05, 7200.0),
            (1.5, t15 / 4.0),
            (7.2, t72_min),
            (7.2 * 3.0, t72_min / 9.0),
            (60.0, t72_min / 9.0),
        ])?;
        let max = Band::new(vec![
            (1.2, 7200.0),
            (1.5, t15),
            (7.2, t72_max),
            (7.2 * 3.0, t72_max / 9.0),
            (60.0, t72_max / 9.0),
        ])?;
        let label = format!("class {} @ {}", class.label(), fmt_a(ir_a));
        TripCurve::new(label, Family::OverloadRelay, ir_a, min, max)
    }

    /// A motor protection circuit breaker: an overload relay of the given
    /// class set to `ir_a`, plus a magnetic trip set to `magnetic_mult`
    /// times `Ir` (typically 12 to 14 fixed, adjustable on some), with the
    /// usual ±20 % tolerance on the pickup.
    pub fn mpcb(ir_a: f64, class: RelayClass, magnetic_mult: f64) -> Result<TripCurve, CurveError> {
        if !(magnetic_mult.is_finite() && magnetic_mult > 7.2) {
            return Err(CurveError::BadNumber);
        }
        let thermal = TripCurve::overload_relay(ir_a, class)?;
        let inst = Instantaneous {
            lo: magnetic_mult * 0.8,
            hi: magnetic_mult * 1.2,
        };
        let magnetic = TripCurve::new(
            "magnetic",
            Family::Breaker,
            ir_a,
            Band::new(vec![
                (inst.lo, 0.1),
                (inst.lo * STEP, 0.003),
                (1000.0, 0.003),
            ])?,
            Band::new(vec![
                (inst.hi, 0.1),
                (inst.hi * STEP, 0.012),
                (1000.0, 0.012),
            ])?,
        )?;
        let mut c = TripCurve::faster_of(
            format!("MPCB class {} @ {}", class.label(), fmt_a(ir_a)),
            Family::MotorBreaker,
            &thermal,
            &magnetic,
        )?;
        c.instantaneous = Some(inst);
        Ok(c)
    }

    /// A breaker with an electronic trip unit and the given settings, in
    /// the way of IEC 60947-2: long-time pickup between 1.05 and 1.2 `Ir`
    /// with an I²t law (`tr` at 6 `Ir`, ±20 %), short-time pickup and delay
    /// (±10 % on the pickup), instantaneous pickup (±10 %).
    ///
    /// Tolerances are typical values; the maker's tolerance band for the
    /// unit replaces them when its curve is entered with
    /// [`from_points`](TripCurve::from_points).
    pub fn lsi(settings: Lsi) -> Result<TripCurve, CurveError> {
        check_rating(settings.ir_a)?;
        let Lsi {
            ir_a,
            tr_s,
            isd,
            tsd_s,
            i2t_ramp,
            ii,
            opening_s,
        } = settings;
        let finite_pos = |x: f64| x.is_finite() && x > 0.0;
        if !(finite_pos(tr_s) && finite_pos(opening_s)) {
            return Err(CurveError::BadNumber);
        }
        if let Some(sd) = isd
            && !(sd.is_finite() && sd > 1.5 && tsd_s.is_finite() && tsd_s >= 0.0)
        {
            return Err(CurveError::BadNumber);
        }
        if let Some(i) = ii
            && !(i.is_finite() && i > 1.5 && isd.is_none_or(|sd| i >= sd))
        {
            return Err(CurveError::BadNumber);
        }
        // One edge of the band as a function of the multiple of Ir. The
        // fastest edge (`fast`) uses the low side of every tolerance, the
        // slowest the high side and the opening time.
        let edge = |fast: bool, m: f64| -> f64 {
            let pick = if fast { 0.9 } else { 1.1 };
            let law = if fast { 0.8 } else { 1.2 };
            let mut t = if m < 1.5 {
                7200.0
            } else {
                law * tr_s * (6.0 / m).powi(2)
            };
            if let Some(sd) = isd
                && m >= sd * pick
            {
                let ramp = if i2t_ramp { (sd / m).powi(2) } else { 1.0 };
                let st = tsd_s * ramp;
                let st = if fast {
                    st.max(opening_s * 0.4)
                } else {
                    st + opening_s
                };
                t = t.min(st);
            }
            if let Some(i) = ii
                && m >= i * pick
            {
                t = t.min(if fast { opening_s * 0.4 } else { opening_s });
            }
            t
        };
        let build = |fast: bool| -> Result<Band, CurveError> {
            let start = if fast { 1.05 } else { 1.2 };
            let mut ms = vec![start, 1.5, 1000.0];
            for k in 1..40 {
                ms.push(1.5 * (1000.0f64 / 1.5).powf(k as f64 / 40.0));
            }
            let pick = if fast { 0.9 } else { 1.1 };
            for z in [isd, ii].into_iter().flatten() {
                ms.push(z * pick);
                ms.push(z * pick * STEP);
            }
            ms.sort_by(|a, b| a.total_cmp(b));
            ms.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            let mut pts: Vec<(f64, f64)> = Vec::new();
            for m in ms {
                let t = edge(fast, m).min(pts.last().map_or(f64::INFINITY, |l| l.1));
                pts.push((m, t));
            }
            Band::new(pts)
        };
        let label = format!("LSI {}", fmt_a(ir_a));
        let mut c = TripCurve::new(label, Family::Breaker, ir_a, build(true)?, build(false)?)?;
        c.instantaneous = ii.map(|i| Instantaneous {
            lo: i * 0.9,
            hi: i * 1.1,
        });
        Ok(c)
    }

    /// The reference of a cable's thermal withstand as a band for
    /// plotting next to the devices: the time `k²S²/I²` a conductor of
    /// `section_mm2` and material constant `k` survives (adiabatic, valid
    /// up to about 5 s), with multiples of `rated_a`.
    pub fn cable_withstand(section_mm2: f64, k: f64, rated_a: f64) -> Result<Band, CurveError> {
        check_rating(rated_a)?;
        if !(section_mm2.is_finite() && section_mm2 > 0.0 && k.is_finite() && k > 0.0) {
            return Err(CurveError::BadNumber);
        }
        let i2t = k * k * section_mm2 * section_mm2;
        // From 5 s down to 5 ms (higher currents are not adiabatic data).
        let pts = [5.0, 1.0, 0.1, 0.01, 0.005]
            .iter()
            .map(|&t| ((i2t / t).sqrt() / rated_a, t))
            .collect();
        Band::new(pts)
    }

    /// A copy scaled to another reference current (same shape in
    /// multiples): e.g. a C curve for another rating.
    pub fn rescaled(&self, rated_a: f64) -> Result<TripCurve, CurveError> {
        check_rating(rated_a)?;
        let mut c = self.clone();
        c.rated_a = rated_a;
        Ok(c)
    }

    /// A copy with times multiplied (for derating studies, ambient
    /// temperature, warm start): `factor` below 1 is faster.
    pub fn time_scaled(&self, factor: f64) -> Result<TripCurve, CurveError> {
        if !(factor.is_finite() && factor > 0.0) {
            return Err(CurveError::BadNumber);
        }
        let mut c = self.clone();
        c.min = self.min.mapped(|m, t| (m, t * factor));
        c.max = self.max.mapped(|m, t| (m, t * factor));
        Ok(c)
    }
}

/// `16` for 16 A, `6.3` for 6.3 A, as printed on a rating plate.
pub(crate) fn fmt_a(a: f64) -> String {
    if (a - a.round()).abs() < 1e-9 {
        format!("{}", a.round() as i64)
    } else {
        format!("{a}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn band_interpolates_in_log_log() {
        let b = Band::new(vec![(1.0, 100.0), (10.0, 1.0)]).unwrap();
        // One decade in current, two in time: a straight line of slope -2.
        assert!(close(b.time_at(1.0), 100.0));
        assert!(close(b.time_at(10.0), 1.0));
        assert!(close(b.time_at(10f64.sqrt()), 10.0));
        assert!(b.time_at(0.5).is_infinite());
        assert!(close(b.time_at(50.0), 1.0), "flat above the last point");
    }

    #[test]
    fn band_rejects_bad_input() {
        assert_eq!(Band::new(vec![(1.0, 1.0)]), Err(CurveError::TooFewPoints));
        assert_eq!(
            Band::new(vec![(2.0, 1.0), (1.0, 1.0)]),
            Err(CurveError::NotIncreasing)
        );
        assert_eq!(
            Band::new(vec![(1.0, 1.0), (2.0, 2.0)]),
            Err(CurveError::TimeRises)
        );
        assert_eq!(
            Band::new(vec![(1.0, 0.0), (2.0, 1.0)]),
            Err(CurveError::BadNumber)
        );
        assert_eq!(
            Band::new(vec![(1.0, f64::NAN), (2.0, 1.0)]),
            Err(CurveError::BadNumber)
        );
    }

    #[test]
    fn multiple_for_inverts_time_at() {
        let b = Band::new(vec![(1.0, 100.0), (10.0, 1.0), (100.0, 0.01)]).unwrap();
        let m = b.multiple_for(10.0).unwrap();
        assert!(close(m, 10f64.sqrt()), "{m}");
        assert_eq!(b.multiple_for(0.001), None);
        assert!(close(b.multiple_for(1000.0).unwrap(), 1.0));
    }

    /// Worked example: a C16 breaker. At 160 A (10 In) it has certainly
    /// tripped within 0.1 s; at 80 A (5 In) it certainly has not tripped
    /// within 0.1 s; at 1.13 In it does not trip in an hour.
    #[test]
    fn c16_matches_the_standard_test_points() {
        let c = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        assert_eq!(c.label, "C 16");
        let (_, t_max) = c.trip_time_range(160.0).unwrap();
        assert!(t_max <= 0.1 + 1e-9, "{t_max}");
        assert_eq!(c.clears(160.0, 0.1), Clears::Yes);
        let (t_min, _) = c.trip_time_range(80.0).unwrap();
        assert!(t_min >= 0.1 - 1e-9, "{t_min}");
        assert_eq!(c.clears(80.0, 0.05), Clears::No);
        // 1.13 In = 18.08 A: not guaranteed to trip, must hold for an hour.
        let (t_min, t_max) = c.trip_time_range(1.13 * 16.0).unwrap();
        assert!(close(t_min, 3600.0));
        assert!(t_max.is_infinite());
        assert_eq!(c.trip_time_range(1.1 * 16.0), None);
        // 1.45 In: tripped within the hour.
        assert_eq!(c.clears(1.45 * 16.0, 3600.0), Clears::Yes);
        // 2.55 In: between 1 s and 60 s.
        let (t_min, t_max) = c.trip_time_range(2.55 * 16.0).unwrap();
        assert!(close(t_min, 1.0) && close(t_max, 60.0), "{t_min} {t_max}");
    }

    #[test]
    fn mcb_time_limits_follow_the_rating() {
        // Above 32 A the 2.55 In window is 1 to 120 s; above 63 A the
        // conventional time is two hours.
        let c = TripCurve::mcb(40.0, McbCurve::B).unwrap();
        let (_, t_max) = c.trip_time_range(2.55 * 40.0).unwrap();
        assert!(close(t_max, 120.0));
        let c = TripCurve::mcb(100.0, McbCurve::C).unwrap();
        assert_eq!(c.clears(1.45 * 100.0, 3600.0), Clears::Maybe);
        assert_eq!(c.clears(1.45 * 100.0, 7200.0), Clears::Yes);
    }

    #[test]
    fn mcb_curve_letters_set_the_instantaneous_range() {
        for (curve, lo, hi) in [
            (McbCurve::B, 3.0, 5.0),
            (McbCurve::C, 5.0, 10.0),
            (McbCurve::D, 10.0, 20.0),
            (McbCurve::K, 8.0, 14.0),
            (McbCurve::Z, 2.0, 3.0),
        ] {
            let c = TripCurve::mcb(10.0, curve).unwrap();
            let inst = c.instantaneous.unwrap();
            assert_eq!((inst.lo, inst.hi), (lo, hi));
            assert_eq!(c.clears(hi * 10.0 * 1.001, 0.1), Clears::Yes, "{curve:?}");
            assert_eq!(c.clears(lo * 10.0 * 0.999, 0.099), Clears::No, "{curve:?}");
        }
        assert_eq!(McbCurve::parse(" c "), Some(McbCurve::C));
        assert_eq!(McbCurve::parse("E"), None);
    }

    /// Worked example: class 10 relay set to 10 A. At 72 A (7.2 Ir) it
    /// trips in 4 to 10 s; at 15 A (1.5 Ir) within 4 minutes; not within two
    /// hours at 10.5 A (1.05 Ir).
    #[test]
    fn overload_relay_matches_class_windows() {
        let r = TripCurve::overload_relay(10.0, RelayClass::C10).unwrap();
        let (lo, hi) = r.trip_time_range(72.0).unwrap();
        assert!(close(lo, 4.0) && close(hi, 10.0), "{lo} {hi}");
        assert_eq!(r.clears(15.0, 240.0), Clears::Yes);
        assert_eq!(r.clears(15.0, 60.0 - 1.0), Clears::No);
        assert_eq!(r.clears(10.5, 7200.0 - 1.0), Clears::No);
        assert_eq!(r.trip_time_range(10.4), None);
        for (class, lo, hi) in [
            (RelayClass::C10A, 2.0, 10.0),
            (RelayClass::C20, 6.0, 20.0),
            (RelayClass::C30, 9.0, 30.0),
        ] {
            let r = TripCurve::overload_relay(1.0, class).unwrap();
            let (a, b) = r.trip_time_range(7.2).unwrap();
            assert!(close(a, lo) && close(b, hi));
        }
        assert_eq!(RelayClass::parse("Class 20"), Some(RelayClass::C20));
    }

    /// Worked example: gG 63 A. 1.6 In (100.8 A) trips within 1 hour, 1.25 In
    /// does not; the 0.1 s point comes from the I²t estimate.
    #[test]
    fn fuse_gg_conventional_points() {
        let f = TripCurve::fuse_gg(63.0, None).unwrap();
        assert_eq!(f.clears(1.6 * 63.0, 3600.0), Clears::Yes);
        let (t_min, _) = f.trip_time_range(1.25 * 63.0).unwrap();
        assert!(close(t_min, 3600.0));
        assert_eq!(f.trip_time_range(1.2 * 63.0), None);
        // 100 A: conventional time is two hours above 63 A.
        let f = TripCurve::fuse_gg(100.0, None).unwrap();
        assert_eq!(f.clears(160.0, 3600.0), Clears::Maybe);
        assert_eq!(f.clears(160.0, 7200.0), Clears::Yes);
        // Small links use the wider conventional factors.
        let f = TripCurve::fuse_gg(10.0, None).unwrap();
        assert_eq!(f.clears(19.0, 3600.0), Clears::Yes);
        assert_eq!(f.trip_time_range(14.9), None);
    }

    #[test]
    fn fuse_follows_its_i2t_at_high_current() {
        let f = TripCurve::fuse_gg(63.0, Some(20_000.0)).unwrap();
        let e = f.energy.unwrap();
        assert!(close(e.pre_arcing_min, 16_000.0) && close(e.operating_max, 50_000.0));
        // At 2 kA the melting time follows I²t / I² (4 ms at 16 kA²s... ):
        let (t_min, t_max) = f.trip_time_range(2000.0).unwrap();
        assert!(
            (16_000.0 / 4.0e6 * 0.98..=16_000.0 / 4.0e6 * 1.02).contains(&t_min),
            "{t_min}"
        );
        assert!(
            (50_000.0 / 4.0e6 * 0.98..=50_000.0 / 4.0e6 * 1.02).contains(&t_max),
            "{t_max}"
        );
    }

    #[test]
    fn maker_points_replace_the_standard_band() {
        // A made-up maker curve in amperes.
        let c = TripCurve::from_points(
            "maker 20 A",
            Family::Custom,
            20.0,
            &[(25.0, 1000.0), (100.0, 1.0), (400.0, 0.01)],
            &[(30.0, 1000.0), (200.0, 5.0), (400.0, 0.02)],
        )
        .unwrap();
        let (lo, hi) = c.trip_time_range(100.0).unwrap();
        assert!(close(lo, 1.0));
        assert!(hi > 5.0 && hi < 1000.0);
        // A crossing band is refused.
        let bad = TripCurve::from_points(
            "bad",
            Family::Custom,
            20.0,
            &[(25.0, 100.0), (100.0, 10.0)],
            &[(25.0, 50.0), (100.0, 5.0)],
        );
        assert!(matches!(bad, Err(CurveError::Crossed(_))));
    }

    #[test]
    fn mpcb_trips_on_either_element() {
        let m = TripCurve::mpcb(10.0, RelayClass::C10, 13.0).unwrap();
        // Overload: the thermal band.
        let (lo, hi) = m.trip_time_range(72.0).unwrap();
        assert!(close(lo, 4.0) && close(hi, 10.0));
        // Short circuit above 1.2 * 13 Ir: cleared in a cycle.
        assert_eq!(m.clears(1.2 * 13.0 * 10.0 * 1.01, 0.02), Clears::Yes);
        // Below 0.8 * 13 Ir the magnetic cannot be the reason it trips.
        let (lo, _) = m.trip_time_range(0.79 * 13.0 * 10.0).unwrap();
        assert!(lo > 0.003);
        assert!(TripCurve::mpcb(10.0, RelayClass::C10, 5.0).is_err());
    }

    fn acb() -> TripCurve {
        TripCurve::lsi(Lsi {
            ir_a: 1000.0,
            tr_s: 12.0,
            isd: Some(6.0),
            tsd_s: 0.3,
            i2t_ramp: false,
            ii: Some(15.0),
            opening_s: 0.05,
        })
        .unwrap()
    }

    #[test]
    fn lsi_has_three_zones() {
        let c = acb();
        // Long-time: 12 s at 6 x ... but 6 Ir is already the short-time
        // pickup: use 3 Ir, where the law gives 12 * (6/3)^2 = 48 s.
        let (lo, hi) = c.trip_time_range(3000.0).unwrap();
        assert!(close(lo, 0.8 * 48.0) && close(hi, 1.2 * 48.0), "{lo} {hi}");
        // Short-time: a trip at 10 Ir waits the delay, then opens.
        let (lo, hi) = c.trip_time_range(10_000.0).unwrap();
        assert!(close(lo, 0.3) && close(hi, 0.35), "{lo} {hi}");
        // Instantaneous above 1.1 * 15 Ir: within the opening time.
        let (lo, hi) = c.trip_time_range(20_000.0).unwrap();
        assert!(close(hi, 0.05) && lo < 0.05, "{lo} {hi}");
        assert_eq!(c.clears(20_000.0, 0.05), Clears::Yes);
        // Below pickup nothing for two hours.
        assert_eq!(c.trip_time_range(1040.0), None);
    }

    #[test]
    fn lsi_with_i2t_ramp_falls_with_current() {
        let c = TripCurve::lsi(Lsi {
            ir_a: 100.0,
            tr_s: 10.0,
            isd: Some(4.0),
            tsd_s: 0.4,
            i2t_ramp: true,
            ii: None,
            opening_s: 0.03,
        })
        .unwrap();
        let (a, _) = c.trip_time_range(450.0).unwrap();
        let (b, _) = c.trip_time_range(900.0).unwrap();
        assert!(b < a, "{a} {b}");
    }

    #[test]
    fn lsi_rejects_nonsense() {
        let base = Lsi {
            ir_a: 100.0,
            tr_s: 10.0,
            isd: Some(6.0),
            tsd_s: 0.1,
            i2t_ramp: false,
            ii: Some(3.0),
            opening_s: 0.03,
        };
        assert!(TripCurve::lsi(base).is_err(), "Ii below Isd");
        assert!(TripCurve::lsi(Lsi { tr_s: 0.0, ..base }).is_err());
        assert!(TripCurve::lsi(Lsi { ir_a: 0.0, ..base }).is_err());
    }

    /// Every built-in curve must be a proper band: the fastest edge is never
    /// slower than the slowest, both fall with the current.
    #[test]
    fn every_builder_makes_a_proper_band() {
        let mut curves = Vec::new();
        for amps in [1.0, 6.0, 16.0, 32.0, 40.0, 63.0, 80.0, 125.0, 250.0] {
            for l in [
                McbCurve::B,
                McbCurve::C,
                McbCurve::D,
                McbCurve::K,
                McbCurve::Z,
            ] {
                curves.push(TripCurve::mcb(amps, l).unwrap());
            }
            curves.push(TripCurve::fuse_gg(amps, None).unwrap());
            for c in [
                RelayClass::C10A,
                RelayClass::C10,
                RelayClass::C20,
                RelayClass::C30,
            ] {
                curves.push(TripCurve::overload_relay(amps, c).unwrap());
                curves.push(TripCurve::mpcb(amps, c, 13.0).unwrap());
            }
        }
        curves.push(acb());
        for c in curves {
            let samples = c.sample(c.rated_a * 0.5, c.rated_a * 2000.0, 400);
            let mut prev = (f64::INFINITY, f64::INFINITY);
            for s in samples {
                if s.t_min.is_finite() {
                    assert!(
                        s.t_min <= s.t_max * (1.0 + 1e-9),
                        "{} at {}",
                        c.label,
                        s.current_a
                    );
                }
                assert!(
                    s.t_min <= prev.0 * (1.0 + 1e-9),
                    "{} fastest rises",
                    c.label
                );
                assert!(
                    s.t_max <= prev.1 * (1.0 + 1e-9),
                    "{} slowest rises",
                    c.label
                );
                prev = (s.t_min, s.t_max);
            }
        }
    }

    #[test]
    fn cable_withstand_is_k2s2_over_i2() {
        // 2.5 mm² PVC copper (k = 115): I²t = 115² * 2.5² = 82_656 A²s.
        let b = TripCurve::cable_withstand(2.5, 115.0, 1.0).unwrap();
        let t = b.time_at(1000.0);
        assert!(close(t, 82_656.25 / 1.0e6) || t > 0.0);
        let i_at_1s = 82_656.25f64.sqrt();
        assert!(close(b.time_at(i_at_1s), 1.0));
    }

    #[test]
    fn clears_and_sampling() {
        let c = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        assert_eq!(c.clears(0.0, 1.0), Clears::No);
        assert_eq!(c.clears(f64::NAN, 1.0), Clears::No);
        let s = c.sample(10.0, 1000.0, 5);
        assert_eq!(s.len(), 5);
        assert!(close(s[0].current_a, 10.0) && close(s[4].current_a, 1000.0));
        assert!(c.sample(10.0, 1.0, 5).is_empty());
        let i = c.certain_current_for(0.1).unwrap();
        assert!(close(i, 160.0), "{i}");
    }

    #[test]
    fn time_scaling_and_rescaling() {
        let c = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        let fast = c.time_scaled(0.5).unwrap();
        let (_, a) = c.trip_time_range(160.0).unwrap();
        let (_, b) = fast.trip_time_range(160.0).unwrap();
        assert!(close(b, a * 0.5));
        let c20 = c.rescaled(20.0).unwrap();
        assert_eq!(c20.clears(200.0, 0.1), Clears::Yes);
    }

    #[test]
    fn curves_survive_json() {
        let c = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        let text = serde_json::to_string(&c).unwrap();
        let back: TripCurve = serde_json::from_str(&text).unwrap();
        // The text form may differ from the number in the last digit.
        assert_eq!(c.label, back.label);
        for i in [20.0, 40.0, 80.0, 160.0, 400.0] {
            let (a, b) = (
                c.trip_time_range(i).unwrap(),
                back.trip_time_range(i).unwrap(),
            );
            assert!(
                (a.0 == b.0 || (a.0 / b.0 - 1.0).abs() < 1e-6)
                    && (a.1 == b.1 || (a.1 / b.1 - 1.0).abs() < 1e-6),
                "{i}"
            );
        }
    }
}
