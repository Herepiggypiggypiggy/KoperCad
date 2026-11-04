//! Wire end terminations, the crimp list and the sleeve text.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Value, Wire};
use novo_reports::{
    FerruleStandard, Labels, Report, SLEEVE_TEXT_KEY, TERMINATION_A_KEY, TERMINATION_B_KEY,
    Termination, TerminationKind, WireCut, bare_ends, crimp_list, wire_cut_jobs,
};
use novo_tests::motor_starter;

fn cabinet() -> Document {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    doc.transact("Cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 600.0,
                margin: 10.0,
            }),
        )?;
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
        tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        Ok(())
    })
    .expect("layout");
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Sizes", |tx| {
        for id in wires {
            tx.update::<Wire>(id, |w| {
                w.cross_section_mm2 = Some(1.5);
                w.color = Some("BK".into());
            })?;
        }
        Ok(())
    })
    .expect("sizes");
    doc
}

fn text(s: &str) -> Option<Value> {
    Some(Value::Text(s.into()))
}

#[test]
fn stated_text_is_read_into_a_kind_and_a_size() {
    let p = |s: &str| Termination::parse(s).unwrap();
    assert_eq!(p("ring M5"), Termination::new(TerminationKind::Ring, "M5"));
    assert_eq!(p("Ring lug M5").stored(), "ring M5");
    assert_eq!(p("fork M4").kind, TerminationKind::Fork);
    assert_eq!(p("Spade tab 6.3").size, "6.3");
    assert_eq!(p("pin").kind, TerminationKind::Pin);
    assert_eq!(p("none").kind, TerminationKind::Bare);
    assert_eq!(p("ferrule A1.5-10").size, "A1.5-10");
    let other = p("crocodile clip");
    assert_eq!(other.kind, TerminationKind::Other);
    assert_eq!(other.text(), "crocodile clip");
    assert_eq!(Termination::parse("  "), None);
    // What is stored reads back the same.
    for kind in TerminationKind::PICKABLE {
        let t = Termination::new(kind, "x");
        assert_eq!(Termination::parse(&t.stored()), Some(t));
    }
}

#[test]
fn without_a_statement_ends_keep_the_old_proposal() {
    let doc = cabinet();
    for j in wire_cut_jobs(doc.project()) {
        assert_eq!(j.end_a.termination.kind, TerminationKind::Ferrule);
        assert_eq!(j.end_a.termination.size, "A1.5-10");
        assert_eq!(j.sleeve, j.number, "the number is printed by default");
    }
}

