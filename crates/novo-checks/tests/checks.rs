//! One test per check on small hand-built projects.

use novo_checks::{Problem, ProblemKind, Severity, Summary, check};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn two_pin(a: &str, b: &str) -> Vec<Pin> {
    vec![Pin::new(a, 0.0, 0.0), Pin::new(b, 0.0, 16.0)]
}

fn device_role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// A project with rules off (as an old file loads), built in one edit.
fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

fn of_kind(problems: &[Problem], kind: ProblemKind) -> Vec<&Problem> {
    problems.iter().filter(|p| p.kind == kind).collect()
}

/// A lamp `-H1` with a part, wired between two feeds so nothing is free.
fn wired_lamp(tx: &mut Tx<'_>, page: PageId, x: f64) -> Result<SymbolId> {
    let h1 = tx.add_device(tag(&format!("-H{}", x as u32)), "Lamp")?;
    let part = tx.add_part("Acme", format!("LAMP-{x}"))?;
    tx.assign_part(h1, part)?;
    let lamp = tx.place_symbol(
        page,
        "iec/signals/lamp",
        Placement::at(Point::new(x, 40.0)),
        two_pin("X1", "X2"),
        device_role(h1, true),
    )?;
    for (pin, y) in [(0u16, 20.0), (1, 70.0)] {
        let j = tx.add_junction(page, Point::new(x, y))?;
        tx.connect(Endpoint::pin(lamp, pin), Endpoint::junction(j))?;
    }
    Ok(lamp)
}

#[test]
fn empty_and_clean_projects_have_no_problems() {
    assert!(check(&Project::new("x")).is_empty());
    let (project, _) = build(|tx| {
        let page = tx.add_page("1")?;
        wired_lamp(tx, page, 40.0)
    });
    let problems = check(&project);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn unconnected_pins_one_problem_per_symbol() {
    let (project, (lamp, k1)) = build(|tx| {
        let page = tx.add_page("1")?;
        let lamp = wired_lamp(tx, page, 40.0)?;
        // Free both ends of the lamp's second wire by removing it.
        let wire = tx
            .project()
            .wires_at(Endpoint::pin(lamp, 1))
            .next()
            .unwrap();
        tx.remove(wire)?;
        let k1 = tx.add_device(tag("-K1"), "")?;
        let coil = tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(80.0, 40.0)),
            two_pin("A1", "A2"),
            device_role(k1, true),
        )?;
        Ok((lamp, coil))
    });
    let problems = check(&project);
    let free = of_kind(&problems, ProblemKind::UnconnectedPin);
    assert_eq!(free.len(), 2, "{free:#?}");
    assert_eq!(free[0].at, vec![AnyId::from(lamp)]);
    assert_eq!(free[0].message, "-H40: X2 not connected");
    assert_eq!(free[0].severity, Severity::Warning);
    assert_eq!(free[1].at, vec![AnyId::from(k1)]);
    assert_eq!(free[1].message, "-K1: A1, A2 not connected");
}

#[test]
fn touching_pins_without_a_wire_are_reported_as_such() {
    let (project, (a, b)) = build(|tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("-K1"), "")?;
        let k2 = tx.add_device(tag("-K2"), "")?;
        let a = tx.place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(Point::new(0.0, 0.0)),
            two_pin("13", "14"),
            device_role(k1, true),
        )?;
        // -K2's 13 sits exactly on -K1's 14.
        let b = tx.place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(Point::new(0.0, 16.0)),
            two_pin("13", "14"),
            device_role(k2, true),
        )?;
        Ok((a, b))
    });
    let free = check(&project);
    let free = of_kind(&free, ProblemKind::UnconnectedPin);
    assert_eq!(free.len(), 2);
    assert_eq!(free[0].at, vec![AnyId::from(a)]);
    assert_eq!(
        free[0].message,
        "-K1: 13 not connected; 14 touches another point but no wire joins them"
    );
    assert_eq!(free[1].at, vec![AnyId::from(b)]);
}

#[test]
fn duplicate_tags_once_per_tag() {
    let (project, (lamp, k1b, w1)) = build(|tx| {
        let page = tx.add_page("1")?;
        let lamp = wired_lamp(tx, page, 40.0)?;
        let k1b = tx.add_device(tag("-H40"), "unplaced twin")?;
        let w1 = tx.add_cable(tag("-H40"), vec![])?;
        Ok((lamp, k1b, w1))
    });
    let problems = check(&project);
    let dup = of_kind(&problems, ProblemKind::DuplicateTag);
    assert_eq!(dup.len(), 1);
    assert_eq!(dup[0].severity, Severity::Error);
    assert_eq!(
        dup[0].at,
        vec![AnyId::from(lamp), AnyId::from(k1b), AnyId::from(w1)]
    );
    assert_eq!(dup[0].message, "-H40 is used by 2 devices and 1 cable");
    assert_eq!(dup[0].page, project.symbol(lamp).map(|s| s.page));
}

