//! Selectivity and back-up protection between two devices in series.
//!
//! *Selective* means that for a fault behind the downstream device only that
//! device clears it and the upstream one keeps the rest of the installation
//! running. Three things decide it, and the analysis uses them in this
//! order at every fault current from the downstream device's lowest trip
//! current up to the prospective current `ik_max`:
//!
//! 1. **Curves.** The upstream device must not trip before the downstream
//!    one has cleared: its fastest time must be longer than the downstream
//!    device's slowest time (the bands may not overlap).
//! 2. **Energy.** Where the bands overlap but the downstream device is
//!    current-limiting, selectivity can still hold: the peak and the I²t it
//!    lets through stay below what the upstream device needs to start to
//!    trip (needs the maker's let-through table of the downstream device,
//!    and a no-trip energy for the upstream one; a fuse brings its own).
//! 3. **Maker tables.** A tested selectivity limit for the exact pair
//!    overrides the computation: it is the maker's guarantee.
//!
//! A delayed upstream trip (electronic breaker with a short-time delay) is
//! selective as far as its curve says, but no further than its short-time
//! withstand `Icw`.
//!
//! The result lists the **non-selective current ranges**, not just one
//! number: a current-limiting downstream device can make a pair selective
//! again at very high currents, and the designer should see that.
//!
//! *Back-up protection* is the other way to live with a downstream device
//! whose breaking capacity is lower than the prospective current: the
//! upstream device limits the fault so the downstream one is not asked to
//! break more than it can (see [`backup`]). Both devices then trip, so the
//! pair is by definition not selective in that range.

use serde::{Deserialize, Serialize};

use super::devices::{Device, Limit, MakerTables};

/// Settings of the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Factor on the downstream device's slowest time before the upstream
    /// one may start: `1.0` is "the bands do not touch", larger values keep
    /// a gap. Never below 1.
    pub time_margin: f64,
    /// Number of currents tried between the lowest trip current and
    /// `ik_max` (log spaced; the key currents of both devices are added).
    pub steps: usize,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            time_margin: 1.0,
            steps: 600,
        }
    }
}

/// What happens to a fault of one current in a pair of devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    /// Neither device trips.
    NoTrip,
    /// Only the downstream device trips: the bands are apart.
    DownstreamOnly,
    /// Only the downstream device trips because it limits the let-through
    /// energy and peak below what the upstream device needs.
    DownstreamOnlyByEnergy,
    /// The downstream device cannot trip here but the upstream one will.
    UpstreamOnly,
    /// Both may trip: not selective.
    Both,
    /// The upstream device relies on its delay but the current is above its
    /// short-time withstand: not selective, and not safe either.
    BeyondWithstand,
}

impl Outcome {
    /// Whether the outcome keeps the upstream device in.
    pub fn is_selective(self) -> bool {
        matches!(
            self,
            Outcome::NoTrip | Outcome::DownstreamOnly | Outcome::DownstreamOnlyByEnergy
        )
    }
}

/// What the verdict of a pair is based on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Basis {
    /// Time-current bands.
    Curves,
    /// Bands plus let-through energy of the downstream device.
    CurvesAndEnergy,
    /// A maker's tested table for the pair.
    MakerTable,
}

/// The overall verdict for a pair up to the prospective current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Verdict {
    /// Selective at every current up to `ik_max`.
    Total,
    /// Selective up to a limit; above it (or in some ranges) not.
    Partial,
    /// Not selective at any fault current.
    None,
}

/// The result for one pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selectivity {
    /// Overall verdict.
    pub verdict: Verdict,
    /// What it is based on.
    pub basis: Basis,
    /// The prospective current the analysis ran up to, amperes.
    pub ik_max_a: f64,
    /// The lowest current looked at (where the downstream device can first
    /// trip), amperes.
    pub from_a: f64,
    /// Selective up to this current, amperes (`ik_max_a` when total).
    pub limit_a: f64,
    /// Current ranges `(from, to)` in amperes where the pair is not
    /// selective, in rising order.
    pub non_selective: Vec<(f64, f64)>,
    /// The limit computed from the curves and energy alone, even when a
    /// maker table decides the verdict, amperes.
    pub computed_limit_a: f64,
}

