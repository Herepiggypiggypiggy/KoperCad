//! A protective device as the coordination analysis sees it: its time-current
//! band, breaking capacity and the energy it lets through.
//!
//! Maker data (let-through tables, selectivity and back-up tables) is
//! *user-supplied*: the program ships the standard bands of
//! [`curves`](super::curves) and nothing from any maker. Where a maker value
//! is missing the analysis says so (it falls back to the curve bands or to a
//! non-limiting estimate) rather than inventing a figure.

use serde::{Deserialize, Serialize};

use super::curves::{CurveError, TripCurve};

/// Peak factor κ of IEC 60909-0 for a source with the given ratio `R/X`:
/// the first peak of the short-circuit current is `κ·√2·Ik`.
pub fn peak_factor(r_over_x: f64) -> f64 {
    let rx = if r_over_x.is_finite() && r_over_x >= 0.0 {
        r_over_x
    } else {
        0.0
    };
    1.02 + 0.98 * (-3.0 * rx).exp()
}

/// What a current-limiting device lets through at a prospective current:
/// the peak and the energy. Points `(prospective rms A, peak A, I²t A²s)`,
/// straight in log-log space; the table is user data from a maker's
/// let-through curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LetThrough {
    points: Vec<(f64, f64, f64)>,
}

impl LetThrough {
    /// Builds a table from points with rising prospective currents.
    pub fn new(points: Vec<(f64, f64, f64)>) -> Result<LetThrough, CurveError> {
        if points.len() < 2 {
            return Err(CurveError::TooFewPoints);
        }
        for &(i, ip, e) in &points {
            if ![i, ip, e].iter().all(|x| x.is_finite() && *x > 0.0) {
                return Err(CurveError::BadNumber);
            }
        }
        if points.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err(CurveError::NotIncreasing);
        }
        Ok(LetThrough { points })
    }

    /// The table's points.
    pub fn points(&self) -> &[(f64, f64, f64)] {
        &self.points
    }

    /// Peak current and I²t at a prospective rms current, `None` below the
    /// first point of the table (the device does not limit there).
    pub fn at(&self, prospective_a: f64) -> Option<(f64, f64)> {
        if !(prospective_a.is_finite() && prospective_a >= self.points[0].0) {
            return None;
        }
        let n = self.points.len();
        let i = self
            .points
            .partition_point(|p| p.0 <= prospective_a)
            .saturating_sub(1)
            .min(n - 2);
        let (i0, p0, e0) = self.points[i];
        let (i1, p1, e1) = self.points[i + 1];
        let f = (prospective_a.ln() - i0.ln()) / (i1.ln() - i0.ln());
        let lerp = |a: f64, b: f64| (a.ln() + f * (b.ln() - a.ln())).exp();
        Some((lerp(p0, p1), lerp(e0, e1)))
    }
}

/// What a device can take on its own terminals while a faster upstream
/// device limits the current (its tested withstand when used as the
/// downstream half of a back-up pair).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Withstand {
    /// Highest peak current, amperes.
    pub peak_a: f64,
    /// Highest let-through energy, A²s.
    pub i2t: f64,
}

/// A protective device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The type designation: the key under which maker tables are looked up
    /// (`"iC60N C16"`, `"3VA2 100"`, `"NH00 gG 63"`).
    pub name: String,
    /// Its time-current band.
    pub curve: TripCurve,
    /// Rated ultimate breaking capacity `Icu` (or `Icn` for a miniature
    /// breaker, `Icu` for a fuse), amperes rms.
    pub icu_a: f64,
    /// Rated service breaking capacity `Ics`, amperes rms, if known.
    pub ics_a: Option<f64>,
    /// Let-through table of a current-limiting device (user data).
    pub let_through: Option<LetThrough>,
    /// Rated short-time withstand `Icw` of a breaker whose trip has a
    /// delay, amperes rms. Set only for such units: it caps the current up
    /// to which the delay can be relied on.
    pub icw_a: Option<f64>,
    /// The energy the device passes without operating, A²s: for a fuse its
    /// smallest pre-arcing I²t (taken from the curve when not set).
    pub no_trip_i2t: Option<f64>,
    /// Tested withstand as the downstream half of a back-up pair.
    pub withstand: Option<Withstand>,
}

impl Device {
    /// A device from its curve and breaking capacity in kiloamperes.
    pub fn new(name: impl Into<String>, curve: TripCurve, icu_ka: f64) -> Device {
        let no_trip_i2t = curve.energy.map(|e| e.pre_arcing_min);
        Device {
            name: name.into(),
            curve,
            icu_a: icu_ka * 1000.0,
            ics_a: None,
            let_through: None,
            icw_a: None,
            no_trip_i2t,
            withstand: None,
        }
    }

    /// The rated current of the curve, amperes.
    pub fn rated_a(&self) -> f64 {
        self.curve.rated_a
    }

    /// The lowest current at which the device can operate at all.
    pub fn lowest_trip_a(&self) -> f64 {
        self.curve.min_band().points()[0].0 * self.curve.rated_a
    }

