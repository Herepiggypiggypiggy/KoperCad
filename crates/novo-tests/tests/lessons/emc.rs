//! The EMC plate lesson's sample project is the shields fixture of the EMC
//! lane (`qa/fixtures/emc-cabinet-shields.novo`): a galvanised plate, a drive
//! and two shielded cables that carry all three shield-end states.

use super::common::{assert_lint, check_file};

#[test]
fn emc_on_the_plate_files_are_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/emc-cabinet-shields.novo");
    let project = novo_storage::ProjectFile::open(path)
        .and_then(|f| f.load())
        .expect("the shields fixture opens");
    check_file("emc-on-the-plate", "project.json", &project);
    check_file("emc-fix-it", "project.json", &project);
    assert_lint(&[("emc-on-the-plate", &project), ("emc-fix-it", &project)]);
}