impl Selectivity {
    /// Whether a fault of `i_a` amperes is cleared selectively.
    pub fn is_selective_at(&self, i_a: f64) -> bool {
        !self
            .non_selective
            .iter()
            .any(|&(lo, hi)| i_a >= lo && i_a <= hi)
    }
}

/// What happens in the pair at one fault current.
pub fn outcome_at(up: &Device, down: &Device, i_a: f64, settings: &Settings) -> Outcome {
    let margin = settings.time_margin.max(1.0);
    let d = down.curve.trip_time_range(i_a);
    let u = up.curve.trip_time_range(i_a);
    if let Some(icw) = up.icw_a
        && i_a > icw
        && u.is_some()
    {
        return Outcome::BeyondWithstand;
    }
    match (d, u) {
        (None, None) => Outcome::NoTrip,
        (None, Some(_)) => Outcome::UpstreamOnly,
        (Some(_), None) => Outcome::DownstreamOnly,
        (Some((_, d_max)), Some((u_min, _))) => {
            if u_min > d_max * margin {
                return Outcome::DownstreamOnly;
            }
            if energy_discriminates(up, down, i_a) {
                Outcome::DownstreamOnlyByEnergy
            } else {
                Outcome::Both
            }
        }
    }
}

/// The energy test: needs the downstream let-through table and an upstream
/// no-trip energy; the upstream peak pickup, if it has an instantaneous
/// trip, must stay above the downstream peak.
fn energy_discriminates(up: &Device, down: &Device, i_a: f64) -> bool {
    let (Some(no_trip), Some(_)) = (up.no_trip_i2t, down.let_through.as_ref()) else {
        return false;
    };
    let (ip, e) = down.let_through_at(i_a);
    if e >= no_trip {
        return false;
    }
    up.peak_trip_a().is_none_or(|p| ip < p)
}

/// The currents worth trying: a log grid plus the break points of both
/// devices.
fn grid(up: &Device, down: &Device, from_a: f64, to_a: f64, steps: usize) -> Vec<f64> {
    let n = steps.max(8);
    let (l0, l1) = (from_a.ln(), to_a.ln());
    let mut v: Vec<f64> = (0..=n)
        .map(|k| (l0 + (l1 - l0) * k as f64 / n as f64).exp())
        .collect();
    for d in [up, down] {
        for &(m, _) in d
            .curve
            .min_band()
            .points()
            .iter()
            .chain(d.curve.max_band().points())
        {
            v.push(m * d.curve.rated_a);
        }
        if let Some(icw) = d.icw_a {
            v.push(icw);
            v.push(icw * (1.0 + 1e-9));
        }
    }
    v.retain(|x| x.is_finite() && *x >= from_a && *x <= to_a);
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9 * a.abs().max(1.0));
    v
}

/// Refines a change of selectivity between a selective current `ok` and a
/// non-selective `bad` (or the other way) to the boundary.
fn refine(f: &dyn Fn(f64) -> bool, ok: f64, bad: f64) -> f64 {
    let (mut a, mut b) = (ok, bad);
    for _ in 0..60 {
        let mid = (a * b).sqrt();
        if f(mid) == f(a) {
            a = mid;
        } else {
            b = mid;
        }
    }
    // The boundary lies between a and b; report the side that is not
    // selective so a limit is never overstated.
    if f(a) { b } else { a }
}

/// Analyses selectivity of `up` behind which `down` sits, for a fault in
/// the circuit of `down` with prospective current up to `ik_max_a`.
pub fn analyse(
    up: &Device,
    down: &Device,
    ik_max_a: f64,
    tables: &MakerTables,
    settings: &Settings,
) -> Selectivity {
    let from_a = down.lowest_trip_a();
    let mut computed = compute(up, down, from_a, ik_max_a, settings);
    if let Some(row) = tables.row(&up.name, &down.name)
        && let Some(limit) = row.selectivity
        && ik_max_a.is_finite()
    {
        let computed_limit = computed.limit_a;
        let mut s = Selectivity {
            basis: Basis::MakerTable,
            computed_limit_a: computed_limit,
            non_selective: Vec::new(),
            ..computed.clone()
        };
        match limit {
            Limit::Total => {
                s.verdict = Verdict::Total;
                s.limit_a = ik_max_a;
            }
            Limit::Amps(x) if x >= ik_max_a => {
                s.verdict = Verdict::Total;
                s.limit_a = ik_max_a;
            }
            Limit::Amps(x) => {
                let x = x.max(from_a);
                s.limit_a = x;
                s.non_selective = vec![(x, ik_max_a)];
                s.verdict = if x <= from_a {
                    Verdict::None
                } else {
                    Verdict::Partial
                };
            }
        }
        return s;
    }
    computed.computed_limit_a = computed.limit_a;
    computed
}

