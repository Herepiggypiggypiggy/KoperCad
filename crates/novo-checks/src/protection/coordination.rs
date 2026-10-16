//! Coordination of a chain of protective devices from the supply towards
//! the load: breaking capacity, back-up, selectivity between neighbours,
//! disconnection time and protection of each circuit's conductor against
//! short circuit.
//!
//! The chain is plain data: one [`Node`] per device, the most upstream
//! first, each with the fault currents at its terminals. The currents come
//! from the calculation library (`calc::short_circuit`), the device from
//! the part catalog or a maker's data, the conductor from the cable. The
//! output is a list of [`Finding`]s, each with a stable [`Rule`] key so the
//! design checks can turn them into problems and waivers can address them.
//!
//! Formulas follow the usual reading of IEC 60364-4-43 (protection against
//! overcurrent; the energy a conductor withstands, `k²S²`, against the
//! energy the device lets through) and IEC 60364-4-41 (disconnection time).
//! The adiabatic formula holds up to about five seconds, so the conductor
//! check scans the currents where the device clears in that time or less.

use serde::{Deserialize, Serialize};

use super::curves::{Family, TripCurve};
use super::devices::{Device, MakerTables};
use super::selectivity::{self, Backup, Selectivity, Settings, Verdict};
use crate::Severity;

/// A conductor protected by the device: cross-section and the material
/// constant `k` (115 for PVC copper, 143 for XLPE copper, 76 and 94 for
/// aluminium).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Conductor {
    /// Cross-section in mm².
    pub section_mm2: f64,
    /// Material and insulation constant `k` in A·√s/mm².
    pub k: f64,
}

impl Conductor {
    /// The energy the conductor withstands, A²s: `k²S²`.
    pub fn withstand_i2t(&self) -> f64 {
        self.k * self.k * self.section_mm2 * self.section_mm2
    }
}

/// One device in the chain with the fault currents at its place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// What to call it in a message: its tag (`-Q1`) or the circuit.
    pub label: String,
    /// The device.
    pub device: Device,
    /// Highest prospective short-circuit current at the device's
    /// terminals, amperes rms.
    pub ik_max_a: f64,
    /// Smallest fault current at the far end of the circuit the device
    /// protects (earth fault or phase to phase, whichever is smaller),
    /// amperes rms.
    pub ik_min_a: Option<f64>,
    /// The conductor on the load side, if it is to be checked.
    pub conductor: Option<Conductor>,
    /// Longest disconnection time allowed for a fault at the far end,
    /// seconds (0.4 for final circuits to 32 A on 230 V TN, 5 for
    /// distribution circuits).
    pub max_disconnect_s: Option<f64>,
}

/// The calculation a finding came from. The keys are stable: waivers are
/// stored under them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rule {
    /// Breaking capacity below the prospective current (and not covered by
    /// back-up protection).
    BreakingCapacity,
    /// Back-up protection relied on or not proven.
    Backup,
    /// Two neighbouring devices that are not selective.
    Selectivity,
    /// The device may not clear the smallest fault in time.
    Disconnection,
    /// The conductor cannot take the energy the device lets through.
    ConductorWithstand,
}

impl Rule {
    /// Every rule.
    pub const ALL: [Rule; 5] = [
        Rule::BreakingCapacity,
        Rule::Backup,
        Rule::Selectivity,
        Rule::Disconnection,
        Rule::ConductorWithstand,
    ];

    /// The stable key, `protection.<snake_case>`.
    pub fn key(self) -> &'static str {
        match self {
            Rule::BreakingCapacity => "protection.breaking_capacity",
            Rule::Backup => "protection.backup",
            Rule::Selectivity => "protection.selectivity",
            Rule::Disconnection => "protection.disconnection",
            Rule::ConductorWithstand => "protection.conductor_withstand",
        }
    }
}

/// One finding of the chain analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// What was checked.
    pub rule: Rule,
    /// How bad.
    pub severity: Severity,
    /// Index of the node the finding is about (for selectivity and
    /// back-up: the downstream one).
    pub node: usize,
    /// A message in plain language with the numbers.
    pub message: String,
}

/// The result for a neighbouring pair.
#[derive(Debug, Clone, PartialEq)]
pub struct PairResult {
    /// Index of the upstream node; the downstream one is the next.
    pub upstream: usize,
    /// The selectivity analysis.
    pub selectivity: Selectivity,
    /// Back-up protection, when the downstream breaking capacity is below
    /// the prospective current.
    pub backup: Backup,
}

