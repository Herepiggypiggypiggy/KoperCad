//! The remaining portable rules of the EPLAN project check (findings 9.3:
//! C02, C03, C05, C06, C07, C10, C11, C14, C15, C17, F01, F04, H01): each
//! rule has a project that breaks it and one that does not.

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

/// A bare four-pin symbol to wire things to.
fn post(tx: &mut Tx<'_>, page: PageId, x: f64, y: f64) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "test/post",
        Placement::at(Point::new(x, y)),
        vec![
            Pin::new("1", 0.0, 0.0),
            Pin::new("2", 0.0, 10.0),
            Pin::new("3", 10.0, 0.0),
            Pin::new("4", 10.0, 10.0),
        ],
        SymbolRole::Unassigned,
    )
}

fn device_symbol(
    tx: &mut Tx<'_>,
    page: PageId,
    key: &str,
    at: Point,
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
        Placement::at(at),
        pins,
        SymbolRole::Device { device, main },
    )
}

fn spec(name: &str, kind: PotentialKind, volts: f64) -> PotentialSpec {
    PotentialSpec::new(name, kind, Some(volts))
}

/// A wire between two fresh posts carrying the potential.
fn fed_wire(tx: &mut Tx<'_>, page: PageId, y: f64, s: PotentialSpec) -> Result<(SymbolId, WireId)> {
    let a = post(tx, page, 0.0, y)?;
    let b = post(tx, page, 100.0, y)?;
    let w = tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))?;
    tx.assign_potentials(&[w], vec![s])?;
    Ok((b, w))
}

#[test]
fn c02_a_terminal_with_no_wire_on_any_side() {
    let (project, t2) = build(|tx| {
        let page = tx.add_page("1")?;
        let strip = tx.add_terminal_strip(tag("-X1"))?;
        let t1 = tx.add_terminal(strip, "1")?;
        let t2 = tx.add_terminal(strip, "2")?;
        let other = post(tx, page, 0.0, 0.0)?;
        for (i, t) in [t1, t2].into_iter().enumerate() {
            let s = tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(50.0 + 20.0 * i as f64, 0.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            tx.update::<SymbolInstance>(s, |s| s.bridges = vec![vec![0, 1]])?;
            if i == 0 {
                tx.connect(Endpoint::pin(other, 0), Endpoint::pin(s, 1))?;
            }
        }
        Ok(t2)
    });
    let found = count(&project, ProblemKind::UnwiredTerminal);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].at.contains(&t2.into()));
    assert!(found[0].message.contains("-X1:2"));
    assert_eq!(found[0].severity, Severity::Warning);
}

fn plug(tx: &mut Tx<'_>, page: PageId, name: &str, mate: &str, x: f64) -> Result<SymbolId> {
    let d = tx.add_device_of_kind(tag(name), "", DeviceKind::Connector)?;
    tx.set_property(d, "plug.mate", Some(Value::Text(mate.into())))?;
    device_symbol(
        tx,
        page,
        "iec/connectors/plug",
        Point::new(x, 0.0),
        &["1", "2", "3"],
        d,
        true,
    )
}

#[test]
fn c17_and_c02_plug_pins_on_both_halves() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let m = plug(tx, page, "-XM1", "-XF1", 0.0)?;
        let f = plug(tx, page, "-XF1", "-XM1", 100.0)?;
        let wiring = post(tx, page, 50.0, 50.0)?;
        // Pin 1 on both halves, pin 2 on the male half only, pin 3 on neither.
        tx.connect(Endpoint::pin(m, 0), Endpoint::pin(wiring, 0))?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(wiring, 1))?;
        tx.connect(Endpoint::pin(m, 1), Endpoint::pin(wiring, 2))?;
        Ok(())
    });
    let one_sided = count(&project, ProblemKind::PlugPinOneSided);
    assert_eq!(one_sided.len(), 1, "{one_sided:#?}");
    assert!(
        one_sided[0]
            .message
            .contains("pin 2 is wired on -XM1 but not on -XF1")
    );
    let unwired = count(&project, ProblemKind::UnwiredTerminal);
    assert_eq!(unwired.len(), 1, "{unwired:#?}");
    assert!(unwired[0].message.contains("pin 3 has no wire"));
}

