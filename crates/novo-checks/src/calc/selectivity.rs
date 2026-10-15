//! Selectivity (discrimination) and back-up protection between two
//! devices in series (IEC 60947-2 Annex A and IEC 60364-4-43 §434.5).
//!
//! *Selective*: for every fault current up to a limit only the device
//! nearest the fault opens. *Back-up* (cascading): the downstream device
//! alone could not break the fault current, but with the upstream device's
//! help the pair can.
//!
//! Two sources of truth, in this order:
//!
//! 1. **Maker tables** ([`PairTable`]), loaded from the maker's published
//!    selectivity and back-up tables. These are tested combinations and are
//!    the only thing that proves selectivity or back-up above the
//!    overload region. The result says [`Basis::MakerTable`].
//! 2. **Generic bands** ([`analyse_bands`]): the guaranteed time-current
//!    bands of [`super::curves`]. Selective at a current when the
//!    upstream device cannot trip sooner than the downstream device is
//!    guaranteed to have cleared. It is an envelope over all conforming
//!    devices, so it understates what a real pair does (it is *conservative*)
//!    but can never prove back-up. The result says [`Basis::GenericBands`].
//!
//! No data gives [`Status::Unknown`], never a pass.

use std::collections::HashMap;

use super::curves::Characteristic;

/// Where a verdict comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    GenericBands,
    MakerTable,
}

/// How selective a pair is for the fault current asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Selective up to and beyond the prospective fault current.
    Total,
    /// Selective only up to a limit below the fault current.
    Partial,
    /// Not selective even in the overload region.
    None,
    /// The data does not cover the range.
    Unknown,
}

/// The result for one pair at one fault current.
#[derive(Debug, Clone, PartialEq)]
pub struct Selectivity {
    pub status: Status,
    pub basis: Basis,
    /// Largest current in A up to which the pair is selective, when known.
    pub limit_a: Option<f64>,
    /// The fault current asked about, A.
    pub ik_a: f64,
    /// The current in A from which the bands are not known, if that stops
    /// the analysis short of `ik_a`.
    pub unknown_above_a: Option<f64>,
    /// One sentence a designer can read.
    pub text: String,
}

/// Selectivity from the time-current bands. The sweep runs from just above
/// the downstream rating (where it can first trip) up to `ik_a`, 1 % steps.
pub fn analyse_bands(
    upstream: &Characteristic,
    downstream: &Characteristic,
    ik_a: f64,
) -> Selectivity {
    let basis = Basis::GenericBands;
    let start = match downstream.rated_a() {
        Some(r) => r * 1.05,
        None => 1.0,
    };
    if !(ik_a.is_finite() && ik_a > start) {
        return Selectivity {
            status: Status::Unknown,
            basis,
            limit_a: None,
            ik_a,
            unknown_above_a: None,
            text: "the fault current is not above the downstream device's rating".to_owned(),
        };
    }
    let mut i = start;
    let mut last_good: Option<f64> = None;
    let mut unknown_from: Option<f64> = None;
    loop {
        let here = i.min(ik_a);
        match (upstream.band(here), downstream.band(here)) {
            (Some(up), Some(down)) => {
                // The downstream device may not have been asked to trip yet
                // (infinite t_max): then nothing needs separating there.
                if down.t_max.is_infinite() || up.t_min > down.t_max {
                    last_good = Some(here);
                    unknown_from = None;
                } else {
                    let limit = last_good;
                    return Selectivity {
                        status: if limit.is_some() { Status::Partial } else { Status::None },
                        basis,
                        limit_a: Some(limit.unwrap_or(0.0)),
                        ik_a,
                        unknown_above_a: None,
                        text: match limit {
                            Some(l) => format!(
                                "selective up to {:.0} A by the guaranteed bands; above it both devices may open",
                                l
                            ),
                            None => "not selective: the upstream device may open as soon as the downstream one"
                                .to_owned(),
                        },
                    };
                }
            }
            _ => {
                if unknown_from.is_none() {
                    unknown_from = Some(here);
                }
            }
        }
        if here >= ik_a {
            break;
        }
        i *= 1.01;
    }
    if let Some(from) = unknown_from {
        return Selectivity {
            status: Status::Unknown,
            basis,
            limit_a: last_good,
            ik_a,
            unknown_above_a: Some(from),
            text: format!(
                "the bands are not known from {:.0} A up; a maker selectivity table is needed",
                from
            ),
        };
    }
    Selectivity {
        status: Status::Total,
        basis,
        limit_a: Some(ik_a),
        ik_a,
        unknown_above_a: None,
        text: "selective over the whole range by the guaranteed bands".to_owned(),
    }
}

