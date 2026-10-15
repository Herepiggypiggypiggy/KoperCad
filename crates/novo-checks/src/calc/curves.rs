//! Time-current characteristics of protective devices, as bands.
//!
//! A real device does not trip at one time for a given current; the
//! standards guarantee a *band*: it does not trip sooner than `t_min` and
//! trips no later than `t_max`. Everything in the protection lane (can this
//! breaker protect that cable, which of two devices opens first) works on
//! these bands. Times are seconds; [`f64::INFINITY`] means the device is not
//! guaranteed to trip at that current.
//!
//! # Where the bands come from
//!
//! | Device | Source | What is guaranteed |
//! | --- | --- | --- |
//! | Miniature breaker B, C, D | IEC 60898-1 Tables 7 and 8 | conventional non-tripping 1.13 In and tripping 1.45 In (1 h, 2 h above 63 A), 2.55 In (1 to 60 s, to 120 s above 32 A), magnetic bands 3–5, 5–10, 10–20 In |
//! | Overload relay class 10A, 10, 20, 30 | IEC 60947-4-1 Table 2 | 1.05 Ir (2 h, no trip), 1.2 Ir (within 2 h), 1.5 Ir (2, 4, 8, 12 min), 7.2 Ir (2 to 10 s, 4 to 10, 6 to 20, 9 to 30) |
//! | Fuse gG | IEC 60269-1 Table 2 | conventional non-fusing and fusing currents only (1.25 / 1.6 In from 16 A, 1.5 / 1.9 and 1.5 / 2.1 below) |
//! | Adjustable (LSI) | parametric | the settings; the *tolerances* are assumptions, see [`Electronic`] |
//! | Points | maker data | exactly what the points say, nothing outside them |
//!
//! Between the guaranteed points the bands are joined by straight lines on
//! log-log axes, which is how published curves are drawn. Where the standard
//! says nothing the answer is `None`, not an extrapolation. A band from the
//! standard is a worst-case envelope of *any* conforming device: a given
//! maker's device is faster, so selectivity read from these bands is
//! conservative, and maker tables (see [`super::selectivity`]) always win.

use super::short_circuit::Trip;

/// The standard's guarantee about one current: no trip sooner than `t_min`,
/// trip no later than `t_max`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeBand {
    pub t_min: f64,
    pub t_max: f64,
}

/// Fastest a magnetic release may act. Not set by the standard (it only
/// bounds the *slowest* instantaneous trip); a quarter of a cycle at 50 Hz.
pub const INSTANT_MIN_S: f64 = 0.005;

/// Interpolates `points` (x ascending, all positive) linearly on log-log
/// axes. `None` outside the listed range.
fn loglog(points: &[(f64, f64)], x: f64) -> Option<f64> {
    let (first, last) = (*points.first()?, *points.last()?);
    if !x.is_finite() || x < first.0 || x > last.0 {
        return None;
    }
    let upper = points.iter().position(|p| p.0 >= x)?;
    if upper == 0 {
        return Some(first.1);
    }
    let ((x0, y0), (x1, y1)) = (points[upper - 1], points[upper]);
    if (x1 - x0).abs() < 1e-12 {
        return Some(y1);
    }
    let f = (x.ln() - x0.ln()) / (x1.ln() - x0.ln());
    Some((y0.ln() + f * (y1.ln() - y0.ln())).exp())
}

/// Miniature breaker characteristic letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McbCurve {
    B,
    C,
    D,
}

impl McbCurve {
    /// Magnetic band as multiples of `In`: no trip below `.0` within 0.1 s,
    /// certain trip within 0.1 s at `.1`.
    pub fn magnetic(self) -> (f64, f64) {
        match self {
            McbCurve::B => (3.0, 5.0),
            McbCurve::C => (5.0, 10.0),
            McbCurve::D => (10.0, 20.0),
        }
    }

    pub fn from_trip(trip: Trip) -> Option<McbCurve> {
        match trip {
            Trip::B => Some(McbCurve::B),
            Trip::C => Some(McbCurve::C),
            Trip::D => Some(McbCurve::D),
            Trip::Factor(_) => None,
        }
    }
}