/// The whole result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    /// Findings in chain order.
    pub findings: Vec<Finding>,
    /// One result per neighbouring pair.
    pub pairs: Vec<PairResult>,
}

/// The adiabatic formula is valid for clearing times up to this, seconds.
const ADIABATIC_LIMIT_S: f64 = 5.0;
/// Below this clearing time the device's let-through energy replaces
/// `I²·t` of the band, seconds.
const LET_THROUGH_BELOW_S: f64 = 0.1;

/// Analyses a chain, most upstream node first.
pub fn analyse_chain(nodes: &[Node], tables: &MakerTables, settings: &Settings) -> Report {
    let mut report = Report::default();
    for (k, node) in nodes.iter().enumerate() {
        let up = k.checked_sub(1).map(|u| &nodes[u]);
        breaking_capacity(&mut report, k, node, up, tables);
        disconnection(&mut report, k, node);
        conductor(&mut report, k, node);
        if let Some(up) = up {
            pair(&mut report, k - 1, up, node, tables, settings);
        }
    }
    report
}

fn breaking_capacity(
    report: &mut Report,
    k: usize,
    node: &Node,
    up: Option<&Node>,
    tables: &MakerTables,
) {
    let d = &node.device;
    if !(node.ik_max_a.is_finite() && node.ik_max_a > d.icu_a) {
        return;
    }
    let over = format!(
        "{} breaks {} kA but the prospective short-circuit current at it is {} kA",
        node.label,
        kilo(d.icu_a),
        kilo(node.ik_max_a)
    );
    match up.map(|u| selectivity::backup(&u.device, d, node.ik_max_a, tables)) {
        Some(Backup::Covered(basis)) => report.findings.push(Finding {
            rule: Rule::Backup,
            severity: Severity::Info,
            node: k,
            message: format!(
                "{over}; covered by back-up protection of {} ({}). Both devices will trip above {} kA.",
                up.map_or("", |u| u.label.as_str()),
                match basis {
                    selectivity::BackupBasis::MakerTable => "maker's table",
                    selectivity::BackupBasis::LetThrough => "let-through comparison",
                },
                kilo(d.icu_a)
            ),
        }),
        Some(Backup::NotCovered(why)) => report.findings.push(Finding {
            rule: Rule::BreakingCapacity,
            severity: Severity::Error,
            node: k,
            message: format!("{over}; back-up protection is not proven: {why}"),
        }),
        _ => report.findings.push(Finding {
            rule: Rule::BreakingCapacity,
            severity: Severity::Error,
            node: k,
            message: format!("{over}; there is no device upstream to give back-up protection"),
        }),
    }
}

fn disconnection(report: &mut Report, k: usize, node: &Node) {
    let (Some(ik), Some(limit)) = (node.ik_min_a, node.max_disconnect_s) else {
        return;
    };
    if !(ik.is_finite() && ik > 0.0 && limit.is_finite() && limit > 0.0) {
        return;
    }
    let curve = &node.device.curve;
    let (clears, detail) = match curve.trip_time_range(ik) {
        None => (false, "does not trip at all".to_string()),
        Some((_, hi)) if hi.is_infinite() => (false, "may not trip".to_string()),
        Some((_, hi)) => (hi <= limit, format!("clears within {} s", num(hi))),
    };
    if clears {
        return;
    }
    let needs = curve
        .certain_current_for(limit)
        .map(|a| format!(" It needs {} A to clear within {} s.", num(a), num(limit)))
        .unwrap_or_default();
    report.findings.push(Finding {
        rule: Rule::Disconnection,
        severity: Severity::Error,
        node: k,
        message: format!(
            "the smallest fault current at the far end of {} is {} A, where the device {} (at most {} s allowed).{needs}",
            node.label,
            num(ik),
            detail,
            num(limit)
        ),
    });
}

/// Energy the device lets through at `i_a`, A²s, or `None` when it cannot
/// be stated (a breaker's instantaneous region without let-through data).
fn energy_at(d: &Device, i_a: f64) -> Option<f64> {
    let (_, hi) = d.curve.trip_time_range(i_a)?;
    if hi.is_finite() && hi >= LET_THROUGH_BELOW_S {
        return Some(i_a * i_a * hi);
    }
    if d.let_through.as_ref().is_some_and(|t| t.at(i_a).is_some()) {
        return Some(d.let_through_at(i_a).1);
    }
    if d.curve.family == Family::Fuse && hi.is_finite() {
        // The fuse's band follows its total I²t.
        return Some(i_a * i_a * hi);
    }
    None
}