    /// Peak current and energy the device lets a fault of `prospective_a`
    /// pass: the maker's table when there is one, otherwise a non-limiting
    /// estimate (peak `κ√2·I` with `κ = 2` as the worst case, energy
    /// `I²` times the slowest clearing time of the band, at least a half
    /// cycle).
    pub fn let_through_at(&self, prospective_a: f64) -> (f64, f64) {
        let unlimited = {
            let t = self
                .curve
                .trip_time_range(prospective_a)
                .map_or(0.01, |(_, hi)| hi.clamp(0.01, 5.0));
            (
                2.0 * std::f64::consts::SQRT_2 * prospective_a,
                prospective_a * prospective_a * t,
            )
        };
        match self.let_through.as_ref().and_then(|t| t.at(prospective_a)) {
            Some((ip, e)) => (ip.min(unlimited.0), e.min(unlimited.1)),
            None => unlimited,
        }
    }

    /// The peak current (amperes) at which the instantaneous trip may
    /// operate: its lowest pickup as a peak value. `None` without an
    /// instantaneous trip.
    pub fn peak_trip_a(&self) -> Option<f64> {
        self.curve
            .instantaneous
            .map(|i| i.lo * self.curve.rated_a * std::f64::consts::SQRT_2)
    }
}

/// A maker's value for a pair of devices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Limit {
    /// Up to this current, amperes rms.
    Amps(f64),
    /// Total: up to the breaking capacity of the downstream device (the
    /// maker's "T" in a selectivity table).
    Total,
}

/// One row of a maker's coordination table: for this upstream and
/// downstream pair, how far selectivity goes and how far back-up
/// protection goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairRow {
    /// Type designation of the upstream device.
    pub upstream: String,
    /// Type designation of the downstream device.
    pub downstream: String,
    /// Selectivity limit, if the table gives one.
    pub selectivity: Option<Limit>,
    /// Back-up (cascading) limit, if the table gives one.
    pub backup: Option<Limit>,
}

/// Maker coordination tables, as user-supplied data (e.g. loaded from a
/// file the user typed or exported from a maker's tool).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MakerTables {
    /// All rows.
    pub rows: Vec<PairRow>,
}

impl MakerTables {
    /// The row for a pair, matched on the type designations (case and
    /// surrounding spaces ignored).
    pub fn row(&self, upstream: &str, downstream: &str) -> Option<&PairRow> {
        let key = |s: &str| s.trim().to_ascii_lowercase();
        self.rows
            .iter()
            .find(|r| key(&r.upstream) == key(upstream) && key(&r.downstream) == key(downstream))
    }

    /// Reads tables from JSON text.
    pub fn from_json(text: &str) -> Result<MakerTables, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Writes the tables as JSON text.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::curves::McbCurve;
    use super::*;

    #[test]
    fn peak_factor_runs_from_1_02_to_2() {
        assert!((peak_factor(0.0) - 2.0).abs() < 1e-12);
        assert!((peak_factor(10.0) - 1.02).abs() < 1e-6);
        // R/X = 0.1 (typical near a transformer): about 1.75.
        assert!((peak_factor(0.1) - 1.7460).abs() < 1e-3);
        assert!((peak_factor(f64::NAN) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn let_through_interpolates_and_is_silent_below_the_table() {
        let t = LetThrough::new(vec![(1000.0, 1500.0, 1.0e4), (10_000.0, 4000.0, 1.0e5)]).unwrap();
        assert_eq!(t.at(500.0), None);
        let (ip, e) = t.at(1000.0).unwrap();
        assert!((ip - 1500.0).abs() < 1e-6 && (e - 1.0e4).abs() < 1e-6);
        let (_, e) = t.at(10f64.sqrt() * 1000.0).unwrap();
        assert!((e / (1.0e4f64 * 1.0e5).sqrt() - 1.0).abs() < 1e-9, "{e}");
        // Above the last point the last slope continues.
        let (ip, _) = t.at(100_000.0).unwrap();
        assert!(ip > 4000.0);
        assert!(LetThrough::new(vec![(1.0, 1.0, 1.0)]).is_err());
        assert!(LetThrough::new(vec![(2.0, 1.0, 1.0), (1.0, 1.0, 1.0)]).is_err());
    }

    #[test]
    fn device_let_through_never_exceeds_the_unlimited_estimate() {
        let curve = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        let mut d = Device::new("C16", curve, 6.0);
        let (ip0, e0) = d.let_through_at(6000.0);
        assert!(ip0 > 10_000.0 && e0 > 1e5);
        d.let_through =
            Some(LetThrough::new(vec![(1000.0, 800.0, 5.0e3), (6000.0, 1500.0, 2.0e4)]).unwrap());
        let (ip, e) = d.let_through_at(6000.0);
        assert!((ip - 1500.0).abs() < 1e-6 && (e - 2.0e4).abs() < 1e-6);
        assert_eq!(d.lowest_trip_a(), 1.13 * 16.0);
        assert_eq!(d.peak_trip_a(), Some(5.0 * 16.0 * 2f64.sqrt()));
    }

    #[test]
    fn maker_tables_match_names_loosely_and_roundtrip() {
        let t = MakerTables {
            rows: vec![PairRow {
                upstream: "NSX100".into(),
                downstream: " iC60N C16 ".into(),
                selectivity: Some(Limit::Amps(6000.0)),
                backup: Some(Limit::Total),
            }],
        };
        assert!(t.row("nsx100", "IC60N C16").is_some());
        assert!(t.row("nsx100", "iC60N C10").is_none());
        let back = MakerTables::from_json(&t.to_json()).unwrap();
        assert_eq!(back, t);
    }
}
