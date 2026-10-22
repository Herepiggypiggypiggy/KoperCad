//! Terminal strips by kind and the bulk Set location: the committed QA
//! fixture is a machine whose field devices were all placed at the cabinet
//! (the way the starter macros do), and the flow the user follows is
//! Devices tab > Set location > Terminals tab > Accept proposed.

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, PotentialSpec, SymbolRole,
    WireId,
};
use novo_parts::{PropertySheet, RowKey};
use novo_terminals::{Grouping, StripClass};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/strip-grouping.novo")
}

/// Cabinet devices Q1, K1, G1 (24 V supply), A1 (PLC input) and E1 (earth
/// bar), and the motor M1 and sensor B1 that belong in the field. Everything
/// starts at `+CAB`, so there is no boundary yet.
fn build_fixture(doc: &mut Document) {
    doc.transact("Strip grouping fixture", |tx| {
        let page = match tx.project().pages_in_order().first() {
            Some(p) => p.id,
            None => tx.add_page("1")?,
        };
        tx.update::<novo_core::Page>(page, |p| p.title = "Motor and sensor".into())?;
        let pins = |list: &[(&str, f64, f64)]| -> Vec<Pin> {
            list.iter().map(|&(n, x, y)| Pin::new(n, x, y)).collect()
        };
        let device = |tx: &mut novo_core::Tx<'_>,
                      t: &str,
                      d: &str,
                      key: &str,
                      x: f64,
                      y: f64,
                      p: Vec<Pin>| {
            let id = tx.add_device(tag(t), d)?;
            tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(x, y)),
                p,
                SymbolRole::Device {
                    device: id,
                    main: true,
                },
            )
        };
        let three = || {
            pins(&[
                ("1", 0.0, 0.0),
                ("2", 0.0, 16.0),
                ("3", 8.0, 0.0),
                ("4", 8.0, 16.0),
                ("5", 16.0, 0.0),
                ("6", 16.0, 16.0),
            ])
        };
        let k1 = device(
            tx,
            "=A1+CAB-K1",
            "Motor contactor",
            "iec/contactors/main_3p",
            40.0,
            40.0,
            three(),
        )?;
        let m1 = device(
            tx,
            "=A1+CAB-M1",
            "Conveyor motor",
            "iec/motors/motor_3ph",
            40.0,
            110.0,
            pins(&[
                ("U1", 0.0, 0.0),
                ("V1", 8.0, 0.0),
                ("W1", 16.0, 0.0),
                ("PE", 24.0, 0.0),
            ]),
        )?;
        let g1 = device(
            tx,
            "=A1+CAB-G1",
            "24 V supply",
            "iec/contacts/no",
            120.0,
            40.0,
            pins(&[("24V", 0.0, 0.0), ("0V", 12.0, 0.0)]),
        )?;
        let b1 = device(
            tx,
            "=A1+CAB-B1",
            "Photo sensor",
            "iec/contacts/no",
            120.0,
            110.0,
            pins(&[("BN", 0.0, 0.0), ("BU", 12.0, 0.0), ("BK", 24.0, 0.0)]),
        )?;
        let a1 = device(
            tx,
            "=A1+CAB-A1",
            "PLC input card",
            "iec/contacts/no",
            144.0,
            40.0,
            pins(&[("I0", 0.0, 0.0)]),
        )?;
        let e1 = device(
            tx,
            "=A1+CAB-E1",
            "Earth bar",
            "iec/contacts/no",
            64.0,
            20.0,
            pins(&[("1", 0.0, 0.0)]),
        )?;
        let w =
            |tx: &mut novo_core::Tx<'_>, a: (_, u16), b: (_, u16)| -> novo_core::Result<WireId> {
                tx.connect(Endpoint::pin(a.0, a.1), Endpoint::pin(b.0, b.1))
            };
        let phases = vec![
            w(tx, (k1, 1), (m1, 0))?,
            w(tx, (k1, 3), (m1, 1))?,
            w(tx, (k1, 5), (m1, 2))?,
        ];
        let supply = w(tx, (g1, 0), (b1, 0))?;
        let zero = w(tx, (g1, 1), (b1, 1))?;
        let signal = w(tx, (b1, 2), (a1, 0))?;
        let earth = w(tx, (m1, 3), (e1, 0))?;
        tx.assign_potentials(
            &phases,
            vec![
                PotentialSpec::new("L1", PotentialKind::Line, Some(400.0)),
                PotentialSpec::new("L2", PotentialKind::Line, Some(400.0)),
                PotentialSpec::new("L3", PotentialKind::Line, Some(400.0)),
            ],
        )?;
        tx.assign_potentials(
            &[supply, zero, signal, earth],
            vec![
                PotentialSpec::new("+24V", PotentialKind::DcPositive, Some(24.0)),
                PotentialSpec::new("0V", PotentialKind::DcNegative, Some(0.0)),
                PotentialSpec::new("DI0", PotentialKind::Signal, None),
                PotentialSpec::new("PE", PotentialKind::ProtectiveEarth, None),
            ],
        )?;
        Ok(())
    })
    .unwrap();
}