fn conductor(report: &mut Report, k: usize, node: &Node) {
    let Some(c) = node.conductor else { return };
    if !(c.section_mm2.is_finite() && c.section_mm2 > 0.0 && c.k.is_finite() && c.k > 0.0) {
        return;
    }
    let d = &node.device;
    let allowed = c.withstand_i2t();
    // Smallest fault: the conductor must not sit in the fault longer than
    // the adiabatic limit.
    if let Some(ik_min) = node.ik_min_a
        && let Some((_, hi)) = d.curve.trip_time_range(ik_min)
        && hi > ADIABATIC_LIMIT_S
    {
        report.findings.push(Finding {
            rule: Rule::ConductorWithstand,
            severity: Severity::Error,
            node: k,
            message: format!(
                "at the smallest fault current {} A the device may take {} s to clear, more than the {} s the conductor check is valid for; the {} mm² conductor is not shown to be protected",
                num(ik_min),
                if hi.is_finite() { num(hi) } else { "unbounded".into() },
                num(ADIABATIC_LIMIT_S),
                num(c.section_mm2)
            ),
        });
        return;
    }
    let start = d
        .curve
        .certain_current_for(ADIABATIC_LIMIT_S)
        .unwrap_or(d.lowest_trip_a())
        .max(node.ik_min_a.unwrap_or(0.0));
    let end = node.ik_max_a;
    if !(start.is_finite() && end.is_finite() && end >= start && start > 0.0) {
        return;
    }
    // Scan, log spaced, plus the ends.
    let n = 400;
    let (l0, l1) = (start.ln(), end.ln());
    let mut worst: Option<(f64, f64)> = None; // (margin ratio, current)
    let mut unchecked_from: Option<f64> = None;
    for j in 0..=n {
        let i = (l0 + (l1 - l0) * j as f64 / n as f64).exp();
        match energy_at(d, i) {
            Some(e) => {
                let ratio = e / allowed;
                if worst.is_none_or(|(r, _)| ratio > r) {
                    worst = Some((ratio, i));
                }
            }
            None => {
                unchecked_from.get_or_insert(i);
            }
        }
    }
    if let Some((ratio, i)) = worst
        && ratio > 1.0
    {
        report.findings.push(Finding {
            rule: Rule::ConductorWithstand,
            severity: Severity::Error,
            node: k,
            message: format!(
                "at {} A the device lets through up to {} A²s but the {} mm² conductor withstands {} A²s (k = {})",
                num(i),
                num(ratio * allowed),
                num(c.section_mm2),
                num(allowed),
                num(c.k)
            ),
        });
    } else if let Some(i) = unchecked_from {
        report.findings.push(Finding {
            rule: Rule::ConductorWithstand,
            severity: Severity::Info,
            node: k,
            message: format!(
                "the conductor is shown to withstand the device's curve up to {} A; above that the device clears in under {} s and its let-through energy is needed (add the maker's let-through table)",
                num(i),
                num(LET_THROUGH_BELOW_S)
            ),
        });
    }
}

fn pair(
    report: &mut Report,
    up_index: usize,
    up: &Node,
    down: &Node,
    tables: &MakerTables,
    settings: &Settings,
) {
    let ik = down.ik_max_a;
    let sel = selectivity::analyse(&up.device, &down.device, ik, tables, settings);
    let bk = selectivity::backup(&up.device, &down.device, ik, tables);
    let name = |a: &str, b: &str| format!("{a} behind {b}");
    match sel.verdict {
        Verdict::Total => {}
        Verdict::Partial => report.findings.push(Finding {
            rule: Rule::Selectivity,
            severity: Severity::Warning,
            node: up_index + 1,
            message: format!(
                "{} is selective up to {} A only; above that both may trip (prospective current {} A)",
                name(&down.label, &up.label),
                num(sel.limit_a),
                num(ik)
            ),
        }),
        Verdict::None => report.findings.push(Finding {
            rule: Rule::Selectivity,
            severity: Severity::Warning,
            node: up_index + 1,
            message: format!(
                "{} is not selective at any fault current: the trip bands overlap from {} A",
                name(&down.label, &up.label),
                num(sel.from_a)
            ),
        }),
    }
    report.pairs.push(PairResult {
        upstream: up_index,
        selectivity: sel,
        backup: bk,
    });
}

/// Checks that a device does not trip while a motor starts: the fastest
/// edge of the band at the starting current must be longer than the
/// starting time. Returns the fastest trip time at the starting current
/// when the device may trip during the start.
pub fn motor_start_trips(curve: &TripCurve, start_a: f64, start_s: f64) -> Option<f64> {
    let (lo, _) = curve.trip_time_range(start_a)?;
    if lo <= start_s { Some(lo) } else { None }
}

