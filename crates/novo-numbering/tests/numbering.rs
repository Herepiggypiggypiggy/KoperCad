//! Numbering engine acceptance: schemes, stability across edits, renumber
//! with undo, manual locks, conflict detection and the reports that read
//! the numbers.

use novo_core::{
    CableCore, DeviceId, DeviceTag, Document, Endpoint, PageId, Pin, Placement, Point,
    PotentialKind, Result, SymbolId, SymbolRole, Tx, WireId,
};
use novo_numbering::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn block(tx: &mut Tx<'_>, page: PageId, device: DeviceId, x: f64, y: f64) -> Result<SymbolId> {
    let pins = (0..3)
        .map(|i| Pin::new(i.to_string(), 8.0 * f64::from(i), 0.0))
        .collect();
    tx.place_symbol(
        page,
        "test/block",
        Placement::at(Point::new(x, y)),
        pins,
        SymbolRole::Device { device, main: true },
    )
}

struct Scene {
    /// Three wires between `-K1` (top) and `-M1` (bottom).
    wires: Vec<WireId>,
    k1: SymbolId,
    m1: SymbolId,
}

fn scene(tx: &mut Tx<'_>) -> Result<Scene> {
    let page = tx.add_page("1")?;
    let k = tx.add_device(tag("-K1"), "Contactor")?;
    let m = tx.add_device(tag("-M1"), "Motor")?;
    let k1 = block(tx, page, k, 40.0, 20.0)?;
    let m1 = block(tx, page, m, 40.0, 100.0)?;
    let mut wires = Vec::new();
    for i in 0..3u16 {
        wires.push(tx.connect(Endpoint::pin(k1, i), Endpoint::pin(m1, i))?);
    }
    Ok(Scene { wires, k1, m1 })
}

fn numbers(doc: &Document, wires: &[WireId]) -> Vec<Option<String>> {
    wires
        .iter()
        .map(|&w| doc.project().wire(w).unwrap().number.clone())
        .collect()
}

fn run(doc: &mut Document, scheme: &Scheme, mode: Mode) -> Plan {
    let p = plan(doc.project(), scheme, mode, &Scope::Project);
    doc.transact("Number wires", |tx| p.apply(tx)).unwrap();
    p
}

#[test]
fn fill_numbers_every_net_in_reading_order_and_is_stable() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    let first = run(&mut doc, &scheme, Mode::Fill);
    assert_eq!(first.changes.len(), 3);
    assert_eq!(
        numbers(&doc, &s.wires),
        [Some("1".into()), Some("2".into()), Some("3".into())]
    );
    // A second run has nothing to do.
    let second = plan(doc.project(), &scheme, Mode::Fill, &Scope::Project);
    assert!(second.is_empty(), "{:?}", second.changes);
    assert_eq!(second.summary(), "Nothing to change, 3 unchanged");
}

#[test]
fn a_small_edit_numbers_only_the_new_wire() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    run(&mut doc, &scheme, Mode::Fill);
    // Delete the middle wire, draw a new one to the left of everything.
    doc.transact("Edit", |tx| {
        tx.remove(s.wires[1])?;
        let a = tx.connect(Endpoint::pin(s.k1, 1), Endpoint::pin(s.m1, 1))?;
        let _ = a;
        Ok(())
    })
    .unwrap();
    let p = run(&mut doc, &scheme, Mode::Fill);
    assert_eq!(p.changes.len(), 1, "only the new wire");
    assert_eq!(p.changes[0].new.as_deref(), Some("4"), "2 stays retired");
    assert_eq!(
        numbers(&doc, &[s.wires[0], s.wires[2]]),
        [Some("1".into()), Some("3".into())]
    );
}

#[test]
fn reuse_gaps_hands_out_a_freed_number() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let mut scheme = Scheme::new(SchemeKind::Sequential);
    scheme.reuse_gaps = true;
    run(&mut doc, &scheme, Mode::Fill);
    doc.transact("Edit", |tx| {
        tx.remove(s.wires[1])?;
        tx.connect(Endpoint::pin(s.k1, 1), Endpoint::pin(s.m1, 1))
            .map(|_| ())
    })
    .unwrap();
    let p = run(&mut doc, &scheme, Mode::Fill);
    assert_eq!(p.changes[0].new.as_deref(), Some("2"));
}

