//! One circuit, judged completely: a protective device feeding a cable
//! feeding a load, at a point of a [`Network`] whose fault currents are known.
//!
//! [`evaluate`] returns a [`CircuitReport`]: one [`Row`] per question a
//! designer must answer, each *Pass*, *Fail* or *Unknown* with the numbers
//! and the sentence, plus the list of assumptions the numbers rest on and
//! the table version they were read from. Unknown is a first-class answer:
//! a missing breaking capacity or protective conductor is never defaulted
//! into a pass (or a fail).
//!
//! | Row | Question | Standard |
//! | --- | --- | --- |
//! | Overload | `Ib ≤ In ≤ Iz` and `I2 ≤ 1.45·Iz` | IEC 60364-4-43 §433.1 |
//! | Voltage drop | within the limit at the load current | IEC 60364-5-52 Annex G |
//! | Breaking capacity | `Icu ≥ Ik` at the device | IEC 60364-4-43 §434.5.1 |
//! | Short-circuit, maximum | let-through `I²t ≤ k²S²` | §434.5.2 |
//! | Short-circuit, minimum | the smallest fault at the cable end clears before the conductor overheats | §434.5.2 |
//! | Disconnection | earth fault at the cable end clears within the time for the voltage | IEC 60364-4-41 §411.4.4 |
//! | Protective conductor | PE section against the adiabatic equation | IEC 60364-5-54 §543.1.2 |

use super::cable::{Circuit, Loaded, Method};
use super::curves::Characteristic;
use super::drop::{Run, Supply};
use super::network::{Branch, FaultLevels, Network, NodeId};
use super::pe::{max_disconnection_time, min_pe_adiabatic, min_pe_section};
use super::short_circuit::Trip;
use super::tables::TableSet;
use super::{Insulation, Rule, num, within};

/// The protective device at the start of the circuit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Device {
    /// Order number or part id, the key into maker selectivity tables.
    pub id: Option<String>,
    pub rated_a: f64,
    /// Time-current characteristic, when known.
    pub characteristic: Option<Characteristic>,
    /// Instantaneous trip as a multiple of the rating, when no
    /// characteristic is given (see [`Trip`]).
    pub trip: Option<Trip>,
    /// Breaking capacity Icu in kA.
    pub icu_ka: Option<f64>,
    /// Let-through energy A²s at the prospective current at its terminals.
    pub let_through_i2t: Option<f64>,
}

/// The cable from the device to the load.
#[derive(Debug, Clone, PartialEq)]
pub struct CableData {
    pub section_mm2: f64,
    pub insulation: Insulation,
    pub method: Method,
    pub ambient_c: f64,
    /// Circuits bunched together, this one included.
    pub grouped_circuits: u32,
    pub length_m: f64,
    /// Identical cables in parallel per phase.
    pub parallel: u32,
    pub pe_mm2: Option<f64>,
}

/// Everything about the circuit.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitDesign {
    pub name: String,
    /// Design current `Ib` of the load, A.
    pub load_current_a: f64,
    pub power_factor: f64,
    pub supply: Supply,
    /// Nominal voltage between the conductors the drop is counted on.
    pub voltage: f64,
    pub device: Device,
    /// Setting of a separate overload device (relay) when the short-circuit
    /// device does not protect against overload, as in a motor starter. The
    /// overload row then uses this instead of the device's rating.
    pub overload_setting_a: Option<f64>,
    pub cable: CableData,
    /// Drop limit, percent.
    pub drop_limit_percent: f64,
    /// Assumptions made while building the design (a defaulted power
    /// factor, a drop limit nobody stated), carried into the report.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// Not a breach of a rule, but something the designer should know
    /// (partial selectivity). Does not count as a failure.
    Warning,
    Unknown,
}

/// One answer of the report.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub rule: Rule,
    /// Short name of the question, `Overload`, `Voltage drop`, ...
    pub title: &'static str,
    pub status: Status,
    /// The calculated value and the limit it is held against, in `unit`.
    pub value: Option<f64>,
    pub limit: Option<f64>,
    pub unit: &'static str,
    /// One plain sentence with the numbers in it.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CircuitReport {
    pub name: String,
    pub rows: Vec<Row>,
    /// Fault currents where the device sits and at the load end.
    pub at_device: Option<FaultLevels>,
    pub at_load: Option<FaultLevels>,
    /// What the numbers rest on, one line each.
    pub assumptions: Vec<String>,
    /// The table version the cable rating was read from.
    pub tables: String,
    /// The cable's rating `Iz`, A, when it could be read.
    pub iz_a: Option<f64>,
}