fn kilo(a: f64) -> String {
    num(a / 1000.0)
}

fn num(x: f64) -> String {
    let a = x.abs();
    let text = if a == 0.0 {
        return "0".into();
    } else if a >= 100.0 {
        format!("{x:.0}")
    } else if a >= 10.0 {
        format!("{x:.1}")
    } else if a >= 0.1 {
        format!("{x:.2}")
    } else {
        format!("{x:.3}")
    };
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::super::curves::{McbCurve, RelayClass};
    use super::super::devices::{LetThrough, Limit, PairRow};
    use super::*;

    const PVC: f64 = 115.0;

    fn node(label: &str, d: Device, ik_max: f64) -> Node {
        Node {
            label: label.into(),
            device: d,
            ik_max_a: ik_max,
            ik_min_a: None,
            conductor: None,
            max_disconnect_s: None,
        }
    }

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

    fn run(nodes: &[Node]) -> Report {
        analyse_chain(nodes, &MakerTables::default(), &Settings::default())
    }

    /// Worked example: a 1.5 mm² PVC conductor withstands 115² × 1.5² =
    /// 29 756 A²s. A 63 A gG fuse (total I²t about 55 kA²s) does not
    /// protect it at 10 kA; a 10 A fuse (about 1.4 kA²s... ) does.
    #[test]
    fn conductor_energy_against_a_fuse() {
        let c = Conductor {
            section_mm2: 1.5,
            k: PVC,
        };
        assert!((c.withstand_i2t() - 29_756.25).abs() < 0.01);
        let mut big = node("-F1", fuse(63.0), 10_000.0);
        big.conductor = Some(c);
        let r = run(&[big]);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == Rule::ConductorWithstand)
            .expect("63 A fuse must not protect 1.5 mm²");
        assert_eq!(f.severity, Severity::Error);
        assert!(
            f.message.contains("29757") || f.message.contains("29756"),
            "{}",
            f.message
        );
        let mut small = node("-F2", fuse(10.0), 10_000.0);
        small.conductor = Some(c);
        assert!(run(&[small]).findings.is_empty());
    }

    /// A breaker without let-through data: the curve part is checked, the
    /// instantaneous part is reported as unchecked, never as a failure.
    #[test]
    fn breaker_without_let_through_data_is_not_a_false_alarm() {
        let mut n = node("-Q1", mcb(16.0, McbCurve::C, 10.0), 6000.0);
        n.conductor = Some(Conductor {
            section_mm2: 2.5,
            k: PVC,
        });
        let r = run(std::slice::from_ref(&n));
        assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
        assert_eq!(r.findings[0].severity, Severity::Info);
        // With a let-through table the check is made, and passes.
        n.device.let_through =
            Some(LetThrough::new(vec![(100.0, 150.0, 1.0e3), (10_000.0, 2000.0, 2.5e4)]).unwrap());
        let r = run(std::slice::from_ref(&n));
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        // And fails for a thinner conductor than its energy.
        n.conductor = Some(Conductor {
            section_mm2: 1.0,
            k: PVC,
        });
        n.device.let_through =
            Some(LetThrough::new(vec![(100.0, 150.0, 1.0e3), (10_000.0, 2000.0, 2.5e4)]).unwrap());
        // 115² = 13 225 A²s; the table lets 25 kA²s through at 10 kA.
        n.ik_max_a = 10_000.0;
        let r = run(std::slice::from_ref(&n));
        assert_eq!(r.findings[0].rule, Rule::ConductorWithstand);
        assert_eq!(r.findings[0].severity, Severity::Error);
    }

    /// Worked example: a 6 kA breaker at a point with 10 kA prospective
    /// current is an error, unless the upstream device gives back-up.
    #[test]
    fn breaking_capacity_and_backup() {
        let up = node("-Q0", mcb(63.0, McbCurve::C, 25.0), 10_000.0);
        let down = node("-Q1", mcb(16.0, McbCurve::C, 6.0), 10_000.0);
        let r = run(&[up.clone(), down.clone()]);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == Rule::BreakingCapacity)
            .unwrap();
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.node, 1);
        assert!(
            f.message.contains("6 kA") && f.message.contains("10 kA"),
            "{}",
            f.message
        );
        // With the maker's back-up table.
        let tables = MakerTables {
            rows: vec![PairRow {
                upstream: up.device.name.clone(),
                downstream: down.device.name.clone(),
                selectivity: None,
                backup: Some(Limit::Amps(25_000.0)),
            }],
        };
        let r = analyse_chain(&[up, down.clone()], &tables, &Settings::default());
        assert!(r.findings.iter().all(|f| f.rule != Rule::BreakingCapacity));
        let f = r.findings.iter().find(|f| f.rule == Rule::Backup).unwrap();
        assert_eq!(f.severity, Severity::Info);
        // The first device of a chain has nothing upstream.
        let r = run(&[down]);
        assert!(r.findings[0].message.contains("no device upstream"));
    }

    /// Worked example: disconnection time. A C16 (certain at 160 A within
    /// 0.1 s) clears an end-of-line fault of 200 A in time for 0.4 s; a
    /// fault of only 60 A is not cleared in 0.4 s (the band allows up to a
    /// minute at 2.55 In, 41 A).
    #[test]
    fn disconnection_time() {
        let mut n = node("-Q1", mcb(16.0, McbCurve::C, 10.0), 6000.0);
        n.max_disconnect_s = Some(0.4);
        n.ik_min_a = Some(200.0);
        assert!(
            run(std::slice::from_ref(&n))
                .findings
                .iter()
                .all(|f| f.rule != Rule::Disconnection)
        );
        n.ik_min_a = Some(100.0);
        let r = run(std::slice::from_ref(&n));
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == Rule::Disconnection)
            .unwrap();
        assert_eq!(f.severity, Severity::Error);
        // It tells what current is needed (160 A for 0.1 s; for 0.4 s the
        // curve gives a lower figure).
        assert!(f.message.contains("It needs"), "{}", f.message);
        n.ik_min_a = Some(15.0);
        let r = run(std::slice::from_ref(&n));
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Disconnection && f.message.contains("does not trip"))
        );
    }

    /// A conductor is not protected when the smallest fault may take
    /// longer than the adiabatic limit to clear.
    #[test]
    fn slow_clearing_at_the_smallest_fault_leaves_the_conductor_unprotected() {
        let mut n = node("-F1", fuse(100.0), 10_000.0);
        n.ik_min_a = Some(250.0);
        n.conductor = Some(Conductor {
            section_mm2: 25.0,
            k: PVC,
        });
        let r = run(std::slice::from_ref(&n));
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == Rule::ConductorWithstand)
            .unwrap();
        assert!(
            f.message.contains("smallest fault current"),
            "{}",
            f.message
        );
    }

    #[test]
    fn selectivity_findings_per_pair() {
        let main = node("-Q0", mcb(32.0, McbCurve::C, 10.0), 6000.0);
        let sub = node("-Q1", mcb(10.0, McbCurve::C, 10.0), 6000.0);
        let r = run(&[main, sub]);
        assert_eq!(r.pairs.len(), 1);
        assert_eq!(r.pairs[0].selectivity.verdict, Verdict::Partial);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == Rule::Selectivity)
            .unwrap();
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.message.contains("-Q1 behind -Q0"), "{}", f.message);
        // gG 63 behind nothing, gG 25 behind it: clean.
        let r = run(&[
            node("-F1", fuse(63.0), 20_000.0),
            node("-F2", fuse(25.0), 20_000.0),
        ]);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }

    #[test]
    fn motor_start_against_breaker_and_relay() {
        // 11 kW, 22 A motor, starts at 7 x In = 154 A for 5 s.
        let mpcb = TripCurve::mpcb(22.0, RelayClass::C10, 13.0).unwrap();
        // Class 10 relay may trip in 4 s at 7.2 Ir (158 A): a 5 s start
        // can trip it.
        assert!(motor_start_trips(&mpcb, 154.0, 5.0).is_some());
        let class30 = TripCurve::overload_relay(22.0, RelayClass::C30).unwrap();
        // Class 30: at least 9 s at 7.2 Ir, so a 5 s start passes.
        assert_eq!(motor_start_trips(&class30, 154.0, 5.0), None);
        // A C32 breaker feeding a motor starting at 154 A: magnetic
        // pickup of C is 160 A (5 x 32): may trip instantly.
        let c32 = TripCurve::mcb(32.0, McbCurve::C).unwrap();
        assert!(motor_start_trips(&c32, 170.0, 0.1).is_some());
        assert_eq!(motor_start_trips(&c32, 100.0, 0.3), None);
    }

    #[test]
    fn rule_keys_are_stable() {
        let keys: Vec<_> = Rule::ALL.iter().map(|r| r.key()).collect();
        assert_eq!(
            keys,
            [
                "protection.breaking_capacity",
                "protection.backup",
                "protection.selectivity",
                "protection.disconnection",
                "protection.conductor_withstand"
            ]
        );
    }
}
