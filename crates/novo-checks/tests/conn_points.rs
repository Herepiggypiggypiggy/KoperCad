//! Checks against the connection data of a part: wire range, conductors per
//! point and ferrules. A part that states nothing is never judged.

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;
use novo_parts::{CatalogPart, DataSource, FerruleRule, PartTerminal};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn found(project: &Project, kind: ProblemKind) -> Vec<Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == kind)
        .collect()
}

/// A terminal-block-like part: point 1 takes 0.5–2.5 mm², one conductor,
/// wants a ferrule; point 2 states nothing.
fn catalog_part(source: Option<DataSource>) -> CatalogPart {
    let mut p = CatalogPart::new("Acme", "CLAMP");
    p.terminals = vec![
        PartTerminal {
            name: "1".into(),
            wire_mm2: Some((0.5, 2.5)),
            max_wires: Some(1),
            ferrule: Some(FerruleRule::Required),
            torque_nm: Some((0.4, 0.5)),
            ..Default::default()
        },
        PartTerminal {
            name: "2".into(),
            ..Default::default()
        },
    ];
    if let Some(s) = source {
        p.sources
            .insert(novo_parts::CONNECTION_SOURCE_KEY.into(), s);
    }
    p
}

struct Fixture {
    project: Project,
    wires: Vec<WireId>,
}

/// Device -K1 with the part above, a post wired to its point `pin` with one
/// wire per entry of `sections`.
fn fixture(part: Option<CatalogPart>, pin: u16, sections: &[Option<f64>]) -> Fixture {
    let mut project = Project::default();
    let mut wires = Vec::new();
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let k1 = tx.add_device(tag("-K1"), "")?;
            if let Some(part) = part {
                let model = part.to_model(PartId::from(0));
                let id = tx.add_part(&model.manufacturer, &model.order_number)?;
                tx.update::<Part>(id, |p| p.properties = model.properties.clone())?;
                tx.assign_part(k1, id)?;
            }
            let k = tx.place_symbol(
                page,
                "test/clamp",
                Placement::at(Point::new(40.0, 40.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            for (i, section) in sections.iter().enumerate() {
                let post = tx.place_symbol(
                    page,
                    "test/post",
                    Placement::at(Point::new(100.0, 20.0 * i as f64)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Unassigned,
                )?;
                let w = tx.connect(Endpoint::pin(k, pin), Endpoint::pin(post, 0))?;
                tx.update::<Wire>(w, |w| w.cross_section_mm2 = *section)?;
                wires.push(w);
            }
            Ok(())
        })
        .unwrap();
    Fixture { project, wires }
}

#[test]
fn a_wire_inside_the_range_is_quiet() {
    let f = fixture(Some(catalog_part(None)), 0, &[Some(1.5)]);
    assert!(found(&f.project, ProblemKind::WireRange).is_empty());
}

#[test]
fn a_wire_too_large_is_an_error_naming_the_source() {
    let f = fixture(
        Some(catalog_part(Some(DataSource::MakerDatasheet))),
        0,
        &[Some(4.0)],
    );
    let p = found(&f.project, ProblemKind::WireRange);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].severity, Severity::Error);
    assert!(p[0].message.contains("larger than"), "{}", p[0].message);
    assert!(p[0].message.contains("0.5–2.5 mm²"), "{}", p[0].message);
    assert!(p[0].message.contains("maker data"), "{}", p[0].message);
    assert!(p[0].at.contains(&AnyId::from(f.wires[0])));
}

#[test]
fn a_wire_too_small_is_a_warning() {
    let f = fixture(Some(catalog_part(None)), 0, &[Some(0.14)]);
    let p = found(&f.project, ProblemKind::WireRange);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].severity, Severity::Warning);
    assert!(p[0].message.contains("smaller than"));
    assert!(p[0].message.contains("source not stated"));
}

#[test]
fn unknown_data_and_unknown_wire_size_are_never_judged() {
    // Point 2 states nothing.
    let f = fixture(Some(catalog_part(None)), 1, &[Some(95.0), Some(95.0)]);
    assert!(found(&f.project, ProblemKind::WireRange).is_empty());
    assert!(found(&f.project, ProblemKind::PointOccupied).is_empty());
    // No part at all.
    let f = fixture(None, 0, &[Some(95.0), Some(95.0)]);
    assert!(found(&f.project, ProblemKind::WireRange).is_empty());
    assert!(found(&f.project, ProblemKind::PointOccupied).is_empty());
    // A wire without a size.
    let f = fixture(Some(catalog_part(None)), 0, &[None]);
    assert!(found(&f.project, ProblemKind::WireRange).is_empty());
}

#[test]
fn two_conductors_on_a_one_conductor_point_are_reported_once() {
    let f = fixture(Some(catalog_part(None)), 0, &[Some(1.5), Some(1.5)]);
    let p = found(&f.project, ProblemKind::PointOccupied);
    assert_eq!(p.len(), 1, "{p:#?}");
    assert!(
        p[0].message
            .contains("takes 1 conductor but 2 are connected")
    );
    assert_eq!(p[0].at.len(), 3);
}

#[test]
fn a_ferrule_switched_off_where_one_is_required() {
    let mut f = fixture(Some(catalog_part(None)), 0, &[Some(1.5)]);
    assert!(found(&f.project, ProblemKind::FerruleUse).is_empty());
    let w = f.wires[0];
    f.project
        .transact("no ferrule", |tx| {
            tx.update::<Wire>(w, |w| {
                w.properties
                    .insert("ferrule".into(), Value::Text("none".into()));
            })
        })
        .unwrap();
    let p = found(&f.project, ProblemKind::FerruleUse);
    assert_eq!(p.len(), 1);
    assert!(p[0].message.contains("asks for one"));
}

#[test]
fn a_ferrule_where_the_part_forbids_one() {
    let mut part = catalog_part(None);
    part.terminals[0].ferrule = Some(FerruleRule::Forbidden);
    let mut f = fixture(Some(part), 0, &[Some(1.5)]);
    let w = f.wires[0];
    f.project
        .transact("ferrule", |tx| {
            tx.update::<Wire>(w, |w| {
                w.properties
                    .insert("ferrule".into(), Value::Text("A1.5-8".into()));
            })
        })
        .unwrap();
    let p = found(&f.project, ProblemKind::FerruleUse);
    assert_eq!(p.len(), 1);
    assert!(p[0].message.contains("does not allow"));
}
