//! Assembly order on the cabinet lesson projects and on hand-built layouts.

use std::collections::BTreeSet;

use novo_core::{Device, Project, Structure};
use novo_layout::assembly::{Phase, Step, WireJob, order_problems, steps};
use novo_layout::model::load;
use novo_layout::state::CabinetLayout;
use novo_layout::{Duct, Margins, Placement, Plate, Rail, Size};

fn lesson(path: &str) -> Project {
    let file = format!("{}/../../assets/lessons/{path}", env!("CARGO_MANIFEST_DIR"));
    Project::from_json(&std::fs::read_to_string(file).expect("lesson file")).expect("project")
}

fn cabinet_steps(project: &Project, wires: &[WireJob]) -> (CabinetLayout, Vec<Step>) {
    let cab = project
        .structures()
        .find(|s: &&Structure| s.enclosure && s.plate.is_some())
        .expect("a plated cabinet");
    let layout = load(project, cab.id).expect("layout");
    let name = |id: u64| {
        project
            .get::<Device>(novo_core::DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_default()
    };
    let list = steps(&layout, &name, wires);
    (layout, list)
}

fn place(id: u64, x: f64, y: f64, rail: Option<u64>) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail,
    }
}

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 800.0,
        height: 600.0,
        margin: 10.0,
    });
    l.rails = vec![
        Rail {
            id: 2,
            x: 50.0,
            y: 300.0,
            length: 400.0,
            vertical: false,
        },
        Rail {
            id: 1,
            x: 50.0,
            y: 100.0,
            length: 400.0,
            vertical: false,
        },
    ];
    l.ducts = vec![Duct {
        id: 3,
        x: 50.0,
        y: 250.0,
        length: 700.0,
        width: 50.0,
        vertical: false,
    }];
    l.placements = vec![
        place(12, 80.0, 400.0, None),
        place(11, 200.0, 100.0, Some(1)),
        place(10, 100.0, 100.0, Some(1)),
        place(13, 100.0, 300.0, Some(2)),
    ];
    l
}

fn names(steps: &[Step]) -> Vec<String> {
    steps
        .iter()
        .map(|s| format!("{}:{}", s.phase.name(), s.mark))
        .collect()
}

#[test]
fn rails_then_ducts_then_devices_then_wires_then_labels() {
    let wires = [WireJob {
        name: "W1".into(),
        text: "Run A to B".into(),
    }];
    let list = steps(&layout(), &|id| format!("-Q{id}"), &wires);
    assert_eq!(
        names(&list),
        [
            "Rails:R1",
            "Rails:R2",
            "Ducts:D1",
            "Devices:-Q10",
            "Devices:-Q11",
            "Devices:-Q13",
            "Devices:-Q12",
            "Wires:W1",
            "Labels:-Q10",
            "Labels:-Q11",
            "Labels:-Q13",
            "Labels:-Q12",
        ]
    );
    assert!(
        order_problems(&list).is_empty(),
        "{:?}",
        order_problems(&list)
    );
}

#[test]
fn a_device_follows_its_rail_and_wires_follow_every_device() {
    let wires = [WireJob {
        name: "W1".into(),
        text: String::new(),
    }];
    let list = steps(&layout(), &|id| format!("-Q{id}"), &wires);
    let rail_of = |subject: u64| {
        list.iter()
            .find(|s| s.subject == Some(subject) && s.phase == Phase::Rails)
            .unwrap()
            .number
    };
    let q10 = list
        .iter()
        .find(|s| s.subject == Some(10) && s.phase == Phase::Devices)
        .unwrap();
    assert_eq!(q10.after, vec![rail_of(1)]);
    let wire = list.iter().find(|s| s.phase == Phase::Wires).unwrap();
    let devices: Vec<usize> = list
        .iter()
        .filter(|s| s.phase == Phase::Devices)
        .map(|s| s.number)
        .collect();
    assert_eq!(wire.after, devices);
    let label = list
        .iter()
        .find(|s| s.phase == Phase::Labels && s.subject == Some(12))
        .unwrap();
    assert_eq!(label.after.len(), 1);
}

#[test]
fn the_order_does_not_depend_on_the_input_order() {
    let mut shuffled = layout();
    shuffled.rails.reverse();
    shuffled.placements.reverse();
    let name = |id: u64| format!("-Q{id}");
    assert_eq!(steps(&layout(), &name, &[]), steps(&shuffled, &name, &[]));
}

#[test]
fn a_device_on_a_missing_rail_is_mounted_on_the_plate() {
    let mut l = layout();
    l.placements = vec![place(10, 100.0, 100.0, Some(99))];
    let list = steps(&l, &|id| format!("-Q{id}"), &[]);
    let d = list.iter().find(|s| s.phase == Phase::Devices).unwrap();
    assert!(d.text.starts_with("Screw"));
    assert!(d.after.is_empty());
}

#[test]
fn order_problems_names_a_backwards_dependency_and_phase() {
    let mut list = steps(&layout(), &|id| format!("-Q{id}"), &[]);
    list[0].after = vec![5];
    list.swap(0, 3);
    let problems = order_problems(&list);
    assert!(
        problems.iter().any(|p| p.contains("is numbered")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("comes before step")),
        "{problems:?}"
    );
}

#[test]
fn the_cabinet_lesson_projects_build_in_a_sound_order() {
    for file in ["cabinet-layout/finished.json", "wire-routing/project.json"] {
        let (layout, list) = cabinet_steps(&lesson(file), &[]);
        assert!(!layout.placements.is_empty(), "{file}");
        assert!(
            order_problems(&list).is_empty(),
            "{file}: {:?}",
            order_problems(&list)
        );
        // Every placed device is mounted once and labelled once.
        for phase in [Phase::Devices, Phase::Labels] {
            let subjects: BTreeSet<u64> = list
                .iter()
                .filter(|s| s.phase == phase)
                .filter_map(|s| s.subject)
                .collect();
            let want: BTreeSet<u64> = layout.placements.iter().map(|p| p.id).collect();
            assert_eq!(subjects, want, "{file} {phase:?}");
        }
        assert_eq!(
            list.iter().filter(|s| s.phase == Phase::Rails).count(),
            layout.rails.len(),
            "{file}"
        );
    }
}

#[test]
fn the_wire_routing_lesson_cuts_the_ducts_before_any_device() {
    let (layout, list) = cabinet_steps(&lesson("wire-routing/project.json"), &[]);
    assert!(!layout.ducts.is_empty());
    let last_duct = list.iter().rposition(|s| s.phase == Phase::Ducts).unwrap();
    let first_device = list.iter().position(|s| s.phase == Phase::Devices).unwrap();
    assert!(last_duct < first_device);
}
