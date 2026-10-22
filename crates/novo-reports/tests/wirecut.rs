//! Wire production against the Gate 1 motor starter placed in a cabinet.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Value, Wire};
use novo_reports::{
    CsvOptions, END_PROCESSING_KEY, FERRULE_KEY, JOB_COLUMNS, Report, STRIP_LENGTH_KEY, StripBasis,
    WireCut, typical_strip_mm, wire_cut_csv, wire_cut_jobs,
};
use novo_tests::{MotorStarter, motor_starter};

fn cabinet() -> (Document, MotorStarter) {
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
    (doc, h)
}

fn between<'a>(
    jobs: &'a [novo_reports::WireCutJob],
    a: &str,
    b: &str,
) -> &'a novo_reports::WireCutJob {
    jobs.iter()
        .find(|j| j.end_a.designation == a && j.end_b.designation == b)
        .unwrap_or_else(|| panic!("no job {a} to {b}"))
}

#[test]
fn every_connection_is_a_job_cut_with_the_cut_list_allowance() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    assert!(!jobs.is_empty());
    for j in &jobs {
        // A 1.5 mm2 wire gets 10 mm per end, the cut list's ferrule allowance.
        assert_eq!(j.cut_mm, j.length_mm + 20.0, "{j:?}");
        assert_eq!(j.cross_section_mm2, Some(1.5));
        assert_eq!(j.end_a.ferrule.as_deref(), Some("A1.5-10"));
        assert_eq!(j.end_a.strip_mm, Some(typical_strip_mm(1.5)));
        assert_eq!(j.end_a.strip_basis, StripBasis::Typical);
    }
    let routed = between(&jobs, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(routed.basis, "routed, no depth");
    assert_eq!(routed.length_mm, 670.0);
    assert_eq!(routed.cut_mm, 690.0);
    assert!(!routed.ducts.is_empty());
}

#[test]
fn jobs_come_in_machine_batches_longest_first() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    for pair in jobs.windows(2) {
        assert!(
            pair[0].cut_mm >= pair[1].cut_mm,
            "one size and colour: longest first"
        );
    }
}

#[test]
fn a_part_overrides_the_strip_length_and_a_wire_can_drop_its_ferrule() {
    let (mut doc, h) = cabinet();
    let part = doc.project().device(h.k1).unwrap().parts[0];
    let before = wire_cut_jobs(doc.project());
    let f1k1 = between(&before, "=A1+CAB-F1:2", "=A1+CAB-K1:1").wires[0];
    doc.transact("Part", |tx| {
        tx.set_property(part, STRIP_LENGTH_KEY, Some(Value::Float(8.0)))?;
        tx.set_property(f1k1, FERRULE_KEY, Some(Value::Text("none".into())))
    })
    .expect("props");
    let jobs = wire_cut_jobs(doc.project());
    let job = between(&jobs, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(job.end_b.strip_mm, Some(8.0));
    assert_eq!(job.end_b.strip_basis, StripBasis::Part);
    assert_eq!(job.end_b.ferrule, None, "marked without a ferrule");
    assert_eq!(job.end_a.ferrule, None, "the wire says none for both ends");
}

#[test]
fn an_unknown_size_gets_no_strip_length_or_ferrule() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let jobs = wire_cut_jobs(doc.project());
    let bare: Vec<_> = jobs
        .iter()
        .filter(|j| j.cross_section_mm2.is_none())
        .collect();
    assert!(!bare.is_empty());
    for j in bare {
        assert_eq!(j.end_a.strip_mm, None);
        assert_eq!(j.end_a.strip_basis, StripBasis::Unknown);
        assert_eq!(j.end_a.ferrule, None);
    }
}

#[test]
fn labels_name_the_wire_and_where_it_goes() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    for j in &jobs {
        assert!(j.end_a.label.ends_with(&j.end_b.designation), "{j:?}");
        assert!(j.end_b.label.ends_with(&j.end_a.designation), "{j:?}");
    }
}

