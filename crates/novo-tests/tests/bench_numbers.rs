//! The bench path of the motor starter: the numbers a person reads off the
//! cut list, and which wires the list holds.

use novo_core::Document;
use novo_numbering::{Mode, Scheme, SchemeKind, Scope, plan};
use novo_reports::wire_cut_jobs;
use novo_tests::motor_starter;

fn numbered() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let scheme = Scheme::new(SchemeKind::Sequential);
    let p = plan(doc.project(), &scheme, Mode::Fill, &Scope::Project);
    doc.transact("Number wires", |tx| p.apply(tx)).unwrap();
    doc
}

/// Motor-starter QA bug 3: the wire into the overload relay and the wire out
/// of it carried one number, so the cut list had two wires with one number.
/// Two wires may share a number only where they meet on one pin (or a
/// terminal); the heater of an overload relay is no such place.
#[test]
fn no_two_cut_list_wires_share_a_number_across_a_device() {
    let doc = numbered();
    let jobs = wire_cut_jobs(doc.project());
    let ends =
        |j: &novo_reports::WireCutJob| [j.end_a.designation.clone(), j.end_b.designation.clone()];
    for (i, a) in jobs.iter().enumerate() {
        for b in &jobs[i + 1..] {
            if a.number.is_empty() || a.number != b.number {
                continue;
            }
            let shared: Vec<String> = ends(a)
                .into_iter()
                .filter(|e| ends(b).contains(e))
                .collect();
            assert!(
                !shared.is_empty(),
                "number {} on two wires that meet at {shared:?}: {:?} and {:?}",
                a.number,
                ends(a),
                ends(b)
            );
        }
    }
}

/// Motor-starter QA bug 4: the supply wires and the motor's earth wire are
/// wires to cut. They are on the list, each with a number.
#[test]
fn the_cut_list_holds_the_feed_wires_and_the_motor_earth() {
    let doc = numbered();
    let jobs = wire_cut_jobs(doc.project());
    for feed in ["L1 (feed)", "L2 (feed)", "L3 (feed)", "PE (feed)"] {
        let job = jobs
            .iter()
            .find(|j| j.end_a.designation == feed || j.end_b.designation == feed)
            .unwrap_or_else(|| panic!("no cut wire for {feed}"));
        assert!(!job.number.is_empty(), "{feed} wire has no number");
    }
}