#[test]
fn c03_an_open_end_lying_on_something() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let a = post(tx, page, 0.0, 0.0)?;
        let b = post(tx, page, 100.0, 0.0)?;
        let c = post(tx, page, 100.0, 50.0)?;
        // Ends exactly on b's pin 2 at (100, 10) without joining it.
        tx.connect(Endpoint::pin(a, 0), Endpoint::free(Point::new(100.0, 10.0)))?;
        // Ends in the middle of the wire a:3 - b:1 (y = 0), not on a pin.
        tx.connect(Endpoint::pin(a, 2), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(c, 0), Endpoint::free(Point::new(50.0, 0.0)))?;
        // Free in empty space is only an open wire, not a stray end.
        tx.connect(
            Endpoint::pin(c, 2),
            Endpoint::free(Point::new(300.0, 300.0)),
        )?;
        Ok(())
    });
    let found = count(&project, ProblemKind::StrayWireEnd);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found
            .iter()
            .any(|p| p.message.contains("lies on test/post:2")
                || p.message.contains("lies on a missing pin")
                || p.message.contains("lies on"))
    );
    assert!(found.iter().any(|p| p.message.contains("another wire")));
}

fn contact(
    tx: &mut Tx<'_>,
    page: PageId,
    device: DeviceId,
    key: &str,
    pins: [&str; 2],
    y: f64,
) -> Result<SymbolId> {
    device_symbol(tx, page, key, Point::new(0.0, y), &pins, device, false)
}

#[test]
fn c05_contact_numbers_follow_the_type() {
    let (project, bad) = build(|tx| {
        let page = tx.add_page("1")?;
        let k = tx.add_device(tag("-K1"), "")?;
        contact(tx, page, k, "iec/contacts/no", ["13", "14"], 0.0)?;
        contact(tx, page, k, "iec/contacts/nc", ["21", "22"], 30.0)?;
        // An NC contact numbered like an NO contact.
        let bad = contact(tx, page, k, "iec/contacts/nc", ["33", "34"], 60.0)?;
        // Overload relay and delayed forms are fine.
        contact(tx, page, k, "iec/contacts/nc", ["95", "96"], 90.0)?;
        contact(tx, page, k, "iec/contacts/no", ["97", "98"], 120.0)?;
        Ok(bad)
    });
    let found = count(&project, ProblemKind::ContactType);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].at, vec![AnyId::from(bad)]);
    assert!(found[0].message.contains("NC contact has pin 33, 34"));
}

#[test]
fn c06_contact_positions_fit_the_relay() {
    let (project, bad) = build(|tx| {
        let page = tx.add_page("1")?;
        let k = tx.add_device(tag("-K1"), "")?;
        tx.set_property(k, "contacts.no", Some(Value::Int(2)))?;
        contact(tx, page, k, "iec/contacts/no", ["13", "14"], 0.0)?;
        contact(tx, page, k, "iec/contacts/no", ["23", "24"], 30.0)?;
        let bad = contact(tx, page, k, "iec/contacts/no", ["53", "54"], 60.0)?;
        // A device that declares nothing is not judged.
        let free = tx.add_device(tag("-K2"), "")?;
        contact(tx, page, free, "iec/contacts/no", ["83", "84"], 90.0)?;
        Ok(bad)
    });
    let found = count(&project, ProblemKind::ContactPosition);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].at, vec![AnyId::from(bad)]);
}

#[test]
fn c07_one_contact_drawn_twice() {
    let (project, ids) = build(|tx| {
        let page = tx.add_page("1")?;
        let k = tx.add_device(tag("-K1"), "")?;
        let a = contact(tx, page, k, "iec/contacts/no", ["13", "14"], 0.0)?;
        let b = contact(tx, page, k, "iec/contacts/no", ["13", "14"], 30.0)?;
        contact(tx, page, k, "iec/contacts/no", ["23", "24"], 60.0)?;
        // The same numbers on another device are another contact.
        let k2 = tx.add_device(tag("-K2"), "")?;
        contact(tx, page, k2, "iec/contacts/no", ["13", "14"], 90.0)?;
        Ok(vec![a, b])
    });
    let found = count(&project, ProblemKind::ContactDrawnTwice);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].at.len(), ids.len());
    assert!(found[0].message.contains("13-14 is drawn 2 times"));
}

#[test]
fn c10_voltage_classes_under_one_function_code() {
    let mixed = |second_function: &str| {
        build(|tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            tx.update::<Page>(p1, |p| p.structure = tag("=A1"))?;
            tx.update::<Page>(p2, |p| p.structure = tag(second_function))?;
            fed_wire(tx, p1, 0.0, spec("24V", PotentialKind::DcPositive, 24.0))?;
            fed_wire(tx, p2, 0.0, spec("L1", PotentialKind::Line, 400.0))?;
            Ok(())
        })
        .0
    };
    let found = count(&mixed("=A1"), ProblemKind::VoltageClassMix);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("=A1"));
    assert!(count(&mixed("=A2"), ProblemKind::VoltageClassMix).is_empty());
}