impl CircuitReport {
    pub fn failures(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|r| r.status == Status::Fail)
    }

    /// Whether nothing fails and nothing is unknown.
    pub fn all_pass(&self) -> bool {
        self.rows.iter().all(|r| r.status == Status::Pass)
    }

    pub fn has_failure(&self) -> bool {
        self.failures().next().is_some()
    }

    /// The report as plain text: one line per row, then the assumptions and
    /// the table disclosure. ASCII status tags so it reads in any console.
    pub fn to_text(&self) -> String {
        let mut out = format!("Circuit {}\n", self.name);
        for r in &self.rows {
            let tag = match r.status {
                Status::Pass => "PASS   ",
                Status::Fail => "FAIL   ",
                Status::Warning => "WARN   ",
                Status::Unknown => "UNKNOWN",
            };
            out.push_str(&format!("  {tag} {}: {}\n", r.title, r.text));
        }
        if !self.assumptions.is_empty() {
            out.push_str("  Assumed:\n");
            for a in &self.assumptions {
                out.push_str(&format!("    - {a}\n"));
            }
        }
        out.push_str(&format!("  {}\n", self.tables));
        out
    }
}

fn row(
    rule: Rule,
    title: &'static str,
    status: Status,
    value: Option<f64>,
    limit: Option<f64>,
    unit: &'static str,
    text: String,
) -> Row {
    Row {
        rule,
        title,
        status,
        value,
        limit,
        unit,
        text,
    }
}

fn unknown(rule: Rule, title: &'static str, text: &str) -> Row {
    row(
        rule,
        title,
        Status::Unknown,
        None,
        None,
        "",
        text.to_owned(),
    )
}

/// `I2`, the current that surely trips the device conventionally, as a
/// multiple of its rating.
fn i2_factor(device: &Device) -> f64 {
    match device.characteristic {
        // IEC 60269-1 conventional fusing current by rating.
        Some(Characteristic::FuseGg { in_a }) if in_a < 4.0 + 1e-9 => 2.1,
        Some(Characteristic::FuseGg { in_a }) if in_a < 16.0 => 1.9,
        Some(Characteristic::FuseGg { .. }) => 1.6,
        _ => 1.45,
    }
}

/// Time the device takes to clear `ik`, worst case, if it can be known.
fn clearing_time(device: &Device, ik: f64) -> Option<f64> {
    if let Some(t) = device
        .characteristic
        .as_ref()
        .and_then(|c| c.max_clearing_time(ik))
        .filter(|t| t.is_finite())
    {
        return Some(t);
    }
    // No curve: a stated instantaneous trip clears within 0.1 s.
    let trip = device.trip?;
    (ik >= trip.factor() * device.rated_a).then_some(0.1)
}

