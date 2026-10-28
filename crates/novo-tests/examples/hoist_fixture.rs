//! Writes the two-speed hoist project (`novo_tests::hoist`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example hoist_fixture -- qa/fixtures/hoist-two-speed.novo

use novo_core::Document;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("a path for the .novo file as the first argument");
    let mut doc = Document::default();
    novo_tests::hoist::hoist(&mut doc).expect("the hoist builds");
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
