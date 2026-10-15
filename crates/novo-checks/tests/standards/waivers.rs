//! Standards problems in the Issues list, waivers and the selection.

use super::*;
use novo_checks::{ProblemKind, Severity, check, check_open, waive, waived_problems};
use novo_core::*;

fn standard_problems(project: &Project) -> Vec<novo_checks::Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::Standard)
        .collect()
}

#[test]
fn nothing_is_checked_until_a_standard_is_selected() {
    let mut g = golden();
    g.edit(|tx, _| standards::select(tx, &[]));
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    assert!(standard_problems(g.project()).is_empty());
    assert!(g.report().results.is_empty());
    assert!(standards::selected(g.project()).is_empty());
}

#[test]
fn the_selection_is_one_undo_step() {
    let mut g = golden();
    g.edit(|tx, _| {
        standards::select(
            tx,
            &[
                standards::StandardSet::Nfpa79,
                standards::StandardSet::Iec60204,
                standards::StandardSet::Nfpa79,
            ],
        )
    });
    assert_eq!(
        standards::selected(g.project()),
        vec![
            standards::StandardSet::Iec60204,
            standards::StandardSet::Nfpa79
        ]
    );
    g.doc.undo().unwrap();
    assert_eq!(
        standards::selected(g.project()),
        standards::StandardSet::ALL.to_vec()
    );
}

#[test]
fn a_failed_rule_is_a_problem_with_its_rule_and_severity() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    let problems = standard_problems(g.project());
    let disconnect = problems
        .iter()
        .find(|p| p.rule == Some("iec60204.supply-disconnect"))
        .expect("the disconnect problem");
    assert_eq!(disconnect.severity, Severity::Error);
    assert!(disconnect.page.is_some());
    assert!(disconnect.message.contains("=A1+FLD-M1"));
    // IEC and NFPA state the same rule; each reports under its own id.
    assert!(
        problems
            .iter()
            .any(|p| p.rule == Some("nfpa79.supply-disconnect"))
    );
    // A warning rule keeps its severity.
    let w = g.power_wires[1];
    g.style(w, Some("BN"), Some(2.5));
    let colour = standard_problems(g.project())
        .into_iter()
        .find(|p| p.rule == Some("iec60204.conductor-colour"))
        .unwrap();
    assert_eq!(colour.severity, Severity::Warning);
}

#[test]
fn unknowns_are_not_problems() {
    let mut g = golden();
    let (w, _) = (g.power_wires[1], ());
    g.style(w, None, Some(2.5));
    assert!(standard_problems(g.project()).is_empty());
    assert_unknown(&g, "iec60204.conductor-colour", "no colour set");
}

#[test]
fn a_waiver_covers_one_rule_and_prints_its_reason() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    let problems = standard_problems(g.project());
    let iec = problems
        .iter()
        .find(|p| p.rule == Some("iec60204.supply-disconnect"))
        .unwrap()
        .clone();
    let nfpa_before = standard_problems(g.project()).len();
    g.edit(|tx, _| waive(tx, &iec, "  Machine is supplied from a plug and socket  ").map(|_| ()));
    // The IEC problem is waived; the NFPA one on the same objects is not.
    let open = check_open(g.project());
    assert!(
        !open
            .iter()
            .any(|p| p.rule == Some("iec60204.supply-disconnect"))
    );
    assert!(
        open.iter()
            .any(|p| p.rule == Some("nfpa79.supply-disconnect"))
    );
    assert_eq!(
        standard_problems(g.project()).len(),
        nfpa_before,
        "waived problems stay in check()"
    );
    let waived = waived_problems(g.project());
    assert_eq!(waived.len(), 1);
    assert_eq!(
        waived[0].reason,
        "Machine is supplied from a plug and socket"
    );
    // The rule's status and the report say so.
    let r = g.result("iec60204.supply-disconnect");
    assert_eq!(r.status, RuleStatus::Waived);
    assert_eq!(
        r.waived[0].reason,
        "Machine is supplied from a plug and socket"
    );
    let lines = standards::lines(g.project(), &g.report());
    assert!(
        lines
            .iter()
            .any(|(_, l)| l.contains("[WAIVED] 5.3 Supply disconnecting device"))
    );
    assert!(
        lines
            .iter()
            .any(|(_, l)| l.contains("Reason: Machine is supplied from a plug and socket"))
    );
    // The design checks report prints it too.
    let design = novo_checks::report_lines(g.project());
    assert!(
        design
            .iter()
            .any(|(_, l)| l.contains("Reason: Machine is supplied from a plug and socket"))
    );
}

#[test]
fn a_project_wide_finding_can_be_waived() {
    let mut g = golden();
    let s0_sym = g.s0_sym;
    g.rekey(s0_sym, "iec/operators/pushbutton_nc");
    g.clear(g.s0, "function");
    let p = standard_problems(g.project())
        .into_iter()
        .find(|p| p.rule == Some("iec60204.estop-present"))
        .unwrap();
    assert!(
        !p.at.is_empty(),
        "marked on the first page so it can be jumped to and waived"
    );
    g.edit(|tx, _| waive(tx, &p, "Hand-fed press, risk assessment RA-12").map(|_| ()));
    assert_eq!(g.status("iec60204.estop-present"), RuleStatus::Waived);
}

#[test]
fn a_waiver_goes_when_the_problem_changes() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    let p = standard_problems(g.project())
        .into_iter()
        .find(|p| p.rule == Some("iec60204.supply-disconnect"))
        .unwrap();
    g.edit(|tx, _| waive(tx, &p, "reason").map(|_| ()));
    assert_eq!(g.status("iec60204.supply-disconnect"), RuleStatus::Waived);
    // A second motor behind the same fuses makes it a different problem.
    let page = g.page;
    g.edit(|tx, h| {
        let m2 = tx.add_device(tag("=A1+FLD-M2"), "Second motor")?;
        let mut d = Draw::new(tx, page);
        let sym = d.sym(
            "iec/motors/motor_3ph",
            m2,
            true,
            &["U1", "V1", "W1", "PE"],
            &[],
            &[],
        );
        d.wire(h.f1_sym, 1, sym, 0);
        Ok(())
    });
    assert_eq!(g.status("iec60204.supply-disconnect"), RuleStatus::Fail);
}
