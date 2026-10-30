//! Writes the travel drive project (`novo_tests::travel`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example travel_fixture -- qa/fixtures/travel-drive.novo
//!   cargo run -p novo-tests --example travel_fixture -- qa/fixtures/travel-brake-wind.novo wind

use novo_core::Document;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("a path for the .novo file as the first argument");
    let mut doc = Document::default();
    if std::env::args().nth(2).as_deref() == Some("wind") {
        novo_tests::travel::travel_with_brake_and_wind(&mut doc).expect("the project builds");
    } else {
        novo_tests::travel::travel(&mut doc).expect("the travel project builds");
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
