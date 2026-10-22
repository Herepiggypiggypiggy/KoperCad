//! The rule catalogue and the engine that runs it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::class::{EmcClass, Termination};
use crate::geometry::{self, Piece};
use crate::input::{Device, FilterState, Finish, Input, Origin, P, Role, Run};
use crate::report::*;

/// One rule of the catalogue (`docs/EMC.md` section 5).
#[derive(Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub title: &'static str,
    pub level: Level,
    /// What the rule asks of the design, in our own words.
    pub requirement: &'static str,
    /// Where the practice is written down. Unverified: nobody has read a
    /// licensed copy; check before quoting.
    pub source: &'static str,
    /// The clause or table numbers behind `source` are still to be verified
    /// by a person with the licensed text.
    pub verify: bool,
    /// What it reads.
    pub inputs: &'static str,
}

pub static RULES: &[Rule] = &[
    Rule {
        id: "EMC-01",
        title: "Classes apart in ducts and cables",
        level: Level::Fact,
        requirement: "Power or motor wires and signal wires do not share a duct or a cable.",
        source: "Cabling practice (IEC 61000-5-2, EN 50174-2); clause numbers not verified",
        verify: true,
        inputs: "Routed paths and wire classes; cable cores",
    },
    Rule {
        id: "EMC-02",
        title: "Distance along parallel runs",
        level: Level::Index,
        requirement: "Where an aggressor and a victim run parallel they keep the planning distance for their classes, less credit for shields and twisting.",
        source: "Cabling practice; planning values, not verified",
        verify: true,
        inputs: "Routed paths, wire classes, shields",
    },
    Rule {
        id: "EMC-04",
        title: "Motor cable shielded and terminated",
        level: Level::Fact,
        requirement: "The motor cable of a drive is shielded, the shield bonded all round at both ends.",
        source: "Drive makers' installation guides; EN 61800-3 context, not verified",
        verify: true,
        inputs: "Motor wires, cable shield, shield ends (`emc.shield_a`, `emc.shield_b`)",
    },
    Rule {
        id: "EMC-05",
        title: "Shield pigtail short",
        level: Level::Estimate,
        requirement: "A shield is led away on a pigtail shorter than the limit; the impedance at the stated frequency is shown.",
        source: "Rule of thumb of 10 nH per cm; not verified",
        verify: true,
        inputs: "Shield ends (`emc.shield_a`, `emc.shield_b`: `360`, `pigtail:<mm>`, `open`)",
    },
    Rule {
        id: "EMC-07",
        title: "Drive input filter",
        level: Level::Index,
        requirement: "A drive has a mains filter (built in, or fitted close by on the same plate).",
        source: "Drive makers' guides; EN 61800-3 context, not verified",
        verify: true,
        inputs: "Drive devices, filter devices and their positions, `emc.filter` on the drive or its part",
    },
    Rule {
        id: "EMC-09",
        title: "Loop area",
        level: Level::Index,
        requirement: "Go and return of a circuit run together: the area between them stays under the limit.",
        source: "Cabling practice; house value, not verified",
        verify: true,
        inputs: "Paths of both conductors between the same two devices",
    },
    Rule {
        id: "EMC-11",
        title: "Plate bonding",
        level: Level::Fact,
        requirement: "Drives and filters sit on a bare, bonded surface: not on paint.",
        source: "Drive and filter makers' guides; not verified",
        verify: true,
        inputs: "`emc.finish` on the enclosure (`galvanised`, `bare`, `painted`), `emc.bonded` on the device",
    },
    Rule {
        id: "EMC-14",
        title: "Signal cables leaving the cabinet",
        level: Level::Fact,
        requirement: "Analog and fieldbus wiring that leaves the cabinet is shielded.",
        source: "Cabling practice; not verified",
        verify: true,
        inputs: "Wires with an end outside the enclosure, cable shields",
    },
    Rule {
        id: "EMC-17",
        title: "Sensitive device away from a drive",
        level: Level::Index,
        requirement: "A PLC or analog module keeps the planning distance from a drive or other noisy device.",
        source: "House value; not verified",
        verify: true,
        inputs: "Device positions on the plate",
    },
];

pub fn rule(id: &str) -> &'static Rule {
    RULES.iter().find(|r| r.id == id).expect("known rule")
}

struct Ctx<'a> {
    input: &'a Input,
    names: HashMap<u64, &'a str>,
    devices: HashMap<u64, &'a Device>,
    plates: HashMap<u64, &'a str>,
}

