//! The closed-loop winch fixture builds and reads back as one winch that the run accepts, with the
//! devices a defect can be injected on.

use novo_core::Document;

#[test]
fn the_fixture_builds_with_one_closed_loop_winch_and_its_bindings() {
    let mut doc = Document::default();
    let w = novo_tests::closed_winch::closed_winch(&mut doc).unwrap();
    let project = doc.project();
    let motor = project.device(w.motor).unwrap();
    assert!(motor.properties.contains_key("cw_mass_kg"));
    assert!(project.symbols().any(|s| s.id == w.motor_sym));
    for id in [w.up_step1, w.up_step2] {
        assert!(project.symbols().any(|s| s.id == id));
    }
    for key in [
        "cw_in_up",
        "cw_in_fast",
        "cw_sto_a",
        "cw_sto_b",
        "cw_encoder",
        "cw_brake_relay",
        "cw_brake_feed",
    ] {
        let tag = motor.properties.get(key).map(|v| v.to_string());
        let tag = tag.unwrap_or_else(|| panic!("{key} is not set"));
        assert!(
            project.devices().any(|d| d.tag.to_string() == tag),
            "{key} names {tag}, which is not a device of the drawing"
        );
    }
}
