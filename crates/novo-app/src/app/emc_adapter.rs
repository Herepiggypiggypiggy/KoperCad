//! The rule engine's picture in the shape the EMC views draw: classes,
//! findings, loops and shield ends come from `novo-emc`, not from the guesses
//! of `emc_stub`. The views stay free of physics; this file only renames and
//! arranges.
//!
//! Frame: the engine's route points are plate millimetres, the same frame as
//! `PlateFrame::plate`, so nothing is converted. Route points sit at z = 0;
//! device bodies take the engine's stated depth, else a default height.

use std::collections::HashSet;

use novo_emc::report::{Fix as EngineFix, Subject};
use novo_emc::{EmcClass, FieldSetup, Input, NOT_CHECKED, Role, Termination};
use novo_view3d::emc::{
    Assumed, Bond, Class, Fix, Hotspot, Kind, Level, LoopSheet, Node, Picture, Provenance, Quality,
    Role as NodeRole, Run, Shield, ShieldEnd,
};

/// How far the bond outline is drawn outside the mounting area, mm, so the
/// line is not hidden by the device's own body.
const BOND_MARGIN_MM: f64 = 8.0;

/// Height of a device body above the plate when the model has none, mm.
const BODY_HEIGHT_MM: f32 = 60.0;

pub(crate) fn class_of(c: EmcClass) -> Class {
    match c {
        EmcClass::Motor => Class::Motor,
        EmcClass::Power => Class::Mains,
        EmcClass::Control => Class::Control,
        EmcClass::Analog => Class::Analog,
        EmcClass::Fieldbus => Class::Bus,
        EmcClass::Safety => Class::Safety,
        EmcClass::Earth => Class::Earth,
    }
}

fn level_of(l: novo_emc::Level) -> Level {
    match l {
        novo_emc::Level::Fact => Level::Rule,
        novo_emc::Level::Index => Level::Index,
        novo_emc::Level::Estimate => Level::Estimate,
        novo_emc::Level::Field => Level::Field,
    }
}

/// A title short enough for the hot spot list (the card shows the rule id and
/// the engine's full title separately).
fn short_title(rule: &str) -> &'static str {
    match rule {
        "EMC-01" => "Classes in one duct",
        "EMC-02" => "Parallel run",
        "EMC-04" => "Motor cable shield",
        "EMC-05" => "Shield pigtail",
        "EMC-07" => "Drive without filter",
        "EMC-09" => "Loop area",
        "EMC-11" => "Plate bonding",
        "EMC-14" => "Signal leaves cabinet",
        "EMC-17" => "Sensitive near drive",
        "EMC-19" => "Field at sensitive device",
        "EMC-20" => "Induced voltage",
        _ => "EMC finding",
    }
}

/// What kind of marker a rule gets.
fn kind_of(rule: &str) -> Kind {
    match rule {
        "EMC-01" | "EMC-02" => Kind::Parallel,
        "EMC-04" | "EMC-05" | "EMC-14" => Kind::Shield,
        "EMC-09" => Kind::Loop,
        "EMC-11" => Kind::Bond,
        "EMC-07" => Kind::Filter,
        "EMC-19" | "EMC-20" => Kind::Field,
        _ => Kind::Proximity,
    }
}

fn pt(p: (f64, f64)) -> [f32; 3] {
    [p.0 as f32, p.1 as f32, 0.0]
}

/// A closed rectangle outline pushed outwards by `margin` on every side.
fn grown(outline: &[(f64, f64)], margin: f64) -> Vec<[f32; 3]> {
    let (cx, cy) = outline
        .iter()
        .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let n = outline.len().max(1) as f64;
    let (cx, cy) = (cx / n, cy / n);
    outline
        .iter()
        .map(|p| {
            let dx = if p.0 >= cx { margin } else { -margin };
            let dy = if p.1 >= cy { margin } else { -margin };
            pt((p.0 + dx, p.1 + dy))
        })
        .collect()
}

fn end_of(t: Termination) -> ShieldEnd {
    match t {
        Termination::Full360 => ShieldEnd::Full,
        Termination::Pigtail(mm) => ShieldEnd::Pigtail(mm as f32),
        Termination::Open | Termination::Unknown => ShieldEnd::Open,
    }
}

