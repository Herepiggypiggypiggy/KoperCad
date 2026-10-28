//! Work cards: the same build order as the print pack, every wire on one
//! card, and the export next to the project.

use novo_core::{DeviceTag, Project, Value};
use novo_layout::workpack::CardKind;
use novo_reports::{AssemblyChecklist, Report};

use super::Level;
use super::assembly::order_sections;
use super::cabinet::Ask;
use super::gate1_tests::Drive;
use super::workpack::{ferrule_text, work_packs};
use crate::actions::Action;
use novo_layout::workpack::CORE_MARK;

fn lesson(path: &str) -> Project {
    let file = format!("{}/../../assets/lessons/{path}", env!("CARGO_MANIFEST_DIR"));
    Project::from_json(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn the_cards_follow_the_print_packs_build_order_step_for_step() {
    let project = lesson("wire-routing/project.json");
    let checklist = AssemblyChecklist.generate(&project);
    let orders = order_sections(&project, &checklist);
    let packs = work_packs(&project);
    assert!(!orders.is_empty() && orders.len() == packs.len());
    for (order, pack) in orders.iter().zip(&packs) {
        assert_eq!(order.rows.len(), pack.overview.len());
        for (row, step) in order.rows.iter().zip(&pack.overview) {
            assert_eq!(row.cells[0].to_string(), step.step.to_string());
            assert_eq!(row.cells[1].to_string(), step.phase.name());
            assert_eq!(row.cells[2].to_string(), step.mark);
        }
    }
}

#[test]
fn every_wire_of_the_build_order_is_on_exactly_one_wire_card() {
    let project = lesson("wire-routing/project.json");
    for pack in work_packs(&project) {
        let wire_steps = pack
            .overview
            .iter()
            .filter(|r| r.phase == novo_layout::assembly::Phase::Wires)
            .count();
        let on_cards: usize = pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Wires)
            .map(|c| c.wires.len())
            .sum();
        assert_eq!(on_cards, wire_steps, "{}", pack.cabinet);
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            assert!(c.wires.iter().all(|w| w.cut_mm > 0.0));
            // Step numbers on the card are the print pack's wire steps.
            let first = pack.overview.iter().find(|r| r.step == c.steps.0).unwrap();
            assert_eq!(first.mark, c.wires[0].number);
        }
    }
}

fn cabinet_with_devices(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2"] {
                let device = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(device, part)?;
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_cards_need_a_plate_then_land_next_to_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.save_to(&dir.path().join("Plant 2.novo")));
    d.frame();

    assert_eq!(d.app.export_workpack(), None);
    let last = d.app.messages().last().unwrap();
    assert_eq!(last.level, Level::Warning);
    assert!(last.text.contains("mounting plate"), "{}", last.text);
    assert!(!dir.path().join("Plant 2 work cards.pdf").exists());

    cabinet_with_devices(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .find(|s| s.enclosure)
        .unwrap()
        .id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab);
    let candidates = novo_layout::model::candidates(project, cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout, &candidates);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);
    d.frame();

    // The commands ask where to save; a cancelled dialog (the only answer in
    // a unit test) writes nothing.
    assert_eq!(d.app.ask_workpack(), None);
    assert!(!dir.path().join("Plant 2 work cards.pdf").exists());

    let pdf = d.app.export_workpack().expect("exported");
    assert_eq!(pdf, dir.path().join("Plant 2 work cards.pdf"));
    let bytes = std::fs::read(&pdf).unwrap();
    let doc = lopdf::Document::load_mem(&bytes).unwrap();
    let cover = doc.extract_text(&[1]).unwrap();
    assert!(cover.contains("Workshop pack +CAB1"), "{cover}");
    // Cover, tick-off sheet, at least one card page.
    assert!(doc.get_pages().len() >= 3);

    // A second export replaces our own file and says so.
    d.app.export_workpack().expect("again");
    assert!(
        d.app
            .messages()
            .last()
            .unwrap()
            .text
            .starts_with("Replaced "),
        "{}",
        d.app.messages().last().unwrap().text
    );

    // One step as one card next to the project; an unknown mark warns.
    let first = d.app.machine().map(|m| {
        work_packs(&m)[0]
            .cards
            .iter()
            .find(|c| c.kind != CardKind::Wires && c.kind != CardKind::Labels)
            .map(|c| c.mark.clone())
            .unwrap()
    });
    let mark = first.unwrap();
    assert_eq!(d.app.ask_workpack_step(cab, &mark), None);
    assert!(
        std::fs::read_dir(dir.path()).unwrap().all(|f| !f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("work card ")),
        "a cancelled dialog writes no card"
    );
    let one = d.app.export_workpack_step(cab, &mark).expect("one card");
    assert!(
        one.file_name()
            .unwrap()
            .to_string_lossy()
            .contains("work card")
    );
    assert_eq!(
        lopdf::Document::load_mem(&std::fs::read(&one).unwrap())
            .unwrap()
            .get_pages()
            .len(),
        1
    );
    // Saving over a file the person chose says "Replaced".
    d.app.write_step_card(&one, b"%PDF-");
    assert!(
        d.app
            .messages()
            .last()
            .unwrap()
            .text
            .starts_with("Replaced "),
        "{}",
        d.app.messages().last().unwrap().text
    );
    assert_eq!(d.app.export_workpack_step(cab, "-NOPE"), None);
    assert_eq!(d.app.messages().last().unwrap().level, Level::Warning);

    // The workshop folder carries the cards too.
    d.app.export_workshop().expect("folder");
    assert!(
        dir.path()
            .join("Plant 2 workshop/1 Work cards.pdf")
            .exists()
    );
}