#[test]
fn c11_a_pe_terminal_off_the_protective_conductor() {
    let build_with = |connected: bool| {
        build(|tx| {
            let page = tx.add_page("1")?;
            let strip = tx.add_terminal_strip(tag("-X1"))?;
            let t = tx.add_terminal(strip, "PE")?;
            tx.update::<Terminal>(t, |t| t.kind = TerminalKind::ProtectiveEarth)?;
            let sym = tx.place_symbol(
                page,
                "iec/terminals/pe",
                Placement::at(Point::new(50.0, 50.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            tx.update::<SymbolInstance>(sym, |s| s.bridges = vec![vec![0, 1]])?;
            let (src, _) = fed_wire(
                tx,
                page,
                0.0,
                spec("PE", PotentialKind::ProtectiveEarth, 0.0),
            )?;
            let other = post(tx, page, 200.0, 200.0)?;
            let to = if connected { src } else { other };
            tx.connect(Endpoint::pin(sym, 0), Endpoint::pin(to, 0))?;
            Ok(t)
        })
    };
    let (broken, t) = build_with(false);
    let found = count(&broken, ProblemKind::ConductorContinuity);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].at.contains(&t.into()));
    assert!(count(&build_with(true).0, ProblemKind::ConductorContinuity).is_empty());
}

fn motor(wires: [&str; 3]) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let d = tx.add_device(tag("-M1"), "")?;
        let m = device_symbol(
            tx,
            page,
            "iec/motors/motor_3ph",
            Point::new(300.0, 0.0),
            &["U1", "V1", "W1"],
            d,
            true,
        )?;
        let mut shared: Option<SymbolId> = None;
        for (i, name) in wires.into_iter().enumerate() {
            let kind = PotentialKind::Line;
            let (src, w) = if name == "shared" {
                let src = *shared.get_or_insert(post(tx, page, 0.0, 400.0)?);
                (
                    src,
                    tx.connect(Endpoint::pin(src, 0), Endpoint::pin(m, i as u16))?,
                )
            } else {
                let src = post(tx, page, 0.0, 50.0 * i as f64)?;
                (
                    src,
                    tx.connect(Endpoint::pin(src, 0), Endpoint::pin(m, i as u16))?,
                )
            };
            let _ = src;
            if name != "shared" {
                tx.assign_potentials(&[w], vec![spec(name, kind, 400.0)])?;
            }
        }
        Ok(())
    })
    .0
}

#[test]
fn c14_motor_windings_get_three_phases() {
    assert!(count(&motor(["L1", "L2", "L3"]), ProblemKind::MotorPhases).is_empty());
    let same_phase = count(&motor(["L1", "L1", "L3"]), ProblemKind::MotorPhases);
    assert_eq!(same_phase.len(), 1, "{same_phase:#?}");
    assert!(same_phase[0].message.contains("same phase"));
    let short = count(&motor(["shared", "shared", "L3"]), ProblemKind::MotorPhases);
    assert_eq!(short.len(), 1, "{short:#?}");
    assert!(short[0].message.contains("short between phases"));
}

fn coil(a1: Option<PotentialSpec>, a2: Option<PotentialSpec>, same_net: bool) -> Project {
    build(|tx| {
        let page = tx.add_page("1")?;
        let d = tx.add_device(tag("-K1"), "")?;
        let c = device_symbol(
            tx,
            page,
            "iec/coils/relay",
            Point::new(300.0, 0.0),
            &["A1", "A2"],
            d,
            true,
        )?;
        let top = post(tx, page, 0.0, 0.0)?;
        let bottom = post(tx, page, 0.0, 100.0)?;
        let w1 = tx.connect(Endpoint::pin(top, 0), Endpoint::pin(c, 0))?;
        let second = if same_net { top } else { bottom };
        let w2 = tx.connect(
            Endpoint::pin(second, if same_net { 0 } else { 1 }),
            Endpoint::pin(c, 1),
        )?;
        if let Some(s) = a1 {
            tx.assign_potentials(&[w1], vec![s])?;
        }
        if let Some(s) = a2 {
            tx.assign_potentials(&[w2], vec![s])?;
        }
        Ok(())
    })
    .0
}

