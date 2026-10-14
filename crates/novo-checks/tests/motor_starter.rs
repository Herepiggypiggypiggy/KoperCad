//! WS-14 done-criterion: every deliberate error in a test project is
//! flagged, here a broken copy of the Gate 1 motor starter, while the clean
//! starter yields nothing at all.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use novo_checks::{ProblemKind, Severity, Summary, check};
use novo_core::*;
use novo_tests::{MotorStarter, motor_starter, stress_page};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).unwrap();
    (doc, handles)
}

#[test]
fn clean_motor_starter_has_no_problems() {
    let (doc, _) = starter();
    let problems = check(doc.project());
    // Every device and the cable carry a part in the fixture, so not even
    // an info is expected.
    assert!(problems.is_empty(), "{problems:#?}");
    assert_eq!(Summary::of(&problems).pill_text(), "No issues");
}

/// The objects each deliberate error should mark.
struct Broken {
    lamp: SymbolId,
    s0_sym: SymbolId,
    k1_aux: SymbolId,
    k1_main: SymbolId,
    k1_coil: SymbolId,
    f2_aux: SymbolId,
    lone: SymbolId,
    shorted_wire: WireId,
    zero_wire: WireId,
    extra_terminal: TerminalId,
}

fn break_it(doc: &mut Document, ms: &MotorStarter) -> Broken {
    let tag = |t: &str| DeviceTag::parse(t).unwrap();
    let (broken, _) = doc
        .transact("Break the starter", |tx| {
            let page = ms.control_page;
            // 1 + 10. A lamp without a part, its X2 left unconnected.
            let h1 = tx.add_device(tag("=A1+CAB-H1"), "Running")?;
            let lamp = tx.place_symbol(
                page,
                "iec/signals/lamp",
                Placement::at(Point::new(88.0, 136.0)),
                vec![Pin::new("X1", 0.0, 0.0), Pin::new("X2", 0.0, 16.0)],
                SymbolRole::Device {
                    device: h1,
                    main: true,
                },
            )?;
            tx.connect(Endpoint::junction(ms.hold_join), Endpoint::pin(lamp, 0))?;
            // 2. Stop button renamed to the start button's tag.
            tx.set_tag(ms.s0, tag("=A1+CAB-S1"))?;
            // 3. The self-hold contact given to a device without a coil.
            let k2 = tx.add_device(tag("=A1+CAB-K2"), "Auxiliary relay")?;
            let part = tx.add_part("Siemens", "3RH2122-1AP00")?;
            tx.assign_part(k2, part)?;
            tx.set_role(
                ms.k1_aux,
                SymbolRole::Device {
                    device: k2,
                    main: false,
                },
            )?;
            // 4. The main contacts of -K1 marked as a second main function.
            tx.set_role(
                ms.k1_main,
                SymbolRole::Device {
                    device: ms.k1,
                    main: true,
                },
            )?;
            // 5. The overload relay has no NC contact, yet one is drawn.
            tx.set_property(ms.f2, "contacts.nc", Some(0i64.into()))?;
            // 6. An interruption point whose partner page was never drawn.
            let lone = tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                Placement::at(Point::new(72.0, 184.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption { name: "N.3".into() },
            )?;
            tx.connect(Endpoint::pin(lone, 0), Endpoint::pin(ms.k1_coil, 1))?;
            // 7. The control supply wire is also given N: L meets N.
            let shorted_wire = tx
                .project()
                .wires_at(Endpoint::pin(ms.f3_sym, 0))
                .next()
                .expect("feed wire");
            tx.set_wire_potential(shorted_wire, Some(ms.n))?;
            // 8. A junction dropped onto -S1:13 and wired to it.
            let j = tx.add_junction(page, Point::new(40.0, 104.0))?;
            let zero_wire = tx.connect(Endpoint::junction(j), Endpoint::pin(ms.s1_sym, 0))?;
            // 9. A second -X1:3 with the same-name rule switched off (new
            //    projects join same-name terminals).
            tx.set_info(|i| i.rules.terminals.same_name = SameName::Off)?;
            let extra_terminal = tx.add_terminal(ms.x1, "3")?;
            Ok(Broken {
                lamp,
                s0_sym: ms.s0_sym,
                k1_aux: ms.k1_aux,
                k1_main: ms.k1_main,
                k1_coil: ms.k1_coil,
                f2_aux: ms.f2_aux,
                lone,
                shorted_wire,
                zero_wire,
                extra_terminal,
            })
        })
        .unwrap();
    broken
}

#[test]
fn every_deliberate_error_is_flagged_exactly_once() {
    let (mut doc, ms) = starter();
    let b = break_it(&mut doc, &ms);
    let problems = check(doc.project());

    let mut by_kind: BTreeMap<ProblemKind, Vec<&novo_checks::Problem>> = BTreeMap::new();
    for p in &problems {
        by_kind.entry(p.kind).or_default().push(p);
    }
    for kind in ProblemKind::ALL {
        let found = by_kind.get(&kind).map_or(0, Vec::len);
        assert_eq!(found, 1, "{kind:?} flagged {found} times: {problems:#?}");
    }
    assert_eq!(problems.len(), ProblemKind::ALL.len());

    let one = |kind| by_kind[&kind][0];
    let ids = |list: &[AnyId]| list.to_vec();
    assert_eq!(
        ids(&one(ProblemKind::UnconnectedPin).at),
        vec![AnyId::from(b.lamp)]
    );
    assert_eq!(
        one(ProblemKind::UnconnectedPin).message,
        "=A1+CAB-H1: X2 not connected"
    );
    let dup = one(ProblemKind::DuplicateTag);
    assert!(dup.at.contains(&b.s0_sym.into()));
    assert_eq!(dup.message, "=A1+CAB-S1 is used by 2 devices");
    assert_eq!(
        one(ProblemKind::NoMainFunction).at,
        vec![AnyId::from(b.k1_aux)]
    );
    assert_eq!(
        one(ProblemKind::SeveralMainFunctions).at,
        vec![AnyId::from(b.k1_main), AnyId::from(b.k1_coil)],
        "main circuit page first"
    );
    assert_eq!(
        one(ProblemKind::TooManyContacts).at,
        vec![AnyId::from(b.f2_aux)]
    );
    assert_eq!(
        one(ProblemKind::LoneInterruption).at,
        vec![AnyId::from(b.lone)]
    );
    let short = one(ProblemKind::PotentialConflict);
    assert!(short.at.contains(&b.shorted_wire.into()));
    assert_eq!(short.message, "L and N are defined on one net: a short");
    assert_eq!(
        one(ProblemKind::ZeroLengthWire).at,
        vec![AnyId::from(b.zero_wire)]
    );
    assert_eq!(
        one(ProblemKind::DuplicateTerminal).at.len(),
        2,
        "the drawn -X1:3 and the new one"
    );
    assert!(
        one(ProblemKind::DuplicateTerminal)
            .at
            .contains(&b.extra_terminal.into())
    );
    assert_eq!(
        one(ProblemKind::MissingPart).message,
        "=A1+CAB-H1 has no part"
    );

    let summary = Summary::of(&problems);
    assert_eq!((summary.errors, summary.warnings, summary.infos), (3, 6, 1));
    assert_eq!(summary.pill_text(), "3 errors · 6 warnings");

    // Reading order: main circuit page, then control page.
    let pages: Vec<Option<PageId>> = problems.iter().map(|p| p.page).collect();
    let first_control = pages
        .iter()
        .position(|p| *p == Some(ms.control_page))
        .unwrap();
    assert!(
        pages[..first_control]
            .iter()
            .all(|p| *p == Some(ms.main_page))
    );
    assert!(problems.iter().all(|p| p.page.is_some()));
}

#[test]
fn undoing_the_damage_clears_every_problem() {
    let (mut doc, ms) = starter();
    break_it(&mut doc, &ms);
    assert!(!check(doc.project()).is_empty());
    doc.undo().unwrap();
    assert!(check(doc.project()).is_empty());
}

#[test]
fn stress_page_checks_in_reasonable_time() {
    // Debug build on CI: a generous bound. Release timing is in
    // `examples/scale.rs` and the handoff notes.
    let mut doc = Document::default();
    stress_page(&mut doc, 10_000).unwrap();
    let start = Instant::now();
    let problems = check(doc.project());
    let took = start.elapsed();
    eprintln!(
        "check, {} symbols: {took:?}",
        doc.project().symbols().count()
    );
    assert!(took < Duration::from_secs(10), "{took:?}");
    let summary = Summary::of(&problems);
    // The ends of the chain are free; the devices have no parts.
    assert_eq!(summary.warnings, 2);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.infos, doc.project().devices().count());
    assert!(
        problems
            .iter()
            .filter(|p| p.severity == Severity::Warning)
            .all(|p| p.kind == ProblemKind::UnconnectedPin)
    );
}