#[test]
fn every_card_length_is_a_cut_list_length_and_nothing_falls_back() {
    use novo_reports::{CutList, Report};
    let project = lesson("wire-routing/project.json");
    let mut cut_list: Vec<f64> = CutList::default()
        .generate(&project)
        .sections
        .iter()
        .filter(|s| s.key.starts_with("group-"))
        .flat_map(|s| {
            let i = s.column("cut_mm").unwrap();
            s.rows.iter().map(move |r| match &r.cells[i] {
                novo_reports::Cell::Number(v) => *v,
                other => panic!("{other:?}"),
            })
        })
        .collect();
    let mut on_cards: Vec<f64> = Vec::new();
    for pack in work_packs(&project) {
        assert!(
            pack.problems
                .iter()
                .all(|p| !p.contains("not in the cut list")),
            "{:?}",
            pack.problems
        );
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            on_cards.extend(c.wires.iter().map(|w| w.cut_mm));
        }
    }
    // The cabinet's cards carry the wires with an end in it: every length
    // is one of the cut list's, each used at most as often as it appears.
    for w in &on_cards {
        let i = cut_list
            .iter()
            .position(|c| c == w)
            .unwrap_or_else(|| panic!("{w} mm is not a cut list length"));
        cut_list.remove(i);
    }
    assert!(!on_cards.is_empty());
}

#[test]
fn cut_list_wires_without_a_cabinet_end_are_named_on_the_cover() {
    let project = lesson("wire-routing/project.json");
    let packs = work_packs(&project);
    let on_cards: usize = packs
        .iter()
        .flat_map(|p| &p.cards)
        .filter(|c| c.kind == CardKind::Wires)
        .map(|c| c.wires.len())
        .sum();
    let named: Vec<&String> = packs
        .iter()
        .flat_map(|p| &p.problems)
        .filter(|p| p.contains("have no end inside a plated cabinet"))
        .collect();
    assert_eq!(named.len(), 1, "{named:?}");
    // 25 wires in the cut list: 19 on cards, the 6 feed leads named.
    assert!(
        named[0].starts_with(&format!("{} of 25 cut-list wires", 25 - on_cards)),
        "{}",
        named[0]
    );
    assert!(named[0].contains("(feed)"), "{}", named[0]);
    // The same wires are on "Outside the plate" cards with tick boxes, not numbered.
    let outside: Vec<_> = packs
        .iter()
        .flat_map(|p| &p.cards)
        .filter(|c| c.kind == CardKind::Outside)
        .collect();
    assert_eq!(
        outside.iter().map(|c| c.wires.len()).sum::<usize>(),
        25 - on_cards
    );
    for c in &outside {
        assert_eq!(c.steps, (0, 0));
        assert_eq!(c.ticks, ["Cut", "Run"]);
        assert!(c.wires.iter().all(|w| w.estimated && w.end_mm.is_some()));
        // A missing size reads the same as on the cabinet cards.
        assert!(c.wires.iter().all(|w| !w.kind.contains("Size unknown")));
    }
    // Every wire the cover names is on a card: same number, ends and cut.
    for w in outside.iter().flat_map(|c| &c.wires) {
        let line = format!(
            "{} ({} to {}) cut ~{} mm",
            w.number,
            w.from,
            w.to,
            novo_layout::drill::mm(w.cut_mm)
        );
        assert!(named[0].contains(&line), "{line} not in {}", named[0]);
    }
    // Their strip text is the cut jobs' own, as on the cabinet cards.
    let jobs = novo_reports::wire_cut_jobs(&project);
    for w in outside.iter().flat_map(|c| &c.wires) {
        let job = jobs
            .iter()
            .find(|j| {
                j.number == w.number
                    && [&j.end_a.designation, &j.end_b.designation].contains(&&w.from)
                    && [&j.end_a.designation, &j.end_b.designation].contains(&&w.to)
            })
            .unwrap_or_else(|| panic!("no job for wire {}", w.number));
        let strips = [job.end_a.strip_mm, job.end_b.strip_mm];
        assert!(
            strips.contains(&w.strip_from_mm) && strips.contains(&w.strip_to_mm),
            "wire {}",
            w.number
        );
    }
    // One cabinet: the fallback to "the first" is trivial and not announced.
    assert!(
        packs
            .iter()
            .flat_map(|p| &p.problems)
            .all(|p| !p.contains("name no cabinet"))
    );
}