/// Rewrites the fixture: `cargo test -p novo-app write_strip_grouping_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/strip-grouping.novo"]
fn write_strip_grouping_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("Strip grouping (example)").unwrap();
    build_fixture(backend.document_mut().unwrap());
    let path = fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}

fn location_edit(doc: &mut Document, tags: &[&str], text: &str) {
    let selection: Vec<novo_core::AnyId> = doc
        .project()
        .devices()
        .filter(|d| tags.iter().any(|t| d.tag.to_string().ends_with(t)))
        .map(|d| d.id.into())
        .collect();
    let sheet = PropertySheet::build(doc.project(), &selection);
    let command = sheet
        .edit(doc.project(), &RowKey::Field("location".into()), text)
        .unwrap()
        .expect("a change");
    doc.execute(command).unwrap();
}

#[test]
fn moving_the_field_devices_creates_proposals_that_land_on_separate_strips() {
    let mut doc = Document::default();
    build_fixture(&mut doc);
    assert!(
        novo_terminals::suggest_terminals(doc.project()).is_empty(),
        "everything is in the cabinet"
    );
    location_edit(&mut doc, &["-M1", "-B1"], "FIELD");
    let suggestions = novo_terminals::suggest_terminals(doc.project());
    let strips: Vec<(StripClass, String)> = suggestions
        .iter()
        .map(|s| (s.class, s.strip.to_string()))
        .collect();
    // Three phases, +24 V, 0 V, the signal, PE: seven wires leave.
    assert_eq!(suggestions.len(), 7, "{strips:?}");
    let on = |class| strips.iter().filter(|(c, _)| *c == class).count();
    assert_eq!(on(StripClass::Power), 3);
    assert_eq!(on(StripClass::Control), 2);
    assert_eq!(on(StripClass::Signal), 1);
    assert_eq!(on(StripClass::Earth), 1);
    for (class, strip) in &strips {
        let expected = match class {
            StripClass::Power | StripClass::Unassigned => "=A1+CAB-X1",
            StripClass::Control => "=A1+CAB-X2",
            StripClass::Signal => "=A1+CAB-X3",
            StripClass::Earth => "=A1+CAB-X4",
        };
        assert_eq!(strip, expected);
    }
    doc.transact("Accept", |tx| {
        novo_terminals::accept_suggestions(tx, &suggestions)
    })
    .unwrap();
    assert_eq!(novo_terminals::strips(doc.project()).len(), 4);
    // One strip per location as before, if the user prefers.
    doc.undo().unwrap();
    doc.transact("Grouping", |tx| {
        novo_terminals::set_grouping(tx, Grouping::Single)
    })
    .unwrap();
    let single = novo_terminals::suggest_terminals(doc.project());
    assert!(single.iter().all(|s| s.strip.to_string() == "=A1+CAB-X1"));
}

#[test]
fn the_committed_fixture_opens_with_everything_in_the_cabinet() {
    use crate::backend::ProjectBackend;
    let path = fixture_path();
    assert!(path.exists(), "run write_strip_grouping_fixture");
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("fixture.novo");
    std::fs::copy(&path, &copy).unwrap();
    let mut backend = crate::backend::StorageBackend::default();
    backend.open(&copy).unwrap();
    let project = backend.document().unwrap().project();
    assert_eq!(project.devices().count(), 6);
    assert!(novo_terminals::suggest_terminals(project).is_empty());
}

#[test]
fn the_terminals_drawer_draws_with_proposals_and_the_grouping_choice() {
    use super::gate1_tests::Drive;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    build_fixture(d.app.backend.document_mut().unwrap());
    location_edit(
        d.app.backend.document_mut().unwrap(),
        &["-M1", "-B1"],
        "FIELD",
    );
    d.app.wb.drawer = Some(super::workbench::Drawer::Terminals);
    for _ in 0..3 {
        d.frame();
    }
    d.app.set_strip_grouping(Grouping::Single);
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(novo_terminals::grouping(project), Grouping::Single);
    d.app.wb.drawer = Some(super::workbench::Drawer::Devices);
    d.frame();
}
