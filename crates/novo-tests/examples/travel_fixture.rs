//! Writes the travel drive project (`novo_tests::travel`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example travel_fixture -- qa/fixtures/travel-drive.novo

use novo_core::Document;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("a path for the .novo file as the first argument");
    let mut doc = Document::default();
    novo_tests::travel::travel(&mut doc).expect("the travel project builds");
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
