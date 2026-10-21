//! Bench packet: circuit sheets and the cover.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Wire};
use novo_reports::packet::{CoverLine, circuit_sheets, cover, cover_notes};
use novo_reports::{TICK_BOX, bench_jobs};
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
        for (i, id) in wires.into_iter().enumerate() {
            tx.update::<Wire>(id, |w| {
                w.cross_section_mm2 = Some(1.5);
                w.color = Some("BK".into());
                w.number = Some((i + 1).to_string());
            })?;
        }
        Ok(())
    })
    .expect("sizes");
    doc
}

#[test]
fn every_wire_is_on_exactly_one_circuit_sheet_with_its_ticks() {
    let doc = cabinet();
    let jobs = bench_jobs(doc.project());
    assert!(!jobs.is_empty());
    let out = circuit_sheets(doc.project(), &jobs);
    assert!(!out.sections.is_empty());
    let wire_rows: usize = out
        .sections
        .iter()
        .map(|s| {
            s.rows
                .iter()
                .filter(|r| r.cells[1].to_string() == "Wire")
                .count()
        })
        .sum();
    assert_eq!(wire_rows, jobs.len(), "no wire missing, none twice");
    for s in &out.sections {
        assert!(s.key.starts_with("circuit:"));
        // Devices first, then wires, then the four tests and the signature.
        let kinds: Vec<String> = s.rows.iter().map(|r| r.cells[1].to_string()).collect();
        let first_wire = kinds.iter().position(|k| k == "Wire");
        let last_fit = kinds.iter().rposition(|k| k == "Fit");
        if let (Some(w), Some(f)) = (first_wire, last_fit) {
            assert!(f < w, "devices are fitted before the wires");
        }
        assert_eq!(kinds.iter().filter(|k| *k == "Test").count(), 4);
        assert_eq!(kinds.last().map(String::as_str), Some("Signed"));
        for r in s.rows.iter().filter(|r| r.cells[1].to_string() != "Signed") {
            assert_eq!(r.cells[0].to_string(), TICK_BOX);
        }
    }
}

#[test]
fn a_device_is_fitted_on_one_sheet_only() {
    let doc = cabinet();
    let jobs = bench_jobs(doc.project());
    let out = circuit_sheets(doc.project(), &jobs);
    let mut tags: Vec<String> = out
        .sections
        .iter()
        .flat_map(|s| s.rows.iter())
        .filter(|r| r.cells[1].to_string() == "Fit")
        .map(|r| r.cells[2].to_string())
        .collect();
    let n = tags.len();
    tags.sort();
    tags.dedup();
    assert_eq!(tags.len(), n, "a device twice on the sheets: {tags:?}");
    assert!(n >= 2, "the breaker and the contactor are fitted");
}

#[test]
fn estimated_lengths_carry_a_tilde_and_the_title_counts_them() {
    let doc = cabinet();
    let mut jobs = bench_jobs(doc.project());
    for j in &mut jobs {
        j.basis = "routed".into();
    }
    jobs[0].basis = "estimate".into();
    jobs[0].cut_mm = 480.0;
    let out = circuit_sheets(doc.project(), &jobs);
    let cuts: Vec<String> = out
        .sections
        .iter()
        .flat_map(|s| s.rows.iter())
        .filter(|r| r.cells[1].to_string() == "Wire")
        .map(|r| r.cells[7].to_string())
        .collect();
    assert!(cuts.iter().any(|c| c == "~480"), "{cuts:?}");
    assert!(out.sections.iter().any(|s| s.title.contains("estimates ~")));
    let notes = cover_notes(&jobs);
    assert!(notes[0].contains("1 estimate (~"), "{}", notes[0]);
}

#[test]
fn the_cover_lists_what_it_is_given_and_never_claims_exactness_by_itself() {
    let doc = cabinet();
    let jobs = bench_jobs(doc.project());
    let lines = vec![
        CoverLine::new(
            "Circuit sheets",
            "p. 2",
            "A4, any printer",
            "Exact: read from the drawing",
        ),
        CoverLine::new(
            "Drilling template",
            "1 Drilling template.pdf",
            "Print at 100 %",
            "Estimate: about 0.5 mm on paper",
        ),
    ];
    let out = cover(doc.project(), "Demo", &jobs, &lines, &cover_notes(&jobs));
    let contents = out.section("contents").expect("contents");
    assert_eq!(contents.rows.len(), 2);
    assert_eq!(contents.rows[1].cells[2].to_string(), "Print at 100 %");
    let facts = out.section("facts").expect("facts");
    let wires = facts
        .rows
        .iter()
        .find(|r| r.cells[0].to_string() == "Wires to cut")
        .expect("wire count");
    assert!(
        wires.cells[1]
            .to_string()
            .starts_with(&jobs.len().to_string())
    );
    // The cover names the drawing revision, not the model's edit counter.
    let revision = facts
        .rows
        .iter()
        .find(|r| r.cells[0].to_string() == "Drawing revision")
        .expect("drawing revision");
    assert_eq!(revision.cells[1].to_string(), "none issued yet");
    assert!(
        !facts
            .rows
            .iter()
            .any(|r| r.cells[0].to_string() == "Project revision")
    );
    let read = out.section("read-first").expect("notes");
    assert!(read.rows.len() >= 4);
}