/// Overload relay trip class (IEC 60947-4-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelayClass {
    C10A,
    C10,
    C20,
    C30,
}

impl RelayClass {
    pub fn parse(text: &str) -> Option<RelayClass> {
        match text.trim().to_ascii_uppercase().as_str() {
            "10A" => Some(RelayClass::C10A),
            "10" => Some(RelayClass::C10),
            "20" => Some(RelayClass::C20),
            "30" => Some(RelayClass::C30),
            _ => None,
        }
    }

    /// Trip time window in seconds at 7.2 × Ir from cold.
    pub fn window_at_7_2(self) -> (f64, f64) {
        match self {
            RelayClass::C10A => (2.0, 10.0),
            RelayClass::C10 => (4.0, 10.0),
            RelayClass::C20 => (6.0, 20.0),
            RelayClass::C30 => (9.0, 30.0),
        }
    }

    /// Longest trip time in seconds at 1.5 × Ir from hot.
    pub fn max_at_1_5(self) -> f64 {
        match self {
            RelayClass::C10A => 120.0,
            RelayClass::C10 => 240.0,
            RelayClass::C20 => 480.0,
            RelayClass::C30 => 720.0,
        }
    }
}

/// Settings of an adjustable electronic trip unit (breaker with long-time,
/// short-time and instantaneous protection).
///
/// The model: long-time delay `tr` is the time at 6 × Ir, falling with the
/// square of the current (I²t); short-time pickup `isd` × Ir with a
/// definite delay `tsd`; instantaneous at `ii` × In. The *tolerance* of
/// each stage is not standardised, so the defaults (±10 % on the pickup,
/// time −20 % / +20 %, breaking time of the instantaneous stage 30 ms) are
/// assumptions that a maker table replaces; the model says so in its
/// [`Characteristic::assumptions`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Electronic {
    pub in_a: f64,
    pub ir_a: f64,
    pub tr_s: f64,
    pub isd_x: f64,
    pub tsd_s: f64,
    pub ii_x: f64,
}

/// One characteristic, ready to ask questions of.
#[derive(Debug, Clone, PartialEq)]
pub enum Characteristic {
    Mcb {
        curve: McbCurve,
        in_a: f64,
    },
    Relay {
        class: RelayClass,
        ir_a: f64,
    },
    FuseGg {
        in_a: f64,
    },
    Electronic(Electronic),
    /// Maker points, `(current in A, time in s)`, ascending in current,
    /// one list per side of the band.
    Points {
        min: Vec<(f64, f64)>,
        max: Vec<(f64, f64)>,
    },
}

impl Characteristic {
    /// The rated current or setting the characteristic is built on, A.
    pub fn rated_a(&self) -> Option<f64> {
        match self {
            Characteristic::Mcb { in_a, .. } | Characteristic::FuseGg { in_a } => Some(*in_a),
            Characteristic::Relay { ir_a, .. } => Some(*ir_a),
            Characteristic::Electronic(e) => Some(e.ir_a),
            Characteristic::Points { .. } => None,
        }
    }

    /// What was assumed beyond the standard, one line each, for the report.
    pub fn assumptions(&self) -> Vec<String> {
        match self {
            Characteristic::Mcb { .. } => vec![format!(
                "no trip faster than {} ms in the magnetic region (not set by IEC 60898-1)",
                INSTANT_MIN_S * 1000.0
            )],
            Characteristic::Relay { .. } => vec![
                "relay band joined from the 1.2, 1.5 and 7.2 × Ir points with an I²t slope; cold and hot states are not separated"
                    .to_owned(),
            ],
            Characteristic::FuseGg { .. } => vec![
                "gG fuse: only the conventional currents are guaranteed; the melting times need the maker's data"
                    .to_owned(),
            ],
            Characteristic::Electronic(_) => vec![
                "electronic trip unit: pickup ±10 %, delay −20 %/+20 %, instantaneous break time 30 ms are assumed; use the maker's curve for guaranteed values"
                    .to_owned(),
            ],
            Characteristic::Points { .. } => Vec::new(),
        }
    }