fn fix_of(rule: &str, i: usize, f: &EngineFix) -> Fix {
    let hint = f.hint();
    let applicable = f.applicable();
    Fix {
        key: format!("{rule}/{i}"),
        label: hint.clone(),
        detail: if applicable {
            format!("{hint} Apply it from Check > EMC check (one undo step).")
        } else {
            format!("{hint} By hand: the program cannot do this one.")
        },
        // Applying goes through the Check > EMC check window.
        applicable: false,
    }
}

/// The keep-clear band of an aggressor run: the planning distance to the
/// victim classes that really have a run on this plate, cut by its shield.
/// The band is drawn no wider than a fifth of the plate's short side so the
/// devices stay readable; the real planning distance is on the card of a
/// finding.
fn halo_mm(
    run: &novo_emc::Run,
    victims: &[EmcClass],
    plate_mm: [f32; 2],
    pigtail_max_mm: f64,
) -> f32 {
    if !run.class.is_aggressor() {
        return 0.0;
    }
    let base = victims
        .iter()
        .map(|v| novo_emc::default_distance_mm(run.class, *v))
        .fold(0.0, f64::max);
    let factor = run.shield.map_or(1.0, |s| s.factor(pigtail_max_mm));
    let cap = plate_mm[0].min(plate_mm[1]) * 0.2;
    ((base * factor) as f32).min(cap)
}

/// Markers closer than this to an earlier one are moved aside so the
/// numbered badges do not sit on top of each other, mm.
const MARKER_GAP_MM: f32 = 30.0;

/// Moves a marker that would sit on an earlier one sideways, a step at a
/// time. The span of a finding still shows where it really is.
fn spread_markers(hotspots: &mut [Hotspot]) {
    for i in 1..hotspots.len() {
        let mut k = 0;
        while k < 8 {
            let at = hotspots[i].at;
            let near = hotspots[..i]
                .iter()
                .any(|h| (h.at[0] - at[0]).hypot(h.at[1] - at[1]) < MARKER_GAP_MM);
            if !near {
                break;
            }
            k += 1;
            // Right, left, right further ...
            let side = if k % 2 == 1 { 1.0 } else { -1.0 };
            hotspots[i].at[0] += side * MARKER_GAP_MM * ((k + 1) / 2) as f32;
        }
    }
}

/// The engine's picture of a plate of the project. `None` when the engine
/// has nothing on that plate (no routed wire, no EMC device), so the caller
/// falls back to the interim picture and says so.
pub(crate) fn from_project(
    project: &novo_core::Project,
    plate: novo_core::StructureId,
    plate_mm: [f32; 2],
    drawn: &HashSet<u64>,
) -> Option<Picture> {
    let netlist = project.netlist();
    let input = novo_emc::project::input_of(project, &netlist);
    let field = novo_emc::project::field_setup_of(project, &netlist, &input);
    let pic = build_with(&input, Some(&field), plate.raw(), plate_mm, drawn);
    (!pic.runs.is_empty() || !pic.nodes.is_empty()).then_some(pic)
}

/// The picture of one plate from the engine's input and report.
///
/// `drawn` holds the wires the host view shows: a run the host leaves out
/// (no duct route, not in the scene) is left out here too, with the findings
/// and loops that need it, so the picture never shows what the view cannot.
#[cfg(test)]
pub(crate) fn build(
    input: &Input,
    plate: u64,
    plate_mm: [f32; 2],
    drawn: &HashSet<u64>,
) -> Picture {
    build_with(input, None, plate, plate_mm, drawn)
}

