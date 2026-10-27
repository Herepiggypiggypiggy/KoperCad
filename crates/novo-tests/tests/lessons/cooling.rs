//! The cooling what-if lesson's sample project is the hot cabinet fixture of
//! the cooling lane (`qa/fixtures/cabinet-cooling-advice.novo`): a sealed
//! enclosure with a device that runs over its limit on a warm day.

use super::common::{assert_lint, check_file};

#[test]
fn cooling_what_if_files_are_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/cabinet-cooling-advice.novo");
    let project = novo_storage::ProjectFile::open(path)
        .and_then(|f| f.load())
        .expect("the cooling fixture opens");
    check_file("cooling-what-if", "project.json", &project);
    assert_lint(&[("cooling-what-if", &project)]);
}