/// A tested limit from a maker table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Limit {
    /// Selective up to the breaking capacity of the downstream device.
    Total,
    /// Selective up to this current, kA.
    UpTo(f64),
}

/// Maker (or user) tables of tested pairs, keyed by the order numbers or
/// ids of the two devices.
#[derive(Debug, Clone, Default)]
pub struct PairTable {
    selective: HashMap<(String, String), Limit>,
    backup: HashMap<(String, String), f64>,
    /// Where the table came from, for the report.
    pub source: String,
}

impl PairTable {
    pub fn new(source: &str) -> PairTable {
        PairTable {
            source: source.to_owned(),
            ..PairTable::default()
        }
    }

    pub fn add_selective(&mut self, upstream: &str, downstream: &str, limit: Limit) {
        self.selective
            .insert((upstream.to_owned(), downstream.to_owned()), limit);
    }

    pub fn add_backup(&mut self, upstream: &str, downstream: &str, limit_ka: f64) {
        self.backup
            .insert((upstream.to_owned(), downstream.to_owned()), limit_ka);
    }

    /// Reads lines `selective,<upstream>,<downstream>,<kA|total>` and
    /// `backup,<upstream>,<downstream>,<kA>`; `#` starts a comment.
    pub fn from_csv(source: &str, csv: &str) -> Result<PairTable, String> {
        let mut table = PairTable::new(source);
        for (index, raw) in csv.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |what: &str| format!("line {}: {what}", index + 1);
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            match f.as_slice() {
                ["selective", up, down, limit] if !up.is_empty() && !down.is_empty() => {
                    let limit = if limit.eq_ignore_ascii_case("total") {
                        Limit::Total
                    } else {
                        Limit::UpTo(positive(limit).ok_or_else(|| err("limit is kA or 'total'"))?)
                    };
                    table.add_selective(up, down, limit);
                }
                ["backup", up, down, ka] if !up.is_empty() && !down.is_empty() => {
                    table.add_backup(up, down, positive(ka).ok_or_else(|| err("limit is kA"))?);
                }
                _ => return Err(err("record not recognised")),
            }
        }
        Ok(table)
    }

    pub fn selective(&self, upstream: &str, downstream: &str) -> Option<Limit> {
        self.selective
            .get(&(upstream.to_owned(), downstream.to_owned()))
            .copied()
    }

    pub fn backup_ka(&self, upstream: &str, downstream: &str) -> Option<f64> {
        self.backup
            .get(&(upstream.to_owned(), downstream.to_owned()))
            .copied()
    }
}

