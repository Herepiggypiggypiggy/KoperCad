//! The pump station and motor control centre templates: structure, pages,
//! circuits, clean checks, cover pages without the crane rows, and the
//! commands.

use novo_core::{DeviceTag, Document, PageKind};
use novo_reports::{FrontMatter, Report};

use super::gate1_tests::Drive as Harness;
use super::template_plants::{Plant, PlantOptions, build_plant_template_on};
use crate::actions::Action;

fn built(plant: Plant, count: u8, extra: bool) -> Document {
    let mut doc = Document::default();
    let options = PlantOptions {
        plant,
        count,
        extra,
    };
    build_plant_template_on(&mut doc, "2026-10-30", options).expect("the template builds");
    doc
}

fn devices_described(doc: &Document, text: &str) -> usize {
    doc.project()
        .devices()
        .filter(|d| d.description.contains(text))
        .count()
}

#[test]
fn the_only_issues_are_the_open_points_the_circuits_name() {
    for (plant, count, extra) in [
        (Plant::PumpStation, 2, false),
        (Plant::PumpStation, 4, true),
        (Plant::ControlCentre, 4, false),
        (Plant::ControlCentre, 8, true),
    ] {
        let doc = built(plant, count, extra);
        let other: Vec<_> = novo_checks::check(doc.project())
            .into_iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .filter(|p| p.kind != novo_checks::ProblemKind::UnconnectedPin)
            .map(|p| format!("{:?}: {}", p.severity, p.message))
            .collect();
        assert!(other.is_empty(), "{plant:?} {count} {extra}: {other:#?}");
    }
}

#[test]
fn the_count_makes_the_pages_and_the_nodes() {
    for count in 1..=4u8 {
        let doc = built(Plant::PumpStation, count, false);
        let p = doc.project();
        for n in 1..=4u8 {
            let has = p
                .structure_by_tag(&DeviceTag::parse(&format!("=PMP+PM{n}")).unwrap())
                .is_some();
            assert_eq!(has, n <= count, "pump {n} of {count}");
        }
        let pages = p.pages().filter(|pg| pg.title.starts_with("Pump ")).count();
        assert_eq!(pages, usize::from(count));
        assert_eq!(
            devices_described(&doc, "Motor protection breaker"),
            usize::from(count)
        );
    }
    for count in 1..=8u8 {
        let doc = built(Plant::ControlCentre, count, false);
        let p = doc.project();
        for n in 1..=8u8 {
            let node = p.structure_by_tag(&DeviceTag::parse(&format!("=MCC+F{n}")).unwrap());
            assert_eq!(node.is_some(), n <= count, "feeder {n} of {count}");
            if let Some(node) = node {
                assert!(node.enclosure, "a feeder section is a cabinet");
            }
        }
    }
}

#[test]
fn the_variant_picks_the_circuit() {
    // Pumps: a thermistor relay per pump when asked for.
    let plain = built(Plant::PumpStation, 3, false);
    let with = built(Plant::PumpStation, 3, true);
    assert_eq!(devices_described(&plain, "hermistor"), 0);
    assert_eq!(
        devices_described(&with, "hermistor"),
        6,
        "a sensor and a relay per pump"
    );
    // Feeders: one contactor pair per reversing feeder.
    let dol = built(Plant::ControlCentre, 3, false);
    let rev = built(Plant::ControlCentre, 3, true);
    assert!(rev.project().devices().count() > dol.project().devices().count());
}

#[test]
fn the_cover_pages_come_first_and_leave_out_the_crane_rows() {
    for plant in [Plant::PumpStation, Plant::ControlCentre] {
        let doc = built(plant, 2, false);
        let order = doc.project().pages_in_order();
        assert!(
            matches!(&order[0].kind, PageKind::Generated { generator } if generator == "front")
        );
        assert_eq!(order[0].name, "00");
        let out = FrontMatter.generate(doc.project());
        assert_eq!(out.sections.len(), 5);
        let rows = out.section("project").unwrap().text_rows();
        let fields: Vec<_> = rows.iter().map(|r| r[0].as_str()).collect();
        assert_eq!(fields, ["Customer", "Site", "Order number"]);
        let general = out.section("general").unwrap().text_rows();
        assert!(
            general
                .iter()
                .any(|r| r[0] == "Control voltage" && r[1] == "48 V AC / 24 V DC")
        );
        assert_eq!(
            doc.project().info().name,
            plant.project_name(),
            "named after the template"
        );
    }
}

#[test]
fn the_new_commands_open_their_window_and_an_unsaved_project() {
    for (id, action, plant) in [
        ("file.new_pump", Action::NewPumpProject, Plant::PumpStation),
        ("file.new_mcc", Action::NewMccProject, Plant::ControlCentre),
    ] {
        // A fresh app each: an open, changed project would ask to save first.
        let dir = tempfile::tempdir().unwrap();
        let mut d = Harness::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        assert!(super::workbench::app_runs(id));
        assert!(d.app.is_enabled(action));
        d.app.run_command(&ctx, id, None);
        d.frame();
        let asked = d.app.wb.plant_new.expect("the options window opens");
        assert_eq!(asked.plant, plant);
        d.app.new_plant_project(asked);
        d.frame();
        assert_eq!(d.doc().project().info().name, plant.project_name());
        assert!(
            d.app.document().is_some_and(|doc| doc.is_dirty()),
            "a template is a new project, not an example"
        );
        // The Project settings fields skip the crane rows.
        assert!(!novo_reports::front_keys::shows_crane_fields(
            &d.doc().project().info().properties
        ));
    }
}