impl<'a> Ctx<'a> {
    fn run(&self, id: u64) -> String {
        self.names
            .get(&id)
            .map_or_else(|| format!("wire {id}"), |n| (*n).to_owned())
    }
    fn dev(&self, id: u64) -> String {
        self.devices
            .get(&id)
            .map_or_else(|| format!("device {id}"), |d| d.tag.clone())
    }
    fn plate(&self, id: u64) -> String {
        self.plates
            .get(&id)
            .map_or_else(|| "a plate".to_owned(), |p| (*p).to_owned())
    }
}

fn mm(v: f64) -> String {
    format!("{} mm", v.round() as i64)
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Runs every rule.
pub fn analyse(input: &Input) -> Report {
    let cx = Ctx {
        input,
        names: input.runs.iter().map(|r| (r.id, r.name.as_str())).collect(),
        devices: input.devices.iter().map(|d| (d.id, d)).collect(),
        plates: input
            .plates
            .iter()
            .map(|p| (p.id, p.tag.as_str()))
            .collect(),
    };
    let mut findings = Vec::new();
    let mut status: Vec<RuleStatus> = Vec::new();
    let note = |status: &mut Vec<RuleStatus>,
                rule: &'static str,
                found: usize,
                applies: bool,
                text: &str,
                missing: bool| {
        let (st, note) = if !applies {
            (Status::NotApplicable, text.to_owned())
        } else if missing {
            (Status::NotEvaluated, text.to_owned())
        } else if found > 0 {
            (Status::Finding, text.to_owned())
        } else {
            (Status::NoFinding, text.to_owned())
        };
        status.push(RuleStatus {
            rule,
            status: st,
            findings: found,
            note,
        });
    };

    let before = findings.len();
    let routed = input.runs.iter().filter(|r| r.path.len() >= 2).count();
    segregation(&cx, &mut findings);
    let seg1 = findings.iter().filter(|f| f.rule == "EMC-01").count();
    let seg2 = findings.iter().filter(|f| f.rule == "EMC-02").count();
    let classes: BTreeSet<EmcClass> = input.runs.iter().map(|r| r.class).collect();
    let mixed_classes = classes.iter().any(|a| {
        classes
            .iter()
            .any(|b| input.params.distance_mm(*a, *b) > 0.0)
    });
    let _ = before;
    note(
        &mut status,
        "EMC-01",
        seg1,
        mixed_classes,
        if routed == 0 {
            "No wire has a route on a plate yet: cable mixing was checked, duct sharing was not"
        } else {
            "Duct sharing and cable mixing"
        },
        false,
    );
    note(
        &mut status,
        "EMC-02",
        seg2,
        mixed_classes,
        if routed == 0 {
            "No wire has a route on a plate yet"
        } else {
            "Parallel stretches of different classes"
        },
        routed == 0 && mixed_classes,
    );

    let n = findings.len();
    motor_cables(&cx, &mut findings);
    let motor = input.runs.iter().any(|r| r.class == EmcClass::Motor);
    note(
        &mut status,
        "EMC-04",
        findings.len() - n,
        motor,
        "Motor wires and their cables",
        false,
    );

    let n = findings.len();
    pigtails(&cx, &mut findings);
    let shielded = input.runs.iter().any(|r| r.shield.is_some());
    let unknown_ends = input
        .runs
        .iter()
        .filter_map(|r| r.shield)
        .any(|s| s.a == Termination::Unknown || s.b == Termination::Unknown);
    note(
        &mut status,
        "EMC-05",
        findings.len() - n,
        shielded,
        if input.params.frequency_hz > BAND_HZ.1 {
            "No model above 30 MHz: the estimate is not given"
        } else if unknown_ends {
            "Some shield ends are not stated (emc.shield_a / emc.shield_b)"
        } else {
            "Shield ends"
        },
        (unknown_ends && findings.len() == n) || input.params.frequency_hz > BAND_HZ.1,
    );

    let n = findings.len();
    drive_filters(&cx, &mut findings);
    let drives = input.devices.iter().any(|d| d.role == Role::Drive);
    note(
        &mut status,
        "EMC-07",
        findings.len() - n,
        drives,
        "Drives and filters",
        false,
    );

    let n = findings.len();
    loops(&cx, &mut findings);
    note(
        &mut status,
        "EMC-09",
        findings.len() - n,
        routed > 0,
        "Go and return pairs between the same two devices",
        false,
    );

    let n = findings.len();
    bonding(&cx, &mut findings);
    let plate_unknown = input.devices.iter().any(|d| {
        matches!(d.role, Role::Drive | Role::Filter)
            && d.plate
                .is_some_and(|p| input.plates.iter().any(|q| q.id == p && q.finish.is_none()))
    });
    note(
        &mut status,
        "EMC-11",
        findings.len() - n,
        input
            .devices
            .iter()
            .any(|d| matches!(d.role, Role::Drive | Role::Filter)),
        if plate_unknown {
            "The plate finish is not stated (emc.finish)"
        } else {
            "Plate finish under drives and filters"
        },
        plate_unknown && findings.len() == n,
    );

    let n = findings.len();
    leaving(&cx, &mut findings);
    note(
        &mut status,
        "EMC-14",
        findings.len() - n,
        input.runs.iter().any(|r| r.leaves_cabinet),
        "Wires leaving the cabinet",
        false,
    );

    let n = findings.len();
    sensitive(&cx, &mut findings);
    note(
        &mut status,
        "EMC-17",
        findings.len() - n,
        input.devices.iter().any(|d| d.role == Role::Sensitive),
        "Sensitive devices against drives",
        false,
    );

    // A fixed order: the worst first, then by rule and subject.
    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(
                b.points
                    .iter()
                    .map(|p| p.1)
                    .max()
                    .cmp(&a.points.iter().map(|p| p.1).max()),
            )
            .then(a.rule.cmp(b.rule))
            .then(a.subjects.cmp(&b.subjects))
    });

    let mut report = Report {
        findings,
        rules: status,
        ..Report::default()
    };
    scores(&cx, &mut report);
    report.assumptions = assumptions(input);
    report
}