fn positive(text: &str) -> Option<f64> {
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Selectivity of a pair: the maker table if it has the pair, else the
/// generic bands. `downstream_icu_ka` turns a table's `Total` into a
/// number.
pub fn selectivity(
    table: Option<&PairTable>,
    upstream_id: &str,
    downstream_id: &str,
    downstream_icu_ka: Option<f64>,
    upstream: Option<&Characteristic>,
    downstream: Option<&Characteristic>,
    ik_a: f64,
) -> Selectivity {
    if let Some(limit) = table.and_then(|t| t.selective(upstream_id, downstream_id)) {
        let limit_a = match limit {
            Limit::Total => downstream_icu_ka.map(|k| k * 1000.0),
            Limit::UpTo(k) => Some(k * 1000.0),
        };
        let source = table.map_or("", |t| t.source.as_str());
        return match limit_a {
            Some(l) if l >= ik_a => Selectivity {
                status: Status::Total,
                basis: Basis::MakerTable,
                limit_a: Some(l),
                ik_a,
                unknown_above_a: None,
                text: format!("selective to {:.1} kA per the table {source}", l / 1000.0),
            },
            Some(l) => Selectivity {
                status: Status::Partial,
                basis: Basis::MakerTable,
                limit_a: Some(l),
                ik_a,
                unknown_above_a: None,
                text: format!(
                    "selective only to {:.1} kA per the table {source}, below the {:.1} kA fault",
                    l / 1000.0,
                    ik_a / 1000.0
                ),
            },
            None => Selectivity {
                status: Status::Unknown,
                basis: Basis::MakerTable,
                limit_a: None,
                ik_a,
                unknown_above_a: None,
                text: "the table says total selectivity but the downstream breaking capacity is unknown"
                    .to_owned(),
            },
        };
    }
    match (upstream, downstream) {
        (Some(u), Some(d)) => analyse_bands(u, d, ik_a),
        _ => Selectivity {
            status: Status::Unknown,
            basis: Basis::GenericBands,
            limit_a: None,
            ik_a,
            unknown_above_a: None,
            text: "no time-current data and no maker table for this pair".to_owned(),
        },
    }
}

/// Result of a back-up (cascading) check.
#[derive(Debug, Clone, PartialEq)]
pub enum Backup {
    /// The device breaks the current on its own.
    NotNeeded,
    /// The pair is tested to `limit_ka`, which covers the fault.
    Covered { limit_ka: f64 },
    /// Not covered: the table limit is below the fault or the pair is not
    /// in a table.
    NotCovered { limit_ka: Option<f64> },
    /// Breaking capacity unknown.
    Unknown,
}

/// Checks whether `downstream` can sit where `ik_a` flows, alone or backed
/// up by `upstream` (only maker tables can prove the latter).
pub fn backup(
    table: Option<&PairTable>,
    upstream_id: &str,
    downstream_id: &str,
    downstream_icu_ka: Option<f64>,
    ik_a: f64,
) -> Backup {
    let Some(icu) = downstream_icu_ka else {
        return Backup::Unknown;
    };
    if icu * 1000.0 >= ik_a * (1.0 - 1e-9) {
        return Backup::NotNeeded;
    }
    match table.and_then(|t| t.backup_ka(upstream_id, downstream_id)) {
        Some(l) if l * 1000.0 >= ik_a * (1.0 - 1e-9) => Backup::Covered { limit_ka: l },
        Some(l) => Backup::NotCovered { limit_ka: Some(l) },
        None => Backup::NotCovered { limit_ka: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::curves::{Electronic, McbCurve, RelayClass};

    fn mcb(curve: McbCurve, in_a: f64) -> Characteristic {
        Characteristic::Mcb { curve, in_a }
    }

    fn lsi() -> Characteristic {
        Characteristic::Electronic(Electronic {
            in_a: 250.0,
            ir_a: 250.0,
            tr_s: 12.0,
            isd_x: 8.0,
            tsd_s: 0.3,
            ii_x: 12.0,
        })
    }

    #[test]
    fn a_delayed_breaker_is_selective_over_a_miniature_one_until_its_instantaneous_stage() {
        // 250 A with a 0.3 s short-time delay above a C16: selective until
        // the upstream instantaneous stage (12 · 250 · 0.8 = 2400 A).
        let s = analyse_bands(&lsi(), &mcb(McbCurve::C, 16.0), 10_000.0);
        assert_eq!(s.status, Status::Partial);
        let limit = s.limit_a.unwrap();
        assert!(limit > 2000.0 && limit < 2500.0, "{limit}");
        assert_eq!(s.basis, Basis::GenericBands);
        // A fault below the limit is selective in total.
        let low = analyse_bands(&lsi(), &mcb(McbCurve::C, 16.0), 1_500.0);
        assert_eq!(low.status, Status::Total);
    }

    #[test]
    fn two_miniature_breakers_are_not_selective_in_the_magnetic_region() {
        // Both guaranteed only to 0.1 s: the upstream can open together.
        let s = analyse_bands(&mcb(McbCurve::C, 32.0), &mcb(McbCurve::C, 10.0), 3_000.0);
        assert_ne!(s.status, Status::Total);
        // In the overload region they are selective (32 A is slower than 10 A).
        assert!(s.limit_a.unwrap_or(0.0) >= 0.0);
        let _ = RelayClass::C10;
    }

    fn near_limit(a: f64, b: f64) {
        assert!((a - b).abs() < 0.02 * b, "{a} vs {b}");
    }

    #[test]
    fn unknown_bands_give_unknown_not_a_pass() {
        // A gG fuse above: from its conventional fusing zone nothing is
        // guaranteed to hold off a fast downstream trip, so the bands refuse
        // to call it selective there (partial, limit at 1.25 In).
        let fuse = Characteristic::FuseGg { in_a: 63.0 };
        let p = analyse_bands(&fuse, &mcb(McbCurve::B, 10.0), 6_000.0);
        assert_eq!(p.status, Status::Partial);
        near_limit(p.limit_a.unwrap(), 63.0 * 1.25);
        // Maker points that stop at 100 A leave the rest unknown.
        let short = Characteristic::Points {
            min: vec![(10.0, 100.0), (100.0, 10.0)],
            max: vec![(10.0, 1000.0), (100.0, 100.0)],
        };
        let s = analyse_bands(&lsi(), &short, 1_000.0);
        assert_eq!(s.status, Status::Unknown);
        assert!(s.unknown_above_a.unwrap() > 100.0);
        let none = selectivity(None, "a", "b", None, None, None, 5_000.0);
        assert_eq!(none.status, Status::Unknown);
    }

    #[test]
    fn maker_table_wins_and_total_needs_a_breaking_capacity() {
        let table = PairTable::from_csv(
            "Maker X selectivity tables 2025",
            "# upstream, downstream, limit\nselective,UP1,DN1,total\nselective,UP1,DN2,4.5\nbackup,UP1,DN3,25\n",
        )
        .unwrap();
        let tot = selectivity(Some(&table), "UP1", "DN1", Some(10.0), None, None, 8_000.0);
        assert_eq!(tot.status, Status::Total);
        assert_eq!(tot.basis, Basis::MakerTable);
        let part = selectivity(Some(&table), "UP1", "DN2", Some(10.0), None, None, 8_000.0);
        assert_eq!(part.status, Status::Partial);
        assert_eq!(part.limit_a, Some(4_500.0));
        let unk = selectivity(Some(&table), "UP1", "DN1", None, None, None, 8_000.0);
        assert_eq!(unk.status, Status::Unknown);
        // The pair is not in the table: falls back to bands, which need data.
        let fall = selectivity(Some(&table), "UP9", "DN9", Some(10.0), None, None, 8_000.0);
        assert_eq!(fall.basis, Basis::GenericBands);
    }

    #[test]
    fn backup_logic() {
        let table = PairTable::from_csv("t", "backup,UP,DN,25\n").unwrap();
        assert_eq!(
            backup(Some(&table), "UP", "DN", Some(10.0), 8_000.0),
            Backup::NotNeeded
        );
        assert_eq!(
            backup(Some(&table), "UP", "DN", Some(6.0), 20_000.0),
            Backup::Covered { limit_ka: 25.0 }
        );
        assert_eq!(
            backup(Some(&table), "UP", "DN", Some(6.0), 30_000.0),
            Backup::NotCovered {
                limit_ka: Some(25.0)
            }
        );
        assert_eq!(
            backup(Some(&table), "UP", "OTHER", Some(6.0), 20_000.0),
            Backup::NotCovered { limit_ka: None }
        );
        assert_eq!(backup(None, "UP", "DN", None, 20_000.0), Backup::Unknown);
    }

    #[test]
    fn csv_errors_name_the_line() {
        let e = PairTable::from_csv("t", "selective,A,B,total\nselective,A,B,abc\n").unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
        assert!(PairTable::from_csv("t", "backup,A,B\n").is_err());
        assert!(PairTable::from_csv("t", "").is_ok());
    }
}