/// Judges the circuit. `at` is the network node the device is connected to.
pub fn evaluate(
    network: &Network,
    at: NodeId,
    design: &CircuitDesign,
    tables: &TableSet,
) -> CircuitReport {
    let cable = &design.cable;
    let loaded = match design.supply {
        Supply::ThreePhase => Loaded::Three,
        Supply::TwoWire => Loaded::Two,
    };
    let mut assumptions = network.source().assumptions.clone();
    assumptions.extend(design.notes.iter().cloned());
    if let Some(c) = &design.device.characteristic {
        assumptions.extend(c.assumptions());
    }
    assumptions.push(format!(
        "conductor resistance taken at {} °C for minimum currents and 20 °C for maximum currents",
        cable.insulation.max_temperature()
    ));
    assumptions.push("cable reactance 0.08 mΩ/m; protective conductor in the same run".to_owned());

    let mut rows = Vec::new();

    // The network seen at the load end.
    let mut net = network.clone();
    let branch = Branch::cable(
        cable.section_mm2,
        cable.length_m,
        cable.parallel.max(1),
        cable.insulation,
        cable.pe_mm2,
    );
    let at_device = network.fault(at);
    let at_load = branch.and_then(|b| {
        let end = net.add_node(at, &design.name, b);
        net.fault(end)
    });

    // 1. Overload.
    let circuit = Circuit {
        design_current: design.load_current_a,
        device_rating: design.device.rated_a,
        section_mm2: cable.section_mm2,
        insulation: cable.insulation,
        method: cable.method,
        loaded,
        ambient_c: cable.ambient_c,
        grouped_circuits: cable.grouped_circuits,
    };
    let iz = circuit
        .rating_with(tables)
        .map(|z| z * f64::from(cable.parallel.max(1)));
    match iz {
        None => rows.push(unknown(
            Rule::CableRating,
            "Overload",
            &format!(
                "no table value for {} mm², {:?}, method {:?}, {:?} loaded conductors: load your licensed table or choose a listed combination",
                num(cable.section_mm2), cable.insulation, cable.method, loaded
            ),
        )),
        Some(iz) => {
            let separate = design.overload_setting_a;
            let (ib, inn) = (
                design.load_current_a,
                separate.unwrap_or(design.device.rated_a),
            );
            // A relay trips for sure at 1.2 Ir (IEC 60947-4-1).
            let i2 = if separate.is_some() {
                1.2 * inn
            } else {
                i2_factor(&design.device) * inn
            };
            let fail_a = !within(ib, inn);
            let fail_b = !within(inn, iz);
            let fail_c = !within(i2, 1.45 * iz);
            let status = if fail_a || fail_b || fail_c {
                Status::Fail
            } else {
                Status::Pass
            };
            let text = if fail_a {
                format!("the load draws {} A but the device is rated {} A", num(ib), num(inn))
            } else if fail_b {
                format!(
                    "the cable carries {} A here (Iz, planning value) but the device is rated {} A: the cable is not protected",
                    num(iz), num(inn)
                )
            } else if fail_c {
                format!(
                    "the device's conventional tripping current {} A exceeds 1.45 × Iz = {} A",
                    num(i2), num(1.45 * iz)
                )
            } else {
                format!(
                    "Ib {} A ≤ In {} A ≤ Iz {} A, and the tripping current {} A is within 1.45 × Iz",
                    num(ib), num(inn), num(iz), num(i2)
                )
            };
            rows.push(row(Rule::CableRating, "Overload", status, Some(inn), Some(iz), "A", text));
        }
    }

    // 2. Voltage drop.
    let run = Run {
        length_m: cable.length_m,
        section_mm2: cable.section_mm2 * f64::from(cable.parallel.max(1)),
        current: design.load_current_a,
        power_factor: design.power_factor,
        supply: design.supply,
        voltage: design.voltage,
        temperature_c: cable.insulation.max_temperature(),
    };
    match run.drop_percent() {
        None => rows.push(unknown(
            Rule::VoltageDrop,
            "Voltage drop",
            "length, current or voltage missing",
        )),
        Some(p) => {
            let ok = within(p, design.drop_limit_percent);
            rows.push(row(
                Rule::VoltageDrop,
                "Voltage drop",
                if ok { Status::Pass } else { Status::Fail },
                Some(p),
                Some(design.drop_limit_percent),
                "%",
                format!(
                    "{} % ({} V) over {} m at {} A, limit {} %",
                    num(p),
                    num(run.drop_volts().unwrap_or(0.0)),
                    num(cable.length_m),
                    num(design.load_current_a),
                    num(design.drop_limit_percent)
                ),
            ));
        }
    }

    // 3. Breaking capacity.
    match (&at_device, design.device.icu_ka) {
        (Some(f), Some(icu)) => {
            let ik = f.ik3_max_a.max(f.ik1_max_a.unwrap_or(0.0));
            let ok = within(ik, icu * 1000.0);
            rows.push(row(
                Rule::BreakingCapacity,
                "Breaking capacity",
                if ok { Status::Pass } else { Status::Fail },
                Some(ik / 1000.0),
                Some(icu),
                "kA",
                format!(
                    "the prospective short-circuit current at the device is {} kA, its breaking capacity {} kA",
                    num(ik / 1000.0), num(icu)
                ),
            ));
        }
        (None, _) => rows.push(unknown(
            Rule::BreakingCapacity,
            "Breaking capacity",
            "the network has no impedance at this point",
        )),
        (_, None) => rows.push(unknown(
            Rule::BreakingCapacity,
            "Breaking capacity",
            "the device's breaking capacity is not known; add it to the part",
        )),
    }

    // 4. Short-circuit withstand, maximum.
    let k = cable.insulation.k_copper();
    let s_total = cable.section_mm2 * f64::from(cable.parallel.max(1));
    let allowed = k * k * s_total * s_total;
    match &at_device {
        None => {}
        Some(f) => {
            let ik = f.ik3_max_a;
            let stated = design.device.let_through_i2t;
            let bound = clearing_time(&design.device, ik).map(|t| ik * ik * t);
            match (stated, bound) {
                (Some(e), _) => {
                    let ok = within(e, allowed);
                    rows.push(row(
                        Rule::ThermalWithstand,
                        "Short-circuit (maximum)",
                        if ok { Status::Pass } else { Status::Fail },
                        Some(e),
                        Some(allowed),
                        "A²s",
                        format!(
                            "{} A²s let through (stated by the maker) against k²S² = {} A²s for {} mm²",
                            num(e.round()),
                            num(allowed.round()),
                            num(s_total)
                        ),
                    ));
                }
                // Without the maker's let-through energy only an upper bound
                // exists (the full prospective current for the whole
                // guaranteed clearing time). It can prove a pass; it cannot
                // prove a fail, because a current-limiting device lets far
                // less through.
                (None, Some(e)) if within(e, allowed) => {
                    rows.push(row(
                        Rule::ThermalWithstand,
                        "Short-circuit (maximum)",
                        Status::Pass,
                        Some(e),
                        Some(allowed),
                        "A²s",
                        format!(
                            "even the unlimited energy {} A²s stays below k²S² = {} A²s for {} mm²",
                            num(e.round()),
                            num(allowed.round()),
                            num(s_total)
                        ),
                    ));
                }
                (None, Some(e)) => rows.push(row(
                    Rule::ThermalWithstand,
                    "Short-circuit (maximum)",
                    Status::Unknown,
                    Some(e),
                    Some(allowed),
                    "A²s",
                    format!(
                        "the unlimited energy {} A²s would exceed k²S² = {} A²s for {} mm²; add the device's let-through I²t at {} kA from its datasheet to decide",
                        num(e.round()),
                        num(allowed.round()),
                        num(s_total),
                        num((ik / 1000.0 * 10.0).round() / 10.0)
                    ),
                )),
                (None, None) => rows.push(unknown(
                    Rule::ThermalWithstand,
                    "Short-circuit (maximum)",
                    "the device's let-through energy and clearing time are not known",
                )),
            }
        }
    }

    // 5. Short-circuit, minimum at the load end.
    if let Some(end) = &at_load {
        let ik_min = end.ik2_min_a.min(end.ik1_min_a.unwrap_or(f64::INFINITY));
        match clearing_time(&design.device, ik_min) {
            None => rows.push(unknown(
                Rule::DisconnectionTime,
                "Short-circuit (minimum)",
                &format!(
                    "the smallest fault current at the cable end is {} A, but the device's time-current curve is not known there",
                    num(ik_min.round())
                ),
            )),
            Some(t) => {
                let t_allowed = allowed / (ik_min * ik_min);
                let ok = within(t, t_allowed);
                rows.push(row(
                    Rule::DisconnectionTime,
                    "Short-circuit (minimum)",
                    if ok { Status::Pass } else { Status::Fail },
                    Some(t),
                    Some(t_allowed),
                    "s",
                    format!(
                        "at the cable end the smallest fault is {} A: the device clears within {} s, the cable allows {} s",
                        num(ik_min.round()), num(t), num(t_allowed)
                    ),
                ));
            }
        }

        // 6. Disconnection on an earth fault (TN).
        let u0 = design.voltage
            / if matches!(design.supply, Supply::ThreePhase) {
                3.0_f64.sqrt()
            } else {
                1.0
            };
        let final_circuit = design.device.rated_a <= 32.0;
        // The table of IEC 60364-4-41 is by the nominal voltage to earth
        // (230 V on a 400 V system), not the 230.94 V that √3 gives.
        let u0 = (u0 / 5.0).round() * 5.0;
        let t_req = if final_circuit {
            max_disconnection_time(u0)
        } else {
            5.0
        };
        match end.ik1_min_a {
            None => rows.push(unknown(
                Rule::ProtectiveConductor,
                "Disconnection",
                "the protective conductor section is not given, so the earth-fault current is unknown",
            )),
            Some(ik1) => match clearing_time(&design.device, ik1) {
                None => rows.push(unknown(
                    Rule::ProtectiveConductor,
                    "Disconnection",
                    &format!(
                        "earth-fault current at the end is {} A; the device's time-current curve is not known there",
                        num(ik1.round())
                    ),
                )),
                Some(t) => {
                    let ok = within(t, t_req);
                    rows.push(row(
                        Rule::ProtectiveConductor,
                        "Disconnection",
                        if ok { Status::Pass } else { Status::Fail },
                        Some(t),
                        Some(t_req),
                        "s",
                        format!(
                            "an earth fault at the end gives {} A and clears within {} s; {} s is allowed at {} V to earth{}",
                            num(ik1.round()), num(t), num(t_req), num(u0.round()),
                            if final_circuit { "" } else { " (distribution circuit)" }
                        ),
                    ));
                    // 7. PE section by the adiabatic equation.
                    if let Some(pe) = cable.pe_mm2
                        && let Some(needed) =
                            min_pe_adiabatic(ik1, t.max(1e-3), cable.insulation)
                    {
                            let ok = within(needed, pe);
                            rows.push(row(
                                Rule::ProtectiveConductor,
                                "Protective conductor",
                                if ok { Status::Pass } else { Status::Fail },
                                Some(pe),
                                Some(needed),
                                "mm²",
                                format!(
                                    "the {} mm² protective conductor against {} mm² needed for {} A over {} s",
                                    num(pe), num(needed), num(ik1.round()), num(t)
                                ),
                            ));
                    }
                }
            },
        }
    } else {
        rows.push(unknown(
            Rule::ProtectiveConductor,
            "Disconnection",
            "the cable's section or length is not a valid number",
        ));
    }

    CircuitReport {
        name: design.name.clone(),
        rows,
        at_device,
        at_load,
        assumptions,
        tables: tables.disclosure(),
        iz_a: iz,
    }
}