    /// The band at `i_a`, or `None` when nothing is known there.
    pub fn band(&self, i_a: f64) -> Option<TimeBand> {
        if !(i_a.is_finite() && i_a > 0.0) {
            return None;
        }
        match self {
            Characteristic::Mcb { curve, in_a } => mcb_band(*curve, *in_a, i_a),
            Characteristic::Relay { class, ir_a } => relay_band(*class, *ir_a, i_a),
            Characteristic::FuseGg { in_a } => fuse_gg_band(*in_a, i_a),
            Characteristic::Electronic(e) => electronic_band(e, i_a),
            Characteristic::Points { min, max } => {
                let t_min = loglog(min, i_a)?;
                let t_max = loglog(max, i_a)?;
                Some(TimeBand { t_min, t_max })
            }
        }
    }

    /// The longest time the device takes to clear a fault of `i_a`: the
    /// upper side of the band. `None` when unknown.
    pub fn max_clearing_time(&self, i_a: f64) -> Option<f64> {
        self.band(i_a).map(|b| b.t_max)
    }

    /// The lowest current that is guaranteed to trip within `t_s`, found
    /// by searching the upper side of the band. `None` when the band never
    /// gets that fast within the range searched (up to 1 MA).
    pub fn current_for_trip_within(&self, t_s: f64) -> Option<f64> {
        let mut i = 1.0_f64;
        while i < 1.0e6 {
            if let Some(b) = self.band(i)
                && b.t_max <= t_s
            {
                // Refine by bisection between i/1.01 and i.
                let (mut lo, mut hi) = (i / 1.01, i);
                for _ in 0..40 {
                    let mid = 0.5 * (lo + hi);
                    match self.band(mid) {
                        Some(m) if m.t_max <= t_s => hi = mid,
                        _ => lo = mid,
                    }
                }
                return Some(hi);
            }
            i *= 1.01;
        }
        None
    }
}

fn mcb_band(curve: McbCurve, in_a: f64, i_a: f64) -> Option<TimeBand> {
    let x = i_a / in_a;
    let (m_lo, m_hi) = curve.magnetic();
    let long = if in_a > 63.0 { 7200.0 } else { 3600.0 };
    let at_2_55_max = if in_a > 32.0 { 120.0 } else { 60.0 };
    // Lower side: not before 1 h at 1.13 In, not before 1 s at 2.55 In,
    // not before 0.1 s up to the lower magnetic limit.
    let t_min = if x < 1.13 {
        f64::INFINITY
    } else if x < 2.55 {
        loglog(&[(1.13, long), (2.55, 1.0)], x)?
    } else if x < m_lo {
        loglog(&[(2.55, 1.0), (m_lo, 0.1)], x)?
    } else {
        INSTANT_MIN_S
    };
    // Upper side: by 1 h at 1.45 In, by 60 s (120 s) at 2.55 In, by 0.1 s
    // at the upper magnetic limit and above.
    let t_max = if x < 1.45 {
        f64::INFINITY
    } else if x < 2.55 {
        loglog(&[(1.45, long), (2.55, at_2_55_max)], x)?
    } else if x < m_hi {
        loglog(&[(2.55, at_2_55_max), (m_hi, 0.1)], x)?
    } else {
        0.1
    };
    Some(TimeBand { t_min, t_max })
}

