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
    // The example's numbers, pinned: a change here is a change of the wire set.
    assert_eq!(
        (
            set.connections,
            set.leads(),
            set.to_cut,
            set.wire_labels,
            set.drawn
        ),
        (19, 6, 25, 50, 27)
    );
    assert!(set.explanation().contains("6 supply and PE leads"));
    assert_eq!(novo_reports::count_text(1, "connections"), "1 connection");
}

/// The Bench output counts wires to cut everywhere: the missing sizes,
/// colours and numbers are counted per connection, whatever number of drawn
/// pieces a connection has.
#[test]
fn the_bench_counts_missing_values_per_wire_to_cut() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let p = d.app.backend.document().unwrap().project();
    let jobs = wire_cut_jobs(p);
    let unset = novo_reports::unset_wires(p);
    assert_eq!(unset.connections, jobs.len());
    assert_eq!(
        unset.connections_no_size,
        jobs.iter()
            .filter(|j| j.cross_section_mm2.is_none())
            .count()
    );
    assert_eq!(
        unset.connections_no_colour,
        jobs.iter().filter(|j| j.colour.trim().is_empty()).count()
    );
    // Drawn pieces can outnumber the wires to cut, and are not what is shown.
    assert!(unset.no_size.len() >= unset.connections_no_size);
    let gaps = super::bench::readiness(&jobs, true);
    let text = gaps
        .iter()
        .find(|g| g.text.contains("no cross-section"))
        .map(|g| g.text.clone())
        .expect("the example has unsized wires");
    assert!(
        text.starts_with(&format!(
            "{} of {} wires to cut",
            unset.connections_no_size,
            jobs.len()
        )),
        "{text}"
    );
    let ready = novo_reports::autopilot::ready(&jobs, true);
    assert_eq!(ready.wires(), jobs.len());
}
