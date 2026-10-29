//! The drive hoist fixture builds and reads back as one drive hoist that the run accepts.

use novo_core::Document;

#[test]
fn the_fixture_builds_with_one_drive_hoist_and_its_bindings() {
    let mut doc = Document::default();
    let h = novo_tests::hoist_drive::drive_hoist(&mut doc).unwrap();
    let project = doc.project();
    let motor = project.device(h.motor).unwrap();
    assert!(motor.properties.contains_key("dh_mass_kg"));
    assert!(project.symbols().any(|s| s.id == h.motor_sym));
    for id in [h.up_step1, h.up_step2] {
        assert!(project.symbols().any(|s| s.id == id));
    }
    for key in [
        "dh_in_up",
        "dh_brake_relay",
        "dh_brake_feed",
        "dh_sense_boost_hz",
    ] {
        assert!(motor.properties.contains_key(key), "{key}");
    }
}
