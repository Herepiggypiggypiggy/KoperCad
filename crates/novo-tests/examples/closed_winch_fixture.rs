//! Writes the closed-loop winch project (`novo_tests::closed_winch`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-pair.novo pair
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-channel.novo channel

use novo_core::Document;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("a path for the .novo file as the first argument");
    let mode = args.next();
    let pair = mode.as_deref() == Some("pair");
    let mut doc = Document::default();
    if mode.as_deref() == Some("channel") {
        novo_tests::closed_winch::closed_winch_channel(&mut doc)
            .expect("the winch project with an encoder channel builds");
    } else if pair {
        novo_tests::closed_winch::closed_winch_pair(&mut doc)
            .expect("the two-winch project builds");
    } else {
        novo_tests::closed_winch::closed_winch(&mut doc)
            .expect("the closed-loop winch project builds");
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
