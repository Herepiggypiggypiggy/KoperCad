//! Writes the closed-loop winch project (`novo_tests::closed_winch`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-pair.novo pair

use novo_core::Document;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("a path for the .novo file as the first argument");
    let pair = args.next().as_deref() == Some("pair");
    let mut doc = Document::default();
    if pair {
        novo_tests::closed_winch::closed_winch_pair(&mut doc)
            .expect("the two-winch project builds");
    } else {
        novo_tests::closed_winch::closed_winch(&mut doc)
            .expect("the closed-loop winch project builds");
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