#[test]
fn joining_two_numbered_nets_keeps_one_number_and_frees_the_other() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    // Two more pins to bridge pin 0 and pin 1 of the contactor by a wire.
    let scheme = Scheme::new(SchemeKind::Sequential);
    run(&mut doc, &scheme, Mode::Fill);
    let (bridge, _) = doc
        .transact("Bridge", |tx| {
            tx.connect(Endpoint::pin(s.k1, 0), Endpoint::pin(s.k1, 1))
        })
        .unwrap();
    // Nets 1 and 2 are now one; the bridge is new.
    let p = run(&mut doc, &scheme, Mode::Fill);
    let n = numbers(&doc, &[s.wires[0], s.wires[1], bridge]);
    assert_eq!(n[0], n[1]);
    assert_eq!(n[1], n[2]);
    assert_eq!(n[0].as_deref(), Some("1"), "the earlier number survives");
    assert!(p.groups.iter().any(|g| g.reason == Reason::Healed));
    assert!(
        check(doc.project(), &scheme)
            .iter()
            .all(|c| c.kind != ConflictKind::Split)
    );
}

#[test]
fn by_page_and_by_column_build_numbers_from_position() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let mut scheme = Scheme::new(SchemeKind::ByPage);
    scheme.separator = "-".into();
    scheme.digits = 2;
    run(&mut doc, &scheme, Mode::Fill);
    assert_eq!(
        numbers(&doc, &s.wires),
        [
            Some("1-01".into()),
            Some("1-02".into()),
            Some("1-03".into())
        ]
    );
    let col = Scheme::new(SchemeKind::ByColumn);
    let p = plan(doc.project(), &col, Mode::Renumber, &Scope::Project);
    let first = p.changes[0].new.clone().unwrap();
    assert!(first.starts_with("1.") && first.ends_with(".1"), "{first}");
    assert_eq!(first.matches('.').count(), 2);
}

#[test]
fn by_potential_names_nets_after_what_they_carry() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    doc.execute(novo_core::Command::AssignPotentials {
        wires: s.wires.clone(),
        potentials: vec![
            novo_core::PotentialSpec {
                name: "L1".into(),
                kind: PotentialKind::Line,
                voltage: Some(230.0),
            },
            novo_core::PotentialSpec {
                name: "PE".into(),
                kind: PotentialKind::ProtectiveEarth,
                voltage: None,
            },
            novo_core::PotentialSpec {
                name: "PE".into(),
                kind: PotentialKind::ProtectiveEarth,
                voltage: None,
            },
        ],
    })
    .ok();
    let scheme = Scheme::new(SchemeKind::ByPotential);
    let p = run(&mut doc, &scheme, Mode::Fill);
    let n = numbers(&doc, &s.wires);
    assert_eq!(n[0].as_deref(), Some("L1.1"), "{:?}", p.changes);
    assert_eq!(n[1].as_deref(), Some("PE"));
    assert_eq!(n[2].as_deref(), Some("PE"), "PE repeats by design");
    assert!(
        check(doc.project(), &scheme)
            .iter()
            .all(|c| c.kind != ConflictKind::Duplicate)
    );
}

#[test]
fn by_cable_uses_cable_and_core() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    doc.transact("Cable", |tx| {
        let c = tx.add_cable(tag("-W1"), vec![CableCore::new("BK"), CableCore::new("BU")])?;
        tx.assign_core(c, 0, Some(s.wires[0]))?;
        tx.assign_core(c, 1, Some(s.wires[1]))
    })
    .unwrap();
    run(&mut doc, &Scheme::new(SchemeKind::ByCable), Mode::Fill);
    let n = numbers(&doc, &s.wires);
    assert_eq!(n[0].as_deref(), Some("W1.BK"));
    assert_eq!(n[1].as_deref(), Some("W1.BU"));
    assert_eq!(
        n[2].as_deref(),
        Some("1.1"),
        "no core falls back to by page"
    );
}

#[test]
fn renumber_restarts_but_keeps_locked_numbers_and_undoes() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    run(&mut doc, &scheme, Mode::Fill);
    doc.transact("Type", |tx| {
        set_number(tx, s.wires[0], "W-PUMP").map(|_| ())
    })
    .unwrap();
    assert!(is_locked(doc.project(), s.wires[0]));
    let p = run(&mut doc, &scheme, Mode::Renumber);
    let n = numbers(&doc, &s.wires);
    assert_eq!(n[0].as_deref(), Some("W-PUMP"), "locked stays");
    assert_eq!(n[1].as_deref(), Some("1"));
    assert_eq!(n[2].as_deref(), Some("2"));
    assert_eq!(p.changed_groups().count(), 2);
    doc.undo().unwrap();
    assert_eq!(
        numbers(&doc, &s.wires)[1].as_deref(),
        Some("2"),
        "one undo step"
    );
}