fn relay_band(class: RelayClass, ir_a: f64, i_a: f64) -> Option<TimeBand> {
    let x = i_a / ir_a;
    let (tmin72, tmax72) = class.window_at_7_2();
    // Upper side: 2 h at 1.2 Ir, class time at 1.5 Ir, then I²t. No
    // guarantee below 1.2 Ir.
    let t_max = if x < 1.2 {
        f64::INFINITY
    } else if x < 1.5 {
        loglog(&[(1.2, 7200.0), (1.5, class.max_at_1_5())], x)?
    } else {
        let k = class.max_at_1_5() * 1.5 * 1.5;
        let by_slope = k / (x * x);
        // Never better than the class window at 7.2 Ir allows to claim:
        // the guaranteed value there is tmax72, and the slope gives about
        // the same.
        if x >= 7.2 {
            by_slope.min(tmax72 * (7.2 / x).powi(2))
        } else {
            by_slope
        }
    };
    // Lower side: no trip within 2 h at 1.05 Ir, not before the class
    // minimum at 7.2 Ir, I²t beyond.
    let t_min = if x < 1.05 {
        f64::INFINITY
    } else if x <= 7.2 {
        loglog(&[(1.05, 7200.0), (7.2, tmin72)], x)?
    } else {
        tmin72 * (7.2 / x).powi(2)
    };
    Some(TimeBand { t_min, t_max })
}

fn fuse_gg_band(in_a: f64, i_a: f64) -> Option<TimeBand> {
    let (inf, if_) = if in_a < 4.0 + 1e-9 {
        (1.5, 2.1)
    } else if in_a < 16.0 {
        (1.5, 1.9)
    } else {
        (1.25, 1.6)
    };
    let conv = if in_a <= 63.0 {
        3600.0
    } else if in_a <= 160.0 {
        7200.0
    } else if in_a <= 400.0 {
        10800.0
    } else {
        14400.0
    };
    let x = i_a / in_a;
    if x <= inf {
        // Does not melt within the conventional time at the non-fusing current.
        Some(TimeBand {
            t_min: conv,
            t_max: f64::INFINITY,
        })
    } else if x < if_ {
        // Between the conventional currents the standard guarantees nothing.
        Some(TimeBand {
            t_min: 0.0,
            t_max: f64::INFINITY,
        })
    } else if (x - if_).abs() < 1e-12 {
        Some(TimeBand {
            t_min: 0.0,
            t_max: conv,
        })
    } else {
        // Above the conventional fusing current only the melting data of
        // the maker can say more.
        None
    }
}