#[test]
fn wires_filed_under_the_first_cabinet_by_default_say_so_and_the_other_packs_point_to_it() {
    use super::workpack::unplaced_notes;
    let unplaced = std::collections::BTreeMap::from([(0, vec!["W7".to_owned(), "W9".to_owned()])]);
    let cabs = ["+CAB1".to_owned(), "+CAB2".to_owned(), "+CAB3".to_owned()];
    let notes = unplaced_notes(&unplaced, &cabs);
    assert!(notes[&0][0].starts_with("Wires W7, W9 name no cabinet"));
    assert!(
        notes[&0][0].contains("listed on this pack only"),
        "{notes:?}"
    );
    // Every other pack says where they are; none claims to list them.
    for at in [1, 2] {
        assert_eq!(notes[&at].len(), 1, "{notes:?}");
        assert!(
            notes[&at][0].contains("pack of +CAB1, not here"),
            "{notes:?}"
        );
    }
    // One cabinet: the fallback is trivial, nothing is said.
    assert!(unplaced_notes(&unplaced, &cabs[..1]).is_empty());
    // No unplaced wires: nothing is said.
    assert!(unplaced_notes(&Default::default(), &cabs).is_empty());
}

/// A saved project with a plated cabinet `+CAB1` whose interior is 600 x 800 x 250 mm.
fn vent_cabinet(dir: &std::path::Path) -> (Drive, novo_core::StructureId) {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.save_to(&dir.join("Plant 3.novo")));
    cabinet_with_devices(&mut d);
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    let cab = d
        .app
        .backend
        .document()
        .unwrap()
        .project()
        .structures()
        .find(|s| s.enclosure)
        .unwrap()
        .id;
    d.app.cabinet_ask(Ask::SetPlate(cab), Some(cab), None, &[]);
    let project = d.app.backend.document().unwrap().project();
    let layout = novo_layout::model::load(project, cab);
    let candidates = novo_layout::model::candidates(project, cab);
    d.app
        .cabinet_ask(Ask::PlaceAll, Some(cab), layout, &candidates);
    d.app.cabinet_ask(Ask::Accept, Some(cab), None, &[]);

    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Interior", |tx| {
            for (k, v) in [
                ("interior_width_mm", 600.0),
                ("interior_height_mm", 800.0),
                ("interior_depth_mm", 250.0),
            ] {
                tx.set_property(cab, k, Some(Value::Float(v)))?;
            }
            Ok(())
        })
        .unwrap();
    (d, cab)
}

fn store_vents(
    d: &mut Drive,
    cab: novo_core::StructureId,
    vents: &[super::heat_settings::VentSpec],
) {
    let held = super::heat_openings_store::text(vents);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Vents", |tx| {
            tx.set_property(
                cab,
                super::heat_openings_store::KEY,
                Some(Value::Text(held)),
            )
        })
        .unwrap();
}

#[test]
fn an_opening_over_a_wall_edge_is_printed_where_the_heat_panel_and_solver_cut_it() {
    use super::heat_settings::{VentSpec, Wall};
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    // 150 wide, centre 5 mm from the top of the left wall: the panel pulls it to 0..150.
    let mut over = VentSpec::new(Wall::Left);
    over.u_mm = 5.0;
    over.width_mm = 150.0;
    over.height_mm = 100.0;
    // Wider than the wall is long: cut down to the wall.
    let mut huge = VentSpec::new(Wall::Top);
    huge.width_mm = 5000.0;
    store_vents(&mut d, cab, &[over, huge]);
    let packs = work_packs(&d.app.machine().unwrap());
    let cards: Vec<_> = packs[0]
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .collect();
    assert_eq!(cards.len(), 2);
    assert!(cards[0].dims[0].contains("75 mm"), "{:?}", cards[0].dims);
    assert!(
        cards[0].dims[1].contains("Top edge 0 mm, bottom edge 150 mm"),
        "{:?}",
        cards[0].dims
    );
    let t = cards[1].template.as_ref().unwrap();
    assert!(t.width <= 600.0 + 20.0 + 1e-6, "{}", t.width);
}

