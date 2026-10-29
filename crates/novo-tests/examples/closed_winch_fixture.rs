//! Writes the closed-loop winch project (`novo_tests::closed_winch`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo

use novo_core::Document;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("a path for the .novo file as the first argument");
    let mut doc = Document::default();
    novo_tests::closed_winch::closed_winch(&mut doc).expect("the closed-loop winch project builds");
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
