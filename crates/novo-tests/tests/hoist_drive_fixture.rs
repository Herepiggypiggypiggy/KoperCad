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

#[test]
fn the_limits_variant_has_a_final_limit_a_slow_down_switch_and_a_clearance() {
    let mut doc = Document::default();
    let h = novo_tests::hoist_drive::drive_hoist_with_limits(&mut doc).unwrap();
    let motor = doc.project().device(h.motor).unwrap();
    for key in [
        "dh_limit_up",
        "dh_limit_up_m",
        "dh_slow_up",
        "dh_slow_up_m",
        "dh_clearance_up_m",
    ] {
        assert!(motor.properties.contains_key(key), "{key}");
    }
    let tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    assert!(tags.iter().any(|t| t.ends_with("LS-F")));
    assert!(tags.iter().any(|t| t.ends_with("LS-S")));
}