#[test]
fn the_heat_panels_vents_and_fans_become_cut_out_cards_outside_the_build_order() {
    use super::heat_settings::{Kind, VentSpec, Wall};

    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    // No vents: no opening cards.
    let none = work_packs(&d.app.machine().unwrap());
    assert!(none[0].cards.iter().all(|c| c.kind != CardKind::Opening));

    let mut fan = VentSpec::new(Wall::Door);
    fan.kind = Kind::FilterFan;
    fan.u_mm = 300.0;
    fan.v_mm = 700.0;
    fan.width_mm = 120.0;
    fan.height_mm = 120.0;
    let grille = VentSpec::new(Wall::Top);
    store_vents(&mut d, cab, &[fan, grille]);
    let packs = work_packs(&d.app.machine().unwrap());
    let cards: Vec<_> = packs[0]
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .collect();
    assert_eq!(cards.len(), 2, "{:?}", packs[0].cards.len());
    assert!(cards[0].mark.to_lowercase().contains("filter fan") && cards[0].mark.contains("door"));
    assert!(cards[0].dims[0].contains("300"), "{:?}", cards[0].dims);
    assert!(cards[1].mark.contains("top wall"));
    // They are not build-order steps: the overview does not grow.
    assert_eq!(packs[0].overview.len(), none[0].overview.len());

    // And they are in the PDF (cover + tick-off + cards).
    let pdf = d.app.export_workpack().expect("exported");
    let doc = lopdf::Document::load_mem(&std::fs::read(&pdf).unwrap()).unwrap();
    let last = doc.get_pages().len() as u32;
    let text = (1..=last)
        .map(|p| doc.extract_text(&[p]).unwrap_or_default())
        .collect::<String>();
    assert!(text.contains("CUT-OUT"), "{text}");
}

#[test]
fn the_one_card_file_name_carries_the_cabinet_only_when_there_are_several() {
    use super::workpack::step_file_name;
    assert_eq!(
        step_file_name("Plant", "+CAB1", "R1", false),
        "Plant work card R1.pdf"
    );
    let a = step_file_name("Plant", "+CAB1", "R1", true);
    let b = step_file_name("Plant", "+CAB2", "R1", true);
    assert_ne!(a, b);
    assert!(a.contains("CAB1") && a.ends_with("R1.pdf"), "{a}");
}

#[test]
fn every_wire_card_row_carries_the_cut_lists_end_allowance() {
    let project = lesson("wire-routing/project.json");
    // The cut list's (cut, each end) pairs.
    let mut pairs: Vec<(f64, f64)> = novo_reports::CutList::default()
        .generate(&project)
        .sections
        .iter()
        .filter(|s| s.key.starts_with("group-"))
        .flat_map(|s| {
            let (c, a) = (
                s.column("cut_mm").unwrap(),
                s.column("allowance_mm").unwrap(),
            );
            s.rows.iter().map(move |r| {
                let num = |i: usize| match &r.cells[i] {
                    novo_reports::Cell::Number(v) => *v,
                    other => panic!("{other:?}"),
                };
                (num(c), num(a) / 2.0)
            })
        })
        .collect();
    let mut seen = 0;
    for pack in work_packs(&project) {
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            for w in &c.wires {
                let end = w.end_mm.expect("an end allowance");
                let i = pairs
                    .iter()
                    .position(|p| *p == (w.cut_mm, end))
                    .unwrap_or_else(|| {
                        panic!("{} mm / {end} per end is not in the cut list", w.cut_mm)
                    });
                pairs.remove(i);
                assert!(!w.end_text().is_empty());
                seen += 1;
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn the_fitting_word_follows_the_cut_lists_allowance_not_a_second_reading_of_the_size() {
    use super::workpack::fitting_of;
    // Rules: ferrule 10 mm, lug 20 mm. A chain of 10 and 35 mm2 wires is cut
    // with the lug allowance (the largest size) and must read "lug".
    assert_eq!(fitting_of(Some(20.0), 10.0, 20.0), "lug");
    assert_eq!(fitting_of(Some(10.0), 10.0, 20.0), "ferrule");
    // Neither, none, or both fittings taking the same allowance: no word.
    assert_eq!(fitting_of(Some(15.0), 10.0, 20.0), "");
    assert_eq!(fitting_of(None, 10.0, 20.0), "");
    assert_eq!(fitting_of(Some(12.0), 12.0, 12.0), "");
}

#[test]
fn a_wire_with_a_mixed_size_chain_gets_the_lug_allowance_of_its_largest_size() {
    // The wire-routing example has sized wires; every card row's word agrees
    // with its allowance under the project's rules.
    let project = lesson("wire-routing/project.json");
    let rules = novo_routing::project_rules::WireRules::of(&project);
    for pack in work_packs(&project) {
        for w in pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Wires)
            .flat_map(|c| &c.wires)
        {
            let end = w.end_mm.unwrap();
            let expected = if (rules.lug_mm - rules.ferrule_mm).abs() < 1e-6 {
                ""
            } else if (end - rules.lug_mm).abs() < 1e-6 {
                "lug"
            } else {
                "ferrule"
            };
            assert_eq!(w.fitting, expected, "{} {end}", w.number);
        }
    }
}

#[test]
fn wiring_openings_of_the_cabinet_become_cut_out_cards() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    let none = work_packs(&d.app.machine().unwrap());
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Openings", |tx| {
            tx.set_property(
                cab,
                "opening.grommet",
                Some(Value::Text("plate round 30 200 150".into())),
            )?;
            tx.set_property(
                cab,
                "opening.entry",
                Some(Value::Text("door rect 80 40 300 100".into())),
            )
        })
        .unwrap();
    let packs = work_packs(&d.app.machine().unwrap());
    let cards: Vec<_> = packs[0]
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .collect();
    assert_eq!(cards.len(), 2, "{:?}", packs[0].problems);
    assert!(cards.iter().any(|c| c.mark.contains("grommet")));
    let door = cards.iter().find(|c| c.mark.contains("entry")).unwrap();
    assert!(door.dims[0].contains("300"), "{:?}", door.dims);
    assert_eq!(packs[0].overview.len(), none[0].overview.len());
    // Both in the PDF the user prints.
    let pdf = d.app.export_workpack().expect("exported");
    let doc = lopdf::Document::load_mem(&std::fs::read(&pdf).unwrap()).unwrap();
    let all: String = (1..=doc.get_pages().len() as u32)
        .map(|p| doc.extract_text(&[p]).unwrap_or_default())
        .collect();
    assert!(
        all.contains("grommet") && all.contains("2 cut-out cards"),
        "{all}"
    );
}