#[test]
fn duplicate_terminals_only_when_the_rule_is_off() {
    let make = |same_name: SameName| {
        build(|tx| {
            tx.set_info(|i| i.rules.terminals.same_name = same_name)?;
            let x1 = tx.add_terminal_strip(tag("-X1"))?;
            tx.add_terminal(x1, "1")?;
            tx.add_terminal(x1, "1")?;
            tx.add_terminal(x1, "2")?;
            Ok(())
        })
        .0
    };
    let off = check(&make(SameName::Off));
    let dup = of_kind(&off, ProblemKind::DuplicateTerminal);
    assert_eq!(dup.len(), 1);
    assert_eq!(dup[0].at.len(), 2);
    assert_eq!(
        dup[0].message,
        "-X1:1 is on its strip 2 times as separate terminals"
    );
    let join = check(&make(SameName::Join));
    assert!(of_kind(&join, ProblemKind::DuplicateTerminal).is_empty());
}

#[test]
fn device_functions_no_main_several_mains_and_contact_limits() {
    let (project, (contact, coils, extra)) = build(|tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("-K1"), "contacts only")?;
        let k2 = tx.add_device(tag("-K2"), "two coils")?;
        let k3 = tx.add_device(tag("-K3"), "one NO available")?;
        tx.set_property(k3, "contacts.no", Some(1i64.into()))?;
        tx.set_property(k3, "contacts.nc", Some(5i64.into()))?;
        let place = |tx: &mut Tx<'_>, key: &str, x: f64, role| {
            tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(x, 0.0)),
                two_pin("1", "2"),
                role,
            )
        };
        let contact = place(tx, "iec/contacts/no", 0.0, device_role(k1, false))?;
        let c1 = place(tx, "iec/coils/relay", 10.0, device_role(k2, true))?;
        let c2 = place(tx, "iec/coils/relay", 20.0, device_role(k2, true))?;
        place(tx, "iec/coils/relay", 30.0, device_role(k3, true))?;
        let n1 = place(tx, "iec/contacts/no", 40.0, device_role(k3, false))?;
        let n2 = place(tx, "iec/contacts/no", 50.0, device_role(k3, false))?;
        place(tx, "iec/contacts/nc", 60.0, device_role(k3, false))?;
        Ok((contact, vec![c1, c2], vec![n1, n2]))
    });
    let problems = check(&project);
    let no_main = of_kind(&problems, ProblemKind::NoMainFunction);
    assert_eq!(no_main.len(), 1);
    assert_eq!(no_main[0].at, vec![AnyId::from(contact)]);
    assert_eq!(
        no_main[0].message,
        "-K1 is drawn without its main function (coil)"
    );
    let several = of_kind(&problems, ProblemKind::SeveralMainFunctions);
    assert_eq!(several.len(), 1);
    assert_eq!(
        several[0].at,
        coils.into_iter().map(AnyId::from).collect::<Vec<_>>()
    );
    let over = of_kind(&problems, ProblemKind::TooManyContacts);
    assert_eq!(over.len(), 1, "NC is within its limit");
    assert_eq!(over[0].severity, Severity::Error);
    assert_eq!(
        over[0].at,
        extra.into_iter().map(AnyId::from).collect::<Vec<_>>()
    );
    assert_eq!(over[0].message, "-K3: 2 NO contacts drawn, 1 available");
}

#[test]
fn contact_limits_are_skipped_without_data() {
    let (project, _) = build(|tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("-K1"), "")?;
        for x in [0.0, 10.0, 20.0] {
            tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(x, 0.0)),
                two_pin("13", "14"),
                device_role(k1, x == 0.0),
            )?;
        }
        Ok(())
    });
    assert!(of_kind(&check(&project), ProblemKind::TooManyContacts).is_empty());
}

#[test]
fn lone_interruption_points() {
    let (project, lone) = build(|tx| {
        let p1 = tx.add_page("1")?;
        let p2 = tx.add_page("2")?;
        let point = |tx: &mut Tx<'_>, page, name: &str| {
            tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                Placement::at(Point::new(0.0, 0.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption { name: name.into() },
            )
        };
        point(tx, p1, "A")?;
        point(tx, p2, "A")?;
        point(tx, p2, "B")
    });
    let problems = check(&project);
    let lonely = of_kind(&problems, ProblemKind::LoneInterruption);
    assert_eq!(lonely.len(), 1);
    assert_eq!(lonely[0].at, vec![AnyId::from(lone)]);
    assert_eq!(lonely[0].message, "Interruption point B has no partner");
}