fn compute(
    up: &Device,
    down: &Device,
    from_a: f64,
    ik_max_a: f64,
    settings: &Settings,
) -> Selectivity {
    let empty = |basis| Selectivity {
        verdict: Verdict::Total,
        basis,
        ik_max_a,
        from_a,
        limit_a: ik_max_a,
        non_selective: Vec::new(),
        computed_limit_a: ik_max_a,
    };
    if !(from_a.is_finite() && ik_max_a.is_finite() && ik_max_a > from_a) {
        return empty(Basis::Curves);
    }
    let currents = grid(up, down, from_a, ik_max_a, settings.steps);
    let sel = |i: f64| outcome_at(up, down, i, settings).is_selective();
    let mut used_energy = false;
    let mut ranges: Vec<(f64, f64)> = Vec::new();
    let mut open: Option<f64> = None;
    let mut prev: Option<(f64, bool)> = None;
    for &i in &currents {
        let o = outcome_at(up, down, i, settings);
        used_energy |= o == Outcome::DownstreamOnlyByEnergy;
        let ok = o.is_selective();
        match (prev, ok) {
            (None, false) => open = Some(i),
            (Some((p, true)), false) => open = Some(refine(&sel, p, i)),
            (Some((p, false)), true) => {
                if let Some(start) = open.take() {
                    ranges.push((start, refine(&sel, i, p)));
                }
            }
            _ => {}
        }
        prev = Some((i, ok));
    }
    if let Some(start) = open {
        ranges.push((start, ik_max_a));
    }
    let basis = if used_energy {
        Basis::CurvesAndEnergy
    } else {
        Basis::Curves
    };
    let mut s = empty(basis);
    if let Some(&(first, _)) = ranges.first() {
        s.limit_a = first;
        s.verdict = if first <= from_a * (1.0 + 1e-9) {
            Verdict::None
        } else {
            Verdict::Partial
        };
        s.non_selective = ranges;
    }
    s.computed_limit_a = s.limit_a;
    s
}

/// How a downstream device whose breaking capacity is lower than the
/// prospective current is covered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Backup {
    /// The downstream device can break the current itself.
    NotNeeded,
    /// Covered by the upstream device.
    Covered(BackupBasis),
    /// Not covered; the text says why.
    NotCovered(String),
}

/// What a back-up verdict rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupBasis {
    /// A maker's tested back-up table for the pair (the preferred proof).
    MakerTable,
    /// A comparison of the upstream let-through peak and I²t with what the
    /// downstream device withstands.
    LetThrough,
}

/// Checks back-up protection for a fault of `ik_a` at the downstream device.
pub fn backup(up: &Device, down: &Device, ik_a: f64, tables: &MakerTables) -> Backup {
    if !(ik_a.is_finite() && ik_a > down.icu_a) {
        return Backup::NotNeeded;
    }
    if up.icu_a < ik_a {
        return Backup::NotCovered(format!(
            "the upstream device {} breaks only {} kA but the prospective current is {} kA",
            up.name,
            ka(up.icu_a),
            ka(ik_a)
        ));
    }
    if let Some(limit) = tables.row(&up.name, &down.name).and_then(|r| r.backup) {
        return match limit {
            Limit::Total => Backup::Covered(BackupBasis::MakerTable),
            Limit::Amps(x) if x >= ik_a => Backup::Covered(BackupBasis::MakerTable),
            Limit::Amps(x) => Backup::NotCovered(format!(
                "the maker's table covers {} up to {} kA behind {} but the prospective current is {} kA",
                down.name,
                ka(x),
                up.name,
                ka(ik_a)
            )),
        };
    }
    match (up.let_through.as_ref(), down.withstand) {
        (Some(_), Some(w)) => {
            let (ip, e) = up.let_through_at(ik_a);
            if ip <= w.peak_a && e <= w.i2t {
                Backup::Covered(BackupBasis::LetThrough)
            } else {
                Backup::NotCovered(format!(
                    "{} lets through {} kA peak and {} A²s but {} withstands {} kA and {} A²s",
                    up.name,
                    ka(ip),
                    sci(e),
                    down.name,
                    ka(w.peak_a),
                    sci(w.i2t)
                ))
            }
        }
        _ => Backup::NotCovered(format!(
            "no tested back-up value for {} behind {}, and no let-through data to compare; add the maker's table",
            down.name, up.name
        )),
    }
}