fn assumptions(input: &Input) -> Vec<Assumption> {
    let p = &input.params;
    let a = |name: &'static str, property: &'static str, value: String| Assumption {
        name,
        value,
        user_set: p.user_set.contains(&property.trim_start_matches("emc.")),
        property,
    };
    vec![
        a(
            "Shortest parallel stretch reported",
            "emc.min_parallel_mm",
            mm(p.min_parallel_mm),
        ),
        a(
            "Height of the wires above the plate (the plate carries the return)",
            "emc.wire_height_mm",
            mm(p.wire_height_mm),
        ),
        a(
            "Current step assumed in an aggressor",
            "emc.didt_a_per_us",
            format!("{} A/us", p.didt_a_per_us),
        ),
        a(
            "Frequency of the shield pigtail impedance",
            "emc.frequency_hz",
            format!(
                "{} MHz (estimates hold up to {} MHz)",
                p.frequency_hz / 1.0e6,
                BAND_HZ.1 / 1.0e6
            ),
        ),
        a(
            "Largest loop between go and return",
            "emc.max_loop_cm2",
            format!("{} cm2", p.max_loop_cm2),
        ),
        a("Filter within", "emc.filter_max_mm", mm(p.filter_max_mm)),
        a(
            "Sensitive device away from a drive",
            "emc.sensitive_min_mm",
            mm(p.sensitive_min_mm),
        ),
        a(
            "Longest shield pigtail",
            "emc.pigtail_max_mm",
            mm(p.pigtail_max_mm),
        ),
    ]
}

// ---- EMC-01 and EMC-02 -----------------------------------------------------

fn victim_first(a: &Run, b: &Run) -> (usize, usize) {
    // Returns indices (aggressor, victim) into [a, b].
    if a.class.aggression() >= b.class.aggression() {
        (0, 1)
    } else {
        (1, 0)
    }
}

fn shield_factor(r: &Run) -> f64 {
    let s = r.shield.map_or(1.0, |s| s.factor());
    if r.twisted { s * 0.8 } else { s }
}

