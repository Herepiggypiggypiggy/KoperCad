//! One-click motor starter sizing: from a motor and the supply at the
//! board to a direct-on-line starter (short-circuit protection, contactor,
//! overload protection) and a cable, judged by the same
//! [`circuit::evaluate`](super::circuit::evaluate) a hand-drawn circuit gets.
//!
//! The answer is a [`Proposal`]: the chosen parts with the reason for each,
//! the candidates that were rejected and why, what had to be assumed, and
//! the full [`CircuitReport`]. When the catalog lacks the data to decide
//! (no coordination table, no gG melting times) the proposal says so as
//! *unresolved*: it never fills a gap with a guess that looks like a result.
//!
//! # Rules used
//!
//! | Step | Rule | Source |
//! | --- | --- | --- |
//! | Motor current | nameplate; else `P / (√3·U·η·cos φ)` with η and cos φ typical for the size, and said so | IEC 60034-30-1 efficiency classes (typical IE3, 4 pole) |
//! | Overload relay | setting range contains `Ie`, not in its lowest tenth; trip class by starting time: ≤ 10 s class 10 (or 10A), ≤ 20 s class 20, ≤ 30 s class 30 | IEC 60947-4-1 |
//! | Contactor | AC-3 current ≥ `Ie` | IEC 60947-4-1 |
//! | Fuse | smallest standard gG ≥ 1.6, 2.0 or 2.5 × `Ie` for starts up to 5, 10 and 30 s, and not above the contactor's and relay's largest coordinated fuse | rule of thumb, to be confirmed on the maker's gG curve |
//! | Motor breaker | setting range contains `Ie`; trip class at least the starting class; magnetic trip ≥ 12 × `Ie` | IEC 60947-4-1 |
//! | Breaking capacity | `Icu` ≥ prospective current at the board, or back-up from a table | IEC 60947-2 |
//! | Coordination | type 1 or 2 from the maker's pair table | IEC 60947-4-1 |
//! | Cable | smallest section for which nothing fails in the circuit report | IEC 60364 |

use super::Insulation;
use super::cable::Method;
use super::circuit::{
    CableData, CircuitDesign, CircuitReport, Device, Status, smallest_passing_section,
};
use super::curves::{Characteristic, RelayClass};
use super::drop::Supply;
use super::motor::full_load_current;
use super::network::{Network, NodeId};
use super::selectivity::PairTable;
use super::tables::TableSet;

/// The motor to start.
#[derive(Debug, Clone, PartialEq)]
pub struct MotorSpec {
    pub power_kw: f64,
    /// Line-to-line voltage, V.
    pub voltage: f64,
    /// Nameplate current, A. Preferred over every estimate.
    pub rated_current_a: Option<f64>,
    pub efficiency: Option<f64>,
    pub cos_phi: Option<f64>,
    /// Starting time in seconds, to the end of the run-up.
    pub start_time_s: Option<f64>,
}

/// Typical IE3 four-pole efficiency by shaft power (IEC 60034-30-1), used
/// only when the plate gives no current.
pub fn typical_efficiency(power_kw: f64) -> f64 {
    const TABLE: [(f64, f64); 21] = [
        (0.75, 0.825),
        (1.1, 0.841),
        (1.5, 0.853),
        (2.2, 0.867),
        (3.0, 0.877),
        (4.0, 0.886),
        (5.5, 0.896),
        (7.5, 0.904),
        (11.0, 0.914),
        (15.0, 0.921),
        (18.5, 0.926),
        (22.0, 0.930),
        (30.0, 0.936),
        (37.0, 0.939),
        (45.0, 0.942),
        (55.0, 0.946),
        (75.0, 0.950),
        (90.0, 0.952),
        (110.0, 0.954),
        (132.0, 0.956),
        (200.0, 0.960),
    ];
    TABLE
        .iter()
        .find(|(p, _)| power_kw <= *p)
        .map_or(0.96, |(_, e)| *e)
}

/// Typical power factor at full load by size.
pub fn typical_cos_phi(power_kw: f64) -> f64 {
    match power_kw {
        p if p <= 1.5 => 0.78,
        p if p <= 4.0 => 0.80,
        p if p <= 7.5 => 0.81,
        p if p <= 11.0 => 0.82,
        p if p <= 22.0 => 0.84,
        p if p <= 37.0 => 0.85,
        p if p <= 75.0 => 0.86,
        _ => 0.87,
    }
}