fn ka(a: f64) -> String {
    let k = a / 1000.0;
    if k >= 100.0 {
        format!("{k:.0}")
    } else if k >= 10.0 {
        format!("{k:.1}")
    } else {
        format!("{k:.2}")
    }
}

fn sci(x: f64) -> String {
    format!("{x:.3e}")
}

#[cfg(test)]
mod tests {
    use super::super::curves::{Lsi, McbCurve, TripCurve};
    use super::super::devices::{LetThrough, PairRow, Withstand};
    use super::*;

    fn mcb(a: f64, c: McbCurve, icu_ka: f64) -> Device {
        Device::new(
            format!("{} {a}", c.letter()),
            TripCurve::mcb(a, c).unwrap(),
            icu_ka,
        )
    }

    fn fuse(a: f64) -> Device {
        Device::new(
            format!("gG {a}"),
            TripCurve::fuse_gg(a, None).unwrap(),
            120.0,
        )
    }

    fn run(up: &Device, down: &Device, ik: f64) -> Selectivity {
        analyse(up, down, ik, &MakerTables::default(), &Settings::default())
    }

    /// Worked example: C32 behind which a C10 sits. The C32 may trip
    /// instantly from 5 x 32 = 160 A (and has a 0.1 s floor until then); the
    /// C10 clears within 12 ms above 100 A. So the pair is selective up to
    /// the C32's lower instantaneous limit, 160 A, and not above.
    #[test]
    fn mcb_behind_mcb_is_selective_up_to_the_upstream_magnetic_pickup() {
        let s = run(
            &mcb(32.0, McbCurve::C, 10.0),
            &mcb(10.0, McbCurve::C, 10.0),
            6000.0,
        );
        assert_eq!(s.verdict, Verdict::Partial);
        assert!((s.limit_a - 160.0).abs() < 2.0, "{}", s.limit_a);
        assert_eq!(s.non_selective.len(), 1);
        assert_eq!(s.non_selective[0].1, 6000.0);
        assert!(s.is_selective_at(100.0) && !s.is_selective_at(1000.0));
        assert_eq!(s.basis, Basis::Curves);
    }

    /// Worked example: gG fuses are selective when the upstream pre-arcing
    /// I²t exceeds the downstream total I²t: the classic ratio of about
    /// 1.6 between ratings.
    #[test]
    fn fuses_are_selective_by_rating_ratio() {
        let up = fuse(63.0);
        let total = run(&up, &fuse(25.0), 50_000.0);
        assert_eq!(total.verdict, Verdict::Total, "{:?}", total.non_selective);
        let weak = run(&up, &fuse(50.0), 50_000.0);
        assert_ne!(weak.verdict, Verdict::Total);
        // The turnover is near the 1.6 rule of thumb.
        let ok = run(&fuse(100.0), &fuse(63.0), 50_000.0);
        assert_eq!(ok.verdict, Verdict::Total, "100/63 = 1.59");
        let bad = run(&fuse(100.0), &fuse(80.0), 50_000.0);
        assert_ne!(bad.verdict, Verdict::Total);
    }

    #[test]
    fn fuse_ratio_threshold_is_close_to_one_point_six() {
        let up = fuse(100.0);
        let mut threshold = None;
        for down_a in [25.0, 32.0, 35.0, 40.0, 50.0, 63.0, 80.0, 100.0] {
            if run(&up, &fuse(down_a), 50_000.0).verdict != Verdict::Total {
                threshold = Some(down_a);
                break;
            }
        }
        // 100 / 63 = 1.59 is selective, 100 / 80 = 1.25 is not.
        assert_eq!(threshold, Some(80.0));
    }

