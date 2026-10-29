//! The radio-hoist QA fixture (lane C2): a receiver with its E-stop chain and main contact, two
//! reversing contactors, a two-speed winding pair and a Dahlander motor, with one generic radio
//! bound to the receiver.
//!
//! Regenerate with `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test radio_hoist_fixture`
//! and commit `qa/fixtures/radio-hoist.novo`.

use std::collections::BTreeSet;

use novo_controller::{ProjectControllers, Target};
use novo_core::{Document, Value};
use novo_sim::pins::{force_pole, pole_of_pin};
use novo_sim::{MotorRun, device_states, rest_state, settle};
use novo_tests::radio_hoist_with_controller;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/radio-hoist.novo")
}

fn stored(project: &novo_core::Project) -> String {
    match project.info().properties.get("controllers.project") {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

#[test]
fn the_committed_fixture_carries_the_radio_and_its_bindings() {
    if std::env::var_os("KOPERCAD_WRITE_FIXTURES").is_some() {
        let mut doc = Document::default();
        radio_hoist_with_controller(&mut doc).expect("fixture builds");
        let path = fixture_path();
        let _ = std::fs::remove_file(&path);
        novo_storage::ProjectFile::create(&path, doc.project()).expect("fixture is written");
    }
    let project = novo_storage::ProjectFile::open(fixture_path())
        .and_then(|f| f.load())
        .expect("fixture opens");
    let data = ProjectControllers::from_text(&stored(&project)).expect("controller data reads");
    assert_eq!(data.instances.len(), 1);
    assert_eq!(data.bindings.len(), 6);
    assert_eq!(project.pages().count(), 2);
}

#[test]
fn the_fixture_has_no_customer_or_vendor_names() {
    let mut doc = Document::default();
    let (_, text) = radio_hoist_with_controller(&mut doc).expect("fixture builds");
    let lower = text.to_lowercase();
    for word in ["demag", "telecrane", "schneider", "siemens", "eplan"] {
        assert!(
            !lower.contains(word),
            "{word} in the stored controller data"
        );
    }
}

#[test]
fn a_radio_closes_the_receiver_and_the_circuit_follows_without_the_app() {
    // The engine, the pin lookup and the solver agree with no window involved: this is the path
    // the live view takes every frame, written out.
    let mut doc = Document::default();
    let (h, text) = radio_hoist_with_controller(&mut doc).expect("fixture builds");
    let project = doc.project();
    let data = ProjectControllers::from_text(&text).unwrap();
    let (mut set, table, notes) = data
        .restore(&{
            let mut lib = novo_controller::ProfileLibrary::new();
            lib.insert(
                novo_controller::Profile::load(
                    &novo_controller::import::library::build("generic-radio-3x2")
                        .unwrap()
                        .profile
                        .to_toml(),
                )
                .unwrap(),
            );
            lib
        })
        .expect("restores");
    assert!(notes.is_empty(), "{notes:?}");
    set.act(
        "radio-1",
        &novo_controller::Action::Press {
            control: "start".into(),
        },
    )
    .unwrap();
    set.act(
        "radio-1",
        &novo_controller::Action::SetPosition {
            control: "hoist".into(),
            position: "up2".into(),
        },
    )
    .unwrap();
    let levels = table.resolve(&set);
    let mut poles = rest_state(project);
    for (target, level) in &levels {
        let Target::Pin(designation) = target else {
            panic!("only pins are bound: {target}");
        };
        force_pole(
            &mut poles,
            pole_of_pin(project, designation).unwrap(),
            *level != 0.0,
        );
    }
    let netlist = project.netlist();
    let settled = settle(
        project,
        &netlist,
        &poles,
        &BTreeSet::new(),
        &BTreeSet::new(),
    );
    let on = &settled.energized;
    assert!(on.contains(&h.k1_coil) && on.contains(&h.k4_coil));
    assert!(!on.contains(&h.k2_coil) && !on.contains(&h.k3_coil));
    let states = device_states(project, &netlist, &settled);
    assert!(matches!(
        states.motors[&h.motor],
        MotorRun::Running { speed: 1, .. }
    ));
}