fn segregation(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let input = cx.input;
    let p = &input.params;

    // Runs sharing a cable.
    let mut by_cable: BTreeMap<u64, Vec<&Run>> = BTreeMap::new();
    for r in &input.runs {
        if let Some(c) = r.cable {
            by_cable.entry(c).or_default().push(r);
        }
    }
    for (cable, runs) in &by_cable {
        let mut worst: Option<(f64, &Run, &Run)> = None;
        for (i, a) in runs.iter().enumerate() {
            for b in &runs[i + 1..] {
                let need = p.distance_mm(a.class, b.class);
                if a.class != b.class && need > 0.0 && worst.is_none_or(|w| need > w.0) {
                    worst = Some((need, a, b));
                }
            }
        }
        let Some((_, a, b)) = worst else { continue };
        let (ia, iv) = victim_first(a, b);
        let pair = [a, b];
        let (agg, vic) = (pair[ia], pair[iv]);
        let safety = (agg.class.is_aggressor() && vic.class == EmcClass::Safety)
            || (vic.class.is_aggressor() && agg.class == EmcClass::Safety);
        let w = (agg.class.aggression() * vic.class.sensitivity()).sqrt();
        let pts = (60.0 + 40.0 * w).round() as u32;
        out.push(Finding {
            rule: "EMC-01",
            level: Level::Fact,
            severity: if safety { Severity::Error } else { Severity::Warning },
            subjects: vec![Subject::Cable(*cable), Subject::Run(vic.id), Subject::Run(agg.id)],
            points: vec![(Subject::Run(vic.id), pts), (Subject::Run(agg.id), pts / 2)],
            message: format!(
                "{} ({}) and {} ({}) share one cable: classes that need {} between them cannot be in the same sheath.",
                cx.run(vic.id),
                vic.class.label(),
                cx.run(agg.id),
                agg.class.label(),
                mm(p.distance_mm(a.class, b.class))
            ),
            metrics: Vec::new(),
            fixes: vec![Fix::SeparateRuns { a: agg.id, b: vic.id, to_mm: p.distance_mm(a.class, b.class) }],
            at: None,
            ribbons: Vec::new(),
            loop_polygon: Vec::new(),
            band_hz: None,
        });
    }

    // Parallel stretches on the plate.
    let mut by_plate: BTreeMap<u64, Vec<&Run>> = BTreeMap::new();
    for r in input.runs.iter().filter(|r| r.path.len() >= 2) {
        if let Some(pl) = r.plate {
            by_plate.entry(pl).or_default().push(r);
        }
    }
    for (plate, runs) in by_plate {
        let cache: Vec<Vec<Piece>> = runs
            .iter()
            .map(|r| geometry::pieces(r, &input.ducts))
            .collect();
        for i in 0..runs.len() {
            for j in i + 1..runs.len() {
                let (a, b) = (runs[i], runs[j]);
                if a.class == b.class || (a.cable.is_some() && a.cable == b.cable) {
                    continue;
                }
                let base = p.distance_mm(a.class, b.class);
                if base <= 0.0 {
                    continue;
                }
                let need = base * shield_factor(a) * shield_factor(b);
                let stretches = geometry::parallel(&cache[i], &cache[j], &input.ducts, need);
                let mut duct: Vec<geometry::Stretch> = Vec::new();
                let mut near: Vec<geometry::Stretch> = Vec::new();
                for mut s in stretches {
                    if s.divider {
                        s.distance_mm = 50.0;
                    }
                    if s.distance_mm >= need {
                        continue;
                    }
                    if s.duct.is_some() && !s.divider {
                        duct.push(s);
                    } else {
                        near.push(s);
                    }
                }
                for (group, rule_id) in [(duct, "EMC-01"), (near, "EMC-02")] {
                    let length: f64 = group.iter().map(|s| s.length_mm).sum();
                    if group.is_empty() || length < p.min_parallel_mm {
                        continue;
                    }
                    let mean_d = group
                        .iter()
                        .map(|s| s.distance_mm * s.length_mm)
                        .sum::<f64>()
                        / length;
                    let min_d = group.iter().map(|s| s.distance_mm).fold(f64::MAX, f64::min);
                    let (ia, iv) = victim_first(a, b);
                    let pair = [a, b];
                    let (agg, vic) = (pair[ia], pair[iv]);
                    let w = (agg.class.aggression() * vic.class.sensitivity()).sqrt();
                    let s_short = clamp01((need - mean_d) / need);
                    let lf = clamp01(length / 1000.0);
                    let pts =
                        (100.0 * w * (0.25 + 0.75 * s_short) * (0.3 + 0.7 * lf)).round() as u32;
                    if pts == 0 {
                        continue;
                    }
                    let mut band_hz = None;
                    let mut metrics = vec![
                        Metric {
                            label: "Parallel length",
                            value: mm(length),
                            level: Level::Index,
                        },
                        Metric {
                            label: "Closest distance",
                            value: mm(min_d),
                            level: Level::Index,
                        },
                        Metric {
                            label: "Planning distance",
                            value: mm(need),
                            level: Level::Index,
                        },
                    ];
                    // Level 2: induced voltage into the victim over the stretch.
                    if agg.class.is_aggressor() {
                        let volts: f64 = group
                            .iter()
                            .map(|s| {
                                geometry::mutual_per_m(
                                    p.wire_height_mm,
                                    s.centre_mm,
                                    vic.diameter_mm.max(agg.diameter_mm),
                                ) * (s.length_mm / 1000.0)
                                    * p.didt_a_per_us
                                    * 1.0e6
                            })
                            .sum();
                        metrics.push(Metric {
                            label: "Induced voltage (estimate)",
                            value: format!(
                                "{volts:.1} V for {} A/us, within a factor of 3",
                                p.didt_a_per_us
                            ),
                            level: Level::Estimate,
                        });
                        band_hz = Some(BAND_HZ);
                    }
                    let ribbons: Vec<Ribbon> = group
                        .iter()
                        .map(|s| Ribbon {
                            a: s.on_a,
                            b: s.on_b,
                            length_mm: s.length_mm,
                            distance_mm: s.distance_mm,
                        })
                        .collect();
                    let in_duct = group.first().and_then(|s| s.duct);
                    let message = match (rule_id, in_duct) {
                        ("EMC-01", Some(_)) => format!(
                            "{} ({}) runs in the same duct as {} ({}) for {}: these classes need {} between them.",
                            cx.run(vic.id),
                            vic.class.label(),
                            cx.run(agg.id),
                            agg.class.label(),
                            mm(length),
                            mm(need)
                        ),
                        _ => format!(
                            "{} ({}) runs parallel to {} ({}) for {} at {} (mean {}); the planning distance is {}.",
                            cx.run(vic.id),
                            vic.class.label(),
                            cx.run(agg.id),
                            agg.class.label(),
                            mm(length),
                            mm(min_d),
                            mm(mean_d),
                            mm(need)
                        ),
                    };
                    let mut fixes = vec![Fix::SeparateRuns {
                        a: agg.id,
                        b: vic.id,
                        to_mm: need.ceil(),
                    }];
                    if let (Some(c), None) = (vic.cable, vic.shield) {
                        fixes.push(Fix::AddShield { cable: c });
                    }
                    for r in [vic, agg] {
                        if r.origin == Origin::Guessed {
                            fixes.push(Fix::DeclareClass {
                                wire: r.id,
                                class: r.class,
                            });
                        }
                    }
                    let spot = group.first().map(|_| plate);
                    out.push(Finding {
                        rule: rule_id,
                        level: rule(rule_id).level,
                        severity: if pts >= 25 {
                            Severity::Warning
                        } else {
                            Severity::Info
                        },
                        subjects: vec![Subject::Run(vic.id), Subject::Run(agg.id)],
                        points: vec![(Subject::Run(vic.id), pts), (Subject::Run(agg.id), pts / 2)],
                        message,
                        metrics,
                        fixes,
                        at: spot.and_then(|pl| vic.path.first().map(|p| (pl, *p))),
                        ribbons,
                        loop_polygon: Vec::new(),
                        band_hz,
                    });
                }
            }
        }
    }
}

