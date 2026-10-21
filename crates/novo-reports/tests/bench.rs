//! Bench output: ferrule standard and table, bench cut list, label sheets.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Wire};
use novo_reports::{
    BenchCut, Cell, FerruleSize, FerruleStandard, FerruleTable, LabelSettings, LabelStock, Labels,
    Report, TICK_BOX, bench_jobs, ferrule_counts, label_items, layout_labels, stock_presets,
    wire_cut_jobs_with, with_spares,
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

fn text(cell: &Cell) -> String {
    cell.to_string()
}

#[test]
fn ferrules_are_counted_per_size_with_twins_where_two_ends_share_a_pin() {
    let doc = cabinet();
    let jobs = bench_jobs(doc.project());
    let standard = FerruleStandard::default();
    let counts = ferrule_counts(&jobs, &standard);
    let ends: usize = counts.iter().map(|c| c.ends).sum();
    assert_eq!(ends, 2 * jobs.len(), "every wire end has a ferrule");
    let single = counts.iter().find(|c| !c.twin).expect("singles");
    assert_eq!(single.ferrule, "A1.5-10");
    assert_eq!(single.colour, "black");
    let twin = counts.iter().find(|c| c.twin).expect("twins");
    assert_eq!(twin.ferrule, "2x1.5-10");
    assert_eq!(twin.ends, 2 * twin.pieces);
    // Twins turned off: all single.
    let off = FerruleStandard {
        twin_to_mm2: 0.0,
        ..standard
    };
    let counts = ferrule_counts(&jobs, &off);
    assert!(counts.iter().all(|c| !c.twin));
    assert_eq!(
        counts.iter().map(|c| c.pieces).sum::<usize>(),
        2 * jobs.len()
    );
}

#[test]
fn the_table_prepares_spares_and_prints_the_standard() {
    let doc = cabinet();
    let out = FerruleTable.generate(doc.project());
    let table = out.section("ferrules").expect("table");
    let total = table.rows.last().expect("total row");
    let crimp: i64 = match total.cells[7] {
        Cell::Int(v) => v,
        ref c => panic!("{c:?}"),
    };
    let prepare = match total.cells[8] {
        Cell::Int(v) => v,
        ref c => panic!("{c:?}"),
    };
    assert!(prepare > crimp, "10% spares round up per row");
    assert_eq!(with_spares(34, 10.0), 38);
    assert_eq!(with_spares(10, 10.0), 11);
    assert_eq!(with_spares(7, 0.0), 7);
    // First column is the tick box; the standard is printed.
    assert_eq!(text(&table.rows[0].cells[0]), TICK_BOX);
    let printed = out.section("standard").expect("standard");
    assert_eq!(text(&printed.rows[3].cells[1]), "black");
}

#[test]
fn an_edited_standard_changes_strip_lengths_colours_and_is_stored_with_the_project() {
    let mut doc = cabinet();
    let mut standard = FerruleStandard::default();
    standard.sizes.iter_mut().for_each(|s| {
        if s.mm2 == 1.5 {
            s.colour = "red".into();
            s.length_mm = 12.0;
        }
    });
    standard.spare_percent = 0.0;
    doc.transact("Standard", |tx| FerruleStandard::store(tx, &standard))
        .expect("stored");
    assert_eq!(FerruleStandard::of(doc.project()), standard);
    let jobs = bench_jobs(doc.project());
    assert!(jobs.iter().all(|j| j.end_a.strip_mm == Some(12.0)));
    assert!(
        jobs.iter()
            .all(|j| j.end_a.ferrule.as_deref() == Some("A1.5-12"))
    );
    let out = FerruleTable.generate(doc.project());
    let table = out.section("ferrules").expect("table");
    assert_eq!(text(&table.rows[0].cells[1]), "A1.5-12");
    assert_eq!(text(&table.rows[0].cells[3]), "red");
    // Back to the default removes the property.
    doc.transact("Reset", |tx| {
        FerruleStandard::store(tx, &FerruleStandard::default())
    })
    .expect("reset");
    assert!(
        !doc.project()
            .info()
            .properties
            .contains_key(novo_reports::STANDARD_KEY)
    );
    // Explicit standard wins over the project's.
    let jobs = wire_cut_jobs_with(doc.project(), &standard);
    assert_eq!(jobs[0].end_a.strip_mm, Some(12.0));
}

#[test]
fn a_standard_that_does_not_read_falls_back_to_the_default() {
    let mut doc = cabinet();
    doc.transact("Junk", |tx| {
        tx.set_info(|i| {
            i.properties.insert(
                novo_reports::STANDARD_KEY.to_owned(),
                novo_core::Value::Text("{ not json".into()),
            );
        })
    })
    .expect("set");
    assert_eq!(
        FerruleStandard::of(doc.project()),
        FerruleStandard::default()
    );
    let twice = FerruleStandard {
        sizes: vec![
            FerruleSize {
                mm2: 1.5,
                colour: "a".into(),
                length_mm: 8.0,
            },
            FerruleSize {
                mm2: 1.5,
                colour: "b".into(),
                length_mm: 8.0,
            },
        ],
        ..FerruleStandard::default()
    };
    assert!(!twice.is_valid());
}

#[test]
fn lookup_takes_the_next_larger_size_for_an_odd_conductor() {
    let s = FerruleStandard::default();
    assert_eq!(s.lookup(1.5).map(|x| x.mm2), Some(1.5));
    assert_eq!(s.lookup(1.2).map(|x| x.mm2), Some(1.5));
    assert_eq!(s.lookup(0.3).map(|x| x.mm2), Some(0.5));
    assert_eq!(s.lookup(70.0).map(|x| x.mm2), Some(35.0));
}

#[test]
fn the_bench_cut_list_groups_ticks_and_marks_estimates() {
    let doc = cabinet();
    let out = BenchCut.generate(doc.project());
    assert_eq!(out.sections[0].key, "summary");
    // The estimates note shares the summary page instead of taking one.
    assert!(out.sections.iter().all(|s| s.key != "estimates"));
    // The motor cable's four cores are not pulled from a reel: a note says so
    // and their metres are left out of the totals.
    assert_eq!(out.sections[1].key, "check");
    assert!(
        out.sections[1]
            .text_rows()
            .iter()
            .any(|r| r[0].contains("4 of 25 wires are cores of cables")),
        "{:?}",
        out.sections[1].text_rows()
    );
    let group = out
        .section(&BenchCut::group_key(Some(1.5), "BK"))
        // Nineteen device-to-device wires and six that end at a supply feed.
        .expect("group");
    assert_eq!(group.rows.len(), 25);
    for row in &group.rows {
        assert_eq!(text(&row.cells[0]), TICK_BOX);
        // Cut = route + 2 x 10 mm.
        let cut = text(&row.cells[2]);
        let route = text(&row.cells[3]);
        assert_eq!(cut.starts_with('~'), route.starts_with('~'));
        let n = |t: &str| t.trim_start_matches('~').parse::<f64>().expect("number");
        assert_eq!(n(&cut), n(&route) + 20.0);
    }
    // The routed wires and the four cores of the motor cable (its stated
    // 12 m) count as stated, the rest are estimates.
    let exact = group
        .rows
        .iter()
        .filter(|r| !text(&r.cells[2]).starts_with('~'))
        .count();
    assert_eq!(exact, 7);
    // Longest first within the group.
    let cuts: Vec<f64> = group
        .rows
        .iter()
        .map(|r| text(&r.cells[2]).trim_start_matches('~').parse().unwrap())
        .collect();
    assert!(cuts.windows(2).all(|w| w[0] >= w[1]));
    assert!(out.sections[0].title.contains("18 of 25"));
    let estimated: i64 = out.sections[0]
        .rows
        .iter()
        .map(|r| match &r.cells[7] {
            Cell::Int(n) => *n,
            other => panic!("{other:?}"),
        })
        .sum();
    assert_eq!(estimated, 18);
}

#[test]
fn no_wires_gives_no_sections() {
    let doc = Document::default();
    assert!(BenchCut.generate(doc.project()).sections.is_empty());
    assert!(
        FerruleTable
            .generate(doc.project())
            .section("ferrules")
            .is_none()
    );
}

fn items(n: usize) -> Vec<novo_reports::LabelItem> {
    (0..n)
        .map(|i| novo_reports::LabelItem {
            kind: "wires".into(),
            lines: vec![
                format!("W{i}"),
                "=A1+CAB-K1:13".into(),
                "=A1+CAB-F2:95".into(),
            ],
        })
        .collect()
}

fn sheet_named(name: &str) -> LabelSettings {
    LabelSettings {
        stock: stock_presets()
            .into_iter()
            .find(|s| s.name().contains(name))
            .expect("preset"),
        ..LabelSettings::default()
    }
}

#[test]
fn every_preset_fits_its_sheet_and_labels_do_not_overlap() {
    for stock in stock_presets() {
        assert!(stock.is_valid(), "{}", stock.name());
        let LabelStock::Sheet(g) = &stock else {
            continue;
        };
        assert!(g.pitch_x >= g.label_w && g.pitch_y >= g.label_h);
        assert!(g.left + g.pitch_x * (g.cols - 1) as f64 + g.label_w <= g.page_w);
        assert!(g.top + g.pitch_y * (g.rows - 1) as f64 + g.label_h <= g.page_h);
    }
}

#[test]
fn labels_fill_sheets_in_reading_order_inside_their_cells() {
    let settings = sheet_named("5×13");
    let LabelStock::Sheet(g) = settings.stock.clone() else {
        panic!()
    };
    let run = layout_labels(&items(70), &settings);
    assert_eq!(run.labels, 70);
    assert_eq!(run.per_page, 65);
    assert_eq!(run.pages.len(), 2);
    assert!(
        run.pages
            .iter()
            .all(|p| (p.width, p.height) == (210.0, 297.0))
    );
    // Every text box sits inside its label cell.
    let first = &run.pages[0];
    assert_eq!(first.texts.len(), 65 * 3);
    for (n, chunk) in first.texts.chunks(3).enumerate() {
        let (col, row) = (n % g.cols, n / g.cols);
        let (x, y) = (
            g.left + col as f64 * g.pitch_x,
            g.top + row as f64 * g.pitch_y,
        );
        for t in chunk {
            assert!(t.x >= x && t.x + t.width <= x + g.label_w + 1e-9, "{t:?}");
            assert!(t.y >= y && t.y + t.height <= y + g.label_h + 1e-9, "{t:?}");
        }
        assert_eq!(chunk[0].text, format!("W{n}"));
        assert!(chunk[0].bold && !chunk[1].bold);
    }
    // Second sheet starts again at the first cell.
    assert_eq!(run.pages[1].texts[0].text, "W65");
    assert_eq!(run.pages[1].texts[0].x, first.texts[0].x);
}

#[test]
fn a_partly_used_sheet_continues_at_the_start_slot_and_copies_repeat() {
    let mut settings = sheet_named("3×8");
    settings.start_slot = 23; // the last two of 24 on the first sheet
    settings.copies = 2;
    let run = layout_labels(&items(2), &settings);
    assert_eq!(run.labels, 4);
    assert_eq!(run.pages.len(), 2);
    assert_eq!(
        run.pages[0].texts.len(),
        2 * 3,
        "two labels fit on the first sheet"
    );
    assert_eq!(run.pages[1].texts.len(), 2 * 3);
    assert_eq!(run.pages[0].texts[0].text, "W0");
    assert_eq!(run.pages[0].texts[3].text, "W0");
    assert_eq!(run.pages[1].texts[0].text, "W1");
    // Offsets move everything by the same amount.
    let base = layout_labels(&items(1), &sheet_named("3×8"));
    let shifted = layout_labels(
        &items(1),
        &LabelSettings {
            offset_x: 1.5,
            offset_y: -0.5,
            ..sheet_named("3×8")
        },
    );
    assert_eq!(shifted.pages[0].texts[0].x, base.pages[0].texts[0].x + 1.5);
    assert_eq!(shifted.pages[0].texts[0].y, base.pages[0].texts[0].y - 0.5);
}

#[test]
fn outlines_are_drawn_only_when_asked() {
    let mut settings = sheet_named("3×8");
    settings.start_slot = 3;
    assert!(
        layout_labels(&items(1), &settings).pages[0]
            .lines
            .is_empty()
    );
    settings.outlines = true;
    // One label plus the two skipped cells.
    assert_eq!(
        layout_labels(&items(1), &settings).pages[0].lines.len(),
        3 * 4
    );
}

#[test]
fn tape_makes_one_page_per_label_as_high_as_the_tape() {
    let settings = LabelSettings {
        stock: stock_presets()
            .into_iter()
            .find(|s| s.name() == "Tape 12 mm")
            .expect("tape"),
        ..LabelSettings::default()
    };
    let one = vec![novo_reports::LabelItem {
        kind: "wires".into(),
        lines: vec!["W1".into()],
    }];
    let long = vec![novo_reports::LabelItem {
        kind: "wires".into(),
        lines: vec!["W1".into(), "=A1+CAB-K1:13".into()],
    }];
    let a = layout_labels(&one, &settings);
    let b = layout_labels(&long, &settings);
    assert_eq!((a.pages.len(), a.pages[0].height), (1, 12.0));
    assert!(a.pages[0].width >= 15.0 && a.pages[0].width % 5.0 == 0.0);
    assert!(
        b.pages[0].width > a.pages[0].width,
        "length follows the text"
    );
    // The text fits the label width.
    for t in &b.pages[0].texts {
        assert!(0.62 * t.text_mm * t.text.chars().count() as f64 <= t.width + 1e-9);
    }
}

#[test]
fn text_that_cannot_be_read_is_counted() {
    let settings = LabelSettings {
        stock: stock_presets()
            .into_iter()
            .find(|s| s.name() == "Tape 6 mm")
            .expect("tape"),
        ..LabelSettings::default()
    };
    let run = layout_labels(&items(2), &settings);
    assert_eq!(run.too_small, 2, "three lines on 6 mm tape are under 2 mm");
}

#[test]
fn label_items_come_from_the_labels_report_by_kind() {
    let doc = cabinet();
    let report = Labels.generate(doc.project());
    let wires = label_items(&report, &["wires".into()]);
    // Two per wire to cut: 19 connections plus 6 supply and PE leads.
    assert_eq!(wires.len(), 2 * 25);
    assert!(
        wires
            .iter()
            .all(|i| i.kind == "wires" && !i.lines.is_empty())
    );
    let all = label_items(&report, &[]);
    assert!(all.len() > wires.len());
}

#[test]
fn label_settings_round_trip_through_the_project() {
    let mut doc = cabinet();
    assert_eq!(LabelSettings::of(doc.project()), LabelSettings::default());
    let mut settings = sheet_named("3×7");
    settings.start_slot = 5;
    settings.kinds = vec!["terminals".into(), "devices".into()];
    doc.transact("Labels", |tx| LabelSettings::store(tx, &settings))
        .expect("stored");
    assert_eq!(LabelSettings::of(doc.project()), settings);
    let bad = LabelSettings {
        copies: 0,
        ..LabelSettings::default()
    };
    assert!(!bad.is_valid());
}

#[test]
fn the_cut_list_says_when_sizes_and_colours_are_not_set() {
    let mut doc = cabinet();
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Clear", |tx| {
        for id in wires {
            tx.update::<Wire>(id, |w| {
                w.cross_section_mm2 = None;
                w.color = None;
            })?;
        }
        Ok(())
    })
    .expect("clear");
    let out = BenchCut.generate(doc.project());
    let notes: Vec<String> = out
        .section("check")
        .expect("a check-before-cutting section")
        .rows
        .iter()
        .map(|r| text(&r.cells[0]))
        .collect();
    assert!(notes.iter().any(|n| n.contains("no cross-section set")));
    assert!(notes.iter().any(|n| n.contains("no insulation colour")));
    // And with everything set, neither note appears.
    let doc = cabinet();
    let out = BenchCut.generate(doc.project());
    let notes: Vec<String> = out
        .section("check")
        .map(|s| s.rows.iter().map(|r| text(&r.cells[0])).collect())
        .unwrap_or_default();
    assert!(notes.iter().all(|n| !n.contains("no cross-section")));
    assert!(notes.iter().all(|n| !n.contains("no insulation colour")));
}

#[test]
fn the_cut_list_says_when_wires_have_no_number() {
    let mut doc = cabinet();
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    let set = |doc: &mut Document, number: Option<&str>| {
        let wires = wires.clone();
        doc.transact("Numbers", |tx| {
            for (i, id) in wires.into_iter().enumerate() {
                tx.update::<Wire>(id, |w| w.number = number.map(|n| format!("{n}{i}")))?;
            }
            Ok(())
        })
        .expect("numbers");
    };
    let notes = |doc: &Document| -> Vec<String> {
        BenchCut
            .generate(doc.project())
            .section("check")
            .map(|s| s.rows.iter().map(|r| text(&r.cells[0])).collect())
            .unwrap_or_default()
    };
    set(&mut doc, None);
    assert!(
        notes(&doc)
            .iter()
            .any(|n| n.contains("have no wire number")),
        "{:?}",
        notes(&doc)
    );
    set(&mut doc, Some("W"));
    assert!(notes(&doc).iter().all(|n| !n.contains("no wire number")));
}
