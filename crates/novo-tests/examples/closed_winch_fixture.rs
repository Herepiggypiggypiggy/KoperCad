//! Writes the closed-loop winch project (`novo_tests::closed_winch`) for the QA scenarios.
//!
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-pair.novo pair
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-walk.novo walk
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-channel.novo channel
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-ground.novo ground
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-load.novo load
//!   cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch-overload.novo overload
//!
//! `load` has a rated load of 3000 kg (83 % of it hangs on the hook) and a load cell signal device;
//! `overload` a rated load of 2000 kg (125 %, above the 110 % cut-out).
//!
//! `ground` is the single winch with the ground 0.3 m below the hook start: lowering lands the hook
//! and leaves the rope slack. `channel` is the single winch with a monitor for encoder channel B and one saved walk-through
//! that opens its coil and asks for the finding. `walk` is the single winch with one saved walk-through (an STO channel coil opens while the
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

const CHANNEL_WALK: &str = r#"[{
  "format": 1,
  "name": "Encoder channel B opens",
  "description": "Open the coil of the encoder channel B monitor: the run finds the lost channel.",
  "tail": 0.0,
  "steps": [
    {"t": 1.0, "do": "operate", "device": "=A1+RAD-S1", "operated": true},
    {"t": 4.0, "do": "inject", "with": {"defect": "coil_open", "device": "=A1+CAB-ENCH"}},
    {"t": 8.0, "do": "expect", "check": {"what": "winch", "winch": "=A1+FLD-M1", "state": "found", "finding": "encoder_channel_lost"}}
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
            if other == Some("channel") {
                novo_tests::closed_winch::closed_winch_channel(&mut doc)
                    .expect("the winch project with an encoder channel builds");
            } else if other == Some("ground") {
                novo_tests::closed_winch::closed_winch_ground(&mut doc, -0.3)
                    .expect("the winch project with a ground level builds");
            } else if other == Some("load") || other == Some("overload") {
                let swl = if other == Some("load") {
                    3000.0
                } else {
                    2000.0
                };
                novo_tests::closed_winch::closed_winch_load(&mut doc, swl)
                    .expect("the winch project with a load cell builds");
            } else {
                novo_tests::closed_winch::closed_winch(&mut doc)
                    .expect("the closed-loop winch project builds");
            }
            let walk = match other {
                Some("walk") => Some(WALK),
                Some("channel") => Some(CHANNEL_WALK),
                _ => None,
            };
            if let Some(walk) = walk {
                doc.transact("Walk-through", |tx| {
                    tx.set_info(|i| {
                        i.properties
                            .insert(WALK_PROPERTY.to_owned(), Value::Text(walk.to_owned()));
                    })
                })
                .expect("the walk-through is saved");
            }
        }
    }
    novo_storage::ProjectFile::create(std::path::Path::new(&out), doc.project()).expect("save");
    println!("wrote {out}");
}