/// The motor's current and the estimates behind it.
pub fn motor_current(m: &MotorSpec) -> Option<(f64, Vec<String>)> {
    if let Some(i) = m.rated_current_a {
        return (i.is_finite() && i > 0.0)
            .then(|| (i, vec![format!("motor current {i} A from the nameplate")]));
    }
    let mut notes = Vec::new();
    let eta = m.efficiency.unwrap_or_else(|| {
        notes.push(format!(
            "efficiency not given: typical IE3 4-pole {:.1} % for {} kW (replace with the nameplate)",
            typical_efficiency(m.power_kw) * 100.0,
            m.power_kw
        ));
        typical_efficiency(m.power_kw)
    });
    let cos = m.cos_phi.unwrap_or_else(|| {
        notes.push(format!(
            "power factor not given: typical {:.2} for {} kW (replace with the nameplate)",
            typical_cos_phi(m.power_kw),
            m.power_kw
        ));
        typical_cos_phi(m.power_kw)
    });
    full_load_current(m.power_kw, m.voltage, eta, cos).map(|i| (i, notes))
}

/// A contactor in the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactorOption {
    pub id: String,
    pub ac3_a: f64,
    /// Largest fuse allowed in front of it for type 1 and type 2
    /// coordination, A, from the maker.
    pub max_fuse_type1_a: Option<f64>,
    pub max_fuse_type2_a: Option<f64>,
}

/// An overload relay in the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayOption {
    pub id: String,
    pub min_a: f64,
    pub max_a: f64,
    pub class: RelayClass,
    pub max_fuse_a: Option<f64>,
}

/// A motor protection breaker in the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct MotorBreakerOption {
    pub id: String,
    pub min_a: f64,
    pub max_a: f64,
    pub class: RelayClass,
    /// Magnetic trip current at the setting `max_a`, A.
    pub ii_a: f64,
    pub icu_ka: Option<f64>,
}

/// A fuse in the catalog (gG, motor use).
#[derive(Debug, Clone, PartialEq)]
pub struct FuseOption {
    pub id: String,
    pub rated_a: f64,
    pub icu_ka: Option<f64>,
    /// Maker's melting data, when loaded.
    pub characteristic: Option<Characteristic>,
    pub let_through_i2t: Option<f64>,
}

/// What the designer may choose from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub contactors: Vec<ContactorOption>,
    pub relays: Vec<RelayOption>,
    pub motor_breakers: Vec<MotorBreakerOption>,
    pub fuses: Vec<FuseOption>,
}

/// Standard gG fuse ratings in amperes (IEC 60269-1 preferred values).
pub const GG_RATINGS: [f64; 26] = [
    2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 25.0, 32.0, 35.0, 40.0, 50.0, 63.0, 80.0, 100.0,
    125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0,
];

/// Typical AC-3 current ratings in amperes at 400 V.
pub const AC3_RATINGS: [f64; 17] = [
    9.0, 12.0, 18.0, 25.0, 32.0, 40.0, 50.0, 65.0, 80.0, 95.0, 115.0, 150.0, 185.0, 225.0, 265.0,
    330.0, 400.0,
];

impl Catalog {
    /// A catalog of *ratings* with no maker data: standard fuses and
    /// typical contactor sizes. Proposals made from it name what to look
    /// for ("contactor AC-3 ≥ 25 A") and leave coordination unresolved.
    pub fn generic() -> Catalog {
        Catalog {
            contactors: AC3_RATINGS
                .iter()
                .map(|a| ContactorOption {
                    id: format!("contactor AC-3 ≥ {a} A"),
                    ac3_a: *a,
                    max_fuse_type1_a: None,
                    max_fuse_type2_a: None,
                })
                .collect(),
            relays: Vec::new(),
            motor_breakers: Vec::new(),
            fuses: GG_RATINGS
                .iter()
                .map(|a| FuseOption {
                    id: format!("gG fuse {a} A"),
                    rated_a: *a,
                    icu_ka: None,
                    characteristic: Some(Characteristic::FuseGg { in_a: *a }),
                    let_through_i2t: None,
                })
                .collect(),
        }
    }
}