// ---- EMC-04 ----------------------------------------------------------------

fn motor_cables(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let mut done: BTreeSet<(u8, u64)> = BTreeSet::new();
    for r in cx.input.runs.iter().filter(|r| r.class == EmcClass::Motor) {
        let key = match (r.cable, r.drive) {
            (Some(c), _) => (0, c),
            (None, Some(d)) => (1, d),
            (None, None) => (2, r.id),
        };
        if !done.insert(key) {
            continue;
        }
        let drive = r.drive.map_or_else(|| "a drive".to_owned(), |d| cx.dev(d));
        let subject = r.cable.map_or(Subject::Run(r.id), Subject::Cable);
        let mut subjects = vec![subject];
        if let Some(d) = r.drive {
            subjects.push(Subject::Device(d));
        }
        match (r.cable, r.shield) {
            (_, None) => {
                let mut fixes = Vec::new();
                if let Some(c) = r.cable {
                    fixes.push(Fix::AddShield { cable: c });
                }
                out.push(Finding {
                    rule: "EMC-04",
                    level: Level::Fact,
                    severity: Severity::Warning,
                    subjects,
                    points: vec![(Subject::Run(r.id), 50)],
                    message: if r.cable.is_some() {
                        format!("The motor cable of {drive} has no shield: its steep edges leave the cabinet as noise.")
                    } else {
                        format!("The motor wires of {drive} are single wires, not a shielded cable: put them in a shielded motor cable.")
                    },
                    metrics: Vec::new(),
                    fixes,
                    at: None,
                    ribbons: Vec::new(),
                    loop_polygon: Vec::new(),
                    band_hz: None,
                });
            }
            (Some(c), Some(s)) if !s.is_full() => {
                let open = matches!(s.a, Termination::Open) || matches!(s.b, Termination::Open);
                let pig = matches!(s.a, Termination::Pigtail(_))
                    || matches!(s.b, Termination::Pigtail(_));
                let unknown =
                    matches!(s.a, Termination::Unknown) || matches!(s.b, Termination::Unknown);
                let (sev, pts, why) = if open {
                    (Severity::Warning, 35, "a shield end is not connected")
                } else if pig {
                    (
                        Severity::Warning,
                        25,
                        "a shield end is a pigtail, not all round",
                    )
                } else if unknown {
                    (Severity::Info, 10, "the shield ends are not stated")
                } else {
                    (
                        Severity::Warning,
                        25,
                        "the shield is not bonded all round at both ends",
                    )
                };
                out.push(Finding {
                    rule: "EMC-04",
                    level: Level::Fact,
                    severity: sev,
                    subjects,
                    points: vec![(Subject::Run(r.id), pts)],
                    message: format!("The motor cable of {drive}: {why}. A shield only works carrying current, from drive to motor frame."),
                    metrics: Vec::new(),
                    fixes: vec![Fix::Terminate360 { cable: c }],
                    at: None,
                    ribbons: Vec::new(),
                    loop_polygon: Vec::new(),
                    band_hz: None,
                });
            }
            _ => {}
        }
    }
}