/// The smallest listed section, at or above the design's, for which
/// nothing fails. A protective conductor given in the design follows the
/// phase conductor by the rule of IEC 60364-5-54 Table 54.2. Returns the
/// section and its report, or `None` when no listed section works (then
/// the device or the length has to change, which only the designer can
/// decide).
pub fn smallest_passing_section(
    network: &Network,
    at: NodeId,
    design: &CircuitDesign,
    tables: &TableSet,
) -> Option<(f64, CircuitReport)> {
    let mut trial = design.clone();
    for &s in tables.sections() {
        if s < design.cable.section_mm2 - 1e-9 {
            continue;
        }
        trial.cable.section_mm2 = s;
        if design.cable.pe_mm2.is_some() {
            trial.cable.pe_mm2 = min_pe_section(s);
        }
        let report = evaluate(network, at, &trial, tables);
        if !report.has_failure() {
            return Some((s, report));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_fusing_factor_follows_the_fuse_rating() {
        let fuse = |in_a| Device {
            characteristic: Some(Characteristic::FuseGg { in_a }),
            ..Device::default()
        };
        assert_eq!(i2_factor(&fuse(2.0)), 2.1);
        assert_eq!(i2_factor(&fuse(10.0)), 1.9);
        assert_eq!(i2_factor(&fuse(20.0)), 1.6);
    }
    use crate::calc::curves::McbCurve;
    use crate::calc::network::SourceModel;

    fn network(ik_ka: f64, ik_min_ka: f64) -> Network {
        Network::new(
            400.0,
            SourceModel::from_short_circuit(ik_ka, Some(ik_min_ka), 400.0).unwrap(),
        )
    }

    fn design(section: f64, length: f64) -> CircuitDesign {
        CircuitDesign {
            name: "M1".into(),
            load_current_a: 14.0,
            power_factor: 0.85,
            supply: Supply::ThreePhase,
            voltage: 400.0,
            overload_setting_a: None,
            device: Device {
                id: None,
                rated_a: 16.0,
                characteristic: Some(Characteristic::Mcb {
                    curve: McbCurve::C,
                    in_a: 16.0,
                }),
                trip: Some(Trip::C),
                icu_ka: Some(10.0),
                let_through_i2t: None,
            },
            cable: CableData {
                section_mm2: section,
                insulation: Insulation::Pvc,
                method: Method::C,
                ambient_c: 30.0,
                grouped_circuits: 1,
                length_m: length,
                parallel: 1,
                pe_mm2: Some(section),
            },
            drop_limit_percent: 5.0,
            notes: Vec::new(),
        }
    }

    fn find<'a>(r: &'a CircuitReport, title: &str) -> &'a Row {
        r.rows
            .iter()
            .find(|x| x.title == title)
            .unwrap_or_else(|| panic!("no {title}: {:?}", r.rows))
    }

    #[test]
    fn a_good_circuit_passes_everything_it_can_judge() {
        // 16 A curve C on 2.5 mm², 20 m, 10 kA supply.
        let net = network(10.0, 8.0);
        let rep = evaluate(&net, net.root(), &design(2.5, 20.0), &TableSet::builtin());
        for title in [
            "Overload",
            "Voltage drop",
            "Breaking capacity",
            "Short-circuit (minimum)",
            "Disconnection",
        ] {
            let r = find(&rep, title);
            assert_eq!(r.status, Status::Pass, "{title}: {}", r.text);
        }
        assert_eq!(rep.iz_a, Some(24.0));
        assert!(rep.tables.contains("not checked against the standard"));
        assert!(rep.assumptions.iter().any(|a| a.contains("R/X")));
    }

    #[test]
    fn a_long_cable_fails_where_it_should() {
        // 150 m of 1.5 mm² on 16 A curve C: overload row fails (Iz 17.5 A at
        // 1.5 mm²: In 16 is allowed, but the drop is far over the limit and
        // the earth fault no longer trips the magnetic stage).
        let net = network(10.0, 8.0);
        let mut d = design(1.5, 150.0);
        d.load_current_a = 10.0;
        let rep = evaluate(&net, net.root(), &d, &TableSet::builtin());
        assert_eq!(find(&rep, "Voltage drop").status, Status::Fail);
        assert_eq!(find(&rep, "Disconnection").status, Status::Fail);
        assert!(rep.has_failure());
    }

    #[test]
    fn unknown_data_stays_unknown() {
        let net = network(10.0, 8.0);
        let mut d = design(2.5, 20.0);
        d.device.icu_ka = None;
        d.device.characteristic = None;
        d.device.trip = None;
        d.cable.pe_mm2 = None;
        let rep = evaluate(&net, net.root(), &d, &TableSet::builtin());
        assert_eq!(find(&rep, "Breaking capacity").status, Status::Unknown);
        assert_eq!(find(&rep, "Disconnection").status, Status::Unknown);
        assert_eq!(
            find(&rep, "Short-circuit (maximum)").status,
            Status::Unknown
        );
        assert!(!rep.all_pass());
        assert!(!rep.has_failure());
        // Overload and drop are still judged.
        assert_eq!(find(&rep, "Overload").status, Status::Pass);
    }

    #[test]
    fn breaking_capacity_fails_on_a_strong_supply() {
        let net = network(25.0, 20.0);
        let rep = evaluate(&net, net.root(), &design(2.5, 5.0), &TableSet::builtin());
        let r = find(&rep, "Breaking capacity");
        assert_eq!(r.status, Status::Fail);
        assert!(r.text.contains("25") || r.text.contains("26"), "{}", r.text);
    }

    #[test]
    fn the_second_overload_condition_catches_a_fuse_on_a_thin_cable() {
        // A 20 A gG fuse on 2.5 mm² (Iz 24 A, PVC C three): In ≤ Iz holds but
        // 1.6 × 20 = 32 A > 1.45 × 24 = 34.8? No: 32 ≤ 34.8, it passes. A
        // 25 A fuse: 40 A > 34.8 fails the second condition? In 25 > Iz 24
        // already fails the first. Use grouping to bring Iz to 22 A:
        // 24 · 0.92... pick ambient 35 °C: Iz = 22.56; 20 A fuse: 32 vs 32.7 ok.
        // At 40 °C Iz = 20.9: 32 > 30.3 fails the second condition while the
        // first (20 ≤ 20.9) holds.
        let net = network(10.0, 8.0);
        let mut d = design(2.5, 10.0);
        d.load_current_a = 18.0;
        d.device = Device {
            rated_a: 20.0,
            characteristic: Some(Characteristic::FuseGg { in_a: 20.0 }),
            icu_ka: Some(100.0),
            ..Device::default()
        };
        d.cable.ambient_c = 40.0;
        let rep = evaluate(&net, net.root(), &d, &TableSet::builtin());
        let r = find(&rep, "Overload");
        assert_eq!(r.status, Status::Fail);
        assert!(r.text.contains("1.45"), "{}", r.text);
        // The same cable with a breaker (I2 = 1.45 In = 29 A) passes.
        d.device.characteristic = Some(Characteristic::Mcb {
            curve: McbCurve::C,
            in_a: 20.0,
        });
        assert_eq!(
            find(
                &evaluate(&net, net.root(), &d, &TableSet::builtin()),
                "Overload"
            )
            .status,
            Status::Pass
        );
    }

    #[test]
    fn parallel_cables_share_the_current_and_the_withstand() {
        let net = network(10.0, 8.0);
        let mut d = design(35.0, 30.0);
        d.load_current_a = 120.0;
        d.device.rated_a = 125.0;
        d.device.characteristic = Some(Characteristic::Mcb {
            curve: McbCurve::C,
            in_a: 125.0,
        });
        d.device.trip = None;
        let single = evaluate(&net, net.root(), &d, &TableSet::builtin());
        assert_eq!(find(&single, "Overload").status, Status::Fail);
        d.cable.parallel = 2;
        d.cable.grouped_circuits = 2;
        let double = evaluate(&net, net.root(), &d, &TableSet::builtin());
        assert_eq!(find(&double, "Overload").status, Status::Pass);
        assert!(double.iz_a.unwrap() > single.iz_a.unwrap());
    }

    #[test]
    fn a_licensed_table_changes_the_answer_and_the_disclosure() {
        let net = network(10.0, 8.0);
        let d = design(2.5, 20.0);
        let licensed = TableSet::builtin()
            .with_csv(
                "IEC 60364-5-52:2009, licensed copy",
                "ampacity,PVC,C,3,2.5,15\nampacity,PVC,C,3,4,20\nambient,PVC,30,1\nambient,PVC,60,0.5\ngrouping,1,1\ngrouping,20,0.38\n",
            )
            .unwrap();
        let rep = evaluate(&net, net.root(), &d, &licensed);
        assert_eq!(rep.iz_a, Some(15.0));
        assert_eq!(find(&rep, "Overload").status, Status::Fail);
        assert!(rep.tables.contains("licensed"));
    }

    #[test]
    fn the_suggested_section_is_the_smallest_that_clears_every_failure() {
        let net = network(10.0, 8.0);
        let mut d = design(1.5, 120.0);
        d.load_current_a = 14.0;
        let tables = TableSet::builtin();
        let before = evaluate(&net, net.root(), &d, &tables);
        assert!(before.has_failure());
        let (section, after) = smallest_passing_section(&net, net.root(), &d, &tables).unwrap();
        assert!(section > 1.5);
        assert!(!after.has_failure());
        // The next smaller listed section still fails.
        let idx = tables
            .sections()
            .iter()
            .position(|s| *s == section)
            .unwrap();
        if idx > 0 && tables.sections()[idx - 1] >= 1.5 {
            let mut smaller = d.clone();
            smaller.cable.section_mm2 = tables.sections()[idx - 1];
            smaller.cable.pe_mm2 = min_pe_section(smaller.cable.section_mm2);
            assert!(evaluate(&net, net.root(), &smaller, &tables).has_failure());
        }
        // A hopeless request has no answer.
        let mut huge = d.clone();
        huge.load_current_a = 900.0;
        huge.device.rated_a = 1000.0;
        assert!(smallest_passing_section(&net, net.root(), &huge, &tables).is_none());
    }

    #[test]
    fn invalid_cable_input_reports_unknown_rows_instead_of_panicking() {
        let net = network(10.0, 8.0);
        let mut d = design(0.0, 20.0);
        d.cable.parallel = 0;
        let rep = evaluate(&net, net.root(), &d, &TableSet::builtin());
        assert!(rep.rows.iter().any(|r| r.status == Status::Unknown));
        assert!(rep.at_load.is_none());
    }
}