#[test]
fn wires_without_a_size_are_named_on_the_cover_with_the_count() {
    let doc = cabinet();
    let mut jobs = bench_jobs(doc.project());
    let n = jobs.len();
    jobs[0].cross_section_mm2 = None;
    let notes = cover_notes(&jobs);
    assert!(
        notes[0].starts_with(&format!(
            "1 of {n} wires to cut have no cross-section: no ferrule is listed"
        )),
        "{}",
        notes[0]
    );
    for j in &mut jobs {
        j.cross_section_mm2 = Some(1.5);
    }
    assert!(
        cover_notes(&jobs)
            .iter()
            .all(|t| !t.contains("no cross-section"))
    );
}

#[test]
fn the_cover_counts_each_length_basis_and_never_says_every_length_is_routed() {
    let doc = cabinet();
    let mut jobs = bench_jobs(doc.project());
    assert!(jobs.len() >= 5);
    for j in &mut jobs {
        j.basis = "routed".into();
    }
    jobs[0].basis = "cable length".into();
    jobs[1].basis = "routed, no depth".into();
    jobs[2].basis = "to entry + stated outside".into();
    jobs[3].basis = "free air".into();
    let n = jobs.len();
    let note = &cover_notes(&jobs)[0];
    assert!(note.contains(&format!("{n} in all")), "{note}");
    assert!(
        note.contains(&format!("{} routed along the ducts", n - 4)),
        "{note}"
    );
    assert!(note.contains("1 routed without duct depth"), "{note}");
    assert!(note.contains("1 from a stated cable length"), "{note}");
    assert!(note.contains("1 with the outside part stated"), "{note}");
    assert!(note.contains("1 estimate (~"), "{note}");
    assert!(!note.contains("Every wire length"), "{note}");
    assert!(!note.contains("exact"), "{note}");
    // Only a stated cable length: no estimate is claimed, and no routing either.
    for j in &mut jobs {
        j.basis = "cable length".into();
    }
    let note = &cover_notes(&jobs)[0];
    assert!(note.contains("from a stated cable length") && !note.contains("estimate"));
    assert!(!note.contains("routed along"), "{note}");
}

#[test]
fn unset_size_and_colour_print_as_not_set() {
    let doc = cabinet();
    let mut jobs = bench_jobs(doc.project());
    jobs[0].cross_section_mm2 = None;
    jobs[0].colour = String::new();
    let out = circuit_sheets(doc.project(), &jobs);
    let wire = out
        .sections
        .iter()
        .flat_map(|s| s.rows.iter())
        .filter(|r| r.cells[1].to_string() == "Wire")
        .find(|r| r.cells[5].to_string() == "not set")
        .expect("a row with no size");
    assert_eq!(wire.cells[6].to_string(), "not set");
}

#[test]
fn the_cover_names_what_each_wire_count_counts() {
    let doc = cabinet();
    let jobs = bench_jobs(doc.project());
    let set = novo_reports::WireSet::of(doc.project());
    assert_eq!(set.to_cut, jobs.len());
    let out = cover(doc.project(), "Test", &jobs, &[], &cover_notes(&jobs));
    let facts: Vec<String> = out
        .sections
        .iter()
        .filter(|s| s.key == "facts")
        .flat_map(|s| s.rows.iter())
        .map(|r| format!("{} | {}", r.cells[0], r.cells[1]))
        .collect();
    let to_cut = facts
        .iter()
        .find(|f| f.starts_with("Wires to cut"))
        .unwrap();
    assert!(to_cut.contains(&jobs.len().to_string()), "{to_cut}");
    let drawn = facts
        .iter()
        .find(|f| f.starts_with("Wires as drawn"))
        .unwrap();
    assert!(drawn.contains(&set.drawn.to_string()), "{drawn}");
    assert!(
        drawn.contains(&format!("{} wires to cut", set.to_cut)),
        "{drawn}"
    );
}