#[test]
fn typing_a_used_number_is_refused_and_unlock_releases() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    run(&mut doc, &Scheme::new(SchemeKind::Sequential), Mode::Fill);
    assert!(
        doc.transact("Type", |tx| set_number(tx, s.wires[0], "2").map(|_| ()))
            .is_err()
    );
    doc.transact("Type", |tx| set_number(tx, s.wires[0], "X9").map(|_| ()))
        .unwrap();
    doc.transact("Unlock", |tx| unlock(tx, s.wires[0])).unwrap();
    assert!(!is_locked(doc.project(), s.wires[0]));
    let strict = plan(
        doc.project(),
        &Scheme::new(SchemeKind::Sequential),
        Mode::FillStrict,
        &Scope::Project,
    );
    assert_eq!(
        strict.changes.len(),
        1,
        "X9 is off the scheme once unlocked"
    );
}

#[test]
fn conflicts_are_found_and_the_plan_shows_what_remains() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    doc.transact("Dirty", |tx| {
        tx.update::<novo_core::Wire>(s.wires[0], |w| w.number = Some("7".into()))?;
        tx.update::<novo_core::Wire>(s.wires[1], |w| w.number = Some("7".into()))
    })
    .unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    let found = check(doc.project(), &scheme);
    assert!(
        found
            .iter()
            .any(|c| c.kind == ConflictKind::Duplicate && c.severity == Severity::Error)
    );
    assert!(found.iter().any(|c| c.kind == ConflictKind::Unnumbered));
    // Fill replaces the later duplicate and numbers the third wire.
    let p = run(&mut doc, &scheme, Mode::Fill);
    assert!(p.conflicts.is_empty(), "{:?}", p.conflicts);
    assert!(check(doc.project(), &scheme).is_empty());
    let n = numbers(&doc, &s.wires);
    assert_eq!(
        n[0].as_deref(),
        Some("7"),
        "the first in reading order keeps it"
    );
}

#[test]
fn a_stale_plan_is_refused() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let p = plan(
        doc.project(),
        &Scheme::default(),
        Mode::Fill,
        &Scope::Project,
    );
    doc.transact("Other", |tx| {
        tx.connect(Endpoint::pin(s.k1, 0), Endpoint::pin(s.m1, 2))
            .map(|_| ())
    })
    .unwrap();
    assert!(doc.transact("Apply", |tx| p.apply(tx)).is_err());
}

#[test]
fn scope_limits_what_a_plan_changes() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let p = plan(
        doc.project(),
        &Scheme::default(),
        Mode::Fill,
        &Scope::Wires(vec![s.wires[2]]),
    );
    assert_eq!(p.changes.len(), 1);
    assert_eq!(p.changes[0].wire, s.wires[2]);
}

#[test]
fn the_scheme_is_stored_in_the_project() {
    let mut doc = Document::default();
    let mut scheme = Scheme::new(SchemeKind::ByPotential);
    scheme.prefix = "W".into();
    scheme.step = 10;
    scheme.digits = 3;
    scheme.reuse_gaps = true;
    scheme.bare_potentials = false;
    doc.transact("Scheme", |tx| scheme.store(tx)).unwrap();
    assert_eq!(Scheme::load(doc.project()), scheme);
    assert_eq!(
        Scheme::load(Document::default().project()),
        Scheme::default()
    );
}

#[test]
fn the_wire_list_shows_the_numbers() {
    use novo_reports::Report;
    let mut doc = Document::default();
    let (_s, _) = doc.transact("Draw", scene).unwrap();
    run(&mut doc, &Scheme::new(SchemeKind::Sequential), Mode::Fill);
    let out = novo_reports::WireList.generate(doc.project());
    let section = &out.sections[0];
    let col = section
        .columns
        .iter()
        .position(|c| c.key == "number")
        .unwrap();
    let shown: Vec<String> = section
        .rows
        .iter()
        .map(|r| format!("{:?}", r.cells[col]))
        .collect();
    assert_eq!(shown.len(), 3, "{shown:?}");
    for want in ["1", "2", "3"] {
        assert!(
            shown.iter().any(|s| s.contains(&format!("\"{want}\""))),
            "{want} in {shown:?}"
        );
    }
}

