//! The heat preview (the picture before Simulate) against the lesson project and the 2D solver.

use novo_core::{Device, DeviceId, Project, Structure};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

use super::heat::{Input, drawn_peak_tags, drawn_temps, items_of};

const LESSON_PROJECT: &str = include_str!("../../../../assets/lessons/wire-routing/project.json");

fn lesson_cabinet() -> (
    Project,
    novo_layout::state::CabinetLayout,
    novo_core::StructureId,
) {
    let mut project = Project::from_json(LESSON_PROJECT).unwrap();
    let cab = project
        .structures()
        .find(|s: &&Structure| s.enclosure)
        .expect("a cabinet")
        .id;
    // The lesson starts without an inside size (the lesson asks for one later); a 2D run needs it.
    project
        .transact("inside size", |tx| {
            novo_layout::model::set_enclosure(
                tx,
                cab,
                &novo_layout::enclosure::Enclosure {
                    width: 600.0,
                    height: 800.0,
                    depth: 250.0,
                    plate_setback: 20.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )
        })
        .unwrap();
    let layout = novo_layout::model::load(&project, cab).unwrap();
    (project, layout, cab)
}

/// The lesson cabinet as the Heat view reads it: the preview input, and the 2D solver's devices.
fn lesson_heat() -> (Input, Vec<novo_thermal::DeviceResult>) {
    let (project, layout, cab) = lesson_cabinet();
    let tag = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_default()
    };
    let input = Input {
        plate: (layout.plate.width, layout.plate.height),
        ambient: 35.0,
        items: items_of(&project, &layout, &tag),
        solution: None,
        kept: None,
    };
    let bridge = scenario_from_project(&project, cab, &BridgeOptions::default()).unwrap();
    let result = novo_thermal::simulate_steady(&bridge.scenario).unwrap();
    (input, result.devices)
}

#[test]
fn the_preview_puts_the_transformer_first_as_the_2d_solver_does() {
    let (input, solved) = lesson_heat();
    let preview = drawn_temps(&input);
    let hottest = |temps: Vec<(u64, f64)>| {
        temps
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .0
    };
    let t1 = input.items.iter().find(|i| i.tag.ends_with("-T1")).unwrap();
    assert_eq!(t1.loss_w, 20.0);
    let by_preview = hottest(input.items.iter().map(|i| i.id).zip(preview).collect());
    let by_solver = hottest(solved.iter().map(|d| (d.id, d.surface_c)).collect());
    assert_eq!(by_preview, t1.id, "the preview's hottest device");
    assert_eq!(by_solver, t1.id, "the 2D solver's hottest device");
}

#[test]
fn the_preview_stays_near_the_2d_solver_on_the_lesson_cabinet() {
    let (input, solved) = lesson_heat();
    let preview = drawn_temps(&input);
    for (item, temp) in input.items.iter().zip(&preview) {
        let d = solved.iter().find(|d| d.id == item.id).unwrap();
        // The preview says "rough" in the legend: within 10 K of the solver's surface
        // temperature, and a device without loss at the air around it.
        assert!(
            (temp - d.surface_c).abs() < 10.0,
            "{}: preview {temp:.1}, 2D surface {:.1}",
            item.tag,
            d.surface_c
        );
    }
    // Larger loss on a comparable footprint is never the cooler reading.
    let temp_of = |tag: &str| {
        let i = input
            .items
            .iter()
            .position(|i| i.tag.ends_with(tag))
            .unwrap();
        preview[i]
    };
    assert!(temp_of("-T1") > temp_of("-F1") + 8.0);
    assert!(temp_of("-F1") > temp_of("-F3"));
    assert!(temp_of("-K1") > temp_of("-S0"));
}

#[test]
fn the_hot_spot_card_names_the_transformer() {
    let (input, _) = lesson_heat();
    let tags = drawn_peak_tags(&input);
    assert!(tags.first().is_some_and(|t| t.ends_with("-T1")), "{tags:?}");
}