#[test]
fn the_csv_has_one_row_per_wire_under_the_job_columns() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let csv = wire_cut_csv(
        &jobs,
        &CsvOptions {
            bom: false,
            ..CsvOptions::default()
        },
    );
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], JOB_COLUMNS.join(","));
    assert_eq!(lines.len(), jobs.len() + 1);
    // Semicolon files use decimal commas.
    let semi = wire_cut_csv(
        &jobs,
        &CsvOptions {
            delimiter: ';',
            bom: false,
            ..CsvOptions::default()
        },
    );
    assert!(semi.contains("690") && semi.lines().count() == lines.len());
}

#[test]
fn the_report_adds_up_stock_ferrules_and_duct_fill() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let out = WireCut.generate(doc.project());
    let by = |key: &str| out.section(key).unwrap_or_else(|| panic!("{key}"));
    assert_eq!(by("cuts").rows.len(), jobs.len());
    let pieces: i64 = by("stock")
        .text_rows()
        .iter()
        .map(|r| r[2].parse::<i64>().unwrap())
        .sum();
    // The stock is hookup wire; the cores of a cable are counted with it.
    let cores: i64 = by("cables")
        .text_rows()
        .iter()
        .map(|r| r[1].parse::<i64>().unwrap())
        .sum();
    assert_eq!((pieces + cores) as usize, jobs.len());
    let ferrules: i64 = by("ferrules")
        .text_rows()
        .iter()
        .map(|r| r[1].parse::<i64>().unwrap())
        .sum();
    assert_eq!(ferrules as usize, 2 * jobs.len());
    let ducts = by("ducts").text_rows();
    assert_eq!(ducts.len(), 1);
    assert_eq!(ducts[0][5], "ok");
    assert!(ducts[0][1].parse::<i64>().unwrap() >= 1);
}

#[test]
fn wires_to_a_supply_feed_are_jobs_too() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let feed_ends = |j: &novo_reports::WireCutJob| {
        [&j.end_a, &j.end_b]
            .into_iter()
            .filter(|e| e.designation.ends_with("(feed)"))
            .count()
    };
    let feeds: Vec<_> = jobs.iter().filter(|j| feed_ends(j) == 1).collect();
    assert!(!feeds.is_empty(), "supply and PE wires are on the list");
    assert!(jobs.iter().all(|j| feed_ends(j) <= 1));
    // The protective earth lead to the motor is a wire to cut.
    assert!(
        feeds.iter().any(|j| [&j.end_a, &j.end_b]
            .iter()
            .any(|e| e.designation == "PE (feed)")),
        "{:?}",
        feeds
            .iter()
            .map(|j| (&j.end_a.designation, &j.end_b.designation))
            .collect::<Vec<_>>()
    );
    // The wire list stays the wiring between devices.
    let list = novo_reports::WireList.generate(doc.project());
    let to_feed = list.sections[0]
        .rows
        .iter()
        .any(|r| r.cells.iter().any(|c| c.to_string().ends_with("(feed)")));
    assert!(!to_feed);
}

#[test]
fn two_feeds_in_one_net_never_pair_with_each_other() {
    use novo_core::{Endpoint, Pin, Placement, PotentialKind, SymbolRole};
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        let page = tx.add_page("1")?;
        let device = tx.add_device(DeviceTag::parse("-X9")?, "Block")?;
        let pins = (0..2).map(|i| Pin::new(i.to_string(), 8.0 * f64::from(i), 0.0));
        let block = tx.place_symbol(
            page,
            "test/block",
            Placement::at(Point::new(40.0, 60.0)),
            pins.collect(),
            SymbolRole::Device { device, main: true },
        )?;
        let mut feed = |name: &str, x: f64| {
            let potential = tx.add_potential(name, PotentialKind::Line)?;
            tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                Placement::at(Point::new(x, 20.0)),
                vec![Pin::new("1", 0.0, 8.0)],
                SymbolRole::PotentialFeed { potential },
            )
        };
        let (fa, fb) = (feed("A", 40.0)?, feed("B", 48.0)?);
        // Both feeds meet in a junction above pin 0.
        let j = Endpoint::Junction {
            junction: tx.add_junction(page, Point::new(44.0, 40.0))?,
        };
        tx.connect(Endpoint::pin(fa, 0), j)?;
        tx.connect(Endpoint::pin(fb, 0), j)?;
        tx.connect(j, Endpoint::pin(block, 0))?;
        Ok(())
    })
    .expect("draw");
    let jobs = wire_cut_jobs(doc.project());
    let ends =
        |j: &novo_reports::WireCutJob| (j.end_a.designation.clone(), j.end_b.designation.clone());
    let all: Vec<_> = jobs.iter().map(ends).collect();
    assert!(
        all.iter()
            .all(|(a, b)| !(a.ends_with("(feed)") && b.ends_with("(feed)"))),
        "{all:?}"
    );
    assert_eq!(all.len(), 2, "{all:?}");
    assert!(
        all.contains(&("-X9:0".into(), "A (feed)".into())),
        "{all:?}"
    );
    assert!(
        all.contains(&("-X9:0".into(), "B (feed)".into())),
        "{all:?}"
    );
}