/// Like [`build`], with the currents of the magnetic field estimate: the
/// exposure map becomes the field slice and EMC-19 and EMC-20 can fire.
pub(crate) fn build_with(
    input: &Input,
    field: Option<&FieldSetup>,
    plate: u64,
    plate_mm: [f32; 2],
    drawn: &HashSet<u64>,
) -> Picture {
    let report = novo_emc::analyse_with_field(input, field);
    let pic = novo_emc::picture(input, &report);
    let on_plate = |p: Option<u64>| p == Some(plate);

    let victims: Vec<EmcClass> = input
        .runs
        .iter()
        .filter(|r| on_plate(r.plate) && drawn.contains(&r.id) && !r.class.is_aggressor())
        .map(|r| r.class)
        .collect();
    let runs: Vec<Run> = pic
        .runs
        .iter()
        .filter(|r| on_plate(r.plate) && drawn.contains(&r.id))
        .map(|r| {
            let src = input.runs.iter().find(|x| x.id == r.id);
            Run {
                id: r.id,
                label: r.name.clone(),
                class: class_of(r.class),
                shielded: r.shielded,
                path: r.path.iter().map(|p| pt(*p)).collect(),
                halo_mm: src.map_or(0.0, |x| {
                    halo_mm(x, &victims, plate_mm, input.params.pigtail_max_mm)
                }),
                class_reason: r.class_reason.clone(),
                overridden: r.overridden,
            }
        })
        .collect();

    let nodes = pic
        .zones
        .iter()
        .filter(|z| z.plate == plate)
        .map(|z| {
            let (role, what, base) = match z.role {
                Role::Drive => (NodeRole::Source, "frequency drive", 0.8),
                Role::Noisy => (NodeRole::Source, "noisy device", 0.5),
                Role::Sensitive => (NodeRole::Victim, "sensitive device", 0.6),
                Role::Filter => (NodeRole::Neutral, "mains filter", 0.0),
                Role::Other => (NodeRole::Neutral, "device", 0.0),
            };
            // The findings' score moves the glow; the role sets its floor.
            let level = if role == NodeRole::Neutral {
                0.0
            } else {
                (base + z.score as f32 / 200.0).min(1.0)
            };
            Node {
                device: z.device,
                tag: z.tag.clone(),
                role,
                rect: [
                    z.rect.0 as f32,
                    z.rect.1 as f32,
                    z.rect.2 as f32,
                    z.rect.3 as f32,
                ],
                height: z.height_mm.map_or(BODY_HEIGHT_MM, |h| h as f32),
                level,
                what: what.to_owned(),
            }
        })
        .collect();

    let mut hotspots = Vec::new();
    let shown = |h: &novo_emc::picture::Hotspot| {
        h.plate == plate
            && h.subjects.iter().all(|s| match s {
                Subject::Run(r) => drawn.contains(r),
                _ => true,
            })
    };
    for h in pic.hotspots.iter().filter(|h| shown(h)) {
        let id = hotspots.len() as u32 + 1;
        let finding = report
            .findings
            .iter()
            .find(|f| f.rule == h.rule && f.at == Some((h.plate, h.at)) && f.message == h.message);
        let points = finding
            .and_then(|f| f.points.first().map(|p| p.1))
            .unwrap_or(0);
        let span = finding
            .map(|f| {
                f.ribbons
                    .iter()
                    .flat_map(|r| [pt(r.a.0), pt(r.a.1)])
                    .collect()
            })
            .unwrap_or_default();
        let mut runs_in = Vec::new();
        let mut devices_in = Vec::new();
        for s in &h.subjects {
            match s {
                Subject::Run(r) => runs_in.push(*r),
                Subject::Device(d) => devices_in.push(*d),
                Subject::Cable(_) | Subject::Plate(_) => {}
            }
        }
        let assumptions = if h.level >= novo_emc::Level::Estimate {
            pic.assumptions
                .iter()
                .map(|a| Assumed {
                    text: format!("{}: {}", a.name, a.value),
                    user_set: a.user_set,
                })
                .collect()
        } else {
            Vec::new()
        };
        hotspots.push(Hotspot {
            id,
            kind: kind_of(h.rule),
            level: level_of(h.level),
            at: pt(h.at),
            span,
            risk: (points as f32 / 100.0).clamp(0.0, 1.0),
            title: short_title(h.rule).to_owned(),
            why: h.message.clone(),
            numbers: h
                .values
                .iter()
                .map(|m| (m.label.to_owned(), m.value.clone()))
                .collect(),
            runs: runs_in,
            devices: devices_in,
            rule: h.rule.to_owned(),
            source: format!("{} (clause numbers unverified)", h.source),
            band: h
                .band_hz
                .map(|(lo, hi)| format!("{} to {} MHz", lo / 1.0e6, hi / 1.0e6)),
            assumptions,
            fixes: finding
                .map(|f| {
                    f.fixes
                        .iter()
                        .enumerate()
                        .map(|(i, x)| fix_of(h.rule, i, x))
                        .collect()
                })
                .unwrap_or_default(),
        });
    }

    spread_markers(&mut hotspots);
    let loops = pic
        .loops
        .iter()
        .filter(|l| on_plate(l.plate) && drawn.contains(&l.runs.0) && drawn.contains(&l.runs.1))
        .map(|l| LoopSheet {
            label: l.message.clone(),
            outline: l.polygon.iter().map(|p| pt(*p)).collect(),
            area_cm2: l.area_cm2 as f32,
            risk: (l.area_cm2 as f32 / 100.0).clamp(0.0, 1.0),
        })
        .collect();

    let shields = input
        .runs
        .iter()
        .filter(|r| on_plate(r.plate) && drawn.contains(&r.id))
        .filter_map(|r| {
            let s = r.shield?;
            let (a, b) = (r.path.first()?, r.path.last()?);
            Some(
                [(*a, s.a, "start"), (*b, s.b, "end")].map(|(at, t, side)| Shield {
                    run: r.id,
                    at: pt(at),
                    end: end_of(t),
                    label: format!("{} {side}", r.name),
                }),
            )
        })
        .flatten()
        .collect();

    Picture {
        plate: plate_mm,
        runs,
        nodes,
        hotspots,
        loops,
        bonds: pic
            .bonds
            .iter()
            .filter(|b| b.plate == plate)
            .map(|b| Bond {
                label: format!("{}: {}", b.tag, b.reason),
                path: grown(&b.outline, BOND_MARGIN_MM),
                quality: match b.quality {
                    novo_emc::BondQuality::Good => Quality::Good,
                    novo_emc::BondQuality::Weak => Quality::Weak,
                    novo_emc::BondQuality::Bad => Quality::Bad,
                },
            })
            .collect(),
        shields,
        field: field.and_then(|f| field_of(input, f, plate, plate_mm)),
        provenance: Provenance::Engine,
        assumptions: pic
            .assumptions
            .iter()
            .map(|a| {
                format!(
                    "{}: {} ({})",
                    a.name,
                    a.value,
                    if a.user_set {
                        "set in this project"
                    } else {
                        "program default"
                    }
                )
            })
            .collect(),
        not_checked: NOT_CHECKED.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// The magnetic field slice of a plate as the views' exposure map: the
/// colour is the logarithm of the flux density with the planning value in the
/// middle, and the legend names both ends in microtesla.
fn field_of(
    input: &Input,
    setup: &FieldSetup,
    plate: u64,
    plate_mm: [f32; 2],
) -> Option<novo_view3d::emc::Field> {
    let m = novo_emc::field::map(
        input,
        setup,
        plate,
        (f64::from(plate_mm[0]), f64::from(plate_mm[1])),
    )?;
    let limit = setup.params.limit_ut;
    let ut = |v: f64| {
        if v < 10.0 {
            format!("{v:.1} \u{b5}T")
        } else {
            format!("{v:.0} \u{b5}T")
        }
    };
    Some(novo_view3d::emc::Field {
        origin: [m.origin.0 as f32, m.origin.1 as f32],
        cell: m.cell_mm as f32,
        nx: m.nx,
        ny: m.ny,
        values: m
            .b_ut
            .iter()
            .map(|b| novo_emc::field::ramp_value(f64::from(*b), limit))
            .collect(),
        label: format!(
            "Magnetic field estimate, model (strongest {} at {} Hz, {} mm above the plate)",
            ut(f64::from(m.peak_ut)),
            setup.params.hz,
            setup.params.slice_mm
        ),
        ends: Some((
            ut(limit / 10.0),
            format!("{limit:.1} \u{b5}T"),
            ut(limit * 10.0),
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_emc::{Device, FilterState, Finish, Origin, Params, Plate};

    fn run(id: u64, class: EmcClass, y: f64) -> novo_emc::Run {
        novo_emc::Run {
            id,
            name: format!("W{id}"),
            class,
            origin: Origin::Derived,
            class_reason: "test".into(),
            cable: None,
            shield: None,
            twisted: false,
            net: Some(id as u32),
            from: Some(1),
            to: Some(2),
            plate: Some(1),
            path: vec![(0.0, y), (600.0, y)],
            ducts: Vec::new(),
            diameter_mm: 3.0,
            drive: None,
            leaves_cabinet: false,
        }
    }

    fn all() -> HashSet<u64> {
        [1, 2].into()
    }

    fn input() -> Input {
        let mut motor = run(1, EmcClass::Motor, 50.0);
        motor.drive = Some(5);
        let analog = run(2, EmcClass::Analog, 80.0);
        Input {
            runs: vec![motor, analog],
            ducts: Vec::new(),
            devices: vec![Device {
                id: 5,
                tag: "-T1".into(),
                role: Role::Drive,
                plate: Some(1),
                rect: Some((0.0, 0.0, 100.0, 200.0)),
                filter: FilterState::Unknown,
                filter_for: None,
                bonded: None,
                height_mm: None,
                on_door: false,
            }],
            plates: vec![Plate {
                id: 1,
                tag: "+CAB".into(),
                finish: None,
                door_bonded: None,
            }],
            params: Params::default(),
        }
    }

    #[test]
    fn classes_findings_and_provenance_come_from_the_engine() {
        let pic = build(&input(), 1, [800.0, 400.0], &all());
        assert_eq!(pic.provenance, Provenance::Engine);
        assert_eq!(
            pic.runs.iter().find(|r| r.id == 1).unwrap().class,
            Class::Motor
        );
        assert_eq!(
            pic.runs.iter().find(|r| r.id == 2).unwrap().class,
            Class::Analog
        );
        // An unshielded motor run with an analog run on the plate gets a
        // keep-clear band, never wider than a fifth of the short side.
        let halo = pic.runs.iter().find(|r| r.id == 1).unwrap().halo_mm;
        assert!(halo > 0.0 && halo <= 80.0);
        assert_eq!(pic.runs.iter().find(|r| r.id == 2).unwrap().halo_mm, 0.0);
        let h = pic
            .hotspots
            .iter()
            .find(|h| h.rule == "EMC-02")
            .expect("parallel finding");
        assert_eq!(h.kind, Kind::Parallel);
        assert!(h.id >= 1 && h.risk > 0.0);
        assert!(!h.span.is_empty());
        assert!(h.runs.contains(&1) && h.runs.contains(&2));
        assert!(pic.not_checked.iter().any(|t| t.contains("Compliance")));
        assert!(pic.assumptions.iter().any(|a| a.contains("divider")));
    }

    #[test]
    fn with_currents_the_exposure_map_is_the_field_slice_and_says_so() {
        use novo_emc::{Current, FieldParams};
        let mut inp = input();
        // The analog run's device is a PLC 50 mm from the motor wire.
        inp.devices.push(Device {
            id: 9,
            tag: "-A1".into(),
            role: Role::Sensitive,
            plate: Some(1),
            rect: Some((250.0, 90.0, 60.0, 40.0)),
            filter: FilterState::Unknown,
            filter_for: None,
            bonded: None,
            height_mm: None,
            on_door: false,
        });
        let setup = FieldSetup {
            currents: vec![Current {
                run: 1,
                amps: 16.0,
                phase_deg: 0.0,
                source: "test".into(),
                declared: true,
                phase_known: true,
            }],
            params: FieldParams::default(),
        };
        let without = build(&inp, 1, [800.0, 400.0], &all());
        assert!(without.field.is_none(), "no currents, the index stays");
        let pic = build_with(&inp, Some(&setup), 1, [800.0, 400.0], &all());
        let f = pic.field.as_ref().expect("the field slice");
        assert!(f.label.contains("Magnetic field estimate"), "{}", f.label);
        assert!(f.label.contains("model"));
        let (lo, mid, hi) = f.ends.clone().expect("the legend ends");
        assert!(lo.contains('\u{b5}') && hi.contains('\u{b5}'), "{lo} {hi}");
        assert_eq!(mid, "12.6 \u{b5}T");
        // Strongest on the wire (y = 50), weak at the far side of the plate.
        assert!(f.at(400.0, 55.0) > f.at(400.0, 395.0));
        assert!(f.values.iter().all(|v| (0.0..=1.0).contains(v)));
        let h = pic
            .hotspots
            .iter()
            .find(|h| h.rule == "EMC-19")
            .expect("the PLC sits in the field");
        assert_eq!(h.level, Level::Estimate);
        assert!(h.devices.contains(&9) && h.runs.contains(&1));
        assert_eq!(h.title, "Field at sensitive device");
        assert!(
            h.numbers
                .iter()
                .any(|(l, v)| l.contains("Field") && v.contains('\u{b5}'))
        );
        assert!(h.band.is_some());
        assert!(h.assumptions.iter().any(|a| a.text.contains("Frequency")));
    }

    #[test]
    fn other_plates_are_left_out_and_nodes_carry_roles() {
        let pic = build(&input(), 9, [800.0, 400.0], &all());
        assert!(pic.runs.is_empty() && pic.hotspots.is_empty() && pic.nodes.is_empty());
        let pic = build(&input(), 1, [800.0, 400.0], &all());
        let n = pic.nodes.iter().find(|n| n.device == 5).unwrap();
        assert_eq!(n.role, NodeRole::Source);
        assert!(n.level > 0.0 && n.level <= 1.0);
    }

    #[test]
    fn markers_on_the_same_spot_are_spread_apart() {
        let mut i = input();
        // A second sensitive device beside the first: two findings at one place.
        i.devices.push(Device {
            id: 6,
            tag: "-A1".into(),
            role: Role::Sensitive,
            plate: Some(1),
            rect: Some((110.0, 0.0, 40.0, 40.0)),
            filter: FilterState::Unknown,
            filter_for: None,
            bonded: None,
            height_mm: None,
            on_door: false,
        });
        let pic = build(&i, 1, [800.0, 400.0], &all());
        for (n, a) in pic.hotspots.iter().enumerate() {
            for b in &pic.hotspots[n + 1..] {
                assert!((a.at[0] - b.at[0]).hypot(a.at[1] - b.at[1]) >= MARKER_GAP_MM - 0.01);
            }
        }
    }

    #[test]
    fn a_project_with_nothing_for_the_engine_falls_back() {
        let project = novo_core::Project::default();
        let plate = novo_core::StructureId::from(1);
        // The views then show the interim picture and its note.
        assert!(from_project(&project, plate, [800.0, 400.0], &HashSet::new()).is_none());
    }

    #[test]
    fn no_victim_on_the_plate_means_no_band() {
        let mut i = input();
        i.runs.truncate(1);
        let pic = build(&i, 1, [800.0, 400.0], &all());
        assert_eq!(pic.runs[0].halo_mm, 0.0);
    }

    #[test]
    fn a_run_the_view_does_not_draw_takes_its_findings_with_it() {
        let only_motor: HashSet<u64> = [1].into();
        let pic = build(&input(), 1, [800.0, 400.0], &only_motor);
        assert_eq!(pic.runs.len(), 1);
        assert!(pic.hotspots.iter().all(|h| !h.runs.contains(&2)));
        assert!(pic.hotspots.iter().all(|h| h.rule != "EMC-02"));
    }

    #[test]
    fn shield_ends_are_taken_from_the_run_ends() {
        let mut i = input();
        i.runs[0].cable = Some(100);
        i.runs[0].shield = Some(novo_emc::Shield {
            a: Termination::Full360,
            b: Termination::Pigtail(80.0),
        });
        let pic = build(&i, 1, [800.0, 400.0], &all());
        assert_eq!(pic.shields.len(), 2);
        assert_eq!(pic.shields[0].end, ShieldEnd::Full);
        assert_eq!(pic.shields[1].end, ShieldEnd::Pigtail(80.0));
        assert_eq!(pic.shields[1].at, [600.0, 50.0, 0.0]);
        // A shield halves the keep-clear distance the run asks for.
        assert!(pic.runs[0].shielded);
    }

    #[test]
    fn heights_and_bonds_come_from_the_engine() {
        let mut i = input();
        i.devices[0].height_mm = Some(210.0);
        // Nothing stated: default body height, no bond drawn.
        let mut plain = i.clone();
        plain.devices[0].height_mm = None;
        let pic = build(&plain, 1, [800.0, 400.0], &all());
        assert_eq!(pic.nodes[0].height, BODY_HEIGHT_MM);
        assert!(pic.bonds.is_empty());

        let pic = build(&i, 1, [800.0, 400.0], &all());
        assert_eq!(pic.nodes[0].height, 210.0);

        // Painted plate: a weak bond around the mounting area; the bad one
        // when the drive is stated as not bonded.
        i.plates[0].finish = Some(Finish::Painted);
        let pic = build(&i, 1, [800.0, 400.0], &all());
        assert_eq!(pic.bonds.len(), 1);
        assert_eq!(pic.bonds[0].quality, Quality::Weak);
        assert_eq!(pic.bonds[0].path.len(), 5);
        assert!(pic.bonds[0].label.contains("-T1"));
        i.devices[0].bonded = Some(false);
        let pic = build(&i, 1, [800.0, 400.0], &all());
        assert_eq!(pic.bonds[0].quality, Quality::Bad);
        // Another plate shows none.
        assert!(build(&i, 2, [800.0, 400.0], &all()).bonds.is_empty());
    }
}