/// How the starter is protected against short circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// gG fuse, contactor and separate overload relay.
    FuseContactorRelay,
    /// Motor protection breaker (thermal and magnetic) and contactor.
    MotorBreakerContactor,
}

/// Coordination type to reach (IEC 60947-4-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coordination {
    Type1,
    Type2,
}

/// Where the cable runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub length_m: f64,
    pub insulation: Insulation,
    pub method: Method,
    pub ambient_c: f64,
    pub grouped_circuits: u32,
}

pub struct SizingInput<'a> {
    pub motor: &'a MotorSpec,
    pub scheme: Scheme,
    pub coordination: Coordination,
    pub network: &'a Network,
    /// Where the starter's protective device is connected.
    pub at: NodeId,
    pub catalog: &'a Catalog,
    pub tables: &'a TableSet,
    pub pairs: Option<&'a PairTable>,
    pub route: Route,
    pub name: &'a str,
}

/// One chosen part.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub id: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub motor_current_a: f64,
    pub short_circuit: Choice,
    pub contactor: Choice,
    /// The overload relay, or the breaker's own thermal release.
    pub overload: Choice,
    pub cable_mm2: f64,
    pub pe_mm2: f64,
    pub design: CircuitDesign,
    pub report: CircuitReport,
    /// Candidates passed over, with the reason.
    pub rejected: Vec<String>,
    /// Things the catalog could not settle, to be resolved by loading the
    /// maker's data.
    pub unresolved: Vec<String>,
    pub assumptions: Vec<String>,
}

/// The short-circuit device and overload protection once chosen.
struct Picked {
    scpd: Choice,
    relay: Choice,
    characteristic: Option<Characteristic>,
    icu_ka: Option<f64>,
    let_through_i2t: Option<f64>,
    id: String,
    rated_a: f64,
}

/// Why no proposal could be made.
#[derive(Debug, Clone, PartialEq)]
pub struct NoProposal {
    pub reasons: Vec<String>,
}

/// Required relay class for a starting time; `None` above 30 s (needs a
/// special solution).
pub fn required_class(start_time_s: f64) -> Option<RelayClass> {
    if start_time_s <= 10.0 {
        Some(RelayClass::C10)
    } else if start_time_s <= 20.0 {
        Some(RelayClass::C20)
    } else if start_time_s <= 30.0 {
        Some(RelayClass::C30)
    } else {
        None
    }
}

fn class_rank(c: RelayClass) -> u8 {
    match c {
        RelayClass::C10A => 0,
        RelayClass::C10 => 1,
        RelayClass::C20 => 2,
        RelayClass::C30 => 3,
    }
}

/// Fuse factor over `Ie` by starting time (rule of thumb).
pub fn fuse_factor(start_time_s: f64) -> f64 {
    if start_time_s <= 5.0 {
        1.6
    } else if start_time_s <= 10.0 {
        2.0
    } else {
        2.5
    }
}

