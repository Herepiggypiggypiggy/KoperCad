//! Writes the closed-loop winch project (`novo_tests::closed_winch`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-pair.novo pair
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-walk.novo walk
//!
//! `walk` is the single winch with one saved walk-through (an STO channel coil opens while the
//! hook rises; the drive must trip with the discrepancy code), which Simulate > Defects and
//! walk-throughs lists and runs.

use novo_core::{Document, Value};

const WALK_PROPERTY: &str = "walkthroughs.project";

const WALK: &str = r#"[{
  "format": 1,
  "name": "STO channel 1 opens while lifting",
  "description": "Hold up step 1, open the coil of STO channel 1: the channels disagree and the drive trips.",
  "tail": 0.0,
  "steps": [
    {"t": 1.0, "do": "operate", "device": "=A1+RAD-S1", "operated": true},
    {"t": 4.0, "do": "inject", "with": {"defect": "coil_open", "device": "=A1+CAB-STOA"}},
    {"t": 9.0, "do": "expect", "check": {"what": "winch", "winch": "=A1+FLD-M1", "state": "fault", "code": "STD"}, "note": "the discrepancy latches after its time"},
    {"t": 9.0, "do": "expect", "check": {"what": "winch", "winch": "=A1+FLD-M1", "state": "standing"}}
  ]
}]"#;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("a path for the .novo file as the first argument");
    let kind = args.next();
    let mut doc = Document::default();
    match kind.as_deref() {
        Some("pair") => {
            novo_tests::closed_winch::closed_winch_pair(&mut doc)
                .expect("the two-winch project builds");
        }
        other => {
            novo_tests::closed_winch::closed_winch(&mut doc)
                .expect("the closed-loop winch project builds");
            if other == Some("walk") {
                doc.transact("Walk-through", |tx| {
                    tx.set_info(|i| {
                        i.properties
                            .insert(WALK_PROPERTY.to_owned(), Value::Text(WALK.to_owned()));
                    })
                })
                .expect("the walk-through is saved");
            }
        }
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