#[test]
fn a_rail_cards_clips_are_the_devices_the_build_order_clips_on_it_at_the_same_distance() {
    let project = lesson("wire-routing/project.json");
    let mut seen = 0;
    for pack in work_packs(&project) {
        for card in pack.cards.iter().filter(|c| c.kind == CardKind::Rail) {
            let clips = &card.template.as_ref().unwrap().clips;
            // "Clip =A1+CAB1-Q1 on R1, 10 mm from its start" for every clipped device.
            let rows: Vec<&str> = pack
                .overview
                .iter()
                .filter(|r| r.phase == novo_layout::assembly::Phase::Devices)
                .map(|r| r.text.as_str())
                .filter(|t| t.contains(&format!(" on {},", card.mark)))
                .collect();
            assert_eq!(rows.len(), clips.len(), "{}", card.mark);
            for clip in clips {
                let line = format!(
                    "Clip {} on {}, {} mm from its start",
                    clip.tag,
                    card.mark,
                    novo_layout::drill::mm(clip.at)
                );
                assert!(rows.iter().any(|r| *r == line), "{line} not in {rows:?}");
                seen += 1;
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn wires_sharing_a_number_are_told_apart_and_no_size_reads_as_not_set() {
    let project = lesson("wire-routing/project.json");
    let mut by_number: std::collections::BTreeMap<String, Vec<Option<(usize, usize)>>> =
        Default::default();
    for pack in work_packs(&project) {
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            for w in &c.wires {
                by_number.entry(w.number.clone()).or_default().push(w.part);
                assert!(!w.kind.is_empty(), "wire {} has a blank kind", w.number);
            }
        }
    }
    let mut shared = 0;
    for (number, parts) in &by_number {
        if parts.len() == 1 {
            assert_eq!(parts[0], None, "wire {number} is used once");
        } else {
            shared += 1;
            let mut want: Vec<_> = (1..=parts.len()).map(|i| Some((i, parts.len()))).collect();
            let mut got = parts.clone();
            got.sort();
            want.sort();
            assert_eq!(got, want, "wire {number}");
        }
    }
    assert!(shared >= 1, "{by_number:?}");
}

#[test]
fn parts_of_counts_each_number_in_row_order() {
    use super::workpack::parts_of;
    let n = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        parts_of(&n(&["4", "5", "4", "8", "4"])),
        [Some((1, 3)), None, Some((2, 3)), None, Some((3, 3))]
    );
    assert!(parts_of(&n(&[])).is_empty());
    // Unnumbered wires are not parts of one another.
    assert_eq!(parts_of(&n(&["", " ", "", "7"])), [None, None, None, None]);
    assert_eq!(
        parts_of(&n(&["", "7", "", "7"])),
        [None, Some((1, 2)), None, Some((2, 2))]
    );
}

#[test]
fn every_wire_card_row_carries_the_strip_length_of_the_cut_jobs() {
    let project = lesson("wire-routing/project.json");
    let jobs = novo_reports::wire_cut_jobs(&project);
    let mut seen = 0;
    for pack in work_packs(&project) {
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            for w in &c.wires {
                // The job of this number whose ends are the row's two ends.
                let job = jobs
                    .iter()
                    .find(|j| {
                        j.number == w.number
                            && [&j.end_a.designation, &j.end_b.designation].contains(&&w.from)
                            && [&j.end_a.designation, &j.end_b.designation].contains(&&w.to)
                    })
                    .unwrap_or_else(|| panic!("no job for wire {}", w.number));
                let (f, t) = if job.end_a.designation == w.from {
                    (&job.end_a, &job.end_b)
                } else {
                    (&job.end_b, &job.end_a)
                };
                assert_eq!(w.strip_from_mm, f.strip_mm, "{} {}", w.number, w.from);
                assert_eq!(w.strip_to_mm, t.strip_mm, "{} {}", w.number, w.to);
                assert_eq!(
                    w.strip_basis.is_empty(),
                    f.strip_mm.is_none() && t.strip_mm.is_none()
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn a_part_that_states_a_strip_length_is_on_the_card_not_the_typical_one() {
    use novo_core::Document;
    let mut doc = Document::new(lesson("wire-routing/project.json"));
    // A device with a part on a wire of the cards: give its part 7.5 mm.
    let (device, part) = doc
        .project()
        .devices()
        .find_map(|d| d.parts.first().map(|p| (d.id, *p)))
        .expect("a device with a part");
    doc.transact("Part", |tx| {
        tx.set_property(
            part,
            novo_reports::STRIP_LENGTH_KEY,
            Some(Value::Float(7.5)),
        )
    })
    .unwrap();
    let tag = doc.project().device(device).unwrap().tag.to_string();
    let mut found = 0;
    for pack in work_packs(doc.project()) {
        for c in pack.cards.iter().filter(|c| c.kind == CardKind::Wires) {
            for w in &c.wires {
                if w.from.starts_with(&format!("{tag}:")) || w.from.contains(&format!("-{tag}:")) {
                    assert_eq!(w.strip_from_mm, Some(7.5), "{} {}", w.number, w.from);
                    assert_eq!(w.strip_basis, "part");
                    found += 1;
                }
            }
        }
    }
    assert!(found > 0, "no wire starts at {tag}");
}

#[test]
fn a_wire_number_repeated_among_the_unplaced_is_named_once() {
    use super::workpack::unplaced_notes;
    let unplaced = std::collections::BTreeMap::from([(
        0,
        vec!["W7".to_owned(), "W7".to_owned(), "W3".to_owned()],
    )]);
    let cabs = ["+CAB1".to_owned(), "+CAB2".to_owned()];
    let notes = unplaced_notes(&unplaced, &cabs);
    assert!(
        notes[&0][0].starts_with("Wires W3, W7 name no cabinet"),
        "{notes:?}"
    );
    assert!(
        notes[&1][0].starts_with("Wires W3, W7 name no cabinet"),
        "{notes:?}"
    );
}

#[test]
fn cable_cores_say_they_come_from_the_cable_and_the_cover_explains_it() {
    // The example's motor cable -W1 (12 m) carries the field wires.
    let mut doc = novo_core::Document::default();
    super::example::build_example(&mut doc).unwrap();
    let project = doc.project().clone();
    let cores = novo_reports::wire_cut_jobs(&project)
        .iter()
        .filter(|j| j.cable.is_some())
        .count();
    assert!(cores > 0, "the lesson has no cable cores");
    let packs = work_packs(&project);
    let on_cards: Vec<_> = packs
        .iter()
        .flat_map(|p| &p.cards)
        .filter(|c| matches!(c.kind, CardKind::Wires | CardKind::Outside))
        .flat_map(|c| &c.wires)
        .filter(|w| w.kind.contains(" core ="))
        .collect();
    assert_eq!(on_cards.len(), cores);
    assert!(
        packs
            .iter()
            .flat_map(|p| &p.problems)
            .any(|t| t.contains("are cores of") && t.contains("not cut from hookup wire")),
        "{:?}",
        packs.iter().map(|p| &p.problems).collect::<Vec<_>>()
    );
}

#[test]
fn core_rows_say_run_cable_and_the_tools_row_counts_only_cut_wires() {
    let mut doc = novo_core::Document::default();
    super::example::build_example(&mut doc).unwrap();
    let packs = work_packs(&doc.project().clone());
    let mut cores_seen = 0;
    for pack in &packs {
        for w in pack.cards.iter().flat_map(|c| &c.wires) {
            if let Some(cable) = w.cable() {
                cores_seen += 1;
                let line = novo_layout::workpack::run_or_cut(w);
                assert!(
                    line.starts_with("run cable ") && line.contains(cable),
                    "{line}"
                );
                assert!(!line.contains("cut"), "{line}");
            }
        }
        // No tick-off row tells the bench to cut a core.
        for row in &pack.overview {
            assert!(
                !(row.text.contains("core") && row.text.contains("cut")),
                "{}",
                row.text
            );
        }
        // The tools row counts the wires the strips cut, cores left out.
        let strip_cut: usize = pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Strips && !c.mark.contains(" cores of "))
            .map(|c| c.wires.len())
            .sum();
        if let Some(t) = pack
            .tools
            .iter()
            .find(|t| t.tool.starts_with("Wire cutter"))
        {
            let n: usize = t
                .for_what
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(n, strip_cut, "{}: {}", pack.cabinet, t.for_what);
        }
    }
    assert!(cores_seen > 0);
}

#[test]
fn the_label_card_names_the_stock_from_the_projects_label_settings() {
    let mut doc = novo_core::Document::default();
    super::example::build_example(&mut doc).unwrap();
    let stock = novo_reports::LabelSettings::of(doc.project()).stock.name();
    let packs = work_packs(doc.project());
    let labels: Vec<_> = packs
        .iter()
        .flat_map(|p| &p.cards)
        .filter(|c| c.kind == CardKind::Labels)
        .collect();
    assert!(!labels.is_empty());
    for c in labels {
        assert!(c.tools[0].contains(&stock), "{:?}", c.tools);
        assert!(c.notes.iter().any(|n| n.contains("Print at 100 %")));
    }
}

#[test]
fn every_cabinet_wire_is_on_a_cut_strip_longest_first_with_its_ferrule() {
    let project = lesson("wire-routing/project.json");
    for pack in work_packs(&project) {
        let on_wires: usize = pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Wires)
            .map(|c| c.wires.len())
            .sum();
        let strips: Vec<_> = pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Strips)
            .collect();
        let on_strips: usize = strips.iter().map(|c| c.wires.len()).sum();
        let outside: usize = pack
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Outside)
            .map(|c| c.wires.len())
            .sum();
        // The strips carry the wires of the wire cards and the outside ones.
        assert_eq!(on_strips, on_wires + outside, "{}", pack.cabinet);
        assert!(on_strips > 0, "{}", pack.cabinet);
        assert!(strips.iter().all(|c| c.steps == (0, 0)));
        // Within a card the lengths never grow.
        for c in strips.iter().filter(|c| !c.mark.contains(" cores of ")) {
            assert!(c.wires.windows(2).all(|w| w[0].cut_mm >= w[1].cut_mm));
            assert!(c.wires.iter().all(|w| !w.kind.contains(CORE_MARK)));
        }
        // Cores are only on their own cards, which say they are not cut.
        for c in strips.iter().filter(|c| c.mark.contains(" cores of ")) {
            assert!(c.wires.iter().all(|w| w.kind.contains(CORE_MARK)));
            assert!(c.text.contains("do not cut a core"), "{}", c.text);
            assert_eq!(c.ticks[0], "Cable run");
        }
        // A wire with a numeric size names its ferrule from the standard.
        for w in strips.iter().flat_map(|c| &c.wires) {
            if w.kind
                .split_whitespace()
                .next()
                .is_some_and(|t| t.parse::<f64>().is_ok())
                && w.fitting != "lug"
            {
                assert!(w.ferrule.contains(" mm2 "), "{}: {:?}", w.number, w.ferrule);
            }
        }
    }
}

#[test]
fn the_ferrule_text_follows_the_standard_and_says_lug_for_a_lug() {
    let standard = novo_reports::FerruleStandard::default();
    assert_eq!(
        ferrule_text(&standard, "1.5 mm2 black", "ferrule"),
        "1.5 mm2 black, 10 mm"
    );
    assert_eq!(ferrule_text(&standard, "1.5 mm2 black", "lug"), "lug");
    assert_eq!(ferrule_text(&standard, "size not set", ""), "");
}

fn all_text(path: &std::path::Path) -> String {
    let doc = lopdf::Document::load_mem(&std::fs::read(path).unwrap()).unwrap();
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    pages
        .iter()
        .map(|p| doc.extract_text(&[*p]).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_changed_layout_marks_the_pack_out_of_date_and_the_changed_cards_on_the_next_print() {
    use super::heat_settings::{VentSpec, Wall};
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    // Nothing printed yet: nothing to be out of date.
    assert_eq!(d.app.workpack_stale_note(), None);

    let first = d.app.export_workpack().expect("first print");
    let text = all_text(&first);
    assert!(text.contains("Revision "), "{text}");
    assert!(text.contains("printed 20"), "{text}");
    assert!(
        !text.contains("CHANGED since"),
        "a first print has no earlier one"
    );
    assert_eq!(d.app.workpack_stale_note(), None);

    // The cooling fix adds a grille cut-out: the pack no longer matches.
    let mut grille = VentSpec::new(Wall::Top);
    grille.width_mm = 146.99;
    grille.height_mm = 146.99;
    store_vents(&mut d, cab, &[grille]);
    let note = d.app.workpack_stale_note().expect("out of date");
    assert!(note.contains("out of date"), "{note}");
    assert!(note.contains("1 card changed"), "{note}");

    let second = d.app.export_workpack().expect("second print");
    let text = all_text(&second);
    assert!(
        text.contains("1 card CHANGED since the print of"),
        "cover: {text}"
    );
    assert!(
        text.contains("CHANGED since the print of 20") && text.contains("check this card"),
        "card banner: {text}"
    );
    // Round saw size: float noise from the part data does not reach the bench.
    assert!(text.contains("147 x 147"), "{text}");
    assert!(!text.contains("146.99"), "{text}");
    assert_eq!(d.app.workpack_stale_note(), None, "printed again");

    // A third print without a change marks nothing and says unchanged.
    let third = d.app.export_workpack().expect("third print");
    let text = all_text(&third);
    assert!(text.contains("unchanged since the print of"), "{text}");
    assert!(!text.contains("check this card"), "{text}");
}

#[test]
fn a_card_printed_on_its_own_clears_its_part_of_the_warning() {
    use super::heat_settings::{VentSpec, Wall};
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    d.app.export_workpack().expect("print");
    store_vents(&mut d, cab, &[VentSpec::new(Wall::Top)]);
    assert!(d.app.workpack_stale_note().is_some());
    let mark = work_packs(&d.app.machine().unwrap())[0]
        .cards
        .iter()
        .find(|c| c.kind == CardKind::Opening)
        .unwrap()
        .mark
        .clone();
    let card = d.app.export_workpack_step(cab, &mark).expect("the card");
    assert!(all_text(&card).contains("CHANGED since the print of"));
    assert_eq!(d.app.workpack_stale_note(), None);
}

#[test]
fn the_cards_never_replace_the_open_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    let project = dir.path().join("Plant 3.novo");
    let before = std::fs::read(&project).unwrap();
    assert!(before.starts_with(b"SQLite format 3"));

    assert_eq!(d.app.export_workpack_to(&project), None);
    let last = d.app.messages().last().unwrap();
    assert_eq!(last.level, Level::Error);
    assert!(last.text.contains("open project file"), "{}", last.text);
    assert_eq!(d.app.write_step_card(&project, b"%PDF-"), None);
    assert_eq!(d.app.messages().last().unwrap().level, Level::Error);
    assert_eq!(std::fs::read(&project).unwrap(), before);
    let _ = cab;
}

#[test]
fn a_workshop_pack_print_counts_as_the_work_cards_print() {
    use super::heat_settings::{VentSpec, Wall};
    let dir = tempfile::tempdir().unwrap();
    let (mut d, cab) = vent_cabinet(dir.path());
    d.app.export_workshop().expect("folder");
    assert_eq!(d.app.workpack_stale_note(), None);
    store_vents(&mut d, cab, &[VentSpec::new(Wall::Top)]);
    assert!(d.app.workpack_stale_note().is_some());
    d.app.export_workshop().expect("folder again");
    assert_eq!(
        d.app.workpack_stale_note(),
        None,
        "the folder holds the new cards"
    );
    let cards = dir.path().join("Plant 3 workshop/1 Work cards.pdf");
    assert!(all_text(&cards).contains("CHANGED since the print of"));
}

#[test]
fn a_new_project_forgets_the_last_print_of_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, _cab) = vent_cabinet(dir.path());
    d.app.export_workpack().expect("print");
    assert!(d.app.wb.workpack_print.is_some());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.app.answer_save_changes(&ctx, Some(false));
    d.frame();
    assert!(d.app.wb.workpack_print.is_none(), "the old print is kept");
    assert!(d.app.wb.workpack_stale.is_none());
}