/// Sizes the starter and its cable.
pub fn size_starter(input: &SizingInput<'_>) -> Result<Proposal, NoProposal> {
    let m = input.motor;
    let fail = |reasons: Vec<String>| Err(NoProposal { reasons });
    let Some((ie, mut assumptions)) = motor_current(m) else {
        return fail(vec!["the motor current cannot be found: give the nameplate current or power, voltage, efficiency and power factor".into()]);
    };
    let mut rejected = Vec::new();
    let mut unresolved = Vec::new();

    let start = m.start_time_s.unwrap_or_else(|| {
        assumptions.push("starting time not given: 5 s assumed (light start, class 10)".into());
        5.0
    });
    let Some(need_class) = required_class(start) else {
        return fail(vec![format!(
            "a starting time of {start} s is above class 30: needs a soft starter or special protection, outside this sizing"
        )]);
    };

    let ik = input.network.fault(input.at);
    let Some(ik) = ik else {
        return fail(vec!["the network has no impedance at the device".into()]);
    };
    let ik_ka = ik.ik3_max_a.max(ik.ik1_max_a.unwrap_or(0.0)) / 1000.0;
    let want_type = match input.coordination {
        Coordination::Type1 => 1,
        Coordination::Type2 => 2,
    };

    // Contactors by size, then the scheme's other parts.
    let mut contactors: Vec<&ContactorOption> = input
        .catalog
        .contactors
        .iter()
        .filter(|c| {
            let ok = c.ac3_a + 1e-9 >= ie;
            if !ok {
                rejected.push(format!(
                    "{}: AC-3 {} A is below the motor's {:.1} A",
                    c.id, c.ac3_a, ie
                ));
            }
            ok
        })
        .collect();
    contactors.sort_by(|a, b| a.ac3_a.total_cmp(&b.ac3_a));

    // The short-circuit device and (if separate) the relay.
    let contactor_choice;
    let chosen_parts: Picked;

    match input.scheme {
        Scheme::FuseContactorRelay => {
            let factor = fuse_factor(start);
            // Relay: narrowest range containing Ie, class at least required.
            let mut relays: Vec<&RelayOption> = Vec::new();
            for r in &input.catalog.relays {
                if class_rank(r.class) < class_rank(need_class) {
                    rejected.push(format!(
                        "{}: class {:?} is too fast for a {start} s start",
                        r.id, r.class
                    ));
                } else if !(r.min_a - 1e-9 <= ie && ie <= r.max_a + 1e-9) {
                    rejected.push(format!(
                        "{}: range {}–{} A does not contain {:.1} A",
                        r.id, r.min_a, r.max_a, ie
                    ));
                } else if ie < r.min_a + 0.1 * (r.max_a - r.min_a) {
                    rejected.push(format!(
                        "{}: {:.1} A is in the bottom tenth of its range",
                        r.id, ie
                    ));
                } else {
                    relays.push(r);
                }
            }
            relays.sort_by(|a, b| (a.max_a - a.min_a).total_cmp(&(b.max_a - b.min_a)));
            let relay = relays.first();
            if relay.is_none() {
                unresolved.push(format!(
                    "overload relay: none in the catalog; look for class {} (or slower) with a range containing {:.1} A",
                    class_name(need_class),
                    ie
                ));
            }
            // Fuse: smallest standard value ≥ factor · Ie that also fits the
            // contactor and relay limits.
            let min_fuse = factor * ie;
            let mut fuses: Vec<&FuseOption> = input
                .catalog
                .fuses
                .iter()
                .filter(|f| f.rated_a + 1e-9 >= min_fuse)
                .collect();
            fuses.sort_by(|a, b| a.rated_a.total_cmp(&b.rated_a));
            let mut picked = None;
            'search: for f in &fuses {
                for c in &contactors {
                    let limit = match input.coordination {
                        Coordination::Type1 => c.max_fuse_type1_a,
                        Coordination::Type2 => c.max_fuse_type2_a,
                    };
                    let table = input.pairs.and_then(|p| p.coordination(&f.id, &c.id));
                    let relay_limit = relay.and_then(|r| r.max_fuse_a);
                    let table_ok = table.is_some_and(|t| t >= want_type);
                    let limit_ok = limit.is_some_and(|l| f.rated_a <= l + 1e-9);
                    let relay_ok = relay_limit.is_none_or(|l| f.rated_a <= l + 1e-9);
                    if !relay_ok {
                        rejected.push(format!(
                            "{} with {}: above the relay's {} A fuse limit",
                            f.id,
                            c.id,
                            relay_limit.unwrap_or(0.0)
                        ));
                        continue;
                    }
                    if table_ok || limit_ok {
                        picked = Some((*f, *c, true));
                        break 'search;
                    }
                    if limit.is_none() && table.is_none() && picked.is_none() {
                        // No coordination data at all for this pair: usable but unresolved.
                        picked = Some((*f, *c, false));
                        break 'search;
                    }
                    rejected.push(format!(
                        "{} with {}: not coordinated to type {}",
                        f.id, c.id, want_type
                    ));
                }
            }
            let Some((f, c, coordinated)) = picked else {
                return fail(vec![format!(
                    "no fuse of at least {:.0} A coordinates with a contactor of the catalog",
                    min_fuse.ceil()
                )]);
            };
            if !coordinated {
                unresolved.push(format!(
                    "coordination type {want_type} of {} with {}: no maker table loaded (largest permitted fuse unknown)",
                    f.id, c.id
                ));
            }
            contactor_choice = Choice {
                id: c.id.clone(),
                why: format!("smallest contactor with AC-3 ≥ {:.1} A ({} A)", ie, c.ac3_a),
            };
            let scpd = Choice {
                id: f.id.clone(),
                why: format!(
                    "smallest standard gG ≥ {factor} × {:.1} A = {:.0} A for a {start} s start (rule of thumb: confirm on the maker's gG curve)",
                    ie, min_fuse
                ),
            };
            let relay_choice = match relay {
                Some(r) => Choice {
                    id: r.id.clone(),
                    why: format!(
                        "range {}–{} A contains {:.1} A, class {}",
                        r.min_a,
                        r.max_a,
                        ie,
                        class_name(r.class)
                    ),
                },
                None => Choice {
                    id: format!(
                        "overload relay class {} for {:.1} A",
                        class_name(need_class),
                        ie
                    ),
                    why: "no maker relay in the catalog: this is the requirement".into(),
                },
            };
            chosen_parts = Picked {
                scpd,
                relay: relay_choice,
                characteristic: f.characteristic.clone(),
                icu_ka: f.icu_ka,
                let_through_i2t: f.let_through_i2t,
                id: f.id.clone(),
                rated_a: f.rated_a,
            };
        }
        Scheme::MotorBreakerContactor => {
            let mut breakers: Vec<&MotorBreakerOption> = Vec::new();
            for b in &input.catalog.motor_breakers {
                if class_rank(b.class) < class_rank(need_class) {
                    rejected.push(format!(
                        "{}: class {:?} is too fast for a {start} s start",
                        b.id, b.class
                    ));
                } else if !(b.min_a - 1e-9 <= ie && ie <= b.max_a + 1e-9) {
                    rejected.push(format!(
                        "{}: range {}–{} A does not contain {:.1} A",
                        b.id, b.min_a, b.max_a, ie
                    ));
                } else if b.ii_a + 1e-9 < 12.0 * ie {
                    rejected.push(format!(
                        "{}: magnetic trip {} A is below 12 × {:.1} A and may trip on starting",
                        b.id, b.ii_a, ie
                    ));
                } else if b.icu_ka.is_some_and(|i| i + 1e-9 < ik_ka) && input.pairs.is_none() {
                    rejected.push(format!(
                        "{}: breaking capacity {} kA is below the prospective {:.1} kA",
                        b.id,
                        b.icu_ka.unwrap_or(0.0),
                        ik_ka
                    ));
                } else {
                    breakers.push(b);
                }
            }
            breakers.sort_by(|a, b| (a.max_a - a.min_a).total_cmp(&(b.max_a - b.min_a)));
            let Some(b) = breakers.first() else {
                return fail({
                    let mut r = vec![
                        "no motor protection breaker in the catalog suits the motor".to_owned(),
                    ];
                    r.extend(rejected);
                    r
                });
            };
            // Contactor coordinated with the breaker.
            let mut chosen = None;
            for c in &contactors {
                match input.pairs.and_then(|p| p.coordination(&b.id, &c.id)) {
                    Some(t) if t >= want_type => {
                        chosen = Some((*c, true));
                        break;
                    }
                    Some(t) => {
                        rejected.push(format!("{} with {}: tested to type {t} only", b.id, c.id))
                    }
                    None if chosen.is_none() => {
                        chosen = Some((*c, false));
                        break;
                    }
                    None => {}
                }
            }
            let Some((c, coordinated)) = chosen else {
                return fail(vec![format!(
                    "no contactor of the catalog is coordinated with {} to type {want_type}",
                    b.id
                )]);
            };
            if !coordinated {
                unresolved.push(format!(
                    "coordination type {want_type} of {} with {}: no maker table loaded",
                    b.id, c.id
                ));
            }
            if b.icu_ka.is_none() {
                unresolved.push(format!(
                    "breaking capacity of {} is not in the catalog",
                    b.id
                ));
            }
            contactor_choice = Choice {
                id: c.id.clone(),
                why: format!("smallest contactor with AC-3 ≥ {:.1} A ({} A)", ie, c.ac3_a),
            };
            let scpd = Choice {
                id: b.id.clone(),
                why: format!(
                    "setting range {}–{} A contains {:.1} A, magnetic trip {} A ≥ 12 × Ie, class {}",
                    b.min_a,
                    b.max_a,
                    ie,
                    b.ii_a,
                    class_name(b.class)
                ),
            };
            let relay_choice = Choice {
                id: format!("{} (thermal release)", b.id),
                why: format!("set to {:.1} A", ie),
            };
            chosen_parts = Picked {
                scpd,
                relay: relay_choice,
                characteristic: Some(Characteristic::ThermalMagnetic {
                    class: b.class,
                    ir_a: ie,
                    ii_a: b.ii_a,
                }),
                icu_ka: b.icu_ka,
                let_through_i2t: None,
                id: b.id.clone(),
                rated_a: b.max_a,
            };
        }
    }

    let cos = m.cos_phi.unwrap_or_else(|| typical_cos_phi(m.power_kw));
    let design = CircuitDesign {
        name: input.name.to_owned(),
        load_current_a: ie,
        power_factor: cos,
        supply: Supply::ThreePhase,
        voltage: m.voltage,
        device: Device {
            id: Some(chosen_parts.id.clone()),
            rated_a: chosen_parts.rated_a,
            characteristic: chosen_parts.characteristic.clone(),
            trip: None,
            icu_ka: chosen_parts.icu_ka,
            let_through_i2t: chosen_parts.let_through_i2t,
        },
        overload_setting_a: Some(ie),
        cable: CableData {
            section_mm2: 1.5,
            insulation: input.route.insulation,
            method: input.route.method,
            ambient_c: input.route.ambient_c,
            grouped_circuits: input.route.grouped_circuits,
            length_m: input.route.length_m,
            parallel: 1,
            pe_mm2: Some(1.5),
        },
        drop_limit_percent: 5.0,
        notes: assumptions.clone(),
    };
    let Some((section, report)) =
        smallest_passing_section(input.network, input.at, &design, input.tables)
    else {
        let last = super::circuit::evaluate(input.network, input.at, &design, input.tables);
        let mut reasons = vec![
            "no listed cable section clears every check for this run: shorten it, use a stronger source or change the protective device".to_owned(),
        ];
        reasons.extend(
            last.rows
                .iter()
                .filter(|r| r.status == Status::Fail)
                .map(|r| format!("{}: {}", r.title, r.text)),
        );
        return fail(reasons);
    };
    let mut design = design;
    design.cable.section_mm2 = section;
    design.cable.pe_mm2 = super::pe::min_pe_section(section);
    for r in report.rows.iter().filter(|r| r.status == Status::Unknown) {
        unresolved.push(format!("{}: {}", r.title, r.text));
    }
    Ok(Proposal {
        motor_current_a: ie,
        short_circuit: chosen_parts.scpd,
        contactor: contactor_choice,
        overload: chosen_parts.relay,
        cable_mm2: section,
        pe_mm2: design.cable.pe_mm2.unwrap_or(section),
        design,
        report,
        rejected,
        unresolved,
        assumptions,
    })
}