// ---- EMC-05 ----------------------------------------------------------------

fn pigtails(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let p = &cx.input.params;
    if p.frequency_hz > BAND_HZ.1 {
        return; // No number outside the band.
    }
    let mut done: BTreeSet<u64> = BTreeSet::new();
    for r in cx.input.runs.iter() {
        let (Some(shield), Some(cable)) = (r.shield, r.cable) else {
            continue;
        };
        if !done.insert(cable) {
            continue;
        }
        for (end, t) in [("first", shield.a), ("second", shield.b)] {
            let Termination::Pigtail(len) = t else {
                continue;
            };
            if len <= p.pigtail_max_mm {
                continue;
            }
            let z = geometry::pigtail_ohm(len, p.frequency_hz);
            out.push(Finding {
                rule: "EMC-05",
                level: Level::Estimate,
                severity: Severity::Warning,
                subjects: vec![Subject::Cable(cable), Subject::Run(r.id)],
                points: vec![(Subject::Run(r.id), (15.0 + (len / p.pigtail_max_mm).min(4.0) * 5.0).round() as u32)],
                message: format!(
                    "The shield of the cable of {} ends in a {} pigtail at its {end} end: about {} nH, {z:.1} ohm at {} MHz, so most of the shield is lost there.",
                    cx.run(r.id),
                    mm(len),
                    geometry::pigtail_nh(len).round() as i64,
                    p.frequency_hz / 1.0e6
                ),
                metrics: vec![
                    Metric { label: "Pigtail", value: mm(len), level: Level::Index },
                    Metric { label: "Impedance (estimate)", value: format!("{z:.1} ohm at {} MHz, rule of thumb 10 nH/cm", p.frequency_hz / 1.0e6), level: Level::Estimate },
                ],
                fixes: vec![Fix::Terminate360 { cable }, Fix::ShortenPigtail { cable, to_mm: p.pigtail_max_mm }],
                at: None,
                ribbons: Vec::new(),
                loop_polygon: Vec::new(),
                band_hz: Some(BAND_HZ),
            });
        }
    }
}

// ---- EMC-07 ----------------------------------------------------------------

fn drive_filters(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let p = &cx.input.params;
    for d in cx.input.devices.iter().filter(|d| d.role == Role::Drive) {
        if d.filter == FilterState::Built || d.filter == FilterState::Declared {
            continue;
        }
        let filters: Vec<&Device> = cx
            .input
            .devices
            .iter()
            .filter(|f| {
                f.role == Role::Filter
                    && (f.filter_for.as_deref() == Some(d.tag.as_str())
                        || (f.filter_for.is_none() && f.plate == d.plate))
            })
            .collect();
        if filters.is_empty() {
            out.push(Finding {
                rule: "EMC-07",
                level: Level::Fact,
                severity: Severity::Warning,
                subjects: vec![Subject::Device(d.id)],
                points: vec![(Subject::Device(d.id), 30)],
                message: format!(
                    "{} has no mains filter. If its part has one built in, say so (emc.filter = built-in).",
                    cx.dev(d.id)
                ),
                metrics: Vec::new(),
                fixes: vec![Fix::AddFilter { drive: d.id }],
                at: d.plate.zip(d.rect).map(|(pl, r)| (pl, (r.0, r.1))),
                ribbons: Vec::new(),
                loop_polygon: Vec::new(),
                band_hz: None,
            });
            continue;
        }
        let best = filters
            .iter()
            .filter_map(|f| Some((f, geometry::gap_mm(d.rect?, f.rect?))))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((f, gap)) = best
            && gap > p.filter_max_mm
        {
            out.push(Finding {
                rule: "EMC-07",
                level: Level::Index,
                severity: Severity::Warning,
                subjects: vec![Subject::Device(d.id), Subject::Device(f.id)],
                points: vec![(Subject::Device(d.id), 20), (Subject::Device(f.id), 10)],
                message: format!(
                    "The filter {} sits {} from {}: the unfiltered wire between them radiates; keep it under {}.",
                    cx.dev(f.id),
                    mm(gap),
                    cx.dev(d.id),
                    mm(p.filter_max_mm)
                ),
                metrics: vec![Metric { label: "Filter to drive", value: mm(gap), level: Level::Index }],
                fixes: vec![Fix::MoveDevice { device: f.id, from: d.id, to_mm: p.filter_max_mm }],
                at: d.plate.zip(f.rect).map(|(pl, r)| (pl, (r.0, r.1))),
                ribbons: Vec::new(),
                loop_polygon: Vec::new(),
                band_hz: None,
            });
        }
    }
}

