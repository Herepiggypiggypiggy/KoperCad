//! The radio-machine QA fixture (lane L5b): a hoist and a travel axis known to the machine model
//! only, with one generic radio bound to their ports.
//!
//! Regenerate with `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test radio_machine_fixture`
//! and commit `qa/fixtures/radio-machine.novo`.

use novo_controller::{
    Action, BindingTable, ControllerSet, Profile, ProfileLibrary, ProjectControllers,
    import::library,
};
use novo_core::Document;
use novo_machine::commands::{CommandedCircuit, Commands, Port, Wiring};
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::HoistLoop;
use novo_machine::port::FakeCircuit;
use novo_machine::spec::hoist_from_properties;
use novo_tests::radio_machine_with_controller;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/radio-machine.novo")
}

fn stored(project: &novo_core::Project) -> String {
    match project.info().addon_data.get("controllers", "project") {
        Some(b) => b.text.clone(),
        None => String::new(),
    }
}

fn restore(text: &str) -> (ControllerSet, BindingTable) {
    let mut lib = ProfileLibrary::new();
    lib.insert(
        Profile::load(
            &library::build("generic-radio-3x2")
                .unwrap()
                .profile
                .to_toml(),
        )
        .unwrap(),
    );
    let (set, table, notes) = ProjectControllers::from_text(text)
        .unwrap()
        .restore(&lib)
        .expect("restores");
    assert!(notes.is_empty(), "{notes:?}");
    (set, table)
}

#[test]
fn the_committed_fixture_carries_the_radio_and_its_machine_bindings() {
    if std::env::var_os("KOPERCAD_WRITE_FIXTURES").is_some() {
        let mut doc = Document::default();
        radio_machine_with_controller(&mut doc).expect("fixture builds");
        let path = fixture_path();
        let _ = std::fs::remove_file(&path);
        novo_storage::ProjectFile::create(&path, doc.project()).expect("fixture is written");
    }
    let project = novo_storage::ProjectFile::open(fixture_path())
        .and_then(|f| f.load())
        .expect("fixture opens");
    let data = ProjectControllers::from_text(&stored(&project)).expect("controller data reads");
    assert_eq!(data.instances.len(), 1);
    assert_eq!(data.bindings.len(), 14);
    assert!(
        data.bindings.iter().all(
            |b| matches!(&b.target, novo_controller::Target::ModulePath(p) if p.contains('@'))
        )
    );
    assert_eq!(project.pages().count(), 1);
}

#[test]
fn the_fixture_has_no_customer_or_vendor_names() {
    let mut doc = Document::default();
    let (_, text) = radio_machine_with_controller(&mut doc).expect("fixture builds");
    let lower = text.to_lowercase();
    for word in ["demag", "telecrane", "schneider", "siemens", "eplan"] {
        assert!(!lower.contains(word), "{word} in the controller data");
    }
}

/// The path of the live view without a window: signals, resolved machine ports, commands, the
/// commanded circuit and the hoist loop, stepped at a fixed 5 ms.
#[test]
fn the_radio_lifts_the_hook_through_its_machine_port_and_the_estop_stops_it() {
    let mut doc = Document::default();
    let (_, text) = radio_machine_with_controller(&mut doc).expect("fixture builds");
    let (mut set, table) = restore(&text);
    let properties = doc
        .project()
        .devices()
        .find(|d| d.tag.to_string() == "-M1")
        .unwrap()
        .properties
        .clone();
    let get = |k: &str| properties.get(k).map(|v| v.to_string());
    let module = hoist_from_properties("-M1", "-M1", &get).unwrap().unwrap();
    let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
    lp.hoists.push(module);
    let run = |lp: &mut HoistLoop, set: &ControllerSet, seconds: f64| {
        let mut inner = FakeCircuit::new();
        for _ in 0..(seconds / 0.005).round() as usize {
            let mut commands = Commands::new();
            for (path, level) in table.resolve_machine(set) {
                let port = Port::parse(path.port().unwrap()).unwrap();
                commands.set(path.module_id(), port, level != 0.0);
            }
            let wirings: Vec<Wiring> = lp
                .hoists
                .iter_mut()
                .filter(|h| !commands.bound(&h.id).is_empty())
                .map(|h| Wiring::hoist(h, &commands.bound(&h.id)))
                .collect();
            let mut circuit = CommandedCircuit::new(&mut inner, &wirings, &commands);
            lp.step(&mut circuit);
        }
    };
    let act = |set: &mut ControllerSet, a: Action| {
        set.act("radio-1", &a).unwrap();
    };
    let press = |c: &str| Action::Press { control: c.into() };
    let pos = |c: &str, p: &str| Action::SetPosition {
        control: c.into(),
        position: p.into(),
    };

    act(&mut set, press("start"));
    run(&mut lp, &set, 1.0);
    assert_eq!(lp.hoists[0].height_m(), 0.0, "nothing is pressed");
    act(&mut set, pos("hoist", "up2"));
    run(&mut lp, &set, 3.0);
    let rising = lp.hoists[0].height_m();
    assert!(rising > 0.5, "step 2 lifts the hook: {rising} m");
    act(&mut set, pos("hoist", "off"));
    run(&mut lp, &set, 4.0);
    let rest = lp.hoists[0].height_m();
    run(&mut lp, &set, 2.0);
    assert_eq!(lp.hoists[0].height_m(), rest, "released: the brake holds");
    // E-stop with the rocker held
    act(&mut set, pos("hoist", "up2"));
    run(&mut lp, &set, 1.0);
    act(&mut set, press("estop"));
    run(&mut lp, &set, 3.0);
    let stopped = lp.hoists[0].height_m();
    run(&mut lp, &set, 2.0);
    assert_eq!(lp.hoists[0].height_m(), stopped, "stays put while pressed");
}