fn class_name(c: RelayClass) -> &'static str {
    match c {
        RelayClass::C10A => "10A",
        RelayClass::C10 => "10",
        RelayClass::C20 => "20",
        RelayClass::C30 => "30",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::network::SourceModel;

    fn board(ik_ka: f64) -> Network {
        Network::new(
            400.0,
            SourceModel::from_short_circuit(ik_ka, Some(ik_ka * 0.7), 400.0).unwrap(),
        )
    }

    fn motor(kw: f64) -> MotorSpec {
        MotorSpec {
            power_kw: kw,
            voltage: 400.0,
            rated_current_a: None,
            efficiency: None,
            cos_phi: None,
            start_time_s: None,
        }
    }

    fn route(len: f64) -> Route {
        Route {
            length_m: len,
            insulation: Insulation::Pvc,
            method: Method::C,
            ambient_c: 30.0,
            grouped_circuits: 1,
        }
    }

    fn maker_catalog() -> Catalog {
        let mut c = Catalog::generic();
        c.contactors = vec![
            ContactorOption {
                id: "KM-12".into(),
                ac3_a: 12.0,
                max_fuse_type1_a: Some(25.0),
                max_fuse_type2_a: Some(20.0),
            },
            ContactorOption {
                id: "KM-25".into(),
                ac3_a: 25.0,
                max_fuse_type1_a: Some(63.0),
                max_fuse_type2_a: Some(40.0),
            },
            ContactorOption {
                id: "KM-40".into(),
                ac3_a: 40.0,
                max_fuse_type1_a: Some(100.0),
                max_fuse_type2_a: Some(80.0),
            },
        ];
        c.relays = vec![
            RelayOption {
                id: "RT 6-10".into(),
                min_a: 6.0,
                max_a: 10.0,
                class: RelayClass::C10,
                max_fuse_a: Some(40.0),
            },
            RelayOption {
                id: "RT 17-25".into(),
                min_a: 17.0,
                max_a: 25.0,
                class: RelayClass::C10,
                max_fuse_a: Some(63.0),
            },
        ];
        c.motor_breakers = vec![
            MotorBreakerOption {
                id: "MPCB 10-16".into(),
                min_a: 10.0,
                max_a: 16.0,
                class: RelayClass::C10,
                ii_a: 208.0,
                icu_ka: Some(50.0),
            },
            MotorBreakerOption {
                id: "MPCB 20-32".into(),
                min_a: 20.0,
                max_a: 32.0,
                class: RelayClass::C10,
                ii_a: 416.0,
                icu_ka: Some(50.0),
            },
        ];
        c
    }

    #[test]
    fn motor_current_from_the_plate_or_a_stated_estimate() {
        let (i, notes) = motor_current(&MotorSpec {
            rated_current_a: Some(21.5),
            ..motor(11.0)
        })
        .unwrap();
        assert_eq!(i, 21.5);
        assert!(notes[0].contains("nameplate"));
        // 11 kW, IE3 4-pole 91.4 %, typical cos φ 0.82:
        // I = 11000/(√3·400·0.914·0.82) = 21.2 A (catalogue motors: 21 to 22 A).
        let (i, notes) = motor_current(&motor(11.0)).unwrap();
        assert!((i - 21.2).abs() < 0.3, "{i}");
        assert_eq!(notes.len(), 2);
        assert!(
            notes
                .iter()
                .all(|n| n.contains("replace with the nameplate"))
        );
        assert!(
            motor_current(&MotorSpec {
                power_kw: 0.0,
                ..motor(0.0)
            })
            .is_none()
        );
    }

    #[test]
    fn a_4_kw_motor_with_fuse_contactor_and_relay() {
        // 4 kW: about 8.2 A; fuse ≥ 1.6 · 8.2 = 13.0 → 16 A; contactor 12 A (max fuse
        // type 2 20 A); relay 6–10 A with class 10.
        let net = board(15.0);
        let cat = maker_catalog();
        let tables = TableSet::builtin();
        let p = size_starter(&SizingInput {
            motor: &motor(4.0),
            scheme: Scheme::FuseContactorRelay,
            coordination: Coordination::Type2,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(30.0),
            name: "M1",
        })
        .unwrap();
        assert!(
            (p.motor_current_a - 8.5).abs() < 0.5,
            "{}",
            p.motor_current_a
        );
        assert_eq!(p.short_circuit.id, "gG fuse 16 A");
        assert_eq!(p.contactor.id, "KM-12");
        assert_eq!(p.overload.id, "RT 6-10");
        assert_eq!(p.cable_mm2, 1.5);
        assert!(!p.report.has_failure(), "{}", p.report.to_text());
        // gG melting data is not in the catalog: the short-circuit rows are
        // honestly unresolved.
        assert!(p.unresolved.iter().any(|u| u.contains("Short-circuit")));
        assert!(
            p.assumptions
                .iter()
                .any(|a| a.contains("starting time not given"))
        );
    }

    #[test]
    fn a_long_run_gets_a_thicker_cable_and_says_why() {
        let net = board(15.0);
        let cat = maker_catalog();
        let tables = TableSet::builtin();
        let short = size_starter(&SizingInput {
            motor: &MotorSpec {
                rated_current_a: Some(16.0),
                ..motor(7.5)
            },
            scheme: Scheme::MotorBreakerContactor,
            coordination: Coordination::Type1,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(15.0),
            name: "M2",
        })
        .unwrap();
        let long = size_starter(&SizingInput {
            motor: &MotorSpec {
                rated_current_a: Some(16.0),
                ..motor(7.5)
            },
            scheme: Scheme::MotorBreakerContactor,
            coordination: Coordination::Type1,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(120.0),
            name: "M2",
        })
        .unwrap();
        assert!(
            long.cable_mm2 > short.cable_mm2,
            "{} vs {}",
            long.cable_mm2,
            short.cable_mm2
        );
        assert_eq!(short.short_circuit.id, "MPCB 10-16");
        assert!(short.overload.id.contains("thermal release"));
        // Coordination table missing: stated as unresolved, not assumed.
        assert!(short.unresolved.iter().any(|u| u.contains("coordination")));
        assert!(!long.report.has_failure(), "{}", long.report.to_text());
    }

    #[test]
    fn a_coordination_table_resolves_the_pair() {
        let net = board(15.0);
        let cat = maker_catalog();
        let tables = TableSet::builtin();
        let pairs = PairTable::from_csv(
            "maker",
            "coordination,MPCB 10-16,KM-25,2\ncoordination,MPCB 10-16,KM-12,1\n",
        )
        .unwrap();
        let p = size_starter(&SizingInput {
            motor: &MotorSpec {
                rated_current_a: Some(11.0),
                ..motor(5.5)
            },
            scheme: Scheme::MotorBreakerContactor,
            coordination: Coordination::Type2,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: Some(&pairs),
            route: route(20.0),
            name: "M3",
        })
        .unwrap();
        // KM-12 is only type 1 with this breaker: skipped for type 2 → KM-25.
        assert_eq!(p.contactor.id, "KM-25");
        assert!(
            p.rejected
                .iter()
                .any(|r| r.contains("tested to type 1 only"))
        );
        assert!(!p.unresolved.iter().any(|u| u.contains("coordination")));
    }

    #[test]
    fn heavy_starts_need_a_slower_class_and_a_bigger_fuse() {
        let net = board(15.0);
        let cat = maker_catalog();
        let tables = TableSet::builtin();
        // 25 s start → class 30 needed; the catalog has class 10 only.
        let m = MotorSpec {
            rated_current_a: Some(11.0),
            start_time_s: Some(25.0),
            ..motor(5.5)
        };
        let p = size_starter(&SizingInput {
            motor: &m,
            scheme: Scheme::FuseContactorRelay,
            coordination: Coordination::Type1,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(20.0),
            name: "M4",
        })
        .unwrap();
        assert!(p.overload.why.contains("requirement"), "{:?}", p.overload);
        assert!(p.unresolved.iter().any(|u| u.contains("class 30")));
        // Fuse factor 2.5 · 11 = 27.5 → 32 A.
        assert_eq!(p.short_circuit.id, "gG fuse 32 A");
        // Beyond 30 s there is no answer here.
        let m = MotorSpec {
            start_time_s: Some(45.0),
            ..m
        };
        let e = size_starter(&SizingInput {
            motor: &m,
            scheme: Scheme::FuseContactorRelay,
            coordination: Coordination::Type1,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(20.0),
            name: "M4",
        })
        .unwrap_err();
        assert!(e.reasons[0].contains("soft starter"));
    }

    #[test]
    fn no_answer_names_the_failing_checks() {
        // A 22 kW motor 400 m away on a weak supply: no section clears it.
        let net = board(3.0);
        let cat = Catalog::generic();
        let tables = TableSet::builtin();
        let e = size_starter(&SizingInput {
            motor: &motor(22.0),
            scheme: Scheme::FuseContactorRelay,
            coordination: Coordination::Type1,
            network: &net,
            at: net.root(),
            catalog: &cat,
            tables: &tables,
            pairs: None,
            route: route(2000.0),
            name: "M5",
        })
        .unwrap_err();
        assert!(e.reasons[0].contains("no listed cable section"));
        assert!(e.reasons.len() > 1);
    }

    #[test]
    fn efficiency_table_is_monotone() {
        let mut last = 0.0;
        for p in [0.75, 1.5, 4.0, 11.0, 30.0, 90.0, 200.0, 400.0] {
            let e = typical_efficiency(p);
            assert!(e >= last);
            last = e;
        }
        assert_eq!(required_class(8.0), Some(RelayClass::C10));
        assert_eq!(required_class(15.0), Some(RelayClass::C20));
        assert_eq!(required_class(31.0), None);
    }
}