fn electronic_band(e: &Electronic, i_a: f64) -> Option<TimeBand> {
    let (lo, hi) = (0.8_f64, 1.2_f64);
    let x = i_a / e.ir_a;
    let pickup_lo = 1.05;
    let pickup_hi = 1.2;
    let isd_lo = e.isd_x * 0.9;
    let isd_hi = e.isd_x * 1.1;
    let ii_lo = e.ii_x * e.in_a / e.ir_a * 0.8;
    let ii_hi = e.ii_x * e.in_a / e.ir_a * 1.2;
    let long = |x: f64, k: f64| e.tr_s * k * (6.0 / x).powi(2);
    let t_max_long = if x < pickup_hi {
        f64::INFINITY
    } else {
        long(x, hi)
    };
    let t_min_long = if x < pickup_lo {
        f64::INFINITY
    } else {
        long(x, lo)
    };
    // Short-time stage.
    let (t_min_short, t_max_short) = if x >= isd_hi {
        (e.tsd_s * lo, e.tsd_s * hi + 0.03)
    } else if x >= isd_lo {
        (e.tsd_s * lo, f64::INFINITY)
    } else {
        (f64::INFINITY, f64::INFINITY)
    };
    // Instantaneous stage.
    let (t_min_inst, t_max_inst) = if x >= ii_hi {
        (INSTANT_MIN_S, 0.03)
    } else if x >= ii_lo {
        (INSTANT_MIN_S, f64::INFINITY)
    } else {
        (f64::INFINITY, f64::INFINITY)
    };
    Some(TimeBand {
        t_min: t_min_long.min(t_min_short).min(t_min_inst),
        t_max: t_max_long.min(t_max_short).min(t_max_inst),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, rel: f64) {
        assert!((a - b).abs() <= rel * b.abs(), "{a} vs {b}");
    }

    #[test]
    fn mcb_follows_the_test_points_of_iec_60898() {
        let c16 = Characteristic::Mcb {
            curve: McbCurve::C,
            in_a: 16.0,
        };
        // 1.13 In: must hold for an hour; below that, never guaranteed to trip.
        let b = c16.band(1.13 * 16.0).unwrap();
        near(b.t_min, 3600.0, 1e-9);
        assert!(b.t_max.is_infinite());
        assert!(c16.band(1.0 * 16.0).unwrap().t_max.is_infinite());
        // 1.45 In: trips within an hour.
        near(c16.band(1.45 * 16.0).unwrap().t_max, 3600.0, 1e-9);
        // 2.55 In: between 1 s and 60 s.
        let b = c16.band(2.55 * 16.0).unwrap();
        near(b.t_min, 1.0, 1e-9);
        near(b.t_max, 60.0, 1e-9);
        // 5 In (curve C lower magnetic limit): not within 0.1 s.
        let b = c16.band(5.0 * 16.0 - 1e-9).unwrap();
        near(b.t_min, 0.1, 1e-3);
        // 10 In: trips within 0.1 s.
        near(c16.band(10.0 * 16.0).unwrap().t_max, 0.1, 1e-9);
        near(c16.band(500.0).unwrap().t_max, 0.1, 1e-9);
    }

    #[test]
    fn mcb_curves_differ_in_the_magnetic_band() {
        let at = |curve, i| {
            Characteristic::Mcb { curve, in_a: 10.0 }
                .band(i)
                .unwrap()
                .t_max
        };
        // 60 A = 6 In: B has tripped, C has not been guaranteed to, D neither.
        assert_eq!(at(McbCurve::B, 60.0), 0.1);
        assert!(at(McbCurve::C, 60.0) > 0.1);
        assert!(at(McbCurve::D, 60.0) > at(McbCurve::C, 60.0));
    }

    #[test]
    fn long_breakers_get_the_longer_times() {
        let big = Characteristic::Mcb {
            curve: McbCurve::C,
            in_a: 80.0,
        };
        near(big.band(1.45 * 80.0).unwrap().t_max, 7200.0, 1e-9);
        near(big.band(2.55 * 80.0).unwrap().t_max, 120.0, 1e-9);
    }

    #[test]
    fn current_for_trip_within_matches_the_magnetic_limit() {
        // Curve B 16 A: certain trip within 0.1 s from 5 In = 80 A.
        let b16 = Characteristic::Mcb {
            curve: McbCurve::B,
            in_a: 16.0,
        };
        near(b16.current_for_trip_within(0.1).unwrap(), 80.0, 0.02);
        // Nothing is guaranteed faster than 0.1 s for an MCB.
        assert!(b16.current_for_trip_within(0.01).is_none());
        // Within 5 s on curve C 16 A comes from the thermal-to-magnetic join.
        let c16 = Characteristic::Mcb {
            curve: McbCurve::C,
            in_a: 16.0,
        };
        let i5 = c16.current_for_trip_within(5.0).unwrap();
        assert!(i5 > 2.55 * 16.0 && i5 < 10.0 * 16.0, "{i5}");
    }

    #[test]
    fn relay_classes_match_the_standard_points() {
        for (class, max15, window) in [
            (RelayClass::C10A, 120.0, (2.0, 10.0)),
            (RelayClass::C10, 240.0, (4.0, 10.0)),
            (RelayClass::C20, 480.0, (6.0, 20.0)),
            (RelayClass::C30, 720.0, (9.0, 30.0)),
        ] {
            let r = Characteristic::Relay { class, ir_a: 10.0 };
            near(r.band(15.0).unwrap().t_max, max15, 1e-9);
            let b = r.band(72.0).unwrap();
            near(b.t_min, window.0, 1e-9);
            // The I²t slope from the 1.5 Ir point lands at or near the class limit.
            assert!(b.t_max <= window.1 + 1e-9, "{class:?}: {}", b.t_max);
            near(r.band(12.0).unwrap().t_max, 7200.0, 1e-9);
            assert!(r.band(10.0).unwrap().t_max.is_infinite());
            near(r.band(10.5).unwrap().t_min, 7200.0, 1e-9);
        }
        // Higher class, slower.
        let slow = Characteristic::Relay {
            class: RelayClass::C30,
            ir_a: 10.0,
        };
        let fast = Characteristic::Relay {
            class: RelayClass::C10,
            ir_a: 10.0,
        };
        assert!(slow.band(60.0).unwrap().t_max > fast.band(60.0).unwrap().t_max);
        assert_eq!(RelayClass::parse(" 10a "), Some(RelayClass::C10A));
        assert_eq!(RelayClass::parse("5"), None);
    }

    #[test]
    fn gg_fuse_knows_only_the_conventional_currents() {
        let f = Characteristic::FuseGg { in_a: 20.0 };
        near(f.band(25.0).unwrap().t_min, 3600.0, 1e-9);
        assert!(f.band(25.0).unwrap().t_max.is_infinite());
        near(f.band(32.0).unwrap().t_max, 3600.0, 1e-9);
        // Between and beyond: no invented melting time.
        assert_eq!(f.band(28.0).unwrap().t_min, 0.0);
        assert!(f.band(100.0).is_none());
        // Low ratings use the 1.5 / 1.9 pair.
        let small = Characteristic::FuseGg { in_a: 10.0 };
        assert!(small.band(15.0).unwrap().t_min > 0.0);
        assert_eq!(small.band(16.0).unwrap().t_min, 0.0);
        assert_eq!(small.band(19.0).unwrap().t_max, 3600.0);
        // Conventional times lengthen with the rating.
        let big = Characteristic::FuseGg { in_a: 250.0 };
        near(big.band(1.6 * 250.0).unwrap().t_max, 10800.0, 1e-9);
    }

    #[test]
    fn electronic_stages() {
        let e = Characteristic::Electronic(Electronic {
            in_a: 250.0,
            ir_a: 250.0,
            tr_s: 12.0,
            isd_x: 6.0,
            tsd_s: 0.2,
            ii_x: 10.0,
        });
        // Long time: 12 s at 6 Ir (±20 %) is on the band near 6 Ir but short-time takes over.
        let b = e.band(3.0 * 250.0).unwrap();
        near(b.t_max, 12.0 * 1.2 * 4.0, 1e-9);
        // Short-time stage above 1.1 · Isd: definite delay.
        let b = e.band(7.0 * 250.0).unwrap();
        assert!(b.t_max < 0.5 && b.t_min > 0.1, "{b:?}");
        // Instantaneous above 1.2 · Ii.
        let b = e.band(13.0 * 250.0).unwrap();
        assert_eq!(b.t_max, 0.03);
        // Nothing guaranteed at rated current.
        assert!(e.band(250.0).unwrap().t_max.is_infinite());
    }

    #[test]
    fn maker_points_are_used_only_inside_their_range() {
        let c = Characteristic::Points {
            min: vec![(100.0, 10.0), (1000.0, 0.01)],
            max: vec![(100.0, 100.0), (1000.0, 0.1)],
        };
        // Midpoint on log axes: geometric mean of the times.
        let b = c.band(316.227766).unwrap();
        near(b.t_min, (10.0_f64 * 0.01).sqrt(), 1e-4);
        near(b.t_max, (100.0_f64 * 0.1).sqrt(), 1e-4);
        assert!(c.band(50.0).is_none());
        assert!(c.band(2000.0).is_none());
        assert!(c.band(f64::NAN).is_none());
        assert!(c.band(-1.0).is_none());
    }

    #[test]
    fn assumptions_are_listed_where_the_standard_is_silent() {
        assert!(
            Characteristic::Electronic(Electronic {
                in_a: 100.0,
                ir_a: 100.0,
                tr_s: 10.0,
                isd_x: 5.0,
                tsd_s: 0.1,
                ii_x: 10.0
            })
            .assumptions()[0]
                .contains("assumed")
        );
        assert!(
            Characteristic::Points {
                min: vec![],
                max: vec![]
            }
            .assumptions()
            .is_empty()
        );
        assert_eq!(McbCurve::from_trip(Trip::D), Some(McbCurve::D));
        assert_eq!(McbCurve::from_trip(Trip::Factor(12.0)), None);
    }
}