#[test]
fn potential_conflicts_mark_what_defines_them() {
    let (project, (feed, wire)) = build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let n = tx.add_potential("N", PotentialKind::Neutral)?;
        let feed = tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            Placement::at(Point::new(0.0, 0.0)),
            vec![Pin::new("1", 0.0, 8.0)],
            SymbolRole::PotentialFeed { potential: l1 },
        )?;
        let j = tx.add_junction(page, Point::new(0.0, 20.0))?;
        let wire = tx.connect(Endpoint::pin(feed, 0), Endpoint::junction(j))?;
        tx.set_wire_potential(wire, Some(n))?;
        Ok((feed, wire))
    });
    let problems = check(&project);
    let shorts = of_kind(&problems, ProblemKind::PotentialConflict);
    assert_eq!(shorts.len(), 1);
    assert_eq!(shorts[0].at, vec![AnyId::from(feed), AnyId::from(wire)]);
    assert_eq!(
        shorts[0].message,
        "L1 and N are defined on one net: a short"
    );
}

#[test]
fn zero_length_wires_except_touching_pins() {
    let (project, bad) = build(|tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("-K1"), "")?;
        let k2 = tx.add_device(tag("-K2"), "")?;
        let a = tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(0.0, 0.0)),
            two_pin("A1", "A2"),
            device_role(k1, true),
        )?;
        let b = tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(0.0, 16.0)),
            two_pin("A1", "A2"),
            device_role(k2, true),
        )?;
        // Touching pins: fine.
        tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 0))?;
        // A junction on top of a pin: zero length, flagged.
        let j = tx.add_junction(page, Point::new(0.0, 0.0))?;
        let bad = tx.connect(Endpoint::junction(j), Endpoint::pin(a, 0))?;
        let j2 = tx.add_junction(page, Point::new(0.0, 40.0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::junction(j2))?;
        Ok(bad)
    });
    let problems = check(&project);
    let zero = of_kind(&problems, ProblemKind::ZeroLengthWire);
    assert_eq!(zero.len(), 1, "{problems:#?}");
    assert_eq!(zero[0].at, vec![AnyId::from(bad)]);
    assert_eq!(
        zero[0].message,
        "The wire from a junction to -K1:A1 has zero length"
    );
}

#[test]
fn missing_parts_are_info() {
    let (project, (k1, x1_sym, w1)) = build(|tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("-K1"), "unplaced")?;
        let x1 = tx.add_terminal_strip(tag("-X1"))?;
        let t = tx.add_terminal(x1, "1")?;
        let sym = tx.place_symbol(
            page,
            "iec/terminals/terminal",
            Placement::at(Point::new(0.0, 0.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Terminal { terminals: vec![t] },
        )?;
        let j = tx.add_junction(page, Point::new(0.0, 10.0))?;
        tx.connect(Endpoint::pin(sym, 0), Endpoint::junction(j))?;
        let w1 = tx.add_cable(tag("-W1"), vec![])?;
        Ok((k1, sym, w1))
    });
    let problems = check(&project);
    assert!(problems.iter().all(|p| p.kind == ProblemKind::MissingPart));
    assert!(problems.iter().all(|p| p.severity == Severity::Info));
    // Drawn first (the strip, through its terminal), then the undrawn ones.
    let at: Vec<&[AnyId]> = problems.iter().map(|p| p.at.as_slice()).collect();
    assert_eq!(
        at,
        vec![
            &[AnyId::from(x1_sym)][..],
            &[AnyId::from(k1)][..],
            &[AnyId::from(w1)][..]
        ]
    );
    assert_eq!(problems[1].page, None);
    let summary = Summary::of(&problems);
    assert_eq!((summary.errors, summary.warnings, summary.infos), (0, 0, 3));
    assert_eq!(summary.pill_text(), "No issues");
}

#[test]
fn problems_come_in_reading_order() {
    let (project, order) = build(|tx| {
        // Page "10" sorts after page "2".
        let p10 = tx.add_page("10")?;
        let p2 = tx.add_page("2")?;
        let mut order = Vec::new();
        for (page, x, y, name) in [
            (p10, 0.0, 0.0, "A"),
            (p2, 50.0, 0.0, "B"),
            (p2, 10.0, 90.0, "C"),
            (p2, 10.0, 5.0, "D"),
        ] {
            let s = tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                Placement::at(Point::new(x, y)),
                vec![],
                SymbolRole::Interruption { name: name.into() },
            )?;
            order.push(s);
        }
        Ok(order)
    });
    let problems = check(&project);
    let marked: Vec<AnyId> = problems.iter().map(|p| p.at[0]).collect();
    let expected: Vec<AnyId> = [order[3], order[2], order[1], order[0]]
        .into_iter()
        .map(AnyId::from)
        .collect();
    assert_eq!(marked, expected);
    assert_eq!(problems[0].position, Some(Point::new(10.0, 5.0)));
    // The same project checks the same way every time.
    assert_eq!(check(&project), problems);
}