// ---- EMC-09 ----------------------------------------------------------------

fn loops(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let p = &cx.input.params;
    let mut groups: BTreeMap<(EmcClass, u64, u64), Vec<&Run>> = BTreeMap::new();
    for r in cx.input.runs.iter().filter(|r| r.path.len() >= 2) {
        if !matches!(
            r.class,
            EmcClass::Control | EmcClass::Analog | EmcClass::Fieldbus | EmcClass::Safety
        ) {
            continue;
        }
        let (Some(a), Some(b)) = (r.from, r.to) else {
            continue;
        };
        if a == b {
            continue;
        }
        groups
            .entry((r.class, a.min(b), a.max(b)))
            .or_default()
            .push(r);
    }
    for ((class, ..), runs) in groups {
        let [a, b] = runs[..] else { continue };
        if a.plate != b.plate || (a.cable.is_some() && a.cable == b.cable) {
            continue;
        }
        let area = geometry::loop_area_cm2(&a.path, &b.path);
        if area <= p.max_loop_cm2 {
            continue;
        }
        let pts = (40.0 * (area / (2.0 * p.max_loop_cm2)).min(1.0))
            .round()
            .max(10.0) as u32;
        out.push(Finding {
            rule: "EMC-09",
            level: Level::Index,
            severity: Severity::Warning,
            subjects: vec![Subject::Run(a.id), Subject::Run(b.id)],
            points: vec![(Subject::Run(a.id), pts), (Subject::Run(b.id), pts)],
            message: format!(
                "{} and {} ({}) enclose {area:.0} cm2 between their two ends: the loop picks up and radiates; run go and return together or in one twisted cable (limit {:.0} cm2).",
                cx.run(a.id),
                cx.run(b.id),
                class.label(),
                p.max_loop_cm2
            ),
            metrics: vec![Metric { label: "Loop area", value: format!("{area:.0} cm2"), level: Level::Index }],
            fixes: vec![Fix::SeparateRuns { a: a.id, b: b.id, to_mm: 0.0 }],
            at: a.plate.zip(a.path.first().copied()),
            ribbons: Vec::new(),
            loop_polygon: loop_polygon(&a.path, &b.path),
            band_hz: None,
        });
    }
}

fn loop_polygon(go: &[P], back: &[P]) -> Vec<P> {
    let d = |p: P, q: P| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
    let (Some(g0), Some(g1), Some(b0), Some(b1)) =
        (go.first(), go.last(), back.first(), back.last())
    else {
        return Vec::new();
    };
    let flip = d(*g0, *b0) + d(*g1, *b1) > d(*g0, *b1) + d(*g1, *b0);
    let mut poly = go.to_vec();
    if flip {
        poly.extend(back.iter().copied());
    } else {
        poly.extend(back.iter().rev().copied());
    }
    poly
}

// ---- EMC-11 ----------------------------------------------------------------

fn bonding(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    for d in cx
        .input
        .devices
        .iter()
        .filter(|d| matches!(d.role, Role::Drive | Role::Filter))
    {
        let Some(plate) = d.plate else { continue };
        let finish = cx
            .input
            .plates
            .iter()
            .find(|p| p.id == plate)
            .and_then(|p| p.finish);
        let painted = finish == Some(Finish::Painted);
        if painted || d.bonded == Some(false) {
            out.push(Finding {
                rule: "EMC-11",
                level: Level::Fact,
                severity: Severity::Warning,
                subjects: vec![Subject::Device(d.id), Subject::Plate(plate)],
                points: vec![(Subject::Device(d.id), 40), (Subject::Plate(plate), 20)],
                message: if painted {
                    format!("{} sits on a painted plate ({}): the case does not bond to it at high frequency; scrape the mounting area bare or use a galvanised plate.", cx.dev(d.id), cx.plate(plate))
                } else {
                    format!("{} is stated as not bonded to the plate: a filter or drive case must make metal contact over its area.", cx.dev(d.id))
                },
                metrics: Vec::new(),
                fixes: vec![Fix::BarePlate { plate }],
                at: d.rect.map(|r| (plate, (r.0, r.1))),
                ribbons: Vec::new(),
                loop_polygon: Vec::new(),
                band_hz: None,
            });
        }
    }
}

// ---- EMC-14 ----------------------------------------------------------------