#[test]
fn wires_in_the_same_ducts_are_one_bundle_laid_longest_first() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let routed: Vec<_> = jobs.iter().filter(|j| !j.ducts.is_empty()).collect();
    assert!(!routed.is_empty());
    for j in &routed {
        assert_eq!(j.track, "D1", "{j:?}");
        assert_eq!(j.bundle, Some(1));
        assert_eq!(j.direction, "A>B along track");
    }
    let mut by_seq: Vec<_> = routed
        .iter()
        .map(|j| (j.sequence.unwrap(), j.cut_mm))
        .collect();
    by_seq.sort_by_key(|(s, _)| *s);
    let seqs: Vec<u32> = by_seq.iter().map(|(s, _)| *s).collect();
    assert_eq!(seqs, (1..=routed.len() as u32).collect::<Vec<_>>());
    assert!(by_seq.windows(2).all(|p| p[0].1 >= p[1].1), "{by_seq:?}");
    assert!(routed[0].bundle_text().is_some());
}

#[test]
fn a_wire_that_lies_in_no_duct_has_no_bundle_and_says_so() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let jobs = wire_cut_jobs(doc.project());
    for j in &jobs {
        assert_eq!((j.bundle, j.sequence), (None, None));
        assert_eq!(j.track, "not routed");
        assert_eq!(j.direction, "A>B as listed");
        assert_eq!(j.bundle_text(), None);
    }
}

#[test]
fn end_processing_is_stated_by_the_wire_else_proposed_and_labelled() {
    let (mut doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let j = between(&jobs, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(j.end_a.processing, "Ferrule A1.5-10");
    assert_eq!(j.end_a.processing_basis, StripBasis::Typical);
    let wire = j.wires[0];
    doc.transact("Lug", |tx| {
        tx.set_property(
            wire,
            END_PROCESSING_KEY,
            Some(Value::Text("Ring lug M5".into())),
        )
    })
    .expect("prop");
    let jobs = wire_cut_jobs(doc.project());
    let j = between(&jobs, "=A1+CAB-F1:2", "=A1+CAB-K1:1");
    assert_eq!(j.end_a.processing, "Ring lug M5");
    assert_eq!(j.end_b.processing_basis, StripBasis::Stated);
    assert_eq!(j.end_a.ferrule, None, "a lug replaces the ferrule");
    assert_eq!(j.end_a.sleeve_text().as_deref(), Some("Ring lug M5"));
    assert_eq!(novo_reports::end_basis_text(j), "stated");
}

#[test]
fn the_job_file_and_report_carry_the_production_columns() {
    let (doc, _) = cabinet();
    let jobs = wire_cut_jobs(doc.project());
    let csv = wire_cut_csv(
        &jobs,
        &CsvOptions {
            bom: false,
            ..CsvOptions::default()
        },
    );
    let header = csv.lines().next().unwrap();
    for c in [
        "Direction",
        "Processing A",
        "Processing B",
        "Bundle",
        "Seq",
        "Track",
        "End basis",
    ] {
        assert!(header.contains(c), "{header}");
    }
    let out = WireCut.generate(doc.project());
    let cuts = &out.sections[0];
    for key in ["bundle", "seq", "track", "processing_a", "end_basis"] {
        assert!(cuts.column(key).is_some(), "{key}");
    }
}
