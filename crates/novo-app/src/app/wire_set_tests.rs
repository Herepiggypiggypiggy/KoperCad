use novo_reports::{CutList, Report, WireSet, headline_count, wire_cut_jobs};

use super::gate1_tests::Drive;
use crate::actions::Action;

/// The example's reports count one wire set: 19 connections plus the supply
/// and PE leads are the wires to cut everywhere, labels are two per wire.
#[test]
fn every_report_counts_the_same_wire_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let p = d.app.backend.document().unwrap().project();
    assert_eq!(novo_reports::wire_set_mismatch(p), None);
    let set = WireSet::of(p);
    assert!(set.to_cut >= set.connections, "{set:?}");
    assert_eq!(set.leads(), set.to_cut - set.connections);
    assert_eq!(wire_cut_jobs(p).len(), set.to_cut);
    let (n, noun) = headline_count("cutlist", &CutList::default().generate(p));
    assert_eq!((n, noun), (set.to_cut, "wires"));
    assert!(set.explanation().contains("supply and PE leads"));
}
