//! The last four portable rules of the EPLAN project check (findings 9.3:
//! C08, C12, C16, C18): each rule has a project that breaks it and one that
//! does not, and stays quiet without the property it reads.

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

fn count(project: &Project, kind: ProblemKind) -> Vec<Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == kind)
        .collect()
}

fn symbol(
    tx: &mut Tx<'_>,
    page: PageId,
    key: &str,
    y: f64,
    pins: &[&str],
    device: DeviceId,
    main: bool,
) -> Result<SymbolId> {
    let pins = pins
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, 0.0, i as f64 * 8.0))
        .collect();
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(0.0, y)),
        pins,
        SymbolRole::Device { device, main },
    )
}

fn relay(sizes: Option<&str>, declared: i64, drawn: usize) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let k = tx.add_device(tag("-K1"), "")?;
        if let Some(s) = sizes {
            tx.set_property(k, "relay.sizes", Some(Value::Text(s.into())))?;
        }
        tx.set_property(k, "contacts.no", Some(Value::Int(declared)))?;
        for i in 0..drawn {
            let pos = i + 1;
            let (a, b) = (format!("{pos}3"), format!("{pos}4"));
            symbol(
                tx,
                page,
                "iec/contacts/no",
                30.0 * i as f64,
                &[&a, &b],
                k,
                false,
            )?;
        }
        Ok(())
    })
    .0
}

#[test]
fn c08_a_relay_larger_than_the_drawn_contacts_need() {
    let found = count(&relay(Some("2,4,8"), 8, 3), ProblemKind::RelaySize);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("a relay with 4 would do"));
    assert_eq!(found[0].severity, Severity::Warning);
    // The right size, no declared sizes, and nothing drawn are all quiet.
    assert!(count(&relay(Some("2,4,8"), 4, 3), ProblemKind::RelaySize).is_empty());
    assert!(count(&relay(None, 8, 3), ProblemKind::RelaySize).is_empty());
    assert!(count(&relay(Some("2,4,8"), 8, 0), ProblemKind::RelaySize).is_empty());
}

fn cable(text: &str, cores: &[(&str, f64)], shield: bool) -> Project {
    build(|tx| {
        let list = cores
            .iter()
            .map(|(n, s)| CableCore {
                cross_section_mm2: Some(*s),
                ..CableCore::new(*n)
            })
            .chain(shield.then(|| CableCore {
                shield: true,
                ..CableCore::new("SH")
            }))
            .collect();
        let c = tx.add_cable(tag("=A1-W1"), list)?;
        tx.update::<Cable>(c, |c| c.description = text.into())?;
        Ok(())
    })
    .0
}

#[test]
fn c12_cable_cores_agree_with_the_cable_text() {
    let good = [("BN", 1.5), ("BU", 1.5), ("BK", 1.5), ("GNYE", 1.5)];
    assert!(
        count(
            &cable("Motor cable 4G1.5", &good, true),
            ProblemKind::CableText
        )
        .is_empty()
    );
    assert!(count(&cable("4 G 1,5 mm2", &good, false), ProblemKind::CableText).is_empty());

    let few = count(&cable("5G1.5", &good, false), ProblemKind::CableText);
    assert_eq!(few.len(), 1, "{few:#?}");
    assert!(
        few[0]
            .message
            .contains("it has 4 cores but the text says 5")
    );

    let thin = [("BN", 1.5), ("BU", 2.5), ("BK", 1.5), ("GNYE", 1.5)];
    let off = count(&cable("4G1.5", &thin, false), ProblemKind::CableText);
    assert!(off[0].message.contains("core BU is not 1.5"), "{off:#?}");

    let no_pe = [("BN", 1.5), ("BU", 1.5), ("BK", 1.5), ("BK2", 1.5)];
    let g = count(&cable("4G1.5", &no_pe, false), ProblemKind::CableText);
    assert!(g[0].message.contains("no core is green-yellow"));
    let x = count(&cable("4x1.5", &good, false), ProblemKind::CableText);
    assert!(x[0].message.contains("a core is green-yellow"));

    // No text of that shape, or no cores yet: not judged.
    assert!(count(&cable("Motor cable", &good, false), ProblemKind::CableText).is_empty());
    assert!(count(&cable("4G1.5", &[], false), ProblemKind::CableText).is_empty());
}

fn neutral_project(rule: bool, external: bool) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let lamp = tx.add_device(tag("-H1"), "")?;
        if external {
            tx.set_property(lamp, "external", Some(Value::Bool(true)))?;
        }
        let l = symbol(tx, page, "iec/lamps/lamp", 0.0, &["X1", "X2"], lamp, true)?;
        let src = tx.place_symbol(
            page,
            "test/post",
            Placement::at(Point::new(100.0, 0.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Unassigned,
        )?;
        let w = tx.connect(Endpoint::pin(l, 1), Endpoint::pin(src, 0))?;
        tx.assign_potentials(
            &[w],
            vec![PotentialSpec::new("N", PotentialKind::Neutral, Some(230.0))],
        )?;
        if rule {
            tx.set_info(|i| {
                i.properties.insert(
                    "neutral.rule".into(),
                    Value::Text("supply-and-external".into()),
                );
            })?;
        }
        Ok(())
    })
    .0
}

#[test]
fn c16_neutral_only_to_supply_and_external_devices_is_opt_in() {
    let found = count(&neutral_project(true, false), ProblemKind::NeutralUse);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("-H1"));
    assert!(count(&neutral_project(false, false), ProblemKind::NeutralUse).is_empty());
    assert!(count(&neutral_project(true, true), ProblemKind::NeutralUse).is_empty());
}

#[test]
fn c18_every_plug_gets_a_pin_overview_as_information() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let d = tx.add_device_of_kind(tag("-XM1"), "", DeviceKind::Connector)?;
        let m = symbol(
            tx,
            page,
            "iec/connectors/plug",
            0.0,
            &["1", "2", "3"],
            d,
            true,
        )?;
        let post = tx.place_symbol(
            page,
            "test/post",
            Placement::at(Point::new(50.0, 0.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Unassigned,
        )?;
        tx.connect(Endpoint::pin(m, 0), Endpoint::pin(post, 0))?;
        Ok(())
    });
    let found = count(&project, ProblemKind::PlugPins);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Info);
    assert!(
        found[0].message.contains("3 pins; wired 1; free 2, 3"),
        "{}",
        found[0].message
    );
}

/// The release probe: a one-pole contactor coil wired into the feed of a
/// three-phase motor is flagged with the way out; a coil on a control
/// circuit (wired to a lamp) is not.
#[test]
fn a_coil_in_a_three_phase_feed_points_to_the_power_contacts() {
    let wire_coil_to = |motor_key: &str, pins: &[&str]| {
        build(|tx| {
            let page = tx.add_page("1")?;
            let k = tx.add_device(tag("-K1"), "")?;
            let m = tx.add_device(tag("-M1"), "")?;
            let coil = symbol(tx, page, "iec/coils/contactor", 0.0, &["A1", "A2"], k, true)?;
            let load = symbol(tx, page, motor_key, 100.0, pins, m, true)?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(load, 0))?;
            Ok(())
        })
        .0
    };
    let feed = wire_coil_to("iec/motors/motor_3ph", &["U1", "V1", "W1"]);
    let found = count(&feed, ProblemKind::CoilOnPowerFeed);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(found[0].message.contains("3-pole power contacts"));
    let control = wire_coil_to("iec/signals/lamp", &["X1", "X2"]);
    assert!(count(&control, ProblemKind::CoilOnPowerFeed).is_empty());
}