fn leaving(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let mut done: BTreeSet<(u8, u64)> = BTreeSet::new();
    for r in
        cx.input.runs.iter().filter(|r| {
            r.leaves_cabinet && matches!(r.class, EmcClass::Analog | EmcClass::Fieldbus)
        })
    {
        if r.shield.is_some() {
            continue;
        }
        let key = r.cable.map_or((1, r.id), |c| (0, c));
        if !done.insert(key) {
            continue;
        }
        out.push(Finding {
            rule: "EMC-14",
            level: Level::Fact,
            severity: Severity::Warning,
            subjects: vec![r.cable.map_or(Subject::Run(r.id), Subject::Cable), Subject::Run(r.id)],
            points: vec![(Subject::Run(r.id), 35)],
            message: format!("{} ({}) leaves the cabinet without a shield: it carries the field outside in and the cabinet's noise out.", cx.run(r.id), r.class.label()),
            metrics: Vec::new(),
            fixes: r.cable.map(|c| Fix::AddShield { cable: c }).into_iter().collect(),
            at: None,
            ribbons: Vec::new(),
            loop_polygon: Vec::new(),
            band_hz: None,
        });
    }
}

// ---- EMC-17 ----------------------------------------------------------------

fn sensitive(cx: &Ctx<'_>, out: &mut Vec<Finding>) {
    let p = &cx.input.params;
    for s in cx
        .input
        .devices
        .iter()
        .filter(|d| d.role == Role::Sensitive)
    {
        for n in cx
            .input
            .devices
            .iter()
            .filter(|d| matches!(d.role, Role::Drive | Role::Noisy))
        {
            if s.plate.is_none() || s.plate != n.plate {
                continue;
            }
            let (Some(a), Some(b)) = (s.rect, n.rect) else {
                continue;
            };
            let gap = geometry::gap_mm(a, b);
            if gap >= p.sensitive_min_mm {
                continue;
            }
            let pts = (30.0 + 20.0 * clamp01((p.sensitive_min_mm - gap) / p.sensitive_min_mm))
                .round() as u32;
            out.push(Finding {
                rule: "EMC-17",
                level: Level::Index,
                severity: Severity::Warning,
                subjects: vec![Subject::Device(s.id), Subject::Device(n.id)],
                points: vec![
                    (Subject::Device(s.id), pts),
                    (Subject::Device(n.id), pts / 2),
                ],
                message: format!(
                    "{} sits {} from {}: keep sensitive devices at least {} away from a drive.",
                    cx.dev(s.id),
                    mm(gap),
                    cx.dev(n.id),
                    mm(p.sensitive_min_mm)
                ),
                metrics: vec![Metric {
                    label: "Gap",
                    value: mm(gap),
                    level: Level::Index,
                }],
                fixes: vec![Fix::MoveDevice {
                    device: s.id,
                    from: n.id,
                    to_mm: p.sensitive_min_mm,
                }],
                at: s.plate.map(|pl| (pl, (a.0, a.1))),
                ribbons: Vec::new(),
                loop_polygon: Vec::new(),
                band_hz: None,
            });
        }
    }
}

// ---- scores ----------------------------------------------------------------

fn scores(cx: &Ctx<'_>, report: &mut Report) {
    let mut by: BTreeMap<Subject, Vec<Reason>> = BTreeMap::new();
    for f in &report.findings {
        for (subject, pts) in &f.points {
            if *pts > 0 {
                by.entry(*subject).or_default().push(Reason {
                    rule: f.rule,
                    points: *pts,
                    text: f.message.clone(),
                });
            }
        }
    }
    let make = |subject: Subject, reasons: Vec<Reason>| {
        let sum: u32 = reasons.iter().map(|r| r.points).sum();
        let points = sum.min(100);
        Score {
            subject,
            points,
            band: Band::of(points),
            reasons,
        }
    };
    // Every run and device gets a score, so a clean one reads "No findings".
    let mut runs = Vec::new();
    for r in &cx.input.runs {
        let s = Subject::Run(r.id);
        runs.push(make(s, by.remove(&s).unwrap_or_default()));
    }
    let mut devices = Vec::new();
    for d in &cx.input.devices {
        if matches!(d.role, Role::Other) {
            continue;
        }
        let s = Subject::Device(d.id);
        devices.push(make(s, by.remove(&s).unwrap_or_default()));
    }
    let mut plates = Vec::new();
    for p in &cx.input.plates {
        let s = Subject::Plate(p.id);
        plates.push(make(s, by.remove(&s).unwrap_or_default()));
    }
    let mut nets: BTreeMap<u32, Score> = BTreeMap::new();
    for (r, sc) in cx.input.runs.iter().zip(&runs) {
        if let Some(n) = r.net {
            let e = nets.entry(n).or_insert_with(|| sc.clone());
            if sc.points > e.points {
                *e = sc.clone();
            }
        }
    }
    report.runs = runs;
    report.devices = devices;
    report.plates = plates;
    report.nets = nets.into_iter().collect();
}