    /// A downstream fuse behind an upstream MCB is the wrong way round and
    /// never selective at overload when the fuse is bigger.
    #[test]
    fn oversized_downstream_is_not_selective_at_all() {
        let s = run(&mcb(16.0, McbCurve::C, 10.0), &fuse(63.0), 10_000.0);
        assert_ne!(s.verdict, Verdict::Total);
        assert!(s.non_selective[0].0 < 400.0);
    }

    /// Energy discrimination: a limiting C16 behind a thermal-magnetic
    /// upstream whose bands overlap at 6 kA, with let-through data.
    #[test]
    fn energy_discrimination_needs_data_and_then_helps() {
        let mut up = Device::new(
            "MCCB 160",
            TripCurve::mcb(160.0, McbCurve::D).unwrap(),
            25.0,
        );
        let down = mcb(16.0, McbCurve::C, 10.0);
        // Without any data: bands overlap above the upstream's pickup and
        // the analysis cannot claim energy discrimination.
        let plain = run(&up, &down, 6000.0);
        assert_eq!(plain.basis, Basis::Curves);
        assert_ne!(plain.verdict, Verdict::Total);
        // With a let-through table for the downstream and a no-trip energy
        // for the upstream the overlap region becomes selective as long as
        // the let-through stays below it.
        let mut down2 = down.clone();
        down2.let_through = Some(
            LetThrough::new(vec![
                (1000.0, 900.0, 3.0e3),
                (6000.0, 1800.0, 2.0e4),
                (25_000.0, 2400.0, 3.0e4),
            ])
            .unwrap(),
        );
        up.no_trip_i2t = Some(5.0e4);
        up.curve.instantaneous = None;
        let s = run(&up, &down2, 6000.0);
        // 160 A D-curve upstream: lo pickup is at 1600 A; below this the
        // curves already separate, above it the energy decides.
        assert_eq!(s.verdict, Verdict::Total, "{:?}", s.non_selective);
        assert_eq!(s.basis, Basis::CurvesAndEnergy);
        // A too-hungry downstream breaks it.
        let mut hungry = down2.clone();
        hungry.let_through =
            Some(LetThrough::new(vec![(1000.0, 900.0, 9.0e5), (6000.0, 1800.0, 2.0e6)]).unwrap());
        let s = run(&up, &hungry, 6000.0);
        assert_ne!(s.verdict, Verdict::Total);
    }

    /// A maker's tested limit overrides the computation.
    #[test]
    fn maker_table_wins() {
        let up = mcb(32.0, McbCurve::C, 10.0);
        let down = mcb(10.0, McbCurve::C, 10.0);
        let mut tables = MakerTables::default();
        tables.rows.push(PairRow {
            upstream: up.name.clone(),
            downstream: down.name.clone(),
            selectivity: Some(Limit::Amps(3000.0)),
            backup: None,
        });
        let s = analyse(&up, &down, 6000.0, &tables, &Settings::default());
        assert_eq!(s.basis, Basis::MakerTable);
        assert_eq!(s.verdict, Verdict::Partial);
        assert_eq!(s.limit_a, 3000.0);
        assert!((s.computed_limit_a - 160.0).abs() < 2.0);
        assert_eq!(s.non_selective, vec![(3000.0, 6000.0)]);
        tables.rows[0].selectivity = Some(Limit::Total);
        let s = analyse(&up, &down, 6000.0, &tables, &Settings::default());
        assert_eq!(s.verdict, Verdict::Total);
        tables.rows[0].selectivity = Some(Limit::Amps(8000.0));
        let s = analyse(&up, &down, 6000.0, &tables, &Settings::default());
        assert_eq!(s.verdict, Verdict::Total);
    }

