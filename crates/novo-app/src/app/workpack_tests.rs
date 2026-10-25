//! Work cards: the same build order as the print pack, every wire on one
//! card, and the export next to the project.

use novo_core::{DeviceTag, Project, Value};
use novo_layout::workpack::CardKind;
use novo_reports::{AssemblyChecklist, Report};

use super::Level;
use super::assembly::order_sections;
use super::cabinet::Ask;
use super::gate1_tests::Drive;
use super::workpack::work_packs;
use crate::actions::Action;

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
