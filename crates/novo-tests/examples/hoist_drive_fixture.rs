//! Writes the drive hoist project (`novo_tests::hoist_drive`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example hoist_drive_fixture -- qa/fixtures/hoist-drive.novo
//!   cargo run -p novo-tests --example hoist_drive_fixture -- qa/fixtures/hoist-drive-floor.novo floor

use novo_core::Document;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("a path for the .novo file as the first argument");
    let mut doc = Document::default();
    if std::env::args().nth(2).as_deref() == Some("floor") {
        novo_tests::hoist_drive::drive_hoist_on_floor(&mut doc).expect("the project builds");
    } else {
        novo_tests::hoist_drive::drive_hoist(&mut doc).expect("the drive hoist project builds");
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