#[test]
fn each_phase_strand_is_numbered_top_to_bottom_before_the_next() {
    let mut doc = Document::default();
    novo_tests::motor_starter(&mut doc).unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    let p = plan(doc.project(), &scheme, Mode::Renumber, &Scope::Project);
    let order: Vec<(String, String)> = p
        .groups
        .iter()
        .filter_map(|g| Some((g.potential.clone()?, g.new.clone()?)))
        .collect();
    // The three phases are all there, so the checks below cannot pass empty.
    for phase in ["L1", "L2", "L3"] {
        assert!(
            order.iter().any(|(p, _)| p == phase),
            "{phase} in {order:?}"
        );
    }
    // Potentials in the order their groups were numbered: each phase once.
    let mut seen: Vec<&str> = Vec::new();
    for (potential, _) in &order {
        if seen.last() != Some(&potential.as_str()) {
            assert!(!seen.contains(&potential.as_str()), "{order:?}");
            seen.push(potential);
        }
    }
    let numbers: Vec<u32> = order.iter().map(|(_, n)| n.parse().unwrap()).collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}

/// A contactor, an overload relay whose heaters conduct (permanent bridges)
/// and a motor, wired in a line: three phases, six wires.
fn through_overload(tx: &mut Tx<'_>) -> Result<Vec<WireId>> {
    let page = tx.add_page("1")?;
    let [k, f, m] = [("-K1", "Contactor"), ("-F2", "Overload"), ("-M1", "Motor")]
        .map(|(t, n)| tx.add_device(tag(t), n).unwrap());
    let k1 = block(tx, page, k, 40.0, 20.0)?;
    let f2 = block(tx, page, f, 40.0, 60.0)?;
    let m1 = block(tx, page, m, 40.0, 100.0)?;
    tx.update::<novo_core::SymbolInstance>(f2, |s| s.bridges = vec![vec![0, 1]])?;
    // Pin 0 of the relay is the heater input, pin 1 its output.
    Ok(vec![
        tx.connect(Endpoint::pin(k1, 0), Endpoint::pin(f2, 0))?,
        tx.connect(Endpoint::pin(f2, 1), Endpoint::pin(m1, 0))?,
    ])
}

#[test]
fn a_wire_into_and_out_of_a_bridged_device_get_different_numbers() {
    let mut doc = Document::default();
    let (wires, _) = doc.transact("Draw", through_overload).unwrap();
    // The netlist conducts through the heater: one electrical node.
    let net = doc.project().netlist();
    assert!(
        net.connected(
            Endpoint::pin(
                doc.project()
                    .wire(wires[0])
                    .unwrap()
                    .from
                    .as_pin()
                    .unwrap()
                    .symbol,
                0
            ),
            doc.project().wire(wires[1]).unwrap().to
        )
    );
    let scheme = Scheme::new(SchemeKind::Sequential);
    run(&mut doc, &scheme, Mode::Fill);
    let n = numbers(&doc, &wires);
    assert!(n[0].is_some() && n[1].is_some());
    assert_ne!(n[0], n[1], "two wires on the bench, two numbers");
    // Stable on a second run, and no conflict is reported.
    let again = run(&mut doc, &scheme, Mode::Fill);
    assert!(again.is_empty());
    assert!(check(doc.project(), &scheme).is_empty());
}

#[test]
fn a_net_on_a_revision_locked_page_is_left_alone_and_reported() {
    let mut doc = Document::default();
    let (s, _) = doc.transact("Draw", scene).unwrap();
    let scheme = Scheme::new(SchemeKind::Sequential);
    let page = doc.project().wire(s.wires[0]).unwrap().page;
    doc.transact("Lock", |tx| tx.lock_page(page, "A")).unwrap();
    let p = plan(doc.project(), &scheme, Mode::Fill, &Scope::Project);
    assert!(p.is_empty(), "nothing may change on a locked page");
    assert!(p.groups.iter().all(|g| g.reason == Reason::PageLocked));
    assert!(p.groups.iter().all(|g| g.page_lock.as_deref() == Some("A")));
    assert_eq!(p.changed_groups().count(), 0);
    assert!(p.summary().contains("3 on locked pages left alone"));
    // Applying the empty plan is harmless; the model would refuse real edits.
    doc.transact("Number", |tx| p.apply(tx)).unwrap();
    assert_eq!(numbers(&doc, &s.wires), [None, None, None]);
    // Unnumbered nets still show up as conflicts so they are not forgotten.
    assert!(!p.conflicts.is_empty());
}