    /// A delayed electronic upstream is selective by time, up to Icw.
    #[test]
    fn delayed_upstream_is_selective_up_to_icw() {
        let curve = TripCurve::lsi(Lsi {
            ir_a: 630.0,
            tr_s: 12.0,
            isd: Some(6.0),
            tsd_s: 0.3,
            i2t_ramp: false,
            ii: None,
            opening_s: 0.05,
        })
        .unwrap();
        let mut up = Device::new("ACB 630", curve, 50.0);
        let down = mcb(63.0, McbCurve::C, 25.0);
        let s = run(&up, &down, 20_000.0);
        assert_eq!(s.verdict, Verdict::Total, "{:?}", s.non_selective);
        // With a short-time withstand of 15 kA the delay cannot be relied
        // on above it.
        up.icw_a = Some(15_000.0);
        let s = run(&up, &down, 20_000.0);
        assert_eq!(s.verdict, Verdict::Partial);
        assert!((s.limit_a - 15_000.0).abs() < 50.0, "{}", s.limit_a);
        assert_eq!(
            outcome_at(&up, &down, 18_000.0, &Settings::default()),
            Outcome::BeyondWithstand
        );
    }

    #[test]
    fn outcomes_at_single_currents() {
        let up = mcb(32.0, McbCurve::C, 10.0);
        let down = mcb(10.0, McbCurve::C, 10.0);
        let st = Settings::default();
        assert_eq!(outcome_at(&up, &down, 5.0, &st), Outcome::NoTrip);
        assert_eq!(outcome_at(&up, &down, 100.0, &st), Outcome::DownstreamOnly);
        assert_eq!(outcome_at(&up, &down, 1000.0, &st), Outcome::Both);
        // Time margin keeps a gap: 100 A has some room but 150 A does not
        // once a 100 x margin is asked for.
        let wide = Settings {
            time_margin: 100.0,
            ..st
        };
        assert_eq!(outcome_at(&up, &down, 150.0, &wide), Outcome::Both);
    }

    #[test]
    fn degenerate_ranges_do_not_panic() {
        let up = mcb(32.0, McbCurve::C, 10.0);
        let down = mcb(10.0, McbCurve::C, 10.0);
        let s = run(&up, &down, 5.0);
        assert_eq!(s.verdict, Verdict::Total);
        let s = run(&up, &down, f64::NAN);
        assert_eq!(s.verdict, Verdict::Total);
    }

    #[test]
    fn backup_rules() {
        let up = mcb(63.0, McbCurve::C, 25.0);
        let down = mcb(16.0, McbCurve::C, 6.0);
        let tables = MakerTables::default();
        assert_eq!(backup(&up, &down, 5000.0, &tables), Backup::NotNeeded);
        // 10 kA against a 6 kA downstream: no data, not covered.
        assert!(matches!(
            backup(&up, &down, 10_000.0, &tables),
            Backup::NotCovered(_)
        ));
        // A table covering up to 25 kA.
        let tables = MakerTables {
            rows: vec![PairRow {
                upstream: up.name.clone(),
                downstream: down.name.clone(),
                selectivity: None,
                backup: Some(Limit::Amps(25_000.0)),
            }],
        };
        assert_eq!(
            backup(&up, &down, 10_000.0, &tables),
            Backup::Covered(BackupBasis::MakerTable)
        );
        // Beyond the table.
        assert!(matches!(
            backup(&up, &down, 30_000.0, &tables),
            Backup::NotCovered(_)
        ));
        // The upstream must itself be able to break it.
        let weak_up = mcb(63.0, McbCurve::C, 6.0);
        match backup(&weak_up, &down, 10_000.0, &MakerTables::default()) {
            Backup::NotCovered(t) => assert!(t.contains("breaks only 6")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn backup_by_let_through_compares_peak_and_energy() {
        let mut up = mcb(63.0, McbCurve::C, 50.0);
        up.let_through = Some(
            LetThrough::new(vec![(6000.0, 3000.0, 3.0e4), (50_000.0, 7000.0, 2.0e5)]).unwrap(),
        );
        let mut down = mcb(16.0, McbCurve::C, 6.0);
        down.withstand = Some(Withstand {
            peak_a: 8000.0,
            i2t: 2.5e5,
        });
        let t = MakerTables::default();
        assert_eq!(
            backup(&up, &down, 20_000.0, &t),
            Backup::Covered(BackupBasis::LetThrough)
        );
        down.withstand = Some(Withstand {
            peak_a: 4000.0,
            i2t: 2.5e5,
        });
        assert!(matches!(
            backup(&up, &down, 20_000.0, &t),
            Backup::NotCovered(_)
        ));
    }
}