#[test]
fn c15_a_coil_between_control_voltage_and_return() {
    let hot = || Some(spec("+24V", PotentialKind::DcPositive, 24.0));
    let ret = || Some(spec("0V", PotentialKind::DcNegative, 24.0));
    assert!(count(&coil(hot(), ret(), false), ProblemKind::CoilSupply).is_empty());
    let shorted = count(&coil(None, None, true), ProblemKind::CoilSupply);
    assert_eq!(shorted.len(), 1, "{shorted:#?}");
    assert!(shorted[0].message.contains("shorted"));
    let same = count(&coil(hot(), hot(), false), ProblemKind::CoilSupply);
    assert_eq!(same.len(), 1, "{same:#?}");
    assert!(same[0].message.contains("same potential"));
    let earth = Some(spec("PE", PotentialKind::ProtectiveEarth, 0.0));
    let on_pe = count(&coil(hot(), earth, false), ProblemKind::CoilSupply);
    assert_eq!(on_pe.len(), 1, "{on_pe:#?}");
}

fn named_project(rule: bool, name: &str) -> Project {
    build(|tx| {
        let page = tx.add_page("3")?;
        let d = tx.add_device(tag(name), "")?;
        // x = 20 mm is column 1 of the default sheet.
        device_symbol(
            tx,
            page,
            "iec/coils/relay",
            Point::new(20.0, 20.0),
            &["A1", "A2"],
            d,
            true,
        )?;
        if rule {
            tx.set_info(|i| {
                i.properties
                    .insert("tag.rule".into(), Value::Text("page-column".into()));
            })?;
        }
        Ok(())
    })
    .0
}

#[test]
fn f01_the_page_and_column_rule_is_opt_in() {
    let wrong = count(&named_project(true, "-K7"), ProblemKind::TagRule);
    assert_eq!(wrong.len(), 1, "{wrong:#?}");
    assert!(wrong[0].message.contains("-K031"));
    assert!(count(&named_project(true, "-K031"), ProblemKind::TagRule).is_empty());
    assert!(count(&named_project(true, "-K031.2"), ProblemKind::TagRule).is_empty());
    assert!(count(&named_project(false, "-K7"), ProblemKind::TagRule).is_empty());
}

#[test]
fn f04_a_typed_tag_that_is_not_the_device_tag() {
    let shown = |text: &str| {
        build(|tx| {
            let page = tx.add_page("1")?;
            let d = tx.add_device(tag("=A1-K1"), "")?;
            let s = device_symbol(
                tx,
                page,
                "iec/coils/relay",
                Point::new(0.0, 0.0),
                &["A1", "A2"],
                d,
                true,
            )?;
            tx.add_text(
                page,
                TextContent::Literal { text: text.into() },
                TextPlacement::default(),
                Some(s.into()),
            )?;
            Ok(())
        })
        .0
    };
    assert_eq!(count(&shown("-K2"), ProblemKind::DisplayedTag).len(), 1);
    assert!(count(&shown("-K1"), ProblemKind::DisplayedTag).is_empty());
    assert!(count(&shown("=A1-K1"), ProblemKind::DisplayedTag).is_empty());
    // A note that is not a tag is free text.
    assert!(count(&shown("spare"), ProblemKind::DisplayedTag).is_empty());
}

#[test]
fn h01_codes_missing_from_the_structure_list() {
    let (project, ()) = build(|tx| {
        // A list that is not declared complete is not used to judge codes.
        tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
        tx.add_device(tag("=Z9+FIELD-K1"), "")?;
        Ok(())
    });
    assert!(count(&project, ProblemKind::StructureCode).is_empty());
    let (project, ()) = build(|tx| {
        tx.set_info(|i| {
            i.properties
                .insert("structure.list".into(), Value::Text("complete".into()));
        })?;
        tx.add_structure(tag("=A1"), "Plant", false)?;
        tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
        tx.add_device(tag("=A1+CAB1-K1"), "")?;
        tx.add_device(tag("=B7+CAB1-K2"), "")?;
        let page = tx.add_page("1")?;
        tx.update::<Page>(page, |p| p.structure = tag("+HALL"))?;
        Ok(())
    });
    let found = count(&project, ProblemKind::StructureCode);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(found.iter().any(|p| p.message.starts_with("=B7 ")));
    assert!(found.iter().any(|p| p.message.starts_with("+HALL ")));
}