#[test]
fn a_stated_end_replaces_the_ferrule_at_that_end_only() {
    let mut doc = cabinet();
    let first = wire_cut_jobs(doc.project())[0].wires[0];
    doc.transact("Ring on A, bare on B", |tx| {
        tx.set_property(first, TERMINATION_A_KEY, text("ring M5"))?;
        tx.set_property(first, TERMINATION_B_KEY, text("bare"))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let j = jobs.iter().find(|j| j.wires.contains(&first)).unwrap();
    assert_eq!(j.end_a.termination.kind, TerminationKind::Ring);
    assert_eq!(j.end_a.ferrule, None);
    assert_eq!(j.end_a.processing, "Ring lug M5");
    assert_eq!(j.end_a.sleeve_text().as_deref(), Some("Ring lug M5"));
    assert_eq!(j.end_b.termination.kind, TerminationKind::Bare);
    assert_eq!(j.end_b.ferrule, None);
    assert!(j.end_b.processing.starts_with("Strip"));
    assert_eq!(bare_ends(&jobs), 1);
    // The other wires still take ferrules at both ends.
    assert!(
        jobs.iter()
            .filter(|o| !o.wires.contains(&first))
            .all(|o| o.end_a.ferrule.is_some() && o.end_b.ferrule.is_some())
    );
}

#[test]
fn a_stated_ferrule_size_wins_over_the_proposed_one() {
    let mut doc = cabinet();
    let first = wire_cut_jobs(doc.project())[0].wires[0];
    doc.transact("Ferrule", |tx| {
        tx.set_property(first, TERMINATION_B_KEY, text("ferrule A1.5-8"))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let j = jobs.iter().find(|j| j.wires.contains(&first)).unwrap();
    assert_eq!(j.end_b.ferrule.as_deref(), Some("A1.5-8"));
    assert_eq!(j.end_a.ferrule.as_deref(), Some("A1.5-10"));
}

#[test]
fn the_crimp_list_counts_lugs_and_ferrules_per_size_with_spares() {
    let mut doc = cabinet();
    let jobs0 = wire_cut_jobs(doc.project());
    let (a, b) = (jobs0[0].wires[0], jobs0[1].wires[0]);
    doc.transact("Lugs", |tx| {
        tx.set_property(a, TERMINATION_A_KEY, text("ring M5"))?;
        tx.set_property(b, TERMINATION_A_KEY, text("ring M5"))?;
        tx.set_property(b, TERMINATION_B_KEY, text("fork M4"))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let standard = FerruleStandard::default();
    let list = crimp_list(&jobs, &standard);
    let ring = list
        .iter()
        .find(|c| c.kind == TerminationKind::Ring)
        .expect("ring row");
    assert_eq!((ring.size.as_str(), ring.ends, ring.pieces), ("M5", 2, 2));
    assert_eq!(ring.mm2, Some(1.5));
    // 10 % spares round up: 2 -> 3.
    assert_eq!(ring.to_prepare, 3);
    let fork = list
        .iter()
        .find(|c| c.kind == TerminationKind::Fork)
        .unwrap();
    assert_eq!(fork.ends, 1);
    // Ferrules come first and no end is counted twice.
    assert_eq!(list[0].kind, TerminationKind::Ferrule);
    let ends: usize = list.iter().map(|c| c.ends).sum();
    assert_eq!(ends, jobs.len() * 2);
    // The report has the section, and the CSV the new columns.
    let out = WireCut.generate(doc.project());
    let section = out
        .sections
        .iter()
        .find(|s| s.key == "crimps")
        .expect("crimps");
    assert_eq!(section.rows.len(), list.len());
}

#[test]
fn a_bare_end_is_not_on_the_crimp_list() {
    let mut doc = cabinet();
    let w = wire_cut_jobs(doc.project())[0].wires[0];
    doc.transact("Bare", |tx| {
        tx.set_property(w, TERMINATION_A_KEY, text("bare"))?;
        tx.set_property(w, TERMINATION_B_KEY, text("bare"))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let ends: usize = crimp_list(&jobs, &FerruleStandard::default())
        .iter()
        .map(|c| c.ends)
        .sum();
    assert_eq!(ends, jobs.len() * 2 - 2);
}

#[test]
fn the_sleeve_text_prints_while_the_wire_number_stays() {
    let mut doc = cabinet();
    let jobs0 = wire_cut_jobs(doc.project());
    let w = jobs0[0].wires[0];
    let number = jobs0[0].number.clone();
    doc.transact("Sleeve", |tx| {
        tx.set_property(w, SLEEVE_TEXT_KEY, text("L1.1 / U1"))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let j = jobs.iter().find(|j| j.wires.contains(&w)).unwrap();
    assert_eq!(j.sleeve, "L1.1 / U1");
    assert_eq!(j.number, number, "the permanent number is unchanged");
    assert!(j.end_a.label.starts_with("L1.1 / U1"));
    // Sleeve and cut-piece labels print the sleeve text; the wire list keeps the number.
    let labels = Labels.generate(doc.project());
    for key in ["wires", "cuts"] {
        let s = labels.sections.iter().find(|s| s.key == key).unwrap();
        assert!(
            s.rows.iter().any(|r| r.cells[0].to_string() == "L1.1 / U1"),
            "{key} labels"
        );
    }
}
