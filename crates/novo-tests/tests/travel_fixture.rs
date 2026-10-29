//! The travel fixture builds and reads back as one travel axis.

use novo_core::Document;

#[test]
fn the_fixture_builds_with_one_axis_and_its_bindings() {
    let mut doc = Document::default();
    let t = novo_tests::travel::travel(&mut doc).unwrap();
    let project = doc.project();
    let motor = project.device(t.motor).unwrap();
    assert!(motor.properties.contains_key("trav_mass_kg"));
    assert!(project.symbols().any(|s| s.id == t.motor_sym));
    for id in [t.forward_step1, t.forward_step2, t.reverse, t.limit] {
        assert!(project.symbols().any(|s| s.id == id));
    }
}
